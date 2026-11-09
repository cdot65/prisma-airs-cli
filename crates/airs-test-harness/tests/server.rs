//! Self-tests of the fixture server through a raw HTTP/1.1 client.

mod support;

use std::time::{Duration, Instant};

use airs_test_harness::{
    AuthIdentity, BasicEncoding, FixtureServer, Planned, Route, base64_encode, fixtures,
    payload_hash, percent_encode,
};
use serde_json::{Value, json};
use support::{Client, bearer, request};

const FORM: (&str, &str) = ("content-type", "application/x-www-form-urlencoded");
const TOKEN_BODY: &[u8] = b"grant_type=client_credentials&scope=tsg_id%3A100";

fn server_with_tenants() -> FixtureServer {
    let server = FixtureServer::start();
    server.add_tenant(
        "100",
        "client-100",
        "FAKE-SECRET-100",
        Some("FAKE-APIKEY-100"),
        Some("FAKE-APITOKEN-100"),
    );
    server.add_tenant("200", "client-200", "s3cr+t/=x y", None, None);
    server
}

fn basic(id: &str, secret: &str) -> String {
    format!(
        "Basic {}",
        base64_encode(format!("{id}:{secret}").as_bytes())
    )
}

#[test]
fn oauth_accepts_basic_raw_basic_form_encoded_and_form_fields() {
    let server = server_with_tenants();

    let raw = basic("client-100", "FAKE-SECRET-100");
    let response = request(
        &server,
        "POST",
        "/oauth/token",
        &[("authorization", &raw), FORM],
        TOKEN_BODY,
    );
    assert_eq!(response.status, 200);
    assert_eq!(
        response.json(),
        json!({"access_token": "mock-token-100", "token_type": "Bearer", "expires_in": 3600})
    );

    // A secret with reserved characters: raw and RFC 6749 form-encoded halves both work.
    let body = b"grant_type=client_credentials&scope=tsg_id:200";
    let raw = basic("client-200", "s3cr+t/=x y");
    let encoded = basic(
        &percent_encode("client-200"),
        &percent_encode("s3cr+t/=x y"),
    );
    for header in [raw, encoded] {
        let response = request(
            &server,
            "POST",
            "/oauth/token",
            &[("authorization", &header), FORM],
            body,
        );
        assert_eq!(response.status, 200);
        assert_eq!(response.json()["access_token"], "mock-token-200");
    }

    let form = b"grant_type=client_credentials&scope=tsg_id%3A100&client_id=client-100&client_secret=FAKE-SECRET-100";
    assert_eq!(
        request(&server, "POST", "/oauth/token", &[FORM], form).status,
        200
    );

    let encodings: Vec<Option<BasicEncoding>> = server
        .requests()
        .into_iter()
        .map(|r| r.basic_encoding)
        .collect();
    assert_eq!(
        encodings,
        vec![
            Some(BasicEncoding::Indistinguishable),
            Some(BasicEncoding::Raw),
            Some(BasicEncoding::FormUrlEncoded),
            None
        ]
    );
    let identities: Vec<AuthIdentity> = server.requests().into_iter().map(|r| r.auth).collect();
    assert_eq!(
        identities,
        vec![
            AuthIdentity::Basic {
                client_id: "client-100".into(),
                tsg: Some("100".into()),
                valid: true
            },
            AuthIdentity::Basic {
                client_id: "client-200".into(),
                tsg: Some("200".into()),
                valid: true
            },
            AuthIdentity::Basic {
                client_id: "client-200".into(),
                tsg: Some("200".into()),
                valid: true
            },
            AuthIdentity::Form {
                client_id: "client-100".into(),
                tsg: Some("100".into()),
                valid: true
            },
        ]
    );
}

#[test]
fn oauth_rejects_bad_credentials_wrong_scope_and_grant() {
    let server = server_with_tenants();
    let wrong = basic("client-100", "WRONG-SECRET");
    let response = request(
        &server,
        "POST",
        "/oauth/token",
        &[("authorization", &wrong), FORM],
        TOKEN_BODY,
    );
    assert_eq!((response.status, response.json()), (401, json!({})));

    // Valid credentials for 100 but a scope naming 200.
    let good = basic("client-100", "FAKE-SECRET-100");
    let body = b"grant_type=client_credentials&scope=tsg_id%3A200";
    assert_eq!(
        request(
            &server,
            "POST",
            "/oauth/token",
            &[("authorization", &good), FORM],
            body
        )
        .status,
        401
    );

    let body = b"grant_type=password&scope=tsg_id%3A100";
    assert_eq!(
        request(
            &server,
            "POST",
            "/oauth/token",
            &[("authorization", &good), FORM],
            body
        )
        .status,
        400
    );

    assert_eq!(
        request(&server, "POST", "/oauth/token", &[FORM], TOKEN_BODY).status,
        401
    );
    let auth: Vec<AuthIdentity> = server.requests().into_iter().map(|r| r.auth).collect();
    assert_eq!(
        auth[0],
        AuthIdentity::Basic {
            client_id: "client-100".into(),
            tsg: Some("100".into()),
            valid: false
        }
    );
    assert_eq!(auth[3], AuthIdentity::None);
}

#[test]
fn management_requires_matching_bearer_and_refuses_scanner_keys() {
    let server = server_with_tenants();
    server.set_profiles("100", vec![fixtures::profile(1, "alpha", 1)]);
    let path = "/aisec/v1/mgmt/profiles/tsg/100?offset=0&limit=100&latest=true";
    let token = bearer("mock-token-100");

    assert_eq!(request(&server, "GET", path, &[], b"").status, 401);
    assert_eq!(
        request(
            &server,
            "GET",
            path,
            &[("authorization", &bearer("nope"))],
            b""
        )
        .status,
        401
    );
    // A registered scanner bearer is not a management token.
    assert_eq!(
        request(
            &server,
            "GET",
            path,
            &[("authorization", &bearer("FAKE-APITOKEN-100"))],
            b""
        )
        .status,
        401
    );
    // Token for another (registered) tenant: path TSG mismatch.
    assert_eq!(
        request(
            &server,
            "GET",
            path,
            &[("authorization", &bearer("mock-token-200"))],
            b""
        )
        .status,
        403
    );
    let with_key = [
        ("authorization", token.as_str()),
        ("x-pan-token", "FAKE-APIKEY-100"),
    ];
    assert_eq!(request(&server, "GET", path, &with_key, b"").status, 403);

    let response = request(&server, "GET", path, &[("authorization", &token)], b"");
    assert_eq!(response.status, 200);
    assert_eq!(response.json()["ai_profiles"][0]["profile_name"], "alpha");
    assert_eq!(response.header("content-type"), Some("application/json"));

    let last = server.requests().pop().expect("logged");
    assert_eq!(last.route, Route::Profiles);
    assert_eq!(last.path_tsg.as_deref(), Some("100"));
    assert_eq!(
        last.auth,
        AuthIdentity::Bearer {
            tsg: Some("100".into())
        }
    );
    assert_eq!(
        last.query,
        vec![
            ("offset".to_owned(), "0".to_owned()),
            ("limit".to_owned(), "100".to_owned()),
            ("latest".to_owned(), "true".to_owned())
        ]
    );
    assert_eq!(
        last.raw_query.as_deref(),
        Some("offset=0&limit=100&latest=true")
    );
}

fn rows(response: &support::Response, key: &str) -> Vec<(String, u64)> {
    response.json()[key]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| {
            let name = row.get("profile_name").or_else(|| row.get("topic_name"));
            (
                name.and_then(Value::as_str).unwrap_or_default().to_owned(),
                row["revision"].as_u64().unwrap_or_default(),
            )
        })
        .collect()
}

#[test]
fn pagination_offset_limit_latest_and_next_offset() {
    let server = server_with_tenants();
    server.set_profiles(
        "100",
        vec![
            fixtures::profile(1, "alpha", 1),
            fixtures::profile(2, "beta", 1),
            fixtures::profile(3, "alpha", 2),
            fixtures::profile(4, "gamma", 3),
            fixtures::profile(5, "beta", 1),
        ],
    );
    server.set_topics(
        "100",
        vec![
            fixtures::topic(11, "fin", 1, "a"),
            fixtures::topic(12, "fin", 2, "b"),
        ],
    );
    let token = bearer("mock-token-100");
    let get = |query: &str| {
        request(
            &server,
            "GET",
            &format!("/aisec/v1/mgmt/profiles/tsg/100?{query}"),
            &[("authorization", &token)],
            b"",
        )
    };

    let latest = get("offset=0&limit=100&latest=true");
    assert_eq!(
        rows(&latest, "ai_profiles"),
        vec![("alpha".into(), 2), ("beta".into(), 1), ("gamma".into(), 3)]
    );
    assert!(latest.json().get("next_offset").is_none());

    let first = get("offset=0&limit=2&latest=false");
    assert_eq!(rows(&first, "ai_profiles").len(), 2);
    assert_eq!(first.json()["next_offset"], 2);
    let last = get("offset=4&limit=2");
    assert_eq!(rows(&last, "ai_profiles"), vec![("beta".into(), 1)]);
    assert!(last.json().get("next_offset").is_none());
    assert_eq!(get("offset=9&limit=2").json(), json!({"ai_profiles": []}));
    assert_eq!(get("offset=x").status, 400);

    server.set_zero_next_offset_on_last_page(true);
    assert_eq!(get("offset=4&limit=2").json()["next_offset"], 0);
    assert_eq!(get("offset=0&limit=2").json()["next_offset"], 2);
    server.set_zero_next_offset_on_last_page(false);

    // Topics never collapse revisions, even with latest=true.
    let topics = request(
        &server,
        "GET",
        "/aisec/v1/mgmt/topics/tsg/100?offset=0&limit=200&latest=true",
        &[("authorization", &token)],
        b"",
    );
    assert_eq!(
        rows(&topics, "custom_topics"),
        vec![("fin".into(), 1), ("fin".into(), 2)]
    );
    // Unknown TSG dataset: empty.
    server.add_tenant("300", "c", "s", None, None);
    let empty = request(
        &server,
        "GET",
        "/aisec/v1/mgmt/profiles/tsg/300",
        &[("authorization", &bearer("mock-token-300"))],
        b"",
    );
    assert_eq!(empty.json(), json!({"ai_profiles": []}));
}

#[test]
fn scanner_verifies_payload_hash_and_rejects_management_bearers() {
    let server = server_with_tenants();
    let body = br#"{"ai_profile":{"profile_name":"P"},"contents":[{"prompt":"hello"}]}"#;
    let hash = payload_hash("FAKE-APIKEY-100", body);
    let path = "/scan/v1/scan/sync/request";
    let json_type = ("content-type", "application/json");

    let ok = request(
        &server,
        "POST",
        path,
        &[
            json_type,
            ("x-pan-token", "FAKE-APIKEY-100"),
            ("x-payload-hash", &hash),
        ],
        body,
    );
    assert_eq!(ok.status, 200);
    assert_eq!(ok.json(), fixtures::scan_allow("P"));

    let bad_hash = payload_hash("FAKE-APIKEY-100", b"other");
    assert_eq!(
        request(
            &server,
            "POST",
            path,
            &[
                ("x-pan-token", "FAKE-APIKEY-100"),
                ("x-payload-hash", &bad_hash)
            ],
            body
        )
        .status,
        401
    );
    assert_eq!(
        request(
            &server,
            "POST",
            path,
            &[("x-pan-token", "FAKE-APIKEY-100")],
            body
        )
        .status,
        401
    );
    let unknown_hash = payload_hash("UNKNOWN-KEY-1", body);
    assert_eq!(
        request(
            &server,
            "POST",
            path,
            &[
                ("x-pan-token", "UNKNOWN-KEY-1"),
                ("x-payload-hash", &unknown_hash)
            ],
            body
        )
        .status,
        401
    );
    assert_eq!(
        request(
            &server,
            "POST",
            path,
            &[("authorization", &bearer("mock-token-100"))],
            body
        )
        .status,
        401
    );
    assert_eq!(
        request(
            &server,
            "POST",
            path,
            &[("authorization", &bearer("FAKE-APITOKEN-100"))],
            body
        )
        .status,
        200
    );
    let both = [
        ("x-pan-token", "FAKE-APIKEY-100"),
        ("x-payload-hash", hash.as_str()),
        ("authorization", "Bearer FAKE-APITOKEN-100"),
    ];
    assert_eq!(request(&server, "POST", path, &both, body).status, 200);

    let results = request(
        &server,
        "GET",
        "/scan/v1/scan/results?scan_ids=00000000-0000-4000-8000-000000000001",
        &[("x-pan-token", "FAKE-APIKEY-100")],
        b"",
    );
    assert_eq!((results.status, results.json()), (200, json!([])));

    let auth: Vec<AuthIdentity> = server.requests().into_iter().map(|r| r.auth).collect();
    assert_eq!(
        auth,
        vec![
            AuthIdentity::ApiKey {
                key_id: Some("100".into()),
                hash_valid: true
            },
            AuthIdentity::ApiKey {
                key_id: Some("100".into()),
                hash_valid: false
            },
            AuthIdentity::ApiKey {
                key_id: Some("100".into()),
                hash_valid: false
            },
            AuthIdentity::ApiKey {
                key_id: None,
                hash_valid: true
            },
            AuthIdentity::Bearer {
                tsg: Some("100".into())
            },
            AuthIdentity::ApiToken {
                token_id: "100".into()
            },
            AuthIdentity::ApiKeyAndToken {
                key_id: Some("100".into()),
                hash_valid: true,
                token_id: Some("100".into())
            },
            AuthIdentity::ApiKey {
                key_id: Some("100".into()),
                hash_valid: true
            },
        ]
    );
    assert_eq!(server.requests()[0].body, body.to_vec());
}

#[test]
fn scan_rules_choose_verdicts_by_marker() {
    let server = server_with_tenants();
    server.add_scan_verdict("BLOCK-ME", &fixtures::scan_block("P"));
    server.add_scan_rule(
        "BREAK-ME",
        Planned::raw(200, "application/json", "not json"),
    );
    let send = |prompt: &str, response: Option<&str>| {
        let mut content = json!({"prompt": prompt});
        if let Some(response) = response {
            content["response"] = json!(response);
        }
        let body = json!({"ai_profile": {"profile_name": "P"}, "contents": [content]}).to_string();
        let hash = payload_hash("FAKE-APIKEY-100", body.as_bytes());
        request(
            &server,
            "POST",
            "/scan/v1/scan/sync/request",
            &[
                ("x-pan-token", "FAKE-APIKEY-100"),
                ("x-payload-hash", &hash),
            ],
            body.as_bytes(),
        )
    };
    assert_eq!(send("please BLOCK-ME now", None).json()["action"], "block");
    assert_eq!(send("fine", Some("BLOCK-ME")).json()["action"], "block");
    assert_eq!(send("BREAK-ME", None).body, b"not json".to_vec());
    assert_eq!(send("fine", None).json()["action"], "allow");
}

#[test]
fn overrides_once_then_default_then_builtin() {
    let server = server_with_tenants();
    let path = "/aisec/v1/mgmt/topics/tsg/100";
    let token = bearer("mock-token-100");
    let auth = [("authorization", token.as_str())];
    server.push_once(Route::Topics, Planned::empty_json(401));
    server.push_once(
        Route::Topics,
        Planned::json(429, &json!({"message": "slow down"})).with_retry_after(2),
    );
    server.set_default(
        Route::Topics,
        Planned::json(503, &json!({"message": "busy"})).with_retry_after(0),
    );

    let first = request(&server, "GET", path, &auth, b"");
    assert_eq!(first.status, 401);
    let second = request(&server, "GET", path, &auth, b"");
    assert_eq!(
        (second.status, second.header("retry-after")),
        (429, Some("2"))
    );
    for _ in 0..2 {
        let busy = request(&server, "GET", path, &auth, b"");
        assert_eq!((busy.status, busy.header("retry-after")), (503, Some("0")));
    }
    server.clear_overrides(Route::Topics);
    assert_eq!(request(&server, "GET", path, &auth, b"").status, 200);

    server.push_once(
        Route::Topics,
        Planned::redirect(302, "http://127.0.0.1:9/elsewhere"),
    );
    let redirect = request(&server, "GET", path, &auth, b"");
    assert_eq!(
        (redirect.status, redirect.header("location")),
        (302, Some("http://127.0.0.1:9/elsewhere"))
    );

    server.push_once(Route::Topics, Planned::raw(502, "text/plain", "oops"));
    let raw = request(&server, "GET", path, &auth, b"");
    assert_eq!(
        (raw.status, raw.body.as_slice(), raw.header("content-type")),
        (502, &b"oops"[..], Some("text/plain"))
    );

    // Overrides apply even to unauthenticated requests, which are still classified.
    server.push_once(
        Route::Token,
        Planned::json(200, &json!({"token_type": "Bearer"})),
    );
    assert_eq!(
        request(&server, "POST", "/oauth/token", &[], b"").json(),
        json!({"token_type": "Bearer"})
    );
    assert_eq!(
        server.requests_for(Route::Token)[0].auth,
        AuthIdentity::None
    );

    server.push_once(
        Route::Topics,
        Planned::empty_json(200).with_delay(Duration::from_millis(300)),
    );
    let started = Instant::now();
    assert_eq!(request(&server, "GET", path, &auth, b"").status, 200);
    assert!(started.elapsed() >= Duration::from_millis(300));
    assert_eq!(server.disconnects(), 0);
}

#[test]
fn hung_request_counts_client_disconnect() {
    let server = server_with_tenants();
    server.push_once(Route::Profiles, Planned::hang());
    let mut client = Client::connect(&server);
    client.send(
        "GET",
        "/aisec/v1/mgmt/profiles/tsg/100",
        &[("authorization", "Bearer mock-token-100")],
        b"",
    );
    assert!(server.wait_for_requests(1, Duration::from_secs(5)));
    assert_eq!(server.disconnects(), 0);
    drop(client);
    assert!(
        server.wait_for_disconnects(1, Duration::from_secs(5)),
        "disconnect not observed"
    );
    assert!(!server.wait_for_requests(2, Duration::from_millis(50)));
}

#[test]
fn keep_alive_reuses_one_connection() {
    let server = server_with_tenants();
    let mut client = Client::connect(&server);
    for _ in 0..3 {
        let response = client.request(
            "GET",
            "/scan/v1/scan/results?scan_ids=a",
            &[("x-pan-token", "FAKE-APIKEY-100")],
            b"",
        );
        assert_eq!(response.status, 200);
    }
    assert_eq!(server.connections(), 1);
    assert!(
        server
            .requests()
            .iter()
            .all(|request| request.connection == 0)
    );
    assert_eq!(
        server.requests().iter().map(|r| r.seq).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn unknown_routes_answer_404_and_are_recorded() {
    let server = server_with_tenants();
    let response = request(&server, "GET", "/nowhere?x=1", &[], b"");
    assert_eq!((response.status, response.json()), (404, json!({})));
    assert_eq!(
        request(&server, "GET", "/oauth/token", &[], b"").status,
        404
    );
    assert_eq!(server.routes(), vec![Route::Unknown, Route::Unknown]);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        server.assert_no_unknown_routes()
    }));
    assert!(outcome.is_err());
    server.clear_requests();
    server.assert_no_unknown_routes();
}

#[test]
fn aigw_workspaces_requires_management_bearer() {
    let server = server_with_tenants();
    let ok = request(
        &server,
        "GET",
        "/ai_gw/v2/workspaces",
        &[
            ("authorization", "Bearer mock-token-100"),
            ("x-tsg-id", "100"),
        ],
        b"",
    );
    assert_eq!(
        (ok.status, ok.json()),
        (200, fixtures::aigw_workspaces_empty())
    );
    assert_eq!(
        request(&server, "GET", "/ai_gw/v2/workspaces", &[], b"").status,
        401
    );
    // The gateway planes check the `x-tsg-id` header against the bearer's tenant.
    for tsg in [None, Some("200")] {
        let mut headers = vec![("authorization", "Bearer mock-token-100")];
        headers.extend(tsg.map(|tsg| ("x-tsg-id", tsg)));
        assert_eq!(
            request(&server, "GET", "/ai_gw/v2/workspaces", &headers, b"").status,
            403
        );
    }
}

/// The gateway routes (DESIGN §10 M6e): the planes, the seeded lists and details by id or
/// slug, archived workspaces only on the admin plane's archived list.
#[test]
fn gateway_routes_serve_the_seeded_dataset() {
    let server = server_with_tenants();
    server.seed_gateway();
    let get = |path: &str| {
        request(
            &server,
            "GET",
            path,
            &[
                ("authorization", "Bearer mock-token-100"),
                ("x-tsg-id", "100"),
            ],
            b"",
        )
    };
    let slugs = |path: &str| -> Vec<String> {
        get(path).json()["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["slug"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(slugs("/ai_gw/v2/workspaces"), ["ws-main-a-349e0e"]);
    assert_eq!(
        slugs("/ai_gw/admin/v2/workspaces"),
        ["ws-main-a-349e0e", "ws-produc-985697"]
    );
    assert_eq!(
        slugs("/ai_gw/admin/v2/workspaces?status=archived"),
        ["ws-old-aaaaaa"]
    );
    assert_eq!(get("/ai_gw/v2/workspaces/ws-produc-985697").status, 404);
    assert_eq!(
        get("/ai_gw/admin/v2/workspaces/ws-produc-985697").json()["status"],
        serde_json::Value::Null
    );
    assert_eq!(get("/ai_gw/admin/v2/workspaces/ws-old-aaaaaa").status, 404);
    assert_eq!(get("/iam/scopes").json()["count"], 3);
    assert_eq!(
        get("/iam/scopes/ws_orphan_zzzzzz").json()["resources"],
        serde_json::json!([])
    );
    assert_eq!(
        get("/ai_gw/v2/logs/charts/errors").json()["data"]["total"],
        49
    );
    assert_eq!(
        get("/ai_gw/v2/logs/groups/users").json()["data"]["total"],
        1
    );
    assert_eq!(
        get("/ai_gw/v2/logs?pageSize=2&currentPage=1").json()["data"]["records"][0]["id"],
        "log-3"
    );
    assert_eq!(get("/ai_gw/v2/analytics/filter-boundaries").status, 200);
    assert_eq!(
        Route::classify("GET", "/ai_gw/admin/v2/workspaces/x"),
        (
            Route::Gateway(airs_test_harness::Gw::AdminWorkspaceGet),
            None
        )
    );
    assert_eq!(
        Route::classify("POST", "/ai_gw/admin/v2/workspaces"),
        (Route::Gateway(airs_test_harness::Gw::WorkspaceCreate), None),
        "the chunk-3 write route"
    );
}

#[test]
fn gateway_inventory_routes_serve_their_fixture_ids_only() {
    use airs_test_harness::Gw;
    use airs_test_harness::fixtures::gateway as fx;
    let server = server_with_tenants();
    server.seed_gateway();
    let get = |path: &str| {
        request(
            &server,
            "GET",
            path,
            &[
                ("authorization", "Bearer mock-token-100"),
                ("x-tsg-id", "100"),
            ],
            b"",
        )
    };
    let ws = fx::uuid(1);
    for (path, route) in [
        (
            format!("/ai_gw/v2/api-keys/service?workspace_id={ws}"),
            Gw::ServiceKeys,
        ),
        (
            format!("/ai_gw/v2/api-keys/user/{}", fx::uuid(fx::USER_KEY)),
            Gw::UserKeyGet,
        ),
        (
            format!("/ai_gw/v2/configs/{}/versions", fx::uuid(fx::CONFIG)),
            Gw::ConfigVersions,
        ),
        (
            format!(
                "/ai_gw/admin/v2/deployments/{}/ping",
                fx::uuid(fx::DEPLOYMENT)
            ),
            Gw::DeploymentPing,
        ),
        (
            "/ai_gw/admin/v2/utils/static-resources/schema?resource=guardrails".to_owned(),
            Gw::GuardrailCatalog,
        ),
        (
            "/ai_gw/admin/v2/utils/static-resources/ai-providers".to_owned(),
            Gw::CatalogProviders,
        ),
        (
            format!(
                "/ai_gw/admin/v2/integrations/{}/workspaces",
                fx::uuid(fx::INTEGRATION)
            ),
            Gw::IntegrationWorkspaces,
        ),
        (
            format!(
                "/ai_gw/admin/v2/mcp-integrations/{}/metadata",
                fx::uuid(fx::MCP_INTEGRATION)
            ),
            Gw::McpMetadata,
        ),
        (
            "/ai_gw/admin/v2/organisations/self".to_owned(),
            Gw::OrganisationSelf,
        ),
        (
            "/ai_gw/admin/v2/organisations/100/auth-settings".to_owned(),
            Gw::AuthSettings,
        ),
        (
            "/ai_gw/admin/v2/guardrails?page_size=5".to_owned(),
            Gw::AdminGuardrails,
        ),
        (
            format!(
                "/ai_gw/admin/v2/guardrails/{}/mcp-servers",
                fx::uuid(fx::ADMIN_GUARDRAIL)
            ),
            Gw::AdminMcpMappings,
        ),
        (
            format!("/ai_gw/v2/guardrails/{}", fx::uuid(fx::GUARDRAIL)),
            Gw::GuardrailGet,
        ),
    ] {
        let path_only = path.split('?').next().unwrap_or_default();
        assert_eq!(
            Route::classify("GET", path_only),
            (Route::Gateway(route), None),
            "{path}"
        );
        assert_eq!(get(&path).status, 200, "{path}");
    }
    assert_eq!(Gw::INVENTORY.len(), 32);
    assert!(Gw::INVENTORY.iter().all(|route| !route.is_write()));
    assert_eq!(Gw::Plugins.plane(), "admin");
    assert_eq!(Gw::ProviderGet.plane(), "data");
    // Another id is a 404, another workspace an empty list, another TSG a 403.
    assert_eq!(
        get(&format!("/ai_gw/v2/configs/{}", fx::uuid(9))).status,
        404
    );
    assert_eq!(
        get(&format!("/ai_gw/v2/providers?workspace_id={}", fx::uuid(9))).json()["data"],
        serde_json::json!([])
    );
    assert_eq!(get("/ai_gw/admin/v2/organisations/999/info").status, 403);
    // The seeded answers carry the credential markers.
    let detail = get(&format!("/ai_gw/v2/providers/{}", fx::uuid(fx::PROVIDER))).json();
    assert_eq!(detail["key"], fx::marker(24));
    assert!(fx::markers().contains(&fx::marker(29)));
    assert_eq!(
        Route::classify("POST", "/ai_gw/v2/configs"),
        (Route::Gateway(airs_test_harness::Gw::ConfigCreate), None),
        "the chunk-3 write route"
    );
}

#[test]
fn gateway_write_routes_change_the_dataset_or_answer_their_fixture_ids() {
    use airs_test_harness::Gw;
    use airs_test_harness::fixtures::gateway as fx;
    let server = server_with_tenants();
    server.seed_gateway();
    let send = |method: &str, path: &str, body: Value| {
        request(
            &server,
            method,
            path,
            &[
                ("authorization", "Bearer mock-token-100"),
                ("x-tsg-id", "100"),
                ("content-type", "application/json"),
            ],
            body.to_string().as_bytes(),
        )
    };
    // The workspace and scope writes change the dataset.
    let created = send(
        "POST",
        "/iam/scopes",
        json!({"name": "ws_new_aaaaaa", "description": "", "resources": []}),
    );
    assert_eq!(created.status, 200);
    assert_eq!(
        send("POST", "/iam/scopes", json!({"name": "ws_new_aaaaaa"})).status,
        409
    );
    let workspace = send(
        "POST",
        "/ai_gw/admin/v2/workspaces",
        json!({"name": "New One", "scope_name": "ws_new_aaaaaa"}),
    )
    .json();
    assert_eq!(workspace["id"], fx::uuid(4));
    assert_eq!(workspace["slug"], "ws-newone-000004");
    assert_eq!(
        send(
            "PUT",
            "/ai_gw/admin/v2/workspaces/ws-newone-000004",
            json!({"icon": "x"})
        )
        .json(),
        json!({})
    );
    assert_eq!(
        send(
            "DELETE",
            "/ai_gw/admin/v2/workspaces/ws-newone-000004",
            json!({})
        )
        .status,
        204
    );
    assert_eq!(
        server.gateway_data().workspaces.last().unwrap()["status"],
        "archived"
    );
    assert_eq!(
        send("DELETE", "/iam/scopes/ws_new_aaaaaa", json!({})).status,
        204
    );
    assert_eq!(
        send("DELETE", "/iam/scopes/ws_new_aaaaaa", json!({})).status,
        404
    );
    // The other writes answer for their fixture ids only.
    assert_eq!(
        send(
            "PUT",
            &format!("/ai_gw/v2/configs/{}", fx::uuid(fx::CONFIG)),
            json!({"name": "n"})
        )
        .status,
        200
    );
    assert_eq!(
        send(
            "PUT",
            &format!("/ai_gw/v2/configs/{}", fx::uuid(9)),
            json!({"name": "n"})
        )
        .status,
        404
    );
    assert_eq!(
        send("POST", "/ai_gw/v2/api-keys/user", json!({})).json()["key"],
        fx::marker(31)
    );
    assert_eq!(
        send(
            "POST",
            &format!(
                "/ai_gw/v2/api-keys/service/{}/rotate",
                fx::uuid(fx::SERVICE_KEY)
            ),
            json!({})
        )
        .json()["key"],
        fx::marker(32)
    );
    assert_eq!(
        send(
            "PUT",
            &format!("/ai_gw/admin/v2/deployments/{}", fx::uuid(fx::DEPLOYMENT)),
            json!({"rotate_auth": true})
        )
        .json()["client_auth"],
        fx::marker(35)
    );
    assert_eq!(
        send(
            "DELETE",
            &format!(
                "/ai_gw/admin/v2/integrations/{}?organisation_id=999",
                fx::uuid(fx::INTEGRATION)
            ),
            json!({})
        )
        .status,
        400
    );
    assert_eq!(
        send(
            "PUT",
            "/ai_gw/admin/v2/organisations/999/auth-settings",
            json!({})
        )
        .status,
        403
    );
    for gw in Gw::WRITES {
        assert!(gw.is_write(), "{gw:?}");
        assert!(gw.label().starts_with("gw-"));
    }
    assert!(!Gw::ConfigGet.is_write());
}

#[test]
fn gateway_inference_routes_check_the_runtime_key() {
    use airs_test_harness::fixtures::gateway as fx;
    use airs_test_harness::{Gw, Route};
    let server = server_with_tenants();
    server.seed_gateway();
    let send = |path: &str, headers: &[(&str, &str)], body: Value| {
        let mut all = vec![("content-type", "application/json")];
        all.extend_from_slice(headers);
        request(&server, "POST", path, &all, body.to_string().as_bytes())
    };
    let key = [("x-portkey-api-key", fx::INFERENCE_KEY)];
    let chat = send(
        "/ai_gw/inference/chat/completions",
        &key,
        json!({"model": "m"}),
    );
    assert_eq!(chat.status, 200);
    assert_eq!(chat.json(), fx::chat_completion("Hello from the fixture"));
    let stream = send(
        "/ai_gw/inference/chat/completions",
        &key,
        json!({"model": "m", "stream": true}),
    );
    assert_eq!(stream.status, 200);
    assert!(
        stream
            .header("content-type")
            .unwrap()
            .starts_with("text/event-stream")
    );
    assert!(String::from_utf8_lossy(&stream.body).ends_with("data: [DONE]\n\n"));
    assert_eq!(
        send("/ai_gw/inference/embeddings", &key, json!({})).json(),
        fx::embedding()
    );
    assert_eq!(
        send("/ai_gw/inference/responses", &key, json!({})).json()["status"],
        "completed"
    );
    // A wrong key, no key, or any Authorization header: 401.
    assert_eq!(
        send(
            "/ai_gw/inference/responses",
            &[("x-portkey-api-key", "other")],
            json!({})
        )
        .status,
        401
    );
    assert_eq!(
        send("/ai_gw/inference/responses", &[], json!({})).status,
        401
    );
    let both = [
        ("x-portkey-api-key", fx::INFERENCE_KEY),
        ("authorization", "Bearer mock-token-100"),
    ];
    assert_eq!(
        send("/ai_gw/inference/embeddings", &both, json!({})).status,
        401
    );
    assert_eq!(
        server.requests_for(Route::Gateway(Gw::InferenceChat)).len(),
        2
    );
    assert!(Gw::InferenceChat.is_inference() && !Gw::InferenceChat.is_write());
    assert_eq!(Gw::InferenceChat.plane(), "runtime");
}

#[test]
fn debug_output_redacts_credentials() {
    let server = server_with_tenants();
    let form = b"grant_type=client_credentials&scope=tsg_id%3A100&client_id=client-100&client_secret=FAKE-SECRET-100";
    request(
        &server,
        "POST",
        "/oauth/token",
        &[
            FORM,
            ("authorization", &basic("client-100", "FAKE-SECRET-100")),
        ],
        form,
    );
    request(
        &server,
        "GET",
        "/scan/v1/scan/results",
        &[("x-pan-token", "FAKE-APIKEY-100")],
        b"",
    );
    let debug = format!("{:?}", server.requests());
    for secret in [
        "FAKE-SECRET-100",
        "FAKE-APIKEY-100",
        &base64_encode(b"client-100:FAKE-SECRET-100"),
    ] {
        assert!(!debug.contains(secret), "Debug leaked a secret: {debug}");
    }
    assert!(debug.contains("client_secret"), "{debug}");
}

#[test]
fn rotating_tokens_and_bearer_echo() {
    let server = server_with_tenants();
    server.set_rotating_tokens(true);
    let raw = basic("client-100", "FAKE-SECRET-100");
    let token = |server: &FixtureServer| {
        request(
            server,
            "POST",
            "/oauth/token",
            &[("authorization", &raw), FORM],
            TOKEN_BODY,
        )
        .json()["access_token"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let first = token(&server);
    let second = token(&server);
    assert_eq!(
        (first.as_str(), second.as_str()),
        ("mock-token-100.r1", "mock-token-100.r2")
    );
    // Rotated tokens are still management bearers for their TSG.
    let profiles = request(
        &server,
        "GET",
        "/aisec/v1/mgmt/profiles/tsg/100",
        &[("authorization", &bearer(&second))],
        b"",
    );
    assert_eq!(profiles.status, 200);

    server.push_once(
        Route::Profiles,
        Planned::json(401, &json!({"code": Planned::BEARER}))
            .with_header("x-request-id", Planned::BEARER)
            .echoing_bearer(),
    );
    let echoed = request(
        &server,
        "GET",
        "/aisec/v1/mgmt/profiles/tsg/100",
        &[("authorization", &bearer(&first))],
        b"",
    );
    assert_eq!(echoed.status, 401);
    assert_eq!(echoed.json(), json!({"code": "mock-token-100.r1"}));
    assert_eq!(echoed.header("x-request-id"), Some("mock-token-100.r1"));
}

/// The stateful M4 write routes (DESIGN §10 M4): the tenant is the bearer's TSG.
#[test]
fn write_routes_change_the_served_state() {
    let server = server_with_tenants();
    let mut alpha = fixtures::profile(1, "alpha", 1);
    alpha["policy"]["ai-security-profiles"][0]["model-configuration"]["model-protection"] = json!([{
        "name": "topic-guardrails", "action": "allow",
        "topic-list": [{"action": "block", "topic": [{"topic_id": fixtures::uuid(11), "topic_name": "fin", "revision": 1}]}]
    }]);
    server.set_profiles("100", vec![alpha]);
    server.set_topics(
        "100",
        vec![
            fixtures::topic(11, "fin", 1, "d"),
            fixtures::topic(12, "other", 1, "d"),
        ],
    );
    let token = bearer("mock-token-100");
    let auth = [("authorization", token.as_str())];
    let json_auth = [
        ("authorization", token.as_str()),
        ("content-type", "application/json"),
    ];
    assert_eq!(
        request(&server, "POST", "/aisec/v1/mgmt/topic", &[], b"{}").status,
        401
    );
    let created = request(
        &server,
        "POST",
        "/aisec/v1/mgmt/topic",
        &json_auth,
        br#"{"topic_name": "new", "description": "x", "examples": ["a", "b"]}"#,
    );
    assert_eq!(created.status, 200);
    assert_eq!(created.json()["topic_id"], fixtures::uuid(901));
    assert_eq!(created.json()["revision"], 1);
    let updated = request(
        &server,
        "PUT",
        &format!("/aisec/v1/mgmt/topic/uuid/{}", fixtures::uuid(12)),
        &json_auth,
        br#"{"topic_name": "other", "description": "changed"}"#,
    );
    assert_eq!(updated.status, 200);
    assert_eq!(updated.json()["topic_id"], fixtures::uuid(12));
    assert_eq!(updated.json()["revision"], 2);
    assert_eq!(updated.json()["description"], "changed");
    let conflict = request(
        &server,
        "DELETE",
        &format!("/aisec/v1/mgmt/topic/{}", fixtures::uuid(11)),
        &auth,
        b"",
    );
    assert_eq!(conflict.status, 409);
    assert_eq!(conflict.json()["payload"][0]["profile_name"], "alpha");
    let no_audit = format!(
        "/aisec/v1/mgmt/topic/{}/force?updated_by=%20",
        fixtures::uuid(11)
    );
    assert_eq!(
        request(&server, "DELETE", &no_audit, &auth, b"").status,
        400
    );
    let forced = format!(
        "/aisec/v1/mgmt/topic/{}/force?updated_by=a%40b",
        fixtures::uuid(11)
    );
    assert_eq!(request(&server, "DELETE", &forced, &auth, b"").status, 200);
    assert!(
        server.profiles("100")[0]
            .to_string()
            .contains("\"topic\":[]"),
        "the force delete removed the refs"
    );
    let topics: Vec<Value> = server.topics("100");
    assert_eq!(topics.len(), 2);
    assert_eq!(
        request(
            &server,
            "DELETE",
            &format!("/aisec/v1/mgmt/topic/{}", fixtures::uuid(11)),
            &auth,
            b""
        )
        .status,
        404
    );
    server.set_put_appends_new_record(true);
    let appended = request(
        &server,
        "PUT",
        &format!("/aisec/v1/mgmt/profile/uuid/{}", fixtures::uuid(1)),
        &json_auth,
        br#"{"profile_name": "alpha", "active": false, "policy": {}}"#,
    );
    assert_eq!(appended.json()["profile_id"], fixtures::uuid(902));
    assert_eq!(appended.json()["revision"], 2);
    assert_eq!(server.profiles("100").len(), 2, "the old revision is kept");
    assert_eq!(
        request(
            &server,
            "DELETE",
            &format!("/aisec/v1/mgmt/profile/{}", fixtures::uuid(1)),
            &auth,
            b""
        )
        .status,
        200
    );
    assert_eq!(server.profiles("100").len(), 1);
    let routes: Vec<Route> = server.requests().iter().map(|r| r.route).collect();
    assert!(routes.contains(&Route::TopicForceDelete));
    assert!(routes.contains(&Route::ProfileUpdate));
}

/// The trap answers `404 {}` to everything and records it (D93).
#[test]
fn the_env_trap_answers_404() {
    let server = server_with_tenants();
    let token = bearer("mock-token-100");
    let response = request(
        &server,
        "GET",
        "/env-trap/v1/mgmt/topics/tsg/100?offset=0&limit=100",
        &[("authorization", &token)],
        b"",
    );
    assert_eq!(response.status, 404);
    assert_eq!(response.json(), json!({}));
    let last = server.requests().pop().unwrap();
    assert_eq!((last.route, last.base), (Route::EnvTrap, "/env-trap"));
    assert!(server.env_trap_url().ends_with("/env-trap"));
}

/// The supplychain routes (DESIGN §10 M6f, §20.13): `GET` only, the bearer and the
/// `x-tsg-id` header checked, skip/limit pages over the seeded dataset in the service's
/// member order (never sorted), the rules `total_items` the page length (G62).
#[test]
fn supplychain_routes_check_the_bearer_and_the_tsg_header() {
    use airs_test_harness::Sc;
    use fixtures::supplychain as fx;
    let server = server_with_tenants();
    server.seed_supplychain();
    let scans = "/agentguard/data/v1/scans?skip=0&limit=2&isBackgroundRefresh=false";
    // No bearer, a wrong or missing TSG header, a write method.
    assert_eq!(request(&server, "GET", scans, &[], b"").status, 401);
    for tsg in [None, Some("200")] {
        let mut headers = vec![("authorization", "Bearer mock-token-100")];
        headers.extend(tsg.map(|tsg| ("x-tsg-id", tsg)));
        assert_eq!(request(&server, "GET", scans, &headers, b"").status, 403);
    }
    let get = |path: &str| {
        request(
            &server,
            "GET",
            path,
            &[
                ("authorization", "Bearer mock-token-100"),
                ("x-tsg-id", "100"),
            ],
            b"",
        )
    };
    let page = get(scans);
    assert_eq!(page.status, 200);
    let text = String::from_utf8(page.body.clone()).unwrap();
    assert!(
        text.starts_with(
            "{\"pagination\":{\"total_items\":12,\"zz_cursor\":null,\"aa_page_hint\":1},\"scans\":[{\"uuid\":"
        ),
        "{text:.200}"
    );
    assert!(text.contains("\"summary\":{\"vulnerability_count\":2,\"attack_chain_count\":1}"));
    assert_eq!(page.json()["scans"].as_array().unwrap().len(), 2);
    let stats = get("/agentguard/data/v1/stats/scans?time_period=30_DAYS");
    assert_eq!(
        stats.json()["unique_skills_scanned"]["percent_change"],
        12.5
    );
    assert_eq!(
        get("/agentguard/data/v1/stats/scans?time_period=7_DAYS").status,
        400
    );
    let findings = get(&format!(
        "/agentguard/data/v1/scans/{}/vulnerabilities",
        fx::uuid(101).to_uppercase()
    ));
    assert_eq!(findings.json()["pagination"]["total_items"], 3);
    assert_eq!(
        get(&format!(
            "/agentguard/data/v1/scans/{}/vulnerabilities",
            fx::uuid(102)
        ))
        .status,
        404
    );
    let rules = get("/agentguard/mgmt/v1/rules?skip=20&limit=10");
    assert_eq!(
        rules.json()["pagination"]["total_items"],
        5,
        "the page length"
    );
    assert_eq!(rules.json()["rules"].as_array().unwrap().len(), 5);
    assert_eq!(
        request(&server, "POST", "/agentguard/mgmt/v1/rules", &[], b"").status,
        404
    );
    assert_eq!(
        server.sc_requests(Sc::Scans)[0].raw_query.as_deref(),
        Some("skip=0&limit=2&isBackgroundRefresh=false")
    );
    assert_eq!(server.sc_requests(Sc::Rules).len(), 1);
    assert_eq!(server.requests_for(Route::Unknown).len(), 1);
}
