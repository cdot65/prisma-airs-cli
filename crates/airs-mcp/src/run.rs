//! Server assembly (contract §2, §4.1, §11): private configuration activation and
//! atomic reload, storage ownership, the unauthenticated metadata routes, per-request
//! two-token authentication, the stateless MCP service, and the private health
//! listener.

use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use serde_json::json;

use crate::auth::Verifier;
use crate::config::{self, Config, ConfigError};
use crate::storage;
use crate::tools::{Service, Storage};

/// One bounded read of a regular file (a projected Secret/ConfigMap is a symlink into
/// `..data`; the target must still be a regular file within the size bound).
pub fn read_config_bytes(path: &Path) -> Result<Vec<u8>, ConfigError> {
    let mut f = File::open(path).map_err(|_| ConfigError::Unreadable)?;
    let m = f.metadata().map_err(|_| ConfigError::Unreadable)?;
    if !m.is_file() {
        return Err(ConfigError::Unreadable);
    }
    if m.size() > config::MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge);
    }
    let mut bytes = Vec::new();
    f.by_ref()
        .take(config::MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ConfigError::Unreadable)?;
    if bytes.len() as u64 > config::MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge);
    }
    Ok(bytes)
}

/// Parse and validate configuration bytes.
pub fn parse_config(bytes: &[u8]) -> Result<Config, ConfigError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ConfigError::Invalid("schema"))?;
    config::parse(text)
}

/// Read, bound, parse and validate a configuration file.
pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    parse_config(&read_config_bytes(path)?)
}

#[derive(Debug)]
pub struct ServeError(&'static str);
impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// One JSON object per line on stdout: fixed codes, allowlisted identifiers and
/// claim shapes only (contract §2.5).
pub fn log(value: serde_json::Value) {
    let line = value.to_string();
    if let Some(tap) = LOG_TAP
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_mut()
    {
        tap.push(line.clone());
    }
    // Diagnostics never interrupt the service: a failed or closed stdout drops the line
    // instead of panicking (`println!` would panic and could abort a reload between
    // publication and installation).
    use std::io::Write;
    #[cfg(test)]
    if FAIL_LOG_WRITES.with(|f| f.get()) {
        // As stdout on a full device: every write fails.
        let _ = writeln!(Full, "{line}");
        return;
    }
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}").and_then(|()| out.flush());
}

#[cfg(test)]
thread_local! {
    /// Test-only fault: every log write fails (stdout redirected to a full device).
    static FAIL_LOG_WRITES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A writer whose every write fails, as `/dev/full` does.
#[cfg(test)]
struct Full;

#[cfg(test)]
impl std::io::Write for Full {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::from_raw_os_error(28))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::from_raw_os_error(28))
    }
}

static LOG_TAP: std::sync::Mutex<Option<Vec<String>>> = std::sync::Mutex::new(None);

/// Test support: start keeping a copy of every log line (the lines still go to stdout).
#[doc(hidden)]
pub fn tap_logs() {
    *LOG_TAP
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Vec::new());
}

/// Test support: the lines kept since [`tap_logs`].
#[doc(hidden)]
pub fn tapped_logs() -> Vec<String> {
    LOG_TAP
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .unwrap_or_default()
}

/// One validated configuration and the verifier built from it. A request verifies
/// and is authorized against exactly one `Live`; a reload swaps in a new one.
pub struct Live {
    pub config: Arc<Config>,
    pub verifier: Verifier,
}

impl Live {
    pub fn new(config: Config) -> Result<Arc<Self>, ServeError> {
        let config = Arc::new(config);
        let verifier = Verifier::new(config.clone()).map_err(ServeError)?;
        Ok(Arc::new(Self { config, verifier }))
    }
}

/// The reloadable state behind every route.
pub struct Shared {
    live: RwLock<Arc<Live>>,
}

impl Shared {
    pub fn new(live: Arc<Live>) -> Arc<Self> {
        Arc::new(Self {
            live: RwLock::new(live),
        })
    }

    pub fn current(&self) -> Arc<Live> {
        self.live
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Activate `next` atomically after complete validation (restart-only settings, then
    /// the verifier and its key sources).
    pub fn reload(&self, next: Config) -> Result<String, ConfigError> {
        self.check_reload(&next)?;
        let revision = next.revision.clone();
        let live = Live::new(next).map_err(|_| ConfigError::Invalid("key sources"))?;
        self.swap(live);
        Ok(revision)
    }

    pub fn swap(&self, live: Arc<Live>) {
        *self
            .live
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = live;
    }

    /// Settings that select the listener, the transport policy built into the router,
    /// the storage or the archive identity cannot change without a restart.
    pub fn check_reload(&self, next: &Config) -> Result<(), ConfigError> {
        let current = self.current();
        let (a, b) = (&current.config, next);
        if a.server.listen != b.server.listen
            || a.server.allowed_hosts != b.server.allowed_hosts
            || a.server.max_request_body_bytes != b.server.max_request_body_bytes
            || a.server.health_listen != b.server.health_listen
            || a.server.data_dir != b.server.data_dir
            || a.server.tls.as_ref().map(|t| (&t.cert_file, &t.key_file))
                != b.server.tls.as_ref().map(|t| (&t.cert_file, &t.key_file))
            || a.service.tenant != b.service.tenant
            || a.airs.tsg != b.airs.tsg
            || (
                &a.airs.management_endpoint,
                &a.airs.dashboard_endpoint,
                &a.airs.token_endpoint,
            ) != (
                &b.airs.management_endpoint,
                &b.airs.dashboard_endpoint,
                &b.airs.token_endpoint,
            )
        {
            return Err(ConfigError::Invalid(
                "listener, transport, storage, tenant or cloud settings need a restart",
            ));
        }
        Ok(())
    }
}

/// Build the public router (metadata, protected `/mcp`). Separate so tests can serve
/// it on loopback.
pub fn router(shared: Arc<Shared>, service: Service) -> Router {
    let live = shared.current();
    let mut http = StreamableHttpServerConfig::default();
    // Stateless (contract §2.3, §4.1): no session IDs, POST only (GET/DELETE are 405),
    // plain JSON responses for request/response tools.
    http.legacy_session_mode = false;
    http.json_response = true;
    http.allowed_hosts = live.config.server.allowed_hosts.clone();
    http.max_request_body_bytes = live.config.server.max_request_body_bytes;
    let mcp = StreamableHttpService::new(
        move || Ok(service.clone()),
        Arc::new(LocalSessionManager::default()),
        http,
    );
    let protected = Router::new()
        .nest_service("/mcp", mcp)
        .layer(middleware::from_fn(normalize_transport))
        .route_layer(middleware::from_fn_with_state(shared.clone(), authenticate));
    let (m1, m2) = (shared.clone(), shared);
    Router::new()
        .route(
            "/.well-known/oauth-protected-resource",
            get(move || async move { Json(metadata(&m1.current().config)) }),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(move || async move { Json(metadata(&m2.current().config)) }),
        )
        .merge(protected)
}

/// RFC 9728 protected-resource metadata (contract §2.4). No tenant data.
fn metadata(config: &Config) -> serde_json::Value {
    let s = &config.server;
    json!({
        "resource": s.resource,
        "authorization_servers": s.authorization_servers,
        "scopes_supported": s.scopes_supported,
        "bearer_methods_supported": ["header"],
        "resource_name": "airs-mcp",
    })
}

/// The private health listener's router: fixed bodies, no tenant data, no
/// authentication (cluster probes only; never routed through the gateway).
pub fn health_router(shared: Arc<Shared>, ready: bool) -> Router {
    health_router_with(shared, ready, None)
}

/// [`health_router`] plus `/metrics` read from the service storage (plan M8.1): fixed
/// Prometheus series, a bounded control read, no tenant or credential data.
pub fn health_router_with(
    shared: Arc<Shared>,
    ready: bool,
    storage: Option<Arc<crate::tools::Storage>>,
) -> Router {
    let metrics_shared = shared.clone();
    Router::new()
        .route("/livez", get(|| async { "ok" }))
        .route(
            "/readyz",
            get(move || {
                let maintenance = shared.current().config.service.maintenance;
                async move {
                    if ready && !maintenance {
                        (StatusCode::OK, "ready")
                    } else {
                        (StatusCode::SERVICE_UNAVAILABLE, "not ready")
                    }
                }
            }),
        )
        .route(
            "/metrics",
            get(move || {
                let maintenance = metrics_shared.current().config.service.maintenance;
                let storage = storage.clone();
                async move {
                    let sample = crate::metrics::sample(storage.as_ref(), ready, maintenance).await;
                    (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
                        crate::metrics::render(&sample),
                    )
                }
            }),
        )
}

fn single<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, ()> {
    let mut values = headers.get_all(name).iter();
    let first = values.next();
    if values.next().is_some() {
        return Err(()); // ambiguous duplicate credential headers are refused
    }
    match first {
        None => Ok(None),
        Some(v) => v.to_str().map(Some).map_err(|_| ()),
    }
}

async fn authenticate(State(shared): State<Arc<Shared>>, mut req: Request, next: Next) -> Response {
    let live = shared.current();
    let headers = req.headers();
    let parsed = single(headers, header::AUTHORIZATION.as_str()).and_then(|auth| {
        let bearer = match auth {
            None => None,
            Some(a) => {
                let (scheme, token) = a.split_once(' ').ok_or(())?;
                if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
                    return Err(());
                }
                Some(token.to_owned())
            }
        };
        let assertion = single(headers, &live.config.assertion.header)?.map(str::to_owned);
        Ok((bearer, assertion))
    });
    let resource_metadata = format!(
        "Bearer resource_metadata=\"{}\"",
        live.config
            .server
            .resource
            .trim_end_matches("/mcp")
            .to_owned()
            + "/.well-known/oauth-protected-resource/mcp"
    );
    let Ok((bearer, assertion)) = parsed else {
        log(
            json!({"event": "denied", "code": "unauthenticated", "reason": "malformed or duplicate credential header"}),
        );
        return reject(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "malformed or duplicate credential header",
            &resource_metadata,
        );
    };
    match live
        .verifier
        .verify(bearer.as_deref(), assertion.as_deref())
        .await
    {
        Ok(p) => {
            // Only fixed codes and contract §2.5 identifiers: no claim values or shapes.
            log(
                json!({"event": "allowed", "method": logged_method(req.method()),
                "permission": format!("{:?}", p.grant.permission), "revision": live.config.revision}),
            );
            // No session table: a forwarded `Mcp-Session-Id` (the gateway sends the
            // last one it saw for this user) is ignored, never trusted (§2.3).
            if req.headers().contains_key("mcp-session-id") {
                log(json!({"event": "session_header_ignored"}));
            }
            req.extensions_mut().insert(p);
            req.extensions_mut().insert(live);
            next.run(req).await
        }
        Err(r) => {
            let d = r.denial;
            // The verified `(sub, client_id)` of an ungranted caller is the only identity a
            // denial records (contract §2.5); never claim values, shapes or tokens.
            log(
                json!({"event": "denied", "code": d.code(), "reason": d.reason(),
                "ungranted": r.ungranted.as_deref().map(|(sub, client)| json!({
                    "sub": crate::auth::bounded(Some(&json!(sub))),
                    "client_id": crate::auth::bounded(Some(&json!(client)))}))}),
            );
            let status = StatusCode::from_u16(d.http_status()).unwrap_or(StatusCode::FORBIDDEN);
            reject(status, d.code(), d.reason(), &resource_metadata)
        }
    }
}

/// The largest response body the normalizer inspects for a JSON-RPC error object; a
/// larger body is a tool result (bounded by contract §5) and passes unchanged.
const NORMALIZE_LIMIT: usize = 512 * 1024;

/// Contract §4.2 safe errors at the transport boundary: an rmcp transport refusal
/// (HTTP status ≥ 400) is answered with a fixed body for its status, and a JSON-RPC
/// error object's message is replaced by a fixed one with its `data` dropped, so no
/// header, body or parser text a client sent is ever reflected.
async fn normalize_transport(req: Request, next: Next) -> Response {
    let response = next.run(req).await;
    let status = response.status();
    if status.as_u16() >= 400 {
        let (code, reason) = match status.as_u16() {
            405 => (
                "invalid_request",
                "method not allowed on the stateless MCP endpoint",
            ),
            406 => ("invalid_request", "unacceptable Accept header"),
            413 => ("invalid_request", "request body exceeds its bound"),
            415 => ("invalid_request", "unsupported content type"),
            400..=499 => ("invalid_request", "request rejected by the MCP transport"),
            _ => ("internal", "the MCP transport failed"),
        };
        let mut fixed = (
            status,
            Json(json!({"schema_version": 1, "code": code, "reason": reason})),
        )
            .into_response();
        if let Some(allow) = response.headers().get(header::ALLOW) {
            fixed.headers_mut().insert(header::ALLOW, allow.clone());
        }
        return fixed;
    }
    let (parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, NORMALIZE_LIMIT).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"schema_version": 1, "code": "internal", "reason": "the MCP transport failed"})),
            )
                .into_response();
        }
    };
    let rewritten = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .filter(|v| v.get("error").is_some_and(serde_json::Value::is_object))
        .map(|mut v| {
            let code = v["error"]["code"].as_i64().unwrap_or(-32603);
            v["error"] = json!({"code": code, "message": jsonrpc_message(code)});
            v.to_string().into_bytes()
        });
    let mut parts = parts;
    let body = match rewritten {
        Some(bytes) => {
            parts.headers.remove(header::CONTENT_LENGTH);
            axum::body::Body::from(bytes)
        }
        None => axum::body::Body::from(bytes),
    };
    Response::from_parts(parts, body)
}

/// A fixed message per JSON-RPC error code.
fn jsonrpc_message(code: i64) -> &'static str {
    match code {
        -32700 => "parse error",
        -32600 => "invalid request",
        -32601 => "method not found",
        -32602 => "invalid params",
        _ => "request failed",
    }
}

fn reject(status: StatusCode, code: &str, reason: &str, www: &str) -> Response {
    let mut resp = (
        status,
        Json(json!({"schema_version": 1, "code": code, "reason": reason})),
    )
        .into_response();
    if let Ok(v) = HeaderValue::from_str(www) {
        resp.headers_mut().insert(header::WWW_AUTHENTICATE, v);
    }
    resp
}

/// The private copy the service activates from (contract §11).
pub const ACTIVE_CONFIG: &str = "active-config.toml";

/// Activate configuration bytes read once. The exact candidate is validated completely
/// (`check`, then the verifier with its key sources, read once) and that prepared
/// `Live` is the one returned: nothing is reread after validation. The same bytes are
/// staged as a private 0600 candidate, read back and compared, and only then renamed
/// over the private copy. Every refusal happens before that rename and leaves the
/// previous private copy and the caller's in-memory configuration untouched; after the
/// rename, activation always succeeds with the prepared configuration, so disk and
/// memory agree.
fn activate(
    bytes: &[u8],
    config_dir: &storage::Admitted,
    check: impl FnOnce(&Config) -> Result<(), ConfigError>,
) -> Result<Activated, ConfigError> {
    let candidate = parse_config(bytes)?;
    check(&candidate)?;
    let prepared = Live::new(candidate).map_err(|_| ConfigError::Invalid("key sources"))?;
    let staged = storage::stage_private(&config_dir.path, ACTIVE_CONFIG, bytes)
        .map_err(|_| ConfigError::Invalid("cannot stage the private configuration copy"))?;
    if read_config_bytes(&staged.path)? != bytes {
        return Err(ConfigError::Invalid("private configuration copy differs"));
    }
    // `Err` here means nothing was published, so refusing keeps disk and memory as
    // they were. Once the rename has happened the new copy is the active one, so the
    // prepared configuration is installed whatever the directory sync then reports;
    // a sync failure is logged, never turned into a refusal (that would leave the new
    // file on disk with the old configuration in memory).
    let published = staged
        .commit()
        .map_err(|_| ConfigError::Invalid("cannot publish the private configuration copy"))?;
    // No diagnostic is emitted here: the caller installs the configuration first and
    // only then reports a non-durable publication.
    Ok(Activated {
        live: prepared,
        durable: published.durable,
    })
}

/// A published configuration, ready to install: `durable` is false when the directory
/// sync after the rename failed (the copy is in place and active either way).
pub struct Activated {
    pub live: Arc<Live>,
    pub durable: bool,
}

/// One reload (SIGHUP): read the source once, activate it, install it, and only then
/// log. Publication is always followed by installation; nothing between them can fail
/// or panic, so disk and memory agree.
fn reload_from(
    source: &Path,
    config_dir: &storage::Admitted,
    shared: &Shared,
) -> Result<String, ConfigError> {
    let bytes = read_config_bytes(source)?;
    let Activated { live, durable } =
        activate(&bytes, config_dir, |next| shared.check_reload(next))?;
    // Install first (an `Arc` clone and a pointer swap: no allocation), then derive
    // the diagnostics.
    shared.swap(live.clone());
    let revision = live.config.revision.clone();
    if !durable {
        log(json!({"event": "config_publish_not_durable", "revision": revision}));
    }
    log(json!({"event": "config_reloaded", "revision": revision}));
    Ok(revision)
}

/// Take storage ownership, activate the configuration from its private copy, serve
/// the gateway-facing listener and the private health listener, and reload on SIGHUP.
pub async fn serve(source: PathBuf) -> Result<(), ServeError> {
    // One bounded read: these bytes choose the storage and become the active copy.
    let bytes = read_config_bytes(&source).map_err(|e| {
        log(json!({"event": "config_refused", "reason": e.to_string()}));
        ServeError("configuration refused")
    })?;
    let checked = parse_config(&bytes).map_err(|e| {
        log(json!({"event": "config_refused", "reason": e.to_string()}));
        ServeError("configuration refused")
    })?;
    let admitted = storage::admit(&checked.server.data_dir).map_err(|e| {
        log(json!({"event": "storage_refused", "reason": e}));
        ServeError("storage admission failed")
    })?;
    let owner = storage::acquire(&admitted).map_err(|e| {
        log(json!({"event": "storage_refused", "reason": e}));
        ServeError("storage ownership failed")
    })?;
    log(json!({"event": "storage_owned", "admitted": admitted, "owner": owner.record}));
    let config_dir = storage::private_child(&admitted, "config")
        .map_err(|_| ServeError("cannot admit the configuration directory"))?;
    let history = storage::private_child(&admitted, "history")
        .map_err(|_| ServeError("cannot admit the history directory"))?;
    let control = storage::private_child(&admitted, "control")
        .map_err(|_| ServeError("cannot admit the control directory"))?;
    let seed = storage::control_seed(&control).map_err(|e| {
        log(json!({"event": "storage_refused", "reason": e}));
        ServeError("control seed unavailable")
    })?;
    let Activated { live, durable } = activate(&bytes, &config_dir, |_| Ok(())).map_err(|e| {
        log(json!({"event": "config_refused", "reason": e.to_string()}));
        ServeError("configuration refused")
    })?;
    if !durable {
        log(json!({"event": "config_publish_not_durable", "revision": live.config.revision}));
    }
    let config = live.config.clone();
    let now = airs_runtime_core::time::now_ms();
    // The control store adopts the seed's epoch and key once, at creation; it is never
    // recreated. Recovery runs under exclusive ownership before any request (C2, C12).
    let mut control = crate::control::Control::open(
        &control.path,
        &config.service.tenant,
        &seed.epoch,
        &seed.cursor_key,
        now,
    )
    .map_err(|r| {
        log(json!({"event": "control_refused", "reason": r.reason}));
        ServeError("control store refused; the service stays down with the data intact")
    })?;
    let recovered = control.recover(now).map_err(|r| {
        log(json!({"event": "control_recovery_failed", "reason": r.reason}));
        ServeError("control store recovery failed")
    })?;
    log(
        json!({"event": "control_recovered", "uncertain_sends": recovered.uncertain, "requeued_jobs": recovered.requeued}),
    );
    let storage = Arc::new(Storage::new(
        admitted,
        owner.record.clone(),
        history.path.clone(),
        control,
    ));
    let shared = Shared::new(live);
    let live = shared.current();
    let app = router(
        shared.clone(),
        Service {
            storage: Some(storage.clone()),
        },
    );
    if let Some(health) = &live.config.server.health_listen {
        let addr: SocketAddr = health.parse().map_err(|_| ServeError("health address"))?;
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|_| ServeError("cannot bind the health listener"))?;
        let health = health_router_with(shared.clone(), true, Some(storage.clone()));
        tokio::spawn(async move {
            let _ = axum::serve(listener, health).await;
        });
    }
    {
        let shared = shared.clone();
        let config_dir = config_dir.clone();
        let storage = storage.clone();
        tokio::spawn(async move {
            let Ok(mut hup) =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
            else {
                return;
            };
            while hup.recv().await.is_some() {
                let previous = shared.current().config.airs.management_revision.clone();
                match reload_from(&source, &config_dir, &shared) {
                    Err(e) => {
                        log(json!({"event": "config_reload_refused", "reason": e.to_string()}));
                    }
                    Ok(_) => {
                        // A new management credential revision resumes the jobs a denial
                        // parked in `needs_credentials` (contract §10).
                        let live = shared.current();
                        let credentials = live.config.airs.management_revision.clone();
                        if credentials != previous {
                            let (storage, config) = (storage.clone(), live.config.clone());
                            let resumed = tokio::task::spawn_blocking(move || {
                                let cloud = crate::tools::cloud(&config)
                                    .map_err(|_| crate::collection::Unavailable)?;
                                crate::collection::credentials_rotated(
                                    &storage,
                                    &config.service.tenant,
                                    &cloud,
                                    &config.airs.management_revision,
                                    airs_runtime_core::time::now_ms(),
                                )
                            })
                            .await;
                            log(
                                json!({"event": "credentials_restored", "resumed_jobs": resumed.ok().and_then(Result::ok).unwrap_or(0)}),
                            );
                        }
                    }
                }
            }
        });
    }
    let addr: SocketAddr = live
        .config
        .server
        .listen
        .parse()
        .map_err(|_| ServeError("listen address"))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|_| ServeError("cannot bind"))?;
    // Every fallible startup step is done: only now does the worker start. It is
    // declared after `owner`, so even an unwinding exit stops and joins it before the
    // ownership lock is released.
    let worker = spawn_worker(shared.clone(), storage, owner.record.owner_id.clone())?;
    log(
        json!({"event": "listening", "tls": live.config.server.tls.is_some(), "config_revision": live.config.revision}),
    );
    let shutdown = async {
        let mut term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = async { if let Some(t) = term.as_mut() { t.recv().await; } else { std::future::pending::<()>().await } } => {}
        }
    };
    let result = match &live.config.server.tls {
        None => axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|_| ServeError("server failed")),
        Some(tls) => serve_tls(listener, app, tls.clone(), shutdown).await,
    };
    // Stop and join the worker before giving up exclusive ownership (contract §9.1).
    worker.stop();
    drop(owner);
    result
}

/// How often the single worker runs a scheduler pass and claims work.
const WORKER_TICK: std::time::Duration = std::time::Duration::from_secs(5);

/// The single in-process worker (contract §6, §10): every tick, a scheduler pass
/// (due retries, deadlines, unheld cancellations), then claims, each under the
/// configuration current at that claim, rechecking the submitter's grant for the
/// job's operation. It runs on its own OS thread, never on the async runtime, so its
/// blocking control-store transactions (and, from M4, executors) cannot stall request
/// handling. M4 registers the `collection_start` executor.
fn worker_storage_admitted(storage: &Storage) -> storage::Admitted {
    storage.admitted.clone()
}

fn spawn_worker(
    shared: Arc<Shared>,
    storage: Arc<Storage>,
    owner: String,
) -> Result<WorkerHandle, ServeError> {
    let mut executors: std::collections::BTreeMap<String, Arc<dyn crate::worker::Executor>> =
        std::collections::BTreeMap::new();
    executors.insert(
        "collection_start".into(),
        Arc::new(crate::collection::Collection {
            shared: shared.clone(),
        }),
    );
    executors.insert(
        "lifecycle_execute".into(),
        Arc::new(crate::lifecycle::Lifecycle {
            shared: shared.clone(),
        }),
    );
    executors.insert(
        "enrichment_start".into(),
        Arc::new(crate::collection::Enrichment {
            shared: shared.clone(),
        }),
    );
    // Reports render from the archive only (M6); the bundle tree is a private child of
    // the admitted data subtree.
    if let Ok(dir) = storage::private_child(&worker_storage_admitted(&storage), "reports") {
        let reporter: Arc<dyn crate::worker::Executor> = Arc::new(crate::reports::Reporter {
            shared: shared.clone(),
            bundles: crate::reports::Bundles { root: dir.path },
        });
        for op in ["reports_create", "daily_report", "catch_up", "revision"] {
            executors.insert(op.into(), reporter.clone());
        }
    }
    let worker = crate::worker::Worker {
        storage,
        owner,
        executors,
    };
    start_worker(shared, worker, WORKER_TICK)
}

/// Start `worker` on its own thread with the given tick.
fn start_worker(
    shared: Arc<Shared>,
    worker: crate::worker::Worker,
    tick: std::time::Duration,
) -> Result<WorkerHandle, ServeError> {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = stop.clone();
    let join = std::thread::Builder::new()
        .name("airs-mcp-worker".into())
        .spawn(move || run_worker(&worker, &shared, &flag, tick))
        .map_err(|_| ServeError("cannot start the worker thread"))?;
    Ok(WorkerHandle {
        stop,
        join: Some(join),
    })
}

/// The worker's lifetime: stopping sets the flag and joins the thread, so no worker
/// step outlives the owner of the data subtree. Dropping stops it too.
pub struct WorkerHandle {
    stop: Arc<std::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl WorkerHandle {
    pub fn stop(mut self) {
        self.halt();
    }

    fn halt(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.halt();
    }
}

/// The archive-authority reconciliation (contract §6.1, §10), run at startup and at
/// every worker tick, whatever ended the jobs involved (a cancellation, an expiry, a
/// failed write, a restart):
/// - a definitive denial the archive journal holds for the configured revision is
///   persisted in the control store;
/// - the management hold the journal implies (the latest API 429 plus the hold, or a
///   longer provider cooldown) is persisted in the control store;
/// - `needs_credentials` jobs parked under another revision resume when the configured
///   revision is not denied (so a startup with a new revision, or a resumption whose
///   write failed, is retried at the next pass).
///
/// The first two are opportunistic copies (admission derives both from the journal
/// itself, so safety never waits on them); a pass that cannot read either store defers
/// and is retried at the next tick.
fn reconcile_pass(worker: &crate::worker::Worker, shared: &Shared) {
    let live = shared.current();
    let tenant = &live.config.service.tenant;
    let revision = &live.config.airs.management_revision;
    let Ok(cloud) = crate::tools::cloud(&live.config) else {
        log(json!({"event": "credential_reconciliation_deferred"}));
        return;
    };
    if crate::collection::revision_blocked(&worker.storage, tenant, &cloud, revision).is_err() {
        log(json!({"event": "credential_reconciliation_deferred"}));
    }
    if crate::collection::management_hold(&worker.storage, tenant, &cloud).is_err() {
        log(json!({"event": "management_hold_deferred"}));
    }
    match crate::collection::credentials_rotated(
        &worker.storage,
        tenant,
        &cloud,
        revision,
        airs_runtime_core::time::now_ms(),
    ) {
        Ok(0) => {}
        Ok(resumed) => log(json!({"event": "credentials_restored", "resumed_jobs": resumed})),
        Err(_) => log(json!({"event": "credential_resumption_deferred"})),
    }
}

/// The worker loop: reconcile once at startup, then tick until stopped, sleeping in
/// short slices so a stop is observed promptly between steps.
fn run_worker(
    worker: &crate::worker::Worker,
    shared: &Shared,
    stop: &std::sync::atomic::AtomicBool,
    tick: std::time::Duration,
) {
    let stopped = || stop.load(std::sync::atomic::Ordering::SeqCst);
    reconcile_pass(worker, shared);
    while !stopped() {
        let mut slept = std::time::Duration::ZERO;
        while slept < tick && !stopped() {
            let slice = WORKER_SLICE.min(tick - slept);
            std::thread::sleep(slice);
            slept += slice;
        }
        if stopped() {
            return;
        }
        worker_tick(worker, shared, &stopped);
    }
}

/// How finely the worker observes a stop request while idle.
const WORKER_SLICE: std::time::Duration = std::time::Duration::from_millis(100);

/// One worker tick: the scheduler pass, then claims until nothing is claimable. The
/// configuration (maintenance flag and grants) is re-read before every claim, so a
/// reload or revocation between two steps applies to the next claim.
/// Test support: one worker tick's passes (scheduler, archive-authority
/// reconciliation, policy reconsideration, schedule) without claiming a job.
#[doc(hidden)]
pub fn tick_passes_for_tests(worker: &crate::worker::Worker, shared: &Shared) {
    worker_tick(worker, shared, &|| true);
}

/// Test support: the startup reconciliation alone (what the worker runs before its
/// first tick).
#[doc(hidden)]
pub fn startup_reconcile_for_tests(worker: &crate::worker::Worker, shared: &Shared) {
    reconcile_pass(worker, shared);
}

fn worker_tick(worker: &crate::worker::Worker, shared: &Shared, stopped: &dyn Fn() -> bool) {
    let tenant = shared.current().config.service.tenant.clone();
    let now = airs_runtime_core::time::now_ms();
    if let Err(r) = worker.storage.control(|c| c.scheduler_pass(&tenant, now)) {
        log(json!({"event": "scheduler_failed", "reason": r.reason}));
        return;
    }
    reconcile_pass(worker, shared);
    // Upload staging: expired uploads swept, ended and aborted uploads' files removed
    // (M5 review 2 finding 6; also the periodic removal the contract promises).
    crate::tools::sweep_staging(&worker.storage, &tenant);
    {
        // Policy-blocked jobs whose grant is restored resume (contract §10).
        let live = shared.current();
        let admit = |job: &crate::control::Job| admits(&live.config, job);
        if let Err(r) = worker
            .storage
            .control(|c| c.reconsider_blocked(&tenant, now, &admit))
        {
            log(json!({"event": "scheduler_failed", "reason": r.reason}));
            return;
        }
    }
    // The quarter-hour collection schedule (contract §6): one coalesced job per due slot.
    if let Some((epoch, _)) = worker.storage.epoch() {
        let live = shared.current();
        let ctx = crate::schedule_store::TickContext {
            interval_minutes: live.config.limits.collection_interval_minutes as i64,
            epoch: &epoch,
            config_revision: &live.config.revision,
            credential_revision: Some(&live.config.airs.management_revision),
            queue_tenant: live.config.limits.queue_per_tenant,
        };
        match worker
            .storage
            .control(|c| c.schedule_tick(&tenant, &ctx, now))
        {
            Ok(crate::schedule_store::Tick::Submitted {
                slot,
                job_id,
                skipped,
            }) => log(
                json!({"event": "schedule_submitted", "slot": slot, "job_id": job_id, "skipped_slots": skipped}),
            ),
            Ok(_) => {}
            Err(r) => log(json!({"event": "schedule_failed", "reason": r.reason})),
        }
        // Daily reports, revisions and daily enrichment (contract §8, M6).
        match worker.storage.control(|c| c.daily_tick(&tenant, &ctx, now)) {
            Ok(t)
                if !(t.reports.is_empty()
                    && t.revisions.is_empty()
                    && t.enrichments.is_empty()) =>
            {
                log(
                    json!({"event": "daily_submitted", "reports": t.reports, "revisions": t.revisions, "enrichments": t.enrichments}),
                )
            }
            Ok(_) => {}
            Err(r) => log(json!({"event": "schedule_failed", "reason": r.reason})),
        }
    }
    while !stopped() {
        let live = shared.current();
        if live.config.service.maintenance {
            return;
        }
        let admit = |job: &crate::control::Job| admits(&live.config, job);
        match worker.step(&tenant, admit) {
            Ok(Some((job, state))) => {
                log(json!({"event": "job_finished_step", "job_id": job, "state": state.name()}))
            }
            _ => return,
        }
    }
}

/// Whether `config`'s grants still admit `job`'s submitter for its operation.
fn admits(config: &Config, job: &crate::control::Job) -> bool {
    match &job.actor {
        crate::control::Actor::Principal { sub, client_id } => config.grants.iter().any(|g| {
            &g.subject == sub && &g.client_id == client_id && g.tools.contains(&job.operation)
        }),
        crate::control::Actor::Service(_) => true,
    }
}

/// Offline control-store maintenance (contract C9–C11): run only while the service is
/// stopped, under the data subtree's exclusive owner lock.
pub fn maintenance(source: &Path, action: Maintenance) -> Result<serde_json::Value, ServeError> {
    let config = load_config(source).map_err(|_| ServeError("configuration refused"))?;
    let admitted = storage::admit(&config.server.data_dir)
        .map_err(|_| ServeError("storage admission failed"))?;
    let owner = storage::acquire(&admitted)
        .map_err(|_| ServeError("the service holds the data directory; stop it first"))?;
    let control_dir = storage::private_child(&admitted, "control")
        .map_err(|_| ServeError("cannot admit the control directory"))?;
    let now = airs_runtime_core::time::now_ms();
    let tenant = &config.service.tenant;
    let result = match action {
        Maintenance::Backup(dir) => {
            let seed = storage::control_seed(&control_dir)
                .map_err(|_| ServeError("control seed unavailable"))?;
            let mut c = crate::control::Control::open(
                &control_dir.path,
                tenant,
                &seed.epoch,
                &seed.cursor_key,
                now,
            )
            .map_err(|r| ServeError(r.reason))?;
            let manifest = c.backup_into(&dir, now).map_err(|r| ServeError(r.reason))?;
            json!({"event": "control_backup", "epoch": manifest.epoch, "bytes": manifest.bytes, "sha256": manifest.sha256})
        }
        Maintenance::Restore(set) => {
            let epoch = crate::control::restore(&set, &control_dir.path, tenant, now)
                .map_err(|r| ServeError(r.reason))?;
            json!({"event": "control_restored", "pending_epoch": epoch})
        }
        Maintenance::ActivateEpoch(epoch) => {
            let seed = storage::control_seed(&control_dir)
                .map_err(|_| ServeError("control seed unavailable"))?;
            let mut c = crate::control::Control::open(
                &control_dir.path,
                tenant,
                &seed.epoch,
                &seed.cursor_key,
                now,
            )
            .map_err(|r| ServeError(r.reason))?;
            c.activate_epoch(&epoch, now)
                .map_err(|r| ServeError(r.reason))?;
            json!({"event": "control_epoch_activated", "epoch": epoch})
        }
        Maintenance::Reconcile => {
            let seed = storage::control_seed(&control_dir)
                .map_err(|_| ServeError("control seed unavailable"))?;
            let mut c = crate::control::Control::open(
                &control_dir.path,
                tenant,
                &seed.epoch,
                &seed.cursor_key,
                now,
            )
            .map_err(|r| ServeError(r.reason))?;
            let was = c.quarantine().map_err(|r| ServeError(r.reason))?;
            c.reconcile().map_err(|r| ServeError(r.reason))?;
            json!({"event": "control_reconciled", "quarantine": was})
        }
        Maintenance::MigrateArchive(source) => {
            let history = storage::private_child(&admitted, "history")
                .map_err(|_| ServeError("cannot admit the history directory"))?;
            let cloud = crate::tools::cloud(&config)
                .map_err(|_| ServeError("archive identity unavailable"))?;
            let secrets = crate::tools::service_secrets(&config).map_err(|_| {
                ServeError("the service's secret registry cannot be loaded; nothing was migrated")
            })?;
            crate::migrate::migrate_archive(
                &admitted.path,
                &history.path,
                &source,
                &cloud,
                &secrets,
            )
            .map_err(ServeError)?
        }
    };
    drop(owner);
    Ok(result)
}

/// One offline control-store maintenance action.
pub enum Maintenance {
    Backup(PathBuf),
    Restore(PathBuf),
    ActivateEpoch(String),
    /// End a restored or recreated store's recovery quarantine (C10/C11) once the
    /// Administrator has reconciled it. The holds it placed stay in force.
    Reconcile,
    /// Migrate an existing local development archive found under this directory into
    /// the service's history through the explicit import workflow (M5.2).
    MigrateArchive(PathBuf),
}

/// TLS reloading the certificate when its file changes (cert-manager rotation).
async fn serve_tls(
    listener: tokio::net::TcpListener,
    app: Router,
    tls: config::Tls,
    shutdown: impl std::future::Future<Output = ()>,
) -> Result<(), ServeError> {
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use hyper_util::server::conn::auto::Builder;
    use hyper_util::service::TowerToHyperService;
    use rustls_pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

    let load =
        |tls: &config::Tls| -> Result<(Option<SystemTime>, tokio_rustls::TlsAcceptor), ServeError> {
            let mtime = std::fs::metadata(&tls.cert_file)
                .and_then(|m| m.modified())
                .ok();
            let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(&tls.cert_file)
                .map_err(|_| ServeError("tls certificate unreadable"))?
                .collect::<Result<_, _>>()
                .map_err(|_| ServeError("tls certificate invalid"))?;
            let key = PrivateKeyDer::from_pem_file(&tls.key_file)
                .map_err(|_| ServeError("tls key unreadable"))?;
            let mut cfg = tokio_rustls::rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .map_err(|_| ServeError("tls configuration invalid"))?;
            cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
            Ok((mtime, tokio_rustls::TlsAcceptor::from(Arc::new(cfg))))
        };
    let mut current = load(&tls)?;
    tokio::pin!(shutdown);
    loop {
        let (stream, _) = tokio::select! {
            _ = &mut shutdown => return Ok(()),
            accepted = listener.accept() => match accepted { Ok(a) => a, Err(_) => continue },
        };
        let mtime = std::fs::metadata(&tls.cert_file)
            .and_then(|m| m.modified())
            .ok();
        if mtime != current.0 {
            match load(&tls) {
                Ok(next) => {
                    current = next;
                    log(json!({"event": "tls_reloaded"}));
                }
                Err(_) => log(json!({"event": "tls_reload_failed_keeping_previous"})),
            }
        }
        let acceptor = current.1.clone();
        let app = app.clone();
        tokio::spawn(async move {
            let Ok(tls_stream) = acceptor.accept(stream).await else {
                return;
            };
            let service = TowerToHyperService::new(app);
            let _ = Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(TokioIo::new(tls_stream), service)
                .await;
        });
    }
}

/// The request method as logged: a standard method's name, otherwise `OTHER`. HTTP admits
/// long extension methods; none of their text reaches the log (review M0-10 minor 1).
fn logged_method(method: &axum::http::Method) -> &'static str {
    use axum::http::Method;
    match *method {
        Method::GET => "GET",
        Method::POST => "POST",
        Method::DELETE => "DELETE",
        Method::PUT => "PUT",
        Method::PATCH => "PATCH",
        Method::HEAD => "HEAD",
        Method::OPTIONS => "OPTIONS",
        _ => "OTHER",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contract §11: the service activates only from its private copy, published 0600
    /// under the data subtree after the exact candidate bytes validated completely; a
    /// refused candidate leaves the previous copy in place.
    #[test]
    fn activation_publishes_and_loads_only_the_private_copy() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let base = std::env::temp_dir().join(format!("airs-mcp-activate-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .unwrap();
        let base = base.canonicalize().unwrap();
        let admitted = storage::admit(&base).unwrap();
        let config_dir = storage::private_child(&admitted, "config").unwrap();
        let text = include_str!("../config/example.toml");
        let active = activate(text.as_bytes(), &config_dir, |_| Ok(()))
            .unwrap()
            .live;
        assert_eq!(active.config.revision, "example-1");
        let copy = config_dir.path.join(ACTIVE_CONFIG);
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), text);
        assert_eq!(
            std::fs::metadata(&copy).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // An invalid candidate, and a valid one the reload check refuses, both leave the
        // previous private copy byte-for-byte in place (review M2-1 finding 3).
        let invalid = text.replace("tsg = \"1234567890\"", "tsg = \"x\"");
        assert!(activate(invalid.as_bytes(), &config_dir, |_| Ok(())).is_err());
        let restart_only = text.replace("revision = \"example-1\"", "revision = \"example-2\"");
        assert!(
            activate(restart_only.as_bytes(), &config_dir, |_| Err(
                ConfigError::Invalid("restart")
            ))
            .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(&copy).unwrap(),
            text,
            "refusals change nothing"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// Review M2-2 finding 1: the key sources are read exactly once, while validating; a
    /// key file that changes afterwards (rotation, or a FIFO serving different bytes on
    /// a second open) cannot make activation fail after staging, and the configuration
    /// returned is the one that was validated. A copy that cannot be staged
    /// leaves the previous private copy and no staged leftovers.
    #[test]
    fn activation_reads_key_sources_once_and_refusals_after_validation_change_nothing() {
        use std::io::Write;
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let base = std::env::temp_dir().join(format!("airs-mcp-activate-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .unwrap();
        let base = base.canonicalize().unwrap();
        let admitted = storage::admit(&base).unwrap();
        let config_dir = storage::private_child(&admitted, "config").unwrap();
        let fifo = base.join("assertion.pem");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &fifo,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::from_raw_mode(0o600),
            0,
        )
        .unwrap();
        let key = include_bytes!("../tests/fixtures/gateway-public.pem");
        let writer = {
            let fifo = fifo.clone();
            // First open: a valid key. Any later open: bytes that are not a key.
            std::thread::spawn(move || {
                for (n, bytes) in [&key[..], b"not a key".as_slice()].into_iter().enumerate() {
                    if n > 0 {
                        // Let the first reader close, so a second open blocks unless the
                        // key source is reopened.
                        std::thread::sleep(std::time::Duration::from_millis(300));
                    }
                    let mut f = std::fs::OpenOptions::new().write(true).open(&fifo).unwrap();
                    f.write_all(bytes).unwrap();
                }
            })
        };
        let text = include_str!("../config/example.toml").replace(
            "jwks_url = \"https://mcp.example.test/.well-known/jwks.json\"",
            &format!("public_key_pem_file = {:?}", fifo.to_str().unwrap()),
        );
        assert!(text.contains("public_key_pem_file"));
        let active = activate(text.as_bytes(), &config_dir, |_| Ok(()))
            .unwrap()
            .live;
        assert_eq!(active.config.revision, "example-1");
        let copy = config_dir.path.join(ACTIVE_CONFIG);
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), text);
        std::thread::sleep(std::time::Duration::from_millis(600));
        assert!(!writer.is_finished(), "the key source was opened only once");
        // A refusal after validation and before publication: the configuration
        // directory refuses the staged write. The previous copy stays, and nothing is
        // left behind. (Failures after the rename: see
        // `failures_after_publication_never_split_disk_and_memory`.)
        let next = include_str!("../config/example.toml")
            .replace("revision = \"example-1\"", "revision = \"example-2\"");
        std::fs::set_permissions(&config_dir.path, std::fs::Permissions::from_mode(0o500)).unwrap();
        let refused = activate(next.as_bytes(), &config_dir, |_| Ok(()));
        std::fs::set_permissions(&config_dir.path, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(refused.is_err());
        assert_eq!(
            std::fs::read_to_string(&copy).unwrap(),
            text,
            "refusal changes nothing"
        );
        let names: Vec<_> = std::fs::read_dir(&config_dir.path)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, [ACTIVE_CONFIG], "no staged leftovers");
        // Release the writer's pending second open, then clean up.
        let _ = std::fs::read(&fifo);
        writer.join().unwrap();
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// Review M2-3 finding 1: a failure after the rename never reports a refusal. The
    /// reviewer's probe (the directory becomes unreadable after staging) now publishes
    /// durably, because the directory was opened before staging; a failed directory
    /// sync after the rename (fault hook) still activates the prepared configuration,
    /// so the activated revision, the returned `Live` and the file on disk agree.
    #[test]
    fn failures_after_publication_never_split_disk_and_memory() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let base = std::env::temp_dir().join(format!("airs-mcp-activate-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .unwrap();
        let base = base.canonicalize().unwrap();
        let admitted = storage::admit(&base).unwrap();
        let config_dir = storage::private_child(&admitted, "config").unwrap();
        let text = include_str!("../config/example.toml");
        activate(text.as_bytes(), &config_dir, |_| Ok(())).unwrap();
        let copy = config_dir.path.join(ACTIVE_CONFIG);
        // The reviewer's probe, through the staging API: stage, then make the directory
        // unreadable (0300), then commit. Publication completes and reports durable.
        let two = text.replace("revision = \"example-1\"", "revision = \"example-2\"");
        let staged =
            storage::stage_private(&config_dir.path, ACTIVE_CONFIG, two.as_bytes()).unwrap();
        std::fs::set_permissions(&config_dir.path, std::fs::Permissions::from_mode(0o300)).unwrap();
        let published = staged.commit();
        std::fs::set_permissions(&config_dir.path, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(published, Ok(storage::Published { durable: true }));
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), two);
        // A directory sync failing after the rename: activation still succeeds with the
        // prepared configuration that is now on disk.
        let three = text.replace("revision = \"example-1\"", "revision = \"example-3\"");
        storage::FAIL_DIRECTORY_SYNC.with(|f| f.set(true));
        let active = activate(three.as_bytes(), &config_dir, |_| Ok(()));
        storage::FAIL_DIRECTORY_SYNC.with(|f| f.set(false));
        let active = active.expect("published, so activated");
        assert!(!active.durable, "the injected sync failure is reported");
        let active = active.live;
        assert_eq!(active.config.revision, "example-3");
        assert_eq!(
            std::fs::read_to_string(&copy).unwrap(),
            three,
            "disk and memory agree"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// Review M2-4 finding 1: a failed directory sync and a failed log write together
    /// cannot stop a reload between publication and installation. Through the real
    /// reload path, twice in a row: each reload installs what it published (the
    /// running configuration, the returned revision and the private copy agree), and
    /// reload handling continues.
    #[test]
    fn reloads_install_what_they_publish_even_when_sync_and_logging_fail() {
        use std::os::unix::fs::DirBuilderExt;
        let base = std::env::temp_dir().join(format!("airs-mcp-reload-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .unwrap();
        let base = base.canonicalize().unwrap();
        let admitted = storage::admit(&base).unwrap();
        let config_dir = storage::private_child(&admitted, "config").unwrap();
        let text = include_str!("../config/example.toml");
        let shared = Shared::new(
            activate(text.as_bytes(), &config_dir, |_| Ok(()))
                .unwrap()
                .live,
        );
        let source = base.join("source.toml");
        let copy = config_dir.path.join(ACTIVE_CONFIG);
        storage::FAIL_DIRECTORY_SYNC.with(|f| f.set(true));
        FAIL_LOG_WRITES.with(|f| f.set(true));
        for n in 2..=3 {
            let next = text.replace(
                "revision = \"example-1\"",
                &format!("revision = \"example-{n}\""),
            );
            std::fs::write(&source, &next).unwrap();
            let revision =
                reload_from(&source, &config_dir, &shared).expect("published, so installed");
            assert_eq!(revision, format!("example-{n}"));
            assert_eq!(shared.current().config.revision, revision, "memory");
            assert_eq!(std::fs::read_to_string(&copy).unwrap(), next, "disk");
        }
        FAIL_LOG_WRITES.with(|f| f.set(false));
        storage::FAIL_DIRECTORY_SYNC.with(|f| f.set(false));
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// A service storage and shared configuration from the example file, with an
    /// active epoch, for worker tests.
    fn worker_fixture() -> (std::path::PathBuf, Arc<Storage>, Arc<Shared>) {
        use std::os::unix::fs::DirBuilderExt;
        let base =
            std::env::temp_dir().join(format!("airs-mcp-worker-run-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .unwrap();
        let base = base.canonicalize().unwrap();
        let admitted = storage::admit(&base).unwrap();
        let control_dir = storage::private_child(&admitted, "control").unwrap();
        let live =
            Live::new(parse_config(include_bytes!("../config/example.toml")).unwrap()).unwrap();
        let tenant = live.config.service.tenant.clone();
        let control = crate::control::Control::open(
            &control_dir.path,
            &tenant,
            "0a1b2c3d",
            &[1u8; 32],
            1_000,
        )
        .unwrap();
        let owner = storage::OwnerRecord {
            owner_id: "o".into(),
            pod: "p".into(),
            node: "n".into(),
            pid: 0,
            acquired_at_unix: 0,
        };
        let storage = Arc::new(Storage::new(admitted, owner, base.join("history"), control));
        (base, storage, Shared::new(live))
    }

    fn submit_as_cli(storage: &Storage, tenant: &str, key: &str) -> String {
        let params = json!({"key": key});
        storage.control(|c| {
            c.submit(
                crate::control::Submission {
                    tenant,
                    operation: "collection_start",
                    actor: crate::control::Actor::Principal {
                        sub: "PLACEHOLDER_CALLER_SUB".into(),
                        client_id: "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID".into(),
                    },
                    idempotency_key: key,
                    expected_epoch: "0a1b2c3d",
                    params: &params,
                    config_revision: "example-1",
                    credential_revision: None,
                    deadline_ms: airs_runtime_core::time::now_ms() + 3_600_000,
                    queue_tenant: 64,
                    queue_principal: 8,
                    join_equivalent: false,
                    scheduled_slot: None,
                },
                airs_runtime_core::time::now_ms(),
            )
            .unwrap()
            .job_id
        })
    }

    /// A test executor: counts runs, optionally applies a configuration on its first
    /// run (a reload during a job), sleeps, and records that it finished.
    struct Probe {
        shared: Arc<Shared>,
        reload: std::sync::Mutex<Option<Arc<Live>>>,
        delay: std::time::Duration,
        runs: std::sync::atomic::AtomicUsize,
        finished: std::sync::atomic::AtomicUsize,
    }

    impl crate::worker::Executor for Probe {
        fn run(&self, _: &crate::worker::JobContext<'_>) -> crate::worker::Outcome {
            self.runs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if let Some(next) = self.reload.lock().unwrap().take() {
                self.shared.swap(next);
            }
            std::thread::sleep(self.delay);
            self.finished
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            crate::worker::Outcome::Succeeded(json!({"probe": true}))
        }
    }

    fn worker_with(storage: &Arc<Storage>, probe: &Arc<Probe>) -> crate::worker::Worker {
        let mut executors: std::collections::BTreeMap<String, Arc<dyn crate::worker::Executor>> =
            std::collections::BTreeMap::new();
        executors.insert("collection_start".into(), probe.clone());
        crate::worker::Worker {
            storage: storage.clone(),
            owner: "owner-test".into(),
            executors,
        }
    }

    fn state_of(storage: &Storage, tenant: &str, job: &str) -> crate::jobs::State {
        storage.control(|c| c.job(tenant, job).unwrap().unwrap().state)
    }

    /// M3 review 1 finding 9: every claim rechecks the grant under the configuration
    /// current at that claim. A reload that revokes the grant while one job runs
    /// blocks the next job instead of running it under the stale snapshot.
    #[test]
    fn a_grant_revoked_during_a_job_blocks_the_next_claim() {
        use crate::jobs::State;
        let (base, storage, shared) = worker_fixture();
        let tenant = shared.current().config.service.tenant.clone();
        let first = submit_as_cli(&storage, &tenant, "key-revoke-00000001");
        let second = submit_as_cli(&storage, &tenant, "key-revoke-00000002");
        let revoked = include_str!("../config/example.toml").replace(
            r#"tools = ["service_info", "history_status", "history_coverage", "collection_start", "receipts_ingest"]"#,
            r#"tools = ["service_info", "history_status", "history_coverage"]"#,
        );
        assert_ne!(revoked, include_str!("../config/example.toml"));
        let probe = Arc::new(Probe {
            shared: shared.clone(),
            reload: std::sync::Mutex::new(Some(
                Live::new(parse_config(revoked.as_bytes()).unwrap()).unwrap(),
            )),
            delay: std::time::Duration::from_millis(50),
            runs: Default::default(),
            finished: Default::default(),
        });
        let handle = start_worker(
            shared.clone(),
            worker_with(&storage, &probe),
            std::time::Duration::from_millis(50),
        )
        .unwrap();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let settled = |id: &str| state_of(&storage, &tenant, id) != State::Queued;
        while !(settled(&first) && settled(&second)) && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        handle.stop();
        // Equal submission times are ordered by job ID, so either job may be claimed
        // first: whichever ran succeeded, and the claim after the reload was blocked.
        let mut states = [
            state_of(&storage, &tenant, &first),
            state_of(&storage, &tenant, &second),
        ];
        states.sort_by_key(|s| s.name());
        assert_eq!(
            states,
            [State::BlockedPolicy, State::Succeeded],
            "the claim after the revocation saw it"
        );
        assert_eq!(probe.runs.load(std::sync::atomic::Ordering::SeqCst), 1);
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M3 review 1 finding 8: stopping the worker joins it, so no step outlives the
    /// owner: a step in progress finishes before `stop` returns, and nothing is claimed
    /// afterwards.
    #[test]
    fn stopping_the_worker_joins_it_before_ownership_is_released() {
        use crate::jobs::State;
        let (base, storage, shared) = worker_fixture();
        let tenant = shared.current().config.service.tenant.clone();
        submit_as_cli(&storage, &tenant, "key-stop-0000000001");
        let probe = Arc::new(Probe {
            shared: shared.clone(),
            reload: std::sync::Mutex::new(None),
            delay: std::time::Duration::from_millis(400),
            runs: Default::default(),
            finished: Default::default(),
        });
        let handle = start_worker(
            shared.clone(),
            worker_with(&storage, &probe),
            std::time::Duration::from_millis(20),
        )
        .unwrap();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while probe.runs.load(std::sync::atomic::Ordering::SeqCst) == 0
            && std::time::Instant::now() < until
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            probe.finished.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "mid-step"
        );
        handle.stop();
        assert_eq!(
            probe.finished.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "stop returned only after the step ended"
        );
        let later = submit_as_cli(&storage, &tenant, "key-stop-0000000002");
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(
            state_of(&storage, &tenant, &later),
            State::Queued,
            "no worker remains"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn logged_methods_are_standard_names_or_other() {
        use axum::http::Method;
        assert_eq!(logged_method(&Method::POST), "POST");
        assert_eq!(logged_method(&Method::GET), "GET");
        let long = Method::from_bytes("X".repeat(8192).as_bytes()).unwrap();
        assert_eq!(logged_method(&long), "OTHER");
    }
}
