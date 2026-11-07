//! The loopback fixture server (hyper 1, HTTP/1.1, keep-alive) on its own thread.

mod dlp;
mod dlp_writes;
mod gateway;
mod gateway_writes;
mod inference;
mod modelsec;
mod modelsec_writes;
mod plan;
mod record;
mod redteam;
mod redteam_writes;
mod routes;
mod runtime;
mod supplychain;

use std::net::SocketAddr;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::Value;
use tokio::sync::oneshot;

pub use dlp::{Collection, Dl, DlpData, PageStyle};
pub use dlp_writes::{Multipart, Part, multipart, sha256_hex};
pub use gateway::{GatewayData, Gw};
pub use modelsec::{GroupDeleteMode, ModelSecData, Ms};
pub use plan::Planned;
pub use record::{AuthIdentity, BasicEncoding, RecordedRequest, Route, paths};
pub use redteam::{RedTeamData, Rt};
pub use redteam_writes::{REGISTRY_EXPIRY, REGISTRY_TOKEN, validate_auth_token_preview};
pub use routes::{AsyncReceipt, AsyncRow, AsyncRule, TOKEN_PREFIX};
pub use runtime::{RuntimeData, Rx};
pub use supplychain::{Sc, SupplychainData};

use crate::encoding::form_decode;
use routes::{State, TenantCreds};

/// State plus a condition variable signalled on every new request and disconnect.
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    /// Wakes held requests when the test releases one.
    released: tokio::sync::Notify,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Waits until `ready(state)` holds or `timeout` elapses; returns whether it held.
    fn wait_until(&self, timeout: Duration, ready: impl Fn(&State) -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        let mut state = self.lock();
        loop {
            if ready(&state) {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// A deterministic AIRS stand-in bound to `127.0.0.1:0`.
///
/// Routes (see [`paths`]): `POST /oauth/token`, `GET /aisec/v1/mgmt/profiles/tsg/{tsg}`,
/// `GET /aisec/v1/mgmt/topics/tsg/{tsg}`, `POST /scan/v1/scan/sync/request`,
/// `GET /scan/v1/scan/results`, `GET /ai_gw/v2/workspaces`; anything else is `404 {}`.
/// Every request is recorded before it is answered. Dropping the server stops it.
pub struct FixtureServer {
    addr: SocketAddr,
    shared: Arc<Shared>,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl FixtureServer {
    /// Binds `127.0.0.1:0` and starts serving on a background thread.
    pub fn start() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|err| panic!("fixture server: cannot bind 127.0.0.1:0: {err}"));
        let addr = listener
            .local_addr()
            .unwrap_or_else(|err| panic!("fixture server: no local address: {err}"));
        listener
            .set_nonblocking(true)
            .unwrap_or_else(|err| panic!("fixture server: cannot set non-blocking: {err}"));
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            released: tokio::sync::Notify::new(),
        });
        let (shutdown, stop) = oneshot::channel();
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name(format!("fixture-server-{}", addr.port()))
            .spawn(move || serve(listener, thread_shared, stop))
            .unwrap_or_else(|err| panic!("fixture server: cannot spawn thread: {err}"));
        Self {
            addr,
            shared,
            shutdown: Some(shutdown),
            thread: Some(thread),
        }
    }

    /// The bound socket address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// `http://127.0.0.1:<port>` (no trailing slash).
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// `base_url()` + `path`.
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url())
    }

    /// Registers (or replaces) a tenant's credentials. OAuth for `scope=tsg_id:<tsg>`
    /// then accepts `client_id`/`client_secret` and issues `mock-token-<tsg>`; the
    /// scanner routes accept `api_key` (with a valid payload hash) and `api_token`.
    pub fn add_tenant(
        &self,
        tsg: &str,
        client_id: &str,
        client_secret: &str,
        api_key: Option<&str>,
        api_token: Option<&str>,
    ) {
        self.shared.lock().tenants.insert(
            tsg.to_owned(),
            TenantCreds {
                tsg: tsg.to_owned(),
                client_id: client_id.to_owned(),
                client_secret: client_secret.to_owned(),
                api_key: api_key.map(str::to_owned),
                api_token: api_token.map(str::to_owned),
            },
        );
    }

    /// Sets the `ai_profiles` rows served for `tsg` (wire shape, in service order).
    pub fn set_profiles(&self, tsg: &str, rows: Vec<Value>) {
        self.shared.lock().profiles.insert(tsg.to_owned(), rows);
    }

    /// Sets the `custom_topics` rows served for `tsg`.
    pub fn set_topics(&self, tsg: &str, rows: Vec<Value>) {
        self.shared.lock().topics.insert(tsg.to_owned(), rows);
    }

    /// The profile rows of `tsg` as they are now (after any write routes changed them).
    pub fn profiles(&self, tsg: &str) -> Vec<Value> {
        self.shared
            .lock()
            .profiles
            .get(tsg)
            .cloned()
            .unwrap_or_default()
    }

    /// The topic rows of `tsg` as they are now.
    pub fn topics(&self, tsg: &str) -> Vec<Value> {
        self.shared
            .lock()
            .topics
            .get(tsg)
            .cloned()
            .unwrap_or_default()
    }

    /// Edit every security profile the write routes store (`POST /v1/mgmt/profile`,
    /// `PUT …/profile/uuid/{id}`) after answering: the service adding a default or
    /// changing a policy (the restore's read-back verification, DESIGN §18.16).
    pub fn set_profile_write_edit(&self, edit: Option<fn(&mut Value)>) {
        self.shared.lock().profile_write_edit = edit;
    }

    /// Service behaviour switch for `PUT …/uuid/{id}`: `false` (the default) replaces the
    /// record in place (same id, revision + 1); `true` appends a new record with a new
    /// id and revision + 1, keeping the old one (both behaviours are tested).
    pub fn set_put_appends_new_record(&self, enabled: bool) {
        self.shared.lock().put_appends = enabled;
    }

    /// `<base>/ts-env/aisec`: the second management base, which shares this server's
    /// state (the TypeScript `PANW_MGMT_ENDPOINT` of the D93 parity tests).
    pub fn ts_env_mgmt_url(&self) -> String {
        format!(
            "{}{}{}",
            self.base_url(),
            paths::TS_ENV_BASE,
            paths::MGMT_PREFIX
        )
    }

    /// `<base>/ts-env-trap`: the trap the runner points every TypeScript `runtime` run's
    /// endpoint variables at (`404 {}` to everything, recorded as
    /// [`Route::TsEnvTrap`]).
    pub fn ts_env_trap_url(&self) -> String {
        format!("{}{}", self.base_url(), paths::TS_ENV_TRAP)
    }

    /// Issue a different token on every token request: `mock-token-<tsg>.r<n>` (`n`
    /// counts issued tokens), instead of the constant `mock-token-<tsg>`. Both forms
    /// are accepted on the management routes.
    pub fn set_rotating_tokens(&self, enabled: bool) {
        self.shared.lock().rotating_tokens = enabled;
    }

    /// Quirk switch: emit `next_offset: 0` on the last page instead of omitting it.
    pub fn set_zero_next_offset_on_last_page(&self, enabled: bool) {
        self.shared.lock().zero_next_offset_on_last_page = enabled;
    }

    /// A sync scan whose prompt or response contains `marker` gets `response` (rules
    /// are tried in insertion order; no match gives the default allow verdict).
    pub fn add_scan_rule(&self, marker: &str, response: Planned) {
        self.shared
            .lock()
            .scan_rules
            .push((marker.to_owned(), response));
    }

    /// Shorthand for a scan rule answering `200` with `verdict`.
    pub fn add_scan_verdict(&self, marker: &str, verdict: &Value) {
        self.add_scan_rule(marker, Planned::json(200, verdict));
    }

    /// Async scans whose prompt contains `marker` follow `rule` (rules are tried in
    /// insertion order; no match: complete with the default allow verdict).
    pub fn add_async_rule(&self, marker: &str, rule: AsyncRule) {
        self.shared
            .lock()
            .async_rules
            .push((marker.to_owned(), rule));
    }

    /// Emit the result rows of a receipt in reverse item order.
    pub fn set_async_reversed(&self, enabled: bool) {
        self.shared.lock().async_reversed = enabled;
    }

    /// The receipts the async route issued, in issue order.
    pub fn receipts(&self) -> Vec<AsyncReceipt> {
        self.shared.lock().receipts.clone()
    }

    /// Replaces the red team dataset with [`RedTeamData::seeded`].
    pub fn seed_redteam(&self) {
        self.shared.lock().redteam = RedTeamData::seeded();
    }

    /// Edits the red team dataset in place.
    pub fn redteam(&self, edit: impl FnOnce(&mut RedTeamData)) {
        edit(&mut self.shared.lock().redteam);
    }

    /// A copy of the red team dataset as it is now.
    pub fn redteam_data(&self) -> RedTeamData {
        self.shared.lock().redteam.clone()
    }

    /// The requests on red team route `rt`.
    pub fn rt_requests(&self, rt: Rt) -> Vec<RecordedRequest> {
        self.requests_for(Route::RedTeam(rt))
    }

    /// Replaces the DLP dataset with [`DlpData::seeded`].
    pub fn seed_dlp(&self) {
        self.shared.lock().dlp = DlpData::seeded();
    }

    /// Edits the DLP dataset in place.
    pub fn dlp(&self, edit: impl FnOnce(&mut DlpData)) {
        edit(&mut self.shared.lock().dlp);
    }

    /// A copy of the DLP dataset as it is now.
    pub fn dlp_data(&self) -> DlpData {
        self.shared.lock().dlp.clone()
    }

    /// The requests on DLP route `dl`.
    pub fn dl_requests(&self, dl: Dl) -> Vec<RecordedRequest> {
        self.requests_for(Route::Dlp(dl))
    }

    /// Serves `bytes` at `GET /uv-release/<file>` (the `--fetch-uv` archive).
    pub fn set_uv_archive(&self, bytes: Vec<u8>) {
        self.shared.lock().uv_archive = Some(bytes);
    }

    /// `http://127.0.0.1:<port>/uv-release/<file>`.
    pub fn uv_archive_url(&self, file: &str) -> String {
        self.url(&format!("{}{file}", paths::UV_RELEASE_PREFIX))
    }

    /// Replaces the model-security dataset with [`ModelSecData::seeded`].
    pub fn seed_modelsec(&self) {
        self.shared.lock().modelsec = ModelSecData::seeded();
    }

    /// Edits the model-security dataset in place.
    pub fn modelsec(&self, edit: impl FnOnce(&mut ModelSecData)) {
        edit(&mut self.shared.lock().modelsec);
    }

    /// A copy of the model-security dataset as it is now.
    pub fn modelsec_data(&self) -> ModelSecData {
        self.shared.lock().modelsec.clone()
    }

    /// The requests on model-security route `ms`.
    pub fn ms_requests(&self, ms: Ms) -> Vec<RecordedRequest> {
        self.requests_for(Route::ModelSecurity(ms))
    }

    /// Replaces the runtime extras dataset with [`RuntimeData::seeded`].
    pub fn seed_runtime(&self) {
        self.shared.lock().runtime = RuntimeData::seeded();
    }

    /// Edits the runtime extras dataset in place.
    pub fn runtime(&self, edit: impl FnOnce(&mut RuntimeData)) {
        edit(&mut self.shared.lock().runtime);
    }

    /// A copy of the runtime extras dataset as it is now.
    pub fn runtime_data(&self) -> RuntimeData {
        self.shared.lock().runtime.clone()
    }

    /// The requests on runtime extras route `rx`.
    pub fn rx_requests(&self, rx: Rx) -> Vec<RecordedRequest> {
        self.requests_for(Route::Runtime(rx))
    }

    /// Replaces the gateway dataset with [`GatewayData::seeded`].
    pub fn seed_gateway(&self) {
        self.shared.lock().gateway = GatewayData::seeded();
    }

    /// Edits the gateway dataset in place.
    pub fn gateway(&self, edit: impl FnOnce(&mut GatewayData)) {
        edit(&mut self.shared.lock().gateway);
    }

    /// A copy of the gateway dataset as it is now.
    pub fn gateway_data(&self) -> GatewayData {
        self.shared.lock().gateway.clone()
    }

    /// The requests on gateway route `gw`.
    pub fn gw_requests(&self, gw: Gw) -> Vec<RecordedRequest> {
        self.requests_for(Route::Gateway(gw))
    }

    /// Replaces the supplychain dataset with [`SupplychainData::seeded`].
    pub fn seed_supplychain(&self) {
        self.shared.lock().supplychain = SupplychainData::seeded();
    }

    /// Edits the supplychain dataset in place.
    pub fn supplychain(&self, edit: impl FnOnce(&mut SupplychainData)) {
        edit(&mut self.shared.lock().supplychain);
    }

    /// The requests on supplychain route `sc`.
    pub fn sc_requests(&self, sc: Sc) -> Vec<RecordedRequest> {
        self.requests_for(Route::Supplychain(sc))
    }

    /// Release the oldest held request ([`Planned::held`]).
    pub fn release_held(&self) {
        self.shared.lock().holds_released += 1;
        self.shared.released.notify_waiters();
    }

    /// Requests on `route` received so far.
    pub fn route_count(&self, route: Route) -> usize {
        self.shared
            .lock()
            .log
            .iter()
            .filter(|request| request.route == route)
            .count()
    }

    /// Waits until at least `count` requests on `route` have arrived; `false` on
    /// timeout (an event for interruption and crash tests).
    pub fn wait_for_route(&self, route: Route, count: usize, timeout: Duration) -> bool {
        self.shared.wait_until(timeout, |state| {
            state
                .log
                .iter()
                .filter(|request| request.route == route)
                .count()
                >= count
        })
    }

    /// Queues a one-shot response for the next request on `route` (FIFO). One-shot
    /// responses win over the route default and the built-in behaviour, including the
    /// credential checks; the request is still recorded with its identity.
    pub fn push_once(&self, route: Route, planned: Planned) {
        self.shared
            .lock()
            .once
            .entry(route)
            .or_default()
            .push_back(planned);
    }

    /// Answers every request on `route` with `planned` (after one-shot responses).
    pub fn set_default(&self, route: Route, planned: Planned) {
        self.shared.lock().defaults.insert(route, planned);
    }

    /// Removes the default and the queued one-shot responses of `route`.
    pub fn clear_overrides(&self, route: Route) {
        let mut state = self.shared.lock();
        state.defaults.remove(&route);
        state.once.remove(&route);
    }

    /// A copy of the request log, in arrival order.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.shared.lock().log.clone()
    }

    /// The logged requests classified as `route`.
    pub fn requests_for(&self, route: Route) -> Vec<RecordedRequest> {
        self.shared
            .lock()
            .log
            .iter()
            .filter(|request| request.route == route)
            .cloned()
            .collect()
    }

    /// The route of every logged request, in order.
    pub fn routes(&self) -> Vec<Route> {
        self.shared
            .lock()
            .log
            .iter()
            .map(|request| request.route)
            .collect()
    }

    /// Number of logged requests.
    pub fn request_count(&self) -> usize {
        self.shared.lock().log.len()
    }

    /// Empties the request log (sequence numbers keep counting from the log length, so
    /// they restart at 0).
    pub fn clear_requests(&self) {
        self.shared.lock().log.clear();
    }

    /// Waits until at least `count` requests have arrived; `false` on timeout.
    pub fn wait_for_requests(&self, count: usize, timeout: Duration) -> bool {
        self.shared
            .wait_until(timeout, |state| state.log.len() >= count)
    }

    /// Requests whose client went away before the response was sent (hung or delayed
    /// responses, or a body that never finished arriving).
    pub fn disconnects(&self) -> usize {
        self.shared.lock().disconnects
    }

    /// Waits until at least `count` client disconnects were observed; `false` on timeout.
    pub fn wait_for_disconnects(&self, count: usize, timeout: Duration) -> bool {
        self.shared
            .wait_until(timeout, |state| state.disconnects >= count)
    }

    /// Number of TCP connections accepted so far.
    pub fn connections(&self) -> u64 {
        self.shared.lock().connections
    }

    /// Panics when a request hit an unknown route (a misrouted endpoint).
    pub fn assert_no_unknown_routes(&self) {
        let unknown: Vec<String> = self
            .requests_for(Route::Unknown)
            .iter()
            .map(RecordedRequest::line)
            .collect();
        assert!(
            unknown.is_empty(),
            "requests reached unknown fixture routes: {unknown:?}"
        );
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            // The receiver is gone only when the server thread already stopped.
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl std::fmt::Debug for FixtureServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixtureServer")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

/// Server thread body: a small multi-thread runtime running the accept loop.
fn serve(listener: std::net::TcpListener, shared: Arc<Shared>, stop: oneshot::Receiver<()>) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap_or_else(|err| panic!("fixture server: cannot build runtime: {err}"));
    runtime.block_on(accept_loop(listener, shared, stop));
    // Hung requests are still pending; drop them without waiting.
    runtime.shutdown_timeout(Duration::from_millis(100));
}

async fn accept_loop(
    listener: std::net::TcpListener,
    shared: Arc<Shared>,
    mut stop: oneshot::Receiver<()>,
) {
    let listener = match tokio::net::TcpListener::from_std(listener) {
        Ok(listener) => listener,
        Err(err) => panic!("fixture server: cannot register listener: {err}"),
    };
    loop {
        tokio::select! {
            _ = &mut stop => break,
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else { continue };
                let connection = {
                    let mut state = shared.lock();
                    state.connections += 1;
                    state.connections - 1
                };
                let shared = Arc::clone(&shared);
                tokio::spawn(async move {
                    let service = service_fn(move |request| {
                        handle(Arc::clone(&shared), connection, request)
                    });
                    // Connection errors (client resets, EOF mid-request) are expected.
                    let _ = http1::Builder::new()
                        .keep_alive(true)
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                });
            }
        }
    }
}

/// Counts a disconnect when dropped before [`DisconnectGuard::answered`]: hyper drops
/// the service future when it sees EOF on a busy connection.
struct DisconnectGuard {
    shared: Arc<Shared>,
    armed: bool,
}

impl DisconnectGuard {
    fn answered(mut self) {
        self.armed = false;
    }
}

impl Drop for DisconnectGuard {
    fn drop(&mut self) {
        if self.armed {
            self.shared.lock().disconnects += 1;
            self.shared.changed.notify_all();
        }
    }
}

/// The service error that makes hyper close the connection without a response.
#[derive(Debug)]
struct Disconnect;

impl std::fmt::Display for Disconnect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("planned disconnect")
    }
}

impl std::error::Error for Disconnect {}

async fn handle(
    shared: Arc<Shared>,
    connection: u64,
    request: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Disconnect> {
    let guard = DisconnectGuard {
        shared: Arc::clone(&shared),
        armed: true,
    };
    let (parts, body) = request.into_parts();
    let body = match body.collect().await {
        Ok(collected) => collected.to_bytes().to_vec(),
        // The client went away mid-body; the guard counts it.
        Err(_) => return Ok(plain(StatusCode::BAD_REQUEST)),
    };
    let method = parts.method.as_str().to_owned();
    let path = parts.uri.path().to_owned();
    let raw_query = parts.uri.query().map(str::to_owned);
    let headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_ascii_lowercase(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect();
    let (route, path_tsg) = Route::classify(&method, &path);
    let base = Route::base_of(&path);
    let bearer = headers
        .iter()
        .find(|(name, _)| name == "authorization")
        .and_then(|(_, value)| value.strip_prefix("Bearer "))
        .map(str::to_owned);

    let (planned, ticket) = {
        let mut state = shared.lock();
        let (auth, basic_encoding) = state.identify(route, &headers, &body);
        let request = RecordedRequest {
            seq: state.log.len(),
            connection,
            method,
            path,
            query: raw_query.as_deref().map(form_decode).unwrap_or_default(),
            raw_query,
            headers,
            body,
            route,
            base,
            path_tsg,
            auth,
            basic_encoding,
        };
        let planned = state.plan(&request);
        let planned = if planned
            .required_headers
            .iter()
            .any(|(name, value)| request.header(name) != Some(value.as_str()))
        {
            Planned::empty_json(403)
        } else {
            planned
        };
        state.log.push(request);
        let ticket = planned.hold.then(|| {
            state.holds_started += 1;
            state.holds_started
        });
        (planned, ticket)
    };
    shared.changed.notify_all();

    if let Some(ticket) = ticket {
        loop {
            let released = shared.released.notified();
            if shared.lock().holds_released >= ticket {
                break;
            }
            released.await;
        }
    }
    if planned.disconnect {
        guard.answered();
        return Err(Disconnect);
    }
    if planned.hang {
        std::future::pending::<()>().await;
    }
    if let Some(delay) = planned.delay {
        tokio::time::sleep(delay).await;
    }
    guard.answered();
    let planned = if planned.echo_bearer {
        echo(planned, bearer.as_deref().unwrap_or_default())
    } else {
        planned
    };
    Ok(build_response(&planned))
}

/// `planned` with [`Planned::BEARER`] replaced by `bearer`.
fn echo(mut planned: Planned, bearer: &str) -> Planned {
    for (_, value) in &mut planned.headers {
        *value = value.replace(Planned::BEARER, bearer);
    }
    planned.body = String::from_utf8_lossy(&planned.body)
        .replace(Planned::BEARER, bearer)
        .into_bytes();
    planned
}

fn build_response(planned: &Planned) -> Response<Full<Bytes>> {
    let mut builder = Response::builder().status(planned.status);
    for (name, value) in &planned.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    builder
        .body(Full::new(Bytes::from(planned.body.clone())))
        // An invalid planned status or header is a test bug; make it visible as a 500.
        .unwrap_or_else(|_| plain(StatusCode::INTERNAL_SERVER_ERROR))
}

fn plain(status: StatusCode) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::new()));
    *response.status_mut() = status;
    response
}
