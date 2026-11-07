//! Rules 1 and 2: the SDK path rules and the resource policies.

use super::*;

#[test]
fn paths_reach_rows_and_wildcards() {
    const POLICY: Policy = Policy {
        operations: &[],
        paths: &[
            &["model_config"],
            &["configurations", "custom_headers", "*"],
        ],
    };
    let (text, _) = applied(
        r#"[{"model_config":{"a":1},"configurations":{"custom_headers":{"x-h":"FAKE-GW-SECRET-3"},"region":"us"}}]"#,
        &POLICY,
    );
    assert_eq!(
        text,
        r#"[{"model_config":"[withheld]","configurations":{"custom_headers":{"x-h":"[withheld]"},"region":"us"}}]"#
    );
}

/// The per-resource paths (§19.3 rule 2) on a detail and on every list row.
#[test]
fn resource_paths() {
    let (text, secrets) = applied(
        r#"{"key":"FAKE-GW-SECRET-24","masked_api_key":"sk-...abcd","model_config":{"region":"us"},"configurations":{"aws_secret_access_key":"FAKE-GW-SECRET-17","region":"us","custom_headers":{"x-a":"FAKE-GW-SECRET-18"}}}"#,
        &PROVIDERS,
    );
    assert_eq!(
        text,
        r#"{"key":"[withheld]","masked_api_key":"sk-...abcd","model_config":"[withheld]","configurations":{"aws_secret_access_key":"[withheld]","region":"us","custom_headers":{"x-a":"[withheld]"}}}"#
    );
    assert!(secrets.carried_by("FAKE-GW-SECRET-18"));
    let (text, _) = applied(
        r#"{"success":true,"data":{"auth_type":"scim","scim_token":"FAKE-GW-SECRET-21","auth_settings":{"client_id":"c","client_secret":"FAKE-GW-SECRET-22"}}}"#,
        &AUTH_SETTINGS,
    );
    assert_eq!(
        text,
        r#"{"success":true,"data":{"auth_type":"scim","scim_token":"[withheld]","auth_settings":{"client_id":"c","client_secret":"[withheld]"}}}"#
    );
    let (text, _) = applied(
        r#"[{"credentials":{"region":"us","api_key":"FAKE-GW-SECRET-23"},"id":"p"}]"#,
        &PLUGINS,
    );
    assert_eq!(
        text, r#"[{"credentials":"[withheld]","id":"p"}]"#,
        "the path rule, then the name rule on the whole object"
    );
    let (text, _) = applied(
        r#"{"id":"o","settings":{"debug_log":0,"url":"https://x/FAKE-GW-SECRET-27"}}"#,
        &ORGANISATION_INFO,
    );
    assert_eq!(text, r#"{"id":"o","settings":"[withheld]"}"#);
    let (text, _) = applied(
        r#"{"client_auth":"FAKE-GW-SECRET-16","credentials":{"password":"FAKE-GW-SECRET-15","username":"u"}}"#,
        &DEPLOYMENTS,
    );
    assert_eq!(
        text, r#"{"client_auth":"[withheld]","credentials":"[withheld]"}"#,
        "the name rule withholds the whole credentials object too"
    );
    assert!(!secrets_of(&DEPLOYMENTS).carried_by("a [withheld] b"));
}

/// A JSON document holding `marker` at `path` (`*` read as one member, and as the items
/// of an array, as a list answer's rows), beside a root member `shown` that no rule
/// withholds.
fn document_at(path: &[&str], marker: &str) -> OValue {
    fn inner(path: &[&str], marker: &str) -> OValue {
        match path.split_first() {
            None => OValue::String(marker.to_owned()),
            Some((first, rest)) if *first == "*" => OValue::Array(vec![inner(rest, marker)]),
            Some((first, rest)) => OValue::Object(vec![((*first).to_owned(), inner(rest, marker))]),
        }
    }
    let mut doc = inner(path, marker);
    if let OValue::Object(entries) = &mut doc {
        entries.push(("shown".to_owned(), OValue::String("visible".to_owned())));
    }
    doc
}

/// Rules 1, 2 and 5 come from the SDK's public table, never a copy: every request and
/// response path of every operation of `AI_GATEWAY_SECRET_FIELDS` is withheld by the
/// policy of its resource (or the operation is one no command reads, [`AUDIT_ONLY`]),
/// and inside a revealed audit field (or the operation is [`NOT_AUDITED`]). An SDK bump
/// that adds an operation fails here until the operation is placed.
#[test]
fn every_sdk_secret_path_is_withheld() {
    assert!(!AI_GATEWAY_SECRET_FIELDS.is_empty());
    for (operation, rules) in AI_GATEWAY_SECRET_FIELDS {
        let policies: Vec<&Policy> = POLICIES
            .iter()
            .copied()
            .filter(|policy| policy.operations.contains(operation))
            .collect();
        assert!(
            !policies.is_empty() || AUDIT_ONLY.contains(operation),
            "{} has no policy and is not audit-only",
            operation.as_str()
        );
        assert!(
            policies.is_empty() || !AUDIT_ONLY.contains(operation),
            "{} is both",
            operation.as_str()
        );
        for rule in rules.request.iter().chain(rules.response) {
            let marker = format!("SDK-PATH-{}-{}", operation.as_str(), rule.path.join("."));
            let doc = document_at(rule.path, &marker);
            for policy in &policies {
                let secrets = SecretSet::default();
                let mut shown = doc.clone();
                apply(&mut shown, policy, &secrets);
                let text = to_compact(&shown).unwrap();
                assert!(!text.contains(&marker), "{marker}: {text}");
                assert!(text.contains("visible"), "{marker}: only the path: {text}");
                assert!(secrets.carried_by(&marker), "{marker} registered");
            }
            if NOT_AUDITED.contains(operation) {
                continue;
            }
            let body = to_compact(&doc).unwrap();
            let enveloped =
                to_compact(&OValue::Object(vec![("data".into(), doc.clone())])).unwrap();
            for field in [body, enveloped] {
                let secrets = SecretSet::default();
                let mut rows = OValue::Array(vec![OValue::Object(vec![(
                    "request_body".into(),
                    OValue::String(field),
                )])]);
                apply_audit(&mut rows, true, &secrets);
                let text = to_compact(&rows).unwrap();
                assert!(!text.contains(&marker), "audit {marker}: {text}");
                assert!(text.contains("visible"), "audit {marker}: revealed: {text}");
                assert!(secrets.carried_by(&marker), "audit {marker} registered");
            }
        }
    }
}

/// Choice 153 (review): a final `*` path rule (a plugin's `credentials.*`) withholds every
/// member and registers each by its own name: the `api_key` whole, the `region` only were
/// it a credential or a password, so the `integration_slug` carrying it stays printed
/// (the name rule then withholds `credentials` whole, registering nothing more).
#[test]
fn d238_a_wildcard_path_registers_each_member_by_its_name() {
    let (text, secrets) = applied(
        r#"[{"credentials":{"api_key":"FAKE-GW-SECRET-0069","region":"us-east-1"},"integration_slug":"openai-us-east-1","note":"FAKE-GW-SECRET-0069"}]"#,
        &PLUGINS,
    );
    assert_eq!(
        text,
        r#"[{"credentials":"[withheld]","integration_slug":"openai-us-east-1","note":"[withheld]"}]"#
    );
    assert!(secrets.carried_by("FAKE-GW-SECRET-0069"));
    assert!(!secrets.carried_by("us-east-1"));
}
