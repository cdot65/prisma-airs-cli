//! Rule 3c: header maps, header lists and `{key|name|header, value}` pairs.

use super::*;

/// D238 (review): a header map at any depth of every document (a model's
/// `configurations.custom_headers`, a webhook guardrail's `parameters.headers`, a
/// `{key, value}` pair or a list of them, a map held as JSON text) prints its header
/// names and only the safe headers' values; every other value is withheld and
/// registered, so a request body's header joins the `SecretSet` (`models set`).
#[test]
fn d238_header_maps_withhold_every_value_but_the_safe_headers() {
    let (text, secrets) = applied(
        r#"{"models":[{"slug":"m1","configurations":{"custom_headers":{"key":"X-Tenant-Auth","value":"FAKE-GW-SECRET-41"}}},{"slug":"m2","configurations":{"custom_headers":{"X-Tenant":"FAKE-GW-SECRET-42","Content-Type":"application/json"}}},{"slug":"m3","configurations":{"custom_headers":[{"key":"x-a","value":"FAKE-GW-SECRET-43"},{"key":"accept","value":"*/*"}]}}],"tags":[{"key":"env","value":"prod"}]}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"models":[{"slug":"m1","configurations":{"custom_headers":{"key":"X-Tenant-Auth","value":"[withheld]"}}},{"slug":"m2","configurations":{"custom_headers":{"X-Tenant":"[withheld]","Content-Type":"application/json"}}},{"slug":"m3","configurations":{"custom_headers":[{"key":"x-a","value":"[withheld]"},{"key":"accept","value":"*/*"}]}}],"tags":[{"key":"env","value":"prod"}]}"#
    );
    for marker in [
        "FAKE-GW-SECRET-41",
        "FAKE-GW-SECRET-42",
        "FAKE-GW-SECRET-43",
    ] {
        assert!(secrets.carried_by(marker), "{marker} registered");
    }
    assert!(!secrets.carried_by("X-Tenant-Auth"), "a header name");
    assert!(!secrets.carried_by("prod"), "a tag value");
    let (text, secrets) = applied(
        r#"[{"checks":[{"id":"default.webhook","parameters":{"webhookURL":"https://hook.example/x","headers":{"X-Functions-Key":"LEAK-1","X-Auth-Key":"LEAK-22x9","user-agent":"bot/1"}}}],"name":"wh"}]"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"[{"checks":[{"id":"default.webhook","parameters":{"webhookURL":"https://hook.example/x","headers":{"X-Functions-Key":"[withheld]","X-Auth-Key":"[withheld]","user-agent":"bot/1"}}}],"name":"wh"}]"#
    );
    assert!(secrets.carried_by("LEAK-22x9"));
    // A header map held as JSON text, and one inside a config's JSON text.
    let (text, _) = applied(
        r#"{"headers":"{\"x-h\":\"FAKE-GW-SECRET-44\",\"host\":\"h\"}","config":"{\"targets\":[{\"custom_headers\":{\"x-k\":\"FAKE-GW-SECRET-45\"}}]}","plain":"x"}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"headers":"{\"x-h\":\"[withheld]\",\"host\":\"h\"}","config":"{\"targets\":[{\"custom_headers\":{\"x-k\":\"[withheld]\"}}]}","plain":"x"}"#
    );
    // A text not holding JSON is withheld whole; `null`, a boolean and a generic header's
    // number stay; a credential header's number and empty string are withheld (choice 187).
    let (text, _) = applied(
        r#"{"headers":"X-Api: FAKE-GW-SECRET-46","custom_headers":null,"request_headers":{"n":3,"b":true,"x-api-key":"","x-auth-token":12345678}}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"headers":"[withheld]","custom_headers":null,"request_headers":{"n":3,"b":true,"x-api-key":"[withheld]","x-auth-token":"[withheld]"}}"#
    );
}

/// D238 (review, choice 139): a configuration header's value (`x-env: prod`,
/// `anthropic-version: <date>`) is withheld in place in every document, by a path rule
/// (`configurations.custom_headers.*`) or a header map, but not registered, so a
/// `created_at` on that date, a name containing it and a member named after it stay
/// printed; a credential header's value, a value that looks like a credential and the
/// token of a `Bearer <token>` value under any header name are registered.
#[test]
fn d238_a_short_configuration_header_does_not_withhold_other_data() {
    let headers = r#"{"anthropic-version":"2026-10-01","x-env":"prod","x-auth-key":"LEAK-7x7k","x-upstream":"Bearer FAKE-GW-SECRET-47"}"#;
    let tail =
        r#""name":"prod-gateway","created_at":"2026-10-01T12:00:00.000Z","prod_region":"us""#;
    let path = Policy {
        paths: &[&["configurations", "custom_headers", "*"]],
        ..PLAIN
    };
    for (policy, map) in [
        (&path, headers.to_owned()),
        (&PLAIN, headers.to_owned()),
        (
            &path,
            r#"[{"key":"x-env","value":"prod"},{"key":"Authorization","value":"Bearer abc"}]"#
                .to_owned(),
        ),
    ] {
        let (text, secrets) = applied(
            &format!(r#"{{"configurations":{{"custom_headers":{map}}},{tail}}}"#),
            policy,
        );
        assert!(text.ends_with(&format!("{tail}}}")), "{text}");
        assert!(!text.contains(r#""prod""#), "{text}");
        for word in ["prod", "2026-10-01", "x-env", "Authorization"] {
            assert!(!secrets.carried_by(word), "{word} registered: {text}");
        }
        if map.starts_with('{') {
            assert!(
                !text.contains("LEAK-7x7k") && !text.contains("FAKE-GW"),
                "{text}"
            );
            assert!(secrets.carried_by("LEAK-7x7k"), "a credential header");
            assert!(secrets.carried_by("FAKE-GW-SECRET-47"), "a bearer token");
        } else {
            assert!(secrets.carried_by("Bearer abc"), "a credential pair");
        }
    }
    // Choice 142: a credential header's `Bearer <token>` registers the bare token too (a
    // header map, a pair list and a path rule's `custom_headers.*` alike), so the token
    // echoed in another member is withheld.
    let bearer = r#"{"Authorization":"Bearer FAKE-GW-SECRET-48"}"#;
    let pairs = r#"[{"key":"Authorization","value":"Bearer FAKE-GW-SECRET-48"}]"#;
    for (policy, map) in [
        (&PLAIN, bearer),
        (&path, bearer),
        (&PLAIN, pairs),
        (&path, pairs),
    ] {
        let (text, secrets) = applied(
            &format!(
                r#"{{"configurations":{{"custom_headers":{map}}},"note":"FAKE-GW-SECRET-48","name":"bot"}}"#
            ),
            policy,
        );
        assert!(
            secrets.carried_by("FAKE-GW-SECRET-48"),
            "the bare token: {text}"
        );
        assert!(secrets.carried_by("Bearer FAKE-GW-SECRET-48"), "{text}");
        assert!(!text.contains("FAKE-GW-SECRET-48"), "{text}");
        assert!(
            text.ends_with(r#""note":"[withheld]","name":"bot"}"#),
            "the echo withheld: {text}"
        );
        assert!(!secrets.carried_by("Authorization"), "a header name");
    }
}

/// Choice 148 (review): a `Bearer <token>` under a plain credential name
/// (`{"authorization": …}`) or in a credential-named `{key, value}` pair outside a header
/// map registers the bare token as well as the whole value (as choice 142 does for a
/// header value), so an echo of the bare token is withheld; the scheme word never is.
#[test]
fn d238_a_bearer_token_under_a_credential_name_registers_the_bare_token() {
    for doc in [
        r#"{"authorization":"Bearer FAKE-GW-SECRET-0054","note":"FAKE-GW-SECRET-0054","name":"Bearer tokens"}"#,
        r#"{"tags":[{"key":"Authorization","value":"Bearer FAKE-GW-SECRET-0054"}],"note":"FAKE-GW-SECRET-0054","name":"Bearer tokens"}"#,
        r#"{"token":"Bearer FAKE-GW-SECRET-0054","note":"FAKE-GW-SECRET-0054","name":"Bearer tokens"}"#,
    ] {
        let (text, secrets) = applied(doc, &PLAIN);
        assert!(!text.contains("FAKE-GW-SECRET-0054"), "{text}");
        assert!(
            text.ends_with(r#""note":"[withheld]","name":"Bearer tokens"}"#),
            "{text}"
        );
        assert!(secrets.carried_by("FAKE-GW-SECRET-0054"), "{doc}");
        assert!(!secrets.carried_by("Bearer"), "{doc}");
    }
    let rows = r#"[{"request_body":"{\"forward\":{\"authorization\":\"Bearer FAKE-GW-SECRET-0055\"}}"},{"note":"FAKE-GW-SECRET-0055"}]"#;
    for reveal in [false, true] {
        let secrets = SecretSet::default();
        let mut doc = parse(rows).unwrap();
        apply_audit(&mut doc, reveal, &secrets);
        let text = to_compact(&doc).unwrap();
        assert!(
            !text.contains("FAKE-GW-SECRET-0055"),
            "reveal {reveal}: {text}"
        );
        assert!(secrets.carried_by("FAKE-GW-SECRET-0055"));
    }
}

/// Choice 159 (review): a `{"key": <name>, "value": …}` pair is read as one header in
/// every document, so a credential header's value (`x-functions-key`) is withheld and
/// registered in a resource document as in a header map and a revealed audit body; a
/// tag (`region`) is printed.
#[test]
fn d238_a_credential_header_pair_is_withheld_in_every_document() {
    let tags = r#"[{"key":"x-functions-key","value":"FNKEY-REVIEW-0001"},{"key":"region","value":"us-east"}]"#;
    let (text, secrets) = applied(
        &format!(r#"{{"tags":{tags},"note":"FNKEY-REVIEW-0001"}}"#),
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"tags":[{"key":"x-functions-key","value":"[withheld]"},{"key":"region","value":"us-east"}],"note":"[withheld]"}"#
    );
    assert!(secrets.carried_by("FNKEY-REVIEW-0001"));
    assert!(!secrets.carried_by("us-east"));
    let mut rows = OValue::Array(vec![OValue::Object(vec![(
        "request_body".into(),
        OValue::String(format!(r#"{{"tags":{tags}}}"#)),
    )])]);
    apply_audit(&mut rows, true, &SecretSet::default());
    assert_eq!(
        to_compact(&rows).unwrap(),
        r#"[{"request_body":"{\"tags\":[{\"key\":\"x-functions-key\",\"value\":\"[withheld]\"},{\"key\":\"region\",\"value\":\"[withheld]\"}]}"}]"#,
        "the audit reads a pair as a header: only a safe header's value is shown (choice 132)"
    );
}

/// The generic names (`key`, `token`) register only values that look like credentials,
/// and a `{key, value}` pair prints its name and withholds a credential's value: a tag
/// name never withholds the other strings that carry it.
#[test]
fn tag_pairs_and_generic_names() {
    let (text, secrets) = applied(
        r#"{"tags":[{"key":"region","value":"us-east"},{"key":"x-api-key","value":"FAKE-GW-SECRET-40"}],"name":"region-east deployment","token":"word"}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"tags":[{"key":"region","value":"us-east"},{"key":"x-api-key","value":"[withheld]"}],"name":"region-east deployment","token":"[withheld]"}"#
    );
    assert!(secrets.carried_by("FAKE-GW-SECRET-40"));
    assert!(!secrets.carried_by("a word here"), "a short generic value");
    let (_, secrets) = applied(r#"{"key":"sk-0123456789abcdef"}"#, &PLAIN);
    assert!(secrets.carried_by("echo sk-0123456789abcdef"));
}

/// Choice 170 (review): a `{"name": <name>, "value": …}` pair (a Kubernetes-style
/// environment list, a header list) is read as a header with that name, as a `{"key": …}`
/// pair is: a credential's value withheld and registered, so its echo is withheld; an
/// ordinary variable printed; inside a header map and a revealed audit body only a safe
/// header's value is shown.
#[test]
fn d238_name_value_pairs_are_read_as_headers() {
    let (text, secrets) = applied(
        r#"{"deployment_config":{"env":[{"name":"OPENAI_API_KEY","value":"sk-proj-ENVPAIR0001abcd"},{"name":"LOG_LEVEL","value":"info"}]},"forward_headers":[{"name":"Authorization","value":"Bearer ENVPAIR-0002"}],"note":"ENVPAIR-0002 sk-proj-ENVPAIR0001abcd"}"#,
        &DEPLOYMENTS,
    );
    assert_eq!(
        text,
        r#"{"deployment_config":{"env":[{"name":"OPENAI_API_KEY","value":"[withheld]"},{"name":"LOG_LEVEL","value":"info"}]},"forward_headers":[{"name":"Authorization","value":"[withheld]"}],"note":"[withheld]"}"#
    );
    assert!(secrets.carried_by("sk-proj-ENVPAIR0001abcd"));
    assert!(secrets.carried_by("ENVPAIR-0002"), "the bearer token alone");
    assert!(
        !secrets.carried_by("OPENAI_API_KEY"),
        "a name is never registered"
    );
    assert!(!secrets.carried_by("info"));
    let (text, _) = applied(
        r#"{"headers":[{"name":"accept","value":"*/*"},{"name":"x-env","value":"prod"}]}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"headers":[{"name":"accept","value":"*/*"},{"name":"x-env","value":"[withheld]"}]}"#
    );
    let mut rows = OValue::Array(vec![OValue::Object(vec![(
        "request_body".into(),
        OValue::String(
            r#"{"env":[{"name":"DB_PASSWORD","value":"ENVPAIR-0003x"},{"name":"region","value":"eu"}]}"#
                .into(),
        ),
    )])]);
    let secrets = SecretSet::default();
    apply_audit(&mut rows, true, &secrets);
    assert_eq!(
        to_compact(&rows).unwrap(),
        r#"[{"request_body":"{\"env\":[{\"name\":\"DB_PASSWORD\",\"value\":\"[withheld]\"},{\"name\":\"region\",\"value\":\"[withheld]\"}]}"}]"#,
        "the audit reading shows only a safe header's value (choice 132)"
    );
    assert!(secrets.carried_by("ENVPAIR-0003x"));
}
