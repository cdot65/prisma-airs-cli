//! The one withholding policy of every printed gateway document (DESIGN §19.3 "Secrets and
//! the withholding policy"; D238, D251, G43).
//!
//! In order: the SDK secret rules of the resource's operations (rules 1 and 2, read from
//! the SDK's public `AI_GATEWAY_SECRET_FIELDS`, never copied, with the CLI's few additions),
//! the name rule at any depth (rule 3), the URL rule (rule 4), then the `SecretSet` rule (a
//! string that carries a registered secret is withheld and a member whose name carries one is
//! dropped). What a rule withholds joins the `SecretSet` first (rule 3g), so the same value
//! anywhere else in the document, or in a later message, is withheld too.
//!
//! Every decision on a name, a pair, a header, a schema or a string is made by the rules
//! module [`super::withhold_rules`] (choices 172, 185), which the live tier compiles too: a
//! member's [`Class`] comes from the one table `names::NAME_RULES`, and the class and the
//! value's shape give its [`Action`] (`names::action`, `names::header_action`), which [`take`]
//! carries out on an [`OValue`]. This module holds the traversals that rewrite a document and
//! collect what they withhold, the SDK path rules and the registration; the rules module's
//! `check::withheld` (test builds) reads a printed document by the same tables, and the tests run it on
//! everything this module prints.

use super::withhold_rules::names::{
    self, AUDIT_FIELDS, Action, Class, Leaf, PAIR_VALUE, Pair, SCHEMA_KEYWORDS,
    SCHEMA_VALUE_KEYWORDS, SUBSCHEMA_KEYWORDS, action, classify, header_action, header_class,
    header_item_action, is_header_map, member_class,
};
use super::withhold_rules::values::{
    has_private_key, looks_like_json_text, model_output_withheld, shows_credential, text_withheld,
    unprinted_credentials, unreadable_json_strings,
};
use super::withhold_rules::view::{self, Json, Kind};
use crate::error::{SecretSet, WITHHELD};
use crate::json::OValue;
use scm::constants::{AI_GATEWAY_SECRET_FIELDS, AIGatewaySecretOperation as Op};

/// The CLI's order-keeping value as the rules read its shapes (and the checker, in tests).
impl Json for OValue {
    fn kind(&self) -> Kind {
        match self {
            Self::Null => Kind::Null,
            Self::Bool(_) => Kind::Bool,
            Self::Number(_) => Kind::Number,
            Self::String(_) => Kind::String,
            Self::Array(_) => Kind::Array,
            Self::Object(_) => Kind::Object,
        }
    }
    fn text(&self) -> Option<&str> {
        self.as_str()
    }
    fn items(&self) -> Vec<&Self> {
        self.as_array()
            .map(|items| items.iter().collect())
            .unwrap_or_default()
    }
    fn members(&self) -> Vec<(&str, &Self)> {
        self.as_object()
            .map(|entries| {
                entries
                    .iter()
                    .map(|(name, member)| (name.as_str(), member))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The CLI's value parses a JSON text as the CLI reads one (the checker's reading in tests).
#[cfg(test)]
impl super::withhold_rules::check::Document for OValue {
    fn document(text: &str) -> Option<Self> {
        json_document(text)
    }
}

/// One secret path: its members in order, `*` for any member or array item (the SDK's
/// path form).
type Path = &'static [&'static str];

/// What one document withholds besides the rules every document shares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    /// The SDK operations of the resource: every request and response rule of each in
    /// `AI_GATEWAY_SECRET_FIELDS` is applied to the document and to every row of a list
    /// document (a `Subtree` rule and a `Value` rule both replace the member whole).
    pub operations: &'static [Op],
    /// The CLI's own paths, beyond the SDK's (applied the same way).
    pub paths: &'static [Path],
}

impl Policy {
    /// The SDK paths of [`Self::operations`], then [`Self::paths`].
    fn rule_paths(&self) -> impl Iterator<Item = Path> + '_ {
        sdk_paths(self.operations).chain(self.paths.iter().copied())
    }
}

/// Every request and response path the SDK gives `operations`, in the SDK's order.
fn sdk_paths(operations: &[Op]) -> impl Iterator<Item = Path> + '_ {
    AI_GATEWAY_SECRET_FIELDS
        .iter()
        .filter(|(operation, _)| operations.contains(operation))
        .flat_map(|(_, rules)| rules.request.iter().chain(rules.response))
        .map(|rule| rule.path)
}

/// The shared rules only (workspaces, scopes, the telemetry charts and logs).
pub const PLAIN: Policy = Policy {
    operations: &[],
    paths: &[],
};

/// `telemetry filter-boundaries`: the whole `data` (the SDK calls its values possibly
/// sensitive; TS replaces it by `[REDACTED]`, choice 18).
pub const FILTER_BOUNDARIES: Policy = Policy {
    operations: &[Op::TelemetryFilterBoundaries],
    paths: &[],
};

/// API key reads and writes: the SDK's one-time `key` (the name rule reaches every depth,
/// TS `redactApiKeyMaterial`).
pub const API_KEYS: Policy = Policy {
    operations: &[
        Op::ApiKeysCreateService,
        Op::ApiKeysCreateUser,
        Op::ApiKeysRotateService,
        Op::ApiKeysRotateUser,
    ],
    paths: &[],
};

/// Workspace provider bindings (the SDK's `providers.get` response paths).
pub const PROVIDERS: Policy = Policy {
    operations: &[Op::ProvidersGet],
    paths: &[],
};

/// Organisation provider integrations (the SDK's `integrations.*` paths).
pub const INTEGRATIONS: Policy = Policy {
    operations: &[Op::IntegrationsCreate, Op::IntegrationsUpdate],
    paths: &[],
};

/// MCP integrations (the SDK's `mcpIntegrations.*` paths).
pub const MCP: Policy = Policy {
    operations: &[Op::McpIntegrationsCreate, Op::McpIntegrationsUpdate],
    paths: &[],
};

/// Organisation authentication settings (`organisations.getAuthSettings` and
/// `updateAuthSettings`; TS withholds only the SCIM token), plus the SSO client secret of
/// an enveloped answer (`data.auth_settings.client_secret`, a CLI addition).
pub const AUTH_SETTINGS: Policy = Policy {
    operations: &[
        Op::OrganisationsGetAuthSettings,
        Op::OrganisationsUpdateAuthSettings,
    ],
    paths: &[&["data", "auth_settings", "client_secret"]],
};

/// Deployment registrations (`deployments.*`: the registration credentials).
pub const DEPLOYMENTS: Policy = Policy {
    operations: &[Op::DeploymentsCreate, Op::DeploymentsUpdate],
    paths: &[],
};

/// Gateway plugins: every credential value (`plugins.create`).
pub const PLUGINS: Policy = Policy {
    operations: &[Op::PluginsCreate],
    paths: &[],
};

/// `organisations info`: the whole `settings` (TS `organisations.getInfo`, choice 18).
pub const ORGANISATION_INFO: Policy = Policy {
    operations: &[Op::OrganisationsGetInfo],
    paths: &[],
};

/// Every resource policy (the audit rule's CLI additions and the SDK coverage test).
const POLICIES: [&Policy; 10] = [
    &PLAIN,
    &FILTER_BOUNDARIES,
    &API_KEYS,
    &PROVIDERS,
    &INTEGRATIONS,
    &MCP,
    &AUTH_SETTINGS,
    &DEPLOYMENTS,
    &PLUGINS,
    &ORGANISATION_INFO,
];

/// The SDK operations no command reads or writes: their bodies reach the CLI only inside
/// an audit record, where the audit rule applies their paths (secret references: the CLI
/// has no `secret-references` command, TS has none either).
#[cfg(test)]
const AUDIT_ONLY: [Op; 5] = [
    Op::SecretReferencesList,
    Op::SecretReferencesCreate,
    Op::SecretReferencesGet,
    Op::SecretReferencesUpdate,
    Op::SecretReferencesDelete,
];

/// The SDK operations whose paths the audit rule does not apply: `telemetry.filterBoundaries`
/// is a data-plane read the admin audit log never records, and its one rule names the
/// response envelope's `data`, which would withhold every revealed body's `data` whole.
const NOT_AUDITED: [Op; 1] = [Op::TelemetryFilterBoundaries];

/// The secret paths of every gateway resource (rule 5): the request and response paths of
/// every SDK operation but [`NOT_AUDITED`], then every policy's own paths. Applied inside a
/// revealed audit field at its root and under its `data` member.
fn audit_paths() -> impl Iterator<Item = Path> {
    AI_GATEWAY_SECRET_FIELDS
        .iter()
        .filter(|(operation, _)| !NOT_AUDITED.contains(operation))
        .flat_map(|(_, rules)| rules.request.iter().chain(rules.response))
        .map(|rule| rule.path)
        .chain(
            POLICIES
                .iter()
                .flat_map(|policy| policy.paths.iter().copied()),
        )
}

/// Apply `policy` and the shared rules to `doc` (in place).
pub fn apply(doc: &mut OValue, policy: &Policy, secrets: &SecretSet) {
    let mut withheld = Vec::new();
    let paths: Vec<Path> = policy.rule_paths().collect();
    if !paths.is_empty() {
        let mut apply_paths = |target: &mut OValue| {
            for path in &paths {
                at_path(target, path, Paths::Resource, &mut withheld);
            }
        };
        match &mut *doc {
            OValue::Array(rows) => rows.iter_mut().for_each(&mut apply_paths),
            other => apply_paths(other),
        }
    }
    names_and_urls(doc, &mut withheld);
    register(withheld, secrets);
    crate::secrets::withhold(doc, secrets);
}

/// The policy of model output (an inference answer or stream event, §19.14): only the rules
/// module's model-output text rules (the URL rule and the scheme rule, choices 177, 186) and the
/// `SecretSet` rule. The answer
/// is not a service resource, so the name rule does not apply (a logprobs `token` is a word
/// of the answer), nor the header and key=value rules, which read names inside a text (an
/// answer's code sample `password=os.environ[…]` or `"api-key": os.environ[…]`, choices 181,
/// 183), and nothing it shows is registered.
pub fn apply_model_output(doc: &mut OValue, secrets: &SecretSet) {
    value_rules(doc);
    crate::secrets::withhold(doc, secrets);
}

/// The text rules of model output at any depth ([`model_output_withheld`]: the URL rule and
/// the scheme rule; the withheld parts are not registered).
fn value_rules(value: &mut OValue) {
    match value {
        OValue::Object(entries) => entries
            .iter_mut()
            .for_each(|(_, member)| value_rules(member)),
        OValue::Array(items) => items.iter_mut().for_each(value_rules),
        OValue::String(text) => {
            if let Some(shown) = model_output_withheld(text) {
                *text = shown;
            }
        }
        _ => {}
    }
}

/// The rules that withhold part of a string in place (D251, choices 177, 181, 183, 186), under any
/// name, as the rules module's [`text_withheld`] applies them: the URL rule, the scheme rule (a
/// credential after `Basic`, `Bearer` or `Token`), the header rule (`X-Api-Key: …`) and the
/// key=value rule (`Password=…;`, `token=sk-…`). What they say to register joins `withheld`:
/// a URL's userinfo, query and fragment and the credential-named values inside a query, a
/// credential header's value, a credential-named key's value that can be a credential by its
/// shape, and a scheme credential after `Bearer`, an `Authorization` header name or as a header
/// value; a word after `token` or `basic` in prose, `auth=none` or `auth=oauth2`, is withheld in
/// this text only, so it does not withhold that word everywhere else.
fn in_place(text: &mut String, withheld: &mut Vec<String>) {
    if let Some((shown, parts)) = text_withheld(text) {
        withheld.extend(parts);
        *text = shown;
    }
}

/// The audit-log rule (rule 5; G44, D239), then [`apply`] with the shared rules: in every
/// record of `rows`, `request_body`, `request_headers` and `query_params` are
/// `[withheld]`; with `reveal`, each one that is or parses as a JSON object or array is
/// printed in its own form (compact JSON text, or the object or array) with the name and
/// URL rules applied inside it, and any other value stays withheld ([`audit_field`]).
pub fn apply_audit(rows: &mut OValue, reveal: bool, secrets: &SecretSet) {
    let mut withheld = Vec::new();
    if let OValue::Array(records) = rows {
        for record in records {
            let OValue::Object(entries) = record else {
                continue;
            };
            for (name, member) in entries.iter_mut() {
                if AUDIT_FIELDS.contains(&name.as_str()) {
                    let headers = name == "request_headers";
                    audit_field(member, headers, reveal, &mut withheld);
                }
            }
        }
    }
    register(withheld, secrets);
    apply(rows, &PLAIN, secrets);
}

/// One audit request field (rule 5): the credentials of a JSON document it holds (as text,
/// or as an object or array) are registered by [`audit_document`] whether or not it is
/// shown (choice 80); with `reveal` the field is printed as that withheld document, in the
/// form it arrived in (a JSON text as compact JSON text, an object or array as that object
/// or array; choice 162); otherwise, and for any other value (a number or a boolean too,
/// choice 171), the field is `[withheld]`. A withheld field is never printed, so only the
/// credentials inside it join the `SecretSet` (choice 187): a JSON document's by
/// [`audit_document`], another text's by [`unprinted_credentials`], a number's when it looks
/// like a credential. `null` stays. `headers`: the field is itself a header map
/// (`request_headers`).
fn audit_field(member: &mut OValue, headers: bool, reveal: bool, withheld: &mut Vec<String>) {
    let document = match &*member {
        OValue::String(text) => json_document(text),
        OValue::Object(_) | OValue::Array(_) => Some(member.clone()),
        _ => None,
    };
    if let Some(mut document) = document {
        audit_document(&mut document, headers, withheld);
        if reveal {
            if !matches!(member, OValue::String(_)) {
                *member = document;
                return;
            }
            if let Ok(text) = crate::json::to_compact(&document) {
                *member = OValue::String(text);
                return;
            }
        }
    }
    match &*member {
        OValue::Null => return,
        OValue::String(text) if text == WITHHELD => return,
        OValue::String(text) => {
            if json_document(text).is_none() {
                withheld.extend(unprinted_credentials(text));
            }
        }
        OValue::Number(number) => {
            withheld.extend(names::registered_number(Leaf::Inner, &number.to_string()));
        }
        _ => {}
    }
    *member = OValue::String(WITHHELD.to_owned());
}

/// One walker of the name rules: [`names_and_urls`] (rule 3, every document) or
/// [`audit_walk`] (rule 5, inside a revealed audit field).
type Walk = fn(&mut OValue, &mut Vec<String>);

/// A revealed audit field (rule 5): the secret paths of every gateway resource at its root
/// and under `data` (rule 2, [`Paths::Audit`]: a path that ends at a generic name, the
/// SDK's `key`, withholds as the name rule does there, choice 141, since the audit applies
/// every resource's paths to every body), then [`audit_walk`]; `headers`: the field is
/// itself a header map (`request_headers`).
fn audit_document(doc: &mut OValue, headers: bool, withheld: &mut Vec<String>) {
    if headers {
        header_map(doc, audit_walk, withheld);
        return;
    }
    for path in audit_paths() {
        let mut at = |target: &mut OValue| match target {
            OValue::Array(rows) => rows
                .iter_mut()
                .for_each(|row| at_path(row, path, Paths::Audit, withheld)),
            other => at_path(other, path, Paths::Audit, withheld),
        };
        at(doc);
        if let OValue::Object(entries) = doc
            && let Some((_, data)) = entries.iter_mut().find(|(name, _)| name == "data")
        {
            at(data);
        }
    }
    audit_walk(doc, withheld);
}

/// The audit name rules at any depth: every member by [`withhold_named`], a `{key|name,
/// value}` pair read as one header whose value is shown only for a safe header (choice 132:
/// a revealed request is the stricter reading), a string holding a JSON object or array
/// withheld inside like a revealed body, a private key and the URL rule ([`withhold_text`]).
/// There is no schema sub-rule: a logged request body holds values, not schemas.
fn audit_walk(value: &mut OValue, withheld: &mut Vec<String>) {
    match value {
        OValue::Object(entries) => {
            let pair = pair_of(entries);
            let located = located(entries);
            for (name, member) in entries.iter_mut() {
                match &pair {
                    Some(pair) if pair.names_member(name, member.as_str()) => {
                        audit_walk(member, withheld);
                    }
                    Some(pair) if name == PAIR_VALUE => take(
                        header_action(pair.class(), view::shape(member)),
                        member,
                        audit_walk,
                        withheld,
                    ),
                    _ => withhold_named(name, located, member, audit_walk, withheld),
                }
            }
        }
        OValue::Array(items) => items.iter_mut().for_each(|item| audit_walk(item, withheld)),
        OValue::String(_) => withhold_text(
            value,
            |doc, withheld| audit_document(doc, false, withheld),
            withheld,
        ),
        _ => {}
    }
}

/// Carry out `action` on `member` (DESIGN §19.3 "Rule 3 by name"): read it by `walk`, read
/// it as a header map, print it, or withhold it and register it as the [`Leaf`] says.
fn take(action: Action, member: &mut OValue, walk: Walk, withheld: &mut Vec<String>) {
    match action {
        Action::Read => walk(member, withheld),
        Action::Headers => header_map(member, walk, withheld),
        Action::Keep => {}
        Action::Withhold(leaf) => withhold_value(leaf, member, withheld),
    }
}

/// One member under its name, by the classification and the action table every walker
/// shares: its [`Class`] ([`member_class`]; `located`: the member sits in a secret mapping or
/// a secret reference, where `secret_key` is a location, choice 164) and its value's shape.
fn withhold_named(
    name: &str,
    located: bool,
    member: &mut OValue,
    walk: Walk,
    withheld: &mut Vec<String>,
) {
    let action = action(member_class(name, located), view::shape(member));
    take(action, member, walk, withheld);
}

/// The [`Pair`] the object `entries` is, if it is one (choices 159, 170).
fn pair_of(entries: &[(String, OValue)]) -> Option<Pair> {
    Pair::of(
        entries
            .iter()
            .map(|(name, member)| (name.as_str(), member.as_str())),
    )
}

/// Whether the members `entries` make an object a secret mapping or a secret reference
/// ([`names::is_secret_location`]).
fn located(entries: &[(String, OValue)]) -> bool {
    names::is_secret_location(entries.iter().map(|(name, _)| name.as_str()))
}

/// A string by the rules every walker shares: one holding a JSON object or array is withheld
/// inside by `inner` ([`within_json_text`]); one holding a private key is withheld and
/// registered whole (choice 170); any other gets the URL, scheme and key=value rules in place
/// ([`in_place`]).
fn withhold_text(value: &mut OValue, inner: Walk, withheld: &mut Vec<String>) {
    if within_json_text(value, inner, withheld) {
        return;
    }
    let OValue::String(text) = value else {
        return;
    };
    if has_private_key(text) {
        register_leaf(Leaf::Credential, text, withheld);
        blank(value);
    } else {
        in_place(text, withheld);
    }
}

/// The JSON object or array a string holds as text, withheld inside by `inner`: when that
/// changes it the text becomes the withheld document (compact JSON), otherwise it stays as
/// received (choices 81, 138). A text shaped as one that does not parse is withheld whole
/// and its strings registered ([`unreadable_json_strings`], choice 167). `false` when the
/// text is not shaped as a JSON document.
fn within_json_text(
    value: &mut OValue,
    inner: impl FnOnce(&mut OValue, &mut Vec<String>),
    withheld: &mut Vec<String>,
) -> bool {
    let Some(text) = value.as_str() else {
        return false;
    };
    let Some(mut parsed) = json_document(text) else {
        if !looks_like_json_text(text) {
            return false;
        }
        withheld.extend(unreadable_json_strings(text));
        blank(value);
        return true;
    };
    let received = parsed.clone();
    inner(&mut parsed, withheld);
    if parsed != received {
        match crate::json::to_compact(&parsed) {
            Ok(compact) => *value = OValue::String(compact),
            Err(_) => blank(value),
        }
    }
    true
}

/// A header map (rule 3c): its header names printed and every value as
/// [`names::header_action`] says of its header's class ([`names::header_class`]): a safe
/// header's read by `walk` (the document's own rules), any other withheld. An object is a
/// map, or a `{key|name|header, value}` pair when it is one (its names read by `walk`, its
/// value as the pair's header, any other member as a header); an array holds maps, pairs,
/// header names (read by `walk`, choice 177) or values ([`names::header_item_action`]); a
/// string holding a JSON document is read inside (printed as that compact document when
/// something was withheld); any other string is withheld whole.
fn header_map(value: &mut OValue, walk: Walk, withheld: &mut Vec<String>) {
    match value {
        OValue::Object(entries) => {
            let pair = pair_of(entries);
            for (name, member) in entries.iter_mut() {
                let class = match &pair {
                    Some(pair) if pair.names_member(name, member.as_str()) => {
                        walk(member, withheld);
                        continue;
                    }
                    Some(pair) if name == PAIR_VALUE => pair.class(),
                    _ => header_class(name),
                };
                take(
                    header_action(class, view::shape(member)),
                    member,
                    walk,
                    withheld,
                );
            }
        }
        OValue::Array(items) => {
            for item in items {
                if matches!(item, OValue::Object(_)) {
                    header_map(item, walk, withheld);
                } else {
                    let action = header_item_action(item.as_str(), view::shape(item));
                    take(action, item, walk, withheld);
                }
            }
        }
        OValue::String(_) => {
            let inner =
                |doc: &mut OValue, withheld: &mut Vec<String>| header_map(doc, walk, withheld);
            if !within_json_text(value, inner, withheld) {
                withhold_value(Leaf::Generic, value, withheld);
            }
        }
        _ => {}
    }
}

/// The [`Leaf`] a header value of `class` registers as: a credential's value for a
/// credential or count header, a generic value for any other (choices 139, 142).
fn header_leaf(class: Class) -> Leaf {
    match class {
        Class::Count | Class::Credential | Class::Member => Leaf::Credential,
        _ => Leaf::Generic,
    }
}

/// Keep for the `SecretSet` what `leaf` says of the withheld string `text`
/// ([`names::registered`], the one registration rule every walker uses; choice 154).
fn register_leaf(leaf: Leaf, text: &str, withheld: &mut Vec<String>) {
    withheld.extend(names::registered(leaf, text));
}

/// A value withheld whole (`[withheld]`; `null` and booleans stay, and an earlier rule's
/// `[withheld]` is kept): a plain string registered as `leaf` says ([`register_leaf`]); a
/// number registered when it is long enough ([`names::registered_number`], choice 171); an
/// object, an array or a JSON document held as text (a stringified service account)
/// registers its credentials ([`credentials_inside`]), a text also as `leaf` says.
fn withhold_value(leaf: Leaf, value: &mut OValue, withheld: &mut Vec<String>) {
    match value {
        OValue::Null | OValue::Bool(_) => return,
        OValue::String(text) if text == WITHHELD => return,
        OValue::String(text) => {
            json_text_credentials(text, withheld);
            register_leaf(leaf, text, withheld);
        }
        OValue::Number(number) => {
            withheld.extend(names::registered_number(leaf, &number.to_string()));
        }
        OValue::Object(_) | OValue::Array(_) => credentials_inside(value, withheld),
    }
    *value = OValue::String(WITHHELD.to_owned());
}

/// `value` replaced by `[withheld]` when it is a string, an object or an array, with nothing
/// registered (its credentials, if any, were registered by the caller).
fn blank(value: &mut OValue) {
    if matches!(
        value,
        OValue::String(_) | OValue::Object(_) | OValue::Array(_)
    ) {
        *value = OValue::String(WITHHELD.to_owned());
    }
}

/// The credentials of a withheld object, array or JSON document (rule 3g; `doc` is consumed,
/// its caller replaces it): every value the name and URL rules withhold inside it
/// ([`names_and_urls`], which registers as each rule says), then every other string inside
/// it by [`inner_leaves`] (a string an earlier rule changed is not read again).
fn credentials_inside(doc: &mut OValue, withheld: &mut Vec<String>) {
    names_and_urls(doc, withheld);
    inner_leaves(doc, false, withheld);
}

/// Every string of a withheld value that no rule changed, by [`register_leaf`]: under a
/// describing member ([`SCHEMA_KEYWORDS`]) of a schema object ([`view::is_schema`], at any
/// level) as [`Leaf::Describing`], any other as [`Leaf::Inner`] (choice 151: a short password
/// beside the user name of a credential object is registered, a schema's words are not).
fn inner_leaves(value: &OValue, describing: bool, withheld: &mut Vec<String>) {
    match value {
        OValue::String(text) if !text.contains(WITHHELD) => {
            let leaf = if describing {
                Leaf::Describing
            } else {
                Leaf::Inner
            };
            register_leaf(leaf, text, withheld);
        }
        OValue::Array(items) => items
            .iter()
            .for_each(|item| inner_leaves(item, describing, withheld)),
        OValue::Object(entries) => {
            let schema = view::is_schema(value);
            for (name, member) in entries {
                let describing = describing || (schema && SCHEMA_KEYWORDS.contains(&name.as_str()));
                inner_leaves(member, describing, withheld);
            }
        }
        _ => {}
    }
}

/// Every string inside `value` that [`shows_credential`] replaced by `[withheld]` in place,
/// nothing registered (rule 3e: the schema describes the credential, it does not hold it).
fn withhold_described_credentials(value: &mut OValue) {
    match value {
        OValue::String(text) if shows_credential(text) => blank(value),
        OValue::Array(items) => items.iter_mut().for_each(withhold_described_credentials),
        OValue::Object(entries) => entries
            .iter_mut()
            .for_each(|(_, member)| withhold_described_credentials(member)),
        _ => {}
    }
}

/// The JSON object or array a string holds as text (a config's `config`, a provider's
/// stringified service-account JSON); `None` for any other text. Only a text that starts
/// (after whitespace) with `{` or `[` is parsed.
fn json_document(text: &str) -> Option<OValue> {
    if !text.trim_start().starts_with(['{', '[']) {
        return None;
    }
    crate::json::parse(text)
        .ok()
        .filter(|parsed| matches!(parsed, OValue::Object(_) | OValue::Array(_)))
}

/// The credentials of a JSON document held as text that is withheld whole: a document's by
/// [`credentials_inside`], a text shaped as one that does not parse by
/// [`unreadable_json_strings`] (choice 167); nothing for any other text.
fn json_text_credentials(text: &str, withheld: &mut Vec<String>) {
    if let Some(mut parsed) = json_document(text) {
        credentials_inside(&mut parsed, withheld);
    } else if looks_like_json_text(text) {
        withheld.extend(unreadable_json_strings(text));
    }
}

/// Every string the rules registered joins the `SecretSet`. Which strings may be registered
/// is the rules module's decision (`names::registrable`: at least `names::MIN_REGISTERED`
/// characters, never the placeholder, a placeholder word or a mask; choices 69, 144, 183):
/// every rules function that hands this module a registration applies it, and it is read
/// once more here so a registration can never bypass it.
fn register(withheld: Vec<String>, secrets: &SecretSet) {
    for value in withheld.iter().filter(|value| names::registrable(value)) {
        secrets.add(value);
    }
}

/// Where a path rule applies (rule 2): a resource document's own paths, or every resource's
/// paths inside a revealed audit field (rule 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Paths {
    Resource,
    Audit,
}

/// The values at `path` (`*` any member or array item, as the SDK reads it) withheld as
/// [`path_leaf`] says, a number too (choice 171); a path that ends in a header map's `*`
/// (`configurations.custom_headers.*`) withholds every value of that map, each registered as
/// a header value ([`header_values`], choice 139).
fn at_path(value: &mut OValue, path: &[&str], paths: Paths, withheld: &mut Vec<String>) {
    let Some((first, rest)) = path.split_first() else {
        return;
    };
    match value {
        OValue::Object(entries) => {
            for (name, member) in entries.iter_mut() {
                if *first != "*" && name != first {
                    continue;
                }
                if rest.is_empty() {
                    withhold_value(path_leaf(name, *first == "*", paths), member, withheld);
                } else if rest == ["*"] && is_header_map(name) {
                    header_values(member, withheld);
                } else {
                    at_path(member, rest, paths, withheld);
                }
            }
        }
        OValue::Array(items) if *first == "*" => {
            for item in items {
                if rest.is_empty() {
                    withhold_value(Leaf::Path, item, withheld);
                } else {
                    at_path(item, rest, paths, withheld);
                }
            }
        }
        _ => {}
    }
}

/// How a path rule withholds the member `name` it ends at (rule 2): a named member as a path
/// rule's value, whole ([`Leaf::Path`]), the SDK naming it a credential (inside an audit field
/// a generic name, the SDK's `key`, as the name rule withholds it there, choice 141); a member
/// a final `*` reaches by its own name (choice 153): a credential or count name's as a path
/// rule's value, a generic name's as a generic value, any other as a string inside a
/// credential ([`Leaf::Inner`]), so the `region` beside a plugin's `credentials.api_key` does
/// not withhold every string carrying `us-east-1`. An array item a final `*` reaches is a path
/// rule's value.
fn path_leaf(name: &str, wildcard: bool, paths: Paths) -> Leaf {
    match classify(name) {
        Class::Count | Class::Credential | Class::Member => Leaf::Path,
        Class::Generic if paths == Paths::Resource => Leaf::Path,
        Class::Generic => Leaf::Generic,
        Class::Plain | Class::Fingerprint | Class::HeaderMap if wildcard => Leaf::Inner,
        Class::Plain | Class::Fingerprint | Class::HeaderMap => Leaf::Path,
    }
}

/// Every value of a header map a path rule names (`custom_headers.*`), safe headers
/// included, withheld whole; each is registered as its header's class says ([`header_leaf`]),
/// a `{key|name, value}` item's value under its pair's names (the names themselves are never
/// registered, choice 132).
fn header_values(map: &mut OValue, withheld: &mut Vec<String>) {
    match map {
        OValue::Object(entries) => {
            for (name, member) in entries.iter_mut() {
                withhold_value(header_leaf(header_class(name)), member, withheld);
            }
        }
        OValue::Array(items) => {
            for item in items {
                let pair = match item {
                    OValue::Object(entries) => pair_of(entries),
                    _ => None,
                };
                let Some(pair) = pair else {
                    withhold_value(header_leaf(header_class("")), item, withheld);
                    continue;
                };
                // A pair: its value registered as a header value, its names never.
                if let OValue::Object(entries) = item {
                    for (name, member) in entries.iter_mut() {
                        if name == PAIR_VALUE {
                            withhold_value(header_leaf(pair.class()), member, withheld);
                        }
                    }
                }
                blank(item);
            }
        }
        _ => {}
    }
}

/// The name rule (rule 3) and the URL rule (rule 4) at any depth.
///
/// Every member by [`withhold_named`] (the classification and the action table); a
/// `{key|name, value}` pair (a tag, a header or environment list) read as one header, its
/// names printed and its value withheld as a header value when a name is a credential header
/// ([`Pair::class_outside_headers`], choices 159, 170), else read by these rules; a
/// `properties` object read as a JSON Schema property map ([`property_map`], rule 3e); a
/// string by [`withhold_text`] (a JSON document inside it withheld by the same rules, rule
/// 3f; a private key; the URL rule). What each rule withholds is registered by rule 3g, but
/// a member's value that repeats a sibling `*_type` or `*_mode` and names a kind of credential
/// (`names::repeats_a_mode`, choice 188), which is withheld in place only.
fn names_and_urls(value: &mut OValue, withheld: &mut Vec<String>) {
    match value {
        OValue::Object(entries) => {
            let pair = pair_of(entries);
            let located = located(entries);
            let owned_modes: Vec<String> = names::mode_values(
                entries
                    .iter()
                    .map(|(name, member)| (name.as_str(), member.as_str())),
            )
            .into_iter()
            .map(str::to_owned)
            .collect();
            let modes: Vec<&str> = owned_modes.iter().map(String::as_str).collect();
            for (name, member) in entries.iter_mut() {
                let mode = member
                    .as_str()
                    .filter(|text| names::repeats_a_mode(text, &modes))
                    .map(str::to_owned);
                let before = withheld.len();
                match &pair {
                    Some(pair) if pair.names_member(name, member.as_str()) => {
                        names_and_urls(member, withheld);
                    }
                    Some(pair) if name == PAIR_VALUE => take(
                        action(pair.class_outside_headers(), view::shape(member)),
                        member,
                        names_and_urls,
                        withheld,
                    ),
                    _ if name == "properties" && matches!(member, OValue::Object(_)) => {
                        property_map(member, withheld);
                    }
                    _ => withhold_named(name, located, member, names_and_urls, withheld),
                }
                // Choice 188: a credential name's value that repeats a sibling `*_type` or
                // `*_mode` (`{"auth": "password", "auth_type": "password"}`) is a mode,
                // withheld in place only.
                if let Some(mode) = mode {
                    let added = withheld.split_off(before);
                    withheld.extend(added.into_iter().filter(|found| *found != mode));
                }
            }
        }
        OValue::Array(items) => items
            .iter_mut()
            .for_each(|item| names_and_urls(item, withheld)),
        OValue::String(_) => withhold_text(value, names_and_urls, withheld),
        _ => {}
    }
}

/// A JSON Schema property map (a `properties` object, rule 3e, choice 146): its member names
/// are property names, not values. A member whose value is a schema ([`view::is_schema`]) is
/// read by [`schema_property`]; any other member by the name rule ([`withhold_named`]), so
/// `{"properties": {"api_key": "<key>"}}` is still withheld.
fn property_map(map: &mut OValue, withheld: &mut Vec<String>) {
    let OValue::Object(entries) = map else {
        return;
    };
    for (name, member) in entries.iter_mut() {
        if view::is_schema(member) {
            schema_property(name, member, withheld);
        } else {
            withhold_named(name, false, member, names_and_urls, withheld);
        }
    }
}

/// The schema of the property `name` (rule 3e): its describing members ([`SCHEMA_KEYWORDS`])
/// are printed and read by the document's rules, and under a property the name rule
/// classifies above plain (a credential, a count, a generic name, a credential member, a
/// header map) a describing string that shows a credential is withheld in place
/// ([`withhold_described_credentials`], choice 152); its `properties` is a property map
/// ([`property_map`]); its subschemas ([`SUBSCHEMA_KEYWORDS`]) are schemas of the same
/// property; an empty `default`, `const`, `enum` or `examples` ([`SCHEMA_VALUE_KEYWORDS`]) is
/// printed (choice 188); and every other member (the instance values `default`, `const`,
/// `enum`, `examples`, and any member outside the schema vocabulary) is a value
/// ([`instance_member`]).
fn schema_property(name: &str, schema: &mut OValue, withheld: &mut Vec<String>) {
    let OValue::Object(entries) = schema else {
        return;
    };
    let credential = classify(name) > Class::Plain;
    for (keyword, member) in entries.iter_mut() {
        let keyword = keyword.as_str();
        if keyword == "properties" && matches!(member, OValue::Object(_)) {
            property_map(member, withheld);
        } else if SUBSCHEMA_KEYWORDS.contains(&keyword) {
            match member {
                OValue::Object(_) => schema_property(name, member, withheld),
                OValue::Array(items) => {
                    for item in items {
                        if matches!(item, OValue::Object(_)) {
                            schema_property(name, item, withheld);
                        } else {
                            instance_member(name, keyword, item, withheld);
                        }
                    }
                }
                other => instance_member(name, keyword, other, withheld),
            }
        } else if SCHEMA_KEYWORDS.contains(&keyword) {
            names_and_urls(member, withheld);
            if credential {
                withhold_described_credentials(member);
            }
        } else if SCHEMA_VALUE_KEYWORDS.contains(&keyword) && view::is_empty(&*member) {
            // Choice 188: an empty instance value says there is none, and prints.
        } else {
            instance_member(name, keyword, member, withheld);
        }
    }
}

/// A member `own` of the property `property`'s schema that holds a value (rule 3e, choice
/// 150): read under whichever of its own name and the property's is the stronger [`Class`]
/// (the order the checker shares; its own on a tie), so a credential named inside a schema
/// (`{"type": "object", "password": …}`) is withheld whatever the property is called, and a
/// credential-named property's `default` is withheld and registered as its credential.
fn instance_member(property: &str, own: &str, member: &mut OValue, withheld: &mut Vec<String>) {
    let name = if classify(own) >= classify(property) {
        own
    } else {
        property
    };
    withhold_named(name, false, member, names_and_urls, withheld);
}

#[cfg(test)]
mod tests;
