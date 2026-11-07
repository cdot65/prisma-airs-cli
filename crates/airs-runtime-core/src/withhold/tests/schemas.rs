//! Rule 3e: JSON Schema property maps and the guardrail catalog.

use super::*;

/// The `evals` row of a guardrail catalog shaped like the service's (Portkey plugin
/// manifests): credential-named parameter schemas (`apiKey`, `credentials`, `token`,
/// `headers`), `description` arrays of `{type, text}`, nested `properties` and `items`.
fn catalog_document(default_key: Option<&str>) -> OValue {
    use serde_json::json;
    let mut api_key = json!({
        "type": "string",
        "label": "API Key",
        "description": [{"type": "subHeading", "text": "The API key of the provider"}],
        "encrypted": true,
    });
    if let Some(value) = default_key {
        api_key["default"] = json!(value);
    }
    let doc = json!({
        "evals": [{
            "description": [{"type": "subHeading", "text": "Checks prompts with a vendor scanner"}],
            "guardrailCategory": "security",
            "id": "vendor.scan",
            "name": "Vendor scan",
            "parameters": {
                "properties": {
                    "apiKey": api_key,
                    "credentials": {
                        "type": "object",
                        "properties": {
                            "client_secret": {"type": "string", "label": "Client secret"},
                            "region": {"type": "string", "default": "us-east-1"},
                        },
                        "required": ["client_secret"],
                    },
                    "token": {"type": ["string", "null"], "description": [{"type": "subHeading", "text": "Optional token"}]},
                    "headers": {"type": "object", "description": [{"type": "subHeading", "text": "Extra headers"}]},
                    "categories": {"type": "array", "items": {"type": "string", "enum": ["prompt", "response"]}},
                    "webhookURL": {"type": "string", "format": "uri"},
                },
                "required": ["apiKey"],
                "type": "object",
            },
            "supportedHooks": ["beforeRequestHook"],
            "type": "guardrail",
        }],
        "object": "list",
    });
    parse(&doc.to_string()).unwrap()
}

/// Choice 146 (live): a JSON Schema property map (`properties`) names schema properties,
/// not values: a credential-named property whose value is a schema object (a JSON Schema
/// `type`) keeps its schema printed, and only the members that hold instance values
/// (`default`, `const`, `enum`, `examples`, any member outside the schema vocabulary)
/// are read under its name. So the guardrail catalog prints as the service sends it, with
/// nothing registered (`string`, `object`, `subHeading` would otherwise withhold every
/// string carrying them and drop the top-level `object` member).
#[test]
fn d238_a_credential_named_schema_withholds_and_registers_no_words() {
    let catalog = catalog_document(None);
    let received = to_compact(&catalog).unwrap();
    let secrets = SecretSet::default();
    let mut doc = catalog.clone();
    apply(&mut doc, &PLAIN, &secrets);
    assert_eq!(to_compact(&doc).unwrap(), received);
    for word in [
        "string",
        "object",
        "subHeading",
        "list",
        "API Key",
        "us-east-1",
        "The API key of the provider",
    ] {
        assert!(!secrets.carried_by(word), "{word} registered");
    }
    // A default value under a credential-named property is its credential.
    let catalog = catalog_document(Some("FAKE-GW-SECRET-51"));
    let secrets = SecretSet::default();
    let mut doc = catalog.clone();
    apply(&mut doc, &PLAIN, &secrets);
    let text = to_compact(&doc).unwrap();
    assert!(!text.contains("FAKE-GW-SECRET-51"), "{text}");
    assert!(text.contains(r#""default":"[withheld]""#), "{text}");
    assert!(text.contains(r#""object":"list""#), "{text}");
    assert!(text.contains(r#""label":"API Key""#), "{text}");
    assert!(secrets.carried_by("FAKE-GW-SECRET-51"));
    assert!(!secrets.carried_by("string"));
    // A `properties` member whose values are not schemas keeps the name rule, and a
    // `type` that is not a JSON Schema type does not make a credential a schema.
    let (text, secrets) = applied(
        r#"{"properties":{"api_key":"FAKE-GW-SECRET-52","credentials":{"type":"basic","password":"pw-short3"},"region":"us"}}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"properties":{"api_key":"[withheld]","credentials":"[withheld]","region":"us"}}"#
    );
    assert!(secrets.carried_by("FAKE-GW-SECRET-52"));
    assert!(secrets.carried_by("pw-short3"));
    assert!(!secrets.carried_by("basic"));
}

/// Choice 150 (review, HIGH): a member of a schema object that holds a value is read
/// under its own name first, so a credential named inside a schema under any property
/// (`{"type": "object", "password": …}`) is withheld and registered, in a resource
/// document, under a policy's paths, in a nested subschema and in a JSON text; the
/// guardrail catalog still prints whole (`d238_a_credential_named_schema_…`).
#[test]
fn d238_a_credential_named_inside_a_schema_is_withheld() {
    for (doc, policy, secret) in [
        (
            r#"{"properties":{"db":{"type":"object","password":"PROBE-PW-001x"}},"note":"PROBE-PW-001x"}"#,
            &PLAIN,
            "PROBE-PW-001x",
        ),
        (
            r#"{"properties":{"openai":{"type":"string","api_key":"FAKE-GW-SECRET-0060"}},"note":"FAKE-GW-SECRET-0060"}"#,
            &PLAIN,
            "FAKE-GW-SECRET-0060",
        ),
        (
            r#"{"properties":{"openai":{"type":"string","key":"sk-0123456789abcdef"}},"note":"sk-0123456789abcdef"}"#,
            &PLAIN,
            "sk-0123456789abcdef",
        ),
        (
            r#"{"properties":{"db":{"type":"string","passwd":"pw-short60"}},"note":"pw-short60"}"#,
            &PLAIN,
            "pw-short60",
        ),
        (
            r#"{"properties":{"svc":{"type":"object","oauth_metadata":{"x":"pw-short61"}}},"note":"pw-short61"}"#,
            &PLAIN,
            "pw-short61",
        ),
        (
            r#"{"configurations":{"properties":{"auth":{"type":"object","client_secret":"pw-short62"}}},"note":"pw-short62"}"#,
            &INTEGRATIONS,
            "pw-short62",
        ),
        (
            r#"{"config":"{\"properties\":{\"t\":{\"type\":\"string\",\"secret\":\"pw-short63\"}}}","note":"pw-short63"}"#,
            &PLAIN,
            "pw-short63",
        ),
        (
            r#"{"properties":{"list":{"type":"array","items":{"type":"object","token":"FAKE-GW-SECRET-0064"}}},"note":"FAKE-GW-SECRET-0064"}"#,
            &PLAIN,
            "FAKE-GW-SECRET-0064",
        ),
        (
            r#"{"properties":{"api_key":{"type":"object","key":"pw-short65"}},"note":"pw-short65"}"#,
            &PLAIN,
            "pw-short65",
        ),
    ] {
        let (text, secrets) = applied(doc, policy);
        assert!(!text.contains(secret), "{text}");
        assert!(text.contains(r#""note":"[withheld]""#), "{text}");
        assert!(secrets.carried_by(secret), "{doc}");
    }
    // A header map inside a schema is read as one; the schema words stay printed.
    let (text, secrets) = applied(
        r#"{"properties":{"svc":{"type":"object","description":"Service","headers":{"x-api":"pw-short66","accept":"*/*"}}}}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"properties":{"svc":{"type":"object","description":"Service","headers":{"x-api":"[withheld]","accept":"*/*"}}}}"#
    );
    assert!(!secrets.carried_by("Service"));
}

/// Choice 152 (review): under a credential-named property a describing string that
/// shows a credential (a long token-shaped word) is withheld in place and not registered;
/// the word after `Bearer` is withheld in place by the scheme rule under any property
/// (choice 177); ordinary describing text, a header name, a URL and the same text under
/// another property stay printed.
#[test]
fn d238_a_credential_in_a_credential_property_description_is_withheld() {
    let (text, secrets) = applied(
        r#"{"properties":{"api_key":{"type":"string","description":"For tests use FAKE-GW-SECRET-0068.","title":"Bearer abcdefgh12345678","label":"API Key","pattern":"^sk-[a-z0-9]{16}$"},"note":{"type":"string","description":"For tests use FAKE-GW-SECRET-0068."}}}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"properties":{"api_key":{"type":"string","description":"[withheld]","title":"Bearer [withheld]","label":"API Key","pattern":"^sk-[a-z0-9]{16}$"},"note":{"type":"string","description":"For tests use FAKE-GW-SECRET-0068."}}}"#
    );
    assert!(!secrets.carried_by("FAKE-GW-SECRET-0068"));
    let ordinary = r#"{"properties":{"apiKey":{"type":"string","description":[{"type":"subHeading","text":"Sent as x-portkey-api-key, see https://docs.example.test/guides/api-keys/rotation"}],"title":"Bearer token","label":"API Key"}}}"#;
    let (text, _) = applied(ordinary, &PLAIN);
    assert_eq!(text, ordinary);
}

/// Choice 188 (review): under a credential-named property an empty `default`, `const`, `enum`
/// or `examples` (the empty string, `[]`, `{}`) prints as TypeScript prints it; a non-empty one
/// is still withheld, an empty value under any other schema member too.
#[test]
fn d238_an_empty_schema_value_prints() {
    let empty = r#"{"properties":{"client_secret":{"type":"string","default":"","const":"","enum":[],"examples":[],"label":"Client secret"},"apiKey":{"type":"object","default":{}}}}"#;
    let (text, secrets) = applied(empty, &PLAIN);
    assert_eq!(text, empty);
    assert!(checked(&text));
    assert_eq!(secrets.form_count(), 0);
    let (text, _) = applied(
        r#"{"properties":{"client_secret":{"type":"string","default":" ","examples":[""],"enum":["PIN-REVIEW-0188x"],"x-sample":""}}}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"properties":{"client_secret":{"type":"string","default":"[withheld]","examples":"[withheld]","enum":"[withheld]","x-sample":"[withheld]"}}}"#
    );
    assert!(checked(&text));
    assert!(!checked(
        r#"{"properties":{"client_secret":{"type":"string","default":" "}}}"#
    ));
}
