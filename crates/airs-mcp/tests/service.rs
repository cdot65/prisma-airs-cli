//! M2 loopback acceptance (plan M2 exit gate): the real service router behind fixture
//! JWKS issuers, driven by rmcp's Streamable HTTP client and raw HTTP, over a real
//! history archive. Loopback only: live gateway proof is M8.

use std::collections::HashMap;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use airs_mcp::{config, run, storage, tools};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientConfig},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Map, Value, json};

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
    shared: Arc<run::Shared>,
    /// The configuration text, so a test can reload a variant of it.
    text: String,
    history: PathBuf,
    storage: Arc<tools::Storage>,
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

const TENANT: &str = "svc-test";
const SCAN_ENDPOINTS: [&str; 3] = [
    "https://api.example.test/aisec",
    "https://api.example.test/aisec",
    "https://auth.example.test/oauth2/access_token",
];

fn config_text(idp: &str, data: &std::path::Path, extra: &str) -> String {
    format!(
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
[upstream]
issuer = "https://login.microsoftonline.com/{TID}/v2.0"
audiences = ["{API}"]
tenant_id = "{TID}"
jwks_url = "http://{idp}/entra/keys"
algorithms = ["RS256"]
accepted_permissions = ["AirsHistory.Read", "AirsHistory.Operate", "AirsHistory.Admin"]
allowed_client_ids = ["{GW_CLIENT}"]
owner_object_ids = ["{OWNER}", "{OWNER2}"]
[assertion]
header = "x-user-jwt"
issuer = "https://mcp.example.test"
audiences = ["{BACKEND}", "org-test"]
algorithms = ["RS256"]
jwks_url = "http://{idp}/gw/jwks.json"
max_lifetime_seconds = 300
workspace_ids = ["ws-test"]
organization_ids = ["org-test"]
{extra}
[airs]
tsg = "100"
management_endpoint = "{m}"
dashboard_endpoint = "{d}"
token_endpoint = "{tok}"
management_client_id_file = "/run/airs-mcp/secrets/mgmt-client-id"
management_client_secret_file = "/run/airs-mcp/secrets/mgmt-client-secret"
management_revision = "00000000-0000-4000-8000-0000000000e1"
[[grants]]
subject = "human-a"
client_id = "claude-code"
owner_object_id = "{OWNER}"
permission = "reader"
tools = ["service_info", "history_status", "history_coverage", "jobs_list", "jobs_get", "reports_list"]
label = "claude-code-reader"
[[grants]]
subject = "human-a"
client_id = "ops-agent"
owner_object_id = "{OWNER}"
permission = "operator"
tools = ["service_info", "history_status", "collection_start", "jobs_list", "jobs_get", "jobs_cancel", "reports_create"]
[[grants]]
subject = "human-a"
client_id = "claude-code-2"
owner_object_id = "{OWNER}"
permission = "reader"
tools = ["service_info", "history_coverage"]
[[grants]]
subject = "human-a"
client_id = "status-only"
owner_object_id = "{OWNER}"
permission = "reader"
tools = ["service_info"]
[[grants]]
subject = "human-a"
client_id = "airs-admin-cli"
owner_object_id = "{OWNER}"
permission = "administrator"
tools = ["service_info", "history_status", "lifecycle_preview", "lifecycle_execute"]
"#,
        data = data.display(),
        m = SCAN_ENDPOINTS[0],
        d = SCAN_ENDPOINTS[1],
        tok = SCAN_ENDPOINTS[2],
    )
}

/// `[service]` for the default harness.
const SERVICE: &str = "[service]\ntenant = \"svc-test\"";

async fn harness() -> Harness {
    harness_with(SERVICE).await
}

async fn harness_with(extra: &str) -> Harness {
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
    let dir = std::env::temp_dir().join(format!("airs-mcp-service-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let text = config_text(&idp_addr, &dir, extra);
    let cfg = config::parse(&text).expect("fixture config");
    let admitted = storage::admit(&cfg.server.data_dir).unwrap();
    let history = storage::private_child(&admitted, "history").unwrap();
    let control = storage::private_child(&admitted, "control").unwrap();
    let seed = storage::control_seed(&control).unwrap();
    let control_store = airs_mcp::control::Control::open(
        &control.path,
        TENANT,
        &seed.epoch,
        &seed.cursor_key,
        airs_runtime_core::time::now_ms(),
    )
    .unwrap();
    let owner = storage::OwnerRecord {
        owner_id: "test".into(),
        pod: "test".into(),
        node: "test".into(),
        pid: 0,
        acquired_at_unix: 0,
    };
    let store = Arc::new(tools::Storage::new(
        admitted,
        owner,
        history.path.clone(),
        control_store,
    ));
    let shared = run::Shared::new(run::Live::new(cfg).unwrap());
    let addr = serve(run::router(
        shared.clone(),
        tools::Service {
            storage: Some(store.clone()),
        },
    ))
    .await;
    Harness {
        storage: store,
        url: format!("http://{addr}/mcp"),
        jwks_fetches: fetches,
        shared,
        text,
        history: history.path.clone(),
        _dir: tempdir::Dir(dir),
    }
}

fn scoped(scp: &'static str) -> String {
    upstream(move |c| {
        c.insert("scp".into(), json!(scp));
    })
}

/// Raw initialize POST: returns HTTP status and the JSON error code if any.
async fn raw(url: &str, headers: &[(&str, String)]) -> (u16, Option<String>) {
    let (status, _, body) = stateless_post(url, None, headers, INIT).await;
    (
        status,
        body.get("code").and_then(Value::as_str).map(str::to_owned),
    )
}

fn creds(bearer: String, ia: String) -> Vec<(&'static str, String)> {
    vec![
        ("authorization", format!("Bearer {bearer}")),
        ("x-user-jwt", ia),
    ]
}

fn reader() -> Vec<(&'static str, String)> {
    creds(
        upstream(|_| {}),
        assertion("human-a", "claude-code", |_| {}),
    )
}

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#;

/// A `tools/call` body with the versioned envelope around `params`.
fn call_body(tool: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": tool,
        "arguments": {"schema_version": 1, "tenant": TENANT, "params": params}}})
    .to_string()
}

/// A tool call: `(is_error, structuredContent)`.
async fn tool(h: &Harness, who: &[(&str, String)], name: &str, params: Value) -> (bool, Value) {
    let (status, _, body) = stateless_post(&h.url, None, who, &call_body(name, params)).await;
    assert_eq!(status, 200, "{body}");
    (
        body["result"]["isError"] == true,
        body["result"]["structuredContent"].clone(),
    )
}

/// Seed the archive with the synthetic legacy evidence fixture (explicit import).
fn seed_archive(h: &Harness) {
    seed_archive_with(h, &["sessions", "scan-results"]);
}

/// Seed with the fixture's evidence under these provider source names (one import).
fn seed_archive_with(h: &Harness, sources: &[&str]) {
    let cloud =
        airs_history_store::identity::Cloud::from_parts("100", SCAN_ENDPOINTS.map(str::to_owned))
            .unwrap();
    // The synthetic fixture with its one source renamed to the collector's provider
    // sources, so the import records coverage the service maps (contract §4.3).
    let text = include_str!("../../../tests/fixtures/signal/evidence.json")
        .replace("\"synthetic reports\"", "\"sessions\"");
    let mut evidence: airs_runtime_core::signal::evidence::Evidence =
        serde_json::from_str(&text).unwrap();
    let template = evidence.sources[0].clone();
    evidence.sources = sources
        .iter()
        .map(|name| {
            let mut s = template.clone();
            s.name = (*name).into();
            s
        })
        .collect();
    evidence.collection_id = uuid::Uuid::new_v4().to_string();
    evidence.seal().unwrap();
    let file = h
        .history
        .parent()
        .unwrap()
        .join(format!("seed-evidence-{}.json", uuid::Uuid::new_v4()));
    std::fs::write(&file, serde_json::to_vec(&evidence).unwrap()).unwrap();
    let original = airs_history_store::repository::read_legacy(&file).unwrap();
    airs_history_store::repository::import_evidence(
        &h.history,
        &cloud,
        &original,
        &airs_runtime_core::error::SecretSet::default(),
        &airs_runtime_core::signal::privacy::PayloadSpans::default(),
        &scm::Session::new(),
    )
    .unwrap();
}

/// Validate `value` against a frozen result schema (`$ref`s already inlined by the
/// catalogue): types, `const`, `enum`, `pattern`, bounds, required and closed objects.
fn validate(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let fail = |why: &str| Err(format!("{path}: {why}: {value}"));
    if let Some(any) = schema.get("anyOf").and_then(Value::as_array) {
        return if any.iter().any(|s| validate(s, value, path).is_ok()) {
            Ok(())
        } else {
            fail("no anyOf branch")
        };
    }
    if schema.get("const").is_some_and(|c| c != value) {
        return fail("const");
    }
    if schema
        .get("enum")
        .and_then(Value::as_array)
        .is_some_and(|e| !e.contains(value))
    {
        return fail("enum");
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => {
            let Some(map) = value.as_object() else {
                return fail("object");
            };
            let props = schema["properties"]
                .as_object()
                .cloned()
                .unwrap_or_default();
            for r in schema["required"].as_array().into_iter().flatten() {
                if !map.contains_key(r.as_str().unwrap()) {
                    return fail("required");
                }
            }
            for (k, v) in map {
                match props.get(k) {
                    Some(s) => validate(s, v, &format!("{path}.{k}"))?,
                    None if schema["additionalProperties"] == false => return fail("closed"),
                    None => {}
                }
            }
        }
        Some("array") => {
            let Some(items) = value.as_array() else {
                return fail("array");
            };
            if schema["maxItems"]
                .as_u64()
                .is_some_and(|max| items.len() as u64 > max)
            {
                return fail("maxItems");
            }
            for (i, item) in items.iter().enumerate() {
                validate(&schema["items"], item, &format!("{path}[{i}]"))?;
            }
        }
        Some("string") => {
            let Some(s) = value.as_str() else {
                return fail("string");
            };
            if schema["pattern"]
                .as_str()
                .is_some_and(|p| !regex::Regex::new(p).unwrap().is_match(s))
            {
                return fail("pattern");
            }
            if schema["maxLength"]
                .as_u64()
                .is_some_and(|m| s.chars().count() as u64 > m)
            {
                return fail("maxLength");
            }
        }
        Some("integer") => {
            let Some(n) = value.as_i64() else {
                return fail("integer");
            };
            if schema["minimum"].as_i64().is_some_and(|m| n < m) {
                return fail("minimum");
            }
        }
        Some("boolean") if !value.is_boolean() => return fail("boolean"),
        Some("null") if !value.is_null() => return fail("null"),
        _ => {}
    }
    Ok(())
}

fn assert_frozen(tool: &str, value: &Value) {
    let schema = &airs_mcp::catalogue::get(tool).unwrap().result_schema;
    validate(schema, value, tool).unwrap_or_else(|e| panic!("{e}"));
}

#[tokio::test]
async fn every_identity_negative_is_denied_before_mcp() {
    let h = harness().await;
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

/// `service_info` has the frozen shape and lists exactly the tools this principal can
/// call now: tier ∩ grant ∩ implemented (`reports_list` is implemented from M6).
#[tokio::test]
async fn service_info_is_frozen_and_lists_only_callable_tools() {
    let h = harness().await;
    let (err, info) = tool(&h, &reader(), "service_info", json!({})).await;
    assert!(!err, "{info}");
    assert_frozen("service_info", &info);
    assert_eq!(info["effective_tier"], "reader");
    assert_eq!(info["service_tenant"], TENANT);
    assert_eq!(info["health"], "ok");
    assert_eq!(info["server"], "airs-mcp");
    assert_eq!(info["negotiated_protocol"], "2025-06-18");
    assert_eq!(
        info["tools"],
        json!([
            "history_coverage",
            "history_status",
            "jobs_get",
            "jobs_list",
            "reports_list",
            "service_info"
        ])
    );
    let list = r#"{"jsonrpc":"2.0","id":7,"method":"tools/list","params":{}}"#;
    let (_, _, body) = stateless_post(&h.url, None, &reader(), list).await;
    let listed: Vec<&str> = body["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        listed,
        [
            "history_coverage",
            "history_status",
            "jobs_get",
            "jobs_list",
            "reports_list",
            "service_info"
        ]
    );
    for t in body["result"]["tools"].as_array().unwrap() {
        assert!(
            !t["inputSchema"].to_string().contains("$ref"),
            "self-contained input schema"
        );
        assert_eq!(t["inputSchema"]["additionalProperties"], false);
    }
    // A Read-scoped token caps an Operator grant at Reader.
    let capped = creds(
        scoped("AirsHistory.Read"),
        assertion("human-a", "ops-agent", |_| {}),
    );
    let (_, info) = tool(&h, &capped, "service_info", json!({})).await;
    assert_eq!(info["effective_tier"], "reader");
    assert_eq!(
        info["tools"],
        json!(["history_status", "jobs_get", "jobs_list", "service_info"])
    );
}

/// The dispatch order is fixed and every refusal has its compiled code: unknown →
/// `not_found`, over-tier or ungranted → `forbidden`, granted but unimplemented →
/// `invalid_request`. A denial never depends on the order of the grant's tool list.
#[tokio::test]
async fn tool_refusals_follow_the_dispatch_order() {
    let h = harness().await;
    let operator = creds(
        scoped("AirsHistory.Operate"),
        assertion("human-a", "ops-agent", |_| {}),
    );
    let status_only = creds(
        upstream(|_| {}),
        assertion("human-a", "status-only", |_| {}),
    );
    let admin = creds(
        scoped("AirsHistory.Admin"),
        assertion("human-a", "airs-admin-cli", |_| {}),
    );
    for (who, creds, name, code) in [
        ("reader", reader(), "no_such_tool", "not_found"),
        ("reader", reader(), "collection_start", "forbidden"),
        ("reader", reader(), "lifecycle_execute", "forbidden"),
        (
            "status-only",
            status_only.clone(),
            "history_status",
            "forbidden",
        ),
        (
            "operator",
            operator.clone(),
            "reports_create",
            "invalid_request",
        ),
        (
            "operator",
            operator.clone(),
            "history_coverage",
            "forbidden",
        ),
        (
            "admin",
            admin.clone(),
            "lifecycle_execute",
            "invalid_request",
        ),
    ] {
        let (err, v) = tool(&h, &creds, name, json!({})).await;
        assert!(err, "{who} {name}: {v}");
        assert_eq!(v["code"], code, "{who} {name}: {v}");
        assert_eq!(v["schema_version"], 1);
    }
    let (err, v) = tool(&h, &admin, "history_status", json!({})).await;
    assert!(!err, "an administrator reads: {v}");
    let (err, v) = tool(&h, &status_only, "service_info", json!({})).await;
    assert!(!err, "{v}");
    assert_eq!(v["tools"], json!(["service_info"]));
}

/// Contract §4.2: strict envelope and the served tenant, checked before any effect.
#[tokio::test]
async fn the_envelope_and_tenant_are_checked_before_any_read() {
    let h = harness().await;
    let send = |arguments: Value| {
        let body = json!({"jsonrpc":"2.0","id":3,"method":"tools/call",
            "params":{"name":"history_status","arguments":arguments}})
        .to_string();
        let url = h.url.clone();
        async move {
            let (status, _, body) = stateless_post(&url, None, &reader(), &body).await;
            assert_eq!(status, 200);
            body["result"]["structuredContent"]["code"].clone()
        }
    };
    for (case, arguments, code) in [
        (
            "another tenant",
            json!({"schema_version":1,"tenant":"other-tenant","params":{}}),
            "forbidden",
        ),
        (
            "schema version 2",
            json!({"schema_version":2,"tenant":TENANT,"params":{}}),
            "unsupported_version",
        ),
        (
            "unknown top-level field",
            json!({"schema_version":1,"tenant":TENANT,"params":{},"sql":"x"}),
            "invalid_request",
        ),
        (
            "unknown parameter",
            json!({"schema_version":1,"tenant":TENANT,"params":{"path":"/etc"}}),
            "invalid_request",
        ),
        (
            "idempotency key on a read",
            json!({"schema_version":1,"tenant":TENANT,"idempotency_key":"k".repeat(20),"expected_epoch":"00000000","params":{}}),
            "invalid_request",
        ),
        (
            "invalid tenant shape",
            json!({"schema_version":1,"tenant":"Upper Case","params":{}}),
            "invalid_request",
        ),
        (
            "missing params",
            json!({"schema_version":1,"tenant":TENANT}),
            "invalid_request",
        ),
        ("no envelope", json!({}), "invalid_request"),
    ] {
        assert_eq!(send(arguments).await, code, "{case}");
    }
}

/// `history_status` reports an absent archive without creating one, then the imported
/// archive's schema; both results have the frozen shape.
#[tokio::test]
async fn history_status_reports_absent_then_present_archives() {
    let h = harness().await;
    let (err, v) = tool(&h, &reader(), "history_status", json!({})).await;
    assert!(!err, "{v}");
    assert_frozen("history_status", &v);
    assert_eq!(
        (v["archive_present"].clone(), v["schema"].clone()),
        (json!(false), Value::Null)
    );
    assert_eq!(
        std::fs::read_dir(&h.history).unwrap().count(),
        0,
        "status creates nothing"
    );
    seed_archive(&h);
    let (err, v) = tool(&h, &reader(), "history_status", json!({})).await;
    assert!(!err, "{v}");
    assert_frozen("history_status", &v);
    assert_eq!(v["archive_present"], true);
    assert!(v["schema"].as_u64().is_some_and(|s| s >= 1), "{v}");
}

fn range(since: &str, until: &str) -> Value {
    json!({"range": {"since": since, "until": until}})
}

/// `history_coverage`: a missing archive is `not_found`; the imported archive's coverage
/// has the frozen shape; ranges and filters are validated; pages are bounded and
/// chained by signed cursors that bind their scope.
#[tokio::test]
async fn history_coverage_is_bounded_paged_and_scope_bound() {
    let h = harness().await;
    let all = range("1970-01-01T00:00:00Z", "1970-03-01T00:00:00Z");
    let (err, v) = tool(&h, &reader(), "history_coverage", all.clone()).await;
    assert!(err && v["code"] == "not_found", "{v}");
    seed_archive(&h);
    let wide = range("2020-01-01T00:00:00Z", "2020-03-31T00:00:00Z");
    for (case, params) in [
        (
            "over 90 days",
            range("2026-01-01T00:00:00Z", "2026-04-02T00:00:00Z"),
        ),
        (
            "reversed",
            range("2026-01-02T00:00:00Z", "2026-01-01T00:00:00Z"),
        ),
        (
            "offset time",
            range("2026-01-01T00:00:00+00:00", "2026-01-02T00:00:00Z"),
        ),
        (
            "page limit 0",
            json!({"range": wide["range"], "page": {"limit": 0}}),
        ),
        (
            "page limit 101",
            json!({"range": wide["range"], "page": {"limit": 101}}),
        ),
        (
            "empty app id",
            json!({"range": wide["range"], "app_ids": [""]}),
        ),
        (
            "bad cursor",
            json!({"range": wide["range"], "page": {"cursor": "c1.AAAAAAAAAAAAAAAAAAAAAAAA"}}),
        ),
    ] {
        let (err, v) = tool(&h, &reader(), "history_coverage", params).await;
        assert!(err && v["code"] == "invalid_request", "{case}: {v}");
    }
    let evidence: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/signal/evidence.json")).unwrap();
    let window = &evidence["window"];
    let (since, until) = (
        airs_runtime_core::time::iso(window["start_ms"].as_i64().unwrap()),
        airs_runtime_core::time::iso(window["end_ms"].as_i64().unwrap()),
    );
    let covered = range(&since, &until);
    let (err, full) = tool(&h, &reader(), "history_coverage", covered.clone()).await;
    assert!(!err, "{full}");
    assert_frozen("history_coverage", &full);
    let items = full["items"].as_array().unwrap().clone();
    assert!(
        !items.is_empty(),
        "imported evidence yields coverage: {full}"
    );
    for item in &items {
        assert_ne!(
            item["state"], "complete",
            "legacy imports never prove traversal: {item}"
        );
    }
    // One item per page, chained by cursors, reproduces the full list exactly.
    let mut paged = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut params = covered.clone();
        params["page"] = match &cursor {
            None => json!({"limit": 1}),
            Some(c) => json!({"limit": 1, "cursor": c}),
        };
        let (err, page) = tool(&h, &reader(), "history_coverage", params).await;
        assert!(!err, "{page}");
        assert_frozen("history_coverage", &page);
        paged.extend(page["items"].as_array().unwrap().iter().cloned());
        match page["next_cursor"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
        assert!(paged.len() <= items.len(), "paging terminates");
    }
    assert_eq!(paged, items);
    // A cursor carries its scope: another range, or a tampered cursor, is refused.
    if items.len() > 1 {
        let mut first = covered.clone();
        first["page"] = json!({"limit": 1});
        let (_, page) = tool(&h, &reader(), "history_coverage", first).await;
        let next = page["next_cursor"].as_str().unwrap().to_owned();
        // A valid range of its own (one hour longer), so only the cursor's scope binding
        // can refuse it.
        let longer = airs_runtime_core::time::iso(window["end_ms"].as_i64().unwrap() + 3_600_000);
        let mut other = range(&since, &longer);
        other["page"] = json!({"cursor": next});
        let (err, v) = tool(&h, &reader(), "history_coverage", other).await;
        assert!(err && v["code"] == "invalid_request", "{v}");
        let mut tampered = covered.clone();
        let mut bytes = next.into_bytes();
        let last = bytes.len() - 1;
        bytes[last] = if bytes[last] == b'A' { b'B' } else { b'A' };
        tampered["page"] = json!({"cursor": String::from_utf8(bytes).unwrap()});
        let (err, v) = tool(&h, &reader(), "history_coverage", tampered).await;
        assert!(err && v["code"] == "invalid_request", "{v}");
    }
}

/// Contract §2.5, §11: a reload takes effect on the next request, so a removed grant
/// is refused at once; a reload that changes the archive identity, or does not
/// validate, is refused and the previous configuration stays active.
#[tokio::test]
async fn reloads_revoke_on_the_next_request_and_never_half_apply() {
    let h = harness().await;
    let (err, _) = tool(&h, &reader(), "service_info", json!({})).await;
    assert!(!err);
    let without = h.text.replace(
        "client_id = \"claude-code\"\nowner_object_id",
        "client_id = \"claude-code-retired\"\nowner_object_id",
    );
    assert_ne!(without, h.text);
    let revision = h
        .shared
        .reload(
            config::parse(&without.replace("revision = \"test-1\"", "revision = \"test-2\""))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(revision, "test-2");
    let (status, _, body) = stateless_post(
        &h.url,
        None,
        &reader(),
        &call_body("service_info", json!({})),
    )
    .await;
    assert_eq!(status, 403, "{body}");
    assert_eq!(body["code"], "forbidden");
    for (case, text) in [
        (
            "tenant",
            without.replace("tenant = \"svc-test\"", "tenant = \"svc-other\""),
        ),
        ("tsg", without.replace("tsg = \"100\"", "tsg = \"101\"")),
        (
            "allowed hosts",
            without.replace(
                "allowed_hosts = [\"127.0.0.1\"]",
                "allowed_hosts = [\"127.0.0.2\"]",
            ),
        ),
        (
            "body bound",
            without.replace(
                "allow_loopback_http_jwks = true",
                "allow_loopback_http_jwks = true\nmax_request_body_bytes = 4096",
            ),
        ),
    ] {
        assert!(
            h.shared.reload(config::parse(&text).unwrap()).is_err(),
            "{case}"
        );
    }
    assert_eq!(
        h.shared.current().config.revision,
        "test-2",
        "refused reloads change nothing"
    );
}

/// M8.1 metrics on the private health listener: fixed series from the real service
/// storage, the bounded control read answering, all ten job states, volume and store
/// sizes, and nothing tenant-, path- or credential-derived; maintenance turns ready off.
#[tokio::test]
async fn the_private_health_listener_serves_bounded_metrics() {
    let h = harness().await;
    let health = serve(run::health_router_with(
        h.shared.clone(),
        true,
        Some(h.storage.clone()),
    ))
    .await;
    let get = || {
        let url = format!("http://{health}/metrics");
        async move {
            let r = reqwest::get(url).await.unwrap();
            let ct = r.headers()["content-type"].to_str().unwrap().to_owned();
            (r.status().as_u16(), ct, r.text().await.unwrap())
        }
    };
    let (status, ct, text) = get().await;
    assert_eq!(status, 200);
    assert!(ct.starts_with("text/plain"), "{ct}");
    for series in [
        "airs_mcp_up 1",
        "airs_mcp_ready 1",
        "airs_mcp_maintenance 0",
        "airs_mcp_control_ok 1",
        "airs_mcp_jobs{state=\"queued\"} 0",
    ] {
        assert!(text.contains(series), "{series} missing:\n{text}");
    }
    assert_eq!(text.matches("airs_mcp_jobs{").count(), 10);
    let free: u64 = text
        .lines()
        .find_map(|l| l.strip_prefix("airs_mcp_data_volume_free_bytes "))
        .unwrap()
        .parse()
        .unwrap();
    assert!(free > 0);
    let control: u64 = text
        .lines()
        .find_map(|l| l.strip_prefix("airs_mcp_control_bytes "))
        .unwrap()
        .parse()
        .unwrap();
    assert!(control > 0, "the control store exists");
    assert!(!text.contains(TENANT), "no tenant name");
    assert!(
        !text.contains(h.history.to_str().unwrap()),
        "no filesystem paths"
    );
    let maint = h.text.replace(
        "tenant = \"svc-test\"",
        "tenant = \"svc-test\"\nmaintenance = true",
    );
    h.shared.reload(config::parse(&maint).unwrap()).unwrap();
    let (_, _, text) = get().await;
    assert!(text.contains("airs_mcp_ready 0\n") && text.contains("airs_mcp_maintenance 1\n"));
}

/// Maintenance mode: every tool but `service_info` answers `maintenance_required`;
/// `service_info` reports it. The private health listener says not ready, with fixed
/// bodies and no authentication or tenant data.
#[tokio::test]
async fn maintenance_mode_and_the_private_health_listener() {
    let h = harness().await;
    let health = serve(run::health_router(h.shared.clone(), true)).await;
    let get = |path: &str| {
        let url = format!("http://{health}{path}");
        async move {
            let r = reqwest::get(url).await.unwrap();
            (r.status().as_u16(), r.text().await.unwrap())
        }
    };
    assert_eq!(get("/livez").await, (200, "ok".into()));
    assert_eq!(get("/readyz").await, (200, "ready".into()));
    assert_eq!(
        get("/mcp").await.0,
        404,
        "the health listener serves no MCP"
    );
    let text = h.text.replace(
        "tenant = \"svc-test\"",
        "tenant = \"svc-test\"\nmaintenance = true",
    );
    h.shared.reload(config::parse(&text).unwrap()).unwrap();
    assert_eq!(get("/readyz").await, (503, "not ready".into()));
    let (err, v) = tool(&h, &reader(), "history_status", json!({})).await;
    assert!(err && v["code"] == "maintenance_required", "{v}");
    let (err, v) = tool(&h, &reader(), "service_info", json!({})).await;
    assert!(!err, "{v}");
    assert_eq!(v["health"], "maintenance");
    assert_frozen("service_info", &v);
}

/// Contract §2.3, §4.1 (M0.6 step 7.6): no session is issued, a forwarded session ID
/// changes nothing, GET and DELETE are 405, every request stands on its own tokens,
/// and an oversized body is refused before any tool runs.
#[tokio::test]
async fn the_transport_is_stateless_and_bounded() {
    let h = harness().await;
    let (status, sid, _) = stateless_post(&h.url, None, &reader(), INIT).await;
    assert_eq!((status, sid), (200, None));
    let operator = creds(
        scoped("AirsHistory.Operate"),
        assertion("human-a", "ops-agent", |_| {}),
    );
    for (c, tier) in [(reader(), "reader"), (operator, "operator")] {
        for forwarded in [None, Some("session-from-another-agent")] {
            let (status, sid, body) =
                stateless_post(&h.url, forwarded, &c, &call_body("service_info", json!({}))).await;
            assert_eq!((status, sid), (200, None));
            assert_eq!(body["result"]["structuredContent"]["effective_tier"], tier);
        }
    }
    for method in [reqwest::Method::GET, reqwest::Method::DELETE] {
        let mut req = reqwest::Client::new()
            .request(method.clone(), &h.url)
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-06-18");
        for (k, v) in reader() {
            req = req.header(k, v);
        }
        assert_eq!(req.send().await.unwrap().status().as_u16(), 405, "{method}");
    }
    let huge = call_body("service_info", json!({"pad": "x".repeat(300 * 1024)}));
    assert_eq!(stateless_post(&h.url, None, &reader(), &huge).await.0, 413);
}

/// Contract §2.2 resolution 3 (M0.6): the assertion `aud` is organisation-wide, so the
/// only per-backend binding is the upstream token (`aud` = this API, `azp` = this
/// integration's gateway client).
#[tokio::test]
async fn org_wide_assertion_audience_is_bound_only_by_the_upstream_token() {
    let h = harness().await;
    let org = || {
        assertion("human-a", "claude-code", |c| {
            c.insert("aud".into(), json!("org-test"));
        })
    };
    let info = call_body("service_info", json!({}));
    let (status, _, body) =
        stateless_post(&h.url, None, &creds(upstream(|_| {}), org()), &info).await;
    assert_eq!(status, 200, "{body}");
    for (case, expected, token) in [
        (
            "another API audience",
            401,
            upstream(|c| {
                c.insert("aud".into(), json!("00000000-0000-4000-8000-0000000000ff"));
            }),
        ),
        (
            "another gateway client",
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
        let (status, _, _) = stateless_post(&h.url, None, &creds(token, org()), &info).await;
        assert_eq!(status, expected, "{case}");
    }
    let bare = vec![("x-user-jwt", org())];
    assert_eq!(stateless_post(&h.url, None, &bare, &info).await.0, 401);
}

/// Unauthenticated discovery (RFC 9728) exposes no tenant data, and a 401 points to it.
#[tokio::test]
async fn protected_resource_metadata_is_public_and_tenant_free() {
    let h = harness().await;
    let base = h.url.trim_end_matches("/mcp").to_owned();
    for path in [
        "/.well-known/oauth-protected-resource",
        "/.well-known/oauth-protected-resource/mcp",
    ] {
        let v: Value = reqwest::get(format!("{base}{path}"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(v["resource"], BACKEND);
        assert_eq!(v["bearer_methods_supported"], json!(["header"]));
        assert!(!v.to_string().contains(TENANT) && !v.to_string().contains("\"100\""));
    }
    let r = reqwest::Client::new()
        .post(&h.url)
        .body(INIT)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 401);
    let www = r.headers()["www-authenticate"].to_str().unwrap().to_owned();
    assert!(
        www.contains("/.well-known/oauth-protected-resource/mcp"),
        "{www}"
    );
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

/// Two agents of one user through rmcp's own Streamable HTTP client, interleaved and
/// concurrent: every call is answered with the caller's own tier (M0.6 step 7.6).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rmcp_clients_of_one_user_are_served_concurrently_with_their_own_tiers() {
    let h = harness().await;
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
    let listed: Vec<String> = reader
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    assert_eq!(
        listed,
        [
            "history_coverage",
            "history_status",
            "jobs_get",
            "jobs_list",
            "reports_list",
            "service_info"
        ]
    );
    let call = |c: &rmcp::service::RunningService<rmcp::RoleClient, ClientConfig>| {
        let peer = c.peer().clone();
        async move {
            let args = json!({"schema_version": 1, "tenant": TENANT, "params": {}});
            let r = peer
                .call_tool(
                    CallToolRequestParams::new("service_info")
                        .with_arguments(args.as_object().unwrap().clone()),
                )
                .await
                .expect("call");
            r.structured_content.expect("structured")["effective_tier"]
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
            if i % 2 == 0 { "reader" } else { "operator" },
            "call {i}"
        );
    }
    let _ = reader.cancel().await;
    let _ = operator.cancel().await;
}

/// Contract §2.5: every tool call writes one audit line holding `(sub, client_id)`, the
/// grant label, the tool, the policy revision and the outcome code, and no log line
/// carries a token, an argument value or a header.
#[tokio::test]
async fn audit_lines_hold_only_identifiers_and_outcomes() {
    let h = harness().await;
    run::tap_logs();
    let bearer = upstream(|_| {});
    let ia = assertion("human-a", "claude-code", |_| {});
    let who = creds(bearer.clone(), ia.clone());
    let (err, _) = tool(&h, &who, "service_info", json!({})).await;
    assert!(!err);
    let (err, _) = tool(
        &h,
        &who,
        "collection_start",
        json!({"secret-looking": "SYNTHETIC_ARGUMENT_VALUE"}),
    )
    .await;
    assert!(err);
    let lines = run::tapped_logs();
    let audits: Vec<Value> = lines
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| {
            v["event"] == "audit" && v["sub"] == "human-a" && v["client_id"] == "claude-code"
        })
        .collect();
    let find = |tool: &str| {
        audits
            .iter()
            .find(|v| v["tool"] == tool)
            .unwrap_or_else(|| panic!("no audit line for {tool}: {audits:?}"))
            .clone()
    };
    let ok = find("service_info");
    assert_eq!(ok["client_id"], "claude-code");
    assert_eq!(ok["grant"], "claude-code-reader");
    assert_eq!(ok["revision"], "test-1");
    assert_eq!(ok["outcome"], "ok");
    let mut keys: Vec<&String> = ok.as_object().unwrap().keys().collect();
    keys.sort();
    const AUDIT: &[&str] = &[
        "client_id",
        "event",
        "grant",
        "job_id",
        "outcome",
        "revision",
        "scope_digest",
        "sub",
        "time",
        "tool",
    ];
    assert_eq!(keys, AUDIT);
    assert_eq!(ok["scope_digest"].as_str().unwrap().len(), 64);
    assert!(airs_runtime_core::dto::utc_ms(ok["time"].as_str().unwrap()).is_some());
    // Every authentication and audit record holds only its allowlisted keys (§2.5).
    for line in &lines {
        let v: Value = serde_json::from_str(line).unwrap();
        let allowed: &[&str] = match v["event"].as_str().unwrap() {
            "audit" => AUDIT,
            "allowed" => &["event", "method", "permission", "revision"],
            "denied" => &["code", "event", "reason", "ungranted"],
            "session_header_ignored" => &["event"],
            other => panic!("unexpected log event {other}: {line}"),
        };
        for key in v.as_object().unwrap().keys() {
            assert!(allowed.contains(&key.as_str()), "{key} in {line}");
        }
    }
    assert_eq!(find("collection_start")["outcome"], "forbidden");
    for line in &lines {
        for secret in [&bearer, &ia, &"SYNTHETIC_ARGUMENT_VALUE".to_owned()] {
            assert!(
                !line.contains(secret.as_str()),
                "log carries a secret: {line}"
            );
        }
    }
}

fn idempotent_body(tool: &str, key: &str, epoch: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": tool,
        "arguments": {"schema_version": 1, "tenant": TENANT, "idempotency_key": key,
            "expected_epoch": epoch, "params": params}}})
    .to_string()
}

async fn submit(
    h: &Harness,
    who: &[(&str, String)],
    key: &str,
    epoch: &str,
    params: Value,
) -> (bool, Value) {
    let (status, _, body) = stateless_post(
        &h.url,
        None,
        who,
        &idempotent_body("collection_start", key, epoch, params),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    (
        body["result"]["isError"] == true,
        body["result"]["structuredContent"].clone(),
    )
}

/// M3 jobs through the service: a durable, idempotent `collection_start` (a lost
/// response is recovered by repeating the call), equivalent submissions joining one job,
/// jobs visible only to their submitter, paging, and explicit cancellation. Every
/// result has the frozen shape.
#[tokio::test]
async fn jobs_are_durable_idempotent_owned_and_cancellable() {
    let h = harness().await;
    let operator = creds(
        scoped("AirsHistory.Operate"),
        assertion("human-a", "ops-agent", |_| {}),
    );
    let (_, info) = tool(&h, &operator, "service_info", json!({})).await;
    let epoch = info["epoch"].as_str().unwrap().to_owned();
    let recent = json!({"mode": "recent"});
    let (err, first) = submit(
        &h,
        &operator,
        "key-collect-00000001",
        &epoch,
        recent.clone(),
    )
    .await;
    assert!(!err, "{first}");
    assert_frozen("collection_start", &first);
    assert_eq!(first["job"]["state"], "queued");
    assert_eq!(first["joined_existing"], false);
    let job_id = first["job"]["job_id"].as_str().unwrap().to_owned();
    let (_, retry) = submit(
        &h,
        &operator,
        "key-collect-00000001",
        &epoch,
        recent.clone(),
    )
    .await;
    assert_eq!(
        retry["job"]["job_id"],
        job_id.as_str(),
        "a retried submission returns the same job"
    );
    let (err, v) = submit(&h, &operator, "key-collect-00000001", &epoch, json!({"mode": "backfill", "range": {"since": "2026-01-01T00:00:00Z", "until": "2026-01-02T00:00:00Z"}})).await;
    assert!(err && v["code"] == "idempotency_conflict", "{v}");
    let (err, v) = submit(
        &h,
        &operator,
        "key-collect-00000002",
        "ffffffff",
        recent.clone(),
    )
    .await;
    assert!(err && v["code"] == "control_restored", "{v}");
    let (_, joined) = submit(
        &h,
        &operator,
        "key-collect-00000003",
        &epoch,
        recent.clone(),
    )
    .await;
    assert_eq!(
        (
            joined["job"]["job_id"].as_str(),
            joined["joined_existing"].clone()
        ),
        (Some(job_id.as_str()), json!(true))
    );
    for (case, params) in [
        (
            "recent with a range",
            json!({"mode": "recent", "range": {"since": "2026-01-01T00:00:00Z", "until": "2026-01-02T00:00:00Z"}}),
        ),
        ("backfill without a range", json!({"mode": "backfill"})),
        (
            "future backfill",
            json!({"mode": "backfill", "range": {"since": "2098-01-01T00:00:00Z", "until": "2099-01-01T00:00:00Z"}}),
        ),
        ("unknown mode", json!({"mode": "everything"})),
    ] {
        let (err, v) = submit(&h, &operator, "key-collect-00000004", &epoch, params).await;
        assert!(err && v["code"] == "invalid_request", "{case}: {v}");
    }
    // A read tool refuses an idempotency key; the idempotent tool requires one.
    let (_, _, body) = stateless_post(
        &h.url,
        None,
        &operator,
        &call_body("collection_start", recent.clone()),
    )
    .await;
    assert_eq!(
        body["result"]["structuredContent"]["code"],
        "invalid_request"
    );
    // Owned: the submitter sees the job; another principal of the same user does not.
    let (err, got) = tool(&h, &operator, "jobs_get", json!({"job_id": job_id})).await;
    assert!(!err, "{got}");
    assert_frozen("jobs_get", &got);
    let (err, v) = tool(&h, &reader(), "jobs_get", json!({"job_id": job_id})).await;
    assert!(err && v["code"] == "not_found", "{v}");
    let (err, v) = tool(&h, &reader(), "jobs_cancel", json!({"job_id": job_id})).await;
    assert!(err && v["code"] == "forbidden", "{v}");
    let (err, v) = tool(&h, &operator, "jobs_get", json!({"job_id": "not-a-job"})).await;
    assert!(err && v["code"] == "invalid_request", "{v}");
    // Paging over two jobs, one per page.
    let backfill = json!({"mode": "backfill", "range": {"since": "2026-01-01T00:00:00Z", "until": "2026-01-02T00:00:00Z"}});
    let (err, second) = submit(&h, &operator, "key-collect-00000005", &epoch, backfill).await;
    assert!(!err, "{second}");
    let (_, page) = tool(&h, &operator, "jobs_list", json!({"page": {"limit": 1}})).await;
    assert_frozen("jobs_list", &page);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    let cursor = page["next_cursor"].as_str().unwrap().to_owned();
    let (_, rest) = tool(
        &h,
        &operator,
        "jobs_list",
        json!({"page": {"limit": 1, "cursor": cursor}}),
    )
    .await;
    assert_frozen("jobs_list", &rest);
    let ids: Vec<&str> = [&page, &rest]
        .iter()
        .flat_map(|p| p["items"].as_array().unwrap())
        .map(|j| j["job_id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&job_id.as_str()) && ids.len() == 2, "{ids:?}");
    assert_eq!(rest["next_cursor"], Value::Null);
    let (_, none) = tool(&h, &reader(), "jobs_list", json!({})).await;
    assert_eq!(none["items"], json!([]), "another principal lists nothing");
    let (err, v) = tool(&h, &operator, "jobs_list", json!({"states": ["done"]})).await;
    assert!(err && v["code"] == "invalid_request", "{v}");
    // Explicit cancellation, idempotent by job.
    let (err, cancelled) = tool(&h, &operator, "jobs_cancel", json!({"job_id": job_id})).await;
    assert!(!err, "{cancelled}");
    assert_frozen("jobs_cancel", &cancelled);
    assert_eq!(cancelled["job"]["state"], "cancel_requested");
    let (_, again) = tool(&h, &operator, "jobs_cancel", json!({"job_id": job_id})).await;
    assert_eq!(again["job"]["state"], "cancel_requested");
    let (_, queued) = tool(&h, &operator, "jobs_list", json!({"states": ["queued"]})).await;
    assert_eq!(queued["items"].as_array().unwrap().len(), 1, "{queued}");
}

/// Review M2-1 finding 5: transport refusals and JSON-RPC errors never reflect what the
/// client sent (headers, bodies, method names or parser text).
#[tokio::test]
async fn transport_errors_never_reflect_client_text() {
    let h = harness().await;
    const SENTINEL: &str = "SYNTHETIC_REFLECTION_SENTINEL";
    let send = |protocol: &'static str, body: String| {
        let url = h.url.clone();
        async move {
            let mut req = reqwest::Client::new()
                .post(&url)
                .header("accept", "application/json, text/event-stream")
                .header("content-type", "application/json")
                .header("mcp-protocol-version", protocol)
                .body(body);
            for (k, v) in reader() {
                req = req.header(k, v);
            }
            let r = req.send().await.unwrap();
            (r.status().as_u16(), r.text().await.unwrap())
        }
    };
    let cases = [
        ("2099-01-01-SYNTHETIC_REFLECTION_SENTINEL", INIT.to_owned()),
        (
            "2025-06-18",
            format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"{SENTINEL}\"}}"),
        ),
        ("2025-06-18", format!("{{not json {SENTINEL}")),
        (
            "2025-06-18",
            format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{{\"name\":7,\"{SENTINEL}\":1}}}}"
            ),
        ),
    ];
    for (protocol, body) in cases {
        let (status, text) = send(protocol, body.clone()).await;
        assert!(!text.contains(SENTINEL), "{status}: {text} for {body}");
        assert!(!text.contains("2099-01-01"), "{status}: {text}");
    }
}

/// Review M2-1 finding 6: a cursor is bound to its exact parameters and principal, and
/// a change to the result set between pages is `conflict` (restart paging).
#[tokio::test]
async fn coverage_cursors_bind_parameters_and_principal_and_detect_changes() {
    let h = harness().await;
    seed_archive(&h);
    let evidence: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/signal/evidence.json")).unwrap();
    let start = evidence["window"]["start_ms"].as_i64().unwrap();
    let end = evidence["window"]["end_ms"].as_i64().unwrap();
    let iso = airs_runtime_core::time::iso;
    let a = range(&iso(start), &iso(end));
    let b = range(&iso(start), &iso(end + 3_600_000));
    let first_page = |r: &Value| {
        let mut p = r.clone();
        p["page"] = json!({"limit": 1});
        p
    };
    let (_, page) = tool(&h, &reader(), "history_coverage", first_page(&a)).await;
    let cursor = page["next_cursor"]
        .as_str()
        .expect("two groups make two pages")
        .to_owned();
    let with = |r: &Value, c: &str| {
        let mut p = r.clone();
        p["page"] = json!({"limit": 1, "cursor": c});
        p
    };
    // Both ranges are valid on their own; the cursor still refuses the other one.
    let (err, v) = tool(&h, &reader(), "history_coverage", b.clone()).await;
    assert!(!err, "{v}");
    let (err, v) = tool(&h, &reader(), "history_coverage", with(&b, &cursor)).await;
    assert!(err && v["code"] == "invalid_request", "another range: {v}");
    // Another principal, granted the same tool, cannot continue this principal's paging.
    let other = creds(
        upstream(|_| {}),
        assertion("human-a", "claude-code-2", |_| {}),
    );
    let (err, v) = tool(&h, &other, "history_coverage", a.clone()).await;
    assert!(!err, "{v}");
    let (err, v) = tool(&h, &other, "history_coverage", with(&a, &cursor)).await;
    assert!(
        err && v["code"] == "invalid_request",
        "another principal: {v}"
    );
    // The owner continues while nothing changed...
    let (err, v) = tool(&h, &reader(), "history_coverage", with(&a, &cursor)).await;
    assert!(!err, "{v}");
    // ...and a new import between pages changes the result set: restart required.
    seed_archive_with(&h, &["sessions", "profiles"]);
    let (err, v) = tool(&h, &reader(), "history_coverage", with(&a, &cursor)).await;
    assert!(err && v["code"] == "conflict", "changed result set: {v}");
}
