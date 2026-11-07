//! The name rule as data (DESIGN §19.3 rule 3, its table "Rule 3 by name"; choices 157–159,
//! 163–165, 170–171, 174, 176, 177).
//!
//! Three steps decide what happens to a member: its name is [`normalised`]; the first row of
//! [`NAME_RULES`] that matches the normalised name gives its [`Class`] ([`classify`],
//! [`member_class`]); the class and the value's [`Shape`] give the [`Action`] ([`action`]).
//! A header's value and a `{key|name, value}` pair's value go through the same table by the
//! header's class ([`header_class`], [`Pair`]). Adding a name is adding it to one list of a
//! row; adding a kind of name is adding a row.

use super::values::{
    has_private_key, looks_like_credential, looks_like_header_name, looks_like_password,
    registers_whole, scheme_credentials, scheme_words,
};

/// What the name rule makes of a member, weakest first: the order decides which of two names
/// a schema member is read under (rule 3e, choice 150), in the CLI and in the checker alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    /// Not withheld by its name: read by the document's rules.
    Plain,
    /// A credential's fingerprint (`api_key_hint`, `token_prefix`, `api_key_last_four`; choice
    /// 187): read as a plain member's value, but a string that looks like a credential
    /// ([`Shape::CredentialLike`]) is withheld and registered as a credential's value, as
    /// TypeScript's `redactDeep` masks the name on the inventory reads.
    Fingerprint,
    /// A header map (rule 3c): its header names printed, every value withheld but the safe
    /// headers'.
    HeaderMap,
    /// A name whose only credential word is `token` (choices 163, 171, 176): a string,
    /// or an object or array with text, is withheld and registered as a credential's value; a
    /// number and a text-free object or array (a usage count, a model price) are printed.
    Count,
    /// A generic name, `key` or `token` (rule 3b, choices 120, 141): withheld, but registered
    /// only where the value looks like a credential.
    Generic,
    /// A credential name (rule 3a): a string (the empty string too), a number or a container
    /// withheld (never `null` or a boolean, choices 171, 187), registered as a credential's
    /// value.
    Credential,
    /// A credential member (rule 3d, choice 136): as [`Class::Credential`], and withheld whole
    /// even when it holds no text.
    Member,
}

/// How a row of [`NAME_RULES`] matches a normalised name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// The name is one of these.
    Exact(&'static [&'static str]),
    /// The name starts with one of these.
    Prefix(&'static [&'static str]),
    /// The name ends with one of these.
    Suffix(&'static [&'static str]),
    /// The name with every `_` removed contains one of these (a credential word anywhere,
    /// TypeScript `redactDeep`'s reading; choice 157).
    Joined(&'static [&'static str]),
    /// A `_` word of the name is one of these (`db_pass`, `DB_PASS`, `smtpPass`; not
    /// `passthrough`, `compass` or `bypass`; choice 176).
    Word(&'static [&'static str]),
    /// The name with the `_` words of the second list dropped and every `_` removed contains
    /// one of the first words (`aws_session_token`, `bearertoken`, `prompt_tokens`,
    /// `tokenvalue`, `TOKENvalue`; not `tokenizer` or `model_tokenizer`; choices 163, 176): the
    /// letters TypeScript's `/token/i` reads in the raw name, whatever the case boundaries.
    Within(&'static [&'static str], &'static [&'static str]),
    /// The name's last `_` word or words are one of these (`user_api_key_count`,
    /// `api_key_last_four`, `token_expires_at`; not `counter` or `atlas`; choice 187).
    Last(&'static [&'static str]),
    /// The name's last `_` word is one of the first words and the rest, with every `_` removed,
    /// contains one of the second (`secret_url`, `credentials_uri`; not `token_url`; choice
    /// 187).
    LastAfter(&'static [&'static str], &'static [&'static str]),
}

/// Whether the normalised `name` is `word` or ends with `_` and `word` (`word` may itself hold
/// `_`, as `last_four` does).
fn ends_with_words(name: &str, word: &str) -> bool {
    name.strip_suffix(word)
        .is_some_and(|rest| rest.is_empty() || rest.ends_with('_'))
}

impl Pattern {
    /// The words of the row (the DESIGN table and the corpus's generated names).
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "the harness's corpus generates names from it")
    )]
    pub const fn words(&self) -> &'static [&'static str] {
        match self {
            Self::Exact(words)
            | Self::Prefix(words)
            | Self::Suffix(words)
            | Self::Joined(words)
            | Self::Word(words)
            | Self::Within(words, _)
            | Self::Last(words)
            | Self::LastAfter(words, _) => words,
        }
    }

    /// Whether the normalised `name` matches.
    pub fn matches(&self, name: &str) -> bool {
        match self {
            Self::Exact(words) => words.contains(&name),
            Self::Prefix(words) => words.iter().any(|word| name.starts_with(word)),
            Self::Suffix(words) => words.iter().any(|word| name.ends_with(word)),
            Self::Joined(words) => {
                let joined = name.replace('_', "");
                words.iter().any(|word| joined.contains(word))
            }
            Self::Word(words) => name.split('_').any(|part| words.contains(&part)),
            Self::Within(words, except) => {
                let kept: String = name
                    .split('_')
                    .filter(|part| !except.contains(part))
                    .collect();
                words.iter().any(|word| kept.contains(word))
            }
            Self::Last(words) => words.iter().any(|word| ends_with_words(name, word)),
            Self::LastAfter(words, within) => words.iter().any(|word| {
                name.strip_suffix(word)
                    .and_then(|rest| rest.strip_suffix('_'))
                    .is_some_and(|rest| {
                        let joined = rest.replace('_', "");
                        within.iter().any(|fragment| joined.contains(fragment))
                    })
            }),
        }
    }
}

/// One row of the name rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NameRule {
    /// What the row matches.
    pub pattern: Pattern,
    /// Only in a secret mapping or a secret reference ([`is_secret_location`]).
    pub located: bool,
    /// The class of a name the row matches.
    pub class: Class,
    /// The DESIGN §19.20 choices (or other sections) that set the row.
    pub choices: &'static str,
    /// What the row is for (one line of the DESIGN table).
    pub reason: &'static str,
}

/// The names that hold a location inside a secret manager in a secret mapping or a secret
/// reference (choice 164): there `secret_key` is the key name inside the referenced secret.
pub const LOCATION_NAMES: &[&str] = &["secret_key"];

/// The members whose presence makes an object a secret mapping (`secret_reference_id`, the
/// SDK's `GatewaySecretMapping`) or a secret reference (`secret_path`), normalised.
pub const LOCATION_ANCHORS: &[&str] = &["secret_path", "secret_reference_id"];

/// The service's designed fingerprints (§18.20 choice 5).
pub const PRINTED_PREFIXES: &[&str] = &["masked_"];

/// The names the rows below them would match that are printed (normalised): identifiers and
/// references of a credential, not the credential; two token counts a usage summary may send
/// as text; two guardrail settings (Acuvity's `secrets_categories`, the JWT guardrail's
/// `maxTokenAge`); the names of a key inside another document (the JWT guardrail's
/// `headerKey`, a log's `metadataKey`, a usage policy's `value_key`); a Portkey virtual key
/// (a slug naming a stored provider key, as `api_key_id` names an API key); and the request
/// flag `rotate_auth` (choices 157, 165, 170); and a log export filter's `virtual_keys` (the
/// slugs of Portkey virtual keys, which the `_keys` suffix would catch; choice 176); the lists
/// of key names the `_keys` suffix would catch (choice 188): the required-metadata guardrail's
/// `metadataKeys` (`GatewayRequiredMetadataKeysParameters`, also a guardrail catalog
/// parameter) and the analytics filter boundaries' `unique_metadata_keys` and
/// `unique_virtual_keys` (metadata key names, virtual key slugs). `unique_api_keys` and
/// `api_keys` stay credentials (the API keys a filter saw; a list of API key records).
pub const PRINTED_NAMES: &[&str] = &[
    "api_key_id",
    "api_key_ids",
    "api_key_defaults_id",
    "header_key",
    "max_token_age",
    "max_tokens",
    "metadata_key",
    "metadata_keys",
    "rotate_auth",
    "secret_mappings",
    "secret_path",
    "secret_reference_id",
    "secrets_categories",
    "total_tokens",
    "unique_metadata_keys",
    "unique_virtual_keys",
    "value_key",
    "virtual_key",
    "virtual_keys",
];

/// The generic credential names (rule 3b): `key`, its plural `keys` (choice 176: a list of
/// keys as often as a list of key names) and `token`.
pub const GENERIC_NAMES: &[&str] = &["key", "keys", "token"];

/// The credential members (rule 3d): the SDK's credential members the words below miss.
pub const CREDENTIAL_MEMBERS: &[&str] = &[
    "vertex_service_account_json",
    "oauth_metadata",
    "model_config",
    "auth_config",
];

/// The credentials whose name ends in a metadata word (choice 187): a HashiCorp Vault AppRole
/// SecretID (`secret_id`, `vault_secret_id`), the password of the role, not an identifier.
pub const CREDENTIAL_IDS: &[&str] = &["secret_id"];

/// The last words of a URL member (choice 187): `token_url`, `auth_uri` and `token_endpoint`
/// name where a credential is sent or obtained, and are printed ([`METADATA_WORDS`]); a URL
/// named after a stored secret ([`SECRET_URL_FRAGMENTS`]: `secret_url`, `credentials_uri`) is
/// a capability URL, the credential itself, and is withheld.
pub const URL_WORDS: &[&str] = &["url", "uri", "endpoint"];

/// The words of a URL name that make the URL a credential (choice 187): the stored-secret
/// words of [`CREDENTIAL_FRAGMENTS`] (not `apikey`, `authorization`, `clientauth` or
/// `authcode`, which name where a credential is used).
pub const SECRET_URL_FRAGMENTS: &[&str] = &[
    "secret",
    "password",
    "passwd",
    "pwd",
    "passphrase",
    "passcode",
    "credential",
    "cookie",
    "privatekey",
    "connectionstring",
];

/// The last words of a credential's fingerprint (choice 187): `prefix`, `last4`, `last8`,
/// `last_four` and `hint` (`api_key_hint`, `token_prefix`, `api_key_last8`). A fingerprint is
/// printed ([`Class::Fingerprint`]), but a value under one that looks like a credential is the
/// credential sent under the wrong name and is withheld (review: `api_key_hint:
/// "sk-live-…"` printed; TypeScript's `redactDeep` masks both names on the inventory reads).
pub const FINGERPRINT_WORDS: &[&str] = &["prefix", "last4", "last8", "last_four", "hint"];

/// The last words that say a member describes a credential rather than holds it (choice 187,
/// the live run of `workspaces get`: `user_api_key_count`, `user_api_key_max_ttl`,
/// `user_api_key_ttl` and `user_api_key_rotation_period` printed `[withheld]`): a count, a
/// limit or a length, a lifetime or a period and its units, a timestamp (`created_at`,
/// `expires_at`, any `_at`) or an expiry, a flag, a type, a name, an identifier, a version or
/// a status, where it is kept (`location`, `path`) and where it is sent or obtained (`url`,
/// `uri`, `endpoint`; but [`SECRET_URL_FRAGMENTS`]). TypeScript's `redactDeep` masks these
/// names (its `/token|secret|…|api[-_]?key/i` reads any name holding the word) on the reads it
/// runs on (the inventory gets and `audit-logs list`), not on `workspaces get`, the read that
/// sends the workspace settings, which prints them.
pub const METADATA_WORDS: &[&str] = &[
    "count",
    "counts",
    "limit",
    "limits",
    "length",
    "size",
    "max",
    "min",
    "ttl",
    "period",
    "interval",
    "duration",
    "ms",
    "seconds",
    "minutes",
    "hours",
    "days",
    "at",
    "date",
    "timestamp",
    "expiry",
    "expiration",
    "expires",
    "expires_in",
    "enabled",
    "disabled",
    "required",
    "type",
    "types",
    "kind",
    "mode",
    "format",
    "name",
    "names",
    "id",
    "ids",
    "version",
    "status",
    "location",
    "path",
    "url",
    "uri",
    "endpoint",
];

/// The exact credential names no word below covers (choices 170, 181, 183): `auth`, a JSON Web
/// Token (`jwt`), a bearer credential (`bearer`), an authorization header's value
/// (`auth_header`, `AuthHeader`, `authheader`), the run-together lowercase `accesskey`,
/// `masterkey` and `accountkey` (which the `_key` suffix reads only with a boundary,
/// `accessKey`), and an Azure shared access signature (`SharedAccessSignature`, run together
/// `sharedaccesssignature`, and its signature parameter `sig`), and a `signature` (choice 187).
pub const CREDENTIAL_NAMES: &[&str] = &[
    "auth",
    "jwt",
    "bearer",
    "auth_header",
    "authheader",
    "accesskey",
    "masterkey",
    "accountkey",
    "shared_access_signature",
    "sharedaccesssignature",
    "sig",
    "signature",
];

/// The credential-name suffixes (choices 170, 176, 181): `_key` (`account_key`, `master_key`,
/// `encryption_key`, `api_key`, `aws_secret_access_key`, `x_portkey_api_key`), its plural
/// `_keys` (`access_keys`, `secret_keys`), `_auth` (`basic_auth`, `proxy_auth`) and `_jwt`
/// (`id_jwt`, `access_jwt`); and `_signature` (`request_signature`, `webhook_signature`;
/// choice 187).
pub const CREDENTIAL_SUFFIXES: &[&str] = &["_key", "_keys", "_auth", "_jwt", "_signature"];

/// The credential words found anywhere in a name with its `_` removed (choices 157, 170):
/// TypeScript `redactDeep`'s words (`secret`, `password`, `passwd`, `credential`, `apikey`,
/// `authorization`, `cookie`, `clientauth`, `authcode`) with `privatekey`, `pwd`,
/// `passphrase` and `connectionstring` (an Azure storage or database connection string holds
/// its account key or password; choice 176), and `passcode` (choice 187).
pub const CREDENTIAL_FRAGMENTS: &[&str] = &[
    "secret",
    "password",
    "passwd",
    "pwd",
    "passphrase",
    "passcode",
    "credential",
    "apikey",
    "authorization",
    "cookie",
    "clientauth",
    "authcode",
    "privatekey",
    "connectionstring",
];

/// The credential words that are whole `_` words only (choice 176): `pass` (`db_pass`,
/// `smtp_pass`, `DB_PASS`), `pat`, a personal access token (`GITHUB_PAT`), and `otp`, a
/// one-time password (`otp`, `sms_otp`; choice 187). Inside a word they are ordinary
/// (`passthrough`, `compass`, `bypass`, `path`, `pattern`, `hotpot`). `pin` is not one
/// (choice 187.4): as a word it names a pinned version or certificate (`pin_version`,
/// `cert_pin`) as often as a PIN, so a numeric PIN under a bare `pin` name prints, while one
/// under a credential name or inside a credential object is withheld (choice 187.1).
pub const CREDENTIAL_WORDS: &[&str] = &["pass", "pat", "otp"];

/// The token word (choices 163, 171, 176): a name whose letters contain it (`token`, `tokens`,
/// `bearertoken`, `tokenvalue`, `accesstokenhash`), as TypeScript's `/token/i` reads it.
pub const TOKEN_WORDS: &[&str] = &["token"];

/// The words that contain `token` and name no credential (choice 176): a model's tokenizer.
pub const TOKEN_EXCEPTIONS: &[&str] = &["tokenizer", "tokenizers"];

/// The header map named by its whole name (rule 3c, normalised).
pub const HEADER_MAPS: &[&str] = &["headers"];

/// The header-map suffix (rule 3c, choice 177): `custom_headers`, `request_headers`,
/// `extra_headers`, `forward_headers`.
pub const HEADER_MAP_SUFFIXES: &[&str] = &["_headers"];

/// The name rule: the first row that matches a normalised name gives its [`Class`]; a name
/// no row matches is [`Class::Plain`]. DESIGN §19.3 "Rule 3 by name" is this table, and a
/// CLI test checks that the two agree ([`rules_table`]).
pub const NAME_RULES: &[NameRule] = &[
    NameRule {
        pattern: Pattern::Exact(LOCATION_NAMES),
        located: true,
        class: Class::Plain,
        choices: "164",
        reason: "a key name inside the referenced secret, beside `secret_path` or `secret_reference_id`",
    },
    NameRule {
        pattern: Pattern::Prefix(PRINTED_PREFIXES),
        located: false,
        class: Class::Plain,
        choices: "§18.20 c5, 157",
        reason: "the service's fingerprints",
    },
    NameRule {
        pattern: Pattern::Exact(PRINTED_NAMES),
        located: false,
        class: Class::Plain,
        choices: "157, 165, 170, 176, 188",
        reason: "identifiers, references, text-capable counts, guardrail settings, key names, a request flag",
    },
    NameRule {
        pattern: Pattern::Exact(GENERIC_NAMES),
        located: false,
        class: Class::Generic,
        choices: "120, 141, 176",
        reason: "a tag name or a word as often as a credential",
    },
    NameRule {
        pattern: Pattern::Exact(CREDENTIAL_MEMBERS),
        located: false,
        class: Class::Member,
        choices: "136",
        reason: "the SDK's credential members",
    },
    NameRule {
        pattern: Pattern::Last(CREDENTIAL_IDS),
        located: false,
        class: Class::Credential,
        choices: "187",
        reason: "a Vault AppRole SecretID, a credential that ends in `id`",
    },
    NameRule {
        pattern: Pattern::LastAfter(URL_WORDS, SECRET_URL_FRAGMENTS),
        located: false,
        class: Class::Credential,
        choices: "187",
        reason: "a capability URL named after a stored secret",
    },
    NameRule {
        pattern: Pattern::Last(FINGERPRINT_WORDS),
        located: false,
        class: Class::Fingerprint,
        choices: "187",
        reason: "a credential's fingerprint, withheld only where it looks like a credential",
    },
    NameRule {
        pattern: Pattern::Last(METADATA_WORDS),
        located: false,
        class: Class::Plain,
        choices: "187",
        reason: "what describes a credential: a count, a lifetime, a timestamp, a type, a name, an id, a location",
    },
    NameRule {
        pattern: Pattern::Exact(CREDENTIAL_NAMES),
        located: false,
        class: Class::Credential,
        choices: "170, 181, 183, 187",
        reason: "a credential named by its kind",
    },
    NameRule {
        pattern: Pattern::Suffix(CREDENTIAL_SUFFIXES),
        located: false,
        class: Class::Credential,
        choices: "157, 170, 176, 181, 187",
        reason: "a key, an authentication, a token or a signature of something",
    },
    NameRule {
        pattern: Pattern::Joined(CREDENTIAL_FRAGMENTS),
        located: false,
        class: Class::Credential,
        choices: "131, 157, 170, 176, 187",
        reason: "a credential word anywhere in the name",
    },
    NameRule {
        pattern: Pattern::Word(CREDENTIAL_WORDS),
        located: false,
        class: Class::Credential,
        choices: "176, 187",
        reason: "a credential word that is ordinary inside a word",
    },
    NameRule {
        pattern: Pattern::Within(TOKEN_WORDS, TOKEN_EXCEPTIONS),
        located: false,
        class: Class::Count,
        choices: "163, 165, 171, 176",
        reason: "a token or a token count",
    },
    NameRule {
        pattern: Pattern::Exact(HEADER_MAPS),
        located: false,
        class: Class::HeaderMap,
        choices: "138",
        reason: "a header map",
    },
    NameRule {
        pattern: Pattern::Suffix(HEADER_MAP_SUFFIXES),
        located: false,
        class: Class::HeaderMap,
        choices: "138, 177",
        reason: "a header map of something",
    },
];

/// The name lowercased, every character that is not a letter or a digit read as `_` (runs of
/// them as one, none at either end), and a camelCase boundary read as `_` (choices 157, 170):
/// `clientSecret` is `client_secret`, `X-API-Key`, `API KEY` and `api.key` are `x_api_key`,
/// `api_key` and `api_key`, `webhookURL` is `webhook_url`. A boundary is an uppercase letter
/// after a lowercase letter or a digit, or the last uppercase letter of a run that a lowercase
/// letter follows (`APIKey` is `api_key`).
pub fn normalised(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    let mut pending = false;
    for (at, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            pending = !out.is_empty();
            continue;
        }
        if c.is_uppercase() && at > 0 {
            let before = chars[at - 1];
            let lower_after = chars.get(at + 1).is_some_and(|next| next.is_lowercase());
            if before.is_lowercase()
                || before.is_ascii_digit()
                || (before.is_uppercase() && lower_after)
            {
                pending = !out.is_empty();
            }
        }
        if pending {
            out.push('_');
            pending = false;
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// The first row of [`NAME_RULES`] that matches `name` where `located` says whether its object
/// is a secret mapping or a secret reference; `None` when no row does.
pub fn rule_of(name: &str, located: bool) -> Option<&'static NameRule> {
    let name = normalised(name);
    NAME_RULES
        .iter()
        .find(|rule| (located || !rule.located) && rule.pattern.matches(&name))
}

/// The [`Class`] of a member named `name` in an object that `located` says is a secret
/// mapping or a secret reference ([`is_secret_location`]).
pub fn member_class(name: &str, located: bool) -> Class {
    rule_of(name, located).map_or(Class::Plain, |rule| rule.class)
}

/// The [`Class`] of a member named `name` anywhere but in a secret location.
pub fn classify(name: &str) -> Class {
    member_class(name, false)
}

/// Whether an object whose member names are `names` is a secret mapping or a secret reference
/// ([`LOCATION_ANCHORS`]; choice 164).
pub fn is_secret_location<'a>(names: impl IntoIterator<Item = &'a str>) -> bool {
    names
        .into_iter()
        .any(|name| LOCATION_ANCHORS.contains(&normalised(name).as_str()))
}

/// Whether the name rule withholds a member named `name` (a [`Class`] above
/// [`Class::HeaderMap`]).
pub fn is_credential_name(name: &str) -> bool {
    classify(name) >= Class::Count
}

/// Whether a member name names a header map (rule 3c).
pub fn is_header_map(name: &str) -> bool {
    classify(name) == Class::HeaderMap
}

/// The shape of a value, as the action table reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Null,
    Bool,
    Number,
    /// A string that looks like a credential ([`looks_like_credential`]: at least
    /// [`super::values::CREDENTIAL_LOOK`] characters, no whitespace, no date; the one shape a
    /// [`Class::Fingerprint`] withholds, choice 187).
    CredentialLike,
    /// Any other string, the empty string included.
    String,
    /// An object or array with no string at any depth and no member name that withholds or
    /// shows a credential (choices 165, 171; `view::text_free`).
    TextFree,
    /// Any other object or array.
    Container,
}

/// What of a withheld value joins the `SecretSet` (rule 3g, choice 154; [`registered`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaf {
    /// A value an SDK secret path names (rules 1 and 2): whole, each of its
    /// whitespace-separated words that looks like a credential, and every word after an
    /// authorization scheme (choices 142, 148, 161).
    Path,
    /// A credential's value by its name (a credential name's or member's, a credential
    /// header's or pair's, a fingerprint's that looks like a credential): as [`Leaf::Path`],
    /// but whole only where the value can be a credential by its shape ([`registers_whole`]:
    /// at least eight characters and no placeholder, or a shorter one that carries a symbol
    /// or holds whitespace, and no date; choice 187), so `{"password": "Bearer"}` withholds
    /// `Bearer` in place and nowhere else, and `{"password": "dragonfly"}` its echo too.
    Credential,
    /// A generic name's (`key`, `token`) or a non-credential header's value: whole or as a
    /// word only where that looks like a credential (choices 120, 139, 141).
    Generic,
    /// A string under a non-credential name inside a withheld credential: whole when it looks
    /// like a credential or a password, and each word that looks like a credential (choices
    /// 147, 151).
    Inner,
    /// A describing string of a schema object inside a withheld credential: whole only when it
    /// looks like a credential, never its words (choices 147, 151).
    Describing,
}

/// What happens to a member's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Read by the document's rules (a plain member).
    Read,
    /// Read as a header map (rule 3c).
    Headers,
    /// Printed as it is.
    Keep,
    /// Replaced by `[withheld]` and registered as the [`Leaf`] says.
    Withhold(Leaf),
}

/// The action table (DESIGN §19.3 "Rule 3 by name", its second table): what a member of
/// `class` whose value has `shape` becomes. `null` and booleans are printed under every name
/// (choices 171, 187: no credential is a flag or an absence, and the organisation's
/// `user_api_key_auto_create` prints as TypeScript prints it); a number is printed only under
/// a header map and a count (a usage count, choice 171), and withheld under every other
/// withholding class, as TypeScript's `redactDeep` masks it (a numeric password or PIN;
/// choice 187); the empty string is a string, withheld under every withholding class (an
/// empty password is the password); a text-free object or array is printed only under
/// [`Class::Count`] (a price, a usage breakdown; choice 165 as narrowed by choice 171); a
/// fingerprint is read as a plain member unless it looks like a credential (choice 187).
pub const fn action(class: Class, shape: Shape) -> Action {
    match (class, shape) {
        (Class::Plain, _) => Action::Read,
        (Class::Fingerprint, Shape::CredentialLike) => Action::Withhold(Leaf::Credential),
        (Class::Fingerprint, _) => Action::Read,
        (_, Shape::Null | Shape::Bool) => Action::Keep,
        (Class::HeaderMap, Shape::Number) => Action::Keep,
        (Class::HeaderMap, _) => Action::Headers,
        (Class::Count, Shape::Number | Shape::TextFree) => Action::Keep,
        (Class::Count, _) => Action::Withhold(Leaf::Credential),
        (Class::Generic, _) => Action::Withhold(Leaf::Generic),
        (Class::Credential | Class::Member, _) => Action::Withhold(Leaf::Credential),
    }
}

/// The headers whose values a header map shows (normalised; never a credential).
pub const SAFE_HEADERS: &[&str] = &[
    "accept",
    "accept_encoding",
    "accept_language",
    "cache_control",
    "connection",
    "content_encoding",
    "content_length",
    "content_type",
    "host",
    "user_agent",
    "x_request_id",
    "x_portkey_provider",
];

/// The words of a header name that make it a credential header: `x-functions-key`,
/// `x-auth-key`, `x-custom-auth`, `x-amz-security-token`, `x-signature`; and the generic
/// name `keys` (choice 176), so a `{"key": "keys", "value": …}` pair is withheld as `key`'s is.
pub const HEADER_CREDENTIAL_WORDS: &[&str] = &[
    "auth",
    "authorization",
    "key",
    "keys",
    "apikey",
    "token",
    "secret",
    "password",
    "passwd",
    "pwd",
    "cookie",
    "credential",
    "credentials",
    "signature",
];

/// Whether a header name's value is shown ([`SAFE_HEADERS`], never a credential name).
pub fn is_safe_header(name: &str) -> bool {
    SAFE_HEADERS.contains(&normalised(name).as_str()) && !is_credential_name(name)
}

/// The [`Class`] a header's value is read under (rule 3c; choices 139, 142, 159): a safe
/// header's [`Class::Plain`] (shown, read by the document's rules); a name the name rule
/// withholds as a credential or a count, that class; a name with a credential header word
/// ([`HEADER_CREDENTIAL_WORDS`]) [`Class::Credential`]; any other header [`Class::Generic`]
/// (withheld in place, registered only where it looks like a credential).
pub fn header_class(name: &str) -> Class {
    if is_safe_header(name) {
        return Class::Plain;
    }
    let class = classify(name);
    if matches!(class, Class::Count | Class::Credential | Class::Member) {
        class
    } else if normalised(name)
        .split('_')
        .any(|word| HEADER_CREDENTIAL_WORDS.contains(&word))
    {
        Class::Credential
    } else {
        Class::Generic
    }
}

/// Whether a header's value is a credential (a credential name, a count name or a credential
/// header word): [`header_class`] above [`Class::Generic`] or a count.
#[cfg(any(test, feature = "test-support"))]
pub fn is_credential_header(name: &str) -> bool {
    matches!(
        header_class(name),
        Class::Count | Class::Credential | Class::Member
    )
}

/// A `{"key": <name>, "value": …}`, `{"name": <name>, "value": …}` or `{"header": <name>,
/// "value": …}` pair (a tag, a header list, a Kubernetes-style environment list; choices 159,
/// 170, 177): an object with a `value` member and a `key`, `name` or `header` member that is
/// a string. Its name members are printed as names (read by the string rules only) and its
/// value is read as the value of a header with that name ([`Pair::class`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    /// The pair's names: its `key`, its `name` and its `header`, those that are strings, in
    /// that order.
    pub names: Vec<String>,
}

/// The members that name a pair, in the order [`Pair::names`] holds them.
pub const PAIR_NAMES: &[&str] = &["key", "name", "header"];

/// The member that holds a pair's value.
pub const PAIR_VALUE: &str = "value";

impl Pair {
    /// The pair of an object whose members are `members` (name, and the value's text when it
    /// is a string); `None` when it is not a pair.
    pub fn of<'a>(members: impl IntoIterator<Item = (&'a str, Option<&'a str>)>) -> Option<Self> {
        let mut has_value = false;
        let mut found: [Option<String>; PAIR_NAMES.len()] = Default::default();
        for (member, text) in members {
            if member == PAIR_VALUE {
                has_value = true;
            } else if let Some(at) = PAIR_NAMES.iter().position(|name| *name == member) {
                found[at] = text.map(str::to_owned);
            }
        }
        let names: Vec<String> = found.into_iter().flatten().collect();
        (has_value && !names.is_empty()).then_some(Self { names })
    }

    /// Whether `member` is one of the pair's names (printed, never registered).
    pub fn names_member(&self, member: &str, text: Option<&str>) -> bool {
        PAIR_NAMES.contains(&member)
            && text.is_some_and(|text| self.names.iter().any(|n| n == text))
    }

    /// The class the pair's value is read under as a header ([`header_class`]): the strongest
    /// of its names' (so either name being a credential header withholds the value), and a
    /// safe header only when every name is one.
    pub fn class(&self) -> Class {
        self.names
            .iter()
            .map(|name| header_class(name))
            .max()
            .unwrap_or(Class::Generic)
    }

    /// The class the pair's value is read under outside a header map (choice 159): its
    /// [`Pair::class`] when that is a credential or a count, else [`Class::Plain`] (a tag's
    /// value read by the document's rules).
    pub fn class_outside_headers(&self) -> Class {
        match self.class() {
            class @ (Class::Count | Class::Credential | Class::Member) => class,
            _ => Class::Plain,
        }
    }
}

/// The action on a header value of class `class` ([`header_class`]) whose value has `shape`:
/// a credential, member or count header's value (`Authorization`, `x-auth-token`; a header
/// is never a usage count) as a credential name's, so its number is withheld and registered
/// (choice 187); a generic header's value withheld whatever its shape but `null`, a boolean
/// and a number (`x-retries: 3`; TypeScript's `redactDeep` masks no value under a header name
/// that is no credential name); a safe header's read as a plain member's.
pub const fn header_action(class: Class, shape: Shape) -> Action {
    match (class, shape) {
        (Class::Generic, Shape::Null | Shape::Bool | Shape::Number) => Action::Keep,
        (Class::Generic, _) => Action::Withhold(Leaf::Generic),
        (Class::Count, _) => action(Class::Credential, shape),
        _ => action(class, shape),
    }
}

/// The action on an item of a header list that is not an object (rule 3c, choice 177): a
/// header name ([`looks_like_header_name`]: a `forward_headers` list of names) is read by the
/// document's rules, any other value as a generic header's value ([`header_action`]).
pub fn header_item_action(text: Option<&str>, shape: Shape) -> Action {
    if text.is_some_and(looks_like_header_name) {
        Action::Read
    } else {
        header_action(header_class(""), shape)
    }
}

/// The shortest withheld string that joins the `SecretSet` (choice 69): a shorter one is
/// withheld in place only, since a one-letter fragment in the set would withhold every string
/// that contains it.
pub const MIN_REGISTERED: usize = 4;

/// The words that say a value is absent, unset or hidden rather than give it (choice 183, any
/// case): the JSON literals, `none`, `nil`, `undefined`, `empty`, `unset` and `redacted`.
/// Under a credential name such a word is withheld in place and never registered, so
/// `{"password": "none"}` or `auth=none` in one row does not withhold `mode: none` in another.
pub const PLACEHOLDER_WORDS: &[&str] = &[
    "true",
    "false",
    "null",
    "none",
    "nil",
    "undefined",
    "empty",
    "unset",
    "redacted",
    "[redacted]",
];

/// The words that name a state, a mode or an OAuth grant type rather than give a credential
/// (choice 188, any case, `-` and `_` ignored: `Client-Credentials`, `CLIENT_CREDENTIALS`).
/// Under a credential name such a word is withheld in place and never registered, so
/// `{"auth": "disabled"}` in one row does not withhold `status: disabled` or `Server is
/// disabled` in another. A shorter one (`enabled`, `oauth2`) is never registered whole anyway
/// (`super::values::registers_whole`).
pub const MODE_WORDS: &[&str] = &[
    "enabled",
    "disabled",
    "required",
    "optional",
    "mandatory",
    "anonymous",
    "unauthenticated",
    "inherited",
    "clientcredentials",
    "oauth2clientcredentials",
    "authorizationcode",
    "oauth2authorizationcode",
    "refreshtoken",
    "devicecode",
    "jwtbearer",
    "implicit",
    "oauth2implicit",
];

/// Whether `text`, trimmed, is one of [`MODE_WORDS`]: letters, digits, `-` and `_` only, which
/// once lowercased and with `-` and `_` removed is a listed word (so `disabled!` is not one).
pub fn is_mode_word(text: &str) -> bool {
    let trimmed = text.trim();
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return false;
    }
    let folded: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    MODE_WORDS.contains(&folded.as_str())
}

/// The last `_` words of a member that names a mode (choice 188): `auth_type`, `type`,
/// `auth_mode`, `mode`.
pub const MODE_MEMBER_WORDS: &[&str] = &["type", "mode"];

/// The string values of the members of an object (`members`: name, and the value's text when
/// it is a string) whose name ends with a [`MODE_MEMBER_WORDS`] word (choice 188): the modes
/// a sibling credential name's value may repeat ([`repeats_a_mode`]).
pub fn mode_values<'a>(
    members: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
) -> Vec<&'a str> {
    members
        .into_iter()
        .filter(|(name, _)| Pattern::Last(MODE_MEMBER_WORDS).matches(&normalised(name)))
        .filter_map(|(_, text)| text)
        .collect()
}

/// Whether a withheld credential name's value `text` is a mode that a sibling `*_type` or
/// `*_mode` member holds too (choice 188; `modes` from [`mode_values`]): the same text in any
/// case, letters, digits, `-` and `_` only, which the name rule reads as a credential name
/// itself (`{"auth": "password", "auth_type": "password"}`, `api_key`, `bearer`). It is
/// withheld in place and not registered, so the sibling prints; a value that names no kind of
/// credential (`hunter2x`) is registered as before, and so is `password` with no such sibling.
pub fn repeats_a_mode(text: &str, modes: &[&str]) -> bool {
    text.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && is_credential_name(text)
        && modes.iter().any(|mode| mode.eq_ignore_ascii_case(text))
}

/// The characters of a mask (choice 183): a string made of these alone (`****`, `••••••••`,
/// `xxxxxxxx`, `####`) stands for a hidden value and is never registered.
pub const MASK_CHARACTERS: &[char] = &['*', '•', '●', '·', 'x', 'X', '#'];

/// Whether a withheld string may join the `SecretSet` (rule 3g; choices 69, 144, 183), the one
/// filter every registration of these rules applies ([`registered`], [`registered_number`],
/// the text rules' [`super::values::TextCredential::registered`], the URL rule's parts and
/// [`super::values::unreadable_json_strings`]): at least [`MIN_REGISTERED`] characters once
/// trimmed of whitespace, never one that carries the placeholder `[withheld]` (an earlier
/// rule's; registering it would withhold every string that shows one), never a
/// [`PLACEHOLDER_WORDS`] word, never a [`MODE_WORDS`] word (choice 188) and never a run of
/// [`MASK_CHARACTERS`].
pub fn registrable(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.chars().count() >= MIN_REGISTERED
        && !text.contains(super::WITHHELD)
        && !PLACEHOLDER_WORDS
            .iter()
            .any(|word| trimmed.eq_ignore_ascii_case(word))
        && !is_mode_word(trimmed)
        && !trimmed.chars().all(|c| MASK_CHARACTERS.contains(&c))
}

/// The strings of a withheld string `text` that join the `SecretSet` as `leaf` says (rule 3g,
/// choice 154; each once, each [`registrable`]): the whole text where the leaf takes it (a
/// path rule's value always, a credential name's where it can be a credential by its shape,
/// choice 187, a text holding a private key always, choice 170), its credential-looking
/// words, and the words after an authorization scheme (in a credential's value every one,
/// elsewhere one that looks like a credential or a password; choice 161).
pub fn registered(leaf: Leaf, text: &str) -> Vec<String> {
    let whole = has_private_key(text)
        || match leaf {
            Leaf::Path => true,
            Leaf::Credential => registers_whole(text),
            Leaf::Generic | Leaf::Describing => looks_like_credential(text),
            Leaf::Inner => looks_like_credential(text) || looks_like_password(text),
        };
    let mut out = Vec::new();
    if whole {
        out.push(text.to_owned());
    }
    if leaf == Leaf::Describing {
        return out;
    }
    let mut words: Vec<&str> = text
        .split_whitespace()
        .filter(|word| *word != text && looks_like_credential(word))
        .collect();
    let schemes = if matches!(leaf, Leaf::Path | Leaf::Credential) {
        scheme_words(text)
    } else {
        scheme_credentials(text)
    };
    for word in schemes {
        if word != text && !words.contains(&word) {
            words.push(word);
        }
    }
    out.extend(words.into_iter().map(str::to_owned));
    out.retain(|found| registrable(found));
    out
}

/// The text of a withheld number that joins the `SecretSet` (choices 171, 187): under a path
/// rule's or a credential's value (a credential name's, member's, header's or pair's) one of
/// at least [`super::values::PASSWORD_LOOK`] characters (a numeric password, PIN or account
/// key, `{"password": 12345678}`), under any other leaf one that looks like a credential. A
/// shorter number is withheld in place only, so a withheld `2026` does not withhold every
/// date.
pub fn registered_number(leaf: Leaf, text: &str) -> Option<String> {
    let enough = match leaf {
        Leaf::Path | Leaf::Credential => text.chars().count() >= super::values::PASSWORD_LOOK,
        Leaf::Generic | Leaf::Inner => looks_like_credential(text),
        Leaf::Describing => false,
    };
    (enough && registrable(text)).then(|| text.to_owned())
}

/// The JSON Schema type names: an object whose `type` is one of them (or a list of them) is a
/// schema (rule 3e).
pub const SCHEMA_TYPES: &[&str] = &[
    "array", "boolean", "integer", "null", "number", "object", "string",
];

/// The members of a schema object that describe the property, not a value of it (the JSON
/// Schema annotation and validation keywords and the service's manifest annotations
/// `label`, `encrypted`, `hidden`).
pub const SCHEMA_KEYWORDS: &[&str] = &[
    "$comment",
    "$id",
    "$ref",
    "$schema",
    "contentEncoding",
    "contentMediaType",
    "deprecated",
    "description",
    "encrypted",
    "exclusiveMaximum",
    "exclusiveMinimum",
    "format",
    "hidden",
    "label",
    "maxItems",
    "maxLength",
    "maxProperties",
    "maximum",
    "minItems",
    "minLength",
    "minProperties",
    "minimum",
    "multipleOf",
    "nullable",
    "pattern",
    "readOnly",
    "required",
    "title",
    "type",
    "uniqueItems",
    "writeOnly",
];

/// The members of a schema object that hold an instance value and print when it is empty
/// (choice 188): the empty string, an empty array and an empty object under `default`,
/// `const`, `enum` or `examples` say there is no value (TypeScript prints a catalog's
/// `"default": ""`), so they print under every property; a non-empty one is read under the
/// stronger name ([`view::is_empty`](super::view::is_empty)).
pub const SCHEMA_VALUE_KEYWORDS: &[&str] = &["const", "default", "enum", "examples"];

/// The members of a schema object that hold subschemas of the same property (an array's
/// `items`, the combinators).
pub const SUBSCHEMA_KEYWORDS: &[&str] = &[
    "additionalProperties",
    "allOf",
    "anyOf",
    "contains",
    "else",
    "if",
    "items",
    "not",
    "oneOf",
    "then",
];

/// The request fields of an audit record (G44, D239).
pub const AUDIT_FIELDS: &[&str] = &["request_body", "request_headers", "query_params"];

/// The rows of [`NAME_RULES`] as the Markdown table of DESIGN §19.3 "Rule 3 by name" (one
/// line per row, in order), which a CLI test finds verbatim in DESIGN.md.
#[cfg(any(test, feature = "test-support"))]
pub fn rules_table() -> String {
    let class = |class: Class| match class {
        Class::Plain => "printed",
        Class::Fingerprint => "fingerprint",
        Class::HeaderMap => "header map",
        Class::Count => "count",
        Class::Generic => "generic",
        Class::Credential => "credential",
        Class::Member => "credential member",
    };
    let mut out = String::from(
        "| # | A normalised name that | Class | Choices | Why |\n|---|---|---|---|---|\n",
    );
    for (at, rule) in NAME_RULES.iter().enumerate() {
        let words = rule
            .pattern
            .words()
            .iter()
            .map(|word| format!("`{word}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let matches = match rule.pattern {
            Pattern::Exact(_) => format!("is {words}"),
            Pattern::Prefix(_) => format!("starts with {words}"),
            Pattern::Suffix(_) => format!("ends with {words}"),
            Pattern::Joined(_) => format!("contains, with `_` removed, {words}"),
            Pattern::Word(_) => format!("has a `_` word that is {words}"),
            Pattern::Within(_, except) => {
                let except = except
                    .iter()
                    .map(|word| format!("`{word}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("contains, with the `_` words {except} and `_` removed, {words}")
            }
            Pattern::Last(_) => format!("ends with the `_` word(s) {words}"),
            Pattern::LastAfter(_, within) => {
                let within = within
                    .iter()
                    .map(|word| format!("`{word}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "ends with the `_` word {words} after a part that, with `_` removed, contains {within}"
                )
            }
        };
        let place = if rule.located {
            " in a secret mapping or reference"
        } else {
            ""
        };
        out.push_str(&format!(
            "| {} | {matches}{place} | {} | {} | {} |\n",
            at + 1,
            class(rule.class),
            rule.choices,
            rule.reason
        ));
    }
    out.push_str(&format!(
        "| {} | matches no row | printed | | |\n",
        NAME_RULES.len() + 1
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_normalised_across_case_and_separators() {
        for (name, expected) in [
            ("clientSecret", "client_secret"),
            ("awsSessionToken", "aws_session_token"),
            ("webhookURL", "webhook_url"),
            ("X-API-Key", "x_api_key"),
            ("APIKey", "api_key"),
            ("oauth2Token", "oauth2_token"),
            ("SESSION_TOKEN", "session_token"),
            ("Content-Type", "content_type"),
            ("API KEY", "api_key"),
            ("api.key", "api_key"),
            ("Api Key", "api_key"),
            ("  api -- key  ", "api_key"),
            ("_secret", "secret"),
            ("metadata.key", "metadata_key"),
            ("x/api:key", "x_api_key"),
        ] {
            assert_eq!(normalised(name), expected, "{name}");
        }
    }

    #[test]
    fn the_table_classifies_every_kind_of_name() {
        for (name, class) in [
            ("region", Class::Plain),
            ("key_id", Class::Plain),
            ("tokenizer", Class::Plain),
            ("model_tokenizers", Class::Plain),
            ("passthrough", Class::Plain),
            ("passthrough_header", Class::Plain),
            ("compass", Class::Plain),
            ("bypass", Class::Plain),
            ("rules_passed", Class::Plain),
            ("path", Class::Plain),
            ("data_pattern", Class::Plain),
            ("virtual_keys", Class::Plain),
            ("api_key_ids", Class::Plain),
            ("monkey", Class::Plain),
            ("auth_type", Class::Plain),
            ("auth_settings", Class::Plain),
            ("authority", Class::Plain),
            ("masked_api_key", Class::Plain),
            ("api_key_id", Class::Plain),
            ("headerKey", Class::Plain),
            ("metadataKey", Class::Plain),
            ("virtual_key", Class::Plain),
            ("rotate_auth", Class::Plain),
            ("custom_headers", Class::HeaderMap),
            ("extra_headers", Class::HeaderMap),
            ("forwardHeaders", Class::HeaderMap),
            ("prompt_tokens", Class::Count),
            ("access_token", Class::Count),
            ("bearertoken", Class::Count),
            ("tokenvalue", Class::Count),
            ("tokenhash", Class::Count),
            ("accesstokenvalue", Class::Count),
            ("TOKENVALUE", Class::Count),
            ("TOKENvalue", Class::Count),
            ("ToKen", Class::Count),
            ("key", Class::Generic),
            ("keys", Class::Generic),
            ("token", Class::Generic),
            ("model_config", Class::Member),
            ("auth", Class::Credential),
            ("pwd", Class::Credential),
            ("db_pwd", Class::Credential),
            ("passphrase", Class::Credential),
            ("keyPassphrase", Class::Credential),
            ("account_key", Class::Credential),
            ("master_key", Class::Credential),
            ("encryption_key", Class::Credential),
            ("API KEY", Class::Credential),
            ("api.key", Class::Credential),
            ("basic_auth", Class::Credential),
            ("secret_key", Class::Credential),
            ("clientSecret", Class::Credential),
            ("adminpassword", Class::Credential),
            ("secret_tokens", Class::Credential),
            ("db_pass", Class::Credential),
            ("smtpPass", Class::Credential),
            ("DB_PASS", Class::Credential),
            ("pass", Class::Credential),
            ("GITHUB_PAT", Class::Credential),
            ("access_keys", Class::Credential),
            ("secretKeys", Class::Credential),
            ("connectionString", Class::Credential),
            ("AZURE_STORAGE_CONNECTION_STRING", Class::Credential),
        ] {
            assert_eq!(classify(name), class, "{name}");
        }
        assert_eq!(member_class("secret_key", true), Class::Plain);
        assert!(is_secret_location(["target_field", "secretReferenceId"]));
    }

    #[test]
    fn the_action_table_prints_only_what_carries_no_credential() {
        use Shape::*;
        for class in [
            Class::Count,
            Class::Generic,
            Class::Credential,
            Class::Member,
        ] {
            assert_eq!(action(class, Null), Action::Keep);
            assert_eq!(action(class, Bool), Action::Keep);
            assert!(matches!(action(class, String), Action::Withhold(_)));
            assert!(matches!(action(class, CredentialLike), Action::Withhold(_)));
            assert!(matches!(action(class, Container), Action::Withhold(_)));
        }
        // Choice 187 (review: `{"password": 12345678}` printed): a number is withheld and
        // registered as a credential's value under a credential name or member and a
        // credential header or pair, as a generic value under a generic name, and printed
        // under a count, a header map, a generic header and every name no row withholds.
        for class in [Class::Credential, Class::Member] {
            assert_eq!(
                action(class, Number),
                Action::Withhold(Leaf::Credential),
                "{class:?}"
            );
            assert_eq!(
                header_action(class, Number),
                Action::Withhold(Leaf::Credential),
                "{class:?}"
            );
        }
        assert_eq!(
            action(Class::Generic, Number),
            Action::Withhold(Leaf::Generic)
        );
        for class in [Class::HeaderMap, Class::Count] {
            assert_eq!(action(class, Number), Action::Keep, "{class:?}");
        }
        assert_eq!(header_action(Class::Generic, Number), Action::Keep);
        // A header named after a token is a credential header, never a usage count.
        assert_eq!(
            header_action(Class::Count, Number),
            Action::Withhold(Leaf::Credential)
        );
        assert_eq!(
            header_action(Class::Count, TextFree),
            Action::Withhold(Leaf::Credential)
        );
        assert_eq!(action(Class::Plain, Number), Action::Read);
        assert_eq!(action(Class::Fingerprint, Number), Action::Read);
        // A fingerprint is read as a plain member unless it looks like a credential.
        assert_eq!(action(Class::Fingerprint, String), Action::Read);
        assert_eq!(
            action(Class::Fingerprint, CredentialLike),
            Action::Withhold(Leaf::Credential)
        );
        assert_eq!(action(Class::Count, TextFree), Action::Keep);
        for class in [Class::Generic, Class::Credential, Class::Member] {
            assert!(matches!(action(class, TextFree), Action::Withhold(_)));
        }
        assert_eq!(action(Class::Plain, String), Action::Read);
        assert_eq!(action(Class::HeaderMap, Container), Action::Headers);
    }

    #[test]
    fn headers_and_pairs_take_the_strongest_name() {
        assert_eq!(header_class("Accept"), Class::Plain);
        assert_eq!(header_class("x-env"), Class::Generic);
        assert_eq!(header_class("X-Functions-Key"), Class::Credential);
        assert_eq!(header_class("token"), Class::Credential);
        assert_eq!(header_class("x-amz-security-token"), Class::Count);
        assert!(!is_credential_header("x-portkey-workspace"));
        let pair =
            |members: &[(&'static str, Option<&'static str>)]| Pair::of(members.iter().copied());
        let env = pair(&[("name", Some("OPENAI_API_KEY")), ("value", None)]).unwrap();
        assert_eq!(env.class_outside_headers(), Class::Credential);
        let tag = pair(&[("key", Some("region")), ("value", Some("us"))]).unwrap();
        assert_eq!(tag.class_outside_headers(), Class::Plain);
        let both = pair(&[
            ("key", Some("accept")),
            ("name", Some("Authorization")),
            ("value", None),
        ])
        .unwrap();
        assert_eq!(both.class(), Class::Credential);
        let header = pair(&[("header", Some("Authorization")), ("value", None)]).unwrap();
        assert_eq!(header.class_outside_headers(), Class::Credential);
        assert!(header.names_member("header", Some("Authorization")));
        let keys = pair(&[("key", Some("keys")), ("value", None)]).unwrap();
        assert_eq!(keys.class_outside_headers(), Class::Credential);
        assert!(pair(&[("name", Some("n"))]).is_none());
        assert!(pair(&[("name", None), ("value", None)]).is_none());
        assert_eq!(
            header_action(Class::Generic, Shape::String),
            Action::Withhold(Leaf::Generic)
        );
        assert_eq!(header_action(Class::Generic, Shape::Number), Action::Keep);
        assert_eq!(
            header_action(Class::Credential, Shape::Number),
            Action::Withhold(Leaf::Credential)
        );
        assert_eq!(header_action(Class::Plain, Shape::Bool), Action::Read);
        // Choice 177: a header list's names are printed, any other item withheld.
        assert_eq!(
            header_item_action(Some("x-trace"), Shape::String),
            Action::Read
        );
        for item in ["sk-proj-ab12cd34ef56gh78", "hunter2x", "Bearer x", ""] {
            assert_eq!(
                header_item_action(Some(item), Shape::String),
                Action::Withhold(Leaf::Generic),
                "{item}"
            );
        }
    }

    #[test]
    fn registration_follows_the_leaf() {
        assert_eq!(
            registered(Leaf::Credential, "Bearer hunter2x"),
            ["Bearer hunter2x", "hunter2x"]
        );
        assert!(registered(Leaf::Generic, "prod").is_empty());
        assert_eq!(registered(Leaf::Inner, "hunter2x!"), ["hunter2x!"]);
        let pem = "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----";
        assert_eq!(registered(Leaf::Describing, pem), [pem]);
        assert_eq!(
            registered_number(Leaf::Credential, "12345678"),
            Some("12345678".into())
        );
        assert_eq!(registered_number(Leaf::Credential, "2026"), None);
        assert_eq!(registered_number(Leaf::Generic, "12345678"), None);
    }

    /// Choice 183: the one registration filter. A credential's value that only says it is
    /// absent, unset or masked, a short string and one carrying the placeholder never join the
    /// `SecretSet`; a real value does.
    #[test]
    fn placeholder_words_and_masks_are_never_registered() {
        for text in [
            "none",
            "None",
            "NULL",
            "true",
            " none ",
            "****",
            "••••••••",
            "xxxxxxxx",
            "abc",
            "[withheld]",
            "Bearer [withheld]",
            "[REDACTED]",
        ] {
            assert!(!registrable(text), "{text}");
            assert!(registered(Leaf::Credential, text).is_empty(), "{text}");
        }
        for text in ["hunter", "sk-****abcd", "nonexistent", "prod"] {
            assert!(registrable(text), "{text}");
        }
        assert_eq!(registered(Leaf::Credential, "Token none"), ["Token none"]);
    }

    /// Choice 188 (review): a state, mode or grant-type word under a credential name is never
    /// registered (any case, `-` and `_` ignored), a word that only resembles one is; a
    /// credential name's value is a mode only where a sibling `*_type` or `*_mode` holds it and
    /// it names a kind of credential.
    #[test]
    fn mode_words_are_never_registered() {
        for text in [
            "disabled",
            "Disabled",
            " required ",
            "optional",
            "anonymous",
            "oauth2_client_credentials",
            "client-credentials",
            "CLIENT_CREDENTIALS",
            "authorization_code",
        ] {
            assert!(!registrable(text), "{text}");
            assert!(registered(Leaf::Credential, text).is_empty(), "{text}");
            assert!(registered(Leaf::Path, text).is_empty(), "{text}");
        }
        for text in ["disabled!", "disabled2x", "password", "dragonfly"] {
            assert!(registrable(text), "{text}");
        }
        let modes = mode_values([
            ("auth_type", Some("password")),
            ("mode", Some("api_key")),
            ("status", Some("hunter2x")),
            ("region", Some("dragonfly")),
            ("type", None),
        ]);
        assert_eq!(modes, ["password", "api_key"]);
        assert!(repeats_a_mode("password", &modes));
        assert!(repeats_a_mode("PASSWORD", &modes));
        assert!(repeats_a_mode("api_key", &modes));
        assert!(!repeats_a_mode("password", &[]));
        assert!(
            !repeats_a_mode("hunter2x", &["hunter2x"]),
            "names no credential"
        );
        assert!(!repeats_a_mode("dragonfly", &modes));
    }

    /// Choice 188 (review): the lists of key names the `_keys` suffix would catch are printed;
    /// the API keys a filter saw and a list of API key records are not.
    #[test]
    fn key_name_lists_are_printed() {
        for name in [
            "metadataKeys",
            "metadata_keys",
            "unique_metadata_keys",
            "unique_virtual_keys",
        ] {
            assert_eq!(classify(name), Class::Plain, "{name}");
        }
        for name in ["unique_api_keys", "api_keys", "access_keys"] {
            assert_eq!(classify(name), Class::Credential, "{name}");
        }
    }

    /// Choice 187 (the live run of `workspaces get`): a name whose last words describe a
    /// credential is printed, but a Vault SecretID and a URL named after a stored secret; the
    /// rows that make it so are matched by their last `_` words only.
    #[test]
    fn metadata_names_describe_a_credential() {
        for name in [
            "user_api_key_count",
            "userApiKeyMaxTtl",
            "user_api_key_rotation_period",
            "token_expires_at",
            "secret_updated_at",
            "auth_token_url",
        ] {
            assert_eq!(classify(name), Class::Plain, "{name}");
        }
        for name in [
            "api_key_last_four",
            "api_key_hint",
            "token_prefix",
            "apiKeyLast8",
            "secret_last4",
        ] {
            assert_eq!(classify(name), Class::Fingerprint, "{name}");
            assert!(!is_credential_name(name), "{name}");
        }
        for (name, class) in [
            ("secret_id", Class::Credential),
            ("vault_secret_id", Class::Credential),
            ("secret_url", Class::Credential),
            ("db_password_uri", Class::Credential),
            ("api_key", Class::Credential),
            ("user_api_key", Class::Credential),
            ("x_api_key", Class::Credential),
            ("api_key_value", Class::Credential),
            ("format_token", Class::Count),
            ("id_token", Class::Count),
            ("atlas_secret", Class::Credential),
        ] {
            assert_eq!(classify(name), class, "{name}");
        }
        assert!(Pattern::Last(&["at"]).matches("expires_at"));
        assert!(!Pattern::Last(&["at"]).matches("format"));
        assert!(Pattern::Last(&["last_four"]).matches("api_key_last_four"));
        assert!(!Pattern::Last(&["last_four"]).matches("api_key_four"));
        assert!(Pattern::LastAfter(&["url"], &["secret"]).matches("client_secret_url"));
        assert!(!Pattern::LastAfter(&["url"], &["secret"]).matches("secret_url_name"));
        assert!(!Pattern::LastAfter(&["url"], &["secret"]).matches("url"));
    }

    /// Choice 187 (review: `{"token_type": "Bearer"}` withheld every string carrying
    /// `Bearer`; then a dictionary password's echo printed): a credential name's value joins
    /// the `SecretSet` whole when it has at least eight characters or a symbol or a space,
    /// never a date, a placeholder or a mask; a shorter word is withheld in place only; a path
    /// rule's value always joins.
    #[test]
    fn a_credential_name_registers_only_credential_shaped_values() {
        for text in [
            "Bearer",
            "hunter2",
            "secret",
            "2026-10-19T00:00:00.000Z",
            "2026-10-19T00:00:00+05:00",
        ] {
            assert!(registered(Leaf::Credential, text).is_empty(), "{text}");
            assert_eq!(registered(Leaf::Path, text), [text], "{text}");
        }
        for text in ["********", "[REDACTED]", "undefined", "  none  "] {
            assert!(registered(Leaf::Credential, text).is_empty(), "{text}");
        }
        for text in [
            "dragonfly",
            "sunshine",
            "hunter2x",
            "p@ss",
            "correct horse battery",
            "sk-proj-0123456789abcdef",
        ] {
            assert_eq!(registered(Leaf::Credential, text)[0], text, "{text}");
        }
    }
}
