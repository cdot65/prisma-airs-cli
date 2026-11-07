//! Server assembly: private config activation, storage ownership, metadata
//! routes, per-request two-token authentication and the MCP service.

use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;
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
use tokio::sync::Mutex;

use crate::auth::Verifier;
use crate::config::{self, Config, ConfigError};
use crate::storage;
use crate::tools::{Spike, State as ToolState, artifact_sha256};

/// Read a bounded regular file (a projected Secret/ConfigMap is a symlink into
/// `..data`; the target must still be a regular file within the size bound).
pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let mut f = File::open(path).map_err(|_| ConfigError::Unreadable)?;
    let m = f.metadata().map_err(|_| ConfigError::Unreadable)?;
    if !m.is_file() {
        return Err(ConfigError::Unreadable);
    }
    if m.size() > config::MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge);
    }
    let mut text = String::new();
    f.by_ref()
        .take(config::MAX_CONFIG_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|_| ConfigError::Unreadable)?;
    if text.len() as u64 > config::MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge);
    }
    config::parse(&text)
}

#[derive(Debug)]
pub struct ServeError(&'static str);
impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

fn log(value: serde_json::Value) {
    println!("{value}");
}

/// Build the router. Returned separately so tests can serve it on loopback.
pub fn router(config: Arc<Config>, spike: Spike) -> Result<Router, ServeError> {
    let verifier = Arc::new(Verifier::new(config.clone()).map_err(ServeError)?);
    let mut http = StreamableHttpServerConfig::default();
    let stateless = config.server.sessions == config::SessionMode::Stateless;
    // Stateless (contract §2.3): no session IDs, POST only (GET/DELETE are 405),
    // plain JSON responses for request/response tools.
    http.legacy_session_mode = !stateless;
    http.json_response = stateless;
    http.allowed_hosts = config.server.allowed_hosts.clone();
    http.max_request_body_bytes = config.server.max_request_body_bytes;
    let mcp = StreamableHttpService::new(
        move || Ok(spike.clone()),
        Arc::new(LocalSessionManager::default()),
        http,
    );
    let metadata = {
        let s = &config.server;
        json!({
            "resource": s.resource,
            "authorization_servers": s.authorization_servers,
            "scopes_supported": s.scopes_supported,
            "bearer_methods_supported": ["header"],
            "resource_name": "airs-mcp M0.6 spike",
        })
    };
    let meta = metadata.clone();
    let protected = Router::new()
        .nest_service("/mcp", mcp)
        .route_layer(middleware::from_fn_with_state(verifier, authenticate));
    Ok(Router::new()
        // Liveness reveals no tenant data and is unauthenticated by design.
        .route("/healthz", get(|| async { "ok" }))
        .route(
            "/.well-known/oauth-protected-resource",
            get(move || async move { Json(metadata) }),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(move || async move { Json(meta) }),
        )
        .merge(protected))
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

async fn authenticate(State(v): State<Arc<Verifier>>, mut req: Request, next: Next) -> Response {
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
        let assertion = single(headers, &v.config.assertion.header)?.map(str::to_owned);
        let route = match &v.config.route_binding {
            Some(rb) => single(headers, &rb.header)?.map(str::to_owned),
            None => None,
        };
        Ok((bearer, assertion, route))
    });
    let resource_metadata = format!(
        "Bearer resource_metadata=\"{}\"",
        v.config.server.resource.trim_end_matches("/mcp").to_owned()
            + "/.well-known/oauth-protected-resource/mcp"
    );
    let Ok((bearer, assertion, route)) = parsed else {
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
    match v
        .verify(bearer.as_deref(), assertion.as_deref(), route.as_deref())
        .await
    {
        Ok(p) => {
            log(
                json!({"event": "allowed", "method": logged_method(req.method()), "permission": format!("{:?}", p.grant.permission),
                "upstream_shape": p.upstream_shape, "assertion_shape": p.assertion_shape}),
            );
            if v.config.server.sessions == config::SessionMode::Stateless {
                // No session table: a forwarded `Mcp-Session-Id` (the gateway sends
                // the last one it saw for this user) is ignored, never trusted.
                if req.headers().contains_key("mcp-session-id") {
                    log(json!({"event": "session_header_ignored"}));
                }
                req.extensions_mut().insert(p);
                return next.run(req).await;
            }
            // Bind every MCP session to the principal that created it and enforce
            // that binding on every POST, GET and DELETE (contract §2.3).
            let key = p.session_key();
            let sid = req
                .headers()
                .get("mcp-session-id")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let max = v.config.server.max_sessions;
            let mut reservation = None;
            {
                let mut table = v.sessions.lock().expect("session table");
                if let Some(sid) = &sid {
                    match table.bound.get(sid) {
                        Some(o) if *o == key => {}
                        Some(o) => {
                            // Names of the differing tuple fields only, never values (M0.6 7.6).
                            const FIELDS: [&str; 6] = [
                                "tid",
                                "owner_oid",
                                "sub",
                                "client_id",
                                "workspace",
                                "organization",
                            ];
                            let differs: Vec<&str> = o
                                .split('\u{0}')
                                .zip(key.split('\u{0}'))
                                .zip(FIELDS)
                                .filter(|((a, b), _)| a != b)
                                .map(|(_, f)| f)
                                .collect();
                            drop(table);
                            log(
                                json!({"event": "denied", "code": "forbidden", "reason": "session belongs to another principal", "differs": differs}),
                            );
                            return reject(
                                StatusCode::FORBIDDEN,
                                "forbidden",
                                "session belongs to another principal",
                                &resource_metadata,
                            );
                        }
                        None => {
                            drop(table);
                            return reject(
                                StatusCode::NOT_FOUND,
                                "not_found",
                                "unknown session",
                                &resource_metadata,
                            );
                        }
                    }
                } else {
                    // A request without a session may create one: reserve a slot
                    // atomically (bound + pending), released by the guard below.
                    if table.bound.len() + table.pending >= max {
                        drop(table);
                        return reject(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "overloaded",
                            "too many sessions",
                            &resource_metadata,
                        );
                    }
                    table.pending += 1;
                    reservation = Some(Reservation(&v.sessions));
                }
            }
            let deleting = req.method() == axum::http::Method::DELETE;
            let delay = v.config.server.test_creation_delay_ms;
            if reservation.is_some() && delay > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }
            req.extensions_mut().insert(p);
            let resp = next.run(req).await;
            let created = resp
                .headers()
                .get("mcp-session-id")
                .and_then(|h| h.to_str().ok())
                .map(str::to_owned);
            let mut table = v.sessions.lock().expect("session table");
            if let Some(r) = reservation.take() {
                // Convert the reservation into the binding (or just release it)
                // under the same lock, so capacity never overshoots.
                std::mem::forget(r);
                table.pending -= 1;
                if let Some(created) = created {
                    table.bound.insert(created, key);
                }
            } else if let Some(sid) = &sid
                && deleting
                && resp.status().is_success()
            {
                table.bound.remove(sid);
            }
            drop(table);
            resp
        }
        Err(r) => {
            let d = r.denial;
            log(
                json!({"event": "denied", "code": d.code(), "reason": d.reason(),
                "upstream_shape": r.upstream_shape, "assertion_shape": r.assertion_shape,
                "ungranted": r.ungranted.as_deref().map(|(sub, client)| json!({
                    "sub": crate::auth::bounded(Some(&json!(sub))),
                    "client_id": crate::auth::bounded(Some(&json!(client)))})),
                "assertion_registered": r.assertion_registered}),
            );
            let status = StatusCode::from_u16(d.http_status()).unwrap_or(StatusCode::FORBIDDEN);
            reject(status, d.code(), d.reason(), &resource_metadata)
        }
    }
}

/// Releases a pending session reservation if the request is cancelled or the
/// transport panics before the reservation is converted (see `authenticate`).
struct Reservation<'a>(&'a std::sync::Mutex<crate::auth::SessionTable>);

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        if let Ok(mut t) = self.0.lock() {
            t.pending = t.pending.saturating_sub(1);
        }
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

/// Activate the config from a private owned copy, take storage ownership, serve.
pub async fn serve(config: Arc<Config>) -> Result<(), ServeError> {
    let admitted = storage::admit(&config.server.data_dir).map_err(|e| {
        log(json!({"event": "storage_refused", "reason": e}));
        ServeError("storage admission failed")
    })?;
    let owner = storage::acquire(&admitted).map_err(|e| {
        log(json!({"event": "storage_refused", "reason": e}));
        ServeError("storage ownership failed")
    })?;
    log(json!({"event": "storage_owned", "admitted": admitted, "owner": owner.record}));
    let spike = Spike {
        config: config.clone(),
        storage: Some((admitted, owner.record.clone())),
        state: Arc::new(Mutex::new(ToolState::default())),
        artifact_sha256: Arc::new(artifact_sha256()),
    };
    let app = router(config.clone(), spike)?;
    let addr: SocketAddr = config
        .server
        .listen
        .parse()
        .map_err(|_| ServeError("listen address"))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|_| ServeError("cannot bind"))?;
    log(
        json!({"event": "listening", "tls": config.server.tls.is_some(), "config_revision": config.revision}),
    );
    let shutdown = async {
        let mut term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = async { if let Some(t) = term.as_mut() { t.recv().await; } else { std::future::pending::<()>().await } } => {}
        }
    };
    let result = match &config.server.tls {
        None => axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|_| ServeError("server failed")),
        Some(tls) => serve_tls(listener, app, tls.clone(), shutdown).await,
    };
    drop(owner);
    result
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
/// long extension methods; none of their text reaches the log (review 10 minor 1).
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

    /// Review 10 minor 1: an extension method's text never reaches the log.
    #[test]
    fn logged_methods_are_standard_names_or_other() {
        use axum::http::Method;
        assert_eq!(logged_method(&Method::POST), "POST");
        assert_eq!(logged_method(&Method::GET), "GET");
        assert_eq!(logged_method(&Method::DELETE), "DELETE");
        let long = Method::from_bytes("X".repeat(8192).as_bytes()).unwrap();
        assert_eq!(logged_method(&long), "OTHER");
        assert_eq!(
            logged_method(&Method::from_bytes(b"PROPFIND").unwrap()),
            "OTHER"
        );
    }

    #[test]
    fn a_dropped_reservation_releases_its_slot() {
        // Models a cancelled request: the future (and its guard) is dropped
        // before the transport answers.
        let table = std::sync::Mutex::new(crate::auth::SessionTable::default());
        table.lock().unwrap().pending = 1;
        drop(Reservation(&table));
        assert_eq!(table.lock().unwrap().pending, 0);
    }
}
