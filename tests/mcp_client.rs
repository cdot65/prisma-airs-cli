//! M7 loopback end-to-end (RUNTIME_MCP_CONTRACT §7, plan M7 exit gate): the real `airs`
//! executable → a fixture gateway and authorization server → the real `airs-mcp`
//! server. The fixture gateway models the deployed hop: it checks the CLI's gateway
//! token, then presents an Entra-shaped upstream token and a signed identity assertion
//! (minted from the synthetic fixture keys) to the server. Loopback only: the live
//! gateway proof is M8.

use std::collections::HashMap;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use airs_test_harness::{Cli, FixtureServer, RunOptions, Sandbox, run, spawn};
use axum::{
    Router,
    body::Body,
    extract::{Form, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

const FIX: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/crates/airs-mcp/tests/fixtures"
);
const TID: &str = "00000000-0000-4000-8000-0000000000aa";
const API: &str = "00000000-0000-4000-8000-0000000000ab";
const GW_CLIENT: &str = "00000000-0000-4000-8000-0000000000ac";
const OWNER: &str = "00000000-0000-4000-8000-0000000000ad";
const BACKEND: &str = "https://mcp-backend-airs.example.test/mcp";
const CLI_CLIENT: &str = "airs-cli-public";
/// The admin-role connection's public client (subject `admin-a`, Administrator grant).
const ADMIN_CLIENT: &str = "airs-cli-admin";
/// An agent's public client (subject `agent-a`, Administrator grant: previews and
/// approved execution only).
const AGENT_CLIENT: &str = "airs-cli-agent";
const SERVICE_TENANT: &str = "svc-test";
const SCANNER_KEY: &str = "FAKE-APIKEY-100";

/// The subject the fixture gateway signs for each public client.
fn subject(client: &str) -> &'static str {
    match client {
        ADMIN_CLIENT => "admin-a",
        AGENT_CLIENT => "agent-a",
        _ => "human-a",
    }
}

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn sign(name: &str, kid: &str, claims: Value) -> String {
    let key =
        EncodingKey::from_rsa_pem(&std::fs::read(format!("{FIX}/{name}-private.pem")).unwrap())
            .unwrap();
    let mut h = Header::new(Algorithm::RS256);
    h.kid = Some(kid.into());
    encode(&h, &claims, &key).unwrap()
}

async fn serve(router: Router) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let a = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, router).await.unwrap() });
    format!("127.0.0.1:{}", a.port())
}

fn private_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("airs-m7-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new().mode(0o700).create(&d).unwrap();
    d.canonicalize().unwrap()
}

/// The real server (with its worker running: collection, reports, lifecycle), with an
/// operator grant for the CLI's public client, Administrator grants for the admin and
/// agent clients, `[admin] mode = "fresh_proof"`, a named scanner and its receipt
/// association, and synthetic secret files.
async fn backend(scan: &str, mgmt: &str) -> (String, std::thread::JoinHandle<()>) {
    use airs_mcp::{config, run, storage, tools};
    let entra: Value =
        serde_json::from_slice(&std::fs::read(format!("{FIX}/entra-jwks.json")).unwrap()).unwrap();
    let gw: Value =
        serde_json::from_slice(&std::fs::read(format!("{FIX}/gateway-jwks.json")).unwrap())
            .unwrap();
    let idp = Router::new()
        .route(
            "/entra/keys",
            get(move || {
                let v = entra.clone();
                async move { axum::Json(v) }
            }),
        )
        .route(
            "/gw/jwks.json",
            get(move || {
                let v = gw.clone();
                async move { axum::Json(v) }
            }),
        );
    let idp = serve(idp).await;
    let base = private_dir("svc");
    let data = base.join("data");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&data)
        .unwrap();
    let secrets = base.join("secrets");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&secrets)
        .unwrap();
    let secret = |name: &str, value: &str| {
        let p = secrets.join(name);
        std::fs::write(&p, value).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
        p.display().to_string()
    };
    let (mgmt_id, mgmt_secret, scanner_key) = (
        secret("mgmt-client-id", "client-100"),
        secret("mgmt-client-secret", "FAKE-SECRET-100"),
        secret("scanner-monitor", SCANNER_KEY),
    );
    let text = format!(
        r#"revision = "m7-1"
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
owner_object_ids = ["{OWNER}"]
[assertion]
header = "x-user-jwt"
issuer = "https://mcp.example.test"
audiences = ["{BACKEND}", "org-test"]
algorithms = ["RS256"]
jwks_url = "http://{idp}/gw/jwks.json"
max_lifetime_seconds = 300
workspace_ids = ["ws-test"]
organization_ids = ["org-test"]
[admin]
mode = "fresh_proof"
proof_nonce_claim = "proof_nonce"
proof_max_age_seconds = 300
[service]
tenant = "{SERVICE_TENANT}"
[airs]
tsg = "100"
management_endpoint = "{mgmt}{mgmt_prefix}"
dashboard_endpoint = "{mgmt}/dashboard"
token_endpoint = "{mgmt}{token_path}"
management_client_id_file = "{mgmt_id}"
management_client_secret_file = "{mgmt_secret}"
management_revision = "00000000-0000-4000-8000-0000000000e1"
[[airs.scanners]]
name = "monitor"
applicationId = "app-example"
endpoint = "{scan}"
numRetries = 0
revision = "00000000-0000-4000-8000-0000000000e2"
api_key_file = "{scanner_key}"
[[receipt_associations]]
subject = "human-a"
client_id = "{CLI_CLIENT}"
declared_scanner = "monitor"
scanner = "monitor"
[[grants]]
subject = "human-a"
client_id = "{CLI_CLIENT}"
owner_object_id = "{OWNER}"
permission = "operator"
tools = ["service_info", "history_status", "history_coverage", "history_query", "jobs_list", "jobs_get", "jobs_cancel", "collection_start", "schedules_list", "schedules_set", "schedules_disable", "receipts_ingest", "reports_create", "reports_list", "reports_get", "artifacts_download"]
[[grants]]
subject = "admin-a"
client_id = "{ADMIN_CLIENT}"
owner_object_id = "{OWNER}"
permission = "administrator"
tools = ["service_info", "jobs_get", "lifecycle_preview", "lifecycle_execute", "admin_confirm", "transfer_upload_begin", "transfer_upload_chunk", "transfer_upload_finalize", "transfer_upload_abort", "export_read"]
[[grants]]
subject = "agent-a"
client_id = "{AGENT_CLIENT}"
owner_object_id = "{OWNER}"
permission = "administrator"
tools = ["service_info", "jobs_get", "lifecycle_preview", "lifecycle_execute"]
"#,
        data = data.display(),
        mgmt_prefix = airs_test_harness::paths::MGMT_PREFIX,
        token_path = airs_test_harness::paths::TOKEN,
    );
    let cfg = config::parse(&text).expect("fixture config");
    let admitted = storage::admit(&cfg.server.data_dir).unwrap();
    let history = storage::private_child(&admitted, "history").unwrap();
    let control = storage::private_child(&admitted, "control").unwrap();
    let seed = storage::control_seed(&control).unwrap();
    let store = airs_mcp::control::Control::open(
        &control.path,
        SERVICE_TENANT,
        &seed.epoch,
        &seed.cursor_key,
        airs_runtime_core::time::now_ms(),
    )
    .unwrap();
    let owner = storage::OwnerRecord {
        owner_id: "m7".into(),
        pod: "m7".into(),
        node: "m7".into(),
        pid: 0,
        acquired_at_unix: 0,
    };
    let storage = Arc::new(tools::Storage::new(
        admitted,
        owner,
        history.path.clone(),
        store,
    ));
    let shared = run::Shared::new(run::Live::new(cfg).unwrap());
    // The worker, as the service runs it (collection, reports, lifecycle executors),
    // on its own thread with a short tick.
    let worker = {
        use airs_mcp::worker::{Executor, Worker};
        let mut executors: std::collections::BTreeMap<String, Arc<dyn Executor>> =
            Default::default();
        executors.insert(
            "collection_start".into(),
            Arc::new(airs_mcp::collection::Collection {
                shared: shared.clone(),
            }),
        );
        executors.insert(
            "lifecycle_execute".into(),
            Arc::new(airs_mcp::lifecycle::Lifecycle {
                shared: shared.clone(),
            }),
        );
        let dir = storage::private_child(&storage.admitted, "reports").unwrap();
        let reporter: Arc<dyn Executor> = Arc::new(airs_mcp::reports::Reporter {
            shared: shared.clone(),
            bundles: airs_mcp::reports::Bundles { root: dir.path },
        });
        executors.insert("reports_create".into(), reporter);
        let w = Worker {
            storage: storage.clone(),
            owner: "m7".into(),
            executors,
        };
        std::thread::spawn(move || {
            loop {
                match w.step(SERVICE_TENANT, |_| true) {
                    Ok(Some(_)) => continue,
                    _ => std::thread::sleep(Duration::from_millis(100)),
                }
            }
        })
    };
    let addr = serve(run::router(
        shared,
        tools::Service {
            storage: Some(storage),
        },
    ))
    .await;
    (format!("http://{addr}/mcp"), worker)
}

/// What one authorization request asked for: its client, and (an administrative
/// sign-in) the preview nonce and whether fresh authentication was requested.
#[derive(Clone, Debug, Default)]
struct Grant {
    client: String,
    nonce: Option<String>,
    fresh: bool,
    auth_time: u64,
}

#[derive(Default)]
struct As {
    codes: HashMap<String, (String, String, Grant)>,
    access: HashMap<String, Grant>,
    refresh: HashMap<String, Grant>,
    authorize_calls: usize,
    /// Every authorization request, in order.
    authorizes: Vec<Grant>,
    /// Refresh-token grants served.
    refreshes: usize,
    /// Every token issued (never printed by the CLI).
    issued: Vec<String>,
    /// A hostile service, modelled at the hop: replace `.0` in every MCP response body
    /// with `.1`, where `{bearer}` stands for the caller's own access token.
    echo: Option<(String, String)>,
    /// Hold every `jobs_get` request this long before forwarding it (a stalled poll).
    stall_polls: Option<std::time::Duration>,
    /// Deny every authorization after this many (a sign-in the user declines).
    deny_authorize_after: Option<usize>,
    /// `transfer_upload_abort` calls seen at the hop.
    aborts: usize,
    /// Forward the next `collection_start`, then lose its response (a submission
    /// admitted on the service whose answer never reached the CLI).
    lose_next_submission: bool,
    /// The idempotency key of every `collection_start` seen at the hop, in order.
    submission_keys: Vec<String>,
    /// A value that must never reach the service, and how often it did.
    watch: Option<String>,
    watch_hits: usize,
    /// After the next `transfer_upload_finalize`, fail the next `service_info` (the
    /// execute submission's epoch read: a failure before anything is sent).
    fail_epoch_after_finalize: bool,
    epoch_armed: bool,
    /// Refuse the next `collection_start` with 401 before forwarding it (a token the
    /// gateway stops accepting between the epoch read and the submission).
    refuse_next_submission: bool,
}

#[derive(Clone)]
struct Gw {
    base: String,
    backend: String,
    state: Arc<Mutex<As>>,
    http: reqwest::Client,
}

async fn prm(State(g): State<Gw>) -> impl IntoResponse {
    axum::Json(
        json!({"resource": format!("{}/mcp", g.base), "authorization_servers": [format!("{}/idira", g.base)],
        "scopes_supported": ["mcp:tools"], "bearer_methods_supported": ["header"]}),
    )
}

async fn as_meta(State(g): State<Gw>) -> impl IntoResponse {
    let i = format!("{}/idira", g.base);
    axum::Json(
        json!({"issuer": i, "authorization_endpoint": format!("{i}/authorize"), "token_endpoint": format!("{i}/token"),
        "response_types_supported": ["code"], "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"], "scopes_supported": ["mcp:tools", "offline_access"],
        "token_endpoint_auth_methods_supported": ["none"], "authorization_response_iss_parameter_supported": true}),
    )
}

async fn authorize(State(g): State<Gw>, Query(q): Query<HashMap<String, String>>) -> Response {
    let get = |k: &str| q.get(k).cloned().unwrap_or_default();
    if ![CLI_CLIENT, ADMIN_CLIENT, AGENT_CLIENT].contains(&get("client_id").as_str())
        || get("code_challenge_method") != "S256"
        || !get("redirect_uri").starts_with("http://127.0.0.1:")
    {
        return (StatusCode::BAD_REQUEST, "bad authorization request").into_response();
    }
    let code = uuid::Uuid::new_v4().to_string();
    let grant = Grant {
        client: get("client_id"),
        nonce: q.get("airs_proof_nonce").cloned(),
        fresh: get("max_age") == "0",
        auth_time: now(),
    };
    let mut st = g.state.lock().await;
    st.authorize_calls += 1;
    if st
        .deny_authorize_after
        .is_some_and(|n| st.authorize_calls > n)
    {
        let mut loc = url::Url::parse(&get("redirect_uri")).unwrap();
        loc.query_pairs_mut()
            .append_pair("error", "access_denied")
            .append_pair("state", &get("state"))
            .append_pair("iss", &format!("{}/idira", g.base));
        return (StatusCode::FOUND, [("location", loc.to_string())]).into_response();
    }
    st.authorizes.push(grant.clone());
    st.codes.insert(
        code.clone(),
        (get("code_challenge"), get("redirect_uri"), grant),
    );
    let mut loc = url::Url::parse(&get("redirect_uri")).unwrap();
    loc.query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", &get("state"))
        .append_pair("iss", &format!("{}/idira", g.base));
    (StatusCode::FOUND, [("location", loc.to_string())]).into_response()
}

async fn token(State(g): State<Gw>, Form(f): Form<HashMap<String, String>>) -> Response {
    use base64::Engine;
    let get = |k: &str| f.get(k).cloned().unwrap_or_default();
    let mut st = g.state.lock().await;
    let granted = match get("grant_type").as_str() {
        "authorization_code" => st
            .codes
            .remove(&get("code"))
            .filter(|(challenge, redirect, grant)| {
                base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(Sha256::digest(get("code_verifier").as_bytes()))
                    == *challenge
                    && *redirect == get("redirect_uri")
                    && get("client_id") == grant.client
            })
            .map(|(_, _, grant)| grant),
        // A refreshed token never carries a proof: the nonce is one sign-in's.
        "refresh_token" => st.refresh.remove(&get("refresh_token")).map(|g| Grant {
            nonce: None,
            fresh: false,
            ..g
        }),
        _ => None,
    };
    let Some(grant) = granted else {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(json!({"error": "invalid_grant"})),
        )
            .into_response();
    };
    let (a, r) = (
        uuid::Uuid::new_v4().to_string(),
        uuid::Uuid::new_v4().to_string(),
    );
    if get("grant_type") == "refresh_token" {
        st.refreshes += 1;
    }
    st.issued.extend([a.clone(), r.clone()]);
    st.access.insert(a.clone(), grant.clone());
    st.refresh.insert(r.clone(), grant);
    axum::Json(json!({"access_token": a, "refresh_token": r, "token_type": "Bearer", "expires_in": 3600, "scope": "mcp:tools offline_access"})).into_response()
}

/// The gateway hop: check the CLI token, then forward with the backend credentials.
async fn mcp(
    State(g): State<Gw>,
    method: axum::http::Method,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    let Some(grant) = g.state.lock().await.access.get(bearer).cloned() else {
        let www = format!(
            "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\"",
            g.base
        );
        return (StatusCode::UNAUTHORIZED, [("www-authenticate", www)]).into_response();
    };
    let upstream = sign(
        "entra",
        "entra-1",
        json!({"iss": format!("https://login.microsoftonline.com/{TID}/v2.0"), "aud": API, "tid": TID,
        "oid": OWNER, "azp": GW_CLIENT, "scp": "AirsHistory.Read AirsHistory.Operate AirsHistory.Admin", "ver": "2.0", "iat": now(), "nbf": now() - 5, "exp": now() + 600}),
    );
    // The modelled step-up: a fresh, nonce-bound sign-in is signed into the assertion
    // as the preview's nonce and the authentication time (contract §3 option B).
    let mut claims = json!({"iss": "https://mcp.example.test", "aud": BACKEND, "sub": subject(&grant.client), "client_id": grant.client,
        "workspace_id": "ws-test", "organisation_id": "org-test", "scope": "mcp", "iat": now(), "exp": now() + 120});
    if let (Some(nonce), true) = (&grant.nonce, grant.fresh) {
        claims["proof_nonce"] = json!(nonce);
        claims["auth_time"] = json!(grant.auth_time);
    }
    let ia = sign("gateway", "gw-1", claims);
    if body.windows(23).any(|w| w == b"\"transfer_upload_abort\"") {
        g.state.lock().await.aborts += 1;
    }
    let stall = g.state.lock().await.stall_polls;
    if let Some(d) = stall
        && body.windows(10).any(|w| w == b"\"jobs_get\"")
    {
        tokio::time::sleep(d).await;
    }
    let mut req = g
        .http
        .request(method, &g.backend)
        .header("authorization", format!("Bearer {upstream}"))
        .header("x-user-jwt", ia)
        .body(body.clone());
    for name in [
        "content-type",
        "accept",
        "mcp-session-id",
        "mcp-protocol-version",
        "last-event-id",
    ] {
        if let Some(v) = headers.get(name) {
            req = req.header(name, v);
        }
    }
    {
        let mut st = g.state.lock().await;
        if let Some(w) = st.watch.clone()
            && String::from_utf8_lossy(&body).contains(&w)
        {
            st.watch_hits += 1;
        }
    }
    {
        let mut st = g.state.lock().await;
        let text = String::from_utf8_lossy(&body);
        if st.fail_epoch_after_finalize && text.contains("\"transfer_upload_finalize\"") {
            st.fail_epoch_after_finalize = false;
            st.epoch_armed = true;
        } else if st.epoch_armed && text.contains("\"service_info\"") {
            st.epoch_armed = false;
            return StatusCode::BAD_GATEWAY.into_response();
        }
    }
    let submission = body.windows(18).any(|w| w == b"\"collection_start\"");
    if submission
        && let Ok(v) = serde_json::from_slice::<Value>(&body)
        && let Some(k) = v["params"]["arguments"]["idempotency_key"].as_str()
    {
        g.state.lock().await.submission_keys.push(k.to_owned());
    }
    if submission && std::mem::take(&mut g.state.lock().await.refuse_next_submission) {
        let www = format!(
            "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\"",
            g.base
        );
        return (StatusCode::UNAUTHORIZED, [("www-authenticate", www)]).into_response();
    }
    let lose = submission && {
        let mut st = g.state.lock().await;
        std::mem::take(&mut st.lose_next_submission)
    };
    let Ok(resp) = req.send().await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    if lose {
        let _ = resp.bytes().await;
        return StatusCode::BAD_GATEWAY.into_response();
    }
    let mut out = Response::builder().status(resp.status().as_u16());
    for name in ["content-type", "mcp-session-id"] {
        if let Some(v) = resp.headers().get(name) {
            out = out.header(name, v.as_bytes());
        }
    }
    let echo = g.state.lock().await.echo.clone();
    if let Some((needle, with)) = echo {
        let text = String::from_utf8_lossy(&resp.bytes().await.unwrap_or_default()).into_owned();
        let body = text.replace(&needle, &with.replace("{bearer}", bearer));
        return out.body(Body::from(body)).unwrap();
    }
    out.body(Body::from_stream(resp.bytes_stream())).unwrap()
}

async fn gateway(backend: String) -> Gw {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let g = Gw {
        base,
        backend,
        state: Default::default(),
        http: reqwest::Client::new(),
    };
    let r = Router::new()
        .route("/.well-known/oauth-protected-resource/mcp", get(prm))
        .route("/.well-known/oauth-protected-resource", get(prm))
        .route(
            "/idira/.well-known/oauth-authorization-server",
            get(as_meta),
        )
        .route(
            "/.well-known/oauth-authorization-server/idira",
            get(as_meta),
        )
        .route("/idira/.well-known/openid-configuration", get(as_meta))
        .route("/idira/authorize", get(authorize))
        .route("/idira/token", post(token))
        .route("/mcp", any(mcp))
        .with_state(g.clone());
    tokio::spawn(async move { axum::serve(l, r).await.unwrap() });
    g
}

struct World {
    server: FixtureServer,
    sandbox: Sandbox,
    state: PathBuf,
    gw: Gw,
    _worker: std::thread::JoinHandle<()>,
}

/// Every run may change the tenant store (connection writes); the private MCP state is
/// under the sandbox's `tmp/`; nothing else in the sandbox may change.
fn opts(state: &Path) -> RunOptions {
    RunOptions::new()
        .env("XDG_STATE_HOME", state.to_str().unwrap())
        .secret(SCANNER_KEY)
        // The tenant store (connection changes); the MCP state lives under `tmp/`.
        .allow_writes_under("store")
}

impl World {
    async fn new() -> Self {
        let server = FixtureServer::start();
        let sandbox = Sandbox::new();
        sandbox.single_tenant(&server, "prod", "100");
        let state = sandbox.tmp().join("xdg-state");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&state)
            .unwrap();
        let base = server.base_url().trim_end_matches('/').to_owned();
        let (url, worker) = backend(&server.url("/scan"), &base).await;
        let gw = gateway(url).await;
        Self {
            server,
            sandbox,
            state,
            gw,
            _worker: worker,
        }
    }

    fn airs(&self, args: &[&str]) -> airs_test_harness::Run {
        run(&rust(), &self.sandbox, args, &opts(&self.state))
    }

    fn set_connection(&self) -> airs_test_harness::Run {
        let url = format!("{}/mcp", self.gw.base);
        self.airs(&[
            "mcp",
            "connections",
            "set",
            "gw",
            "--url",
            &url,
            "--client-id",
            CLI_CLIENT,
            "--service-tenant",
            SERVICE_TENANT,
            "--expected-tsg",
            "100",
            "--approved-origin",
            &self.gw.base,
        ])
    }

    /// Drive `airs mcp login` as a user agent: read the printed URL, follow the
    /// authorization redirect into the CLI's loopback callback.
    async fn login(&self) -> airs_test_harness::Run {
        let mut child = spawn(
            &rust(),
            &self.sandbox,
            &["mcp", "login", "gw"],
            &opts(&self.state),
        );
        assert!(
            child.wait_for_stderr("/idira/authorize", Duration::from_secs(20)),
            "{}",
            child.stderr_now()
        );
        let text = child.stderr_now();
        let auth = text
            .lines()
            .find(|l| l.starts_with("http://"))
            .unwrap()
            .to_owned();
        let no_redirect = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let r = no_redirect.get(auth).send().await.unwrap();
        let callback = r
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let _ = reqwest::get(callback).await;
        tokio::task::spawn_blocking(move || child.wait())
            .await
            .unwrap()
    }
}

impl World {
    fn set(&self, name: &str, client: &str, role: &str) -> airs_test_harness::Run {
        let url = format!("{}/mcp", self.gw.base);
        self.airs(&[
            "mcp",
            "connections",
            "set",
            name,
            "--url",
            &url,
            "--client-id",
            client,
            "--service-tenant",
            SERVICE_TENANT,
            "--expected-tsg",
            "100",
            "--approved-origin",
            &self.gw.base,
            "--role",
            role,
        ])
    }

    /// Run `args`, completing each browser sign-in it prints (up to `sign_ins`): read
    /// the k-th authorization URL from stderr and follow it into the CLI's callback.
    async fn signed(&self, args: &[&str], sign_ins: usize) -> airs_test_harness::Run {
        let child = spawn(&rust(), &self.sandbox, args, &opts(&self.state));
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        for k in 0..sign_ins {
            let deadline = std::time::Instant::now() + Duration::from_secs(20);
            let url = loop {
                let urls: Vec<String> = child
                    .stderr_now()
                    .lines()
                    .filter(|l| l.starts_with("http://") && l.contains("/idira/authorize"))
                    .map(str::to_owned)
                    .collect();
                if let Some(u) = urls.get(k) {
                    break Some(u.clone());
                }
                if std::time::Instant::now() >= deadline {
                    break None;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            };
            let Some(url) = url else { break };
            let r = client.get(url).send().await.unwrap();
            let callback = r.headers()["location"].to_str().unwrap().to_owned();
            let _ = reqwest::get(callback).await;
        }
        tokio::task::spawn_blocking(move || child.wait())
            .await
            .unwrap()
    }

    fn scanner(&self) {
        let input = format!(
            "applicationId = \"app-example\"\nendpoint = {}\napiKey = {}\n",
            json!(self.server.url("/scan")),
            json!(SCANNER_KEY)
        );
        run(
            &rust(),
            &self.sandbox,
            &[
                "tenant",
                "scanners",
                "set",
                "prod",
                "monitor",
                "--stdin",
                "--replace",
            ],
            &opts(&self.state).stdin(input),
        )
        .assert_success();
    }

    fn sessions(&self) -> usize {
        std::fs::read_dir(self.state.join("airs-cli/mcp/sessions"))
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
                    .count()
            })
            .unwrap_or(0)
    }
}

fn bulk_state(state: &Path) -> PathBuf {
    std::fs::read_dir(state.join("airs-cli/bulk-scans"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().ends_with(".bulk-scan.json"))
        .unwrap()
}

fn sha256_file(path: &Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

fn json_out(r: &airs_test_harness::Run) -> Value {
    serde_json::from_str(&r.stdout).unwrap_or(Value::Null)
}

/// Contract §7.3: connections live in the tenant file's strict `mcp` namespace; the
/// first write saves a private rollback snapshot; invalid connections are refused
/// before anything is written; listing writes nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connections_are_strict_tenant_namespace_entries_with_a_rollback_snapshot() {
    let w = World::new().await;
    let refused = w.airs(&[
        "mcp",
        "connections",
        "set",
        "gw",
        "--url",
        "http://mcp.example.test/mcp",
        "--client-id",
        CLI_CLIENT,
        "--service-tenant",
        SERVICE_TENANT,
        "--expected-tsg",
        "100",
        "--approved-origin",
        "http://mcp.example.test",
    ]);
    refused.assert_code(2);
    w.set_connection().assert_success();
    let tenants = w.sandbox.store_tenant_path("prod");
    let text = std::fs::read_to_string(&tenants).unwrap();
    assert!(text.contains("[mcp.connections.gw]"), "{text}");
    let snapshots: Vec<_> = std::fs::read_dir(tenants.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().into_string().unwrap())
        .filter(|n| n.starts_with(".prod.pre-mcp."))
        .collect();
    assert_eq!(
        snapshots.len(),
        1,
        "one rollback snapshot on the first write"
    );
    let before = w.sandbox.snapshot();
    let list = w.airs(&["mcp", "connections", "list", "--output", "json"]);
    list.assert_success();
    assert_eq!(json_out(&list)["gw"]["serviceTenant"], SERVICE_TENANT);
    w.sandbox.assert_unchanged(&before);
    w.set_connection().assert_code(1); // exists: --replace required
    w.airs(&["mcp", "connections", "remove", "gw", "--force"])
        .assert_success();
    assert!(
        !std::fs::read_to_string(&tenants)
            .unwrap()
            .contains("[mcp.connections.gw]")
    );
}

/// The M7 vertical slice: without a session every service command fails noninteractively
/// (exit 1, no sign-in started); `login` (PKCE through the fixture gateway) saves a
/// private session; then history, jobs, sync (with a durable operation receipt) and
/// schedules reach the real server; `logout` removes the session.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn login_then_history_jobs_sync_and_schedules_through_the_gateway() {
    let w = World::new().await;
    w.set_connection().assert_success();
    // Noninteractive: no session means "authentication required", never a prompt.
    let denied = w.airs(&["runtime", "history", "status", "--mcp", "gw"]);
    denied.assert_code(1);
    assert!(
        denied.stderr.contains("authentication required"),
        "{}",
        denied.stderr
    );
    assert_eq!(
        w.gw.state.lock().await.authorize_calls,
        0,
        "no sign-in was started"
    );
    // Interactive login.
    let login = w.login().await;
    login.assert_success();
    let sessions = w.state.join("airs-cli/mcp/sessions");
    let files: Vec<_> = std::fs::read_dir(&sessions)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(
        files[0].metadata().unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(&sessions).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let status = w.airs(&["mcp", "status", "--output", "json"]);
    status.assert_success();
    assert_eq!(json_out(&status)["gw"]["session"], true);
    // Reads.
    let history = w.airs(&[
        "runtime", "history", "status", "--mcp", "gw", "--output", "json",
    ]);
    history.assert_success();
    assert_eq!(
        json_out(&history)["schema_version"],
        1,
        "{}",
        history.stdout
    );
    let coverage = w.airs(&[
        "runtime", "history", "coverage", "--mcp", "gw", "--output", "json",
    ]);
    assert!(
        coverage.code == 0 || coverage.stderr.contains("refused"),
        "{}",
        coverage.stderr
    );
    let jobs = w.airs(&["runtime", "jobs", "list", "--mcp", "gw", "--output", "json"]);
    jobs.assert_success();
    assert_eq!(json_out(&jobs)["items"], json!([]));
    // A submission with a durable operation receipt, without waiting.
    let sync = w.airs(&[
        "runtime",
        "sync",
        "--mcp",
        "gw",
        "--no-wait",
        "--output",
        "json",
    ]);
    sync.assert_success();
    let job = json_out(&sync)["job"]["job_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let receipts: Vec<Value> = std::fs::read_dir(w.state.join("airs-cli/mcp/receipts/gw"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
        .map(|e| serde_json::from_slice(&std::fs::read(e.path()).unwrap()).unwrap())
        .collect();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0]["jobId"], job.as_str());
    assert!(receipts[0].get("token").is_none() && !receipts[0].to_string().contains("Bearer"));
    let got = w.airs(&[
        "runtime", "jobs", "get", "--mcp", "gw", &job, "--output", "json",
    ]);
    got.assert_success();
    // The service's worker may already have claimed it.
    assert_eq!(json_out(&got)["job"]["job_id"], job.as_str());
    let cancel = w.airs(&[
        "runtime", "jobs", "cancel", "--mcp", "gw", &job, "--output", "json",
    ]);
    cancel.assert_success();
    // The service's worker may already have run it to completion; cancelling a finished
    // job returns its terminal state unchanged.
    assert!(matches!(
        json_out(&cancel)["job"]["state"].as_str(),
        Some("cancel_requested" | "canceled" | "succeeded" | "partial" | "failed")
    ));
    // Schedules (keyed mutation with the service epoch).
    let set = w.airs(&[
        "runtime",
        "schedules",
        "set",
        "--mcp",
        "gw",
        "--kind",
        "collection",
        "--enabled",
        "true",
        "--effective-at",
        "2026-11-06T00:00:00Z",
        "--interval-minutes",
        "15",
        "--output",
        "json",
    ]);
    set.assert_success();
    let id = json_out(&set)["schedule_id"].as_str().unwrap().to_owned();
    let listed = w.airs(&[
        "runtime",
        "schedules",
        "list",
        "--mcp",
        "gw",
        "--output",
        "json",
    ]);
    listed.assert_success();
    assert_eq!(
        json_out(&listed)["schedules"][0]["schedule_id"],
        id.as_str()
    );
    w.airs(&[
        "runtime",
        "schedules",
        "disable",
        "--mcp",
        "gw",
        &id,
        "--effective-at",
        "2026-11-07T00:00:00Z",
    ])
    .assert_success();
    // Logout: the session is gone and calls are denied again.
    w.airs(&["mcp", "logout", "gw"]).assert_success();
    w.airs(&["runtime", "jobs", "list", "--mcp", "gw"])
        .assert_code(1);
}

/// Contract §7.1/§7.2: the service commands without `--mcp` are usage errors naming
/// the optional service, before any tenant, state or network access; a connection
/// that does not exist is refused before any network access.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn service_commands_need_an_explicit_connection_and_never_fall_back() {
    let w = World::new().await;
    let before = w.sandbox.snapshot();
    for args in [
        vec!["runtime", "sync"],
        vec!["runtime", "history", "status"],
        vec!["runtime", "history", "coverage"],
        vec!["runtime", "jobs", "list"],
        vec!["runtime", "schedules", "list"],
    ] {
        let out = w.airs(&args);
        out.assert_code(2);
        assert!(
            out.stderr.contains("--mcp CONNECTION"),
            "{args:?}: {}",
            out.stderr
        );
    }
    let missing = w.airs(&["runtime", "jobs", "list", "--mcp", "nope"]);
    missing.assert_code(1);
    assert!(
        missing.stderr.contains("No such MCP connection"),
        "{}",
        missing.stderr
    );
    w.sandbox.assert_unchanged(&before);
    assert!(
        !w.state.join("airs-cli").exists(),
        "no MCP state without a selection"
    );
}

/// M7.3 receipt retention and M7.2 archive-only reports through the gateway:
/// `bulk-scan --record-history` retains the received results (no extra AIRS call);
/// `receipts upload` of the same checkpoint is idempotent (all duplicates); a direct
/// `scan --record-history` retains its one result; `report --mcp` renders on the
/// service with zero AIRS calls and downloads a verified private bundle; `history
/// reports|download|query` read it back; a download never overwrites.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn receipts_reports_and_downloads_through_the_gateway() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.scanner();
    // Noninteractive: retention without a session never starts a sign-in.
    let prompts = w.sandbox.tmp().join("prompts.txt");
    std::fs::write(&prompts, "Synthetic sample one\nSynthetic sample two\n").unwrap();
    let csv = w.sandbox.tmp().join("results.csv");
    let unsigned = w.airs(&[
        "runtime",
        "bulk-scan",
        "--profile",
        "monitor",
        "--scanner",
        "monitor",
        "--file",
        prompts.to_str().unwrap(),
        "--output-file",
        csv.to_str().unwrap(),
        "--record-history",
        "gw",
    ]);
    unsigned.assert_success();
    assert!(
        unsigned.stderr.contains("not retained on the MCP service")
            && unsigned.stderr.contains("authentication required"),
        "{}",
        unsigned.stderr
    );
    assert_eq!(
        w.gw.state.lock().await.authorize_calls,
        0,
        "no sign-in started"
    );
    w.login().await.assert_success();
    // The checkpoint's received results, uploaded explicitly.
    let state = bulk_state(&w.state);
    let scans = w.server.requests().len();
    let first = w.airs(&[
        "runtime",
        "receipts",
        "upload",
        "--mcp",
        "gw",
        "--from-bulk-state",
        state.to_str().unwrap(),
        "--output",
        "json",
    ]);
    first.assert_success();
    let v = json_out(&first);
    assert_eq!(
        (v["records"].as_u64(), v["accepted"].as_u64()),
        (Some(2), Some(2)),
        "{v}"
    );
    // A retry of the same checkpoint: the same deterministic batch key, so the service
    // returns the stored result of that batch (contract §3 rule 6), never new rows.
    let again = w.airs(&[
        "runtime",
        "receipts",
        "upload",
        "--mcp",
        "gw",
        "--from-bulk-state",
        state.to_str().unwrap(),
        "--output",
        "json",
    ]);
    again.assert_success();
    assert_eq!(json_out(&again), v, "{}", again.stdout);
    assert_eq!(
        w.server.requests().len(),
        scans,
        "receipt upload makes no AIRS call"
    );
    // A second bulk job with --record-history while signed in: retained, no warning.
    let csv2 = w.sandbox.tmp().join("results-2.csv");
    let recorded = w.airs(&[
        "runtime",
        "bulk-scan",
        "--profile",
        "monitor",
        "--scanner",
        "monitor",
        "--file",
        prompts.to_str().unwrap(),
        "--output-file",
        csv2.to_str().unwrap(),
        "--record-history",
        "gw",
    ]);
    recorded.assert_success();
    assert!(
        !recorded.stderr.contains("not retained"),
        "{}",
        recorded.stderr
    );
    let scan = w.airs(&[
        "runtime",
        "scan",
        "--profile",
        "monitor",
        "--scanner",
        "monitor",
        "--record-history",
        "gw",
        "Synthetic direct prompt",
    ]);
    scan.assert_success();
    assert!(!scan.stderr.contains("not retained"), "{}", scan.stderr);
    // `--record-history` needs a named scanner (usage, before any tenant read).
    w.airs(&[
        "runtime",
        "scan",
        "--profile",
        "monitor",
        "--record-history",
        "gw",
        "x",
    ])
    .assert_code(2);
    // An archive-only report: zero AIRS calls, a verified private bundle.
    let calls = w.server.requests().len();
    let bundle = w.sandbox.tmp().join("bundle");
    let report = w.airs(&[
        "runtime",
        "report",
        "--mcp",
        "gw",
        "--interval",
        "2",
        "--unit",
        "days",
        "--bundle-dir",
        bundle.to_str().unwrap(),
        "--timeout",
        "60",
        "--output",
        "json",
    ]);
    report.assert_success();
    assert_eq!(
        w.server.requests().len(),
        calls,
        "render makes zero AIRS calls"
    );
    let done = json_out(&report);
    assert_eq!(done["job"]["state"], "succeeded", "{done}");
    let version = done["job"]["result"]["version_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        std::fs::metadata(&bundle).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for name in ["report.html", "evidence.json", "review.md"] {
        assert!(bundle.join(name).is_file(), "{name}");
    }
    let listed = w.airs(&[
        "runtime", "history", "reports", "--mcp", "gw", "--output", "json",
    ]);
    listed.assert_success();
    let manifest = json_out(&listed)["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["version_id"] == version.as_str())
        .cloned()
        .unwrap();
    for f in manifest["files"].as_array().unwrap() {
        let name = f["name"].as_str().unwrap();
        assert_eq!(
            sha256_file(&bundle.join(name)),
            f["sha256"].as_str().unwrap(),
            "{name}"
        );
    }
    let only = w.sandbox.tmp().join("only-review");
    let dl = w.airs(&[
        "runtime",
        "history",
        "download",
        "--mcp",
        "gw",
        "--version-id",
        &version,
        "--file",
        "review.md",
        "--output-dir",
        only.to_str().unwrap(),
        "--output",
        "json",
    ]);
    dl.assert_success();
    let names: Vec<_> = std::fs::read_dir(&only)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, vec!["review.md".to_owned()]);
    // Never overwrite: the directory exists now.
    let clobber = w.airs(&[
        "runtime",
        "history",
        "download",
        "--mcp",
        "gw",
        "--version-id",
        &version,
        "--output-dir",
        only.to_str().unwrap(),
    ]);
    clobber.assert_code(1);
    assert_eq!(std::fs::read_dir(&only).unwrap().count(), 1);
    let query = w.airs(&[
        "runtime",
        "history",
        "query",
        "--mcp",
        "gw",
        "--since",
        "2026-01-01T00:00:00Z",
        "--until",
        &airs_runtime_core::time::iso(airs_runtime_core::time::now_ms()),
        "--group-by",
        "verdict",
        "--output",
        "json",
    ]);
    assert!(
        query.code == 0 || query.stderr.contains("refused"),
        "{}",
        query.stderr
    );
    // `--from-evidence` conflicts with `--mcp` before any tenant or network access.
    let before = w.gw.state.lock().await.authorizes.len();
    w.airs(&[
        "runtime",
        "report",
        "--mcp",
        "gw",
        "--from-evidence",
        "x.json",
    ])
    .assert_code(2);
    w.airs(&["runtime", "report", "--mcp", "gw", "--max-pages", "3"])
        .assert_code(2);
    assert_eq!(w.gw.state.lock().await.authorizes.len(), before);
    assert_eq!(
        w.sessions(),
        1,
        "one private session (the ordinary connection)"
    );
}

/// Contract §3 option B through the CLI: an admin-role connection keeps no session;
/// a lifecycle preview signs in once, `--execute` signs in afresh with the preview's
/// nonce and a fresh-authentication request; ordinary commands refuse the admin
/// connection; an agent-prepared preview is approved by `airs mcp confirm` (its own
/// fresh, nonce-bound sign-in) and executed by the agent with `--approval`; export
/// and import run the whole administrative flow with hash-verified transfers.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn admin_lifecycle_needs_a_fresh_nonce_bound_sign_in_and_keeps_no_session() {
    let w = World::new().await;
    // An archive to maintain: the ordinary connection retains two received results.
    w.set_connection().assert_success();
    w.login().await.assert_success();
    w.scanner();
    let prompts = w.sandbox.tmp().join("prompts.txt");
    std::fs::write(&prompts, "Synthetic sample one\nSynthetic sample two\n").unwrap();
    let csv = w.sandbox.tmp().join("results.csv");
    let seeded = w.airs(&[
        "runtime",
        "bulk-scan",
        "--profile",
        "monitor",
        "--scanner",
        "monitor",
        "--file",
        prompts.to_str().unwrap(),
        "--output-file",
        csv.to_str().unwrap(),
        "--record-history",
        "gw",
    ]);
    seeded.assert_success();
    assert!(!seeded.stderr.contains("not retained"), "{}", seeded.stderr);
    w.set("adm", ADMIN_CLIENT, "admin").assert_success();
    w.set("agent", AGENT_CLIENT, "ordinary").assert_success();
    // An admin connection never serves an ordinary command (no session to use).
    let refused = w.airs(&["runtime", "jobs", "list", "--mcp", "adm"]);
    refused.assert_code(1);
    assert!(
        refused.stderr.contains("Admin connections keep no session"),
        "{}",
        refused.stderr
    );
    // A preview: one sign-in without a proof; nothing changes.
    let preview = w
        .signed(
            &[
                "runtime", "history", "compact", "--mcp", "adm", "--output", "json",
            ],
            1,
        )
        .await;
    preview.assert_success();
    let pv = json_out(&preview);
    let id = pv["preview_id"].as_str().unwrap().to_owned();
    let nonce = pv["proof_nonce"].as_str().unwrap().to_owned();
    // `--execute` on an ordinary connection without an approval: usage, no network.
    let sent = w.gw.state.lock().await.authorizes.len();
    let foreign = w.airs(&[
        "runtime",
        "history",
        "compact",
        "--mcp",
        "agent",
        "--execute",
        &id,
    ]);
    foreign.assert_code(1);
    assert!(
        foreign.stderr.contains("No preview record"),
        "{}",
        foreign.stderr
    );
    assert_eq!(w.gw.state.lock().await.authorizes.len(), sent);
    // Execute: a fresh sign-in carrying exactly this preview's nonce.
    let executed = w
        .signed(
            &[
                "runtime",
                "history",
                "compact",
                "--mcp",
                "adm",
                "--execute",
                &id,
                "--output",
                "json",
            ],
            1,
        )
        .await;
    executed.assert_success();
    assert_eq!(
        json_out(&executed)["job"]["state"],
        "succeeded",
        "{}",
        executed.stdout
    );
    {
        let st = w.gw.state.lock().await;
        let last = st.authorizes.last().unwrap();
        assert_eq!(last.client, ADMIN_CLIENT);
        assert_eq!(last.nonce.as_deref(), Some(nonce.as_str()));
        assert!(last.fresh, "a fresh authentication was requested");
        assert!(
            st.authorizes[sent - 1].nonce.is_none(),
            "the preview sign-in carried no proof"
        );
    }
    // The preview is consumed: no second execution from this record.
    w.airs(&[
        "runtime",
        "history",
        "compact",
        "--mcp",
        "adm",
        "--execute",
        &id,
    ])
    .assert_code(1);
    assert_eq!(
        w.sessions(),
        1,
        "admin sign-ins are never written (one ordinary session)"
    );
    // Agent-prepared: the agent previews (its own session), a human confirms.
    let mut login = spawn(
        &rust(),
        &w.sandbox,
        &["mcp", "login", "agent"],
        &opts(&w.state),
    );
    assert!(login.wait_for_stderr("/idira/authorize", Duration::from_secs(20)));
    let url = login
        .stderr_now()
        .lines()
        .find(|l| l.starts_with("http://"))
        .unwrap()
        .to_owned();
    let nr = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let cb = nr.get(url).send().await.unwrap().headers()["location"]
        .to_str()
        .unwrap()
        .to_owned();
    let _ = reqwest::get(cb).await;
    tokio::task::spawn_blocking(move || login.wait())
        .await
        .unwrap()
        .assert_success();
    let prune = w.airs(&[
        "runtime",
        "history",
        "prune",
        "--mcp",
        "agent",
        "--before",
        "2026-01-01T00:00:00Z",
        "--examples-only",
        "--output",
        "json",
    ]);
    prune.assert_success();
    let pid = json_out(&prune)["preview_id"].as_str().unwrap().to_owned();
    let pnonce = json_out(&prune)["proof_nonce"].as_str().unwrap().to_owned();
    let confirm = w
        .signed(
            &[
                "mcp",
                "confirm",
                &pid,
                "--connection",
                "adm",
                "--output",
                "json",
            ],
            1,
        )
        .await;
    confirm.assert_success();
    let approval = json_out(&confirm)["approval_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        w.gw.state
            .lock()
            .await
            .authorizes
            .last()
            .unwrap()
            .nonce
            .as_deref(),
        Some(pnonce.as_str())
    );
    let run_it = w.airs(&[
        "runtime",
        "history",
        "prune",
        "--mcp",
        "agent",
        "--execute",
        &pid,
        "--approval",
        &approval,
        "--output",
        "json",
    ]);
    run_it.assert_success();
    assert_eq!(
        json_out(&run_it)["job"]["state"],
        "succeeded",
        "{}",
        run_it.stdout
    );
    // M7 review 3 minor: a failed export delivers its result before its failure exit
    // (and removes its receipt), and writes no file.
    let failed_file = w.sandbox.tmp().join("failed-archive.jsonl");
    w.gw.state.lock().await.echo = Some((
        "\"state\":\"succeeded\"".into(),
        "\"state\":\"failed\"".into(),
    ));
    let failed_export = w
        .signed(
            &[
                "runtime",
                "history",
                "export",
                "--mcp",
                "adm",
                "--file",
                failed_file.to_str().unwrap(),
                "--output",
                "json",
            ],
            2,
        )
        .await;
    w.gw.state.lock().await.echo = None;
    failed_export.assert_code(1);
    assert_eq!(
        json_out(&failed_export)["job"]["state"],
        "failed",
        "{}",
        failed_export.stderr
    );
    assert!(
        failed_export.stderr.contains("ended failed"),
        "{}",
        failed_export.stderr
    );
    assert!(!failed_file.exists());
    let adm_receipts = std::fs::read_dir(w.state.join("airs-cli/mcp/receipts/adm"))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
                .count()
        })
        .unwrap_or(0);
    assert_eq!(
        adm_receipts, 0,
        "the delivered failed result removes its receipt"
    );
    // Export: preview sign-in, then a nonce-bound sign-in; the file is verified.
    let file = w.sandbox.tmp().join("archive.jsonl");
    let export = w
        .signed(
            &[
                "runtime",
                "history",
                "export",
                "--mcp",
                "adm",
                "--file",
                file.to_str().unwrap(),
                "--output",
                "json",
            ],
            2,
        )
        .await;
    export.assert_success();
    let ex = json_out(&export);
    assert_eq!(
        sha256_file(&file),
        ex["job"]["result"]["sha256"].as_str().unwrap(),
        "{ex}"
    );
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    // A second export never overwrites, and is refused before any sign-in.
    let signs = w.gw.state.lock().await.authorizes.len();
    let again = w.airs(&[
        "runtime",
        "history",
        "export",
        "--mcp",
        "adm",
        "--file",
        file.to_str().unwrap(),
    ]);
    again.assert_code(1);
    assert_eq!(w.gw.state.lock().await.authorizes.len(), signs);
    // Import of the export: the staged upload (one nonce-bound begin) and the import
    // (its own nonce-bound execute).
    let endpoints = {
        let base = w.server.base_url().trim_end_matches('/').to_owned();
        [
            format!("{base}{}", airs_test_harness::paths::MGMT_PREFIX),
            format!("{base}/dashboard"),
            format!("{base}{}", airs_test_harness::paths::TOKEN),
        ]
    };
    let (env, _) = airs_runtime_core::cloud::ids("100", &endpoints).unwrap();
    let import_args = [
        "runtime",
        "history",
        "import",
        "--mcp",
        "adm",
        "--file",
        file.to_str().unwrap(),
        "--expect-tsg",
        "100",
        "--expect-environment",
        &env,
        "--output",
        "json",
    ];
    // M7 review 1 finding 6: a failure after staging but before execution (here the
    // fresh execute sign-in is declined) aborts the staged upload.
    {
        let mut st = w.gw.state.lock().await;
        st.deny_authorize_after = Some(st.authorize_calls + 2);
        st.aborts = 0;
    }
    let declined = w.signed(&import_args, 3).await;
    declined.assert_code(1);
    {
        let mut st = w.gw.state.lock().await;
        assert_eq!(
            st.aborts, 1,
            "the staged upload is aborted: {}",
            declined.stderr
        );
        st.deny_authorize_after = None;
        // M7 review 2 finding 3: a submission that fails before anything is sent
        // (here its epoch read) aborts the staged upload too.
        st.aborts = 0;
        st.fail_epoch_after_finalize = true;
    }
    let unsent = w.signed(&import_args, 3).await;
    unsent.assert_code(1);
    {
        let st = w.gw.state.lock().await;
        assert_eq!(
            st.aborts, 1,
            "an unsent execution aborts the upload: {}",
            unsent.stderr
        );
        assert!(!st.epoch_armed, "the epoch read failed once");
    }
    let import = w.signed(&import_args, 3).await;
    import.assert_success();
    assert_eq!(
        json_out(&import)["job"]["state"],
        "succeeded",
        "{}",
        import.stdout
    );
    let st = w.gw.state.lock().await;
    let proofs = st
        .authorizes
        .iter()
        .filter(|g| g.client == ADMIN_CLIENT && g.nonce.is_some())
        .count();
    // compact --execute, confirm, the failed export, export, the declined import's
    // upload begin, the unsent import's upload begin and execute, and the completed
    // import's upload begin and execute.
    assert_eq!(
        proofs, 9,
        "every administrative mutation had its own proof sign-in"
    );
    assert_eq!(
        w.sessions(),
        2,
        "only the two ordinary sessions were written"
    );
}

/// Contract §7.4: one bounded authentication recovery. When the gateway stops
/// accepting the cached access token (revoked or expired early), a submission refreshes
/// once and replays the *same* idempotency key: exactly one job, its receipt keeps the
/// key, and no token is ever printed. A second failure after the refresh is reported,
/// never retried again; reads simply fail (no replay is needed for them).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rejected_token_is_refreshed_once_and_the_submission_replayed_with_its_key() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    w.gw.state.lock().await.access.clear();
    let sync = w.airs(&[
        "runtime",
        "sync",
        "--mcp",
        "gw",
        "--no-wait",
        "--output",
        "json",
    ]);
    sync.assert_success();
    let job = json_out(&sync)["job"]["job_id"]
        .as_str()
        .unwrap()
        .to_owned();
    {
        let st = w.gw.state.lock().await;
        assert_eq!(st.refreshes, 1, "one refresh");
        for token in &st.issued {
            assert!(!sync.stdout.contains(token.as_str()) && !sync.stderr.contains(token.as_str()));
        }
    }
    let jobs = w.airs(&["runtime", "jobs", "list", "--mcp", "gw", "--output", "json"]);
    jobs.assert_success();
    let items = json_out(&jobs)["items"].as_array().unwrap().clone();
    assert_eq!(items.len(), 1, "the replay joined the same key: one job");
    assert_eq!(items[0]["job_id"], job.as_str());
    // Refresh tokens revoked too: the one recovery fails at the epoch read, before any
    // key is minted or anything submitted; no sign-in is started.
    {
        let mut st = w.gw.state.lock().await;
        st.access.clear();
        st.refresh.clear();
    }
    let failed = w.airs(&["runtime", "sync", "--mcp", "gw", "--no-wait"]);
    failed.assert_code(1);
    assert_eq!(
        w.gw.state.lock().await.authorize_calls,
        1,
        "never a new sign-in"
    );
    let receipts = std::fs::read_dir(w.state.join("airs-cli/mcp/receipts/gw"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
        .count();
    assert_eq!(
        receipts, 1,
        "only the first (--no-wait, undelivered) submission's receipt"
    );
    let status = w.airs(&["mcp", "status", "gw", "--output", "json"]);
    status.assert_success();
    assert_eq!(
        json_out(&status)["gw"]["unresolved"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

/// Contract §7.3: binaries before M7 refuse a tenant file with an `mcp` table rather
/// than misread it. `AIRS_PRE_M7_BIN` names a frozen pre-M7 `airs` (built by
/// `scripts/build-pre-m7-cli.sh` from the 8.0.0 release commit); without it the
/// test reports that it did not run.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_frozen_pre_m7_binary_refuses_the_mcp_table() {
    let Some(bin) = std::env::var_os("AIRS_PRE_M7_BIN") else {
        // A qualification run sets AIRS_REQUIRE_PRE_M7=1, so a missing binary fails
        // rather than passing unrun (M7 review 4 minor).
        assert!(
            std::env::var_os("AIRS_REQUIRE_PRE_M7").is_none(),
            "AIRS_REQUIRE_PRE_M7 is set but AIRS_PRE_M7_BIN is not"
        );
        eprintln!("PRE-M7 SKIPPED: set AIRS_PRE_M7_BIN to a frozen pre-M7 airs binary");
        return;
    };
    eprintln!("PRE-M7 RAN: {}", std::path::Path::new(&bin).display());
    let w = World::new().await;
    w.set_connection().assert_success();
    let tenant = w.sandbox.store_tenant_path("prod");
    let before = std::fs::read(&tenant).unwrap();
    let old = Cli::rust(std::path::PathBuf::from(bin));
    let out = run(
        &old,
        &w.sandbox,
        &["tenant", "read", "prod"],
        &RunOptions::new(),
    );
    assert_eq!(out.code, 1, "{}", out.stderr);
    // 8.0.0's strict tenant parser refuses the unknown table with its generic message.
    assert!(
        out.stderr.contains("Cannot read a valid tenant config"),
        "{}",
        out.stderr
    );
    assert_eq!(
        std::fs::read(&tenant).unwrap(),
        before,
        "the old binary wrote nothing"
    );
}

/// Plan M7.4 (packaged CLI installs): every packaging path ships only the `airs`
/// executable and its two names, whose MCP client is built in; never the `airs-mcp`
/// server, which is a separate deployment (M8). The built `airs` lists the client.
#[test]
fn packages_carry_the_client_in_airs_and_never_the_server() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for script in [
        "scripts/release.sh",
        "scripts/package-release.sh",
        "scripts/npm-package.sh",
        "scripts/homebrew-formula.sh",
    ] {
        let text = std::fs::read_to_string(root.join(script)).unwrap();
        assert!(
            !text.contains("airs-mcp"),
            "{script} must not package the server"
        );
    }
    let release = std::fs::read_to_string(root.join("scripts/release.sh")).unwrap();
    assert!(
        release.contains("BIN=\"airs\""),
        "a release builds the airs executable"
    );
    let help = std::process::Command::new(env!("CARGO_BIN_EXE_airs"))
        .args(["mcp", "--help"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&help.stdout);
    for word in ["connections", "login", "logout", "status", "confirm"] {
        assert!(text.contains(word), "{word}: {text}");
    }
}

/// M7 review 1 finding 1: the CLI registers every MCP token it obtains with the
/// invocation's secret set. A service (or hop) that echoes the caller's access token in a
/// refusal or in a result never gets it printed: the refusal text is withheld and the
/// result is refused for display.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_echoed_mcp_token_is_never_printed() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    let tokens = |st: &As| st.issued.clone();
    // A refusal whose code carries the token.
    w.gw.state.lock().await.echo = Some(("\"not_found\"".into(), "\"{bearer}\"".into()));
    let refusal = w.airs(&[
        "runtime",
        "jobs",
        "get",
        "--mcp",
        "gw",
        "j1_0a1b2c3d_00000000-0000-4000-8000-000000000000",
    ]);
    refusal.assert_code(1);
    assert!(refusal.stderr.contains("refused"), "{}", refusal.stderr);
    // A result that carries the token.
    w.gw.state.lock().await.echo = Some(("\"items\":[]".into(), "\"items\":[\"{bearer}\"]".into()));
    let result = w.airs(&["runtime", "jobs", "list", "--mcp", "gw", "--output", "json"]);
    result.assert_code(1);
    assert!(
        result.stderr.contains("cannot be safely displayed"),
        "{}",
        result.stderr
    );
    let st = w.gw.state.lock().await;
    assert!(!tokens(&st).is_empty());
    for token in tokens(&st) {
        for out in [&refusal, &result] {
            assert!(
                !out.stdout.contains(token.as_str()) && !out.stderr.contains(token.as_str()),
                "an MCP token was printed"
            );
        }
    }
}

fn receipt_count(w: &World) -> usize {
    std::fs::read_dir(w.state.join("airs-cli/mcp/receipts/gw"))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
                .count()
        })
        .unwrap_or(0)
}

/// M7 review 1 finding 2: a submission's receipt is removed only after its terminal
/// result reached stdout. A closed stdout keeps the receipt (the result was never
/// received); a `--strict` partial report writes its result and then exits 1 with the
/// receipt removed (the result is not discarded by the failure exit).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_receipt_outlives_an_undelivered_result_and_strict_exits_keep_the_result() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    let sync = [
        "runtime",
        "sync",
        "--mcp",
        "gw",
        "--timeout",
        "60",
        "--output",
        "json",
    ];
    let closed = run(
        &rust(),
        &w.sandbox,
        &sync,
        &opts(&w.state).close_stdout_after(0),
    );
    assert_eq!(
        closed.code, 0,
        "a closed pipe is success: {}",
        closed.stderr
    );
    assert_eq!(
        receipt_count(&w),
        1,
        "the undelivered result keeps its receipt"
    );
    let delivered = w.airs(&sync);
    delivered.assert_success();
    assert_eq!(json_out(&delivered)["job"]["state"], "succeeded");
    assert_eq!(
        receipt_count(&w),
        1,
        "a delivered result removes only its own receipt"
    );
    // The report must be partial for `--strict` to fail. Its one-day period ends at
    // submission, and the archive's coverage ends at the last collection (the sync
    // above), so the tail between them is uncovered — but only when it spans a second:
    // within the same second the period is fully covered and the report is complete.
    // Before application-telemetry left inventory coverage (contract §4.3) that source
    // was unavailable for a one-day window and made the report partial regardless;
    // since then this test passed only when the sync and the report straddled a
    // second boundary (about half the runs). Leave a two-second uncovered tail.
    std::thread::sleep(std::time::Duration::from_secs(2));
    let strict = w.airs(&[
        "runtime",
        "report",
        "--mcp",
        "gw",
        "--interval",
        "1",
        "--unit",
        "days",
        "--strict",
        "--timeout",
        "60",
        "--output",
        "json",
    ]);
    let done = json_out(&strict);
    assert_eq!(done["job"]["result"]["completeness"], "partial", "{done}");
    strict.assert_code(1);
    assert!(strict.stderr.contains("--strict"), "{}", strict.stderr);
    assert_eq!(
        receipt_count(&w),
        1,
        "the strict exit still delivered its result"
    );
}

/// M7 review 1 finding 3: a schedule change is a keyed mutation like any submission. It
/// keeps a durable receipt until its result is delivered, and a rejected token is
/// refreshed once and the change replayed with the same key (one schedule).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn schedule_changes_keep_receipts_and_recover_authentication_once() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    let set = [
        "runtime",
        "schedules",
        "set",
        "--mcp",
        "gw",
        "--kind",
        "collection",
        "--enabled",
        "true",
        "--effective-at",
        "2026-11-06T00:00:00Z",
        "--interval-minutes",
        "15",
        "--output",
        "json",
    ];
    w.gw.state.lock().await.access.clear();
    let recovered = w.airs(&set);
    recovered.assert_success();
    assert_eq!(w.gw.state.lock().await.refreshes, 1, "one refresh");
    assert_eq!(
        w.gw.state.lock().await.authorize_calls,
        1,
        "never a new sign-in"
    );
    assert_eq!(
        receipt_count(&w),
        0,
        "a delivered change removes its receipt"
    );
    let closed = run(
        &rust(),
        &w.sandbox,
        &set,
        &opts(&w.state).close_stdout_after(0),
    );
    assert_eq!(closed.code, 0, "{}", closed.stderr);
    assert_eq!(
        receipt_count(&w),
        1,
        "an undelivered change keeps its receipt"
    );
    let listed = w.airs(&[
        "runtime",
        "schedules",
        "list",
        "--mcp",
        "gw",
        "--output",
        "json",
    ]);
    listed.assert_success();
    let schedules = json_out(&listed)["schedules"].as_array().unwrap().clone();
    assert_eq!(schedules.len(), 1, "{schedules:?}");
}

/// M7 review 1 finding 4: the wait's deadline covers the outstanding poll. A `jobs_get`
/// that stalls at the hop cannot hold `--timeout 2` open: the command ends near its
/// deadline with the job ID, and the receipt stays for the undelivered result.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stalled_poll_cannot_outlast_the_wait_deadline() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    w.gw.state.lock().await.stall_polls = Some(std::time::Duration::from_secs(60));
    let started = std::time::Instant::now();
    let out = w.airs(&["runtime", "sync", "--mcp", "gw", "--timeout", "2"]);
    let took = started.elapsed();
    out.assert_code(1);
    assert!(took < std::time::Duration::from_secs(20), "took {took:?}");
    assert!(out.stderr.contains("after 2 seconds"), "{}", out.stderr);
    assert!(out.stderr.contains("(job j1_"), "{}", out.stderr);
    assert_eq!(receipt_count(&w), 1);
}

/// M7 review 1 finding 5: single-file report retrieval requires the manifest's hash and
/// length. A hop that strips the manifest hash cannot get an unverified file published.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_single_report_file_is_not_published_without_its_manifest_hash() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    // An archive to report on.
    w.airs(&["runtime", "sync", "--mcp", "gw", "--timeout", "60"])
        .assert_success();
    w.gw.state.lock().await.echo = Some(("\"sha256\":".into(), "\"sha256_withheld\":".into()));
    let file = w.sandbox.tmp().join("report.html");
    let out = w.airs(&[
        "runtime",
        "report",
        "--mcp",
        "gw",
        "--interval",
        "1",
        "--unit",
        "days",
        "--output-file",
        file.to_str().unwrap(),
        "--timeout",
        "60",
    ]);
    out.assert_code(1);
    assert!(out.stderr.contains("no valid hash"), "{}", out.stderr);
    assert!(!file.exists(), "nothing was published");
}

/// M7 review 1 finding 7: ranges are validated before any tenant access, with checked
/// arithmetic. An overflowing interval and a reversed range are usage errors (exit 2),
/// even with no tenant configured (which would otherwise exit 1 at tenant loading).
#[test]
fn invalid_report_and_history_ranges_are_usage_errors_before_tenant_access() {
    let sandbox = Sandbox::new();
    let state = sandbox.tmp().join("xdg-state");
    let o = opts(&state);
    let report = ["runtime", "report", "--mcp", "gw"];
    let cases: [(&str, Vec<&str>); 5] = [
        (
            "at most 90 days",
            vec!["--interval", "9223372036854775807", "--unit", "days"],
        ),
        (
            "at most 90 days",
            vec!["--interval", "91", "--unit", "days"],
        ),
        (
            "before --until",
            vec![
                "--since",
                "2026-11-06T00:00:00Z",
                "--until",
                "2026-11-05T00:00:00Z",
            ],
        ),
        (
            "at most 90 days",
            vec![
                "--since",
                "2026-01-01T00:00:00Z",
                "--until",
                "2026-11-05T00:00:00Z",
            ],
        ),
        (
            "before --until",
            vec![
                "--since",
                "2026-11-06T00:00:00Z",
                "--until",
                "2026-11-06T00:00:00Z",
            ],
        ),
    ];
    for (n, (want, tail)) in cases.into_iter().enumerate() {
        // The last case goes through a history command's range.
        let head: Vec<&str> = if n == 4 {
            vec!["runtime", "history", "coverage", "--mcp", "gw"]
        } else {
            report.to_vec()
        };
        let args: Vec<&str> = head.into_iter().chain(tail).collect();
        let out = run(&rust(), &sandbox, &args, &o);
        assert_eq!(out.code, 2, "{args:?}: {}", out.stderr);
        assert!(out.stderr.contains(want), "{args:?}: {}", out.stderr);
    }
}

/// M7 review 1 minor: the recovery after admission. The service admits the submission
/// but its response is lost at the hop; the CLI refreshes once and replays the original
/// key, which joins the admitted job (one job), and the delivered result removes the
/// receipt.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lost_response_after_admission_replays_the_same_key() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    w.gw.state.lock().await.lose_next_submission = true;
    let sync = w.airs(&[
        "runtime",
        "sync",
        "--mcp",
        "gw",
        "--no-wait",
        "--output",
        "json",
    ]);
    sync.assert_success();
    let job = json_out(&sync)["job"]["job_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        !w.gw.state.lock().await.lose_next_submission,
        "the response was lost once"
    );
    assert_eq!(w.gw.state.lock().await.refreshes, 1, "one refresh");
    let keys = w.gw.state.lock().await.submission_keys.clone();
    assert_eq!(keys.len(), 2, "the submission and its one replay: {keys:?}");
    assert_eq!(keys[0], keys[1], "the replay carries the original key");
    let jobs = w.airs(&["runtime", "jobs", "list", "--mcp", "gw", "--output", "json"]);
    jobs.assert_success();
    let items = json_out(&jobs)["items"].as_array().unwrap().clone();
    assert_eq!(
        items.len(),
        1,
        "the replay joined the admitted job: {items:?}"
    );
    assert_eq!(items[0]["job_id"], job.as_str());
}

/// M7 review 2 finding 1: a job ID is validated before it is stored or shown. A hop that
/// echoes the caller's token as the job ID gets an "invalid job ID" failure. The token
/// is in no output and in no receipt file, and the final failure is filtered on an
/// MCP-only invocation.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_token_echoed_as_a_job_id_is_never_stored_or_shown() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    w.gw.state.lock().await.echo =
        Some(("\"job_id\":\"j1_".into(), "\"job_id\":\"{bearer}j1_".into()));
    let out = w.airs(&["runtime", "sync", "--mcp", "gw", "--timeout", "2"]);
    out.assert_code(1);
    assert!(out.stderr.contains("invalid job ID"), "{}", out.stderr);
    let dir = w.state.join("airs-cli/mcp/receipts/gw");
    let receipts: Vec<String> = std::fs::read_dir(&dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(receipts.len(), 1, "the key-only receipt is kept");
    let st = w.gw.state.lock().await;
    for token in &st.issued {
        assert!(!out.stdout.contains(token.as_str()) && !out.stderr.contains(token.as_str()));
        assert!(
            receipts.iter().all(|r| !r.contains(token.as_str())),
            "a token was persisted"
        );
    }
}

/// M7 review 2 finding 2: `--record-history` never sends a credential. The scanner's
/// API key has UUID syntax and AIRS answers with it as the scan ID. stdout withholds it,
/// the receipt carrying it is withheld from the batch with a warning, and the key never
/// reaches the service.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn record_history_never_sends_a_credential_returned_as_a_scan_id() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    // The CLI lowercases scan IDs: an uppercase credential must be caught too (M7
    // review 3 finding 1).
    for key in [
        "0f0e0d0c-0b0a-4908-8706-050403020100",
        "0F0E0D0C-0B0A-4908-8706-0504030201FF",
    ] {
        let input = format!(
            "applicationId = \"app-example\"\nendpoint = {}\napiKey = {}\n",
            json!(w.server.url("/scan")),
            json!(key)
        );
        run(
            &rust(),
            &w.sandbox,
            &[
                "tenant",
                "scanners",
                "set",
                "prod",
                "monitor",
                "--stdin",
                "--replace",
            ],
            &opts(&w.state).stdin(input),
        )
        .assert_success();
        w.server.set_default(
            airs_test_harness::Route::ScanSync,
            airs_test_harness::Planned::json(
                200,
                &json!({"report_id": "R-1", "scan_id": key, "category": "benign", "action": "allow",
                    "timeout": false, "error": false, "errors": [],
                    "prompt_detected": {"agent": false, "source_code": false, "malicious_code": false,
                        "url_cats": false, "dlp": false, "toxic_content": false, "injection": false,
                        "topic_violation": false}}),
            ),
        );
        {
            let mut st = w.gw.state.lock().await;
            st.watch = Some(key.to_ascii_lowercase());
            st.watch_hits = 0;
        }
        let scan = run(
            &rust(),
            &w.sandbox,
            &[
                "runtime",
                "scan",
                "--profile",
                "monitor",
                "--scanner",
                "monitor",
                "--record-history",
                "gw",
                "Synthetic direct prompt",
            ],
            &opts(&w.state).secret(key),
        );
        scan.assert_success();
        assert!(
            scan.stderr
                .contains("withheld because they carry a credential"),
            "{key}: {}",
            scan.stderr
        );
        assert_eq!(
            w.gw.state.lock().await.watch_hits,
            0,
            "{key}: the credential reached the service"
        );
    }
}

/// M7 review 3 finding 2: a token inside a well-formed job ID (as its UUID part) is
/// never persisted, so a later invocation that knows nothing of that token cannot
/// print it from a receipt.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_token_inside_a_valid_job_id_is_never_persisted_for_a_later_status() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    w.gw.state.lock().await.echo = Some((
        "\"job_id\":\"j1_".into(),
        "\"job_id\":\"j1_00000000_{bearer}\",\"was\":\"".into(),
    ));
    let sync = w.airs(&["runtime", "sync", "--mcp", "gw", "--no-wait"]);
    sync.assert_code(1);
    assert!(sync.stderr.contains("invalid job ID"), "{}", sync.stderr);
    w.gw.state.lock().await.echo = None;
    let status = w.airs(&["mcp", "status", "gw", "--output", "json"]);
    status.assert_success();
    let st = w.gw.state.lock().await;
    for token in &st.issued {
        for out in [&sync, &status] {
            assert!(
                !out.stdout.contains(token.as_str()) && !out.stderr.contains(token.as_str()),
                "a token was shown"
            );
        }
    }
}

/// M7 review 3 minor: the final failure of an MCP-only invocation is filtered. The
/// service reports the caller's token as the job state, and the timeout diagnostic,
/// which names that state, never prints it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_timeout_diagnostic_never_prints_a_registered_token() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    w.gw.state.lock().await.echo = Some(("\"state\":\"".into(), "\"state\":\"{bearer}".into()));
    let out = w.airs(&["runtime", "sync", "--mcp", "gw", "--timeout", "2"]);
    out.assert_code(1);
    // The whole failure is withheld: it carried the token.
    assert!(out.stderr.contains("[withheld]"), "{}", out.stderr);
    let st = w.gw.state.lock().await;
    for token in &st.issued {
        assert!(
            !out.stdout.contains(token.as_str()) && !out.stderr.contains(token.as_str()),
            "{}",
            out.stderr
        );
    }
}

/// M7 review 3 minor: an authentication refusal at the submission itself (after a
/// successful epoch read) is recovered once: one refresh, and the replay carries the
/// original key.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_submission_time_authentication_refusal_replays_the_same_key() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    w.gw.state.lock().await.refuse_next_submission = true;
    let sync = w.airs(&[
        "runtime",
        "sync",
        "--mcp",
        "gw",
        "--no-wait",
        "--output",
        "json",
    ]);
    sync.assert_success();
    let st = w.gw.state.lock().await;
    assert!(
        !st.refuse_next_submission,
        "the submission was refused once"
    );
    assert_eq!(st.refreshes, 1, "one refresh");
    assert_eq!(st.submission_keys.len(), 2, "{:?}", st.submission_keys);
    assert_eq!(
        st.submission_keys[0], st.submission_keys[1],
        "the original key"
    );
}

/// M7 review 4 finding: a preview record never stores a secret, and a later invocation
/// never shows an unsafe one. The service returns the caller's token inside the
/// preview's expiry: nothing is recorded. A record already on disk with a token in a
/// field is refused by a fresh `mcp confirm`, which never signs in and never prints it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_preview_carrying_a_token_is_never_recorded_or_shown_later() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    w.set("adm", ADMIN_CLIENT, "admin").assert_success();
    w.gw.state.lock().await.echo = Some((
        "\"expires_at\":\"".into(),
        "\"expires_at\":\"{bearer}".into(),
    ));
    let preview = w
        .signed(
            &[
                "runtime", "history", "compact", "--mcp", "adm", "--output", "json",
            ],
            1,
        )
        .await;
    w.gw.state.lock().await.echo = None;
    preview.assert_code(1);
    assert!(
        preview.stderr.contains("invalid preview"),
        "{}",
        preview.stderr
    );
    let dir = w.state.join("airs-cli/mcp/previews/adm");
    let recorded = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0);
    assert_eq!(recorded, 0, "nothing was recorded");
    // A record already on disk with a token in its expiry (current format): refused
    // before it is shown or used.
    let token = w.gw.state.lock().await.issued[0].clone();
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .unwrap();
    let plant = |format: &str, id: &str, expires: &str| {
        let file = dir.join(format!("{id}.json"));
        std::fs::write(
            &file,
            serde_json::to_vec(&json!({"format": format, "previewId": id,
                "action": "compact", "parameterDigest": "a".repeat(64), "proofNonce": "b".repeat(32),
                "expiresAt": expires, "connectionRevision": "x"}))
            .unwrap(),
        )
        .unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    };
    let id = "p1_0a1b2c3d_00000000-0000-4000-8000-000000000001";
    plant("airs-mcp-preview/2", id, &token);
    let signs = w.gw.state.lock().await.authorizes.len();
    let confirm = w.airs(&["mcp", "confirm", "--connection", "adm", id]);
    confirm.assert_code(1);
    assert!(
        confirm.stderr.contains("local preview record is invalid"),
        "{}",
        confirm.stderr
    );
    // M7 review 5: a legacy (version 1) record is never read, even when every field is
    // well formed and the token sits inside a valid preview ID.
    let legacy = format!("p1_0a1b2c3d_{token}");
    plant("airs-mcp-preview/1", &legacy, "2026-11-07T00:00:00Z");
    let old = w.airs(&["mcp", "confirm", "--connection", "adm", &legacy]);
    old.assert_code(2);
    assert!(
        old.stderr.contains("No local record of this preview"),
        "{}",
        old.stderr
    );
    assert_eq!(
        w.gw.state.lock().await.authorizes.len(),
        signs,
        "no sign-in"
    );
    // A new preview whose ID carries the token in a valid shape is never recorded.
    w.gw.state.lock().await.echo = Some((
        "\"preview_id\":\"p1_".into(),
        "\"preview_id\":\"p1_0a1b2c3d_{bearer}\",\"was\":\"".into(),
    ));
    let shaped = w
        .signed(
            &[
                "runtime", "history", "compact", "--mcp", "adm", "--output", "json",
            ],
            1,
        )
        .await;
    w.gw.state.lock().await.echo = None;
    shaped.assert_code(1);
    assert!(
        shaped.stderr.contains("invalid preview"),
        "{}",
        shaped.stderr
    );
    let recorded: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        recorded.len(),
        2,
        "only the two planted records: {recorded:?}"
    );
    let st = w.gw.state.lock().await;
    for t in &st.issued {
        for out in [&preview, &confirm, &old, &shaped] {
            assert!(
                !out.stdout.contains(t.as_str()) && !out.stderr.contains(t.as_str()),
                "a token was shown"
            );
        }
    }
}

/// M7 review 6 finding: a legacy (version 1) receipt predates the job-ID secret guard,
/// so a later `mcp status` keeps it listed as unresolved (with its key), but never shows
/// its server-derived job ID. A version-2 receipt, written under the guard, shows its job.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_legacy_receipt_stays_unresolved_but_never_shows_its_job_id() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    // A current receipt: an undelivered --no-wait submission.
    let sync = w.airs(&[
        "runtime",
        "sync",
        "--mcp",
        "gw",
        "--no-wait",
        "--output",
        "json",
    ]);
    sync.assert_success();
    let job = json_out(&sync)["job"]["job_id"]
        .as_str()
        .unwrap()
        .to_owned();
    // A legacy receipt an earlier, unguarded version wrote: the token as its job's UUID.
    let token = w.gw.state.lock().await.issued[0].clone();
    let key = format!("cli-{}", "c".repeat(32));
    let dir = w.state.join("airs-cli/mcp/receipts/gw");
    let file = dir.join(format!("{key}.json"));
    std::fs::write(
        &file,
        serde_json::to_vec(
            &json!({"format": "airs-mcp-op/1", "operation": "collection_start",
            "paramsDigest": "d".repeat(64), "idempotencyKey": key, "epoch": "0a1b2c3d",
            "createdAt": 1, "jobId": format!("j1_0a1b2c3d_{token}"), "connectionRevision": "x"}),
        )
        .unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let status = w.airs(&["mcp", "status", "gw", "--output", "json"]);
    status.assert_success();
    assert!(!status.stdout.contains(token.as_str()) && !status.stderr.contains(token.as_str()));
    let unresolved = json_out(&status)["gw"]["unresolved"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(
        unresolved.len(),
        2,
        "both submissions stay unresolved: {unresolved:?}"
    );
    let legacy = unresolved.iter().find(|r| r["legacy"] == true).unwrap();
    assert_eq!(
        legacy["idempotencyKey"],
        key.as_str(),
        "its key is kept for reconciliation"
    );
    assert!(legacy["jobId"].is_null(), "its job ID is withheld");
    let current = unresolved.iter().find(|r| r["legacy"] == false).unwrap();
    assert_eq!(current["jobId"], job.as_str());
}

/// M7 review 7 finding 1: a verified transfer never writes a registered secret. The
/// report's title is the caller's own access token, so the service renders it into
/// the HTML; the hash-verified single-file download is refused and nothing is written.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_transferred_file_carrying_a_token_is_never_published() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    w.airs(&["runtime", "sync", "--mcp", "gw", "--timeout", "60"])
        .assert_success();
    let token = {
        let st = w.gw.state.lock().await;
        st.access.keys().next().unwrap().clone()
    };
    let file = w.sandbox.tmp().join("report.html");
    let out = w.airs(&[
        "runtime",
        "report",
        "--mcp",
        "gw",
        "--interval",
        "1",
        "--unit",
        "days",
        "--title",
        &token,
        "--output-file",
        file.to_str().unwrap(),
        "--timeout",
        "60",
    ]);
    out.assert_code(1);
    assert!(
        out.stderr.contains("carries a credential"),
        "{}",
        out.stderr
    );
    assert!(!file.exists(), "nothing was published");
    assert!(!out.stdout.contains(token.as_str()) && !out.stderr.contains(token.as_str()));
}

/// M7 review 7 finding 2: `mcp status` never presents a partial receipt inventory as
/// complete. A receipt directory that is no longer private, or a receipt that cannot
/// be parsed, fails the command, and every receipt is kept.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn status_refuses_an_incomplete_receipt_inventory() {
    let w = World::new().await;
    w.set_connection().assert_success();
    w.login().await.assert_success();
    w.airs(&["runtime", "sync", "--mcp", "gw", "--no-wait"])
        .assert_success();
    let dir = w.state.join("airs-cli/mcp/receipts/gw");
    assert_eq!(receipt_count(&w), 1);
    // A malformed receipt beside the valid one.
    let bad = dir.join(format!("cli-{}.json", "e".repeat(32)));
    std::fs::write(&bad, b"{not a receipt").unwrap();
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o600)).unwrap();
    let malformed = w.airs(&["mcp", "status", "gw", "--output", "json"]);
    malformed.assert_code(1);
    assert!(
        malformed.stderr.contains("cannot all be read"),
        "{}",
        malformed.stderr
    );
    std::fs::remove_file(&bad).unwrap();
    w.airs(&["mcp", "status", "gw", "--output", "json"])
        .assert_success();
    // The directory loosened to 0770.
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o770)).unwrap();
    let loose = w.airs(&["mcp", "status", "gw", "--output", "json"]);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    loose.assert_code(1);
    assert!(
        loose.stderr.contains("cannot all be read"),
        "{}",
        loose.stderr
    );
    // M7 review 8 finding 2: an ancestor without search permission is not "absent".
    let parent = w.state.join("airs-cli/mcp/receipts");
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o600)).unwrap();
    let hidden = w.airs(&["mcp", "status", "gw", "--output", "json"]);
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    hidden.assert_code(1);
    assert!(
        hidden.stderr.contains("cannot all be read"),
        "{}",
        hidden.stderr
    );
    assert_eq!(receipt_count(&w), 1, "the receipt is kept");
}
