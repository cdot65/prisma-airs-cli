//! Rule 5: the audit-log request fields, withheld and revealed.

use super::*;

/// Rule 5 (D239): the request fields withheld, or revealed as JSON with credentials
/// still withheld; non-JSON text stays withheld.
#[test]
fn audit_fields() {
    let rows = r#"[{"request_body":"{\"key\":\"FAKE-GW-SECRET-12\",\"name\":\"bot\"}","request_headers":"{\"authorization\":\"Bearer FAKE-GW-SECRET-13\",\"accept\":\"*/*\"}","query_params":"plain FAKE-GW-SECRET-14 text","api_key":"FAKE-GW-SECRET-30","uri":"/v2"}]"#;
    let secrets = SecretSet::default();
    let mut doc = parse(rows).unwrap();
    apply_audit(&mut doc, false, &secrets);
    assert_eq!(
        to_compact(&doc).unwrap(),
        r#"[{"request_body":"[withheld]","request_headers":"[withheld]","query_params":"[withheld]","api_key":"[withheld]","uri":"/v2"}]"#
    );
    assert!(secrets.carried_by("echo FAKE-GW-SECRET-12"), "choice 80");
    assert!(secrets.carried_by("Bearer FAKE-GW-SECRET-13"));
    let secrets = SecretSet::default();
    let mut doc = parse(rows).unwrap();
    apply_audit(&mut doc, true, &secrets);
    assert_eq!(
        to_compact(&doc).unwrap(),
        r#"[{"request_body":"{\"key\":\"[withheld]\",\"name\":\"bot\"}","request_headers":"{\"authorization\":\"[withheld]\",\"accept\":\"*/*\"}","query_params":"[withheld]","api_key":"[withheld]","uri":"/v2"}]"#
    );
    assert!(secrets.carried_by("FAKE-GW-SECRET-12"));
    assert!(secrets.carried_by("plain FAKE-GW-SECRET-14 text"));
}

/// D239: a revealed request field withholds the SDK's request secret paths of every
/// gateway resource (rule 2), credentials held as JSON text, the request credential
/// members and every header-map value but the safe headers; never a credential.
#[test]
fn d239_revealed_audit_fields_withhold_every_credential() {
    let body = r#"{"configurations":{"vertex_service_account_json":"{\"private_key\":\"PRIVKEY-REVIEW-111\"}","custom_headers":{"x-auth-review":"HDRVAL-REVIEW-222"},"aws_secret_key":"AWSSK-REVIEW-333","region":"us"},"model_config":{"secret_key":"MC-REVIEW-444x"},"data":{"credentials":{"user":"u"},"nested":"{\"subscription_key\":\"NESTED-REVIEW-555\",\"n\":1}"},"name":"bot"}"#;
    let headers = r#"{"ocp-apim-subscription-key":"OCP-REVIEW-777x","x-goog-api-key":"GOOG-REVIEW-888x","x-custom-auth":"CUSTOM-REVIEW-999","accept":"*/*","content-type":"application/json"}"#;
    let record = OValue::Array(vec![OValue::Object(vec![
        ("request_body".into(), OValue::String(body.into())),
        ("request_headers".into(), OValue::String(headers.into())),
        ("uri".into(), OValue::String("/v2/integrations".into())),
    ])]);
    let secrets = SecretSet::default();
    let mut doc = record.clone();
    apply_audit(&mut doc, true, &secrets);
    let text = to_compact(&doc).unwrap();
    for marker in [
        "PRIVKEY-REVIEW-111",
        "HDRVAL-REVIEW-222",
        "AWSSK-REVIEW-333",
        "MC-REVIEW-444x",
        "NESTED-REVIEW-555",
        "OCP-REVIEW-777x",
        "GOOG-REVIEW-888x",
        "CUSTOM-REVIEW-999",
    ] {
        assert!(!text.contains(marker), "{marker}: {text}");
        assert!(
            secrets.carried_by(marker),
            "{marker} registered (choice 80)"
        );
    }
    let fields: Vec<OValue> = ["request_body", "request_headers"]
        .iter()
        .map(|name| {
            parse(
                doc.as_array().unwrap()[0]
                    .get(name)
                    .unwrap()
                    .as_str()
                    .unwrap(),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        to_compact(&fields[0]).unwrap(),
        r#"{"configurations":{"vertex_service_account_json":"[withheld]","custom_headers":{"x-auth-review":"[withheld]"},"aws_secret_key":"[withheld]","region":"us"},"model_config":"[withheld]","data":{"credentials":"[withheld]","nested":"{\"subscription_key\":\"[withheld]\",\"n\":1}"},"name":"bot"}"#
    );
    assert_eq!(
        to_compact(&fields[1]).unwrap(),
        r#"{"ocp-apim-subscription-key":"[withheld]","x-goog-api-key":"[withheld]","x-custom-auth":"[withheld]","accept":"*/*","content-type":"application/json"}"#
    );
}

/// A revealed secret-reference body (the SDK's `secretReferences.*` rules): the whole
/// `auth_config` of every secret manager, at the root, under `data` and in every
/// `data` row; a `secret_id` anywhere else by the name rule.
#[test]
fn revealed_secret_reference_bodies_withhold_the_auth_config() {
    for body in [
        r#"{"name":"vault","manager_type":"hashicorp_vault","auth_config":{"vault_auth_type":"approle","vault_addr":"https://vault.example","vault_role_id":"ROLE-REVIEW-121","vault_secret_id":"APPROLE-REVIEW-122"},"secret_path":"kv/app"}"#,
        r#"{"data":{"name":"aws","auth_config":{"aws_auth_type":"accessKey","aws_access_key_id":"AKID-REVIEW-123","aws_secret_access_key":"AWSKEY-REVIEW-124","aws_region":"us-east-1"}}}"#,
        r#"{"data":[{"name":"aws","auth_config":{"aws_auth_type":"accessKey","aws_access_key_id":"AKID-REVIEW-123","aws_secret_access_key":"AWSKEY-REVIEW-124"}}]}"#,
        r#"{"nested":{"auth_config":{"vault_secret_id":"APPROLE-REVIEW-122"}},"other":{"vault_secret_id":"APPROLE-REVIEW-125"}}"#,
    ] {
        let secrets = SecretSet::default();
        let mut rows = OValue::Array(vec![OValue::Object(vec![(
            "request_body".into(),
            OValue::String(body.into()),
        )])]);
        apply_audit(&mut rows, true, &secrets);
        let text = to_compact(&rows).unwrap();
        assert!(text.contains("[withheld]"), "{text}");
        for marker in [
            "ROLE-REVIEW-121",
            "APPROLE-REVIEW-122",
            "AKID-REVIEW-123",
            "AWSKEY-REVIEW-124",
            "APPROLE-REVIEW-125",
        ] {
            assert!(!text.contains(marker), "{marker}: {text}");
        }
    }
    assert!(is_credential_name("vault_secret_id"));
    assert!(is_credential_name("secret_id"));
    assert!(!is_credential_name("secret_reference_id"));
}

/// Inside a revealed audit field a `{"key", "value"}` pair prints its name (never
/// registered) and shows the value only for a safe header.
#[test]
fn revealed_audit_pairs_print_names_and_withhold_values() {
    let body = r#"{"forward":[{"key":"Authorization","value":"Bearer PAIR-REVIEW-131"},{"key":"x-tenant","value":"PAIR-REVIEW-132"},{"key":"Content-Type","value":"application/json"}],"note":"Authorization"}"#;
    let secrets = SecretSet::default();
    let mut rows = OValue::Array(vec![OValue::Object(vec![(
        "request_body".into(),
        OValue::String(body.into()),
    )])]);
    apply_audit(&mut rows, true, &secrets);
    let revealed = parse(
        rows.as_array().unwrap()[0]
            .get("request_body")
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        to_compact(&revealed).unwrap(),
        r#"{"forward":[{"key":"Authorization","value":"[withheld]"},{"key":"x-tenant","value":"[withheld]"},{"key":"Content-Type","value":"application/json"}],"note":"Authorization"}"#
    );
    assert!(secrets.carried_by("Bearer PAIR-REVIEW-131"));
    assert!(
        !secrets.carried_by("PAIR-REVIEW-132"),
        "a short non-credential header value is withheld, not registered (choice 138)"
    );
    assert!(
        !secrets.carried_by("Authorization"),
        "a header name is never registered"
    );
}

/// Choice 141 (review): inside an audit field a generic name's short value
/// (`{"key": "prod"}`, by the name rule at any depth and by the SDK's `key` path at the
/// root and under `data`) is withheld in place but not registered, so another record's
/// `prod-…` uri and request id stay printed, revealed or not; a real credential (a long
/// `key`, a short value under a credential name, a bearer token) is still registered and
/// its echo in another record withheld.
#[test]
fn d239_a_generic_name_in_an_audit_body_does_not_withhold_the_log() {
    use serde_json::json;
    let body = json!({
        "key": "prod",
        "data": {"key": "prod"},
        "nested": {"token": "prod", "password": "hunter22"},
        "provider": {"key": "AUDIT-REVIEW-KEY-0001"},
    })
    .to_string();
    let headers = json!({"authorization": "Bearer AUDIT-REVIEW-TOKEN-0002"}).to_string();
    let rows = json!([
        {"request_body": body, "request_headers": headers, "uri": "/v2/configs"},
        {
            "uri": "/v2/workspaces/prod-gateway",
            "request_id": "prod-1",
            "note": "rotated AUDIT-REVIEW-KEY-0001",
            "agent": "AUDIT-REVIEW-TOKEN-0002 again",
            "user": "hunter22",
        },
    ])
    .to_string();
    for reveal in [false, true] {
        let secrets = SecretSet::default();
        let mut doc = parse(&rows).unwrap();
        apply_audit(&mut doc, reveal, &secrets);
        let text = to_compact(&doc).unwrap();
        let records = doc.as_array().unwrap();
        for (member, value) in [
            ("uri", "/v2/workspaces/prod-gateway"),
            ("request_id", "prod-1"),
            ("note", WITHHELD),
            ("agent", WITHHELD),
            ("user", WITHHELD),
        ] {
            assert_eq!(
                records[1].get(member).and_then(OValue::as_str),
                Some(value),
                "reveal {reveal}: {member}: {text}"
            );
        }
        assert!(!secrets.carried_by("prod"), "reveal {reveal}: {text}");
        for credential in [
            "AUDIT-REVIEW-KEY-0001",
            "AUDIT-REVIEW-TOKEN-0002",
            "hunter22",
        ] {
            assert!(!text.contains(credential), "reveal {reveal}: {text}");
            assert!(
                secrets.carried_by(credential),
                "reveal {reveal}: {credential}"
            );
        }
        if reveal {
            let revealed = records[0].get("request_body").unwrap().as_str().unwrap();
            assert_eq!(
                revealed,
                r#"{"data":{"key":"[withheld]"},"key":"[withheld]","nested":{"password":"[withheld]","token":"[withheld]"},"provider":{"key":"[withheld]"}}"#
            );
        }
    }
}

/// D239 (review): an audit record's transport headers (`x-forwarded-for`,
/// `x-portkey-workspace`, `x-forwarded-proto`) are withheld but not registered, so the
/// record's `client_ip`, `uri` and `workspace_id` stay printed in every record; a
/// credential header and a value that looks like a credential are registered.
#[test]
fn d239_audit_header_values_are_withheld_not_registered() {
    let rows = r#"[{"request_headers":"{\"x-forwarded-for\":\"203.0.113.9\",\"x-portkey-workspace\":\"ws-main-a\",\"x-forwarded-proto\":\"https\",\"x-custom-auth\":\"CUSTOM-REVIEW-0099\",\"authorization\":\"Bearer abc\"}","request_body":"{\"tags\":[{\"key\":\"env\",\"value\":\"https\"}]}","client_ip":"203.0.113.9","uri":"/v2/workspaces/ws-main-a","workspace_id":"ws-main-a","proto":"https"},{"request_headers":"{}","client_ip":"203.0.113.9","uri":"/v2/configs"}]"#;
    for reveal in [false, true] {
        let secrets = SecretSet::default();
        let mut doc = parse(rows).unwrap();
        apply_audit(&mut doc, reveal, &secrets);
        let records = doc.as_array().unwrap();
        for (record, member, value) in [
            (0, "client_ip", "203.0.113.9"),
            (0, "uri", "/v2/workspaces/ws-main-a"),
            (0, "workspace_id", "ws-main-a"),
            (0, "proto", "https"),
            (1, "client_ip", "203.0.113.9"),
        ] {
            assert_eq!(
                records[record].get(member).and_then(OValue::as_str),
                Some(value),
                "reveal {reveal}: {member}"
            );
        }
        assert!(secrets.carried_by("CUSTOM-REVIEW-0099"));
        assert!(secrets.carried_by("Bearer abc"));
        assert!(!secrets.carried_by("203.0.113.9"));
        if reveal {
            let headers = records[0].get("request_headers").unwrap().as_str().unwrap();
            assert_eq!(
                headers,
                r#"{"x-forwarded-for":"[withheld]","x-portkey-workspace":"[withheld]","x-forwarded-proto":"[withheld]","x-custom-auth":"[withheld]","authorization":"[withheld]"}"#
            );
        }
    }
}

/// Choice 144 (review): a revealed audit URL keeps its withheld query, and the second
/// pass of [`apply_audit`] (the shared rules over the printed rows) reads the
/// placeholder as already withheld, never as a query to register, so the revealed
/// fields and another record's `[withheld]` text stay printed. Applying the shared rules
/// again changes nothing.
#[test]
fn d239_a_revealed_url_never_registers_the_placeholder() {
    let rows = r#"[{"request_body":"{\"name\":\"visible-name\",\"callback\":\"https://hooks.example/cb?sig=abcd\"}","query_params":"{\"next\":\"https://h.example/x?cursor=zz\"}"},{"note":"see [withheld] above"}]"#;
    let secrets = SecretSet::default();
    let mut doc = parse(rows).unwrap();
    apply_audit(&mut doc, true, &secrets);
    let text = to_compact(&doc).unwrap();
    assert_eq!(
        text,
        r#"[{"request_body":"{\"name\":\"visible-name\",\"callback\":\"https://hooks.example/cb?[withheld]\"}","query_params":"{\"next\":\"https://h.example/x?[withheld]\"}"},{"note":"see [withheld] above"}]"#
    );
    assert!(!secrets.carried_by(WITHHELD), "the placeholder registered");
    assert!(secrets.carried_by("sig=abcd"), "the query is registered");
    let mut again = doc.clone();
    apply(&mut again, &PLAIN, &secrets);
    assert_eq!(
        to_compact(&again).unwrap(),
        text,
        "the shared rules are idempotent"
    );
    assert!(!secrets.carried_by(WITHHELD));
    // A URL whose userinfo or fragment an earlier pass withheld is unchanged too; a real
    // part beside a withheld one is still withheld and registered.
    assert_eq!(
        url_withheld("https://[withheld]@h.example/p?[withheld]#[withheld]"),
        None
    );
    assert_eq!(
        url_withheld("https://[withheld]@h.example/p?q=FAKE-GW-SECRET-50"),
        Some((
            "https://[withheld]@h.example/p?[withheld]".to_owned(),
            vec!["q=FAKE-GW-SECRET-50".to_owned()]
        ))
    );
    let secrets = SecretSet::default();
    register(
        vec![WITHHELD.to_owned(), format!("a {WITHHELD} b")],
        &secrets,
    );
    assert!(
        !secrets.carried_by(WITHHELD),
        "register never adds the placeholder"
    );
}

/// Choice 162 (review): an audit request field that arrives as a JSON object or array is
/// revealed as the withheld object or array, as a JSON text is revealed as the withheld
/// text; unrevealed it is `[withheld]`.
#[test]
fn d239_a_revealed_object_field_is_printed_withheld() {
    let record = || {
        OValue::Array(vec![OValue::Object(vec![
            (
                "request_body".into(),
                parse(r#"{"clientSecret":"OBJ-REVIEW-0001x","name":"bot"}"#).unwrap(),
            ),
            (
                "request_headers".into(),
                parse(r#"{"authorization":"Bearer OBJ-REVIEW-0002","accept":"*/*"}"#).unwrap(),
            ),
        ])])
    };
    let secrets = SecretSet::default();
    let mut rows = record();
    apply_audit(&mut rows, true, &secrets);
    assert_eq!(
        to_compact(&rows).unwrap(),
        r#"[{"request_body":{"clientSecret":"[withheld]","name":"bot"},"request_headers":{"authorization":"[withheld]","accept":"*/*"}}]"#
    );
    assert!(secrets.carried_by("OBJ-REVIEW-0001x"));
    assert!(secrets.carried_by("OBJ-REVIEW-0002"));
    let secrets = SecretSet::default();
    let mut rows = record();
    apply_audit(&mut rows, false, &secrets);
    assert_eq!(
        to_compact(&rows).unwrap(),
        r#"[{"request_body":"[withheld]","request_headers":"[withheld]"}]"#
    );
    assert!(secrets.carried_by("OBJ-REVIEW-0001x"), "choice 80");
}

/// Choice 187 (review: `audit-logs list` took 0.09, 0.23, 0.88 and 3.45 s for 500, 1000, 2000
/// and 4000 records): the read is linear in the records. An unrevealed field is never
/// printed, so its text joins the `SecretSet` neither whole nor as its encodings, only the
/// credentials inside it; and the final pass checks every printed string and member name in
/// one matcher pass over its bytes, whatever the number of registered forms. Both are bounds
/// on structure, not on time: at most the four forms of each record's one credential, and at
/// most two matcher steps per byte of the printed document.
#[test]
fn d239_a_large_audit_log_is_withheld_in_linear_work() {
    const RECORDS: usize = 4000;
    let records: Vec<OValue> = (0..RECORDS)
        .map(|at| {
            let body = format!(
                r#"{{"name":"bot-{at}","note":"record number {at} of the log","key":"sk-proj-{at:016}"}}"#
            );
            let headers = format!(
                r#"{{"x-trace":"trace-{at:08}","content-type":"application/json"}}"#
            );
            OValue::Object(vec![
                ("request_id".into(), OValue::String(format!("req-{at}"))),
                ("request_body".into(), OValue::String(body)),
                ("request_headers".into(), OValue::String(headers)),
                (
                    "query_params".into(),
                    OValue::String(format!("page={at}&sort=name&q=record {at} of the log")),
                ),
                ("uri".into(), OValue::String("/v2/api-keys/service".into())),
            ])
        })
        .collect();
    let received = OValue::Array(records);
    let secrets = SecretSet::default();
    let mut rows = received.clone();
    apply_audit(&mut rows, false, &secrets);
    let printed = to_compact(&rows).unwrap();
    for (at, record) in rows.as_array().unwrap().iter().enumerate() {
        for field in AUDIT_FIELDS {
            assert_eq!(record.get(field).and_then(OValue::as_str), Some(WITHHELD));
        }
        assert_eq!(
            record.get("request_id").and_then(OValue::as_str),
            Some(format!("req-{at}").as_str())
        );
    }
    let forms = secrets.snapshot();
    let first = &received.as_array().unwrap()[0];
    for field in AUDIT_FIELDS {
        let text = first.get(field).and_then(OValue::as_str).unwrap();
        assert!(
            !forms.iter().any(|form| form == text),
            "{field} joined whole"
        );
    }
    assert!(secrets.carried_by("sk-proj-0000000000000007"), "choice 80");
    assert!(
        secrets.form_count() <= 4 * RECORDS,
        "{} forms for {RECORDS} records",
        secrets.form_count()
    );
    let before = secrets.steps();
    crate::secrets::withhold(&mut rows, &secrets);
    let steps = secrets.steps() - before;
    assert!(
        steps <= 2 * printed.len() as u64,
        "{steps} matcher steps for {} printed bytes",
        printed.len()
    );
    assert!(
        secrets.steps() <= 4 * printed.len() as u64,
        "the read's own pass"
    );
}
