//! The name rule (rule 3): classification, the TypeScript floor, credential members and
//! objects, numbers, fingerprints, registration (3g) and the DESIGN table.

use super::*;

#[test]
fn credential_names() {
    for name in [
        "key",
        "API-Key",
        "x-portkey-api-key",
        "client_secret",
        "oauth_client_secret",
        "scim_token",
        "Authorization",
        "set-cookie",
        "aws_secret_access_key",
        "db_password",
        "service_account_credentials",
        "credentials",
        "access_key",
        "secret_key",
        "aws_secret_key",
        "Ocp-Apim-Subscription-Key",
        "signing_key",
        // Choice 157: camelCase boundaries and credential words anywhere in the name.
        "clientSecret",
        "awsSessionToken",
        "bearerToken",
        "adminpassword",
        "secret_value",
        "apiKey",
        "xApiKey",
        "api-key-value",
        "authorizationCode",
        "dAuthCode",
        "sessionCookie",
        "credentialJson",
        // Choice 187: a signature, a passcode, a one-time password, a Vault SecretID and a
        // capability URL named after a stored secret.
        "signature",
        "webhook_signature",
        "X-Signature",
        "passcode",
        "userPasscode",
        "otp",
        "sms_otp",
        "secret_id",
        "vaultSecretId",
        "secret_url",
        "credentials_uri",
        // Choice 163: a word that is or ends with `tokens`.
        "access_tokens",
        "refreshTokens",
        "apiTokens",
        "tokens",
        "prompt_tokens",
        "completion_tokens_details",
    ] {
        assert!(is_credential_name(name), "{name}");
    }
    for name in [
        "api_key_id",
        "apiKeyIds",
        "api_key_defaults_id",
        "total_tokens",
        "max_tokens",
        "secrets_categories",
        "maxTokenAge",
        "secret_mappings",
        "secret_reference_id",
        "masked_api_key",
        "key_id",
        "tokenizer",
        "webhookURL",
        "monkey",
        "virtual_keys",
        "passthrough",
        "compass",
        // Choice 187: what describes a credential (the live `workspaces get` settings and the
        // review's cascade), and `pin`, which names a pinned version as often as a PIN.
        "user_api_key_count",
        "user_api_key_max_ttl",
        "user_api_key_ttl",
        "user_api_key_rotation_period",
        "api_key_name",
        "api_key_last8",
        "apiKeyLastFour",
        "api_key_prefix",
        "token_type",
        "credential_type",
        "token_limit",
        "token_expires_at",
        "clientSecretExpiresAt",
        "secret_created_at",
        "password_hint",
        "secret_version",
        "private_key_id",
        "token_url",
        "oauth2_token_url",
        "token_endpoint",
        "api_key_location",
        "key_transition_period_ms",
        "pin",
        "pin_version",
        "cert_pin",
        "hotpot",
    ] {
        assert!(!is_credential_name(name), "{name}");
    }
    for (name, expected) in [
        ("clientSecret", "client_secret"),
        ("awsSessionToken", "aws_session_token"),
        ("webhookURL", "webhook_url"),
        ("X-API-Key", "x_api_key"),
        ("APIKey", "api_key"),
        ("oauth2Token", "oauth2_token"),
        ("SESSION_TOKEN", "session_token"),
        ("Content-Type", "content_type"),
    ] {
        assert_eq!(normalised(name), expected, "{name}");
    }
}

/// Choices 150, 158 and 172: one ordered [`Class`] decides which of two names a schema
/// member is read under, in the CLI and the checker alike (weakest first).
#[test]
fn classes_order_the_name_rule() {
    let order = [
        ("region", Class::Plain),
        ("custom_headers", Class::HeaderMap),
        ("prompt_tokens", Class::Count),
        ("key", Class::Generic),
        ("secret_data", Class::Credential),
        ("clientSecret", Class::Credential),
        ("model_config", Class::Member),
    ];
    for (name, expected) in order {
        assert_eq!(classify(name), expected, "{name}");
    }
    assert!(order.windows(2).all(|pair| pair[0].1 <= pair[1].1));
    // A schema member holding a value under the stronger name (choice 150).
    let (text, secrets) = applied(
        r#"{"properties":{"key":{"type":"object","adminPassword":"hunter2x-pw","region":"eu-west-1"}}}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"properties":{"key":{"type":"object","adminPassword":"[withheld]","region":"[withheld]"}}}"#
    );
    assert!(secrets.carried_by("hunter2x-pw"));
    assert!(!secrets.carried_by("eu-west-1"), "a generic value");
}

/// Rules 1, 3 and the `SecretSet` rule: a withheld value echoed elsewhere is withheld.
#[test]
fn subtrees_names_and_echoes() {
    let (text, secrets) = applied(
        r#"{"success":true,"data":{"unique_api_keys":["FAKE-GW-SECRET-1"],"n":3}}"#,
        &FILTER_BOUNDARIES,
    );
    assert_eq!(text, r#"{"success":true,"data":"[withheld]"}"#);
    assert!(secrets.carried_by("FAKE-GW-SECRET-1"));
    let (text, _) = applied(
        r#"{"rows":[{"token":"FAKE-GW-SECRET-2","n":1},{"note":"has FAKE-GW-SECRET-2 inside","password":7,"secret":null,"key":{"a":"b"}}],"FAKE-GW-SECRET-2":"x"}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"rows":[{"token":"[withheld]","n":1},{"note":"[withheld]","password":"[withheld]","secret":null,"key":"[withheld]"}]}"#,
        "a number under a credential name is withheld and `null` printed (choices 171, 187)"
    );
    let (_, secrets) = applied(r#"{"token":"abc"}"#, &PLAIN);
    assert!(
        !secrets.carried_by("abc"),
        "short values are not registered"
    );
}

/// The credential members are withheld whole at any depth of a plain document, with the
/// strings of a stringified service account registered, and inside a JSON document held
/// as text (a config's Portkey target), however deep (review B1).
#[test]
fn credential_members_are_withheld_at_any_depth_and_as_json_text() {
    use serde_json::json;
    let account = json!({
        "client_email": "sa@p.example",
        "private_key": "SAKEY-REVIEW-141",
        "type": "service_account",
    })
    .to_string();
    let doc = json!({
        "auth_config": {"vault_role_id": "ROLE-REVIEW-00143", "vault_auth_type": "approle"},
        "model_config": "MODEL-REVIEW-144",
        "name": "bot",
        "rows": [{"nested": {
            "oauth_metadata": {"client_id": "OAUTH-REVIEW-142"},
            "vertex_service_account_json": account,
        }}],
    });
    let (text, secrets) = applied(&doc.to_string(), &PLAIN);
    assert_eq!(
        text,
        r#"{"auth_config":"[withheld]","model_config":"[withheld]","name":"bot","rows":[{"nested":{"oauth_metadata":"[withheld]","vertex_service_account_json":"[withheld]"}}]}"#
    );
    for marker in [
        "SAKEY-REVIEW-141",
        "OAUTH-REVIEW-142",
        "ROLE-REVIEW-00143",
        "MODEL-REVIEW-144",
    ] {
        assert!(secrets.carried_by(marker), "{marker} registered");
    }
    // Choice 147: a plain word inside a withheld credential object is withheld in place,
    // not registered.
    assert!(!secrets.carried_by("approle"));
    // A config text whose Portkey target holds a stringified service account, and the
    // same kind of member inside a JSON text nested in that JSON text.
    let config = json!({
        "meta": json!({"auth_config": {"token_url": "TOKEN-REVIEW-146"}}).to_string(),
        "targets": [{
            "provider": "vertex-ai",
            "vertex_region": "us",
            "vertex_service_account_json": json!({"private_key": "SAKEY-REVIEW-145"}).to_string(),
        }],
    })
    .to_string();
    let (text, secrets) = applied(
        &json!([{"config": config, "name": "c"}]).to_string(),
        &PLAIN,
    );
    assert!(!text.contains("SAKEY-REVIEW-145"), "{text}");
    assert!(!text.contains("TOKEN-REVIEW-146"), "{text}");
    let shown = parse(&text).unwrap();
    let config = shown.as_array().unwrap()[0].get("config").unwrap();
    assert_eq!(
        config.as_str().unwrap(),
        r#"{"meta":"{\"auth_config\":\"[withheld]\"}","targets":[{"provider":"vertex-ai","vertex_region":"us","vertex_service_account_json":"[withheld]"}]}"#
    );
    assert!(secrets.carried_by("SAKEY-REVIEW-145"));
    assert!(secrets.carried_by("TOKEN-REVIEW-146"));
    // A credential name holding JSON text registers the strings inside it too; a JSON
    // text with nothing to withhold stays as received.
    let (text, secrets) = applied(
        &json!({"credentials": json!({"private_key": "SAKEY-REVIEW-147"}).to_string(), "plain": "[1, 2]"})
            .to_string(),
        &PLAIN,
    );
    assert_eq!(text, r#"{"credentials":"[withheld]","plain":"[1, 2]"}"#);
    assert!(secrets.carried_by("SAKEY-REVIEW-147"));
}

/// Review B3: every credential member is withheld by every resource policy (`PLAIN`
/// included, the policy of the config reads) and by the revealed audit rule: at the
/// root, at depth inside rows, inside a JSON document held as text (a config's `config`
/// text) and in a JSON text at depth, whether its value is a string, an object or a
/// stringified JSON document; the credential is registered every time.
#[test]
fn every_credential_member_is_withheld_everywhere() {
    fn json_text_of(doc: &OValue) -> OValue {
        OValue::String(to_compact(doc).unwrap())
    }
    fn object(members: Vec<(&str, OValue)>) -> OValue {
        OValue::Object(
            members
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect(),
        )
    }
    for member in CREDENTIAL_MEMBERS {
        let marker = format!("MEMBER-REVIEW-{member}");
        let values = [
            OValue::String(marker.clone()),
            object(vec![("inner", OValue::String(marker.clone()))]),
            json_text_of(&object(vec![(
                "private_key",
                OValue::String(marker.clone()),
            )])),
        ];
        for value in values {
            let holder = object(vec![(member, value)]);
            let deep = object(vec![(
                "rows",
                OValue::Array(vec![object(vec![("nested", holder.clone())])]),
            )]);
            let placements = [
                holder.clone(),
                deep.clone(),
                object(vec![("config", json_text_of(&holder))]),
                object(vec![(
                    "outer",
                    object(vec![("config", json_text_of(&deep))]),
                )]),
                OValue::Array(vec![object(vec![("config", json_text_of(&deep))])]),
            ];
            for placed in placements {
                let mut doc = placed.clone();
                if let OValue::Object(entries) = &mut doc {
                    entries.push(("shown".into(), OValue::String("visible".into())));
                }
                for policy in POLICIES {
                    let secrets = SecretSet::default();
                    let mut shown = doc.clone();
                    apply(&mut shown, policy, &secrets);
                    let text = to_compact(&shown).unwrap();
                    assert!(!text.contains(&marker), "{member}: {text}");
                    assert!(
                        matches!(shown, OValue::Array(_)) || text.contains("visible"),
                        "{member}: only the member: {text}"
                    );
                    assert!(secrets.carried_by(&marker), "{member} registered: {text}");
                }
                let secrets = SecretSet::default();
                let mut rows =
                    OValue::Array(vec![object(vec![("request_body", json_text_of(&doc))])]);
                apply_audit(&mut rows, true, &secrets);
                let text = to_compact(&rows).unwrap();
                assert!(!text.contains(&marker), "audit {member}: {text}");
                assert!(secrets.carried_by(&marker), "audit {member} registered");
            }
        }
    }
}

/// Choice 145 (review): an object or array under a generic name (`key`, `token`) is
/// withheld whole and the name rule runs inside it, so a short credential under a
/// credential name there (`{"key": {"password": …}}`) is registered and its echo
/// withheld, in an audit body (revealed or not) and in a resource document alike.
#[test]
fn a_credential_inside_a_generic_object_is_registered() {
    let rows = r#"[{"request_body":"{\"key\":{\"password\":\"pw-short1\"},\"name\":\"bot\"}"},{"uri":"/x/pw-short1","request_id":"req-1"}]"#;
    for reveal in [false, true] {
        let secrets = SecretSet::default();
        let mut doc = parse(rows).unwrap();
        apply_audit(&mut doc, reveal, &secrets);
        let text = to_compact(&doc).unwrap();
        assert!(!text.contains("pw-short1"), "reveal {reveal}: {text}");
        assert!(secrets.carried_by("pw-short1"), "reveal {reveal}");
        let records = doc.as_array().unwrap();
        assert_eq!(
            records[1].get("uri").and_then(OValue::as_str),
            Some(WITHHELD)
        );
        assert_eq!(
            records[1].get("request_id").and_then(OValue::as_str),
            Some("req-1")
        );
        if reveal {
            assert_eq!(
                records[0].get("request_body").and_then(OValue::as_str),
                Some(r#"{"key":"[withheld]","name":"bot"}"#)
            );
        }
    }
    let (text, secrets) = applied(
        r#"{"token":[{"secret":"pw-short2"},"word"],"uri":"/x/pw-short2","name":"word-list"}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"token":"[withheld]","uri":"[withheld]","name":"word-list"}"#
    );
    assert!(secrets.carried_by("pw-short2"));
    assert!(
        !secrets.carried_by("word"),
        "a plain word inside is not registered"
    );
}

/// Choice 147 (live): a credential-named member holding an object or array is withheld
/// whole, but only its leaves that are credentials join the `SecretSet` (a string under
/// a credential name inside it, a header value as rule 3 says, a URL part, a string that
/// looks like a credential), never its plain words, so a schema-like value under a
/// credential name (`{"type": "string", "description": …}`) does not withhold every
/// string that carries `string` nor drop a member named `object`.
#[test]
fn d238_a_credential_object_registers_only_its_credentials() {
    let (text, secrets) = applied(
        r#"{"credentials":{"type":"string","description":[{"type":"subHeading","text":"Use the vault"}],"username":"edge","password":"pw-short4","api":"https://user:pw@h.example/x","long":"FAKE-GW-SECRET-0053"},"object":"list","kind":"string","note":"pw-short4 again"}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"credentials":"[withheld]","object":"list","kind":"string","note":"[withheld]"}"#
    );
    for credential in ["pw-short4", "user:pw", "FAKE-GW-SECRET-0053"] {
        assert!(secrets.carried_by(credential), "{credential}");
    }
    for word in ["string", "subHeading", "edge", "Use the vault"] {
        assert!(!secrets.carried_by(word), "{word} registered");
    }
}

/// Choice 151 (review): inside a withheld credential object a string under a
/// non-credential name that looks like a password (8 characters or more, no whitespace,
/// a symbol or letters mixed with digits) is registered, so its echo is withheld; a
/// schema's describing words are not, nor a word-shaped value (`edge`, `approle`,
/// `us-east-1`), nor a date.
#[test]
fn d238_a_short_password_inside_a_credential_object_is_registered() {
    for (doc, policy, secret) in [
        (
            r#"{"client_auth":{"user":"u","pwd":"hunter2x"},"note":"login hunter2x"}"#,
            &DEPLOYMENTS,
            "hunter2x",
        ),
        (
            r#"{"credentials":{"user":{"name":"n","pass":"p4ssw0rd"}},"note":"p4ssw0rd"}"#,
            &PLUGINS,
            "p4ssw0rd",
        ),
        (
            r#"{"credentials":{"pass":"p4ssw0rd"},"note":"p4ssw0rd"}"#,
            &PLUGINS,
            "p4ssw0rd",
        ),
        (
            r#"{"auth_config":{"vault_auth_type":"approle","sid":"s3cr3tid"},"note":"s3cr3tid"}"#,
            &PLAIN,
            "s3cr3tid",
        ),
        (
            r#"{"key":{"pass":"shortpw20"},"note":"shortpw20"}"#,
            &PLAIN,
            "shortpw20",
        ),
        (
            r#"{"credentials":{"user":"u","pass":"P@ss!word","schema":{"type":"string","description":"string text"}},"note":"P@ss!word"}"#,
            &PLAIN,
            "P@ss!word",
        ),
        (
            r#"{"credentials":{"type":"object","description":"Login","default":"pw2short9"},"note":"pw2short9"}"#,
            &PLAIN,
            "pw2short9",
        ),
    ] {
        let (text, secrets) = applied(doc, policy);
        assert!(!text.contains(secret), "{text}");
        assert!(text.contains(r#""note":"[withheld]""#), "{text}");
        assert!(secrets.carried_by(secret), "{doc}");
    }
    let (text, secrets) = applied(
        r#"{"credentials":{"username":"edge","password":"pw-short67","region":"us-east-1","type":"approle","expires_at":"2026-10-19T00:00:00Z","schema":{"type":"string","description":"string text","title":"Login1234"}},"name":"edge us-east-1 approle string text Login1234","created_at":"2026-10-19T00:00:00Z"}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"credentials":"[withheld]","name":"edge us-east-1 approle string text Login1234","created_at":"2026-10-19T00:00:00Z"}"#
    );
    assert!(secrets.carried_by("pw-short67"));
    for word in ["edge", "us-east-1", "approle", "string text", "Login1234"] {
        assert!(!secrets.carried_by(word), "{word} registered");
    }
    for (text, password) in [
        ("hunter2x", true),
        ("P@ss!word", true),
        ("a1b2c3d4", true),
        ("hunter2", false),
        ("us-east-1", false),
        ("prod-eu-west-1", false),
        ("10.0.0.1", false),
        ("approle-auth", false),
        ("2026-10-19T00:00:00Z", false),
        ("https://h.example/v1", false),
        ("two words1", false),
    ] {
        assert_eq!(looks_like_password(text), password, "{text}");
    }
}

/// Choice 154 (review): one registration rule ([`register_leaf`]) for every walker, each
/// value registered once: a credential's value whole and its credential-looking words, a
/// generic one only where it looks like a credential, a string inside a credential also
/// when it looks like a password, a schema's describing string only whole.
#[test]
fn every_walker_registers_through_one_leaf_rule() {
    let leaf = |leaf: Leaf, text: &str| {
        let mut withheld = Vec::new();
        register_leaf(leaf, text, &mut withheld);
        withheld
    };
    let token = "FAKE-GW-SECRET-0070";
    assert_eq!(leaf(Leaf::Credential, token), [token]);
    assert_eq!(
        leaf(Leaf::Credential, &format!("Bearer {token}")),
        [format!("Bearer {token}"), token.to_owned()]
    );
    // Choice 187: a credential name's word-shaped value is withheld in place only; a path
    // rule's is registered whole.
    assert!(leaf(Leaf::Credential, "prod").is_empty());
    assert_eq!(leaf(Leaf::Path, "prod"), ["prod"]);
    assert!(leaf(Leaf::Generic, "prod").is_empty());
    assert_eq!(leaf(Leaf::Generic, &format!("Bearer {token}")), [token]);
    assert_eq!(leaf(Leaf::Inner, "hunter2x"), ["hunter2x"]);
    assert!(leaf(Leaf::Inner, "us-east-1").is_empty());
    assert!(leaf(Leaf::Describing, "hunter2x").is_empty());
    assert!(leaf(Leaf::Describing, &format!("use {token}")).is_empty());
    assert_eq!(leaf(Leaf::Describing, token), [token]);
    // A withheld credential value registers once, through every entry point.
    for doc in [
        format!(r#"{{"api_key":"{token}"}}"#),
        format!(r#"{{"headers":{{"x-auth":"{token}"}}}}"#),
        format!(r#"{{"tags":[{{"key":"x-api-key","value":"{token}"}}]}}"#),
    ] {
        let mut parsed = parse(&doc).unwrap();
        let mut withheld = Vec::new();
        names_and_urls(&mut parsed, &mut withheld);
        assert_eq!(withheld, [token], "{doc}");
    }
}

/// TypeScript's `redactDeep` key pattern (`debug-logger.ts`), the safety floor of the
/// name rule (choice 157): `/token|secret|password|passwd|credential|authorization|
/// cookie|api[-_]?key|client[-_]?auth|auth[-_]?code|^key$/i` over the raw name.
fn typescript_masks(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let squeezed = |a: &str, b: &str| {
        [a.to_owned() + b, format!("{a}-{b}"), format!("{a}_{b}")]
            .iter()
            .any(|form| lower.contains(form.as_str()))
    };
    [
        "token",
        "secret",
        "password",
        "passwd",
        "credential",
        "authorization",
        "cookie",
    ]
    .iter()
    .any(|word| lower.contains(word))
        || squeezed("api", "key")
        || squeezed("client", "auth")
        || squeezed("auth", "code")
        || lower == "key"
}

/// The names TypeScript's pattern masks in camelCase, snake_case, upper-case, prefix and
/// plural forms (one per alternative and form; choices 157, 163), and the names it masks
/// that airs prints by name, each one listed (D238): fingerprints, identifiers and
/// references of a credential, two token counts, two guardrail settings.
const TS_MASKED_NAMES: [&str; 45] = [
    "accessToken",
    "tokenvalue",
    "accesstokenhash",
    "TOKENvalue",
    "ToKen_hash",
    "access_tokens",
    "refreshTokens",
    "apiTokens",
    "tokens",
    "awsSessionToken",
    "bearerToken",
    "token_value",
    "sessiontoken",
    "SESSION_TOKEN",
    "clientSecret",
    "secret_value",
    "clientsecret",
    "SecretAccessKey",
    "adminpassword",
    "dbPassword",
    "password_hash",
    "PASSWORD",
    "userPasswd",
    "passwd_file",
    "credentialJson",
    "serviceCredentials",
    "credential_blob",
    "authorizationHeader",
    "proxyAuthorization",
    "authorization_code",
    "sessionCookie",
    "cookie_jar",
    "apiKey",
    "xApiKey",
    "api-key-value",
    "openaiApiKey",
    "apikeyValue",
    "clientAuth",
    "client_auth_method",
    "authCode",
    "dAuthCode",
    "key",
    "Key",
    "x-functions-key-secret",
    "Authorization",
];

/// The names TypeScript masks that airs prints whatever their value (D238,
/// choices 157, 163, 165, 176); `secret_key` beside a secret anchor is printed too (choice
/// 164, [`d238_a_secret_mapping_key_is_a_location`]).
const TS_MASKED_PRINTED: [&str; 13] = [
    "masked_api_key",
    "secret_path",
    "api_key_id",
    "apiKeyIds",
    "api_key_defaults_id",
    "secret_mappings",
    "secret_reference_id",
    "total_tokens",
    "max_tokens",
    "tokenizer",
    "model_tokenizers",
    "secrets_categories",
    "maxTokenAge",
];

/// A provider detail's `configurations` and a revealed audit body holding a value under
/// each name: `(document, value of name n)`.
fn vocabulary_document(names: &[&str]) -> (String, Vec<String>) {
    let values: Vec<String> = (0..names.len())
        .map(|n| format!("VOCAB-{n:02}-hunter2x-0123456789"))
        .collect();
    let members: Vec<String> = names
        .iter()
        .zip(&values)
        .map(|(name, value)| format!("\"{name}\":\"{value}\""))
        .collect();
    (format!("{{{}}}", members.join(",")), values)
}

/// Choice 157 (review): every name TypeScript's `redactDeep` masks is withheld by
/// airs in a `providers get` detail and in a revealed audit body, but the listed
/// exceptions, which TypeScript masks and airs prints; nothing withheld is
/// printed where it is echoed.
#[test]
fn d238_every_name_typescript_masks_is_withheld() {
    for name in TS_MASKED_NAMES.iter().chain(&TS_MASKED_PRINTED) {
        assert!(typescript_masks(name), "{name}: TypeScript masks it");
    }
    let (configurations, values) = vocabulary_document(&TS_MASKED_NAMES);
    let echo = values.join(" ");
    let provider = format!(r#"{{"id":"p1","configurations":{configurations},"note":"{echo}"}}"#);
    let (text, _) = applied(&provider, &PROVIDERS);
    for value in &values {
        assert!(!text.contains(value.as_str()), "{value}: {text}");
    }
    let doc = parse(&text).unwrap();
    for name in TS_MASKED_NAMES {
        assert_eq!(
            doc.get("configurations").and_then(|c| c.get(name)),
            Some(&OValue::String(WITHHELD.into())),
            "{name}"
        );
    }
    let audit = |names: &[&str]| {
        let (body, values) = vocabulary_document(names);
        let rows = OValue::Array(vec![OValue::Object(vec![
            ("request_body".into(), OValue::String(body)),
            ("uri".into(), OValue::String(values.join(" "))),
        ])]);
        let secrets = SecretSet::default();
        let mut rows = rows;
        apply_audit(&mut rows, true, &secrets);
        (to_compact(&rows).unwrap(), values)
    };
    let (text, values) = audit(&TS_MASKED_NAMES);
    for value in &values {
        assert!(!text.contains(value.as_str()), "{value}: {text}");
    }
    // The exceptions: printed in both documents.
    let (configurations, values) = vocabulary_document(&TS_MASKED_PRINTED);
    let (text, _) = applied(
        &format!(r#"{{"id":"p1","configurations":{configurations}}}"#),
        &PROVIDERS,
    );
    for value in &values {
        assert!(text.contains(value.as_str()), "{value}: {text}");
    }
    let (text, values) = audit(&TS_MASKED_PRINTED);
    for value in &values {
        assert!(text.contains(value.as_str()), "{value}: {text}");
    }
}

/// Choice 157 (review): the names of the review, camelCase and with the credential word
/// not a suffix, are withheld and registered in a `providers get` detail and a revealed
/// audit body, each registered whole as a credential name's value, so its echo is withheld;
/// `kv/ai` under `secret_data` is withheld in place only, a short word-shaped value (choice
/// 187), so the same path elsewhere prints; a secret reference's `secret_path` is printed
/// (`names::PRINTED_NAMES`).
#[test]
fn d238_camel_case_and_worded_credential_names_are_withheld() {
    let configurations = r#"{"clientSecret":"CS-REVIEW-0001x","awsSessionToken":"AST-REVIEW-0002x","bearerToken":"BT-REVIEW-0003x","adminpassword":"AP-REVIEW-0004x","secret_value":"SV-REVIEW-0005x","secret_data":"kv/ai","secret_path":"kv/app","region":"us-east-1"}"#;
    let echo = "CS-REVIEW-0001x AST-REVIEW-0002x BT-REVIEW-0003x AP-REVIEW-0004x SV-REVIEW-0005x";
    let (text, secrets) = applied(
        &format!(
            r#"{{"configurations":{configurations},"note":"{echo}","path":"kv/ai","slug":"openai-prod"}}"#
        ),
        &PROVIDERS,
    );
    assert_eq!(
        text,
        r#"{"configurations":{"clientSecret":"[withheld]","awsSessionToken":"[withheld]","bearerToken":"[withheld]","adminpassword":"[withheld]","secret_value":"[withheld]","secret_data":"[withheld]","secret_path":"kv/app","region":"us-east-1"},"note":"[withheld]","path":"kv/ai","slug":"openai-prod"}"#
    );
    for value in echo.split(' ') {
        assert!(secrets.carried_by(value), "{value}");
    }
    assert!(!secrets.carried_by("kv/ai"), "choice 187");
    assert!(
        !secrets.carried_by("us-east-1"),
        "an ordinary member beside them"
    );
    let secrets = SecretSet::default();
    let mut rows = OValue::Array(vec![OValue::Object(vec![
        ("request_body".into(), OValue::String(configurations.into())),
        ("uri".into(), OValue::String(format!("/x/{echo}"))),
    ])]);
    apply_audit(&mut rows, true, &secrets);
    assert_eq!(
        to_compact(&rows).unwrap(),
        r#"[{"request_body":"{\"clientSecret\":\"[withheld]\",\"awsSessionToken\":\"[withheld]\",\"bearerToken\":\"[withheld]\",\"adminpassword\":\"[withheld]\",\"secret_value\":\"[withheld]\",\"secret_data\":\"[withheld]\",\"secret_path\":\"kv/app\",\"region\":\"us-east-1\"}","uri":"[withheld]"}]"#
    );
}

/// Choice 161 (review): the word after an authorization scheme is registered with the
/// whole value (in a credential's value always, under a generic name or a non-credential
/// header when it looks like a credential or a password), so `hunter2x` of
/// `Bearer hunter2x` is withheld where it is echoed bare; the scheme word never is.
#[test]
fn d238_the_word_after_an_authorization_scheme_is_registered() {
    for doc in [
        r#"{"authorization":"Bearer hunter2x","note":"hunter2x"}"#,
        r#"{"token":"Basic dXNlcjpwYXNzMQ==","note":"dXNlcjpwYXNzMQ=="}"#,
        r#"{"authorization":"Bearer OBJ-REVIEW-02","note":"OBJ-REVIEW-02"}"#,
        r#"{"headers":{"x-custom-auth":"Token p4ssw0rd"},"note":"p4ssw0rd"}"#,
    ] {
        let (text, secrets) = applied(doc, &PLAIN);
        assert!(text.ends_with(r#""note":"[withheld]"}"#), "{doc}: {text}");
        assert!(!secrets.carried_by("Bearer"), "{doc}");
        assert!(!secrets.carried_by("a Basic auth"), "{doc}");
    }
}

/// Choice 160 (review): only a whole ISO date or timestamp is a date; a credential that
/// starts with a date is registered.
#[test]
fn d238_only_a_whole_timestamp_is_a_date() {
    for date in [
        "2026-10-19",
        "2026-10-19T00:00",
        "2026-10-19T00:00:00Z",
        "2026-10-01T12:00:00.000Z",
        "2026-10-19T05:30:00+05:30",
        "2026-10-19T05:30:00.123456-08:00",
    ] {
        assert!(is_date(date), "{date}");
        let (_, secrets) = applied(&format!(r#"{{"credentials":{{"at":"{date}"}}}}"#), &PLAIN);
        assert!(!secrets.carried_by(date), "{date}");
    }
    for text in [
        "2026-01-01-k3y5ecr3tvalue",
        "2026-10-19T00:00:00Zsecret",
        "2026-10-19Tsecret",
        "2026-10-19 hunter",
        "20261019T000000Z",
        "2026-10-19T00:00:00+0530",
    ] {
        assert!(!is_date(text), "{text}");
    }
    let (text, secrets) = applied(
        r#"{"credentials":{"user":"u","sid":"2026-01-01-k3y5ecr3tvalue"},"note":"2026-01-01-k3y5ecr3tvalue"}"#,
        &PLAIN,
    );
    assert_eq!(text, r#"{"credentials":"[withheld]","note":"[withheld]"}"#);
    assert!(secrets.carried_by("2026-01-01-k3y5ecr3tvalue"));
}

/// Choice 165 (review): an object or array under a credential or generic name with no
/// text inside (a model price, a usage breakdown) is printed as it is, as a number under
/// that name is; one with a string, or keyed by a credential, is withheld; a credential
/// member is withheld whole whatever it holds.
#[test]
fn d238_a_value_without_text_under_a_credential_name_is_printed() {
    let models = r#"{"models":[{"slug":"gpt-4o","pricing_config":{"currency":"USD","pay_as_you_go":{"request_token":{"price":0.5},"response_token":{"price":1.5}}}}]}"#;
    let (text, secrets) = applied(models, &INTEGRATIONS);
    assert_eq!(text, models, "the prices are printed");
    assert!(!secrets.carried_by("gpt-4o") && !secrets.carried_by("USD"));
    let (text, _) = applied(
        r#"{"usage":{"completion_tokens":4,"completion_tokens_details":{"reasoning_tokens":1},"tokens":[1,2]}}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"usage":{"completion_tokens":4,"completion_tokens_details":{"reasoning_tokens":1},"tokens":[1,2]}}"#
    );
    let (text, secrets) = applied(
        r#"{"access_tokens":["FAKEGW-tokenlist-0001"],"refreshTokens":{"ttl":5,"value":"FAKEGW-refresh-0002"},"api_tokens":{"sk-0123456789abcdef0":1},"model_config":{"a":1},"note":"FAKEGW-tokenlist-0001 FAKEGW-refresh-0002"}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"access_tokens":"[withheld]","refreshTokens":"[withheld]","api_tokens":"[withheld]","model_config":"[withheld]","note":"[withheld]"}"#
    );
    assert!(secrets.carried_by("FAKEGW-tokenlist-0001"));
    assert!(secrets.carried_by("FAKEGW-refresh-0002"));
}

/// Choice 164 (review): a secret mapping's `secret_key` (beside `secret_reference_id`) and
/// a secret reference's (beside `secret_path`) name a key inside the referenced secret:
/// printed, never registered, so the mapping's `target_field`, an unrelated `note` and
/// the `masked_api_key` fingerprint survive; anywhere else `secret_key` is a credential.
#[test]
fn d238_a_secret_mapping_key_is_a_location() {
    let provider = r#"{"id":"p1","key":"FAKEGW-provider-key-0001","masked_api_key":"sk-...abcd","note":"rotate the api_key monthly","secret_mappings":[{"target_field":"api_key","secret_reference_id":"sr-1","secret_key":"api_key","value_format":null}]}"#;
    let (text, secrets) = applied(provider, &PROVIDERS);
    assert_eq!(
        text,
        r#"{"id":"p1","key":"[withheld]","masked_api_key":"sk-...abcd","note":"rotate the api_key monthly","secret_mappings":[{"target_field":"api_key","secret_reference_id":"sr-1","secret_key":"api_key","value_format":null}]}"#
    );
    assert!(!secrets.carried_by("api_key"));
    assert!(secrets.carried_by("FAKEGW-provider-key-0001"));
    // A secret-reference body in a revealed audit record: `secret_key` printed beside
    // `secret_path`, the `auth_config` withheld.
    let body = r#"{"name":"vault","secret_path":"kv/openai","secret_key":"OPENAI_KEY","auth_config":{"vault_token":"FAKEGW-vault-0003"}}"#;
    let mut rows = OValue::Array(vec![OValue::Object(vec![(
        "request_body".into(),
        OValue::String(body.into()),
    )])]);
    let secrets = SecretSet::default();
    apply_audit(&mut rows, true, &secrets);
    assert_eq!(
        to_compact(&rows).unwrap(),
        r#"[{"request_body":"{\"name\":\"vault\",\"secret_path\":\"kv/openai\",\"secret_key\":\"OPENAI_KEY\",\"auth_config\":\"[withheld]\"}"}]"#
    );
    assert!(!secrets.carried_by("OPENAI_KEY"));
    assert!(secrets.carried_by("FAKEGW-vault-0003"));
    // Elsewhere a credential.
    let (text, secrets) = applied(
        r#"{"aws":{"secret_key":"FAKEGW-aws-0004x","region":"us"}}"#,
        &PLAIN,
    );
    assert_eq!(text, r#"{"aws":{"secret_key":"[withheld]","region":"us"}}"#);
    assert!(secrets.carried_by("FAKEGW-aws-0004x"));
}

/// Choice 170 (review): `pwd`, `passphrase`, `auth`, the `_key` and `_auth` suffixes and a
/// space or `.` as a separator name a credential; the key names, the virtual key, the
/// `rotate_auth` flag and `auth_type` stay printed.
#[test]
fn d238_more_credential_name_shapes_are_withheld() {
    let names = [
        "pwd",
        "db_pwd",
        "passphrase",
        "keyPassphrase",
        "auth",
        "basic_auth",
        "account_key",
        "master_key",
        "encryptionKey",
        "API KEY",
        "api.key",
        "Api Key",
    ];
    let members: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(at, name)| format!("\"{name}\":\"SHAPE-{at:02}-hunter2x\""))
        .collect();
    let (text, secrets) = applied(&format!("{{{}}}", members.join(",")), &PLAIN);
    let doc = parse(&text).unwrap();
    for (at, name) in names.iter().enumerate() {
        assert_eq!(
            doc.get(name),
            Some(&OValue::String(WITHHELD.into())),
            "{name}"
        );
        assert!(
            secrets.carried_by(&format!("SHAPE-{at:02}-hunter2x")),
            "{name}"
        );
    }
    let printed = r#"{"headerKey":"Authorization","metadataKey":["env"],"value_key":"cost","virtual_key":"openai-prod","virtual_keys":["openai-prod"],"rotate_auth":true,"auth_type":"oauth","auth_settings":{"mode":"sso"},"passthrough":"on","compass":"n","bypass_cache":"yes","rules_passed":"all","tokenizer":"cl100k","api_key_ids":["k1"]}"#;
    assert_eq!(applied(printed, &PLAIN).0, printed);
}

/// Choice 176 (review): `pass` and `pat` as whole words, the `_keys` suffix and `keys`,
/// `token` anywhere in a word but `tokenizer`, and a connection string name a credential,
/// in a resource document, a name/value pair and a revealed audit body; the names around
/// them stay printed.
#[test]
fn d238_pass_keys_and_run_together_token_names_are_withheld() {
    let names = [
        "db_pass",
        "smtp_pass",
        "DB_PASS",
        "smtpPass",
        "pass",
        "GITHUB_PAT",
        "access_keys",
        "keys",
        "tokenvalue",
        "tokenhash",
        "accesstokenvalue",
        "connection_string",
    ];
    let members: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(at, name)| format!("\"{name}\":\"WORD-{at:02}-hunter2x-0123456789\""))
        .collect();
    let (text, secrets) = applied(&format!("{{{}}}", members.join(",")), &DEPLOYMENTS);
    let doc = parse(&text).unwrap();
    for (at, name) in names.iter().enumerate() {
        assert_eq!(
            doc.get(name),
            Some(&OValue::String(WITHHELD.into())),
            "{name}"
        );
        assert!(
            secrets.carried_by(&format!("WORD-{at:02}-hunter2x-0123456789")),
            "{name}"
        );
    }
    let (text, secrets) = applied(
        r#"{"deployment_config":{"env":[{"name":"DB_PASS","value":"hunter2x!"},{"name":"LOG_LEVEL","value":"info"}]},"access_keys":["AKIAEXAMPLE/wJalrXUtnFEMI/K7MDENG"],"keys":["sk-WORDKEYS0123456789"]}"#,
        &DEPLOYMENTS,
    );
    assert_eq!(
        text,
        r#"{"deployment_config":{"env":[{"name":"DB_PASS","value":"[withheld]"},{"name":"LOG_LEVEL","value":"info"}]},"access_keys":"[withheld]","keys":"[withheld]"}"#
    );
    assert!(secrets.carried_by("hunter2x!"));
    assert!(secrets.carried_by("AKIAEXAMPLE/wJalrXUtnFEMI/K7MDENG"));
    assert!(secrets.carried_by("sk-WORDKEYS0123456789"));
    let mut rows = OValue::Array(vec![OValue::Object(vec![(
        "request_body".into(),
        OValue::String(
            r#"{"db_pass":"hunter2x!","smtp_pass":"p4ssw0rd!","tokenvalue":"abcdefghij0123456789xyz","passthrough":"on"}"#
                .into(),
        ),
    )])]);
    apply_audit(&mut rows, true, &SecretSet::default());
    assert_eq!(
        to_compact(&rows).unwrap(),
        r#"[{"request_body":"{\"db_pass\":\"[withheld]\",\"smtp_pass\":\"[withheld]\",\"tokenvalue\":\"[withheld]\",\"passthrough\":\"on\"}"}]"#
    );
}

/// Choices 171 and 187 (review: `{"password": 12345678}` printed): a number is withheld under
/// a credential name, a generic name, a path rule and inside a withheld credential object,
/// and registered as a credential's value when it has at least 8 characters (so its echo is
/// withheld and a short one never withholds a date); the empty string under a credential name
/// is withheld; a count, `null`, a boolean and the workspace settings whose names describe a
/// credential (`user_api_key_count`, `user_api_key_auto_create`) print, as TypeScript prints
/// them on `workspaces get`.
#[test]
fn d238_numbers_under_credential_names_and_path_rules_are_withheld_counts_and_flags_print() {
    let (text, secrets) = applied(
        r#"{"password":12345678,"pin_code":{"secret":4242},"credentials":{"pin":87654321},"key":7,"is_secret":true,"secret":null,"api_key":"","prompt_tokens":5,"usage":{"completion_tokens":2},"user_api_key_count":5,"user_api_key_auto_create":false,"note":"call 87654321","echo":"pin 12345678","year":"2026"}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"password":"[withheld]","pin_code":{"secret":"[withheld]"},"credentials":"[withheld]","key":"[withheld]","is_secret":true,"secret":null,"api_key":"[withheld]","prompt_tokens":5,"usage":{"completion_tokens":2},"user_api_key_count":5,"user_api_key_auto_create":false,"note":"call 87654321","echo":"[withheld]","year":"2026"}"#
    );
    assert!(secrets.carried_by("12345678"), "a numeric password");
    assert!(!secrets.carried_by("4242"), "a short number in place only");
    assert!(checked(&text));
    // A plugin's `credentials.*` path rule and a revealed audit body.
    let (text, _) = applied(
        r#"{"credentials":{"port":5432,"api_key":99887766}}"#,
        &PLUGINS,
    );
    assert_eq!(text, r#"{"credentials":"[withheld]"}"#);
    let mut rows = OValue::Array(vec![OValue::Object(vec![(
        "request_body".into(),
        OValue::String(
            r#"{"credentials":{"pin":11223344},"password":55667788,"retries":3}"#.into(),
        ),
    )])]);
    apply_audit(&mut rows, true, &SecretSet::default());
    assert_eq!(
        to_compact(&rows).unwrap(),
        r#"[{"request_body":"{\"credentials\":\"[withheld]\",\"password\":\"[withheld]\",\"retries\":3}"}]"#
    );
    let mut rows = OValue::Array(vec![OValue::Object(vec![(
        "query_params".into(),
        parse("12345").unwrap(),
    )])]);
    apply_audit(&mut rows, false, &SecretSet::default());
    assert_eq!(
        to_compact(&rows).unwrap(),
        r#"[{"query_params":"[withheld]"}]"#
    );
}

/// Choice 187 (review probe): on the integrations reads a number under a credential name, a
/// generic name, a credential header and a credential pair is withheld, as TypeScript's
/// `redactDeep` masks it on the inventory gets, and an eight-character one is registered, so
/// its echo under a plain name is withheld too; a generic header's number prints.
#[test]
fn d238_numbers_under_credential_headers_and_pairs_are_withheld() {
    let (text, secrets) = applied(
        r#"{"password":12345678,"api_key":9876543210123,"token":123456789}"#,
        &INTEGRATIONS,
    );
    assert_eq!(
        text,
        r#"{"password":"[withheld]","api_key":"[withheld]","token":"[withheld]"}"#
    );
    assert!(secrets.carried_by("12345678") && secrets.carried_by("9876543210123"));
    let (text, secrets) = applied(
        r#"{"rows":[{"headers":{"Authorization":23456789,"x-retries":3}},{"tags":[{"key":"password","value":34567890},{"key":"region","value":7}]},{"note":"23456789 and 34567890"}]}"#,
        &INTEGRATIONS,
    );
    assert_eq!(
        text,
        r#"{"rows":[{"headers":{"Authorization":"[withheld]","x-retries":3}},{"tags":[{"key":"password","value":"[withheld]"},{"key":"region","value":7}]},{"note":"[withheld]"}]}"#
    );
    assert!(secrets.carried_by("23456789") && secrets.carried_by("34567890"));
    assert!(checked(&text));
}

/// Choice 187 (review: a dictionary password's echo printed): a credential name's value of at
/// least eight characters joins the `SecretSet` whole, so `dragonfly` and `sunshine` are
/// withheld where they are echoed; a placeholder, a mask, a date and a shorter word are
/// withheld in place only.
#[test]
fn d238_a_weak_password_under_a_credential_name_withholds_its_echo() {
    let (text, secrets) = applied(
        r#"[{"password":"dragonfly"},{"description":"dragonfly"},{"secret":"sunshine"},{"note":"pw sunshine"}]"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"[{"password":"[withheld]"},{"description":"[withheld]"},{"secret":"[withheld]"},{"note":"[withheld]"}]"#
    );
    assert!(secrets.carried_by("dragonfly") && secrets.carried_by("sunshine"));
    let (text, secrets) = applied(
        r#"[{"password":"********","secret":"undefined","client_secret":"2026-10-19","api_key":"Bearer"},{"mask":"********","state":"undefined","day":"2026-10-19","note":"Send a Bearer header"}]"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"[{"password":"[withheld]","secret":"[withheld]","client_secret":"[withheld]","api_key":"[withheld]"},{"mask":"********","state":"undefined","day":"2026-10-19","note":"Send a Bearer header"}]"#
    );
    for word in ["********", "undefined", "2026-10-19", "Bearer"] {
        assert!(!secrets.carried_by(word), "{word} registered");
    }
}

/// Choice 187 (review: `api_key_hint: "sk-live-…"` and `token_prefix: "ghp_…"` printed): a
/// fingerprint prints unless its value looks like a credential, which is withheld and
/// registered as a credential's value, so its echo is withheld too.
#[test]
fn d238_a_credential_under_a_fingerprint_name_is_withheld() {
    let (text, secrets) = applied(
        r#"{"api_key_hint":"sk-live-1234567890abcdefghij","token_prefix":"ghp_0123456789abcdefABCDEF","api_key_last4":"wxyz","apiKeyLast8":"abcd1234","secret_hint":"the one ending in wxyz","note":"sk-live-1234567890abcdefghij"}"#,
        &INTEGRATIONS,
    );
    assert_eq!(
        text,
        r#"{"api_key_hint":"[withheld]","token_prefix":"[withheld]","api_key_last4":"wxyz","apiKeyLast8":"abcd1234","secret_hint":"the one ending in wxyz","note":"[withheld]"}"#
    );
    assert!(secrets.carried_by("ghp_0123456789abcdefABCDEF"));
    assert!(
        !secrets.carried_by("abcd1234"),
        "a fingerprint is not registered"
    );
    assert!(checked(&text));
    assert!(!checked(
        r#"{"api_key_hint":"sk-live-1234567890abcdefghij"}"#
    ));
}

/// Choice 172: DESIGN §19.3 "Rule 3 by name" is the rows of `names::NAME_RULES`
/// (`names::rules_table`), verbatim, so the contract and the code name the same rules.
#[test]
fn the_design_table_is_the_name_rules() {
    let design =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/DESIGN.md"))
            .unwrap();
    // The table sits in a list item of rule 3: compare with each line's indentation removed.
    let design: String = design
        .lines()
        .map(|line| format!("{}\n", line.trim_start()))
        .collect();
    let table = names::rules_table();
    assert!(design.contains(&table), "DESIGN §19.3 must hold:\n{table}");
}

/// Choice 187 (review: the COMPATIBILITY rows of the printed names and of rule 3 by shape
/// said TypeScript prints these values "on the reads that send them" while citing the
/// `redactDeep` that masks them): each row names the reads TypeScript masks on (every
/// inventory get and `audit-logs list`) and the one that prints (`workspaces get`).
#[test]
fn the_compatibility_rows_name_the_reads_typescript_masks() {
    let checklist = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/COMPATIBILITY.md"
    ))
    .unwrap();
    for row in ["| Rule 3a, the printed names:", "| Rule 3, by shape:"] {
        let line = checklist
            .lines()
            .find(|line| line.starts_with(row))
            .unwrap_or_else(|| panic!("no row {row}"));
        for read in [
            "every inventory get",
            "`audit-logs list`",
            "`inventory.ts:387`",
        ] {
            assert!(line.contains(read), "{row} names {read}");
        }
        assert!(line.contains("`workspaces get`"), "{row}");
        for stale in [
            "on the reads that send them",
            "the empty string printed under every name",
        ] {
            assert!(!line.contains(stale), "{row} still says {stale:?}");
        }
    }
}

/// Choice 187 (review: the name rule's registration cascaded): a word-shaped value shorter
/// than eight characters under a credential name is withheld in place but never joins the
/// `SecretSet`, and the names whose last words describe a credential are no credential names
/// at all, so `Bearer`, `production`, `service_account`, `unlimited` and a timestamp print
/// everywhere else; a value shaped as a credential or a password still registers and
/// withholds its echo.
#[test]
fn d238_word_shaped_values_do_not_cascade() {
    let (text, secrets) = applied(
        r#"{"rows":[{"token_type":"Bearer","api_key_name":"production","credential_type":"service_account","token_limit":"unlimited","token_expires_at":"2026-10-01T12:00:00.000Z","clientSecretExpiresAt":"2026-11-01T00:00:00Z","password":"Bearer","secret":"staging"},{"note":"Send a Bearer header","name":"Bearer","env":"production","type":"service_account","quota":"unlimited","created_at":"2026-10-01T12:00:00.000Z","renew":"2026-11-01T00:00:00Z"},{"secret_key":"p@ss","access_token":"hunter2x","echo":"p@ss and hunter2x"}]}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"rows":[{"token_type":"Bearer","api_key_name":"production","credential_type":"service_account","token_limit":"unlimited","token_expires_at":"2026-10-01T12:00:00.000Z","clientSecretExpiresAt":"2026-11-01T00:00:00Z","password":"[withheld]","secret":"[withheld]"},{"note":"Send a Bearer header","name":"Bearer","env":"production","type":"service_account","quota":"unlimited","created_at":"2026-10-01T12:00:00.000Z","renew":"2026-11-01T00:00:00Z"},{"secret_key":"[withheld]","access_token":"[withheld]","echo":"[withheld]"}]}"#
    );
    for word in ["Bearer", "production", "2026-11-01T00:00:00Z"] {
        assert!(!secrets.carried_by(word), "{word} registered");
    }
    for value in ["p@ss", "hunter2x"] {
        assert!(secrets.carried_by(value), "{value}");
    }
    assert!(checked(&text));
}

/// Choice 188 (review): a state, mode or grant-type word under a credential name is withheld
/// in place and never registered, so the same word under a status or in prose prints; a
/// credential name's value that repeats a sibling `*_type` or `*_mode` is a mode, withheld in
/// place only; `password` with no such sibling, and a real password beside one, still withhold
/// their echo.
#[test]
fn d238_mode_words_under_credential_names_do_not_cascade() {
    let (text, secrets) = applied(
        r#"{"rows":[{"auth":"disabled","status":"disabled","note":"Server is disabled"},{"auth":"required","policy":"required"},{"credential":"optional","field":"optional"},{"auth":"anonymous","user":"anonymous"},{"auth":"password","auth_type":"password","help":"Enter the password"},{"auth":"oauth2_client_credentials","grant_type":"oauth2_client_credentials"}]}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"rows":[{"auth":"[withheld]","status":"disabled","note":"Server is disabled"},{"auth":"[withheld]","policy":"required"},{"credential":"[withheld]","field":"optional"},{"auth":"[withheld]","user":"anonymous"},{"auth":"[withheld]","auth_type":"password","help":"Enter the password"},{"auth":"[withheld]","grant_type":"oauth2_client_credentials"}]}"#
    );
    for word in [
        "disabled",
        "required",
        "optional",
        "anonymous",
        "password",
        "oauth2_client_credentials",
    ] {
        assert!(!secrets.carried_by(word), "{word} registered");
    }
    assert!(checked(&text));
    // Without the sibling `password` is a credential's value, registered; a real password
    // beside a `*_type` that repeats it is registered too (it names no kind of credential).
    let (text, secrets) = applied(
        r#"{"rows":[{"secret":"password"},{"note":"the password"},{"auth":"hunter2x-pw","auth_type":"hunter2x-pw"}]}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"rows":[{"secret":"[withheld]"},{"note":"[withheld]"},{"auth":"[withheld]","auth_type":"[withheld]"}]}"#
    );
    assert!(secrets.carried_by("password"));
    assert!(secrets.carried_by("hunter2x-pw"));
}
