//! M0.2/M0.3 loopback feasibility: the real spike router behind a fixture JWKS
//! issuer, driven by rmcp's Streamable HTTP client and raw HTTP. Loopback only;
//! this cannot pass the M0.6 live gate.

use std::collections::HashMap;
use std::os::unix::fs::DirBuilderExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use airs_mcp_spike::{config, run, tools};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientConfig},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Map, Value, json};
use tokio::sync::Mutex;

const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const TID: &str = "00000000-0000-4000-8000-0000000000aa";
const API: &str = "00000000-0000-4000-8000-0000000000ab";
const GW_CLIENT: &str = "00000000-0000-4000-8000-0000000000ac";
const OWNER: &str = "00000000-0000-4000-8000-0000000000ad";
/// A second allowlisted integration owner that no grant names.
const OWNER2: &str = "00000000-0000-4000-8000-0000000000ae";
const BACKEND: &str = "https://mcp-backend-airs.example.test/mcp";

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn key(name: &str) -> EncodingKey {
    EncodingKey::from_rsa_pem(&std::fs::read(format!("{FIX}/{name}-private.pem")).unwrap()).unwrap()
}

fn sign(key_name: &str, kid: &str, claims: &Value) -> String {
    let mut h = Header::new(Algorithm::RS256);
    h.kid = Some(kid.into());
    encode(&h, claims, &key(key_name)).unwrap()
}

fn upstream_claims() -> Value {
    json!({"iss": format!("https://login.microsoftonline.com/{TID}/v2.0"), "aud": API, "tid": TID,
        "oid": OWNER, "azp": GW_CLIENT, "scp": "AirsHistory.Read", "ver": "2.0",
        "iat": now(), "nbf": now() - 5, "exp": now() + 600})
}

fn assertion_claims(sub: &str, agent: &str) -> Value {
    json!({"iss": "https://mcp.example.test", "aud": BACKEND, "sub": sub, "client_id": agent,
        "workspace_id": "ws-test", "organisation_id": "org-test", "scope": "mcp",
        "iat": now(), "exp": now() + 120})
}

fn upstream(f: impl FnOnce(&mut Map<String, Value>)) -> String {
    let mut c = upstream_claims();
    f(c.as_object_mut().unwrap());
    sign("entra", "entra-1", &c)
}

fn assertion(sub: &str, agent: &str, f: impl FnOnce(&mut Map<String, Value>)) -> String {
    let mut c = assertion_claims(sub, agent);
    f(c.as_object_mut().unwrap());
    sign("gateway", "gw-1", &c)
}

struct Harness {
    url: String,
    jwks_fetches: Arc<AtomicUsize>,
    _dir: tempdir::Dir,
}

mod tempdir {
    pub struct Dir(pub std::path::PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

async fn serve(router: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("127.0.0.1:{}", addr.port())
}

async fn harness(admin: &str) -> Harness {
    harness_with("", admin).await
}

async fn harness_with(server_extra: &str, admin: &str) -> Harness {
    harness_full(server_extra, admin, &format!("[\"{BACKEND}\"]"), None).await
}

/// `route_secret`: when set, a `[route_binding]` section is added with this secret.
async fn harness_full(
    server_extra: &str,
    admin: &str,
    assertion_audiences: &str,
    route_secret: Option<&str>,
) -> Harness {
    let fetches = Arc::new(AtomicUsize::new(0));
    let (f1, f2) = (fetches.clone(), fetches.clone());
    let entra: Value =
        serde_json::from_slice(&std::fs::read(format!("{FIX}/entra-jwks.json")).unwrap()).unwrap();
    let gw: Value =
        serde_json::from_slice(&std::fs::read(format!("{FIX}/gateway-jwks.json")).unwrap())
            .unwrap();
    let idp = axum::Router::new()
        .route(
            "/entra/keys",
            axum::routing::get(move || {
                f1.fetch_add(1, Ordering::SeqCst);
                let v = entra.clone();
                async move { axum::Json(v) }
            }),
        )
        .route(
            "/gw/jwks.json",
            axum::routing::get(move || {
                f2.fetch_add(1, Ordering::SeqCst);
                let v = gw.clone();
                async move { axum::Json(v) }
            }),
        );
    let idp_addr = serve(idp).await;

    let dir = std::env::temp_dir().join(format!("airs-mcp-loopback-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let route = match route_secret {
        None => String::new(),
        Some(secret) => {
            let f = dir.join("route-secret");
            std::fs::write(&f, format!("{secret}\n")).unwrap();
            format!(
                "[route_binding]\nheader = \"x-airs-backend-binding\"\nsecret_file = \"{}\"\n",
                f.display()
            )
        }
    };
    // Session-binding tests predate the stateless default; they opt into `bound`
    // unless the caller names a mode.
    let server_extra = if server_extra
        .lines()
        .any(|l| l.trim_start().starts_with("sessions "))
    {
        server_extra.to_owned()
    } else {
        format!("sessions = \"bound\"\n{server_extra}")
    };
    let text = format!(
        r#"
revision = "test-1"
[server]
listen = "127.0.0.1:0"
allowed_hosts = ["127.0.0.1"]
resource = "{BACKEND}"
authorization_servers = ["https://login.microsoftonline.com/{TID}/v2.0"]
scopes_supported = ["{BACKEND}/AirsHistory.Read"]
data_dir = "{data}"
allow_loopback_http_jwks = true
{server_extra}
[upstream]
issuer = "https://login.microsoftonline.com/{TID}/v2.0"
audiences = ["{API}"]
tenant_id = "{TID}"
jwks_url = "http://{idp_addr}/entra/keys"
algorithms = ["RS256"]
accepted_permissions = ["AirsHistory.Read", "AirsHistory.Operate", "AirsHistory.Admin"]
allowed_client_ids = ["{GW_CLIENT}"]
owner_object_ids = ["{OWNER}", "{OWNER2}"]
[assertion]
header = "x-user-jwt"
issuer = "https://mcp.example.test"
audiences = {assertion_audiences}
algorithms = ["RS256"]
jwks_url = "http://{idp_addr}/gw/jwks.json"
max_lifetime_seconds = 300
workspace_ids = ["ws-test"]
organization_ids = ["org-test"]
{route}
{admin}
[[grants]]
subject = "human-a"
client_id = "claude-code"
owner_object_id = "{OWNER}"
permission = "reader"
tools = ["service_info", "echo", "text_page", "artifact_chunk"]
[[grants]]
subject = "human-a"
client_id = "ops-agent"
owner_object_id = "{OWNER}"
permission = "operator"
tools = ["service_info", "echo"]
[[grants]]
subject = "human-a"
client_id = "ops-agent-2"
owner_object_id = "{OWNER}"
permission = "operator"
tools = ["service_info", "operator_noop"]
[[grants]]
subject = "human-a"
client_id = "airs-admin-cli-2"
owner_object_id = "{OWNER}"
permission = "administrator"
tools = ["service_info", "admin_preview", "admin_execute"]
[[grants]]
subject = "human-a"
client_id = "airs-admin-cli"
owner_object_id = "{OWNER}"
permission = "administrator"
tools = ["service_info", "operator_noop", "admin_preview", "admin_execute"]
"#,
        data = dir.display()
    );
    let cfg = Arc::new(config::parse(&text).expect("fixture config"));
    let admitted = airs_mcp_spike::storage::admit(&cfg.server.data_dir).unwrap();
    let spike = tools::Spike {
        config: cfg.clone(),
        storage: Some((
            admitted,
            airs_mcp_spike::storage::OwnerRecord {
                owner_id: "test".into(),
                pod: "test".into(),
                node: "test".into(),
                pid: 0,
                acquired_at_unix: 0,
            },
        )),
        state: Arc::new(Mutex::new(tools::State::default())),
        artifact_sha256: Arc::new(tools::artifact_sha256()),
    };
    let addr = serve(run::router(cfg, spike).unwrap()).await;
    Harness {
        url: format!("http://{addr}/mcp"),
        jwks_fetches: fetches,
        _dir: tempdir::Dir(dir),
    }
}

/// Raw initialize POST: returns HTTP status and the JSON error code if any.
async fn raw(url: &str, headers: &[(&str, String)]) -> (u16, Option<String>) {
    let client = reqwest::Client::new();
    let mut req = client
        .post(url)
        .header("accept", "application/json, text/event-stream")
        .header("content-type", "application/json")
        .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#);
    for (k, v) in headers {
        req = req.header(*k, v);
    }
    let resp = req.send().await.unwrap();
    let status = resp.status().as_u16();
    let code = resp
        .json::<Value>()
        .await
        .ok()
        .and_then(|v| v.get("code").and_then(Value::as_str).map(str::to_owned));
    (status, code)
}

fn creds(bearer: String, ia: String) -> Vec<(&'static str, String)> {
    vec![
        ("authorization", format!("Bearer {bearer}")),
        ("x-user-jwt", ia),
    ]
}

async fn client(
    url: &str,
    bearer: String,
    ia: String,
) -> rmcp::service::RunningService<rmcp::RoleClient, ClientConfig> {
    let mut headers = HashMap::new();
    headers.insert(
        http::HeaderName::from_static("x-user-jwt"),
        http::HeaderValue::from_str(&ia).unwrap(),
    );
    let cfg = StreamableHttpClientTransportConfig::with_uri(url.to_owned())
        .auth_header(bearer)
        .custom_headers(headers);
    ClientConfig::default()
        .serve(StreamableHttpClientTransport::from_config(cfg))
        .await
        .expect("connect")
}

async fn call(
    c: &rmcp::service::RunningService<rmcp::RoleClient, ClientConfig>,
    tool: &str,
    args: Value,
) -> (bool, Value) {
    let args = args.as_object().cloned().unwrap_or_default();
    let r = c
        .call_tool(CallToolRequestParams::new(tool.to_owned()).with_arguments(args))
        .await
        .expect("call");
    (
        r.is_error.unwrap_or(false),
        r.structured_content.clone().unwrap_or(Value::Null),
    )
}

/// Like `call`, also returning the COMPLETE serialized result size (value plus
/// rmcp's duplicated text content), which is what the bounds govern.
async fn call_sized(
    c: &rmcp::service::RunningService<rmcp::RoleClient, ClientConfig>,
    tool: &str,
    args: Value,
) -> (bool, Value, usize) {
    let args = args.as_object().cloned().unwrap_or_default();
    let r = c
        .call_tool(CallToolRequestParams::new(tool.to_owned()).with_arguments(args))
        .await
        .expect("call");
    let size = serde_json::to_vec(&r).unwrap().len();
    (
        r.is_error.unwrap_or(false),
        r.structured_content.clone().unwrap_or(Value::Null),
        size,
    )
}

fn scoped(scp: &'static str) -> String {
    upstream(move |c| {
        c.insert("scp".into(), json!(scp));
    })
}

#[tokio::test]
async fn reader_flow_shapes_pagination_and_32mib_transfer() {
    let h = harness("").await;
    let c = client(
        &h.url,
        upstream(|_| {}),
        assertion("human-a", "claude-code", |_| {}),
    )
    .await;
    let tools: Vec<String> = c
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    assert!(
        tools.contains(&"service_info".to_owned()) && !tools.contains(&"admin_execute".to_owned())
    );

    let (err, info) = call(&c, "service_info", json!({})).await;
    assert!(!err, "{info}");
    // Shapes only: no claim values appear anywhere in the result.
    let text = info.to_string();
    for secretish in [OWNER, TID, "human-a", "claude-code", "ws-test"] {
        assert!(
            !text.contains(secretish),
            "service_info leaked a claim value"
        );
    }
    assert_eq!(info["claim_shapes"]["assertion"]["sub"], "string");

    // Operator tool on a reader grant is denied even though it is in the tool list.
    let (err, v) = call(&c, "operator_noop", json!({})).await;
    assert!(err && v["code"] == "forbidden");
    // Unknown arguments are refused before effects.
    let (err, v) = call(&c, "echo", json!({"text": "x", "sql": "drop"})).await;
    assert!(err && v["code"] == "invalid_request");

    // The runbook's echo sizes: 120 KB of text fits the 256 KiB complete-result
    // bound; 200 KB is refused locally by the spike (duplicated content).
    let (err, v, size) = call_sized(&c, "echo", json!({"text": "xxxx", "repeat": 30000})).await;
    assert!(!err && size <= tools::CLI_RESULT_LIMIT, "{size}");
    assert_eq!(v["bytes"], 120000);
    let (err, v, _) = call_sized(&c, "echo", json!({"text": "xxxx", "repeat": 50000})).await;
    assert!(
        err && v["reason"] == "result exceeds its response bound",
        "{v}"
    );
    assert_eq!(v["code"], "result_too_large", "specific bound code (M0.6)");

    // Agent-facing pages each stay within 16 KiB serialized.
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    let mut max_page = 0usize;
    loop {
        let args = cursor
            .as_ref()
            .map(|c| json!({"cursor": c}))
            .unwrap_or(json!({}));
        let (err, v, size) = call_sized(&c, "text_page", args).await;
        assert!(!err, "{v}");
        assert!(
            size <= tools::AGENT_PAGE_LIMIT,
            "complete page result {size} > 16 KiB"
        );
        max_page = max_page.max(size);
        pages += 1;
        match v["next_cursor"].as_str() {
            Some(n) => cursor = Some(n.to_owned()),
            None => break,
        }
    }
    assert!(pages > 1);
    println!("M0.2 loopback agent pages: pages={pages} max_complete_result_bytes={max_page}");

    // CLI byte transfer: 32 MiB in 64 KiB chunks, hash-verified.
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let start = Instant::now();
    let mut hasher = Sha256::new();
    let mut offset = 0u64;
    let mut calls = 0;
    let mut max_result = 0usize;
    while offset < tools::VIRTUAL_ARTIFACT_BYTES {
        let (err, v, size) = call_sized(
            &c,
            "artifact_chunk",
            json!({"offset": offset, "length": 65536}),
        )
        .await;
        assert!(!err, "{v}");
        max_result = max_result.max(size);
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(v["data_base64"].as_str().unwrap())
            .unwrap();
        hasher.update(&bytes);
        offset += bytes.len() as u64;
        calls += 1;
    }
    let total: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(total, tools::artifact_sha256());
    assert_eq!(calls, 512);
    assert!(
        max_result <= 256 * 1024,
        "chunk result {max_result} exceeds 256 KiB"
    );
    println!(
        "M0.2 loopback 32MiB transfer: calls={calls} elapsed_ms={} max_result_bytes={max_result}",
        start.elapsed().as_millis()
    );
    let _ = c.cancel().await;
}

#[tokio::test]
async fn every_identity_negative_is_denied_before_mcp() {
    let h = harness("").await;
    let good_ia = || assertion("human-a", "claude-code", |_| {});
    type Case<'a> = (&'a str, Vec<(&'a str, String)>, u16, &'a str);
    let cases: Vec<Case> = vec![
        ("no credentials", vec![], 401, "unauthenticated"),
        (
            "no assertion",
            vec![("authorization", format!("Bearer {}", upstream(|_| {})))],
            401,
            "unauthenticated",
        ),
        (
            "no bearer",
            vec![("x-user-jwt", good_ia())],
            401,
            "unauthenticated",
        ),
        (
            "wrong upstream audience",
            creds(
                upstream(|c| {
                    c.insert("aud".into(), json!(BACKEND));
                }),
                good_ia(),
            ),
            401,
            "unauthenticated",
        ),
        (
            "wrong tenant",
            creds(
                upstream(|c| {
                    c.insert("tid".into(), json!("other"));
                }),
                good_ia(),
            ),
            403,
            "forbidden",
        ),
        (
            "app-only token",
            creds(
                upstream(|c| {
                    c.remove("scp");
                    c.insert("roles".into(), json!(["AirsHistory.Read"]));
                }),
                good_ia(),
            ),
            403,
            "forbidden",
        ),
        (
            "wrong gateway client",
            creds(
                upstream(|c| {
                    c.insert("azp".into(), json!("other"));
                }),
                good_ia(),
            ),
            403,
            "forbidden",
        ),
        (
            "missing permission",
            creds(
                upstream(|c| {
                    c.insert("scp".into(), json!("User.Read"));
                }),
                good_ia(),
            ),
            403,
            "forbidden",
        ),
        (
            "wrong owner",
            creds(
                upstream(|c| {
                    c.insert("oid".into(), json!("other"));
                }),
                good_ia(),
            ),
            403,
            "forbidden",
        ),
        (
            "expired upstream",
            creds(
                upstream(|c| {
                    c.insert("exp".into(), json!(now() - 3600));
                }),
                good_ia(),
            ),
            401,
            "unauthenticated",
        ),
        (
            "assertion for another backend",
            creds(
                upstream(|_| {}),
                assertion("human-a", "claude-code", |c| {
                    c.insert(
                        "aud".into(),
                        json!("https://mcp-backend-bonbon.example.test/mcp"),
                    );
                }),
            ),
            403,
            "forbidden",
        ),
        (
            "assertion without aud",
            creds(
                upstream(|_| {}),
                assertion("human-a", "claude-code", |c| {
                    c.remove("aud");
                }),
            ),
            403,
            "forbidden",
        ),
        (
            "assertion wrong issuer",
            creds(
                upstream(|_| {}),
                assertion("human-a", "claude-code", |c| {
                    c.insert("iss".into(), json!("https://evil.test"));
                }),
            ),
            401,
            "unauthenticated",
        ),
        (
            "assertion lifetime too long",
            creds(
                upstream(|_| {}),
                assertion("human-a", "claude-code", |c| {
                    c.insert("exp".into(), json!(now() + 3600));
                }),
            ),
            401,
            "unauthenticated",
        ),
        (
            "unknown agent",
            creds(
                upstream(|_| {}),
                assertion("human-a", "unknown-agent", |_| {}),
            ),
            403,
            "forbidden",
        ),
        (
            "unknown caller",
            creds(
                upstream(|_| {}),
                assertion("human-b", "claude-code", |_| {}),
            ),
            403,
            "forbidden",
        ),
        (
            "missing agent",
            creds(
                upstream(|_| {}),
                assertion("human-a", "claude-code", |c| {
                    c.remove("client_id");
                }),
            ),
            403,
            "forbidden",
        ),
        (
            "wrong workspace",
            creds(
                upstream(|_| {}),
                assertion("human-a", "claude-code", |c| {
                    c.insert("workspace_id".into(), json!("ws-other"));
                }),
            ),
            403,
            "forbidden",
        ),
        (
            "wrong organization",
            creds(
                upstream(|_| {}),
                assertion("human-a", "claude-code", |c| {
                    c.insert("organisation_id".into(), json!("org-other"));
                }),
            ),
            403,
            "forbidden",
        ),
        (
            "allowlisted owner that the grant does not name",
            creds(
                upstream(|c| {
                    c.insert("oid".into(), json!(OWNER2));
                }),
                good_ia(),
            ),
            403,
            "forbidden",
        ),
        (
            "app identity token even with a delegated-looking scp",
            creds(
                upstream(|c| {
                    c.insert("idtyp".into(), json!("app"));
                }),
                good_ia(),
            ),
            403,
            "forbidden",
        ),
        (
            "upstream without nbf",
            creds(
                upstream(|c| {
                    c.remove("nbf");
                }),
                good_ia(),
            ),
            401,
            "unauthenticated",
        ),
        (
            "assertion without iat",
            creds(
                upstream(|_| {}),
                assertion("human-a", "claude-code", |c| {
                    c.remove("iat");
                }),
            ),
            401,
            "unauthenticated",
        ),
        (
            "assertion lifetime one second over the ceiling",
            creds(
                upstream(|_| {}),
                assertion("human-a", "claude-code", |c| {
                    let iat = now();
                    c.insert("iat".into(), json!(iat));
                    c.insert("exp".into(), json!(iat + 301));
                }),
            ),
            401,
            "unauthenticated",
        ),
        (
            "rogue key, known kid",
            creds(sign("rogue", "entra-1", &upstream_claims()), good_ia()),
            401,
            "unauthenticated",
        ),
        (
            "upstream signed by gateway key",
            creds(sign("gateway", "gw-1", &upstream_claims()), good_ia()),
            401,
            "unauthenticated",
        ),
        (
            "duplicate assertion header",
            vec![
                ("authorization", format!("Bearer {}", upstream(|_| {}))),
                ("x-user-jwt", good_ia()),
                ("x-user-jwt", good_ia()),
            ],
            401,
            "unauthenticated",
        ),
        (
            "non-bearer scheme",
            vec![
                ("authorization", format!("Basic {}", upstream(|_| {}))),
                ("x-user-jwt", good_ia()),
            ],
            401,
            "unauthenticated",
        ),
    ];
    for (name, headers, status, code) in cases {
        let (s, c) = raw(&h.url, &headers).await;
        assert_eq!((s, c.as_deref()), (status, Some(code)), "case: {name}");
    }
    // jku/HS256/alg confusion.
    let mut jku = Header::new(Algorithm::RS256);
    jku.kid = Some("entra-1".into());
    jku.jku = Some("https://evil.test/keys".into());
    let t = encode(&jku, &upstream_claims(), &key("entra")).unwrap();
    assert_eq!(
        raw(&h.url, &creds(t, good_ia())).await,
        (401, Some("unauthenticated".into()))
    );
    let hs = encode(
        &Header::new(Algorithm::HS256),
        &upstream_claims(),
        &EncodingKey::from_secret(b"k"),
    )
    .unwrap();
    assert_eq!(
        raw(&h.url, &creds(hs, good_ia())).await,
        (401, Some("unauthenticated".into()))
    );
    // The good pair is accepted (200) so the negatives above are not vacuous.
    assert_eq!(
        raw(&h.url, &creds(upstream(|_| {}), good_ia())).await.0,
        200
    );

    // Unknown kids force at most one refetch per minute per key source.
    let before = h.jwks_fetches.load(Ordering::SeqCst);
    for _ in 0..5 {
        let t = sign("entra", "rotated-kid", &upstream_claims());
        assert_eq!(raw(&h.url, &creds(t, good_ia())).await.0, 401);
    }
    assert!(
        h.jwks_fetches.load(Ordering::SeqCst) - before <= 1,
        "unknown kid caused a fetch storm"
    );
}

/// Raw MCP POST/GET/DELETE inside one session, so reauthorization is tested on
/// an *existing* session rather than on a fresh initialize.
async fn session_request(
    url: &str,
    method: reqwest::Method,
    session: Option<&str>,
    creds: &[(&str, String)],
    body: Option<&str>,
) -> (u16, Option<String>) {
    let client = reqwest::Client::new();
    let mut req = client
        .request(method, url)
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-06-18");
    if let Some(b) = body {
        req = req
            .header("content-type", "application/json")
            .body(b.to_owned());
    }
    if let Some(sid) = session {
        req = req.header("mcp-session-id", sid);
    }
    for (k, v) in creds {
        req = req.header(*k, v);
    }
    let resp = req.send().await.unwrap();
    let status = resp.status().as_u16();
    let sid = resp
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    (status, sid)
}

#[tokio::test]
async fn every_request_is_reauthorized_within_an_existing_session() {
    let h = harness("").await;
    let good = || {
        creds(
            upstream(|_| {}),
            assertion("human-a", "claude-code", |_| {}),
        )
    };
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#;
    let (status, sid) =
        session_request(&h.url, reqwest::Method::POST, None, &good(), Some(init)).await;
    assert_eq!(status, 200);
    let sid = sid.expect("legacy session id");
    let initialized = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    assert_eq!(
        session_request(
            &h.url,
            reqwest::Method::POST,
            Some(&sid),
            &good(),
            Some(initialized)
        )
        .await
        .0,
        202
    );
    let call = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"service_info","arguments":{}}}"#;
    assert_eq!(
        session_request(
            &h.url,
            reqwest::Method::POST,
            Some(&sid),
            &good(),
            Some(call)
        )
        .await
        .0,
        200
    );

    // Same live session, changed credentials: every request (POST, GET, DELETE)
    // is checked again for every credential failure.
    let expired = creds(
        upstream(|_| {}),
        assertion("human-a", "claude-code", |c| {
            c.insert("iat".into(), json!(now() - 400));
            c.insert("exp".into(), json!(now() - 300));
        }),
    );
    let unknown_agent = creds(
        upstream(|_| {}),
        assertion("human-a", "unknown-agent", |_| {}),
    );
    type Row<'a> = (&'a str, &'a [(&'a str, String)], u16);
    let matrix: [Row; 3] = [
        ("expired", &expired, 401),
        ("unknown agent", &unknown_agent, 403),
        ("none", &[], 401),
    ];
    for (label, bad, status) in matrix {
        for (method, body) in [
            (reqwest::Method::POST, Some(call)),
            (reqwest::Method::GET, None),
            (reqwest::Method::DELETE, None),
        ] {
            let got = session_request(&h.url, method.clone(), Some(&sid), bad, body)
                .await
                .0;
            assert_eq!(
                got, status,
                "{method} with {label} credentials on a live session"
            );
        }
    }
    // The good credentials still work on the same session afterwards.
    assert_eq!(
        session_request(
            &h.url,
            reqwest::Method::POST,
            Some(&sid),
            &good(),
            Some(call)
        )
        .await
        .0,
        200
    );
    let deleted = session_request(&h.url, reqwest::Method::DELETE, Some(&sid), &good(), None)
        .await
        .0;
    assert!(
        (200..300).contains(&deleted),
        "delete with good creds: {deleted}"
    );
}

#[tokio::test]
async fn token_scope_tier_is_intersected_with_the_grant_tier() {
    let h = harness("").await;
    // Positive control: Operate scope + operator grant listing the tool.
    let op = client(
        &h.url,
        scoped("AirsHistory.Operate"),
        assertion("human-a", "ops-agent-2", |_| {}),
    )
    .await;
    let (err, v) = call(&op, "operator_noop", json!({})).await;
    assert!(!err, "{v}");
    let _ = op.cancel().await;
    // Read-only token with the same operator grant: denied and not listed.
    let read = client(
        &h.url,
        scoped("AirsHistory.Read"),
        assertion("human-a", "ops-agent-2", |_| {}),
    )
    .await;
    let listed: Vec<String> = read
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    assert!(!listed.contains(&"operator_noop".to_owned()));
    let (err, v) = call(&read, "operator_noop", json!({})).await;
    assert!(err && v["code"] == "forbidden");
    let _ = read.cancel().await;
    // Admin scope cannot lift a Reader grant.
    let reader = client(
        &h.url,
        scoped("AirsHistory.Admin"),
        assertion("human-a", "claude-code", |_| {}),
    )
    .await;
    let (err, v) = call(&reader, "operator_noop", json!({})).await;
    assert!(err && v["code"] == "forbidden");
    let _ = reader.cancel().await;
    // Read token with an Administrator grant cannot even prepare previews.
    let admin_read = client(
        &h.url,
        scoped("AirsHistory.Read"),
        assertion("human-a", "airs-admin-cli", |_| {}),
    )
    .await;
    let (err, v) = call(&admin_read, "admin_preview", json!({"parameter": "x"})).await;
    assert!(err && v["code"] == "forbidden", "{v}");
    let _ = admin_read.cancel().await;
}

#[tokio::test]
async fn operator_needs_explicit_tool_and_admin_is_disabled_without_qualified_option() {
    let h = harness("").await;
    let op = client(
        &h.url,
        scoped("AirsHistory.Operate"),
        assertion("human-a", "ops-agent", |_| {}),
    )
    .await;
    // Operator tier, but `operator_noop` is not in its allowed-tool list.
    let (err, v) = call(&op, "operator_noop", json!({})).await;
    assert!(err && v["code"] == "forbidden");
    let _ = op.cancel().await;

    let admin = client(
        &h.url,
        scoped("AirsHistory.Admin"),
        assertion("human-a", "airs-admin-cli", |_| {}),
    )
    .await;
    let (err, p) = call(&admin, "admin_preview", json!({"parameter": "x"})).await;
    assert!(!err, "{p}");
    let (err, v) = call(
        &admin,
        "admin_execute",
        json!({"preview_id": p["preview_id"], "parameter": "x"}),
    )
    .await;
    assert!(
        err && v["code"] == "forbidden",
        "admin mutation must be disabled by default: {v}"
    );
    // The cached admin session cannot reach a lower-tier mutation either.
    let (err, v) = call(&admin, "operator_noop", json!({})).await;
    assert!(
        err && v["code"] == "forbidden",
        "lower-tier mutation via admin identity: {v}"
    );
    let _ = admin.cancel().await;
}

#[tokio::test]
async fn fresh_proof_is_bound_once_to_nonce_parameters_actor_and_freshness() {
    let h = harness("[admin]\nmode = \"fresh_proof\"\nproof_nonce_claim = \"airs_nonce\"\nproof_max_age_seconds = 120").await;
    let admin = |agent: &'static str, f: &dyn Fn(&mut Map<String, Value>)| {
        (
            scoped("AirsHistory.Admin"),
            assertion("human-a", agent, |c| f(c)),
        )
    };
    // A recent cached admin session without proof: preview works, execute and
    // lower-tier mutation are denied.
    let (b, ia) = admin("airs-admin-cli", &|c| {
        c.insert("auth_time".into(), json!(now()));
    });
    let cached = client(&h.url, b, ia).await;
    let (_, p) = call(&cached, "admin_preview", json!({"parameter": "x"})).await;
    let id = p["preview_id"].as_str().unwrap().to_owned();
    let nonce = p["proof_nonce"].as_str().unwrap().to_owned();
    assert!(
        call(
            &cached,
            "admin_execute",
            json!({"preview_id": id, "parameter": "x"})
        )
        .await
        .0
    );
    let (err, v) = call(&cached, "operator_noop", json!({})).await;
    assert!(err && v["code"] == "forbidden", "{v}");
    let _ = cached.cancel().await;

    // Stale authentication with the right nonce is denied.
    let n = nonce.clone();
    let (b, ia) = admin("airs-admin-cli", &move |c| {
        c.insert("airs_nonce".into(), json!(n));
        c.insert("auth_time".into(), json!(now() - 600));
    });
    let stale = client(&h.url, b, ia).await;
    assert!(
        call(
            &stale,
            "admin_execute",
            json!({"preview_id": id, "parameter": "x"})
        )
        .await
        .0
    );
    let _ = stale.cancel().await;

    // A different, fully authorized administrator holding a fresh proof with the
    // nonce cannot consume the still-unconsumed preview of another actor.
    let n = nonce.clone();
    let (b, ia) = admin("airs-admin-cli-2", &move |c| {
        c.insert("airs_nonce".into(), json!(n));
        c.insert("auth_time".into(), json!(now()));
    });
    let other = client(&h.url, b, ia).await;
    let (err, v) = call(
        &other,
        "admin_execute",
        json!({"preview_id": id, "parameter": "x"}),
    )
    .await;
    assert!(err && v["code"] == "conflict", "foreign actor: {v}");
    let _ = other.cancel().await;

    // The original actor with fresh, bound proof: changed parameters are refused;
    // exact ones succeed once; replay is refused.
    let n = nonce.clone();
    let (b, ia) = admin("airs-admin-cli", &move |c| {
        c.insert("airs_nonce".into(), json!(n));
        c.insert("auth_time".into(), json!(now()));
    });
    let fresh = client(&h.url, b, ia).await;
    assert!(
        call(
            &fresh,
            "admin_execute",
            json!({"preview_id": id, "parameter": "y"})
        )
        .await
        .0
    );
    let (err, v) = call(
        &fresh,
        "admin_execute",
        json!({"preview_id": id, "parameter": "x"}),
    )
    .await;
    assert!(!err, "{v}");
    let (err, v) = call(
        &fresh,
        "admin_execute",
        json!({"preview_id": id, "parameter": "x"}),
    )
    .await;
    assert!(
        err && v["code"] == "conflict",
        "proof replay must be refused"
    );
    // Even with valid proof present, lower-tier mutation stays refused.
    let (err, _) = call(&fresh, "operator_noop", json!({})).await;
    assert!(err);
    let _ = fresh.cancel().await;
}

#[tokio::test]
async fn a_valid_granted_principal_cannot_use_or_end_another_principals_session() {
    let h = harness("").await;
    let a = || {
        creds(
            upstream(|_| {}),
            assertion("human-a", "claude-code", |_| {}),
        )
    };
    let b = creds(
        scoped("AirsHistory.Operate"),
        assertion("human-a", "ops-agent", |_| {}),
    );
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#;
    let initialized = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    let call = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"service_info","arguments":{}}}"#;
    let (status, sid) =
        session_request(&h.url, reqwest::Method::POST, None, &a(), Some(init)).await;
    assert_eq!(status, 200);
    let sid = sid.expect("session id");
    assert_eq!(
        session_request(
            &h.url,
            reqwest::Method::POST,
            Some(&sid),
            &a(),
            Some(initialized)
        )
        .await
        .0,
        202
    );
    // B is valid and granted on its own, but not the session's owner.
    let (status, _) = session_request(&h.url, reqwest::Method::POST, None, &b, Some(init)).await;
    assert_eq!(status, 200, "positive control: B is a valid principal");
    for (method, body) in [
        (reqwest::Method::POST, Some(call)),
        (reqwest::Method::GET, None),
        (reqwest::Method::DELETE, None),
    ] {
        assert_eq!(
            session_request(&h.url, method.clone(), Some(&sid), &b, body)
                .await
                .0,
            403,
            "{method} by another principal"
        );
    }
    // The owner's session survived every denial; a refreshed token for the same
    // identity (new iat/exp) is the same owner.
    let refreshed = creds(
        upstream(|c| {
            c.insert("iat".into(), json!(now() + 1));
        }),
        assertion("human-a", "claude-code", |c| {
            c.insert("iat".into(), json!(now() + 1));
        }),
    );
    assert_eq!(
        session_request(
            &h.url,
            reqwest::Method::POST,
            Some(&sid),
            &refreshed,
            Some(call)
        )
        .await
        .0,
        200
    );
    assert_eq!(
        session_request(&h.url, reqwest::Method::GET, Some(&sid), &a(), None)
            .await
            .0,
        200
    );
    let ended = session_request(&h.url, reqwest::Method::DELETE, Some(&sid), &a(), None)
        .await
        .0;
    assert!((200..300).contains(&ended), "owner delete: {ended}");
    // The binding is gone with the session; an unknown or guessed ID is refused.
    assert_eq!(
        session_request(&h.url, reqwest::Method::POST, Some(&sid), &a(), Some(call))
            .await
            .0,
        404
    );
    assert_eq!(
        session_request(
            &h.url,
            reqwest::Method::GET,
            Some("guessed-session-id"),
            &a(),
            None
        )
        .await
        .0,
        404
    );
    // Those 404s come from the server's own session guard (before the transport
    // is reached), not only from rmcp's session lookup.
    for (method, id) in [
        (reqwest::Method::GET, "guessed-session-id"),
        (reqwest::Method::POST, sid.as_str()),
    ] {
        let mut req = reqwest::Client::new()
            .request(method, &h.url)
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json")
            .header("mcp-protocol-version", "2025-06-18")
            .header("mcp-session-id", id);
        for (k, v) in a() {
            req = req.header(k, v);
        }
        let body: Value = req
            .body(call)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap_or(Value::Null);
        assert_eq!(
            (body["code"].as_str(), body["reason"].as_str()),
            (Some("not_found"), Some("unknown session")),
            "{id}"
        );
    }
}

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#;

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn session_capacity_is_reserved_atomically_under_concurrency() {
    // Each creating request is held 300 ms after reserving, so all 32 overlap.
    let h = harness_with("max_sessions = 1\ntest_creation_delay_ms = 300", "").await;
    let a = || {
        creds(
            upstream(|_| {}),
            assertion("human-a", "claude-code", |_| {}),
        )
    };
    // 32 concurrent initializations against one free slot: exactly one succeeds.
    let mut tasks = Vec::new();
    for _ in 0..32 {
        let (url, c) = (h.url.clone(), a());
        tasks.push(tokio::spawn(async move {
            session_request(&url, reqwest::Method::POST, None, &c, Some(INIT)).await
        }));
    }
    let mut ok = Vec::new();
    let mut refused = 0;
    for t in tasks {
        match t.await.unwrap() {
            (200, Some(sid)) => ok.push(sid),
            (503, _) => refused += 1,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!((ok.len(), refused), (1, 31), "capacity overshoot");
    // Full: further creation is refused until the owner ends the session.
    assert_eq!(
        session_request(&h.url, reqwest::Method::POST, None, &a(), Some(INIT))
            .await
            .0,
        503
    );
    let ended = session_request(&h.url, reqwest::Method::DELETE, Some(&ok[0]), &a(), None)
        .await
        .0;
    assert!((200..300).contains(&ended));
    assert_eq!(
        session_request(&h.url, reqwest::Method::POST, None, &a(), Some(INIT))
            .await
            .0,
        200,
        "delete releases capacity"
    );
}

#[tokio::test]
async fn a_failed_creation_releases_its_reservation() {
    let h = harness_with("max_sessions = 1", "").await;
    let a = || {
        creds(
            upstream(|_| {}),
            assertion("human-a", "claude-code", |_| {}),
        )
    };
    // Rejected by the transport (malformed JSON-RPC): no session, no leaked slot.
    for _ in 0..5 {
        let (status, sid) =
            session_request(&h.url, reqwest::Method::POST, None, &a(), Some("{not json")).await;
        assert!(status >= 400 && sid.is_none(), "{status}");
    }
    let (status, sid) =
        session_request(&h.url, reqwest::Method::POST, None, &a(), Some(INIT)).await;
    assert_eq!(
        (status, sid.is_some()),
        (200, true),
        "reservations leaked by failed requests"
    );
}

const ROUTE_SECRET: &str = "m06-route-binding-test-secret-0123456789abcdef";

fn routed(bearer: String, ia: String, binding: Option<&str>) -> Vec<(&'static str, String)> {
    let mut h = creds(bearer, ia);
    if let Some(b) = binding {
        h.push(("x-airs-backend-binding", b.to_owned()));
    }
    h
}

/// Contract §2.2 resolution 3: with a route binding, an assertion without `aud` is
/// accepted only alongside the per-integration route secret and the backend's own
/// upstream `aud`; every other combination is denied.
#[tokio::test]
async fn route_binding_admits_only_the_gateway_route_for_this_backend() {
    let h = harness_full("", "", "[]", Some(ROUTE_SECRET)).await;
    let unbound = || {
        assertion("human-a", "claude-code", |c| {
            c.remove("aud");
        })
    };
    // The gateway's real shape: no `aud`, route secret present.
    assert_eq!(
        raw(
            &h.url,
            &routed(upstream(|_| {}), unbound(), Some(ROUTE_SECRET))
        )
        .await
        .0,
        200
    );
    // Missing, wrong, truncated and case-changed secrets are denied (403).
    for bad in [
        None,
        Some("wrong"),
        Some(&ROUTE_SECRET[..40]),
        Some("M06-ROUTE-BINDING-TEST-SECRET-0123456789ABCDEF"),
    ] {
        let (status, code) = raw(&h.url, &routed(upstream(|_| {}), unbound(), bad)).await;
        assert_eq!(
            (status, code.as_deref()),
            (403, Some("forbidden")),
            "{bad:?}"
        );
    }
    // A duplicated binding header is ambiguous: refused before verification (401).
    let mut dup = routed(upstream(|_| {}), unbound(), Some(ROUTE_SECRET));
    dup.push(("x-airs-backend-binding", ROUTE_SECRET.to_owned()));
    assert_eq!(raw(&h.url, &dup).await.0, 401);
    // Replay from another backend: the route secret alone does not admit a request
    // whose upstream token was issued for a different API (`aud`): 401, as for any
    // upstream token not issued to this backend.
    let other_api = upstream(|c| {
        c.insert("aud".into(), json!("00000000-0000-4000-8000-0000000000ff"));
    });
    assert_eq!(
        raw(&h.url, &routed(other_api, unbound(), Some(ROUTE_SECRET)))
            .await
            .0,
        401
    );
    // Grants still apply: an ungranted agent with the right route is denied.
    let stranger = assertion("human-a", "unknown-agent", |c| {
        c.remove("aud");
    });
    assert_eq!(
        raw(
            &h.url,
            &routed(upstream(|_| {}), stranger, Some(ROUTE_SECRET))
        )
        .await
        .0,
        403
    );
}

/// When audiences are configured as well, the signed `aud` is still enforced.
#[tokio::test]
async fn route_binding_with_audiences_still_requires_signed_aud() {
    let h = harness_full("", "", &format!("[\"{BACKEND}\"]"), Some(ROUTE_SECRET)).await;
    let good = assertion("human-a", "claude-code", |_| {});
    assert_eq!(
        raw(&h.url, &routed(upstream(|_| {}), good, Some(ROUTE_SECRET)))
            .await
            .0,
        200
    );
    let unbound = assertion("human-a", "claude-code", |c| {
        c.remove("aud");
    });
    assert_eq!(
        raw(
            &h.url,
            &routed(upstream(|_| {}), unbound, Some(ROUTE_SECRET))
        )
        .await
        .0,
        403
    );
}

/// Without a route binding the assertion `aud` stays mandatory: the deploy
/// template (empty audiences + route binding) becomes a configuration error when the
/// route binding is removed, so no configuration accepts an unbound request.
#[test]
fn unbound_configurations_are_refused() {
    let base = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../deploy/m0.6/spike-config.toml"
    ))
    .unwrap();
    const SECTION: &str = "[route_binding]\nheader = \"x-airs-backend-binding\"\nsecret_file = \"/etc/airs-mcp-spike/route/binding\"\n";
    assert!(base.contains(SECTION) && base.contains("audiences = []"));
    assert!(config::parse(&base).is_ok(), "template parses");
    assert!(
        config::parse(&base.replace(SECTION, "")).is_err(),
        "empty audiences without a route binding"
    );
    for header in [
        "x-user-jwt",
        "authorization",
        "X-Airs-Backend-Binding",
        "",
        "x airs",
    ] {
        let t = base.replace(
            "header = \"x-airs-backend-binding\"",
            &format!("header = \"{header}\""),
        );
        assert!(config::parse(&t).is_err(), "route header {header:?}");
    }
}

/// A short or non-printable route secret is refused at start.
#[test]
fn weak_route_secrets_are_refused() {
    let dir = std::env::temp_dir().join(format!("airs-route-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
    let base = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../deploy/m0.6/spike-config.toml"
    ))
    .unwrap();
    for (i, secret) in ["short", "has a space inside it but is long enough ok", ""]
        .iter()
        .enumerate()
    {
        let f = dir.join(format!("s{i}"));
        std::fs::write(&f, secret).unwrap();
        let t = base.replace(
            "/etc/airs-mcp-spike/route/binding",
            &f.display().to_string(),
        );
        let cfg = Arc::new(config::parse(&t).expect("template parses"));
        assert!(
            airs_mcp_spike::auth::Verifier::new(cfg).is_err(),
            "{secret:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// POST one JSON-RPC message in stateless mode; returns status, any session ID
/// header in the response, and the parsed JSON body.
async fn stateless_post(
    url: &str,
    session: Option<&str>,
    creds: &[(&str, String)],
    body: &str,
) -> (u16, Option<String>, Value) {
    let mut req = reqwest::Client::new()
        .post(url)
        .header("accept", "application/json, text/event-stream")
        .header("content-type", "application/json")
        .header("mcp-protocol-version", "2025-06-18")
        .body(body.to_owned());
    if let Some(sid) = session {
        req = req.header("mcp-session-id", sid);
    }
    for (k, v) in creds {
        req = req.header(*k, v);
    }
    let resp = req.send().await.unwrap();
    let status = resp.status().as_u16();
    let sid = resp
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let text = resp.text().await.unwrap_or_default();
    // JSON responses, or an SSE fallback whose `data:` line carries the message.
    let json = serde_json::from_str(&text).unwrap_or_else(|_| {
        text.lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok())
            .next_back()
            .unwrap_or(Value::Null)
    });
    (status, sid, json)
}

const CALL_INFO: &str = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"service_info","arguments":{}}}"#;

/// Contract §2.3 (M0.6 step 7.6): stateless mode issues no session, ignores a
/// forwarded session ID from another agent, refuses GET/DELETE, and still
/// authorizes every request on its own tokens.
#[tokio::test]
async fn stateless_mode_has_no_sessions_to_share_between_agents() {
    let h = harness_with("sessions = \"stateless\"", "").await;
    let a = || {
        creds(
            upstream(|_| {}),
            assertion("human-a", "claude-code", |_| {}),
        )
    };
    let b = || {
        creds(
            scoped("AirsHistory.Operate"),
            assertion("human-a", "ops-agent", |_| {}),
        )
    };
    // Initialize succeeds and issues no session ID.
    let (status, sid, _) = stateless_post(&h.url, None, &a(), INIT).await;
    assert_eq!(status, 200);
    assert_eq!(sid, None, "stateless mode must not issue Mcp-Session-Id");
    // A tool call needs no prior initialize, and a session ID the gateway forwards
    // from another agent changes nothing: each principal gets its own answer.
    for (who, c, tier) in [("a", a(), "Reader"), ("b", b(), "Operator")] {
        for forwarded in [None, Some("session-from-another-agent")] {
            let (status, sid, body) = stateless_post(&h.url, forwarded, &c, CALL_INFO).await;
            assert_eq!(status, 200, "{who} {forwarded:?}");
            assert_eq!(sid, None);
            let sc = &body["result"]["structuredContent"];
            assert_eq!(sc["principal"]["effective_tier"], tier, "{who}: {body}");
        }
    }
    // Authorization still runs per request: a Reader cannot call an Operator tool,
    // and a missing assertion is unauthenticated.
    let noop = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"operator_noop","arguments":{}}}"#;
    let (status, _, body) = stateless_post(&h.url, Some("x"), &a(), noop).await;
    assert_eq!(status, 200);
    assert_eq!(body["result"]["isError"], true, "{body}");
    assert_eq!(body["result"]["structuredContent"]["code"], "forbidden");
    let bare = vec![("authorization", format!("Bearer {}", upstream(|_| {})))];
    assert_eq!(stateless_post(&h.url, None, &bare, CALL_INFO).await.0, 401);
    // There is no session to stream from or end.
    for method in [reqwest::Method::GET, reqwest::Method::DELETE] {
        let mut req = reqwest::Client::new()
            .request(method.clone(), &h.url)
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-06-18")
            .header("mcp-session-id", "session-from-another-agent");
        for (k, v) in a() {
            req = req.header(k, v);
        }
        assert_eq!(req.send().await.unwrap().status().as_u16(), 405, "{method}");
    }
}

/// Two agents of one user, interleaved and concurrent, through rmcp's own client
/// in stateless mode: every call succeeds with the caller's own tier.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stateless_mode_serves_concurrent_agents_of_one_user() {
    let h = harness_with("sessions = \"stateless\"", "").await;
    let reader = client(
        &h.url,
        upstream(|_| {}),
        assertion("human-a", "claude-code", |_| {}),
    )
    .await;
    let operator = client(
        &h.url,
        scoped("AirsHistory.Operate"),
        assertion("human-a", "ops-agent", |_| {}),
    )
    .await;
    let call = |c: &rmcp::service::RunningService<rmcp::RoleClient, ClientConfig>| {
        let peer = c.peer().clone();
        async move {
            let r = peer
                .call_tool(CallToolRequestParams::new("service_info"))
                .await
                .expect("call");
            r.structured_content.expect("structured")["principal"]["effective_tier"]
                .as_str()
                .unwrap()
                .to_owned()
        }
    };
    let mut tasks = Vec::new();
    for i in 0..20 {
        let fut = if i % 2 == 0 {
            call(&reader)
        } else {
            call(&operator)
        };
        tasks.push(tokio::spawn(async move { (i, fut.await) }));
    }
    for t in tasks {
        let (i, tier) = t.await.unwrap();
        assert_eq!(
            tier,
            if i % 2 == 0 { "Reader" } else { "Operator" },
            "call {i}"
        );
    }
    let _ = reader.cancel().await;
    let _ = operator.cancel().await;
}

/// Contract §2.2 resolution 3 as measured in M0.6: the gateway signs the
/// organisation ID as `aud` for every backend and forwards no route header. The
/// only per-backend binding is the upstream token (`aud` = this API, `azp` = this
/// integration's gateway client). An assertion minted for another backend in the
/// same org is valid here only with this backend's upstream token.
#[tokio::test]
async fn org_wide_assertion_audience_is_bound_only_by_the_upstream_token() {
    let h = harness_full("sessions = \"stateless\"", "", r#"["org-test"]"#, None).await;
    let org_assertion = || {
        assertion("human-a", "claude-code", |c| {
            c.insert("aud".into(), json!("org-test"));
        })
    };
    // Positive control: this backend's upstream token + the org-wide assertion.
    let (status, _, body) = stateless_post(
        &h.url,
        None,
        &creds(upstream(|_| {}), org_assertion()),
        CALL_INFO,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["result"]["structuredContent"]["principal"]["effective_tier"],
        "Reader"
    );
    // Replays of another backend's traffic: its assertion is identical in form, but
    // its upstream token names another API or another gateway client.
    for (case, expected, token) in [
        (
            "another backend's API audience",
            401,
            upstream(|c| {
                c.insert("aud".into(), json!("00000000-0000-4000-8000-0000000000ff"));
            }),
        ),
        (
            "another integration's gateway client",
            403,
            upstream(|c| {
                c.insert("azp".into(), json!("00000000-0000-4000-8000-0000000000fe"));
            }),
        ),
        (
            "another tenant",
            403,
            upstream(|c| {
                c.insert("tid".into(), json!("00000000-0000-4000-8000-0000000000fd"));
            }),
        ),
    ] {
        let (status, _, _) =
            stateless_post(&h.url, None, &creds(token, org_assertion()), CALL_INFO).await;
        assert_eq!(status, expected, "{case}");
    }
    // The assertion alone, or with an app-only token, is never enough.
    let bare = vec![("x-user-jwt", org_assertion())];
    assert_eq!(stateless_post(&h.url, None, &bare, CALL_INFO).await.0, 401);
    // A different organisation's assertion is refused even with this backend's token.
    let other_org = assertion("human-a", "claude-code", |c| {
        c.insert("aud".into(), json!("org-other"));
    });
    assert_ne!(
        stateless_post(&h.url, None, &creds(upstream(|_| {}), other_org), CALL_INFO)
            .await
            .0,
        200
    );
}

/// `service_info` reports only what the principal can call now: tier ∩ grant
/// tools. `ops-agent-2` holds an Operator grant listing `operator_noop`, but a
/// Read-scoped token caps it at Reader (M0.6 finding).
#[tokio::test]
async fn service_info_lists_only_callable_tools() {
    let h = harness_with("sessions = \"stateless\"", "").await;
    let capped = creds(
        scoped("AirsHistory.Read"),
        assertion("human-a", "ops-agent-2", |_| {}),
    );
    let (status, _, body) = stateless_post(&h.url, None, &capped, CALL_INFO).await;
    assert_eq!(status, 200, "{body}");
    let p = &body["result"]["structuredContent"]["principal"];
    assert_eq!(p["grant_tier"], "Operator");
    assert_eq!(p["effective_tier"], "Reader");
    assert_eq!(p["tools"], json!(["service_info"]), "{p}");
    let full = creds(
        scoped("AirsHistory.Operate"),
        assertion("human-a", "ops-agent-2", |_| {}),
    );
    let (_, _, body) = stateless_post(&h.url, None, &full, CALL_INFO).await;
    assert_eq!(
        body["result"]["structuredContent"]["principal"]["tools"],
        json!(["service_info", "operator_noop"])
    );
}

#[tokio::test]
async fn administrator_listing_omits_lower_tier_mutations_it_cannot_call() {
    // The airs-admin-cli grant names operator_noop, but an administrative identity
    // mutates only through admin_execute, so neither service_info nor tools/list may
    // advertise it, and calling it is still refused (review 9 minor). With
    // administrative mode disabled admin_execute is not listed either (review 10 minor 2).
    let h = harness_with("sessions = \"stateless\"", "").await;
    let admin = creds(
        scoped("AirsHistory.Admin"),
        assertion("human-a", "airs-admin-cli", |_| {}),
    );
    let (status, _, body) = stateless_post(&h.url, None, &admin, CALL_INFO).await;
    assert_eq!(status, 200, "{body}");
    let tools = &body["result"]["structuredContent"]["principal"]["tools"];
    assert_eq!(tools, &json!(["service_info", "admin_preview"]), "{tools}");
    let list = r#"{"jsonrpc":"2.0","id":7,"method":"tools/list","params":{}}"#;
    let (status, _, body) = stateless_post(&h.url, None, &admin, list).await;
    assert_eq!(status, 200, "{body}");
    let listed: Vec<&str> = body["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().expect("name"))
        .collect();
    assert_eq!(listed, ["service_info", "admin_preview"]);
    let call = r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"operator_noop","arguments":{}}}"#;
    let (_, _, body) = stateless_post(&h.url, None, &admin, call).await;
    assert_eq!(body["result"]["isError"], true, "{body}");
    assert_eq!(body["result"]["structuredContent"]["code"], "forbidden"); // Review 10 minor 2: with administrative mode disabled, admin_execute can never run,
    // so it is not listed either; called by name it still fails closed.
    let call = r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"admin_execute","arguments":{}}}"#;
    let (_, _, body) = stateless_post(&h.url, None, &admin, call).await;
    assert_eq!(body["result"]["isError"], true, "{body}");
    assert_eq!(body["result"]["structuredContent"]["code"], "forbidden");
}
