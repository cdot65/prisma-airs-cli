//! Loopback end-to-end: CLI (PKCE login, private cache, refresh, noninteractive
//! calls, chunked download) → fixture gateway + authorization server → the real
//! spike backend. The fixture gateway models the deployed hop: it validates the
//! CLI's gateway token, then presents an Entra-shaped upstream token plus a signed
//! identity assertion. Loopback only; this cannot pass the M0.6 live gate.

use std::collections::HashMap;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

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
    "/../airs-mcp-spike/tests/fixtures"
);
const BIN: &str = env!("CARGO_BIN_EXE_airs-mcp-spike-cli");
const TID: &str = "00000000-0000-4000-8000-0000000000aa";
const API: &str = "00000000-0000-4000-8000-0000000000ab";
const GW_CLIENT: &str = "00000000-0000-4000-8000-0000000000ac";
const OWNER: &str = "00000000-0000-4000-8000-0000000000ad";
const BACKEND: &str = "https://mcp-backend-airs.example.test/mcp";
const CLI_CLIENT: &str = "airs-cli-public";

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

fn private_temp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("airs-mcp-e2e-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new().mode(0o700).create(&d).unwrap();
    d.canonicalize().unwrap()
}

async fn backend() -> String {
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
    let data = private_temp("data");
    let text = format!(
        r#"revision = "e2e-1"
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
accepted_permissions = ["AirsHistory.Read"]
allowed_client_ids = ["{GW_CLIENT}"]
owner_object_ids = ["{OWNER}"]
[assertion]
header = "x-user-jwt"
issuer = "https://mcp.example.test"
audiences = ["{BACKEND}"]
algorithms = ["RS256"]
jwks_url = "http://{idp}/gw/jwks.json"
max_lifetime_seconds = 300
workspace_ids = ["ws-test"]
organization_ids = ["org-test"]
[[grants]]
subject = "human-a"
client_id = "{CLI_CLIENT}"
owner_object_id = "{OWNER}"
permission = "reader"
tools = ["service_info", "artifact_chunk"]
"#,
        data = data.display()
    );
    let cfg = Arc::new(airs_mcp_spike::config::parse(&text).unwrap());
    let admitted = airs_mcp_spike::storage::admit(&cfg.server.data_dir).unwrap();
    let spike = airs_mcp_spike::tools::Spike {
        config: cfg.clone(),
        storage: Some((
            admitted,
            airs_mcp_spike::storage::OwnerRecord {
                owner_id: "e2e".into(),
                pod: "e2e".into(),
                node: "e2e".into(),
                pid: 0,
                acquired_at_unix: 0,
            },
        )),
        state: Arc::new(Mutex::new(Default::default())),
        artifact_sha256: Arc::new(airs_mcp_spike::tools::artifact_sha256()),
    };
    let addr = serve(airs_mcp_spike::run::router(cfg, spike).unwrap()).await;
    format!("http://{addr}/mcp")
}

#[derive(Default)]
struct As {
    codes: HashMap<String, (String, String, String)>, // code -> (challenge, redirect, resource)
    access: HashMap<String, ()>,
    refresh: HashMap<String, ()>,
    refreshes: usize,
    rejected_refreshes: usize,
    authorize_calls: usize,
    wrong_iss: bool,
    wrong_state: bool,
    foreign_prm: bool,
    foreign_as: bool,
    refresh_delay_ms: u64,
}

#[derive(Clone)]
struct Gw {
    base: String,
    backend: String,
    state: Arc<Mutex<As>>,
    forwarded: Arc<AtomicUsize>,
    http: reqwest::Client,
    /// A second, never-approved loopback origin that counts every request it sees.
    foreign: String,
    foreign_hits: Arc<AtomicUsize>,
}

async fn prm(State(g): State<Gw>) -> impl IntoResponse {
    // `foreign_prm`: protected-resource metadata names an unapproved authorization server.
    let as_url = if g.state.lock().await.foreign_prm {
        format!("{}/idira", g.foreign)
    } else {
        format!("{}/idira", g.base)
    };
    axum::Json(
        json!({"resource": format!("{}/mcp", g.base), "authorization_servers": [as_url],
        "scopes_supported": ["mcp:tools"], "bearer_methods_supported": ["header"]}),
    )
}

async fn as_meta(State(g): State<Gw>) -> impl IntoResponse {
    // `foreign_as`: advertise endpoints on an origin the CLI did not approve.
    let i = format!("{}/idira", g.base);
    // `foreign_as`: same issuer, but the authorization/token endpoints sit on an
    // origin the CLI did not approve (a code/verifier exfiltration attempt).
    let e = if g.state.lock().await.foreign_as {
        "http://127.0.0.1:9/idira".to_owned()
    } else {
        i.clone()
    };
    axum::Json(
        json!({"issuer": i, "authorization_endpoint": format!("{e}/authorize"), "token_endpoint": format!("{e}/token"),
        "response_types_supported": ["code"], "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"], "scopes_supported": ["mcp:tools", "offline_access"],
        "token_endpoint_auth_methods_supported": ["none"], "authorization_response_iss_parameter_supported": true}),
    )
}

async fn authorize(State(g): State<Gw>, Query(q): Query<HashMap<String, String>>) -> Response {
    let get = |k: &str| q.get(k).cloned().unwrap_or_default();
    let resource_ok = get("resource") == format!("{}/mcp", g.base);
    if get("client_id") != CLI_CLIENT
        || get("code_challenge_method") != "S256"
        || !get("redirect_uri").starts_with("http://127.0.0.1:")
        || !resource_ok
    {
        return (StatusCode::BAD_REQUEST, "bad authorization request").into_response();
    }
    let code = uuid::Uuid::new_v4().to_string();
    let mut st = g.state.lock().await;
    st.authorize_calls += 1;
    let state = if st.wrong_state {
        format!("{}x", get("state"))
    } else {
        get("state")
    };
    st.codes.insert(
        code.clone(),
        (get("code_challenge"), get("redirect_uri"), get("resource")),
    );
    let iss = if st.wrong_iss {
        "http://127.0.0.1:1/evil".to_owned()
    } else {
        format!("{}/idira", g.base)
    };
    let mut loc = url::Url::parse(&get("redirect_uri")).unwrap();
    loc.query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", &state)
        .append_pair("iss", &iss);
    (StatusCode::FOUND, [("location", loc.to_string())]).into_response()
}

async fn token(State(g): State<Gw>, Form(f): Form<HashMap<String, String>>) -> Response {
    use base64::Engine;
    let get = |k: &str| f.get(k).cloned().unwrap_or_default();
    let mut st = g.state.lock().await;
    let ok = match get("grant_type").as_str() {
        "authorization_code" => match st.codes.remove(&get("code")) {
            Some((challenge, redirect, _)) => {
                let computed = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(Sha256::digest(get("code_verifier").as_bytes()));
                computed == challenge
                    && redirect == get("redirect_uri")
                    && get("client_id") == CLI_CLIENT
            }
            None => false,
        },
        "refresh_token" => {
            st.refreshes += 1;
            // Rotation: the presented token is consumed on receipt (single-use).
            let valid = st.refresh.remove(&get("refresh_token")).is_some();
            if !valid {
                st.rejected_refreshes += 1;
            }
            let delay = st.refresh_delay_ms;
            drop(st);
            // Hold the response open so overlapping clients race unless serialized.
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            st = g.state.lock().await;
            valid
        }
        _ => false,
    };
    if !ok {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(json!({"error": "invalid_grant"})),
        )
            .into_response();
    }
    let (a, r) = (
        uuid::Uuid::new_v4().to_string(),
        uuid::Uuid::new_v4().to_string(),
    );
    st.access.insert(a.clone(), ());
    st.refresh.insert(r.clone(), ());
    axum::Json(json!({"access_token": a, "refresh_token": r, "token_type": "Bearer", "expires_in": 3600, "scope": "mcp:tools offline_access"})).into_response()
}

/// The gateway hop: validate the CLI token, then forward with backend credentials.
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
    if !g.state.lock().await.access.contains_key(bearer) {
        let www = format!(
            "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\"",
            g.base
        );
        return (StatusCode::UNAUTHORIZED, [("www-authenticate", www)]).into_response();
    }
    g.forwarded.fetch_add(1, Ordering::SeqCst);
    let upstream = sign(
        "entra",
        "entra-1",
        json!({"iss": format!("https://login.microsoftonline.com/{TID}/v2.0"), "aud": API, "tid": TID,
        "oid": OWNER, "azp": GW_CLIENT, "scp": "AirsHistory.Read", "iat": now(), "nbf": now() - 5, "exp": now() + 600}),
    );
    let ia = sign(
        "gateway",
        "gw-1",
        json!({"iss": "https://mcp.example.test", "aud": BACKEND, "sub": "human-a", "client_id": CLI_CLIENT,
        "workspace_id": "ws-test", "organisation_id": "org-test", "iat": now(), "exp": now() + 120}),
    );
    let mut req = g
        .http
        .request(method, &g.backend)
        .header("authorization", format!("Bearer {upstream}"))
        .header("x-user-jwt", ia)
        .body(body);
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
    let Ok(resp) = req.send().await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let mut out = Response::builder().status(resp.status().as_u16());
    for name in ["content-type", "mcp-session-id"] {
        if let Some(v) = resp.headers().get(name) {
            out = out.header(name, v.as_bytes());
        }
    }
    out.body(Body::from_stream(resp.bytes_stream())).unwrap()
}

async fn gateway(backend: String) -> Gw {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let fl = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let foreign = format!("http://127.0.0.1:{}", fl.local_addr().unwrap().port());
    let foreign_hits: Arc<AtomicUsize> = Default::default();
    let hits = foreign_hits.clone();
    let foreign_app = Router::new().fallback(move || {
        hits.fetch_add(1, Ordering::SeqCst);
        async { StatusCode::NOT_FOUND }
    });
    tokio::spawn(async move { axum::serve(fl, foreign_app).await.unwrap() });
    let g = Gw {
        base,
        backend,
        state: Default::default(),
        forwarded: Default::default(),
        http: reqwest::Client::new(),
        foreign,
        foreign_hits,
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

fn origin(url: &str) -> String {
    url::Url::parse(url).unwrap().origin().ascii_serialization()
}

/// Run the CLI with the `--url` origin as the single approved origin.
fn cli(state: &std::path::Path, args: &[&str]) -> std::process::Output {
    let i = args.iter().position(|a| *a == "--url").expect("--url");
    let approved = origin(args[i + 1]);
    let mut all: Vec<&str> = args.to_vec();
    all.extend(["--approved-origin", approved.as_str()]);
    cli_raw(state, &all)
}

fn cli_raw(state: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(BIN)
        .args(args)
        .env_clear()
        .env("XDG_STATE_HOME", state)
        .env("HOME", state)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// Drive `login` as the user agent would: read the printed URL, follow the
/// authorization redirect into the CLI's loopback callback.
async fn login(state: &std::path::Path, url: &str) -> std::process::Output {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut child = tokio::process::Command::new(BIN)
        .args([
            "login",
            "--url",
            url,
            "--client-id",
            CLI_CLIENT,
            "--scope",
            "mcp:tools",
            "--approved-origin",
            &origin(url),
        ])
        .env_clear()
        .env("XDG_STATE_HOME", state)
        .env("HOME", state)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut err = BufReader::new(child.stderr.take().unwrap()).lines();
    let mut auth = None;
    let mut seen = String::new();
    while let Some(line) = err.next_line().await.unwrap() {
        if line.starts_with("http://") {
            auth = Some(line);
            break;
        }
        seen.push_str(&line);
        seen.push('\n');
    }
    let Some(auth) = auth else {
        // No authorization URL: the CLI refused before starting sign-in.
        let mut out = child.wait_with_output().await.unwrap();
        out.stderr = seen.into_bytes();
        return out;
    };
    let no_redirect = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = no_redirect.get(auth).send().await.unwrap();
    let callback = r
        .headers()
        .get("location")
        .expect("redirect")
        .to_str()
        .unwrap()
        .to_owned();
    let _ = reqwest::get(callback).await;
    child.wait_with_output().await.unwrap()
}

fn json_out(o: &std::process::Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or(Value::Null)
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_login_refresh_noninteractive_and_verified_download_through_gateway_fixture() {
    let gw = gateway(backend().await).await;
    let url = format!("{}/mcp", gw.base);
    let state = private_temp("state");

    // A download that fails before any chunk (no session) leaves no partial file.
    let early = private_temp("early-out");
    let o = cli(
        &state,
        &[
            "download",
            "--url",
            &url,
            "--out",
            early.join("x.bin").to_str().unwrap(),
        ],
    );
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        std::fs::read_dir(&early).unwrap().count(),
        0,
        "a failed download left files behind"
    );
    let _ = std::fs::remove_dir_all(&early);

    // Before login: noninteractive calls fail clearly and never prompt.
    let o = cli(&state, &["call", "--url", &url, "--tool", "service_info"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("authentication required"));
    assert_eq!(gw.forwarded.load(Ordering::SeqCst), 0);

    let o = login(&state, &url).await;
    assert!(
        o.status.success(),
        "login: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    // The cache is private and holds no token values in any printed output.
    let sessions = state.join("airs-cli-spike/sessions");
    assert_eq!(
        std::fs::metadata(&sessions).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let file = std::fs::read_dir(&sessions)
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| e.path().extension().is_some_and(|x| x == "json"))
        .unwrap()
        .path();
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let cached: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    let access = cached["credentials"]["token_response"]["access_token"]
        .as_str()
        .unwrap()
        .to_owned();

    let o = cli(&state, &["status", "--url", &url]);
    let s = json_out(&o);
    assert_eq!(s["session"]["has_refresh_token"], true);
    assert!(!String::from_utf8_lossy(&o.stdout).contains(&access));

    let o = cli(&state, &["call", "--url", &url, "--tool", "service_info"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v = json_out(&o);
    assert_eq!(
        v["result"]["structuredContent"]["principal"]["effective_tier"],
        "Reader"
    );
    assert!(!String::from_utf8_lossy(&o.stdout).contains(&access));

    // Refresh rotates the refresh token; the old one is now single-use spent.
    let o = cli(&state, &["refresh", "--url", &url]);
    assert_eq!(
        json_out(&o)["refresh_token_rotated"],
        true,
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        cli(&state, &["call", "--url", &url, "--tool", "service_info"])
            .status
            .success()
    );

    // Verified, private, no-clobber 32 MiB download through the gateway hop.
    let out_dir = private_temp("out");
    let dest = out_dir.join("report.bin");
    let o = cli(
        &state,
        &["download", "--url", &url, "--out", dest.to_str().unwrap()],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let d = json_out(&o);
    assert_eq!(d["bytes"], 32 * 1024 * 1024);
    assert_eq!(d["calls"], 512);
    println!("M0 loopback CLI→gateway fixture→spike download: {d}");
    assert_eq!(
        std::fs::metadata(&dest).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let o = cli(
        &state,
        &["download", "--url", &url, "--out", dest.to_str().unwrap()],
    );
    assert_eq!(
        o.status.code(),
        Some(1),
        "second download must not overwrite"
    );

    // Logout removes only this session; later calls fail without prompting.
    assert!(cli(&state, &["logout", "--url", &url]).status.success());
    let o = cli(&state, &["call", "--url", &url, "--tool", "service_info"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("authentication required"));
    for d in [state, out_dir] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn callback_with_a_wrong_issuer_saves_no_session() {
    let gw = gateway(backend().await).await;
    gw.state.lock().await.wrong_iss = true;
    let url = format!("{}/mcp", gw.base);
    let state = private_temp("state-iss");
    let o = login(&state, &url).await;
    assert_eq!(o.status.code(), Some(1), "mix-up attack must fail login");
    let o = cli(&state, &["status", "--url", &url]);
    assert_eq!(json_out(&o)["session"], Value::Null);
    let _ = std::fs::remove_dir_all(state);
}

#[tokio::test(flavor = "multi_thread")]
async fn callback_with_a_wrong_state_saves_no_session() {
    let gw = gateway(backend().await).await;
    gw.state.lock().await.wrong_state = true;
    let url = format!("{}/mcp", gw.base);
    let state = private_temp("state-csrf");
    let o = login(&state, &url).await;
    assert_eq!(
        o.status.code(),
        Some(1),
        "CSRF state mismatch must fail login"
    );
    let o = cli(&state, &["status", "--url", &url]);
    assert_eq!(json_out(&o)["session"], Value::Null);
    let _ = std::fs::remove_dir_all(state);
}

#[tokio::test(flavor = "multi_thread")]
async fn unapproved_authorization_server_origin_is_refused_before_sign_in() {
    let gw = gateway(backend().await).await;
    gw.state.lock().await.foreign_as = true;
    let url = format!("{}/mcp", gw.base);
    let state = private_temp("state-origin");
    let o = login(&state, &url).await;
    assert_eq!(o.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("not approved"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(
        gw.state.lock().await.authorize_calls,
        0,
        "nothing sent to the unapproved origin"
    );
    // The --url origin itself must be approved; downgrades are refused with no network.
    let other = format!("{}/mcp", gw.base);
    let o = cli_raw(
        &state,
        &[
            "call",
            "--url",
            &other,
            "--tool",
            "service_info",
            "--approved-origin",
            "https://mcp.example",
        ],
    );
    assert_eq!(o.status.code(), Some(2));
    let o = cli_raw(
        &state,
        &[
            "call",
            "--url",
            "http://mcp.example.invalid/mcp",
            "--tool",
            "service_info",
            "--approved-origin",
            "http://mcp.example.invalid",
        ],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("HTTPS"));
    assert_eq!(gw.forwarded.load(Ordering::SeqCst), 0);
    let _ = std::fs::remove_dir_all(state);
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_refreshes_are_serialized_across_processes() {
    let gw = gateway(backend().await).await;
    let url = format!("{}/mcp", gw.base);
    let state = private_temp("state-race");
    assert!(login(&state, &url).await.status.success());
    gw.state.lock().await.refresh_delay_ms = 400;
    let approved = origin(&url);
    let spawn = || {
        Command::new(BIN)
            .args(["refresh", "--url", &url, "--approved-origin", &approved])
            .env_clear()
            .env("XDG_STATE_HOME", &state)
            .env("HOME", &state)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let (a, b) = (spawn(), spawn());
    let (a, b) = (a.wait_with_output().unwrap(), b.wait_with_output().unwrap());
    assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
    assert!(b.status.success(), "{}", String::from_utf8_lossy(&b.stderr));
    let st = gw.state.lock().await;
    assert_eq!(
        (st.refreshes, st.rejected_refreshes),
        (2, 0),
        "the second refresh must use the rotated token"
    );
    drop(st);
    assert!(
        cli(&state, &["call", "--url", &url, "--tool", "service_info"])
            .status
            .success()
    );
    let _ = std::fs::remove_dir_all(state);
}

#[tokio::test(flavor = "multi_thread")]
async fn discovery_never_contacts_an_unapproved_authorization_server_origin() {
    let gw = gateway(backend().await).await;
    gw.state.lock().await.foreign_prm = true;
    let url = format!("{}/mcp", gw.base);
    let state = private_temp("state-discovery");
    let o = login(&state, &url).await;
    assert_eq!(
        o.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("unapproved origin"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(
        gw.foreign_hits.load(Ordering::SeqCst),
        0,
        "the unapproved origin received a request"
    );
    // Control: the same foreign listener is reachable, so zero hits is meaningful.
    let _ = reqwest::get(format!("{}/probe", gw.foreign)).await;
    assert_eq!(gw.foreign_hits.load(Ordering::SeqCst), 1);
    let _ = std::fs::remove_dir_all(state);
}
