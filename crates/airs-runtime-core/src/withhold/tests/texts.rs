//! The rules on a string: JSON texts (3f), private keys, the URL rule, the scheme rule, the
//! key=value rule and model output.

use super::*;

/// Choice 81: a config's JSON text is withheld inside (rule 3's JSON-text clause, no
/// per-resource policy); an untouched one stays as sent.
#[test]
fn json_texts() {
    let (text, secrets) = applied(
        r#"[{"config":"{\"targets\": [{\"api_key\": \"FAKE-GW-SECRET-29\", \"provider\": \"openai\"}]}","name":"c"},{"config":"{ \"retry\": 3 }"},{"config":"not json"}]"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"[{"config":"{\"targets\":[{\"api_key\":\"[withheld]\",\"provider\":\"openai\"}]}","name":"c"},{"config":"{ \"retry\": 3 }"},{"config":"not json"}]"#
    );
    assert!(secrets.carried_by("FAKE-GW-SECRET-29"));
}

/// A short value withheld inside a JSON text (a generic name's value is withheld but
/// not registered) still changes the printed text: the text is printed withheld.
#[test]
fn a_short_withheld_value_inside_a_json_text_is_not_printed() {
    let (text, secrets) = applied(r#"{"config":"{\"token\":\"abcde\"}"}"#, &PLAIN);
    assert_eq!(text, r#"{"config":"{\"token\":\"[withheld]\"}"}"#);
    assert!(!secrets.carried_by("abcde"));
    let secrets = SecretSet::default();
    let mut rows = parse(
        r#"[{"request_body":"{\"note\":\"{\\\"headers\\\":{\\\"x-h\\\":\\\"short\\\"}}\"}"}]"#,
    )
    .unwrap();
    apply_audit(&mut rows, true, &secrets);
    let text = to_compact(&rows).unwrap();
    assert!(!text.contains("short"), "{text}");
}

/// Model output keeps its words: no name rule, the URL and `SecretSet` rules only, and
/// nothing registered.
#[test]
fn model_output_keeps_logprob_tokens() {
    let secrets = SecretSet::default();
    secrets.add("FAKE-GW-INFERENCE-KEY");
    let mut doc = parse(
        r#"{"choices":[{"message":{"content":"Hello world"},"logprobs":{"content":[{"token":"Hello"},{"token":" world"}]}}],"link":"https://x.example/a?q=1","echo":"FAKE-GW-INFERENCE-KEY"}"#,
    )
    .unwrap();
    apply_model_output(&mut doc, &secrets);
    assert_eq!(
        to_compact(&doc).unwrap(),
        r#"{"choices":[{"message":{"content":"Hello world"},"logprobs":{"content":[{"token":"Hello"},{"token":" world"}]}}],"link":"https://x.example/a?[withheld]","echo":"[withheld]"}"#
    );
    assert!(!secrets.carried_by("Hello"));
    assert!(!secrets.carried_by("q=1"));
}

/// D251: userinfo, query and fragment withheld; host and path kept. The credential of a
/// `key=value` segment inside the query is a part of its own (choice 183).
#[test]
fn urls_lose_userinfo_queries_and_fragments() {
    assert_eq!(
        url_withheld("https://user:pw@host.example/v1/chat?key=FAKE-GW-SECRET-4#frag"),
        Some((
            "https://[withheld]@host.example/v1/chat?[withheld]#[withheld]".to_owned(),
            vec![
                "user:pw".to_owned(),
                "key=FAKE-GW-SECRET-4".to_owned(),
                "FAKE-GW-SECRET-4".to_owned(),
                "frag".to_owned()
            ]
        ))
    );
    assert_eq!(url_withheld("https://host.example/v1/chat"), None);
    assert_eq!(url_withheld("not a url ?x"), None);
    assert_eq!(url_withheld("1://h?q"), None, "no letter, no scheme");
    assert_eq!(
        url_withheld("9ttp://h?q").map(|(shown, _)| shown),
        Some("9ttp://h?[withheld]".to_owned()),
        "the scheme starts at its first letter (choice 169)"
    );
    assert_eq!(
        url_withheld("http://h?").map(|(shown, _)| shown),
        None,
        "an empty query withholds nothing"
    );
    let (text, secrets) = applied(
        r#"{"request_url":"https://api.example/v1?key=FAKE-GW-SECRET-5","other":"key=FAKE-GW-SECRET-5"}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"request_url":"https://api.example/v1?[withheld]","other":"key=[withheld]"}"#
    );
    assert!(secrets.carried_by("key=FAKE-GW-SECRET-5"));
    assert!(secrets.carried_by("FAKE-GW-SECRET-5"), "choice 181");
}

/// The JSON texts `JSON.parse` reads and serde_json cannot (a lone surrogate escape, a
/// number beyond `f64`, nesting deeper than 128 levels) and a truncated one, each holding
/// `key` (D238 rule 3f, choice 167).
fn unreadable_json_texts(key: &str) -> Vec<String> {
    vec![
        format!(r#"{{"targets":[{{"api_key":"{key}","note":"\ud83d"}}]}}"#),
        format!(r#"{{"api_key":"{key}","n":"\udc00"}}"#),
        format!(r#"{{"targets":[{{"api_key":"{key}","weight":1e400}}]}}"#),
        format!("{}\"{key}\"{}", "[".repeat(129), "]".repeat(129)),
        format!(r#" {{"api_key":"{key}","#),
    ]
}

/// D238 rule 3f fails closed (choice 167): a text shaped as a JSON document that
/// serde_json cannot read is withheld whole and its strings registered, in a resource
/// document, a header map and a revealed audit body alike; the live tier's check refuses
/// it printed as received. A text that only starts with a bracket stays printed.
#[test]
fn d238_a_json_text_serde_json_cannot_read_is_withheld_whole() {
    let key = "sk-proj-abcdefghij0123456789";
    for inner in unreadable_json_texts(key) {
        assert!(parse(&inner).is_err(), "readable: {inner}");
        let received = serde_json::json!({"config": inner, "name": "n"}).to_string();
        let (text, secrets) = applied(&received, &PLAIN);
        assert_eq!(text, r#"{"config":"[withheld]","name":"n"}"#, "{inner}");
        assert!(secrets.carried_by(key), "not registered: {inner}");
        assert!(!checked(&received), "the live check accepts: {inner}");
        assert!(checked(&text));

        let received = serde_json::json!({"headers": inner}).to_string();
        let (text, secrets) = applied(&received, &PLAIN);
        assert_eq!(text, r#"{"headers":"[withheld]"}"#);
        assert!(secrets.carried_by(key), "header map: {inner}");

        let received = serde_json::json!({"api_key": inner}).to_string();
        let (_, secrets) = applied(&received, &PLAIN);
        assert!(secrets.carried_by(key), "credential name: {inner}");

        for reveal in [false, true] {
            let body = serde_json::json!({"config": inner, "name": "bot"}).to_string();
            let rows = serde_json::json!([{"request_body": body}]).to_string();
            let secrets = SecretSet::default();
            let mut doc = parse(&rows).unwrap();
            apply_audit(&mut doc, reveal, &secrets);
            let text = to_compact(&doc).unwrap();
            assert!(!text.contains(key), "revealed {reveal}: {text}");
            assert!(secrets.carried_by(key), "audit {reveal}: {inner}");
            let rows = serde_json::json!([{"request_body": inner}]).to_string();
            let secrets = SecretSet::default();
            let mut doc = parse(&rows).unwrap();
            apply_audit(&mut doc, reveal, &secrets);
            assert_eq!(
                to_compact(&doc).unwrap(),
                r#"[{"request_body":"[withheld]"}]"#
            );
            assert!(secrets.carried_by(key), "audit field {reveal}: {inner}");
        }
    }
    // A URL inside such a text registers only its withheld parts, never the host.
    let inner = r#"{"base_url":"https://api.example.test/v1?key=abcd1234","n":"\ud83d"}"#;
    let (text, secrets) = applied(&serde_json::json!({"config": inner}).to_string(), &PLAIN);
    assert_eq!(text, r#"{"config":"[withheld]"}"#);
    assert!(secrets.carried_by("key=abcd1234"));
    assert!(!secrets.carried_by("https://api.example.test/v1"));
    for printed in [
        "[withheld]",
        "[INFO] started",
        "{{template}}",
        "[1/3] step",
        "{ x }",
    ] {
        let received = serde_json::json!({"note": printed}).to_string();
        assert_eq!(applied(&received, &PLAIN).0, received, "{printed}");
    }
}

/// D251: a userinfo is read up to the last `@` of the URL, so a password holding an
/// unencoded `/`, `?` or `#` (an AWS secret access key) is withheld and registered whole
/// (choice 168); an `@` in a path or query withholds what precedes it, registering it only
/// when it holds a `:` (a `user:password`).
#[test]
fn d251_a_password_holding_a_delimiter_is_withheld() {
    for (url, shown, userinfo) in [
        (
            "https://AKIAIOSFODNN7EXAMPLE:wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY@s3.example.test/bucket",
            "https://[withheld]@s3.example.test/bucket",
            "AKIAIOSFODNN7EXAMPLE:wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        ),
        (
            "https://user:pa/ss@host.example/p",
            "https://[withheld]@host.example/p",
            "user:pa/ss",
        ),
        (
            "https://user:pa?ss@host.example/p?q=1",
            "https://[withheld]@host.example/p?[withheld]",
            "user:pa?ss",
        ),
        (
            "https://user:pa#ss@host.example/p#f",
            "https://[withheld]@host.example/p#[withheld]",
            "user:pa#ss",
        ),
    ] {
        let parts = url_withheld(url).map(|(text, parts)| (text, parts[0].clone()));
        assert_eq!(
            parts,
            Some((shown.to_owned(), userinfo.to_owned())),
            "{url}"
        );
        let received = serde_json::json!({"custom_host": url}).to_string();
        let (text, secrets) = applied(&received, &PLAIN);
        assert_eq!(text, serde_json::json!({"custom_host": shown}).to_string());
        assert!(secrets.carried_by(userinfo), "{url}");
        assert!(!checked(&received), "the live check accepts {url}");
        assert!(checked(&text));
    }
    assert_eq!(
        url_withheld("https://social.example/@someone/post"),
        Some(("https://[withheld]@someone/post".to_owned(), Vec::new())),
        "a path @ is withheld in place, not registered"
    );
}

/// D251: every absolute URL inside a text is read, not only one that starts it (an
/// upstream error, prose, a value with leading whitespace; choice 169).
#[test]
fn d251_urls_inside_a_text_are_withheld() {
    for (received, printed, registered) in [
        (
            r#"{"outbound":{"status":"error","error":"connect ECONNREFUSED https://admin:Pa55word@llm.internal/v1"}}"#,
            r#"{"outbound":{"status":"error","error":"connect ECONNREFUSED https://[withheld]@llm.internal/v1"}}"#,
            &["admin:Pa55word"][..],
        ),
        (
            r#"{"description":"connect via https://admin:hunter2x@db.local/x?token=abc"}"#,
            r#"{"description":"connect via https://[withheld]@db.local/x?[withheld]"}"#,
            &["admin:hunter2x", "token=abc"][..],
        ),
        (
            r#"{"base_url":" https://admin:hunter2x@db.local/x"}"#,
            r#"{"base_url":" https://[withheld]@db.local/x"}"#,
            &["admin:hunter2x"][..],
        ),
        (
            r#"{"errors":["a https://u:p4ssw0rd@h.example/x then (http://h2.example/y?sig=abcd) end"]}"#,
            r#"{"errors":["a https://[withheld]@h.example/x then (http://h2.example/y?[withheld] end"]}"#,
            &["u:p4ssw0rd", "sig=abcd)"][..],
        ),
    ] {
        let (text, secrets) = applied(received, &PLAIN);
        assert_eq!(text, printed);
        for part in registered {
            assert!(secrets.carried_by(part), "{part} not registered");
        }
        assert!(!checked(received), "the live check accepts {received}");
        assert!(checked(printed));
        assert_eq!(applied(printed, &PLAIN).0, printed, "idempotent");
    }
    let secrets = SecretSet::default();
    let mut answer = parse(r#"{"content":"see https://u:p4ssw0rd@h.example/x"}"#).unwrap();
    apply_model_output(&mut answer, &secrets);
    assert_eq!(
        to_compact(&answer).unwrap(),
        r#"{"content":"see https://[withheld]@h.example/x"}"#
    );
}

/// Choice 177 (review): a credential after `Basic`, `Bearer` or `Token` is withheld in
/// place under any name (a header value, a command line, a Basic credential) and
/// registered, so its echo is withheld; a `_headers` map, a `{header, value}` pair and a
/// list of header names are read as headers; prose keeps its words.
#[test]
fn d238_scheme_credentials_and_header_shapes_are_withheld() {
    let (text, secrets) = applied(
        r#"{"auth_header":"Bearer sk-proj-SCHEME0001abcdef","authentication":"Basic dXNlcjpwYXNz","note":"curl -H 'Authorization: Bearer SCHEME0002abcdefgh' https://h/v1","echo":"sk-proj-SCHEME0001abcdef","about":"a basic introduction to Token auth","extra_headers":{"X-Signature":"SCHEME-0003x","accept":"*/*"},"forward_headers":[{"header":"Authorization","value":"SCHEME-0004x"}],"allowed_headers":["x-trace","Authorization"]}"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"{"auth_header":"[withheld]","authentication":"Basic [withheld]","note":"curl -H 'Authorization: Bearer [withheld]' https://h/v1","echo":"[withheld]","about":"a basic introduction to Token auth","extra_headers":{"X-Signature":"[withheld]","accept":"*/*"},"forward_headers":[{"header":"Authorization","value":"[withheld]"}],"allowed_headers":["x-trace","Authorization"]}"#
    );
    for secret in [
        "sk-proj-SCHEME0001abcdef",
        "dXNlcjpwYXNz",
        "SCHEME0002abcdefgh",
        "SCHEME-0003x",
        "SCHEME-0004x",
    ] {
        assert!(secrets.carried_by(secret), "{secret}");
    }
    assert!(checked(&text));
    // Model output: the scheme rule in place, nothing registered.
    let secrets = SecretSet::default();
    let mut answer =
        parse(r#"{"content":"use Authorization: Bearer sk-proj-SCHEME0005abcdef"}"#).unwrap();
    apply_model_output(&mut answer, &secrets);
    assert_eq!(
        to_compact(&answer).unwrap(),
        r#"{"content":"use Authorization: Bearer [withheld]"}"#
    );
    assert!(!secrets.carried_by("sk-proj-SCHEME0005abcdef"));
}

/// Choice 170 (review): a text holding a PEM private key is withheld and registered whole
/// under any name, in a JSON text, a safe header and a schema description too.
#[test]
fn d238_a_private_key_is_withheld_under_any_name() {
    let pem =
        "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA0PEMBODY\n-----END RSA PRIVATE KEY-----";
    let doc = serde_json::json!({
        "cert": pem,
        "config": serde_json::json!({"bundle": pem, "name": "b"}).to_string(),
        "headers": {"accept": pem},
        "properties": {"region": {"type": "string", "description": pem}},
        "note": "MIIEpAIBAAKCAQEA0PEMBODY",
        "certificate": "-----BEGIN CERTIFICATE-----\nMIIBpublic\n-----END CERTIFICATE-----",
    })
    .to_string();
    let (text, secrets) = applied(&doc, &PLAIN);
    assert_eq!(
        text,
        r#"{"cert":"[withheld]","certificate":"-----BEGIN CERTIFICATE-----\nMIIBpublic\n-----END CERTIFICATE-----","config":"{\"bundle\":\"[withheld]\",\"name\":\"b\"}","headers":{"accept":"[withheld]"},"note":"[withheld]","properties":{"region":{"description":"[withheld]","type":"string"}}}"#
    );
    assert!(secrets.carried_by(pem));
    assert!(
        secrets.carried_by("MIIEpAIBAAKCAQEA0PEMBODY"),
        "its body line"
    );
    assert!(checked(&text) && !checked(&doc));
}

/// Choice 181 (review: a long word after the prose words `token` and `basic` was
/// registered, so one row's error text withheld the `ai_model` of every row and dropped a
/// member named after it): the scheme rule withholds that word in its own text only; the
/// model id stays printed under another name and as a member name, in json, pretty and
/// table, and nothing is registered.
#[test]
fn d238_a_model_id_after_a_prose_scheme_word_stays_printed_elsewhere() {
    for (doc, printed, model) in [
        (
            r#"[{"error":"token gpt-4o-mini-2024-07-18 quota exceeded","ai_model":"o1"},{"error":"","ai_model":"gpt-4o-mini-2024-07-18"},{"usage":{"gpt-4o-mini-2024-07-18":3}}]"#,
            r#"[{"error":"token [withheld] quota exceeded","ai_model":"o1"},{"error":"","ai_model":"gpt-4o-mini-2024-07-18"},{"usage":{"gpt-4o-mini-2024-07-18":3}}]"#,
            "gpt-4o-mini-2024-07-18",
        ),
        (
            r#"[{"description":"basic claude-3-5-sonnet-20241022 routing","model":"m"},{"description":"","model":"claude-3-5-sonnet-20241022"}]"#,
            r#"[{"description":"basic [withheld] routing","model":"m"},{"description":"","model":"claude-3-5-sonnet-20241022"}]"#,
            "claude-3-5-sonnet-20241022",
        ),
    ] {
        let (text, secrets) = applied(doc, &PLAIN);
        assert_eq!(text, printed);
        assert!(!secrets.carried_by(model), "{model}");
        assert!(checked(&text));
        let rows = parse(&text).unwrap();
        for (format, shown) in printed_formats(&rows) {
            assert!(shown.contains(model), "{format:?}: {shown}");
        }
    }
    // In header form the same word is a credential: registered, so its echo is withheld.
    let (text, secrets) = applied(
        r#"[{"error":"Authorization: token gpt-4o-mini-2024-07-18 rejected"},{"ai_model":"gpt-4o-mini-2024-07-18"}]"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"[{"error":"Authorization: token [withheld] rejected"},{"ai_model":"[withheld]"}]"#
    );
    assert!(secrets.carried_by("gpt-4o-mini-2024-07-18"));
}

/// Choice 183 (review: under a credential-named key the key=value rule registered any value
/// of 4 or more characters, so `auth=none` or `password=true` in one row withheld `mode: none`,
/// `enabled: "true"` and the `guardrail` naming `nonexistent-check` in others; and the whole
/// two-word label `token <model>` registered the model id, withholding every `ai_model`): the
/// value is withheld in its text and registered only when it looks like a credential or is
/// shaped as a password, never a placeholder word or a mask; the lowercase two-word label is
/// withheld in its text only. Every other row prints, in json, pretty and table.
#[test]
fn d238_a_credential_key_word_stays_printed_elsewhere() {
    let (text, secrets) = applied(
        r#"[{"description":"Set auth=none for local"},{"mode":"none","guardrail":"nonexistent-check"},{"note":"password=true"},{"enabled":"true"},{"note":"Pwd=****;"},{"mask":"****"},{"label":"token gpt-4o-mini-2024-07-18"},{"ai_model":"gpt-4o-mini-2024-07-18"},{"password":"none"},{"state":"none"}]"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"[{"description":"Set auth=[withheld] for local"},{"mode":"none","guardrail":"nonexistent-check"},{"note":"password=[withheld]"},{"enabled":"true"},{"note":"Pwd=[withheld];"},{"mask":"****"},{"label":"token [withheld]"},{"ai_model":"gpt-4o-mini-2024-07-18"},{"password":"[withheld]"},{"state":"none"}]"#
    );
    for word in [
        "none",
        "true",
        "****",
        "gpt-4o-mini-2024-07-18",
        "nonexistent-check",
    ] {
        assert!(!secrets.carried_by(word), "{word}");
    }
    assert!(checked(&text));
    let rows = parse(&text).unwrap();
    for (format, shown) in printed_formats(&rows) {
        for word in ["nonexistent-check", "gpt-4o-mini-2024-07-18"] {
            assert!(shown.contains(word), "{format:?}: {shown}");
        }
    }
    // A real credential under the same keys is still registered, so its echo is withheld.
    let (text, secrets) = applied(
        r#"[{"note":"auth=Hunter2x!;Pwd=abc+/=="},{"echo":"Hunter2x!"},{"echo":"abc+/=="},{"label":"Token gpt-4o-mini-2024-07-18"},{"ai_model":"gpt-4o-mini-2024-07-18"}]"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"[{"note":"auth=[withheld];Pwd=[withheld]"},{"echo":"[withheld]"},{"echo":"[withheld]"},{"label":"Token [withheld]"},{"ai_model":"[withheld]"}]"#
    );
    assert!(secrets.carried_by("Hunter2x!") && secrets.carried_by("abc+/=="));
}

/// Choice 186 (review: `auth=oauth2` registered `oauth2`, so one row's note withheld the
/// `oauth2` of every other row; `Password=p@ss w0rd;` printed `w0rd` and `PWD={p@ss;w0rd};`
/// printed `w0rd}`; `password = hunter2x!` printed whole): a short letters-and-digits word
/// under a credential key is withheld in its text only, a connection string's password is
/// withheld and registered whole, and a credential name with spaces around its `=` is read.
#[test]
fn d238_short_key_words_stay_local_and_connection_passwords_are_whole() {
    let (text, secrets) = applied(
        r#"[{"note":"Use auth=oauth2 for remote"},{"auth_type":"oauth2"},{"note":"secret=sha256 and pwd=v2.0"},{"algorithm":"sha256","version":"v2.0"},{"conn":"Server=db;User Id=sa;Password=p@ss w0rd;"},{"echo":"p@ss w0rd"},{"odbc":"Server=db;PWD={p@ss;w0rd};"},{"echo":"p@ss;w0rd"},{"ini":"set password = hunter2x! here"},{"echo":"hunter2x!"}]"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"[{"note":"Use auth=[withheld] for remote"},{"auth_type":"oauth2"},{"note":"secret=[withheld] and pwd=[withheld]"},{"algorithm":"sha256","version":"v2.0"},{"conn":"Server=db;User Id=sa;Password=[withheld];"},{"echo":"[withheld]"},{"odbc":"Server=db;PWD=[withheld];"},{"echo":"[withheld]"},{"ini":"set password = [withheld] here"},{"echo":"[withheld]"}]"#
    );
    for word in ["oauth2", "sha256", "v2.0"] {
        assert!(!secrets.carried_by(word), "{word}");
    }
    for secret in ["p@ss w0rd", "p@ss;w0rd", "hunter2x!"] {
        assert!(secrets.carried_by(secret), "{secret}");
    }
    assert!(checked(&text));
    let rows = parse(&text).unwrap();
    for (format, shown) in printed_formats(&rows) {
        for word in ["oauth2", "sha256", "v2.0"] {
            assert!(shown.contains(word), "{format:?}: {shown}");
        }
        for secret in ["w0rd", "hunter2x!"] {
            assert!(!shown.contains(secret), "{format:?}: {shown}");
        }
    }
}

/// Choice 183 (review: an Azure shared access signature and an API-key header line printed
/// their credentials under a neutral name): `SharedAccessSignature=` is withheld to the end of
/// its segment and its `sig` registered; `sig=` in prose and a credential header's value in a
/// `curl` line are withheld in place and registered, so their echoes are withheld too.
#[test]
fn d238_shared_access_signatures_and_api_key_headers_are_withheld() {
    let (text, secrets) = applied(
        r#"[{"connection":"BlobEndpoint=https://a.blob.core.windows.net/;SharedAccessSignature=sv=2020-08-04&ss=b&sig=SAS0001abcdefghijk%3D"},{"note":"curl -H 'X-Api-Key: HDR0001abcdefghijklm' https://h/v1"},{"note":"curl -H \"api-key:HDR0002abcdefghijklm\" https://h/v1"},{"echo":"SAS0001abcdefghijk%3D"},{"echo":"HDR0001abcdefghijklm"},{"echo":"HDR0002abcdefghijklm"},{"sig":"SAS0002abcdefghijk"}]"#,
        &PLAIN,
    );
    assert_eq!(
        text,
        r#"[{"connection":"BlobEndpoint=https://a.blob.core.windows.net/;SharedAccessSignature=[withheld]"},{"note":"curl -H 'X-Api-Key: [withheld]' https://h/v1"},{"note":"curl -H \"api-key:[withheld]\" https://h/v1"},{"echo":"[withheld]"},{"echo":"[withheld]"},{"echo":"[withheld]"},{"sig":"[withheld]"}]"#
    );
    for secret in [
        "SAS0001abcdefghijk%3D",
        "HDR0001abcdefghijklm",
        "HDR0002abcdefghijklm",
        "SAS0002abcdefghijk",
    ] {
        assert!(secrets.carried_by(secret), "{secret}");
    }
    assert!(checked(&text));
    // Model output keeps a header line of its answer (the header rule reads names in a text).
    let secrets = SecretSet::default();
    let mut answer = parse(r#"{"content":"curl -H 'X-Api-Key: $KEY_VALUE_123'"}"#).unwrap();
    apply_model_output(&mut answer, &secrets);
    assert_eq!(
        to_compact(&answer).unwrap(),
        r#"{"content":"curl -H 'X-Api-Key: $KEY_VALUE_123'"}"#
    );
}
