//! The service's tools (contract §4). Every call is authorized again here against the
//! principal the HTTP layer verified for *this* request and the configuration that
//! verified it; tool annotations and listings are hints, never authorization.
//!
//! Dispatch order (each refusal is a fixed code and reason, §4.2):
//! catalogue (`not_found`) → grant and tier (`forbidden`) → administrative side door
//! (`forbidden`) → implemented in this version (`invalid_request`) → maintenance
//! (`maintenance_required`) → strict envelope (`invalid_request`/`unsupported_version`)
//! → service tenant (`forbidden`) → the tool → result bound (`result_too_large`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use airs_history_store::identity::Cloud;
use airs_runtime_core::dto::{
    self, CollectionMode, CollectionParams, CoverageParams, ErrorCode, JobIdParams, JobsListParams,
    NoParams,
};
use base64::Engine;
use hmac::{Hmac, Mac};
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool, ToolAnnotations,
    },
    service::{RequestContext, RoleServer},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::auth::Principal;
use crate::catalogue;
use crate::config::{Config, Permission};
use crate::control::{Actor, Control, Job, Submission};
use crate::jobs::State;
use crate::run::{Live, log};
use crate::storage::{Admitted, OwnerRecord};

/// The ordinary read budget (contract §6: one coherent read, 2 s; a timeout is an error).
pub const READ_BUDGET: Duration = Duration::from_secs(2);
/// Server name reported by `service_info`.
pub const SERVER: &str = "airs-mcp";
/// The service contract version reported by `service_info` (RUNTIME_MCP_CONTRACT).
pub const CONTRACT_VERSION: u64 = 1;

/// Persistent service storage: the admitted data subtree, its owner record, the
/// history archive root, and the control store (jobs, idempotency, epochs).
pub struct Storage {
    pub admitted: Admitted,
    pub owner: OwnerRecord,
    pub history_root: PathBuf,
    control: std::sync::Mutex<Control>,
    /// The active epoch and cursor key, published once when the store is opened. The
    /// epoch changes only through offline maintenance (`control-activate-epoch`, under
    /// the data subtree's exclusive owner lock), never while this process serves, so
    /// request handlers read it without touching the control store.
    epoch: Option<(String, [u8; 32])>,
    /// Upload content validations of this service run one at a time (bounded memory
    /// and CPU; M5 review 1 finding 8).
    validations: tokio::sync::Semaphore,
    /// Where the data volume's free space comes from (`statvfs`; tests inject theirs).
    space_probe: std::sync::Mutex<crate::space::Probe>,
    /// The shared reserve sampler: per tenant, the newest measurement and whether one
    /// is in flight. At most one blocking measurement runs at a time (M8 review 2
    /// finding 8).
    space_cache: std::sync::Mutex<std::collections::HashMap<String, SpaceEntry>>,
}

/// How long a reserve measurement is reused before a new one is started.
pub const SPACE_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(2);
/// How long a request handler or a scrape waits for a measurement in flight.
pub const SPACE_BUDGET: std::time::Duration = std::time::Duration::from_millis(500);

#[derive(Default)]
struct SpaceEntry {
    last: Option<(std::time::Instant, crate::space::Space)>,
    in_flight: bool,
}

impl Storage {
    pub fn new(
        admitted: Admitted,
        owner: OwnerRecord,
        history_root: PathBuf,
        control: Control,
    ) -> Self {
        let epoch = control.active_epoch().ok();
        Self {
            admitted,
            owner,
            history_root,
            control: std::sync::Mutex::new(control),
            epoch,
            validations: tokio::sync::Semaphore::new(1),
            space_probe: std::sync::Mutex::new(crate::space::default_probe()),
            space_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Test support: replace where free space comes from (simulated low space).
    #[doc(hidden)]
    pub fn set_space_probe_for_tests(&self, probe: crate::space::Probe) {
        *self
            .space_probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = probe;
        // A new probe invalidates cached measurements; one in flight finishes, but
        // its result is replaced by the next sample.
        for e in self
            .space_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values_mut()
        {
            e.last = None;
        }
    }

    /// The data volume against the disk reserve (contract §6, §9.5): the store bytes
    /// (control and archives), `tenant`'s outstanding upload reservations, and the free
    /// bytes. Blocking (it reads the control store): worker paths only; request handlers
    /// use [`Storage::space_bounded`].
    ///
    /// Any part that cannot be measured makes the level `Unknown` (fail closed, M8
    /// review 2 finding 7): the store bytes ([`crate::metrics::store_bytes_checked`]),
    /// the reservation query, or the free bytes.
    pub fn space(&self, tenant: &str) -> crate::space::Space {
        let db = crate::metrics::store_bytes_checked(&self.history_root).and_then(|h| {
            crate::metrics::store_bytes_checked(&self.admitted.path.join("control"))
                .map(|c| h.saturating_add(c))
        });
        let now = airs_runtime_core::time::now_ms();
        let reserved = self
            .control(|c| c.staging_reserved(tenant, now))
            .ok()
            .map(|n| n.max(0) as u64);
        let probe = self
            .space_probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let free = probe(&self.admitted.path);
        match (db, reserved) {
            (Some(db), Some(reserved)) => {
                let reserve = crate::space::reserve(db, reserved);
                crate::space::Space {
                    free,
                    reserve,
                    level: crate::space::Level::of(free, reserve),
                }
            }
            _ => crate::space::Space {
                free,
                reserve: 0,
                level: crate::space::Level::Unknown,
            },
        }
    }

    /// The reserve for request handlers, bounded: [`Storage::space_sampled`]'s space.
    pub async fn space_bounded(self: &Arc<Self>, tenant: &str) -> crate::space::Space {
        self.space_sampled(tenant).await.0
    }

    /// The reserve through the shared sampler, and the age of the measurement used:
    /// - a measurement younger than [`SPACE_MAX_AGE`] is reused;
    /// - otherwise one is started on the blocking pool, unless one is already in
    ///   flight (at most one runs at a time, however many callers);
    /// - the caller waits at most [`SPACE_BUDGET`].
    ///
    /// When no fresh measurement arrives in time, the answer is `Unknown` with no age,
    /// which fails closed, rather than a stale level. A stalled measurement keeps
    /// running, and is never started twice.
    pub async fn space_sampled(
        self: &Arc<Self>,
        tenant: &str,
    ) -> (crate::space::Space, Option<std::time::Duration>) {
        let unknown = crate::space::Space {
            free: None,
            reserve: 0,
            level: crate::space::Level::Unknown,
        };
        let fresh = |e: &SpaceEntry| {
            e.last
                .as_ref()
                .is_some_and(|(at, _)| at.elapsed() < SPACE_MAX_AGE)
        };
        {
            let mut map = self
                .space_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let e = map.entry(tenant.to_owned()).or_default();
            if let (true, Some((at, s))) = (fresh(e), e.last) {
                return (s, Some(at.elapsed()));
            }
            if !e.in_flight {
                e.in_flight = true;
                let (storage, t) = (self.clone(), tenant.to_owned());
                tokio::task::spawn_blocking(move || {
                    let s = storage.space(&t);
                    let mut map = storage
                        .space_cache
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let e = map.entry(t).or_default();
                    e.last = Some((std::time::Instant::now(), s));
                    e.in_flight = false;
                });
            }
        }
        let deadline = std::time::Instant::now() + SPACE_BUDGET;
        loop {
            {
                let map = self
                    .space_cache
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(e) = map.get(tenant)
                    && let (true, Some((at, s))) = (fresh(e), e.last)
                {
                    return (s, Some(at.elapsed()));
                }
            }
            if std::time::Instant::now() >= deadline {
                return (unknown, None);
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    /// Run `f` on the control store (short, bounded transactions only). Blocking: for
    /// the worker thread and offline maintenance. Request handlers use
    /// [`Storage::control_bounded`].
    pub fn control<T>(&self, f: impl FnOnce(&mut Control) -> T) -> T {
        f(&mut self
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
    }

    /// The active recovery epoch and its cursor key (contract §4.2), or `None` when no
    /// epoch is active (a restored store awaiting activation).
    pub fn epoch(&self) -> Option<(String, [u8; 32])> {
        self.epoch.clone()
    }

    /// Run `f` on the control store off the async runtime within `budget`, queueing
    /// included: when the store stays busy past the budget `f` never runs and the
    /// request is refused `storage_unavailable`. Once the lock is held `f` (one short
    /// transaction) completes even if the caller has gone; every request-side mutation
    /// is idempotent (keyed submission, cancellation), so a retry observes its effect.
    pub async fn control_bounded<T: Send + 'static>(
        self: &Arc<Self>,
        budget: Duration,
        f: impl FnOnce(&mut Control) -> Result<T, Failure> + Send + 'static,
    ) -> Result<T, Failure> {
        let until = std::time::Instant::now() + budget;
        let storage = self.clone();
        let task = tokio::task::spawn_blocking(move || {
            let mut guard = loop {
                match storage.control.try_lock() {
                    Ok(guard) => break guard,
                    Err(std::sync::TryLockError::Poisoned(p)) => break p.into_inner(),
                    Err(std::sync::TryLockError::WouldBlock) => {
                        if std::time::Instant::now() >= until {
                            return Err(ErrorCode::StorageUnavailable.into());
                        }
                        std::thread::sleep(Duration::from_millis(2));
                    }
                }
            };
            // The budget includes every queue: a task that only started (or only got
            // the lock) after the budget expired never runs its transaction.
            if std::time::Instant::now() >= until {
                return Err(ErrorCode::StorageUnavailable.into());
            }
            f(&mut guard)
        });
        let abort = task.abort_handle();
        match tokio::time::timeout(budget, task).await {
            Ok(Ok(result)) => result,
            _ => {
                // A blocking task that has not started yet is dropped from the queue; one
                // already running finishes its single transaction (idempotent by design).
                abort.abort();
                Err(ErrorCode::StorageUnavailable.into())
            }
        }
    }
}

/// The MCP handler. `live` is swapped atomically on reload; each request uses the
/// snapshot its HTTP layer verified with (carried in the request extensions).
#[derive(Clone)]
pub struct Service {
    pub storage: Option<Arc<Storage>>,
}

fn description(name: &str) -> &'static str {
    match name {
        "service_info" => {
            "Service version, contract and schema versions, negotiated protocol, your effective tier and callable tools, health. No tenant data beyond the public service tenant ID."
        }
        "history_status" => {
            "Archive presence and schema, last collection, falling-behind and gap summary."
        }
        "history_coverage" => {
            "Per-source traversed-as-of coverage for a half-open UTC range of at most 90 days (paginated)."
        }
        "jobs_list" => "Your jobs, newest first, optionally filtered by state (paginated).",
        "jobs_get" => "One of your jobs: state, progress, result and gaps.",
        "jobs_cancel" => {
            "Request cancellation of one of your jobs; it completes at the job's next checkpoint."
        }
        "schedules_list" => {
            "Schedules with their effective times, backlog and last completed slot."
        }
        "history_query" => {
            "Full-cohort counts for a ≤90-day range, grouped by day, application, detector, verdict, action or profile. No payloads."
        }
        "reports_list" => "Published report versions (manifests), newest first.",
        "reports_get" => "One report version's manifest and short summary (latest by default).",
        "artifacts_read" => {
            "Read review.md or the summary of a report version in bounded UTF-8 pages."
        }
        "artifacts_download" => "Download one hash-bound byte chunk of a report version's file.",
        "reports_create" => {
            "Queue a manual historical report of a whole-hour range from retained history (no AIRS calls). Quota-bound."
        }
        "assessments_submit" => {
            "Attach an attributed assessment to a report version, bound to the review.md you read. Never changes evidence."
        }
        "enrichment_start" => {
            "Queue sparse strategic-example enrichment for a report's period (daily Reports allowance)."
        }
        "schedules_set" => {
            "Enable or re-time the quarter-hour collection schedule from an explicit effective time (history is kept). Daily report and enrichment schedules arrive with reports."
        }
        "schedules_disable" => {
            "Stop a schedule from an explicit effective time; slots and history are kept."
        }
        "receipts_ingest" => {
            "Record up to 200 normalized client-asserted scan receipts (no payloads) for later reconciliation with collected history. Retries with the same batch key return the same result."
        }
        "collection_start" => {
            "Submit a durable collection job (recent or explicit backfill). Returns the job promptly; retries with the same idempotency key return the same job."
        }
        _ => "",
    }
}

/// A refusal: its compiled code and reason, and the retry hint when there is one
/// (`overloaded`, `quota_exhausted`).
type Refusal = (ErrorCode, &'static str, Option<u64>);

fn deny(code: ErrorCode, reason: &'static str) -> Result<CallToolResult, Refusal> {
    Err((code, reason, None))
}

fn error_result((code, reason, retry): Refusal) -> CallToolResult {
    let mut body = json!({"schema_version": 1, "code": code, "reason": reason});
    if let Some(seconds) = retry {
        body["retry_after_seconds"] = json!(seconds);
    }
    CallToolResult::structured_error(body)
}

/// A handler failure: the error code and any retry hint the control store gave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Failure {
    pub code: ErrorCode,
    pub retry_after_seconds: Option<u64>,
}

impl From<ErrorCode> for Failure {
    fn from(code: ErrorCode) -> Self {
        Self {
            code,
            retry_after_seconds: None,
        }
    }
}

impl From<crate::control::Refused> for Failure {
    fn from(r: crate::control::Refused) -> Self {
        Self {
            code: r.code,
            retry_after_seconds: r.retry_after_seconds,
        }
    }
}

/// The durable-acceptance budget of a submission (contract §6: ≤ 10 s), queueing
/// included.
pub const SUBMIT_BUDGET: Duration = Duration::from_secs(10);

/// Complete serialized size of a tool result as a client receives it. rmcp's
/// `structured` duplicates the value into text content, so the bound applies to the
/// whole `CallToolResult` (contract §5).
pub fn result_bytes(r: &CallToolResult) -> usize {
    serde_json::to_vec(r).map(|v| v.len()).unwrap_or(usize::MAX)
}

/// Token scope tier ∩ grant tier ∩ grant allowed-tool list (§2.3).
fn granted(p: &Principal, t: &catalogue::Tool) -> bool {
    p.effective_tier() >= t.tier && p.grant.tools.contains(&t.name)
}

/// An administrative identity never reaches a mutating `grant`-authority tool: its
/// mutations go only through the proof-bound tools of §3 (R1a).
fn admin_side_door(p: &Principal, t: &catalogue::Tool) -> bool {
    p.grant.permission == Permission::Administrator && !catalogue::administrator_may_list(t)
}

/// Whether the tool can run for this principal now: granted, no side door, and
/// implemented in this version. Behind `tools/list` and `service_info.tools`.
fn callable(p: &Principal, t: &catalogue::Tool) -> bool {
    granted(p, t) && !admin_side_door(p, t) && catalogue::implemented(&t.name)
}

fn tier_name(tier: Permission) -> &'static str {
    match tier {
        Permission::Reader => "reader",
        Permission::Operator => "operator",
        Permission::Administrator => "administrator",
    }
}

fn parts(ctx: &RequestContext<RoleServer>) -> Option<&axum::http::request::Parts> {
    ctx.extensions.get::<axum::http::request::Parts>()
}

fn principal(ctx: &RequestContext<RoleServer>) -> Option<(Principal, Arc<Live>)> {
    let parts = parts(ctx)?;
    Some((
        parts.extensions.get::<Principal>()?.clone(),
        parts.extensions.get::<Arc<Live>>()?.clone(),
    ))
}

fn negotiated(ctx: &RequestContext<RoleServer>) -> Option<String> {
    parts(ctx)?
        .headers
        .get("mcp-protocol-version")?
        .to_str()
        .ok()
        .filter(|v| v.len() <= 32 && v.bytes().all(|b| b.is_ascii_graphic()))
        .map(str::to_owned)
}

/// One audit line per tool call (contract §2.5): `(sub, client_id)`, the grant label,
/// the operation, the job ID when one is involved, the approved scope digest, the policy
/// revision, the time and the outcome. Never a token, argument value, body or header.
fn audit(p: &Principal, live: &Live, tool: &str, scope: &str, job_id: Option<&str>, outcome: &str) {
    log(json!({
        "event": "audit",
        "time": utc(airs_runtime_core::time::now_ms()),
        "sub": p.subject,
        "client_id": p.client_id,
        "grant": p.grant.label(),
        "tool": tool,
        "job_id": job_id,
        "scope_digest": scope,
        "revision": live.config.revision,
        "outcome": outcome,
    }));
}

/// The approved scope digest of one call: SHA-256 over the tool, the served tenant and
/// the canonical (key-sorted) arguments. It identifies what was asked without
/// recording any argument value.
fn call_digest(tool: &str, tenant: &str, args: &Map<String, Value>) -> String {
    let (canonical, _) = crate::control::canonical(&Value::Object(args.clone()));
    let mut h = Sha256::new();
    for part in [tool, tenant, canonical.as_str()] {
        h.update((part.len() as u64).to_be_bytes());
        h.update(part.as_bytes());
    }
    hex(&h.finalize())
}

// ---- cursors (contract §4.2) ----

type HmacSha256 = Hmac<Sha256>;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CursorBody {
    /// Digest of the tool, tenant, principal and every non-page parameter.
    d: String,
    /// Digest of the complete result set the first page was cut from.
    s: String,
    /// Items already returned.
    p: usize,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn scope_digest(tool: &str, tenant: &str, p: &Principal, epoch: &str, params: &Value) -> String {
    let mut h = Sha256::new();
    for part in [tool, tenant, &p.subject, &p.client_id, epoch] {
        h.update((part.len() as u64).to_be_bytes());
        h.update(part.as_bytes());
    }
    h.update(serde_json::to_vec(params).unwrap_or_default());
    hex(&h.finalize())
}

fn mac(key: &[u8; 32], body: &[u8]) -> [u8; 32] {
    let mut m = <HmacSha256 as Mac>::new_from_slice(key).expect("any key length");
    m.update(body);
    m.finalize().into_bytes().into()
}

fn encode_cursor(key: &[u8; 32], body: &CursorBody) -> String {
    let mut bytes = serde_json::to_vec(body).expect("cursor body serializes");
    let tag = mac(key, &bytes);
    bytes.extend_from_slice(&tag);
    format!(
        "c1.{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    )
}

/// A cursor from another tool, tenant, principal, epoch or parameter set, or one with
/// a bad tag, is `invalid_request`; a changed result set is `conflict` (restart paging).
fn decode_cursor(key: &[u8; 32], text: &str, scope: &str) -> Result<CursorBody, ErrorCode> {
    let raw = text
        .strip_prefix("c1.")
        .filter(|r| (16..=2048).contains(&r.len()))
        .and_then(|r| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(r)
                .ok()
        })
        .filter(|b| b.len() > 32)
        .ok_or(ErrorCode::InvalidRequest)?;
    let (body, tag) = raw.split_at(raw.len() - 32);
    let mut m = <HmacSha256 as Mac>::new_from_slice(key).expect("any key length");
    m.update(body);
    m.verify_slice(tag).map_err(|_| ErrorCode::InvalidRequest)?;
    let cursor: CursorBody = serde_json::from_slice(body).map_err(|_| ErrorCode::InvalidRequest)?;
    if cursor.d != scope {
        return Err(ErrorCode::InvalidRequest);
    }
    Ok(cursor)
}

// ---- tool bodies ----

fn utc(ms: i64) -> String {
    airs_runtime_core::time::iso(ms)
}

/// Test support: the archive's cloud ID for a configuration.
#[doc(hidden)]
pub fn cloud_for_tests(config: &Config) -> String {
    cloud(config).map(|c| c.cloud_id).unwrap_or_default()
}

pub(crate) fn cloud(config: &Config) -> Result<Cloud, ErrorCode> {
    Cloud::from_parts(
        &config.airs.tsg,
        [
            config.airs.management_endpoint.clone(),
            config.airs.dashboard_endpoint.clone(),
            config.airs.token_endpoint.clone(),
        ],
    )
    .map_err(|_| ErrorCode::StorageUnavailable)
}

/// Run one bounded archive read off the async runtime; a read past its budget is
/// cancelled and refused, never answered from a mixed snapshot.
async fn read<T: Send + 'static>(
    f: impl FnOnce(&scm::Session) -> Result<T, ErrorCode> + Send + 'static,
) -> Result<T, ErrorCode> {
    let session = scm::Session::new();
    let guard = session.clone();
    let task = tokio::task::spawn_blocking(move || f(&session));
    match tokio::time::timeout(READ_BUDGET, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(ErrorCode::StorageUnavailable),
        Err(_) => {
            guard.cancel();
            Err(ErrorCode::StorageUnavailable)
        }
    }
}

impl Service {
    fn health(&self, live: &Live) -> &'static str {
        if live.config.service.maintenance {
            "maintenance"
        } else if self.storage.is_none() {
            "storage_unavailable"
        } else {
            "ok"
        }
    }

    fn service_info(&self, p: &Principal, live: &Live, ctx: &RequestContext<RoleServer>) -> Value {
        json!({
            "schema_version": 1,
            "contract_version": CONTRACT_VERSION,
            "schema_version_supported": [dto::SCHEMA_VERSION],
            "server": SERVER,
            "version": env!("CARGO_PKG_VERSION"),
            "service_tenant": live.config.service.tenant,
            "negotiated_protocol": negotiated(ctx),
            "effective_tier": tier_name(p.effective_tier()),
            "epoch": self.storage.as_ref().and_then(|s| s.epoch()).map_or_else(|| "00000000".to_owned(), |(e, _)| e),
            "health": self.health(live),
            "tools": catalogue::all().iter().filter(|t| callable(p, t)).map(|t| t.name.as_str()).collect::<Vec<_>>(),
        })
    }

    async fn history_status(&self, live: &Live) -> Result<Value, ErrorCode> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let cloud = cloud(&live.config)?;
        let doc = read(move |session| {
            airs_history_store::inspect::status(&storage.history_root, &cloud, None, session)
                .map_err(|_| ErrorCode::StorageUnavailable)
        })
        .await?;
        let present = doc["archive_present"].as_bool() == Some(true);
        // Derived from durable records (contract §4.3): the latest collection job, the
        // collection schedule's backlog, and the last 24 h of recorded coverage.
        let tenant = live.config.service.tenant.clone();
        let (last, behind_schedule) = self
            .control(READ_BUDGET, move |c| {
                let last = c.latest_job(&tenant, "collection_start")?;
                let backlog = c
                    .schedules_list(&tenant, airs_runtime_core::time::now_ms())?
                    .iter()
                    .any(|s| {
                        s["kind"] == "collection"
                            && s["enabled"] == true
                            && s["backlog_slots"].as_i64().unwrap_or(0) > 0
                    });
                Ok((last, backlog))
            })
            .await
            .map_err(|f| f.code)?;
        let (behind_coverage, gap) = if present {
            let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
            let cloud = crate::tools::cloud(&live.config)?;
            let now = airs_runtime_core::time::now_ms();
            let groups = read(move |session| {
                airs_history_store::inspect::coverage_groups(
                    &storage.history_root,
                    &cloud,
                    now - 86_400_000,
                    now,
                    &[],
                    session,
                )
                .map_err(|_| ErrorCode::StorageUnavailable)
            })
            .await?
            .unwrap_or_default();
            let polled: Vec<_> = groups.iter().filter(|g| g.source != "reports").collect();
            (
                polled.iter().any(|g| g.state == "falling_behind"),
                polled.iter().map(|g| (g.gap_ms.max(0) + 999) / 1000).max(),
            )
        } else {
            (false, None)
        };
        Ok(json!({
            "schema_version": 1,
            "archive_present": present,
            "schema": if present { doc["schema_version"].clone() } else { Value::Null },
            "last_collection": last.as_ref().map(Self::job_view),
            "falling_behind": behind_schedule || behind_coverage,
            "oldest_gap_seconds": gap,
            // A partially imported transfer is reported, never hidden (contract §5.1).
            "unfinished_imports": doc["unfinished_imports"].as_u64().unwrap_or(0),
        }))
    }

    async fn history_coverage(
        &self,
        p: &Principal,
        live: &Live,
        tenant: &str,
        params: CoverageParams,
    ) -> Result<Value, ErrorCode> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let (epoch, cursor_key) = storage.epoch().ok_or(ErrorCode::ControlRestored)?;
        let (since, until) = params.range.bounds(dto::MAX_COVERAGE_RANGE_MS)?;
        let apps = params.app_ids()?;
        let page = params.page.clone().unwrap_or_default();
        let limit = page.limit()?;
        let scope = scope_digest(
            "history_coverage",
            tenant,
            p,
            &epoch,
            &json!({"since": since, "until": until, "app_ids": apps}),
        );
        let start = match &page.cursor {
            None => None,
            Some(text) => Some(decode_cursor(&cursor_key, text, &scope)?),
        };
        let cloud = cloud(&live.config)?;
        let root = storage.history_root.clone();
        // An archive holding a partially imported transfer is never presented as
        // coverage: the unfinished check and the coverage read share one read
        // transaction, so no import can commit between them (M5 review 3 finding 5,
        // review 4 finding 2).
        let published = read(move |session| {
            airs_history_store::inspect::coverage_if_published(
                &root, &cloud, since, until, &apps, session,
            )
            .map_err(|_| ErrorCode::StorageUnavailable)
        })
        .await?;
        let groups = match published {
            airs_history_store::inspect::PublishedCoverage::Absent => {
                return Err(ErrorCode::NotFound);
            }
            airs_history_store::inspect::PublishedCoverage::Unfinished => {
                return Err(ErrorCode::MaintenanceRequired);
            }
            airs_history_store::inspect::PublishedCoverage::Groups(groups) => groups,
        };
        let items: Vec<Value> = groups
            .iter()
            .map(|g| {
                json!({
                    "source": g.source,
                    "application_id": g.application_id,
                    "state": g.state,
                    "traversed_as_of": g.traversed_as_of_ms.map(utc),
                    "gap_seconds": (g.gap_ms.max(0) + 999) / 1000,
                })
            })
            .collect();
        let snapshot = hex(&Sha256::digest(
            serde_json::to_vec(&items).unwrap_or_default(),
        ));
        let offset = match start {
            None => 0,
            Some(c) if c.s != snapshot => return Err(ErrorCode::Conflict),
            Some(c) if c.p > items.len() => return Err(ErrorCode::InvalidRequest),
            Some(c) => c.p,
        };
        // The largest page within `limit` whose complete result fits the agent bound.
        let tool = catalogue::get("history_coverage").expect("catalogued");
        let mut take = limit.min(items.len() - offset);
        loop {
            let end = offset + take;
            let next = (end < items.len()).then(|| {
                encode_cursor(
                    &cursor_key,
                    &CursorBody {
                        d: scope.clone(),
                        s: snapshot.clone(),
                        p: end,
                    },
                )
            });
            let value =
                json!({"schema_version": 1, "items": &items[offset..end], "next_cursor": next});
            if result_bytes(&CallToolResult::structured(value.clone())) <= tool.result_limit {
                return Ok(value);
            }
            if take <= 1 {
                return Err(ErrorCode::ResultTooLarge);
            }
            take -= 1;
        }
    }

    fn job_view(job: &Job) -> Value {
        let (kind, label) = match &job.actor {
            Actor::Principal { client_id, .. } => ("principal", client_id.as_str()),
            Actor::Service(name) => ("service", name.as_str()),
        };
        json!({
            "job_id": job.job_id,
            "epoch": job.epoch,
            "operation": job.operation,
            "params_digest": job.params_digest,
            "state": job.state.name(),
            "submitted_at": utc(job.submitted_ms),
            "updated_at": utc(job.updated_ms),
            "submitter": {"kind": kind, "label": label},
            "progress": Value::Null,
            "result": job.result.clone().unwrap_or(Value::Null),
            "retry_after_seconds": job.retry_after_ms.map(|ms| ((ms - airs_runtime_core::time::now_ms()).max(0) + 999) / 1000),
            "gaps": job.gaps.iter().take(50).collect::<Vec<_>>(),
        })
    }

    fn owned(p: &Principal, job: &Job) -> bool {
        job.actor
            == Actor::Principal {
                sub: p.subject.clone(),
                client_id: p.client_id.clone(),
            }
    }

    async fn control<T: Send + 'static>(
        &self,
        budget: Duration,
        f: impl FnOnce(&mut Control) -> Result<T, Failure> + Send + 'static,
    ) -> Result<T, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        storage.control_bounded(budget, f).await
    }

    async fn jobs_get(
        &self,
        p: &Principal,
        tenant: &str,
        params: JobIdParams,
    ) -> Result<Value, Failure> {
        if !dto::is_job_id(&params.job_id) {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let tenant = tenant.to_owned();
        let job = self
            .control(READ_BUDGET, move |c| Ok(c.job(&tenant, &params.job_id)?))
            .await?
            .filter(|j| Self::owned(p, j))
            .ok_or(ErrorCode::NotFound)?;
        Ok(json!({"schema_version": 1, "job": Self::job_view(&job)}))
    }

    async fn jobs_cancel(
        &self,
        p: &Principal,
        tenant: &str,
        params: JobIdParams,
    ) -> Result<Value, Failure> {
        self.jobs_get(p, tenant, params.clone()).await?;
        let tenant_owned = tenant.to_owned();
        let id = params.job_id.clone();
        self.control(SUBMIT_BUDGET, move |c| {
            Ok(c.cancel(&tenant_owned, &id, airs_runtime_core::time::now_ms())?)
        })
        .await?;
        self.jobs_get(p, tenant, params).await
    }

    async fn jobs_list(
        &self,
        p: &Principal,
        tenant: &str,
        params: JobsListParams,
    ) -> Result<Value, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let (epoch, key) = storage.epoch().ok_or(ErrorCode::ControlRestored)?;
        let page = params.page.clone().unwrap_or_default();
        let limit = page.limit()?.min(50);
        let mut states: Vec<State> = Vec::new();
        for name in params.states.clone().unwrap_or_default() {
            states.push(State::parse(&name).ok_or(ErrorCode::InvalidRequest)?);
        }
        if states.len() > 10 {
            return Err(ErrorCode::InvalidRequest.into());
        }
        states.sort_by_key(|s| s.name());
        states.dedup();
        let scope = scope_digest(
            "jobs_list",
            tenant,
            p,
            &epoch,
            &json!({"states": states.iter().map(|s| s.name()).collect::<Vec<_>>()}),
        );
        let start = match &page.cursor {
            None => None,
            Some(text) => Some(decode_cursor(&key, text, &scope)?),
        };
        let offset = start.as_ref().map_or(0, |c| c.p);
        let actor = Actor::Principal {
            sub: p.subject.clone(),
            client_id: p.client_id.clone(),
        };
        let tenant = tenant.to_owned();
        // One coherent read: the page (one item past it, to know whether another
        // follows) and the digest of the whole filtered set it belongs to.
        let (jobs, snapshot) = storage
            .control_bounded(READ_BUDGET, move |c| {
                let jobs = c.list(&tenant, &actor, &states, limit + 1, offset)?;
                let digest = c.list_digest(&tenant, &actor, &states)?;
                Ok((jobs, digest))
            })
            .await?;
        if start.is_some_and(|c| c.s != snapshot) {
            return Err(ErrorCode::Conflict.into());
        }
        let items: Vec<Value> = jobs.iter().take(limit).map(Self::job_view).collect();
        // The largest page within `limit` whose complete result fits the agent bound.
        let tool = catalogue::get("jobs_list").expect("catalogued");
        let mut take = items.len();
        loop {
            let more = take < jobs.len();
            let next = more.then(|| {
                encode_cursor(
                    &key,
                    &CursorBody {
                        d: scope.clone(),
                        s: snapshot.clone(),
                        p: offset + take,
                    },
                )
            });
            let value = json!({"schema_version": 1, "items": &items[..take], "next_cursor": next});
            if result_bytes(&CallToolResult::structured(value.clone())) <= tool.result_limit {
                return Ok(value);
            }
            if take <= 1 {
                return Err(ErrorCode::ResultTooLarge.into());
            }
            take -= 1;
        }
    }

    /// The run an unbound (earlier-build) receipt reservation owns, from proven
    /// provenance only:
    /// - the run named by the reservation's complete scope exists: it is that run (the
    ///   identifier is a hash of the full scope, so no other batch can own it);
    /// - neither candidate run exists: nothing was written, so the scope-named run;
    /// - only the epoch-less run exists: it is this reservation's only when no restore
    ///   ever happened (one recovery epoch), else ownership is ambiguous and the batch
    ///   is refused (`conflict`) with no archive effect, for explicit reconciliation.
    ///
    /// A lookup that fails is `storage_unavailable`: nothing is bound or written.
    async fn resolve_unbound_receipt_run(
        &self,
        storage: &Arc<Storage>,
        cloud: &Cloud,
        bound: &str,
        legacy: &str,
    ) -> Result<(String, u8), Failure> {
        let (root, cloud, b, l) = (
            storage.history_root.clone(),
            cloud.clone(),
            bound.to_owned(),
            legacy.to_owned(),
        );
        let found = tokio::time::timeout(
            SUBMIT_BUDGET,
            tokio::task::spawn_blocking(move || {
                let session = scm::Session::new();
                Ok::<_, airs_runtime_core::error::CliError>((
                    airs_history_store::run_summary::run_exists(&root, &cloud, &b, &session)?,
                    airs_history_store::run_summary::run_exists(&root, &cloud, &l, &session)?,
                ))
            }),
        )
        .await;
        let (scoped, epochless) = match found {
            Ok(Ok(Ok(v))) => v,
            _ => return Err(ErrorCode::StorageUnavailable.into()),
        };
        if scoped || !epochless {
            return Ok((bound.to_owned(), 2));
        }
        let epochs = storage
            .control_bounded(SUBMIT_BUDGET, |c| Ok(c.epoch_count()?))
            .await?;
        if epochs == 1 {
            Ok((legacy.to_owned(), 1))
        } else {
            Err(ErrorCode::Conflict.into())
        }
    }

    /// `receipts_ingest` (contract §4.1): one bounded batch of normalized
    /// `client_asserted` receipts. Validation precedes any write; each record is
    /// attributed only through an explicit receipt association to a configured scanner
    /// (a client's own application ID or name is never stored); the archive write and
    /// per-scan reconciliation are the CLI receipt path's; the result is idempotent by
    /// batch key. Zero AIRS sends.
    async fn receipts_ingest(
        &self,
        p: &Principal,
        live: &Live,
        tenant: &str,
        key: &str,
        expected_epoch: &str,
        params: ReceiptsParams,
    ) -> Result<Value, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        // Receipt ingestion writes the archive: it pauses at the reserve (§9.5).
        if !storage.space_bounded(tenant).await.allows_collection() {
            return Err(ErrorCode::StorageUnavailable.into());
        }
        let config = &live.config;
        let cloud = cloud(config)?;
        if params.cloud_id != cloud.cloud_id {
            return Err(ErrorCode::InvalidRequest.into());
        }
        if params.records.is_empty() || params.records.len() > 200 {
            return Err(ErrorCode::InvalidRequest.into());
        }
        if serde_json::to_vec(&params.records).map_or(usize::MAX, |b| b.len()) > 256 * 1024 {
            return Err(ErrorCode::InvalidRequest.into());
        }
        if params.records.iter().any(|r| !r.well_formed()) {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let (canonical_params, digest) = crate::control::canonical(
            &json!({"cloud_id": params.cloud_id, "records": params.records}),
        );
        let _ = canonical_params;
        // Fail closed: the receipt fields are normalized against the service's complete
        // secret registry (every configured credential file, read bounded), as collection
        // is. When any of them cannot be loaded nothing is reserved or written.
        let secrets = airs_runtime_core::error::SecretSet::default();
        if crate::airs_tenant::tenant(config, &secrets).is_err() {
            return Err(ErrorCode::CredentialBlocked.into());
        }
        let caller = (p.subject.clone(), p.client_id.clone());
        let (t, k, e, d) = (
            tenant.to_owned(),
            key.to_owned(),
            expected_epoch.to_owned(),
            digest.clone(),
        );
        let c2 = caller.clone();
        // The batch's archive run is named by the key's complete idempotency scope
        // (tenant, caller, recovery epoch, key), so the same key text under a later epoch
        // is a different batch (M4 review 10); the reservation binds it.
        let bound = receipt_run_id(tenant, (&caller.0, &caller.1), expected_epoch, key);
        let legacy = legacy_receipt_run_id(tenant, (&caller.0, &caller.1), key);
        let bound2 = bound.clone();
        // The key is reserved (bound to this digest and its run) before any effect: other
        // parameters under it are a conflict now, and a replay after a crash repeats
        // idempotently in the bound run.
        let prior = storage
            .control_bounded(SUBMIT_BUDGET, move |c| {
                Ok(c.ingest_reserve(
                    &t,
                    (&c2.0, &c2.1),
                    &e,
                    &k,
                    &d,
                    &bound2,
                    airs_runtime_core::time::now_ms(),
                )?)
            })
            .await?;
        // Every batch past this point runs in the run its reservation bound; a
        // reservation without a binding (it cannot be, after `ingest_reserve`) is refused
        // rather than guessed.
        let prior = match prior {
            crate::ingest_store::Prior::Pending(None) => {
                // An earlier build's reservation: bind it only to a run whose ownership
                // is proven, before any effect (M4 review 12).
                let run = self
                    .resolve_unbound_receipt_run(&storage, &cloud, &bound, &legacy)
                    .await?;
                let (t, k, e, d) = (
                    tenant.to_owned(),
                    key.to_owned(),
                    expected_epoch.to_owned(),
                    digest.clone(),
                );
                let c4 = caller.clone();
                storage
                    .control_bounded(SUBMIT_BUDGET, move |c| {
                        Ok(c.ingest_bind(&t, (&c4.0, &c4.1), &e, &k, &d, &run.0, run.1)?)
                    })
                    .await?
            }
            other => other,
        };
        let run = match prior {
            crate::ingest_store::Prior::Same(result) => return Ok(result),
            crate::ingest_store::Prior::Pending(Some(run)) => run,
            _ => return Err(ErrorCode::StorageUnavailable.into()),
        };
        // Attribution only through an explicit association (contract §11).
        let mut refused = Vec::new();
        let mut inputs = Vec::new();
        for (index, record) in params.records.iter().enumerate() {
            let scanner = config
                .receipt_associations
                .iter()
                .find(|a| {
                    a.subject == p.subject
                        && a.client_id == p.client_id
                        && a.declared_scanner == record.declared_scanner
                })
                .and_then(|a| config.airs.scanners.iter().find(|s| s.name == a.scanner));
            match scanner {
                None => refused.push(json!({"index": index, "reason": "no receipt association for this declared scanner"})),
                Some(scanner) => inputs.push((index, record.received(scanner))),
            }
        }
        // A refusal-only batch touches no archive (not even this check).
        if !inputs.is_empty() {
            // A bound run written before acceptance records existed, already holding
            // receipts, cannot have its counts proven: it is refused before any effect, for
            // explicit reconciliation (M4 review 13).
            let (root, cloud2, run2) = (storage.history_root.clone(), cloud.clone(), run.clone());
            let record = tokio::time::timeout(
                SUBMIT_BUDGET,
                tokio::task::spawn_blocking(move || {
                    airs_history_store::run_summary::receipt_run_record(
                        &root,
                        &cloud2,
                        &run2,
                        &scm::Session::new(),
                    )
                }),
            )
            .await;
            match record {
                Ok(Ok(Ok(airs_history_store::run_summary::ReceiptRunRecord::Unrecorded {
                    members,
                }))) if members > 0 => {
                    return Err(ErrorCode::Conflict.into());
                }
                Ok(Ok(Ok(_))) => {}
                _ => return Err(ErrorCode::StorageUnavailable.into()),
            }
        }
        let root = storage.history_root.clone();
        let received: Vec<_> = inputs.into_iter().map(|(_, r)| r).collect();
        let written = tokio::task::spawn_blocking(move || {
            // A batch with nothing to retain never opens (or creates) the archive.
            if received.is_empty() {
                return Ok::<_, airs_runtime_core::error::CliError>((0, 0));
            }
            let spans = Default::default();
            airs_history_store::repository::ingest_client_receipts(
                &root, &cloud, &run, &received, &secrets, &spans,
            )?;
            airs_history_store::repository::finish_received_within(
                &root,
                &cloud,
                &run,
                "complete",
                airs_history_store::repository::SERVICE_RECEIPT_BUDGET,
            )?;
            // Counts come from the archive's own record of this run, so a replay of the
            // same deterministic run reports the same numbers.
            airs_history_store::run_summary::receipt_run_counts(
                &root,
                &cloud,
                &run,
                &scm::Session::new(),
            )
        });
        let (accepted, duplicates) = match tokio::time::timeout(SUBMIT_BUDGET, written).await {
            Ok(Ok(Ok(counts))) => counts,
            _ => return Err(ErrorCode::StorageUnavailable.into()),
        };
        let result = json!({
            "schema_version": 1,
            "accepted": accepted,
            "duplicates": duplicates,
            "refused": refused,
        });
        let (t, k, e) = (tenant.to_owned(), key.to_owned(), expected_epoch.to_owned());
        let now = airs_runtime_core::time::now_ms();
        storage
            .control_bounded(SUBMIT_BUDGET, move |c| {
                Ok(c.ingest_record(&t, (&caller.0, &caller.1), &e, &k, &digest, &result, now)?)
            })
            .await
    }

    /// The administrative authority settings: refused unless `fresh_proof` is on.
    fn admin(live: &Live) -> Result<u64, Failure> {
        let a = &live.config.admin;
        match (a.mode, a.proof_max_age_seconds) {
            (crate::config::AdminMode::FreshProof, Some(age)) => Ok(age),
            _ => Err(ErrorCode::Forbidden.into()),
        }
    }

    /// The request's administrative proof: the configured nonce claim and `auth_time`
    /// of the verified identity assertion (contract §3).
    fn proof(p: &Principal, live: &Live) -> Option<crate::authority_store::Proof> {
        let claim = live.config.admin.proof_nonce_claim.as_deref()?;
        Some(crate::authority_store::Proof {
            nonce: p.assertion_claims.get(claim)?.as_str()?.to_owned(),
            auth_time: p.assertion_claims.get("auth_time")?.as_u64()?,
        })
    }

    fn authority_actor(p: &Principal) -> crate::authority_store::Actor {
        crate::authority_store::Actor {
            sub: p.subject.clone(),
            client_id: p.client_id.clone(),
        }
    }

    /// `lifecycle_preview` (Administrator, read-only): the affected data and a durable,
    /// single-use preview bound to the exact parameters, their digest and the data
    /// generation.
    async fn lifecycle_preview(
        &self,
        p: &Principal,
        live: &Live,
        tenant: &str,
        params: LifecyclePreviewParams,
    ) -> Result<Value, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let cloud = cloud(&live.config)?;
        let mut generation = self.generation_for(live, tenant, &params.action).await?;
        let mut affected = json!({});
        let mut summary = format!("{} preview", params.action);
        if params.action == "prune" {
            if params.examples_only != Some(true) {
                return Err(ErrorCode::InvalidRequest.into());
            }
            let before = params
                .before
                .as_deref()
                .and_then(dto::utc_ms)
                .ok_or(ErrorCode::InvalidRequest)?;
            let apps = params.app_ids.clone().unwrap_or_default();
            let root = storage.history_root.clone();
            let v = read(move |session| {
                airs_history_store::repository::prune_examples(
                    &root,
                    &cloud,
                    before,
                    &apps,
                    false,
                    READ_BUDGET,
                    session,
                )
                .map_err(|_| ErrorCode::StorageUnavailable)
            })
            .await?;
            // The binding is the generation the counts were read under (one snapshot).
            generation = v["generation"]
                .as_i64()
                .ok_or(ErrorCode::StorageUnavailable)?;
            affected = json!({
                "examples": v["examples"],
                "facts": v["event_facts_with_excerpts"].as_u64().unwrap_or(0) + v["receipt_facts_with_excerpts"].as_u64().unwrap_or(0),
            });
            summary = format!(
                "examples-only prune of {} events; earlier exports and external backups are not erased",
                v["events"]
            );
        }
        if params.action == "artifact_delete" {
            let ids = params.artifact_version_ids.clone().unwrap_or_default();
            if ids.is_empty() {
                return Err(ErrorCode::InvalidRequest.into());
            }
            let bundles = self.bundles()?;
            let t = tenant.to_owned();
            let (plan, fence) = self
                .control(READ_BUDGET, move |c| {
                    Ok((c.deletion_plan(&t, &ids)?, c.fence(&t)?))
                })
                .await?;
            generation = fence;
            let (mut files, mut bytes) = (0u64, 0u64);
            for v in &plan.versions {
                let (n, b) = bundles.size(&v.report_id, &v.version_id);
                files += n;
                bytes += b;
            }
            affected = json!({"report_versions": plan.versions.len()});
            let moves: Vec<String> = plan
                .latest
                .iter()
                .filter(|(_, before, after)| before != after)
                .map(|(report, _, after)| match after {
                    Some(v) => format!("{report} latest -> {v}"),
                    None => format!("{report} latest cleared"),
                })
                .collect();
            summary = format!(
                "delete {} report version(s): {files} file(s), {bytes} bytes, {} attributed assessment(s); latest pointers: {}; the reports are not revised automatically again; earlier downloads and external backups are not erased",
                plan.versions.len(),
                plan.assessments,
                if moves.is_empty() {
                    "unchanged".to_owned()
                } else {
                    moves.join(", ")
                },
            );
        }
        let value = serde_json::to_value(&params).map_err(|_| ErrorCode::InvalidRequest)?;
        let actor = Self::authority_actor(p);
        let tenant_owned = tenant.to_owned();
        let import_upload = (params.action == "import_finalize")
            .then(|| params.upload_id.clone())
            .flatten();
        if params.action == "import_finalize" && import_upload.is_none() {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let action = params.action.clone();
        let opened = self
            .control(SUBMIT_BUDGET, move |c| {
                let now = airs_runtime_core::time::now_ms();
                let r = match &import_upload {
                    Some(u) => c.preview_import(&tenant_owned, &actor, u, &value, generation, now),
                    None => c.preview(&tenant_owned, &actor, &action, &value, generation, now),
                };
                r.map_err(|e| Failure::from(e.code()))
            })
            .await?;
        Ok(json!({
            "schema_version": 1,
            "action": params.action,
            "affected": affected,
            "data_generation": generation,
            "expires_at": utc(opened.expires_ms),
            "parameter_digest": opened.digest,
            "preview_id": opened.preview_id,
            "proof_nonce": opened.nonce,
            "summary": summary,
        }))
    }

    /// `lifecycle_execute` (Administrator + direct proof or a one-use approval).
    async fn lifecycle_execute(
        &self,
        p: &Principal,
        live: &Live,
        tenant: &str,
        key: &str,
        epoch: &str,
        params: LifecycleExecuteParams,
    ) -> Result<Value, Failure> {
        let max_age = Self::admin(live)?;
        let action = self.preview_action(tenant, &params.preview_id).await?;
        let generation = self.generation_for(live, tenant, &action).await?;
        let proof = Self::proof(p, live);
        let actor = Self::authority_actor(p);
        let (t, k, e, rev) = (
            tenant.to_owned(),
            key.to_owned(),
            epoch.to_owned(),
            live.config.revision.clone(),
        );
        let limits = (
            live.config.limits.queue_per_tenant,
            live.config.limits.queue_per_principal,
        );
        let job = self
            .control(SUBMIT_BUDGET, move |c| {
                let now = airs_runtime_core::time::now_ms();
                let a = crate::authority_store::Attempt {
                    tenant: &t,
                    actor: &actor,
                    preview_id: &params.preview_id,
                    digest: &params.parameter_digest,
                    generation,
                    key: &k,
                    expected_epoch: &e,
                    config_revision: &rev,
                    queue_tenant: limits.0,
                    queue_principal: limits.1,
                };
                let id = match &params.approval_id {
                    Some(approval) => c.execute_with_approval(&a, approval, now),
                    None => c.execute_direct(&a, proof.as_ref(), max_age, now),
                }
                .map_err(|e| Failure::from(e.code()))?;
                c.job(&t, &id)?.ok_or(ErrorCode::StorageUnavailable.into())
            })
            .await?;
        Ok(json!({"schema_version": 1, "job": Self::job_view(&job), "joined_existing": false}))
    }

    /// The binding a preview of `action` carries: the archive's mutation generation, or
    /// for `recover` the archive and journal identity (an archive that needs recovery has
    /// a journal, which the ordinary reader refuses; M5 review 2 finding 9).
    async fn generation_for(
        &self,
        live: &Live,
        tenant: &str,
        action: &str,
    ) -> Result<i64, Failure> {
        // A report-artifact deletion binds the reports fence (it advances with every
        // deletion and prune, so a stale preview of a since-deleted set is refused).
        if action == "artifact_delete" {
            let t = tenant.to_owned();
            return self.control(READ_BUDGET, move |c| Ok(c.fence(&t)?)).await;
        }
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let cloud = cloud(&live.config)?;
        let root = storage.history_root.clone();
        let recover = action == "recover";
        Ok(read(move |session| {
            if recover {
                airs_history_store::recovery_identity(&root, &cloud, READ_BUDGET, session)
            } else {
                airs_history_store::inspect::data_generation(&root, &cloud, session)
            }
            .map_err(|_| ErrorCode::StorageUnavailable)
        })
        .await?)
    }

    /// The action of a stored preview (tenant-scoped), for its execution binding.
    async fn preview_action(&self, tenant: &str, preview_id: &str) -> Result<String, Failure> {
        let (t, id) = (tenant.to_owned(), preview_id.to_owned());
        self.control(READ_BUDGET, move |c| {
            Ok(c.preview_params(&t, &id)
                .map_err(|e| Failure::from(e.code()))?["action"]
                .as_str()
                .unwrap_or_default()
                .to_owned())
        })
        .await
    }

    /// `admin_confirm` (Administrator + direct proof): a one-use approval of another
    /// actor's preview (never one's own).
    async fn admin_confirm(
        &self,
        p: &Principal,
        live: &Live,
        tenant: &str,
        params: ConfirmParams,
    ) -> Result<Value, Failure> {
        let max_age = Self::admin(live)?;
        let action = self.preview_action(tenant, &params.preview_id).await?;
        let generation = self.generation_for(live, tenant, &action).await?;
        let proof = Self::proof(p, live);
        let actor = Self::authority_actor(p);
        let t = tenant.to_owned();
        let (id, expires) = self
            .control(SUBMIT_BUDGET, move |c| {
                let now = airs_runtime_core::time::now_ms();
                let id = c
                    .confirm(
                        &t,
                        &actor,
                        &params.preview_id,
                        &params.parameter_digest,
                        generation,
                        proof.as_ref(),
                        max_age,
                        now,
                    )
                    .map_err(|e| Failure::from(e.code()))?;
                let expires = c
                    .approval_expires(&id)
                    .map_err(|e| Failure::from(e.code()))?;
                Ok((id, expires))
            })
            .await?;
        Ok(json!({"schema_version": 1, "approval_id": id, "expires_at": utc(expires)}))
    }

    /// `transfer_upload_begin` (Administrator + direct proof): consumes an
    /// `upload_admit` preview and creates an owned, private staging upload of the
    /// declared length and hash.
    async fn upload_begin(
        &self,
        p: &Principal,
        live: &Live,
        tenant: &str,
        key: &str,
        epoch: &str,
        params: ConfirmParams,
    ) -> Result<Value, Failure> {
        let max_age = Self::admin(live)?;
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        // Staging pauses with publication, at reserve + 512 MiB (§9.5).
        if !storage.space_bounded(tenant).await.allows_publication() {
            return Err(ErrorCode::StorageUnavailable.into());
        }
        let generation = self.generation_for(live, tenant, "upload_admit").await?;
        let proof = Self::proof(p, live);
        let actor = Self::authority_actor(p);
        let (t, k, e, rev) = (
            tenant.to_owned(),
            key.to_owned(),
            epoch.to_owned(),
            live.config.revision.clone(),
        );
        let limits = (
            live.config.limits.queue_per_tenant,
            live.config.limits.queue_per_principal,
        );
        // Abandoned uploads are swept before a new one reserves staging.
        self.sweep_uploads(&storage, tenant).await;
        let quota = live.config.limits.staging_quota_bytes;
        let (upload, total, sha, expires) = self
            .control(SUBMIT_BUDGET, move |c| {
                let declared = c
                    .preview_params(&t, &params.preview_id)
                    .map_err(|e| Failure::from(e.code()))?;
                let total = declared["upload"]["total_bytes"]
                    .as_u64()
                    .ok_or(ErrorCode::InvalidRequest)?;
                let sha = declared["upload"]["sha256"]
                    .as_str()
                    .ok_or(ErrorCode::InvalidRequest)?
                    .to_owned();
                let a = crate::authority_store::Attempt {
                    tenant: &t,
                    actor: &actor,
                    preview_id: &params.preview_id,
                    digest: &params.parameter_digest,
                    generation,
                    key: &k,
                    expected_epoch: &e,
                    config_revision: &rev,
                    queue_tenant: limits.0,
                    queue_principal: limits.1,
                };
                let id = c
                    .upload_begin_within(
                        &a,
                        proof.as_ref(),
                        max_age,
                        total,
                        quota,
                        airs_runtime_core::time::now_ms(),
                    )
                    .map_err(|e| Failure::from(e.code()))?;
                let expires = c
                    .upload_expires(&t, &id)
                    .map_err(|e| Failure::from(e.code()))?;
                Ok((id, total, sha, expires))
            })
            .await?;
        let path = crate::lifecycle::upload_path(&storage.admitted.path, &upload);
        let created = (|| {
            use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
            let dir = path.parent().expect("staging dir");
            match std::fs::DirBuilder::new().mode(0o700).create(dir) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(_) => Ok(()),
                // A retry with the same key returns the same upload: keep its staging.
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
                Err(e) => Err(e),
            }
        })();
        if created.is_err() {
            return Err(ErrorCode::StorageUnavailable.into());
        }
        Ok(json!({
            "schema_version": 1,
            "upload_id": upload,
            "chunk_bytes": 65536,
            "total_bytes": total,
            "sha256": sha,
            // The persisted expiry: a replayed begin reports the original window.
            "expires_at": utc(expires),
        }))
    }

    /// The upload staging sweep (on every upload begin, and by the worker on every
    /// pass): expired uploads are aborted, and the staging of every aborted upload and of
    /// every upload whose consuming job ended is removed (verified owned files only) and
    /// only then marked removed, releasing its quota. Best effort: anything left is swept
    /// again.
    pub(crate) async fn sweep_uploads(&self, storage: &Arc<Storage>, tenant: &str) {
        let (s, t) = (storage.clone(), tenant.to_owned());
        let _ = tokio::task::spawn_blocking(move || sweep_staging(&s, &t)).await;
    }

    /// `transfer_upload_chunk` (upload owner): one contiguous, hash-checked chunk (M5
    /// review 1 finding 6). Transitions of one upload are serialized; the bytes are
    /// written at their offset and synced *before* the progress is recorded, so a failed
    /// write or a crash in between leaves the progress where it was and the same chunk
    /// can be sent again; a chunk already recorded (a retry after a lost response) is
    /// accepted only if the staged bytes at its offset are exactly its bytes.
    async fn upload_chunk(
        &self,
        p: &Principal,
        tenant: &str,
        params: UploadChunkParams,
    ) -> Result<Value, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        if !storage.space_bounded(tenant).await.allows_publication() {
            return Err(ErrorCode::StorageUnavailable.into());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&params.data_base64)
            .map_err(|_| ErrorCode::InvalidRequest)?;
        if bytes.is_empty()
            || bytes.len() > 65536
            || hex(&Sha256::digest(&bytes)) != params.chunk_sha256
        {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let path = staging_path(&storage.admitted.path, &params.upload_id)?;
        let lock = upload_lock(&params.upload_id);
        let _serialized = lock.lock().await;
        let actor = Self::authority_actor(p);
        let (t, id, offset, len) = (
            tenant.to_owned(),
            params.upload_id.clone(),
            params.offset,
            bytes.len() as u64,
        );
        let a2 = actor.clone();
        let next = self
            .control(SUBMIT_BUDGET, move |c| {
                c.upload_chunk_state(&t, &a2, &id, offset, len, airs_runtime_core::time::now_ms())
                    .map_err(|e| Failure::from(e.code()))
            })
            .await?;
        let (p2, b2) = (path.clone(), bytes.clone());
        let staged = tokio::task::spawn_blocking(move || -> std::io::Result<bool> {
            use std::os::unix::fs::FileExt;
            let f = open_owned_staging(&p2, next.is_some())?;
            if next.is_none() {
                // Already recorded: the retry is the same chunk only if its bytes are.
                let mut held = vec![0u8; b2.len()];
                f.read_exact_at(&mut held, offset)?;
                return Ok(held == b2);
            }
            f.write_all_at(&b2, offset)?;
            f.sync_data()?;
            Ok(true)
        })
        .await
        .map_err(|_| ErrorCode::StorageUnavailable)?
        .map_err(|_| ErrorCode::StorageUnavailable)?;
        if !staged {
            return Err(ErrorCode::IdempotencyConflict.into());
        }
        if next.is_none() {
            let (t, id) = (tenant.to_owned(), params.upload_id.clone());
            let received = self
                .control(READ_BUDGET, move |c| {
                    c.upload_progress(&t, &id)
                        .map_err(|e| Failure::from(e.code()))
                })
                .await?;
            return Ok(json!({"schema_version": 1, "received_bytes": received}));
        }
        let (t, id) = (tenant.to_owned(), params.upload_id.clone());
        let received = self
            .control(SUBMIT_BUDGET, move |c| {
                c.upload_chunk(
                    &t,
                    &actor,
                    &id,
                    offset,
                    len,
                    airs_runtime_core::time::now_ms(),
                )
                .map_err(|e| Failure::from(e.code()))
            })
            .await?;
        Ok(json!({"schema_version": 1, "received_bytes": received}))
    }

    /// `transfer_upload_finalize` (upload owner), M5 review 1 finding 7: the owner and
    /// the complete length are checked before any content is read; the staged bytes are
    /// then hashed by streaming and validated against the whole admission declaration
    /// (kind, length, SHA-256, the expected TSG and environment, and the content for its
    /// kind, with the service's complete secret registry); only a valid upload is
    /// published `finalized` (importable). An invalid upload is aborted and its staging
    /// removed: it can never be imported. Validations run one at a time (`overloaded`
    /// otherwise).
    async fn upload_finalize(
        &self,
        p: &Principal,
        live: &Live,
        tenant: &str,
        params: UploadIdParams,
    ) -> Result<Value, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let path = staging_path(&storage.admitted.path, &params.upload_id)?;
        let lock = upload_lock(&params.upload_id);
        let _serialized = lock.lock().await;
        let actor = Self::authority_actor(p);
        let (t, id, a2) = (tenant.to_owned(), params.upload_id.clone(), actor.clone());
        let (state, total, declared) = self
            .control(SUBMIT_BUDGET, move |c| {
                c.upload_finalize_admit(&t, &a2, &id, airs_runtime_core::time::now_ms())
                    .map_err(|e| Failure::from(e.code()))
            })
            .await?;
        if state == "finalized" {
            return Ok(
                json!({"schema_version": 1, "upload_id": params.upload_id, "validated": true}),
            );
        }
        let Ok(_permit) = storage.validations.try_acquire() else {
            return Err(Failure {
                code: ErrorCode::Overloaded,
                retry_after_seconds: Some(5),
            });
        };
        let secrets = service_secrets(&live.config)?;
        let cloud = cloud(&live.config)?;
        let p2 = path.clone();
        let checked = tokio::task::spawn_blocking(move || {
            validate_upload(&p2, total, &declared, &cloud, &secrets)
        })
        .await
        .map_err(|_| ErrorCode::StorageUnavailable)?;
        let (t, id) = (tenant.to_owned(), params.upload_id.clone());
        match checked {
            Ok((sha, records)) => {
                self.control(SUBMIT_BUDGET, move |c| {
                    c.upload_finalize(&t, &actor, &id, &sha, airs_runtime_core::time::now_ms())
                        .map_err(|e| Failure::from(e.code()))
                })
                .await?;
                Ok(json!({
                    "schema_version": 1,
                    "upload_id": params.upload_id,
                    "validated": true,
                    "records": records,
                }))
            }
            Err(reason) => {
                self.control(SUBMIT_BUDGET, move |c| {
                    c.upload_reject(&t, &actor, &id)
                        .map_err(|e| Failure::from(e.code()))
                })
                .await?;
                remove_owned_staging(&storage.admitted.path, &params.upload_id);
                Ok(json!({
                    "schema_version": 1,
                    "upload_id": params.upload_id,
                    "validated": false,
                    "reason": reason,
                }))
            }
        }
    }

    /// `transfer_upload_abort` (upload owner): never after import; staging is removed.
    async fn upload_abort(
        &self,
        p: &Principal,
        tenant: &str,
        params: UploadIdParams,
    ) -> Result<Value, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let actor = Self::authority_actor(p);
        let (t, id) = (tenant.to_owned(), params.upload_id.clone());
        self.control(SUBMIT_BUDGET, move |c| {
            c.upload_abort(&t, &actor, &id, airs_runtime_core::time::now_ms())
                .map_err(|e| Failure::from(e.code()))
        })
        .await?;
        remove_owned_staging(&storage.admitted.path, &params.upload_id);
        Ok(json!({"schema_version": 1, "upload_id": params.upload_id, "aborted": true}))
    }

    /// `export_read` (Administrator): hash-bound chunks of a finished export job.
    async fn export_read(&self, tenant: &str, params: ExportReadParams) -> Result<Value, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        if !dto::is_job_id(&params.job_id) || params.length == 0 || params.length > 65536 {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let (t, id) = (tenant.to_owned(), params.job_id.clone());
        let job = self
            .control(READ_BUDGET, move |c| Ok(c.job(&t, &id)?))
            .await?
            .filter(|j| {
                j.state == State::Succeeded
                    && j.result
                        .as_ref()
                        .is_some_and(|r| r["operation"] == "export")
            })
            .ok_or(ErrorCode::NotFound)?;
        let file_sha = job
            .result
            .as_ref()
            .and_then(|r| r["sha256"].as_str())
            .unwrap_or_default()
            .to_owned();
        let path = crate::lifecycle::export_path(&storage.admitted.path, &params.job_id);
        let (offset, length) = (params.offset, params.length);
        let (chunk, total) =
            tokio::task::spawn_blocking(move || -> std::io::Result<(Vec<u8>, u64)> {
                use std::os::unix::fs::FileExt;
                let f = std::fs::File::open(&path)?;
                let total = f.metadata()?.len();
                let n = length.min(total.saturating_sub(offset)) as usize;
                let mut buf = vec![0u8; n];
                f.read_exact_at(&mut buf, offset)?;
                Ok((buf, total))
            })
            .await
            .map_err(|_| ErrorCode::StorageUnavailable)?
            .map_err(|_| ErrorCode::NotFound)?;
        if offset > total {
            return Err(ErrorCode::InvalidRequest.into());
        }
        Ok(json!({
            "schema_version": 1,
            "offset": offset,
            "length": chunk.len(),
            "total_bytes": total,
            "data_base64": base64::engine::general_purpose::STANDARD.encode(&chunk),
            "chunk_sha256": hex(&Sha256::digest(&chunk)),
            "file_sha256": file_sha,
        }))
    }

    /// `schedules_list` (Reader).
    async fn schedules_list(&self, tenant: &str) -> Result<Value, Failure> {
        let tenant = tenant.to_owned();
        let schedules = self
            .control(READ_BUDGET, move |c| {
                Ok(c.schedules_list(&tenant, airs_runtime_core::time::now_ms())?)
            })
            .await?;
        Ok(json!({"schema_version": 1, "schedules": schedules}))
    }

    /// `schedules_set` (Operator, explicit): the collection schedule only in M4.
    async fn schedules_set(
        &self,
        p: &Principal,
        tenant: &str,
        key: &str,
        epoch: &str,
        params: ScheduleSetParams,
    ) -> Result<Value, Failure> {
        let effective = dto::utc_ms(&params.effective_at).ok_or(ErrorCode::InvalidRequest)?;
        if !["collection", "daily_report", "daily_enrichment"].contains(&params.kind.as_str()) {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let digest =
            crate::control::canonical(&serde_json::to_value(&params).unwrap_or_default()).1;
        self.idempotent(
            p,
            tenant,
            "schedules_set",
            key,
            epoch,
            &digest,
            move |c, tenant| {
                let (id, revision) = c.schedule_set_kind(
                    tenant,
                    &params.kind,
                    params.enabled,
                    if params.kind == "collection" {
                        params.interval_minutes
                    } else {
                        None
                    },
                    effective,
                )?;
                Ok(json!({"schema_version": 1, "schedule_id": id, "revision": revision}))
            },
        )
        .await
    }

    /// `schedules_disable` (Operator, explicit).
    async fn schedules_disable(
        &self,
        p: &Principal,
        tenant: &str,
        key: &str,
        epoch: &str,
        params: ScheduleDisableParams,
    ) -> Result<Value, Failure> {
        let effective = dto::utc_ms(&params.effective_at).ok_or(ErrorCode::InvalidRequest)?;
        let digest =
            crate::control::canonical(&serde_json::to_value(&params).unwrap_or_default()).1;
        self.idempotent(
            p,
            tenant,
            "schedules_disable",
            key,
            epoch,
            &digest,
            move |c, tenant| {
                let (id, revision) = c.schedule_disable(tenant, &params.schedule_id, effective)?;
                Ok(json!({"schema_version": 1, "schedule_id": id, "revision": revision}))
            },
        )
        .await
    }

    /// One idempotent, keyed control mutation whose result is not a job (schedule
    /// changes): the same key and parameters return the stored result, other
    /// parameters are `idempotency_conflict`, another epoch is `control_restored`; the
    /// change and its recorded result commit atomically (one savepoint).
    #[allow(clippy::too_many_arguments)]
    async fn idempotent(
        &self,
        p: &Principal,
        tenant: &str,
        tool: &'static str,
        key: &str,
        epoch: &str,
        digest: &str,
        change: impl FnOnce(&mut Control, &str) -> Result<Value, Failure> + Send + 'static,
    ) -> Result<Value, Failure> {
        let (tenant, key, epoch, digest) = (
            tenant.to_owned(),
            format!("{tool}:{key}"),
            epoch.to_owned(),
            digest.to_owned(),
        );
        let caller = (p.subject.clone(), p.client_id.clone());
        self.control(SUBMIT_BUDGET, move |c| {
            let who = (caller.0.as_str(), caller.1.as_str());
            if let crate::ingest_store::Prior::Same(v) =
                c.ingest_prior(&tenant, who, &epoch, &key, &digest)?
            {
                return Ok(v);
            }
            // The change and its recorded result commit together, or neither does.
            c.begin_unit()?;
            let done = change(c, &tenant).and_then(|result| {
                Ok(c.ingest_record(
                    &tenant,
                    who,
                    &epoch,
                    &key,
                    &digest,
                    &result,
                    airs_runtime_core::time::now_ms(),
                )?)
            });
            c.end_unit(done.is_ok())?;
            done
        })
        .await
    }

    /// `history_query` (Reader): full-cohort aggregates for a range of at most 90
    /// days (`airs_history_store::query`), rows paged under a signed cursor bound to
    /// the caller, parameters and result set; display capped at 10,000 rows.
    async fn history_query(
        &self,
        p: &Principal,
        live: &Live,
        tenant: &str,
        params: QueryParams,
    ) -> Result<Value, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let (epoch, key) = storage.epoch().ok_or(ErrorCode::ControlRestored)?;
        let (since, until) = params.range.bounds(90 * 86_400_000)?;
        let mut apps = params.app_ids.clone().unwrap_or_default();
        apps.sort();
        apps.dedup();
        let group_by = params.group_by.clone().unwrap_or_default();
        let page = params.page.clone().unwrap_or_default();
        let limit = page.limit()?.min(100);
        let scope = scope_digest(
            "history_query",
            tenant,
            p,
            &epoch,
            &json!({"since": since, "until": until, "app_ids": apps, "group_by": group_by}),
        );
        let start = page
            .cursor
            .as_ref()
            .map(|t| decode_cursor(&key, t, &scope))
            .transpose()?;
        let cloud = cloud(&live.config)?;
        let root = storage.history_root.clone();
        let (cohort, mut rows) = read(move |session| {
            airs_history_store::query::cohort(
                &root,
                &cloud,
                (since, until),
                &apps,
                &group_by,
                session,
            )
            .map_err(|_| ErrorCode::InvalidRequest)
        })
        .await?
        .ok_or(ErrorCode::NotFound)?;
        let truncated = rows.len() > 10_000;
        rows.truncate(10_000);
        let items: Vec<Value> = rows
            .iter()
            .map(|r| json!({"key": r.key, "events": r.events, "malicious": r.malicious, "unknown": r.unknown, "allowed": r.allowed, "blocked": r.blocked}))
            .collect();
        let snapshot = hex(&Sha256::digest(
            serde_json::to_vec(&items).unwrap_or_default(),
        ));
        let offset = match start {
            None => 0,
            Some(c) if c.s != snapshot => return Err(ErrorCode::Conflict.into()),
            Some(c) if c.p > items.len() => return Err(ErrorCode::InvalidRequest.into()),
            Some(c) => c.p,
        };
        let tool = catalogue::get("history_query").expect("catalogued");
        let mut take = limit.min(items.len() - offset);
        loop {
            let end = offset + take;
            let next = (end < items.len()).then(|| {
                encode_cursor(
                    &key,
                    &CursorBody {
                        d: scope.clone(),
                        s: snapshot.clone(),
                        p: end,
                    },
                )
            });
            let value = json!({
                "schema_version": 1,
                "cohort": {"events": cohort.events, "unknown_time": cohort.unknown_time, "unattributed_receipts": cohort.unattributed_receipts},
                "rows": &items[offset..end],
                "display_truncated": truncated,
                "next_cursor": next,
            });
            if result_bytes(&CallToolResult::structured(value.clone())) <= tool.result_limit {
                return Ok(value);
            }
            if take <= 1 {
                return Err(ErrorCode::ResultTooLarge.into());
            }
            take -= 1;
        }
    }

    /// The private report bundle tree (M6).
    fn bundles(&self) -> Result<crate::reports::Bundles, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let dir = crate::storage::private_child(&storage.admitted, "reports")
            .map_err(|_| ErrorCode::StorageUnavailable)?;
        Ok(crate::reports::Bundles { root: dir.path })
    }

    /// Submit one Operator job (shared by the M6 job tools): validation is done by the
    /// caller; `quota` runs in the same control call before submission, and is skipped
    /// for a repeated key so a replay always returns its job.
    #[allow(clippy::too_many_arguments)]
    async fn submit_job(
        &self,
        p: &Principal,
        live: &Live,
        tenant: &str,
        key: &str,
        epoch: &str,
        operation: &'static str,
        params: Value,
        deadline_ms: i64,
        quota: impl FnOnce(&Control, i64) -> Result<(), Failure> + Send + 'static,
    ) -> Result<Value, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let now = airs_runtime_core::time::now_ms();
        let limits = live.config.limits.clone();
        let (tenant, key, epoch) = (tenant.to_owned(), key.to_owned(), epoch.to_owned());
        let actor = Actor::Principal {
            sub: p.subject.clone(),
            client_id: p.client_id.clone(),
        };
        let revision = live.config.revision.clone();
        let (submitted, job) = storage
            .control_bounded(SUBMIT_BUDGET, move |c| {
                if !c.key_known(&tenant, &actor, operation, &epoch, &key)? {
                    quota(c, now)?;
                }
                let submitted = c.submit(
                    Submission {
                        tenant: &tenant,
                        operation,
                        actor,
                        idempotency_key: &key,
                        expected_epoch: &epoch,
                        params: &params,
                        config_revision: &revision,
                        credential_revision: None,
                        deadline_ms: now + deadline_ms,
                        queue_tenant: limits.queue_per_tenant,
                        queue_principal: limits.queue_per_principal,
                        join_equivalent: false,
                        scheduled_slot: None,
                    },
                    now,
                )?;
                let job = c
                    .job(&tenant, &submitted.job_id)?
                    .ok_or(ErrorCode::StorageUnavailable)?;
                Ok((submitted, job))
            })
            .await?;
        Ok(
            json!({"schema_version": 1, "job": Self::job_view(&job), "joined_existing": submitted.joined}),
        )
    }

    /// `reports_create` (Operator): a manual report of a whole-hour range of at most 90
    /// days. Quotas are charged at admission (contract §6): `manual_reports_per_principal_hour`
    /// in any rolling hour and `manual_reports_per_tenant_day` per Chicago day; a
    /// cancelled job is never refunded.
    #[allow(clippy::too_many_arguments)]
    async fn reports_create(
        &self,
        p: &Principal,
        live: &Live,
        tenant: &str,
        key: &str,
        epoch: &str,
        params: ReportsCreateParams,
    ) -> Result<Value, Failure> {
        let (since, until) = params.range.bounds(90 * 86_400_000)?;
        if (until - since) % 3_600_000 != 0 {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let mut apps = params.app_ids.unwrap_or_default();
        if apps.len() > 100
            || apps
                .iter()
                .any(|a| !airs_runtime_core::tenant::scanners::valid_application(a))
        {
            return Err(ErrorCode::InvalidRequest.into());
        }
        apps.sort();
        apps.dedup();
        let title = params.title.unwrap_or_default();
        if title.chars().count() > 200 {
            return Err(ErrorCode::InvalidRequest.into());
        }
        // The report ID derives from the key's complete idempotency scope (tenant,
        // caller, recovery epoch, key) as a canonical tuple, so a replay has identical
        // parameters (and returns its job) instead of minting another report, and the same
        // key text under a later epoch is a different report (M6 review 3).
        let report_id = {
            let scope = serde_json::to_string(&json!([
                "manual-report",
                2,
                tenant,
                p.subject,
                p.client_id,
                epoch,
                key
            ]))
            .expect("a tuple of strings serializes");
            let h = Sha256::digest(scope.as_bytes());
            let mut b = [0u8; 16];
            b.copy_from_slice(&h[..16]);
            format!("r1_{}", uuid::Builder::from_random_bytes(b).into_uuid())
        };
        let job_params = json!({
            "report_id": report_id,
            "kind": "manual",
            "since_ms": since,
            "until_ms": until,
            "app_ids": apps,
            "title": title,
        });
        let per_hour = live.config.limits.manual_reports_per_principal_hour;
        let per_day = live.config.limits.manual_reports_per_tenant_day;
        let (t, sub, client) = (tenant.to_owned(), p.subject.clone(), p.client_id.clone());
        self.submit_job(
            p,
            live,
            tenant,
            key,
            epoch,
            "reports_create",
            job_params,
            72 * 3_600_000,
            move |c, now| {
                c.manual_report_quota(&t, (&sub, &client), (per_hour, per_day), now)
                    .map_err(Failure::from)
            },
        )
        .await
    }

    /// `enrichment_start` (Operator, explicit): strategic examples for a report's period.
    #[allow(clippy::too_many_arguments)]
    async fn enrichment_start(
        &self,
        p: &Principal,
        live: &Live,
        tenant: &str,
        key: &str,
        epoch: &str,
        params: EnrichmentParams,
    ) -> Result<Value, Failure> {
        let max = params.max_reports.unwrap_or(5);
        if !(1..=10).contains(&max) {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let (t, id) = (tenant.to_owned(), params.report_id.clone());
        let report = self
            .control(READ_BUDGET, move |c| Ok(c.report(&t, &id)?))
            .await?
            .ok_or(ErrorCode::NotFound)?;
        let job_params = json!({"since_ms": report.since_ms, "until_ms": report.until_ms, "max_reports": max, "report_id": report.report_id});
        self.submit_job(
            p,
            live,
            tenant,
            key,
            epoch,
            "enrichment_start",
            job_params,
            72 * 3_600_000,
            |_, _| Ok(()),
        )
        .await
    }

    /// `reports_list` (Reader): published versions newest first, paginated under a
    /// signed cursor bound to the filter and the version set.
    async fn reports_list(
        &self,
        p: &Principal,
        tenant: &str,
        params: ReportsListParams,
    ) -> Result<Value, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let (epoch, key) = storage.epoch().ok_or(ErrorCode::ControlRestored)?;
        let page = params.page.clone().unwrap_or_default();
        let limit = page.limit()?.min(50);
        let range = params
            .range
            .as_ref()
            .map(|r| r.bounds(i64::MAX))
            .transpose()?;
        let scope = scope_digest(
            "reports_list",
            tenant,
            p,
            &epoch,
            &json!({"kind": params.kind, "range": range}),
        );
        let start = page
            .cursor
            .as_ref()
            .map(|t| decode_cursor(&key, t, &scope))
            .transpose()?;
        let offset = start.as_ref().map_or(0, |c| c.p);
        let (t, kind) = (tenant.to_owned(), params.kind.clone());
        let (rows, total) = storage
            .control_bounded(READ_BUDGET, move |c| {
                let rows = c.versions(&t, kind.as_deref(), range, limit + 1, offset)?;
                let total = c.versions(&t, kind.as_deref(), range, 100_000, 0)?;
                Ok((rows, total))
            })
            .await?;
        let snapshot = hex(&Sha256::digest(
            total
                .iter()
                .map(|v| v.version_id.as_str())
                .collect::<Vec<_>>()
                .join(",")
                .as_bytes(),
        ));
        if start.is_some_and(|c| c.s != snapshot) {
            return Err(ErrorCode::Conflict.into());
        }
        let items: Vec<Value> = rows
            .iter()
            .take(limit)
            .map(|v| v.manifest.clone())
            .collect();
        let tool = catalogue::get("reports_list").expect("catalogued");
        let mut take = items.len();
        loop {
            let next = (take < rows.len()).then(|| {
                encode_cursor(
                    &key,
                    &CursorBody {
                        d: scope.clone(),
                        s: snapshot.clone(),
                        p: offset + take,
                    },
                )
            });
            let value = json!({"schema_version": 1, "items": &items[..take], "next_cursor": next});
            if result_bytes(&CallToolResult::structured(value.clone())) <= tool.result_limit {
                return Ok(value);
            }
            if take <= 1 {
                return Err(ErrorCode::ResultTooLarge.into());
            }
            take -= 1;
        }
    }

    /// `reports_get` (Reader): a version's manifest (latest by default) and summary.
    async fn reports_get(&self, tenant: &str, params: ReportsGetParams) -> Result<Value, Failure> {
        let t = tenant.to_owned();
        let (report, version) = self
            .control(READ_BUDGET, move |c| {
                let report = c.report(&t, &params.report_id)?;
                let wanted = params
                    .version_id
                    .clone()
                    .or_else(|| report.as_ref().and_then(|r| r.latest_version.clone()));
                let version = match wanted {
                    Some(v) => c.version(&t, &v)?,
                    None => None,
                };
                Ok((report, version))
            })
            .await?;
        let (Some(report), Some(version)) = (report, version) else {
            return Err(ErrorCode::NotFound.into());
        };
        if version.report_id != report.report_id {
            return Err(ErrorCode::NotFound.into());
        }
        Ok(json!({
            "schema_version": 1,
            "latest_version": report.latest_version,
            "manifest": version.manifest,
            "summary": version.summary,
        }))
    }

    /// `artifacts_read` (Reader): UTF-8 pages of `review.md` (or the summary) of at most
    /// 6,144 characters, under a cursor bound to the caller, version and file.
    async fn artifacts_read(
        &self,
        p: &Principal,
        tenant: &str,
        params: ArtifactsReadParams,
    ) -> Result<Value, Failure> {
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let (epoch, key) = storage.epoch().ok_or(ErrorCode::ControlRestored)?;
        if !["review.md", "summary.md"].contains(&params.name.as_str()) {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let (t, vid) = (tenant.to_owned(), params.version_id.clone());
        let version = self
            .control(READ_BUDGET, move |c| Ok(c.version(&t, &vid)?))
            .await?
            .ok_or(ErrorCode::NotFound)?;
        let bundles = self.bundles()?;
        let text = if params.name == "summary.md" {
            version.summary.clone()
        } else {
            let bytes = bundles.read(&version, "review.md")?;
            String::from_utf8(bytes).map_err(|_| ErrorCode::StorageUnavailable)?
        };
        let file_sha = hex(&Sha256::digest(text.as_bytes()));
        let scope = scope_digest(
            "artifacts_read",
            tenant,
            p,
            &epoch,
            &json!({"version_id": params.version_id, "name": params.name}),
        );
        let offset = match &params.cursor {
            None => 0,
            Some(c) => {
                let body = decode_cursor(&key, c, &scope)?;
                if body.s != file_sha {
                    return Err(ErrorCode::Conflict.into());
                }
                body.p
            }
        };
        let chars: Vec<char> = text.chars().collect();
        if offset > chars.len() {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let tool = catalogue::get("artifacts_read").expect("catalogued");
        let mut take = (chars.len() - offset).min(6144);
        loop {
            let end = offset + take;
            let segment: String = chars[offset..end].iter().collect();
            let next = (end < chars.len()).then(|| {
                encode_cursor(
                    &key,
                    &CursorBody {
                        d: scope.clone(),
                        s: file_sha.clone(),
                        p: end,
                    },
                )
            });
            let value = json!({"schema_version": 1, "segment": segment, "sha256": file_sha, "total_bytes": text.len(), "next_cursor": next});
            if result_bytes(&CallToolResult::structured(value.clone())) <= tool.result_limit
                || take == 0
            {
                return Ok(value);
            }
            take = take * 3 / 4;
        }
    }

    /// `artifacts_download` (Reader, CLI bound): one byte chunk of a published file,
    /// reauthorized on every call, its file verified against the manifest hash, and the
    /// chunk's own SHA-256 returned with the file's.
    async fn artifacts_download(
        &self,
        tenant: &str,
        params: ArtifactsDownloadParams,
    ) -> Result<Value, Failure> {
        if !crate::reports::FILES.contains(&params.name.as_str())
            || !(1..=65_536).contains(&params.length)
            || params.offset > 67_108_863
        {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let (t, vid) = (tenant.to_owned(), params.version_id.clone());
        let version = self
            .control(READ_BUDGET, move |c| Ok(c.version(&t, &vid)?))
            .await?
            .ok_or(ErrorCode::NotFound)?;
        let bytes = self.bundles()?.read(&version, &params.name)?;
        let offset = params.offset as usize;
        if offset > bytes.len() {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let end = (offset + params.length as usize).min(bytes.len());
        let chunk = &bytes[offset..end];
        Ok(json!({
            "schema_version": 1,
            "offset": offset,
            "length": chunk.len(),
            "total_bytes": bytes.len(),
            "data_base64": base64::engine::general_purpose::STANDARD.encode(chunk),
            "chunk_sha256": hex(&Sha256::digest(chunk)),
            "file_sha256": hex(&Sha256::digest(&bytes)),
        }))
    }

    /// `assessments_submit` (Operator, explicit for agents): an attributed assessment of
    /// a version, bound to the exact `review.md` it assessed (its SHA-256). It is stored
    /// beside the version and never changes the bundle or its evidence.
    async fn assessments_submit(
        &self,
        p: &Principal,
        tenant: &str,
        key: &str,
        epoch: &str,
        params: AssessmentParams,
    ) -> Result<Value, Failure> {
        if params.markdown.len() > 65_536
            || params
                .reviewer_label
                .as_ref()
                .is_some_and(|l| l.chars().count() > 128)
        {
            return Err(ErrorCode::InvalidRequest.into());
        }
        let digest =
            crate::control::canonical(&serde_json::to_value(&params).unwrap_or_default()).1;
        let label = p.grant.label.clone().unwrap_or_else(|| p.client_id.clone());
        let caller = (p.subject.clone(), p.client_id.clone());
        self.idempotent(p, tenant, "assessments_submit", key, epoch, &digest, move |c, tenant| {
            let version = c.version(tenant, &params.version_id)?.ok_or(ErrorCode::NotFound)?;
            let review = version.manifest["files"]
                .as_array()
                .and_then(|f| f.iter().find(|e| e["name"] == "review.md"))
                .and_then(|e| e["sha256"].as_str().map(str::to_owned));
            if review.as_deref() != Some(params.review_sha256.as_str()) {
                return Err(ErrorCode::Conflict.into());
            }
            let id = c.add_assessment(tenant, &params.version_id, (&caller.0, &caller.1, &label), &params.review_sha256, &params.markdown, airs_runtime_core::time::now_ms())?;
            Ok(json!({"schema_version": 1, "assessment_id": id, "attributed_to": {"kind": "principal", "label": label}}))
        })
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn collection_start(
        &self,
        p: &Principal,
        live: &Live,
        tenant: &str,
        key: &str,
        expected_epoch: &str,
        params: CollectionParams,
    ) -> Result<Value, Failure> {
        // Validation before any effect (contract §4.2, §6).
        let mut apps = params.app_ids.clone().unwrap_or_default();
        if apps.len() > 100
            || apps
                .iter()
                .any(|id| !airs_runtime_core::tenant::scanners::valid_application(id))
        {
            return Err(ErrorCode::InvalidRequest.into());
        }
        apps.sort();
        apps.dedup();
        let range = match (params.mode, &params.range) {
            (CollectionMode::Recent, None) => None,
            (CollectionMode::Backfill, Some(r)) => {
                let (since, until) = r.bounds(dto::MAX_BACKFILL_RANGE_MS)?;
                if until > airs_runtime_core::time::now_ms() {
                    return Err(ErrorCode::InvalidRequest.into());
                }
                Some((since, until))
            }
            _ => return Err(ErrorCode::InvalidRequest.into()),
        };
        let canonical = json!({
            "mode": match params.mode { CollectionMode::Recent => "recent", CollectionMode::Backfill => "backfill" },
            "range": range.map(|(a, b)| json!({"since_ms": a, "until_ms": b})),
            "app_ids": apps,
        });
        let storage = self.storage.clone().ok_or(ErrorCode::StorageUnavailable)?;
        let now = airs_runtime_core::time::now_ms();
        let limits = &live.config.limits;
        let (tenant, key, epoch) = (tenant.to_owned(), key.to_owned(), expected_epoch.to_owned());
        let actor = Actor::Principal {
            sub: p.subject.clone(),
            client_id: p.client_id.clone(),
        };
        let revision = live.config.revision.clone();
        let credentials = live.config.airs.management_revision.clone();
        let (queue_tenant, queue_principal) = (limits.queue_per_tenant, limits.queue_per_principal);
        // One bounded control transaction pair: submit, then read the job back.
        let (submitted, job) = storage
            .control_bounded(SUBMIT_BUDGET, move |c| {
                let submitted = c.submit(
                    Submission {
                        tenant: &tenant,
                        operation: "collection_start",
                        actor,
                        idempotency_key: &key,
                        expected_epoch: &epoch,
                        params: &canonical,
                        config_revision: &revision,
                        credential_revision: Some(&credentials),
                        // Collections default to a 24 h deadline (contract §10).
                        deadline_ms: now + 86_400_000,
                        queue_tenant,
                        queue_principal,
                        join_equivalent: true,
                        scheduled_slot: None,
                    },
                    now,
                )?;
                let job = c
                    .job(&tenant, &submitted.job_id)?
                    .ok_or(ErrorCode::StorageUnavailable)?;
                Ok((submitted, job))
            })
            .await?;
        Ok(
            json!({"schema_version": 1, "job": Self::job_view(&job), "joined_existing": submitted.joined}),
        )
    }

    async fn dispatch(
        &self,
        name: &str,
        args: Map<String, Value>,
        p: &Principal,
        live: &Live,
        ctx: &RequestContext<RoleServer>,
    ) -> Result<CallToolResult, Refusal> {
        let Some(tool) = catalogue::get(name) else {
            return deny(ErrorCode::NotFound, "unknown tool");
        };
        if !granted(p, tool) {
            return deny(
                ErrorCode::Forbidden,
                "tool not granted to this caller and agent",
            );
        }
        if admin_side_door(p, tool) {
            return deny(
                ErrorCode::Forbidden,
                "administrative identities mutate only through proof-bound tools",
            );
        }
        if !catalogue::implemented(name) {
            return deny(
                ErrorCode::InvalidRequest,
                "tool not available in this service version",
            );
        }
        if live.config.service.maintenance && name != "service_info" {
            return deny(
                ErrorCode::MaintenanceRequired,
                "the service is in maintenance",
            );
        }
        let raw = Value::Object(args).to_string();
        let parsed = Parsed::from(name, &raw, tool.idempotency_key);
        let (tenant, idempotency, request) = match parsed {
            Ok(v) => v,
            Err(ErrorCode::UnsupportedVersion) => {
                return deny(ErrorCode::UnsupportedVersion, "unsupported schema_version");
            }
            Err(_) => return deny(ErrorCode::InvalidRequest, "invalid request envelope"),
        };
        if tenant != live.config.service.tenant {
            return deny(ErrorCode::Forbidden, "tenant is not served by this service");
        }
        let result = match request {
            Parsed::None => match name {
                "service_info" => Ok(self.service_info(p, live, ctx)),
                "history_status" => self.history_status(live).await.map_err(Failure::from),
                "schedules_list" => self.schedules_list(&tenant).await,
                _ => Err(ErrorCode::InvalidRequest.into()),
            },
            Parsed::Coverage(params) => self
                .history_coverage(p, live, &tenant, params)
                .await
                .map_err(Failure::from),
            Parsed::JobsList(params) => self.jobs_list(p, &tenant, params).await,
            Parsed::JobId(params) if name == "jobs_cancel" => {
                self.jobs_cancel(p, &tenant, params).await
            }
            Parsed::JobId(params) => self.jobs_get(p, &tenant, params).await,
            Parsed::Preview(params) => self.lifecycle_preview(p, live, &tenant, params).await,
            Parsed::Execute(params) => {
                let (key, epoch) = idempotency.expect("idempotent tools carry a key");
                self.lifecycle_execute(p, live, &tenant, &key, &epoch, params)
                    .await
            }
            Parsed::Confirm(params) => self.admin_confirm(p, live, &tenant, params).await,
            Parsed::UploadBegin(params) => {
                let (key, epoch) = idempotency.expect("idempotent tools carry a key");
                self.upload_begin(p, live, &tenant, &key, &epoch, params)
                    .await
            }
            Parsed::UploadChunk(params) => self.upload_chunk(p, &tenant, params).await,
            Parsed::UploadId(params) if name == "transfer_upload_abort" => {
                self.upload_abort(p, &tenant, params).await
            }
            Parsed::UploadId(params) => self.upload_finalize(p, live, &tenant, params).await,
            Parsed::ExportRead(params) => self.export_read(&tenant, params).await,
            Parsed::ScheduleSet(params) => {
                let (key, epoch) = idempotency.expect("idempotent tools carry a key");
                self.schedules_set(p, &tenant, &key, &epoch, params).await
            }
            Parsed::ScheduleDisable(params) => {
                let (key, epoch) = idempotency.expect("idempotent tools carry a key");
                self.schedules_disable(p, &tenant, &key, &epoch, params)
                    .await
            }
            Parsed::Receipts(params) => {
                let (key, epoch) = idempotency.expect("idempotent tools carry a key");
                self.receipts_ingest(p, live, &tenant, &key, &epoch, params)
                    .await
            }
            Parsed::Collection(params) => {
                let (key, epoch) = idempotency.expect("idempotent tools carry a key");
                self.collection_start(p, live, &tenant, &key, &epoch, params)
                    .await
            }
            Parsed::ReportsCreate(params) => {
                let (key, epoch) = idempotency.expect("idempotent tools carry a key");
                self.reports_create(p, live, &tenant, &key, &epoch, params)
                    .await
            }
            Parsed::ReportsList(params) => self.reports_list(p, &tenant, params).await,
            Parsed::Query(params) => self.history_query(p, live, &tenant, params).await,
            Parsed::ReportsGet(params) => self.reports_get(&tenant, params).await,
            Parsed::ArtifactsRead(params) => self.artifacts_read(p, &tenant, params).await,
            Parsed::ArtifactsDownload(params) => self.artifacts_download(&tenant, params).await,
            Parsed::Assessment(params) => {
                let (key, epoch) = idempotency.expect("idempotent tools carry a key");
                self.assessments_submit(p, &tenant, &key, &epoch, params)
                    .await
            }
            Parsed::Enrichment(params) => {
                let (key, epoch) = idempotency.expect("idempotent tools carry a key");
                self.enrichment_start(p, live, &tenant, &key, &epoch, params)
                    .await
            }
        };
        match result {
            Ok(value) => {
                let result = CallToolResult::structured(value);
                if result_bytes(&result) > tool.result_limit {
                    return deny(
                        ErrorCode::ResultTooLarge,
                        "result exceeds its response bound",
                    );
                }
                Ok(result)
            }
            Err(f) => Err((f.code, reason(f.code), f.retry_after_seconds)),
        }
    }
}

/// A strictly parsed tool request: its tenant, its idempotency key and expected epoch
/// (idempotent tools only), and its typed parameters.
enum Parsed {
    None,
    Preview(LifecyclePreviewParams),
    Execute(LifecycleExecuteParams),
    Confirm(ConfirmParams),
    UploadBegin(ConfirmParams),
    UploadChunk(UploadChunkParams),
    UploadId(UploadIdParams),
    ExportRead(ExportReadParams),
    Query(QueryParams),
    ReportsCreate(ReportsCreateParams),
    ReportsList(ReportsListParams),
    ReportsGet(ReportsGetParams),
    ArtifactsRead(ArtifactsReadParams),
    ArtifactsDownload(ArtifactsDownloadParams),
    Assessment(AssessmentParams),
    Enrichment(EnrichmentParams),
    ScheduleSet(ScheduleSetParams),
    ScheduleDisable(ScheduleDisableParams),
    Receipts(ReceiptsParams),
    Coverage(CoverageParams),
    JobsList(JobsListParams),
    JobId(JobIdParams),
    Collection(CollectionParams),
}

type ParsedRequest = (String, Option<(String, String)>, Parsed);

impl Parsed {
    fn from(name: &str, raw: &str, idempotent: bool) -> Result<ParsedRequest, ErrorCode> {
        fn split<T>(r: dto::Request<T>) -> (String, Option<(String, String)>, T) {
            let idempotency = r.idempotency_key.zip(r.expected_epoch);
            (r.tenant, idempotency, r.params)
        }
        Ok(match name {
            "history_coverage" => {
                let (t, i, p) = split(dto::parse_tool_request::<CoverageParams>(raw, idempotent)?);
                (t, i, Parsed::Coverage(p))
            }
            "jobs_list" => {
                let (t, i, p) = split(dto::parse_tool_request::<JobsListParams>(raw, idempotent)?);
                (t, i, Parsed::JobsList(p))
            }
            "jobs_get" | "jobs_cancel" => {
                let (t, i, p) = split(dto::parse_tool_request::<JobIdParams>(raw, idempotent)?);
                (t, i, Parsed::JobId(p))
            }
            "lifecycle_preview" => {
                let (t, i, p) = split(dto::parse_tool_request::<LifecyclePreviewParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::Preview(p))
            }
            "lifecycle_execute" => {
                let (t, i, p) = split(dto::parse_tool_request::<LifecycleExecuteParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::Execute(p))
            }
            "admin_confirm" => {
                let (t, i, p) = split(dto::parse_tool_request::<ConfirmParams>(raw, idempotent)?);
                (t, i, Parsed::Confirm(p))
            }
            "transfer_upload_begin" => {
                let (t, i, p) = split(dto::parse_tool_request::<ConfirmParams>(raw, idempotent)?);
                (t, i, Parsed::UploadBegin(p))
            }
            "transfer_upload_chunk" => {
                let (t, i, p) = split(dto::parse_tool_request::<UploadChunkParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::UploadChunk(p))
            }
            "transfer_upload_finalize" | "transfer_upload_abort" => {
                let (t, i, p) = split(dto::parse_tool_request::<UploadIdParams>(raw, idempotent)?);
                (t, i, Parsed::UploadId(p))
            }
            "export_read" => {
                let (t, i, p) = split(dto::parse_tool_request::<ExportReadParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::ExportRead(p))
            }
            "schedules_set" => {
                let (t, i, p) = split(dto::parse_tool_request::<ScheduleSetParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::ScheduleSet(p))
            }
            "schedules_disable" => {
                let (t, i, p) = split(dto::parse_tool_request::<ScheduleDisableParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::ScheduleDisable(p))
            }
            "history_query" => {
                let (t, i, p) = split(dto::parse_tool_request::<QueryParams>(raw, idempotent)?);
                (t, i, Parsed::Query(p))
            }
            "reports_create" => {
                let (t, i, p) = split(dto::parse_tool_request::<ReportsCreateParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::ReportsCreate(p))
            }
            "reports_list" => {
                let (t, i, p) = split(dto::parse_tool_request::<ReportsListParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::ReportsList(p))
            }
            "reports_get" => {
                let (t, i, p) = split(dto::parse_tool_request::<ReportsGetParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::ReportsGet(p))
            }
            "artifacts_read" => {
                let (t, i, p) = split(dto::parse_tool_request::<ArtifactsReadParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::ArtifactsRead(p))
            }
            "artifacts_download" => {
                let (t, i, p) = split(dto::parse_tool_request::<ArtifactsDownloadParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::ArtifactsDownload(p))
            }
            "assessments_submit" => {
                let (t, i, p) = split(dto::parse_tool_request::<AssessmentParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::Assessment(p))
            }
            "enrichment_start" => {
                let (t, i, p) = split(dto::parse_tool_request::<EnrichmentParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::Enrichment(p))
            }
            "receipts_ingest" => {
                let (t, i, p) = split(dto::parse_tool_request::<ReceiptsParams>(raw, idempotent)?);
                (t, i, Parsed::Receipts(p))
            }
            "collection_start" => {
                let (t, i, p) = split(dto::parse_tool_request::<CollectionParams>(
                    raw, idempotent,
                )?);
                (t, i, Parsed::Collection(p))
            }
            _ => {
                let (t, i, _) = split(dto::parse_tool_request::<NoParams>(raw, idempotent)?);
                (t, i, Parsed::None)
            }
        })
    }
}

/// The compiled reason for a tool-body failure code (never SQLite, SDK or HTTP text).
fn reason(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::InvalidRequest => "invalid request parameters",
        ErrorCode::NotFound => "not found for this caller and service tenant",
        ErrorCode::IdempotencyConflict => "the idempotency key was used with different parameters",
        ErrorCode::ControlRestored => {
            "the idempotency key belongs to another recovery epoch; read service_info and retry with a new key"
        }
        ErrorCode::Overloaded => "the job queue is full; retry later",
        ErrorCode::Conflict => "the result set changed; restart paging without a cursor",
        ErrorCode::StorageUnavailable => "history storage is unavailable",
        ErrorCode::ResultTooLarge => "result exceeds its response bound",
        _ => "request refused",
    }
}

impl ServerHandler for Service {
    fn get_info(&self) -> ServerConfig {
        let mut info = Implementation::default();
        info.name = SERVER.into();
        info.version = env!("CARGO_PKG_VERSION").into();
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(info)
            .with_instructions(
                "Prisma AIRS runtime history service. Tool inputs use the versioned envelope \
                 {schema_version, tenant, params}; see service_info.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // Only tools this principal can call now are listed; listing is not authorization.
        let Some((p, _)) = principal(&ctx) else {
            return Ok(ListToolsResult::with_all_items(vec![]));
        };
        let tools = catalogue::all()
            .iter()
            .filter(|t| callable(&p, t))
            .map(|t| {
                Tool::new(
                    t.name.clone(),
                    description(&t.name),
                    Arc::new(t.input_schema.clone()),
                )
                .with_annotations(
                    ToolAnnotations::new()
                        .read_only(!t.mutating)
                        .destructive(false),
                )
            })
            .collect();
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let Some((p, live)) = principal(&ctx) else {
            return Ok(error_result((
                ErrorCode::Unauthenticated,
                "no verified principal for this request",
                None,
            ))
            .into());
        };
        let args = request.arguments.clone().unwrap_or_default();
        let tool = catalogue::get(&request.name).map_or("<unknown>", |t| t.name.as_str());
        let scope = call_digest(tool, &live.config.service.tenant, &args);
        let outcome = self.dispatch(&request.name, args, &p, &live, &ctx).await;
        let job_id = outcome
            .as_ref()
            .ok()
            .and_then(|r| r.structured_content.as_ref())
            .and_then(|v| v["job"]["job_id"].as_str())
            .filter(|id| dto::is_job_id(id))
            .map(str::to_owned);
        let code = match &outcome {
            Ok(_) => "ok".to_owned(),
            Err((code, _, _)) => serde_json::to_value(code)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| "internal".into()),
        };
        audit(&p, &live, tool, &scope, job_id.as_deref(), &code);
        Ok(outcome.unwrap_or_else(error_result).into())
    }
}

/// The private staging path of an upload; the ID must be one this service minted.
fn staging_path(root: &std::path::Path, upload_id: &str) -> Result<std::path::PathBuf, Failure> {
    let minted = upload_id
        .strip_prefix("u1_")
        .and_then(|u| uuid::Uuid::parse_str(u).ok())
        .is_some_and(|u| upload_id == format!("u1_{u}"));
    if !minted {
        return Err(ErrorCode::InvalidRequest.into());
    }
    Ok(crate::lifecycle::upload_path(root, upload_id))
}

/// Open an upload's staging file (never following a link; only a regular file).
fn open_owned_staging(path: &std::path::Path, write: bool) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(write)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)?;
    if !f.metadata()?.file_type().is_file() {
        return Err(std::io::Error::other("staging is not a regular file"));
    }
    Ok(f)
}

/// Remove an upload's staging only when it is the service's own regular file (a link or
/// anything else at that path is left alone).
/// Remove an upload's staging file (a regular file only); whether none remains after.
pub(crate) fn remove_owned_staging(root: &std::path::Path, upload_id: &str) -> bool {
    let Ok(path) = staging_path(root, upload_id) else {
        return false;
    };
    match std::fs::symlink_metadata(&path) {
        Ok(m) if m.file_type().is_file() => {
            std::fs::remove_file(&path).is_ok() && std::fs::symlink_metadata(&path).is_err()
        }
        Ok(_) => false,
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
    }
}

/// The synchronous staging sweep ([`Service::sweep_uploads`], and the worker's pass).
pub fn sweep_staging(storage: &Storage, tenant: &str) {
    let now = airs_runtime_core::time::now_ms();
    let ids = storage.control(|c| {
        let mut ids = c.sweep_expired_uploads(tenant, now).unwrap_or_default();
        ids.extend(c.uploads_to_remove(tenant).unwrap_or_default());
        ids.sort();
        ids.dedup();
        ids
    });
    for id in ids {
        if remove_owned_staging(&storage.admitted.path, &id) {
            let _ = storage.control(|c| c.upload_removed(tenant, &id));
        }
    }
}

/// One async lock per upload: its chunk and finalize transitions are serialized.
fn upload_lock(upload_id: &str) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    > = std::sync::OnceLock::new();
    let mut map = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if map.len() > 4096 {
        map.retain(|_, l| Arc::strong_count(l) > 1);
    }
    map.entry(upload_id.to_owned()).or_default().clone()
}

/// The service's complete secret registry (every configured credential file, read
/// bounded), for transfer validation, import and export (M5 review 1 finding 9): when any
/// cannot be loaded, the operation is refused `credential_blocked` (fail closed).
pub(crate) fn service_secrets(
    config: &crate::config::Config,
) -> Result<airs_runtime_core::error::SecretSet, Failure> {
    let secrets = airs_runtime_core::error::SecretSet::default();
    if crate::airs_tenant::tenant(config, &secrets).is_err() {
        return Err(ErrorCode::CredentialBlocked.into());
    }
    Ok(secrets)
}

/// SHA-256 and length of a file, streamed in bounded chunks (never read whole).
pub(crate) fn stream_sha256(path: &std::path::Path) -> std::io::Result<(String, u64)> {
    use std::io::Read;
    let mut f = open_owned_staging(path, false)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut total = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    Ok((hex(&hasher.finalize()), total))
}

/// Validate a completed upload against its whole admission declaration: its actual
/// length and streamed SHA-256, the expected TSG and environment of the service's own
/// cloud, and the content for its declared kind. Returns `(sha256, records)`, or the
/// fixed reason it is refused.
fn validate_upload(
    path: &std::path::Path,
    total: u64,
    declared: &Value,
    cloud: &airs_history_store::identity::Cloud,
    secrets: &airs_runtime_core::error::SecretSet,
) -> Result<(String, u64), &'static str> {
    let (sha, length) = stream_sha256(path).map_err(|_| "the staged upload cannot be read")?;
    if length != total || declared["total_bytes"].as_u64() != Some(total) {
        return Err("the staged length differs from the declaration");
    }
    if declared["sha256"].as_str() != Some(sha.as_str()) {
        return Err("the staged bytes do not have the declared SHA-256");
    }
    if declared["expect_tsg"].as_str() != Some(cloud.tsg.as_str())
        || declared["expect_environment"].as_str() != Some(cloud.environment_id.as_str())
    {
        return Err("the declared TSG or environment is not this service's cloud");
    }
    let records = match declared["kind"].as_str() {
        Some("logical_transfer") => airs_history_store::transfer::validate_file(
            cloud,
            path,
            crate::lifecycle::MAINTENANCE,
            &scm::Session::new(),
            secrets,
        )
        .map_err(|_| "the logical transfer failed validation")?,
        Some("legacy_bundle") => crate::lifecycle::validate_legacy_bundle(path, cloud, secrets)
            .map_err(|_| "the legacy bundle failed validation")?,
        _ => return Err("the declared kind is not supported"),
    };
    Ok((sha, records))
}

/// The archive run of one receipt batch: a UUID from a canonical tuple of the batch
/// key's complete idempotency scope (tenant, caller subject and client, recovery epoch,
/// key). The tuple is JSON-encoded, so no field can run into another.
pub(crate) fn receipt_run_id(
    tenant: &str,
    (sub, client): (&str, &str),
    epoch: &str,
    key: &str,
) -> String {
    let scope = serde_json::to_string(&json!(["receipts", 2, tenant, sub, client, epoch, key]))
        .expect("a tuple of strings serializes");
    uuid_from(&Sha256::digest(scope.as_bytes()))
}

/// The run ID batches received before round 11 used (no epoch): only to recover a batch
/// such a build left pending.
fn legacy_receipt_run_id(tenant: &str, (sub, client): (&str, &str), key: &str) -> String {
    uuid_from(&Sha256::digest(
        format!("receipts\u{1f}{tenant}\u{1f}{sub}\u{1f}{client}\u{1f}{key}").as_bytes(),
    ))
}

fn uuid_from(h: &[u8]) -> String {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&h[..16]);
    uuid::Builder::from_random_bytes(bytes)
        .into_uuid()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer_cases;

    fn principal(client: &str) -> Principal {
        Principal::for_tests("human-a", client)
    }

    /// The real scope construction binds the epoch, tool, tenant, principal and every
    /// non-page parameter: a cursor minted under any other value is refused even with
    /// the right key.
    #[test]
    fn scope_digests_bind_epoch_tool_tenant_principal_and_parameters() {
        let key = [3u8; 32];
        let params = json!({"since": 1, "until": 2, "app_ids": []});
        let scope = scope_digest(
            "history_coverage",
            "svc-test",
            &principal("a"),
            "0a1b2c3d",
            &params,
        );
        let cursor = encode_cursor(
            &key,
            &CursorBody {
                d: scope,
                s: "snap".into(),
                p: 1,
            },
        );
        for other in [
            scope_digest(
                "history_coverage",
                "svc-test",
                &principal("a"),
                "ffffffff",
                &params,
            ),
            scope_digest(
                "jobs_list",
                "svc-test",
                &principal("a"),
                "0a1b2c3d",
                &params,
            ),
            scope_digest(
                "history_coverage",
                "svc-othr",
                &principal("a"),
                "0a1b2c3d",
                &params,
            ),
            scope_digest(
                "history_coverage",
                "svc-test",
                &principal("b"),
                "0a1b2c3d",
                &params,
            ),
            scope_digest(
                "history_coverage",
                "svc-test",
                &principal("a"),
                "0a1b2c3d",
                &json!({"since": 1, "until": 3, "app_ids": []}),
            ),
        ] {
            assert_eq!(
                decode_cursor(&key, &cursor, &other).err(),
                Some(ErrorCode::InvalidRequest)
            );
        }
    }

    /// Contract §6: one coherent ordinary read within 2 s; a slower read is refused and
    /// its session cancelled, never answered late.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_read_past_its_budget_is_refused_and_cancelled() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let observer = seen.clone();
        let started = std::time::Instant::now();
        let result = read(move |session| {
            *observer.lock().unwrap() = Some(session.clone());
            std::thread::sleep(READ_BUDGET + Duration::from_millis(800));
            Ok::<_, ErrorCode>(())
        })
        .await;
        assert_eq!(result, Err(ErrorCode::StorageUnavailable));
        assert!(
            started.elapsed() < READ_BUDGET + Duration::from_millis(600),
            "refused at the budget"
        );
        let session = seen.lock().unwrap().clone().expect("the read started");
        assert!(session.is_cancelled(), "the late read is told to stop");
    }

    /// A disposable service storage with an active epoch and no archive.
    fn contended_service() -> (std::path::PathBuf, Arc<Storage>, Service, Arc<Live>) {
        use std::os::unix::fs::DirBuilderExt;
        let base = std::env::temp_dir().join(format!("airs-mcp-contend-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .unwrap();
        let base = base.canonicalize().unwrap();
        let admitted = crate::storage::admit(&base).unwrap();
        let control_dir = crate::storage::private_child(&admitted, "control").unwrap();
        let live =
            Live::new(crate::run::parse_config(include_bytes!("../config/example.toml")).unwrap())
                .unwrap();
        let tenant = live.config.service.tenant.clone();
        let control =
            Control::open(&control_dir.path, &tenant, "0a1b2c3d", &[1u8; 32], 1_000).unwrap();
        let owner = crate::storage::OwnerRecord {
            owner_id: "o".into(),
            pod: "p".into(),
            node: "n".into(),
            pid: 0,
            acquired_at_unix: 0,
        };
        let storage = Arc::new(Storage::new(admitted, owner, base.join("history"), control));
        let service = Service {
            storage: Some(storage.clone()),
        };
        (base, storage, service, live)
    }

    /// A service with administrative execution on (`fresh_proof`, nonce claim `nonce`).
    fn admin_service() -> (std::path::PathBuf, Arc<Storage>, Service, Arc<Live>) {
        use std::os::unix::fs::PermissionsExt;
        let (base, storage, service, _) = contended_service();
        // Synthetic credential files: transfers apply the service's complete secret
        // registry and fail closed without it (M5 review 1 finding 9).
        let write = |name: &str, value: &str| {
            let path = base.join(name);
            std::fs::write(&path, value).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            path.display().to_string()
        };
        let id = write("mgmt-client-id", "synthetic-client-id");
        let key = write("mgmt-client-secret", "SYNTHETIC-ADMIN-SERVICE-SECRET-01");
        let scanner = write("scanner-monitor", "synthetic-scanner-key");
        let text = include_str!("../config/example.toml")
            .replace("/run/airs-mcp/secrets/mgmt-client-id", &id)
            .replace("/run/airs-mcp/secrets/mgmt-client-secret", &key)
            .replace("/run/airs-mcp/secrets/scanner-monitor", &scanner)
            .replace(
                "[admin]\nmode = \"disabled\"",
                "[admin]\nmode = \"fresh_proof\"\nproof_nonce_claim = \"nonce\"\nproof_max_age_seconds = 300",
            );
        assert!(text.contains("fresh_proof"));
        let live = Live::new(crate::run::parse_config(text.as_bytes()).unwrap()).unwrap();
        (base, storage, service, live)
    }

    /// An Administrator principal whose assertion carries `nonce` and a fresh
    /// `auth_time` (the §3 direct proof).
    fn admin(client: &str, nonce: Option<&str>) -> Principal {
        let mut p = Principal::for_tests("human-admin", client);
        p.grant.permission = Permission::Administrator;
        p.token_tier = Permission::Administrator;
        if let Some(n) = nonce {
            p.assertion_claims.insert("nonce".into(), json!(n));
            p.assertion_claims.insert(
                "auth_time".into(),
                json!(airs_runtime_core::time::now_ms() / 1000),
            );
        }
        p
    }

    /// Seed the service archive with the shared evidence fixture (one event, an
    /// example excerpt), as an explicit legacy import would.
    fn seed_archive(storage: &Storage, live: &Live) {
        let mut evidence: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/signal/evidence.json"))
                .unwrap();
        evidence["events"][0]["detections"][0]["snippets"][0]["excerpts"] =
            json!(["Synthetic excerpt kept for the prune check."]);
        evidence.as_object_mut().unwrap().remove("digest");
        evidence["digest"] = json!(hex(&Sha256::digest(serde_json::to_vec(&evidence).unwrap())));
        let path = storage.admitted.path.join("seed.json");
        std::fs::write(&path, serde_json::to_vec(&evidence).unwrap()).unwrap();
        let original = airs_history_store::repository::read_legacy(&path).unwrap();
        airs_history_store::repository::import_evidence(
            &storage.history_root,
            &cloud(&live.config).unwrap(),
            &original,
            &airs_runtime_core::error::SecretSet::default(),
            &Default::default(),
            &scm::Session::new(),
        )
        .unwrap();
        std::fs::remove_file(path).unwrap();
    }

    /// Run queued lifecycle jobs with the lifecycle executor.
    fn run_lifecycle(storage: &Arc<Storage>, live: &Arc<Live>) {
        let mut executors: std::collections::BTreeMap<String, Arc<dyn crate::worker::Executor>> =
            Default::default();
        executors.insert(
            "lifecycle_execute".into(),
            Arc::new(crate::lifecycle::Lifecycle {
                shared: crate::run::Shared::new(live.clone()),
            }),
        );
        let worker = crate::worker::Worker {
            storage: storage.clone(),
            owner: "w".into(),
            executors,
        };
        while worker
            .step(&live.config.service.tenant, |_| true)
            .unwrap()
            .is_some()
        {}
    }

    fn archive_holds(storage: &Storage, live: &Live, needle: &str) -> bool {
        let cloud = cloud(&live.config).unwrap().cloud_id;
        std::fs::read(storage.history_root.join(cloud).join("archive.sqlite3"))
            .unwrap()
            .windows(needle.len())
            .any(|w| w == needle.as_bytes())
    }

    /// M5 review 3 minor (mutation H1): the service reports the tombstones prune actually
    /// recorded, not the example-row count. The example rows are removed while their
    /// excerpts stay in the fact copies, so the fact copies yield tombstones with no
    /// example row: the two counts differ, and the reported count is the archive's
    /// tombstone rows.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_prune_reports_its_recorded_tombstones_not_its_example_rows() {
        let (_base, storage, service, live) = admin_service();
        let tenant = live.config.service.tenant.clone();
        seed_archive(&storage, &live);
        let db_path = storage
            .history_root
            .join(cloud(&live.config).unwrap().cloud_id)
            .join("archive.sqlite3");
        {
            let db = rusqlite::Connection::open(&db_path).unwrap();
            // Every example row goes; the excerpts stay inside their fact copies, as for
            // a receipt-only excerpt.
            let removed = db.execute("DELETE FROM examples", []).unwrap();
            assert!(removed > 0, "the fixture has example rows");
        }
        let anon = admin("airs-admin-cli", None);
        let preview: LifecyclePreviewParams = serde_json::from_value(
            json!({"action": "prune", "before": "2026-11-02T00:00:00Z", "examples_only": true}),
        )
        .unwrap();
        let pv = service
            .lifecycle_preview(&anon, &live, &tenant, preview)
            .await
            .unwrap();
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        let exec: LifecycleExecuteParams = serde_json::from_value(json!({
            "preview_id": pv["preview_id"], "parameter_digest": pv["parameter_digest"]
        }))
        .unwrap();
        let job = service
            .lifecycle_execute(
                &proved,
                &live,
                &tenant,
                "key-prune-h1-0000001",
                "0a1b2c3d",
                exec,
            )
            .await
            .unwrap();
        run_lifecycle(&storage, &live);
        let id = job["job"]["job_id"].as_str().unwrap().to_owned();
        let done = storage.control(|c| c.job(&tenant, &id).unwrap().unwrap());
        assert_eq!(done.state, State::Succeeded, "{:?}", done.result);
        let result = done.result.unwrap();
        let recorded: i64 = rusqlite::Connection::open(&db_path)
            .unwrap()
            .query_row("SELECT count(*) FROM pruned_examples", [], |r| r.get(0))
            .unwrap();
        assert_eq!(result["tombstones"].as_i64().unwrap(), recorded, "{result}");
        assert_ne!(
            result["tombstones"], result["affected"]["examples"],
            "the fixture separates tombstones from example rows: {result}"
        );
    }

    /// M5 through the service tools: an examples-only prune is previewed (read-only),
    /// refused without the direct proof, executed with it as a durable job that erases
    /// every live excerpt copy; an export is produced and read back in hash-bound
    /// chunks; that export is uploaded to another service in owned, hash-checked chunks,
    /// validated at finalize, and imported through its own separately authorized
    /// `import_finalize` preview.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn lifecycle_prune_export_upload_and_import_through_the_tools() {
        use base64::Engine;
        let (base, storage, service, live) = admin_service();
        let tenant = live.config.service.tenant.clone();
        seed_archive(&storage, &live);
        let excerpt = "Synthetic excerpt kept for the prune check.";
        assert!(archive_holds(&storage, &live, excerpt));
        let anon = admin("airs-admin-cli", None);
        // Preview: affected counts, no writes.
        let preview: LifecyclePreviewParams = serde_json::from_value(
            json!({"action": "prune", "before": "2026-11-02T00:00:00Z", "examples_only": true}),
        )
        .unwrap();
        let pv = service
            .lifecycle_preview(&anon, &live, &tenant, preview)
            .await
            .unwrap();
        assert!(pv["affected"]["examples"].as_u64().unwrap() > 0, "{pv}");
        assert!(
            archive_holds(&storage, &live, excerpt),
            "a preview writes nothing"
        );
        let exec = |pid: &str, digest: &str| -> LifecycleExecuteParams {
            serde_json::from_value(json!({"preview_id": pid, "parameter_digest": digest})).unwrap()
        };
        let pid = pv["preview_id"].as_str().unwrap();
        let digest = pv["parameter_digest"].as_str().unwrap();
        // Without the proof: refused, nothing queued.
        let refused = service
            .lifecycle_execute(
                &anon,
                &live,
                &tenant,
                "key-prune-000000001",
                "0a1b2c3d",
                exec(pid, digest),
            )
            .await
            .unwrap_err();
        assert_eq!(refused.code, ErrorCode::Forbidden);
        // With it: one durable job, which erases every live copy.
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        let job = service
            .lifecycle_execute(
                &proved,
                &live,
                &tenant,
                "key-prune-000000001",
                "0a1b2c3d",
                exec(pid, digest),
            )
            .await
            .unwrap();
        assert_eq!(job["job"]["operation"], "lifecycle_execute");
        run_lifecycle(&storage, &live);
        let id = job["job"]["job_id"].as_str().unwrap().to_owned();
        let done = storage.control(|c| c.job(&tenant, &id).unwrap().unwrap());
        assert_eq!(done.state, State::Succeeded, "{:?}", done.result);
        assert_eq!(done.result.as_ref().unwrap()["action"], "prune");
        assert!(
            !archive_holds(&storage, &live, excerpt),
            "no live copy remains"
        );
        // Export, read back in hash-bound chunks.
        let ep: LifecyclePreviewParams =
            serde_json::from_value(json!({"action": "export"})).unwrap();
        let pv = service
            .lifecycle_preview(&anon, &live, &tenant, ep)
            .await
            .unwrap();
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        let job = service
            .lifecycle_execute(
                &proved,
                &live,
                &tenant,
                "key-export-00000001",
                "0a1b2c3d",
                exec(
                    pv["preview_id"].as_str().unwrap(),
                    pv["parameter_digest"].as_str().unwrap(),
                ),
            )
            .await
            .unwrap();
        run_lifecycle(&storage, &live);
        let export_job = job["job"]["job_id"].as_str().unwrap().to_owned();
        let mut bytes = Vec::new();
        let mut file_sha;
        loop {
            let read: ExportReadParams = serde_json::from_value(
                json!({"job_id": export_job, "offset": bytes.len(), "length": 4096}),
            )
            .unwrap();
            let chunk = service.export_read(&tenant, read).await.unwrap();
            let data = base64::engine::general_purpose::STANDARD
                .decode(chunk["data_base64"].as_str().unwrap())
                .unwrap();
            assert_eq!(chunk["chunk_sha256"], hex(&Sha256::digest(&data)));
            file_sha = chunk["file_sha256"].as_str().unwrap().to_owned();
            bytes.extend_from_slice(&data);
            if bytes.len() as u64 >= chunk["total_bytes"].as_u64().unwrap() || data.is_empty() {
                break;
            }
        }
        assert_eq!(
            hex(&Sha256::digest(&bytes)),
            file_sha,
            "the reassembled export matches its hash"
        );
        // The published export is private: an owner-only regular file under the
        // service's admitted data directory.
        {
            use std::os::unix::fs::PermissionsExt;
            let path = crate::lifecycle::export_path(&storage.admitted.path, &export_job);
            let meta = std::fs::symlink_metadata(&path).unwrap();
            assert!(meta.file_type().is_file(), "a regular file");
            assert_eq!(meta.permissions().mode() & 0o777, 0o600, "owner-only");
        }
        // Upload that export into a second service, and import it.
        let (base2, storage2, service2, _) = admin_service();
        let live2 = live.clone();
        let decl = json!({"action": "upload_admit", "upload": {
            "kind": "logical_transfer", "total_bytes": bytes.len(), "sha256": file_sha,
            "expect_tsg": cloud(&live2.config).unwrap().tsg,
            "expect_environment": cloud(&live2.config).unwrap().environment_id}});
        let up: LifecyclePreviewParams = serde_json::from_value(decl).unwrap();
        let pv = service2
            .lifecycle_preview(&anon, &live2, &tenant, up)
            .await
            .unwrap();
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        let begin: ConfirmParams = serde_json::from_value(
            json!({"preview_id": pv["preview_id"], "parameter_digest": pv["parameter_digest"]}),
        )
        .unwrap();
        let begun = service2
            .upload_begin(
                &proved,
                &live2,
                &tenant,
                "key-upload-0000001",
                "0a1b2c3d",
                begin,
            )
            .await
            .unwrap();
        let upload = begun["upload_id"].as_str().unwrap().to_owned();
        for (n, chunk) in bytes.chunks(4096).enumerate() {
            let c: UploadChunkParams = serde_json::from_value(json!({
                "upload_id": upload, "offset": n * 4096,
                "data_base64": base64::engine::general_purpose::STANDARD.encode(chunk),
                "chunk_sha256": hex(&Sha256::digest(chunk))}))
            .unwrap();
            service2.upload_chunk(&anon, &tenant, c).await.unwrap();
        }
        let fin: UploadIdParams = serde_json::from_value(json!({"upload_id": upload})).unwrap();
        let finalized = service2
            .upload_finalize(&anon, &live2, &tenant, fin)
            .await
            .unwrap();
        assert_eq!(finalized["validated"], true, "{finalized}");
        let imp: LifecyclePreviewParams =
            serde_json::from_value(json!({"action": "import_finalize", "upload_id": upload}))
                .unwrap();
        let pv = service2
            .lifecycle_preview(&anon, &live2, &tenant, imp)
            .await
            .unwrap();
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        service2
            .lifecycle_execute(
                &proved,
                &live2,
                &tenant,
                "key-import-00000001",
                "0a1b2c3d",
                exec(
                    pv["preview_id"].as_str().unwrap(),
                    pv["parameter_digest"].as_str().unwrap(),
                ),
            )
            .await
            .unwrap();
        run_lifecycle(&storage2, &live2);
        let db = rusqlite::Connection::open_with_flags(
            storage2
                .history_root
                .join(cloud(&live2.config).unwrap().cloud_id)
                .join("archive.sqlite3"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let events: i64 = db
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(events, 1, "the imported archive holds the event");
        std::fs::remove_dir_all(&base).unwrap();
        std::fs::remove_dir_all(&base2).unwrap();
    }

    // ---- M5 review 1: the upload, finalize, lifecycle and transfer service adapters ----

    /// `lifecycle_preview` + `transfer_upload_begin` for `bytes` of `kind`, declared for
    /// the service's own cloud unless `decl` overrides fields. Returns the begin result.
    async fn admit_upload(
        service: &Service,
        live: &Arc<Live>,
        bytes: &[u8],
        kind: &str,
        key: &str,
        overrides: Value,
    ) -> Result<Value, Failure> {
        let tenant = live.config.service.tenant.clone();
        let c = cloud(&live.config).unwrap();
        let mut upload = json!({
            "kind": kind, "total_bytes": bytes.len(), "sha256": hex(&Sha256::digest(bytes)),
            "expect_tsg": c.tsg, "expect_environment": c.environment_id});
        for (k, v) in overrides.as_object().unwrap() {
            upload[k] = v.clone();
        }
        let up: LifecyclePreviewParams =
            serde_json::from_value(json!({"action": "upload_admit", "upload": upload})).unwrap();
        let anon = admin("airs-admin-cli", None);
        let pv = service.lifecycle_preview(&anon, live, &tenant, up).await?;
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        let begin: ConfirmParams = serde_json::from_value(
            json!({"preview_id": pv["preview_id"], "parameter_digest": pv["parameter_digest"]}),
        )
        .unwrap();
        service
            .upload_begin(&proved, live, &tenant, key, "0a1b2c3d", begin)
            .await
    }

    fn chunk_params(upload: &str, offset: usize, bytes: &[u8]) -> UploadChunkParams {
        use base64::Engine;
        serde_json::from_value(json!({
            "upload_id": upload, "offset": offset,
            "data_base64": base64::engine::general_purpose::STANDARD.encode(bytes),
            "chunk_sha256": hex(&Sha256::digest(bytes))}))
        .unwrap()
    }

    async fn send_all(service: &Service, live: &Arc<Live>, upload: &str, bytes: &[u8]) {
        let tenant = live.config.service.tenant.clone();
        let anon = admin("airs-admin-cli", None);
        for (n, chunk) in bytes.chunks(4096).enumerate() {
            service
                .upload_chunk(&anon, &tenant, chunk_params(upload, n * 4096, chunk))
                .await
                .unwrap();
        }
    }

    async fn finalize(service: &Service, live: &Arc<Live>, upload: &str) -> Result<Value, Failure> {
        let tenant = live.config.service.tenant.clone();
        let fin: UploadIdParams = serde_json::from_value(json!({"upload_id": upload})).unwrap();
        service
            .upload_finalize(&admin("airs-admin-cli", None), live, &tenant, fin)
            .await
    }

    /// M5 review 5 minor (recovery regression through MCP): a canceled import (its
    /// unfinished journal row) is retired by a fresh upload and job that import the same
    /// transfer, making coverage usable, while an unfinished import of a different
    /// transfer stays and keeps coverage refused (negative control) until it is resolved.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_fresh_import_job_retires_a_canceled_one_through_the_tools() {
        let (base, storage, service, live) = admin_service();
        let export = seeded_export(&service, &storage, &live).await;
        let (base2, storage2, service2, live2) = admin_service();
        let tenant = live2.config.service.tenant.clone();
        // The canceled job had committed its batches before it stopped: the destination
        // holds the transfer's rows (a first import), and its journal row is unfinished.
        let begun = admit_upload(
            &service2,
            &live2,
            &export,
            "logical_transfer",
            "key-replace-up-0000",
            json!({}),
        )
        .await
        .unwrap();
        let first = begun["upload_id"].as_str().unwrap().to_owned();
        send_all(&service2, &live2, &first, &export).await;
        assert_eq!(
            finalize(&service2, &live2, &first).await.unwrap()["validated"],
            true
        );
        assert_eq!(
            import_job(&service2, &storage2, &live2, &first, "key-replace-imp-000")
                .await
                .unwrap()
                .state,
            State::Succeeded
        );
        let archive = storage2
            .history_root
            .join(cloud(&live2.config).unwrap().cloud_id)
            .join("archive.sqlite3");
        {
            let db = rusqlite::Connection::open(&archive).unwrap();
            // The journal identifies a transfer by its staged content hash, as the
            // first import recorded it.
            let same: String = db
                .query_row(
                    "SELECT transfer_sha256 FROM import_operations LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            db.execute(
                "INSERT INTO import_operations VALUES ('job-canceled',?1,1,3,3,0,0,1)",
                [&same],
            )
            .unwrap();
            db.execute(
                "INSERT INTO import_operations VALUES ('job-other',?1,1,3,3,0,0,1)",
                ["c".repeat(64)],
            )
            .unwrap();
        }
        let p = principal("claude-code");
        let coverage = || -> CoverageParams {
            serde_json::from_value(json!({"range": {"since": "2026-01-01T00:00:00Z", "until": "2026-01-02T00:00:00Z"}}))
                .unwrap()
        };
        assert_eq!(
            service2.history_status(&live2).await.unwrap()["unfinished_imports"],
            2
        );
        assert_eq!(
            service2
                .history_coverage(&p, &live2, &tenant, coverage())
                .await
                .unwrap_err(),
            ErrorCode::MaintenanceRequired
        );
        // A fresh upload and job import the same transfer in full.
        let begun = admit_upload(
            &service2,
            &live2,
            &export,
            "logical_transfer",
            "key-replace-up-0001",
            json!({}),
        )
        .await
        .unwrap();
        let upload = begun["upload_id"].as_str().unwrap().to_owned();
        send_all(&service2, &live2, &upload, &export).await;
        assert_eq!(
            finalize(&service2, &live2, &upload).await.unwrap()["validated"],
            true
        );
        let job = import_job(&service2, &storage2, &live2, &upload, "key-replace-imp-001")
            .await
            .unwrap();
        assert_eq!(job.state, State::Succeeded, "{:?}", job.result);
        // The canceled import is retired; the other transfer's is not.
        assert_eq!(
            service2.history_status(&live2).await.unwrap()["unfinished_imports"],
            1
        );
        assert_eq!(
            service2
                .history_coverage(&p, &live2, &tenant, coverage())
                .await
                .unwrap_err(),
            ErrorCode::MaintenanceRequired,
            "another transfer's unfinished import still refuses coverage"
        );
        let kept: i64 = rusqlite::Connection::open(&archive)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM import_operations WHERE operation_id='job-canceled'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kept, 1, "the retired row is kept for audit");
        rusqlite::Connection::open(&archive)
            .unwrap()
            .execute(
                "DELETE FROM import_operations WHERE operation_id='job-other'",
                [],
            )
            .unwrap();
        assert!(
            service2
                .history_coverage(&p, &live2, &tenant, coverage())
                .await
                .is_ok()
        );
        std::fs::remove_dir_all(&base).unwrap();
        std::fs::remove_dir_all(&base2).unwrap();
    }

    /// Preview, execute and run an import of `upload`; the finished job.
    async fn import_job(
        service: &Service,
        storage: &Arc<Storage>,
        live: &Arc<Live>,
        upload: &str,
        key: &str,
    ) -> Result<crate::control::Job, Failure> {
        let tenant = live.config.service.tenant.clone();
        let imp: LifecyclePreviewParams =
            serde_json::from_value(json!({"action": "import_finalize", "upload_id": upload}))
                .unwrap();
        let pv = service
            .lifecycle_preview(&admin("airs-admin-cli", None), live, &tenant, imp)
            .await?;
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        let exec: LifecycleExecuteParams = serde_json::from_value(json!({
            "preview_id": pv["preview_id"], "parameter_digest": pv["parameter_digest"]}))
        .unwrap();
        let job = service
            .lifecycle_execute(&proved, live, &tenant, key, "0a1b2c3d", exec)
            .await?;
        run_lifecycle(storage, live);
        let id = job["job"]["job_id"].as_str().unwrap().to_owned();
        Ok(storage.control(|c| c.job(&tenant, &id).unwrap().unwrap()))
    }

    /// Preview and execute (and run) an action; the finished job.
    async fn lifecycle_job(
        service: &Service,
        storage: &Arc<Storage>,
        live: &Arc<Live>,
        params: Value,
        key: &str,
    ) -> (Value, crate::control::Job) {
        let tenant = live.config.service.tenant.clone();
        let pp: LifecyclePreviewParams = serde_json::from_value(params).unwrap();
        let pv = service
            .lifecycle_preview(&admin("airs-admin-cli", None), live, &tenant, pp)
            .await
            .unwrap();
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        let exec: LifecycleExecuteParams = serde_json::from_value(json!({
            "preview_id": pv["preview_id"], "parameter_digest": pv["parameter_digest"]}))
        .unwrap();
        let job = service
            .lifecycle_execute(&proved, live, &tenant, key, "0a1b2c3d", exec.clone())
            .await
            .unwrap();
        run_lifecycle(storage, live);
        let id = job["job"]["job_id"].as_str().unwrap().to_owned();
        (
            pv,
            storage.control(|c| c.job(&tenant, &id).unwrap().unwrap()),
        )
    }

    fn upload_state(storage: &Storage, upload: &str) -> (String, i64) {
        storage.control(|c| {
            c.conn
                .query_row(
                    "SELECT state, received FROM upload_sessions WHERE upload_id=?1",
                    [upload],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap()
        })
    }

    /// An export of the seeded archive (its bytes).
    async fn seeded_export(service: &Service, storage: &Arc<Storage>, live: &Arc<Live>) -> Vec<u8> {
        seed_archive(storage, live);
        let (_, job) = lifecycle_job(
            service,
            storage,
            live,
            json!({"action": "export"}),
            "key-export-seeded1",
        )
        .await;
        assert_eq!(job.state, State::Succeeded, "{:?}", job.result);
        std::fs::read(crate::lifecycle::export_path(
            &storage.admitted.path,
            &job.job_id,
        ))
        .unwrap()
    }

    /// Finding 6 (the reviewer's probe): a chunk whose write fails never advances the
    /// recorded progress, so the same chunk can be sent again; a retry of a recorded chunk
    /// (a lost response) succeeds only with its exact bytes.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_failed_chunk_write_never_advances_progress() {
        let (base, storage, service, live) = admin_service();
        let tenant = live.config.service.tenant.clone();
        let bytes = b"synthetic upload bytes".to_vec();
        let begun = admit_upload(
            &service,
            &live,
            &bytes,
            "logical_transfer",
            "key-upload-chunk01",
            json!({}),
        )
        .await
        .unwrap();
        let upload = begun["upload_id"].as_str().unwrap().to_owned();
        let path = crate::lifecycle::upload_path(&storage.admitted.path, &upload);
        std::fs::remove_file(&path).unwrap();
        let anon = admin("airs-admin-cli", None);
        assert!(
            service
                .upload_chunk(&anon, &tenant, chunk_params(&upload, 0, &bytes[..8]))
                .await
                .is_err()
        );
        assert_eq!(
            upload_state(&storage, &upload).1,
            0,
            "a failed write advances nothing"
        );
        std::fs::write(&path, []).unwrap();
        let ok = service
            .upload_chunk(&anon, &tenant, chunk_params(&upload, 0, &bytes[..8]))
            .await
            .unwrap();
        assert_eq!(ok["received_bytes"], 8, "the same chunk is sent again");
        // A lost response: the identical retry is accepted, a different one refused.
        let again = service
            .upload_chunk(&anon, &tenant, chunk_params(&upload, 0, &bytes[..8]))
            .await
            .unwrap();
        assert_eq!(again["received_bytes"], 8);
        let other = service
            .upload_chunk(&anon, &tenant, chunk_params(&upload, 0, b"XXXXXXXX"))
            .await;
        assert_eq!(other.unwrap_err().code, ErrorCode::IdempotencyConflict);
        // A gap is refused.
        assert!(
            service
                .upload_chunk(&anon, &tenant, chunk_params(&upload, 12, &bytes[12..]))
                .await
                .is_err()
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// Finding 7 (the reviewer's probe): content that fails validation is never
    /// published `finalized` and can never be imported; its staging is removed. The
    /// declaration's TSG, environment and kind are enforced.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_invalid_or_misdeclared_upload_is_never_importable() {
        let (base, storage, service, live) = admin_service();
        let bytes = b"synthetic invalid logical transfer".to_vec();
        let begun = admit_upload(
            &service,
            &live,
            &bytes,
            "logical_transfer",
            "key-upload-invalid1",
            json!({}),
        )
        .await
        .unwrap();
        let upload = begun["upload_id"].as_str().unwrap().to_owned();
        send_all(&service, &live, &upload, &bytes).await;
        let r = finalize(&service, &live, &upload).await.unwrap();
        assert_eq!(r["validated"], false, "{r}");
        assert_eq!(upload_state(&storage, &upload).0, "aborted");
        assert!(!crate::lifecycle::upload_path(&storage.admitted.path, &upload).exists());
        assert!(
            import_job(&service, &storage, &live, &upload, "key-import-invalid01")
                .await
                .is_err()
        );
        // A valid transfer declared for another TSG (or environment) is refused too.
        let export = seeded_export(&service, &storage, &live).await;
        for (n, wrong) in [
            json!({"expect_tsg": "999"}),
            json!({"expect_environment": format!("env1:{}", "b".repeat(64))}),
            json!({"kind": "legacy_bundle"}),
        ]
        .into_iter()
        .enumerate()
        {
            let begun = admit_upload(
                &service,
                &live,
                &export,
                "logical_transfer",
                &format!("key-upload-wrong-{n:03}"),
                wrong,
            )
            .await
            .unwrap();
            let upload = begun["upload_id"].as_str().unwrap().to_owned();
            send_all(&service, &live, &upload, &export).await;
            let r = finalize(&service, &live, &upload).await.unwrap();
            assert_eq!(r["validated"], false, "{r}");
        }
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// Finding 8: the aggregate staging quota is reserved at begin; an expired upload is
    /// swept (its staging removed) and can still be aborted by its owner. Minor finding:
    /// a replayed begin reports the persisted expiry.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn staging_is_quota_reserved_swept_and_abortable_after_expiry() {
        let (base, storage, service, live) = admin_service();
        let tenant = live.config.service.tenant.clone();
        let mut config = (*live.config).clone();
        config.limits.staging_quota_bytes = 100;
        let small = Live::new(config).unwrap();
        let first = admit_upload(
            &service,
            &small,
            &[7u8; 60],
            "logical_transfer",
            "key-upload-quota01",
            json!({}),
        )
        .await
        .unwrap();
        let refused = admit_upload(
            &service,
            &small,
            &[8u8; 60],
            "logical_transfer",
            "key-upload-quota02",
            json!({}),
        )
        .await
        .unwrap_err();
        assert_eq!(refused.code, ErrorCode::Overloaded, "the quota is reserved");
        // The replayed begin (same key) reports the persisted expiry.
        let upload = first["upload_id"].as_str().unwrap().to_owned();
        let persisted = storage.control(|c| c.upload_expires(&tenant, &upload).unwrap());
        assert_eq!(first["expires_at"], json!(utc(persisted)));
        // Expire it: abort still works for its owner and removes the staging.
        let path = crate::lifecycle::upload_path(&storage.admitted.path, &upload);
        std::fs::write(&path, b"x").unwrap();
        storage.control(|c| {
            c.conn
                .execute("UPDATE upload_sessions SET expires_ms=0", [])
                .unwrap()
        });
        let abort: UploadIdParams = serde_json::from_value(json!({"upload_id": upload})).unwrap();
        service
            .upload_abort(&admin("airs-admin-cli", None), &tenant, abort)
            .await
            .unwrap();
        assert!(!path.exists());
        // An abandoned expired upload is swept by the next begin; its staging removed.
        let second = admit_upload(
            &service,
            &small,
            &[9u8; 60],
            "logical_transfer",
            "key-upload-quota03",
            json!({}),
        )
        .await
        .unwrap();
        let abandoned = second["upload_id"].as_str().unwrap().to_owned();
        let path = crate::lifecycle::upload_path(&storage.admitted.path, &abandoned);
        storage.control(|c| {
            c.conn
                .execute(
                    "UPDATE upload_sessions SET expires_ms=0 WHERE upload_id=?1",
                    [&abandoned],
                )
                .unwrap()
        });
        admit_upload(
            &service,
            &small,
            &[1u8; 60],
            "logical_transfer",
            "key-upload-quota04",
            json!({}),
        )
        .await
        .unwrap();
        // Aborted, then its staging verifiably removed (only then is its quota released).
        assert_eq!(upload_state(&storage, &abandoned).0, "removed");
        assert!(!path.exists(), "the sweep removed the abandoned staging");
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M5 review 2 finding 5: an import whose job is canceled while it runs stops at its
    /// first batch checkpoint (before any further batch or the final rebuild), and the job
    /// ends canceled with its committed batches reported.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_import_canceled_at_a_checkpoint_stops_and_reports_its_batches() {
        struct CancelThenRun(Arc<Storage>, crate::lifecycle::Lifecycle);
        impl crate::worker::Executor for CancelThenRun {
            fn run(&self, ctx: &crate::worker::JobContext<'_>) -> crate::worker::Outcome {
                self.0
                    .control(|c| {
                        c.cancel(
                            &ctx.job.tenant,
                            &ctx.job.job_id,
                            airs_runtime_core::time::now_ms(),
                        )
                    })
                    .unwrap();
                self.1.run(ctx)
            }
        }
        let (sbase, sstorage, sservice, slive) = admin_service();
        let bytes = seeded_export(&sservice, &sstorage, &slive).await;
        let (base, storage, service, live) = admin_service();
        let tenant = live.config.service.tenant.clone();
        let begun = admit_upload(
            &service,
            &live,
            &bytes,
            "logical_transfer",
            "key-cancel-upload1",
            json!({}),
        )
        .await
        .unwrap();
        let upload = begun["upload_id"].as_str().unwrap().to_owned();
        send_all(&service, &live, &upload, &bytes).await;
        assert_eq!(
            finalize(&service, &live, &upload).await.unwrap()["validated"],
            true
        );
        let imp: LifecyclePreviewParams =
            serde_json::from_value(json!({"action": "import_finalize", "upload_id": upload}))
                .unwrap();
        let pv = service
            .lifecycle_preview(&admin("airs-admin-cli", None), &live, &tenant, imp)
            .await
            .unwrap();
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        let exec: LifecycleExecuteParams = serde_json::from_value(json!({
            "preview_id": pv["preview_id"], "parameter_digest": pv["parameter_digest"]}))
        .unwrap();
        let job = service
            .lifecycle_execute(
                &proved,
                &live,
                &tenant,
                "key-cancel-import1",
                "0a1b2c3d",
                exec,
            )
            .await
            .unwrap();
        let id = job["job"]["job_id"].as_str().unwrap().to_owned();
        let worker = crate::worker::Worker {
            storage: storage.clone(),
            owner: "w".into(),
            executors: [(
                "lifecycle_execute".to_owned(),
                Arc::new(CancelThenRun(
                    storage.clone(),
                    crate::lifecycle::Lifecycle {
                        shared: crate::run::Shared::new(live.clone()),
                    },
                )) as Arc<dyn crate::worker::Executor>,
            )]
            .into_iter()
            .collect(),
        };
        worker.step(&tenant, |_| true).unwrap();
        let job = storage.control(|c| c.job(&tenant, &id).unwrap().unwrap());
        assert_eq!(job.state, State::Canceled);
        let result = job.result.unwrap();
        assert_eq!(result["outcome"], "partially_committed", "{result}");
        assert_eq!(result["committed_batches"], 1, "{result}");
        // It stopped before the final rebuild: the archive journal says so.
        let rebuilt: i64 = rusqlite::Connection::open_with_flags(
            storage
                .history_root
                .join(cloud(&live.config).unwrap().cloud_id)
                .join("archive.sqlite3"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap()
        .query_row(
            "SELECT rebuilt FROM import_operations WHERE operation_id=?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
        assert_eq!(rebuilt, 0);
        std::fs::remove_dir_all(base).unwrap();
        std::fs::remove_dir_all(sbase).unwrap();
    }

    /// M5 review 2 finding 1: a prune preview made before a compaction can never become
    /// valid again: compaction restarts the file change counter, and even after enough
    /// writes to bring the counter back to the preview's value, the new incarnation keeps
    /// the generation different, so executing the stale preview is refused.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_preview_from_before_a_compaction_never_validates_again() {
        let (base, storage, service, live) = admin_service();
        let tenant = live.config.service.tenant.clone();
        seed_archive(&storage, &live);
        let cloud = cloud(&live.config).unwrap();
        let generation = || {
            airs_history_store::inspect::data_generation(
                &storage.history_root,
                &cloud,
                &scm::Session::new(),
            )
            .unwrap()
        };
        // Enough writes that the counter is well above what a compaction restarts at.
        let db = storage
            .history_root
            .join(&cloud.cloud_id)
            .join("archive.sqlite3");
        let write = |n: i64| {
            rusqlite::Connection::open(&db)
                .unwrap()
                .execute(
                    "UPDATE archive_meta SET bootstrap_ms=?1 WHERE singleton=1",
                    [n],
                )
                .unwrap();
        };
        for n in 0..6 {
            write(n);
        }
        let pp: LifecyclePreviewParams = serde_json::from_value(
            json!({"action": "prune", "before": "2026-11-02T00:00:00Z", "examples_only": true}),
        )
        .unwrap();
        let pv = service
            .lifecycle_preview(&admin("airs-admin-cli", None), &live, &tenant, pp)
            .await
            .unwrap();
        let stale = pv["data_generation"].as_i64().unwrap();
        assert_eq!(stale, generation());
        let (_, job) = lifecycle_job(
            &service,
            &storage,
            &live,
            json!({"action": "compact"}),
            "key-compact-gen0001",
        )
        .await;
        assert_eq!(job.state, State::Succeeded, "{:?}", job.result);
        // Bring the counter back to the stale preview's value.
        let mut n = 100;
        while generation() & 0xffff_ffff < stale & 0xffff_ffff {
            write(n);
            n += 1;
        }
        assert_eq!(
            generation() & 0xffff_ffff,
            stale & 0xffff_ffff,
            "same counter"
        );
        assert_ne!(generation(), stale, "a different incarnation");
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        let exec: LifecycleExecuteParams = serde_json::from_value(json!({
            "preview_id": pv["preview_id"], "parameter_digest": pv["parameter_digest"]}))
        .unwrap();
        let refused = service
            .lifecycle_execute(
                &proved,
                &live,
                &tenant,
                "key-prune-stale0002",
                "0a1b2c3d",
                exec,
            )
            .await;
        assert!(
            refused.is_err(),
            "the stale preview is refused at admission"
        );
        std::fs::remove_dir_all(base).unwrap();
    }

    /// M5 review 2 finding 9: an archive left with a real hot rollback journal (a write
    /// transaction that spilled pages, interrupted before commit) cannot be read
    /// ordinarily, yet `recover` previews (bound to the archive and journal identity),
    /// executes through the service and rolls the journal back: the archive is readable
    /// again with exactly its pre-crash contents.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_hot_journal_is_recovered_through_the_service() {
        let (base, storage, service, live) = admin_service();
        let tenant = live.config.service.tenant.clone();
        seed_archive(&storage, &live);
        let cloud = cloud(&live.config).unwrap();
        let dir = storage.history_root.join(&cloud.cloud_id);
        let db = dir.join("archive.sqlite3");
        let journal = dir.join("archive.sqlite3-journal");
        let runs = |path: &std::path::Path| -> i64 {
            rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap()
                .query_row("SELECT count(*) FROM collection_runs", [], |r| r.get(0))
                .unwrap()
        };
        let before = runs(&db);
        // A large uncommitted transaction with a tiny cache spills pages to the file and
        // writes a hot journal; the crash state (both files) is captured mid-transaction.
        let (crashed_db, crashed_journal) = {
            let c = rusqlite::Connection::open(&db).unwrap();
            c.execute_batch("PRAGMA cache_size=1; PRAGMA journal_mode=DELETE; BEGIN IMMEDIATE;")
                .unwrap();
            for n in 0..4000 {
                c.execute(
                    "INSERT INTO collection_runs VALUES (?1,'import',1,2,NULL,NULL,'complete',?2)",
                    rusqlite::params![
                        format!("hot-{n}"),
                        json!({"pad": "x".repeat(200)}).to_string()
                    ],
                )
                .unwrap();
            }
            let state = (
                std::fs::read(&db).unwrap(),
                std::fs::read(&journal).unwrap(),
            );
            c.execute_batch("ROLLBACK").unwrap();
            state
        };
        assert!(
            crashed_journal.len() > 512 && crashed_journal[..8] != [0; 8],
            "a hot journal"
        );
        std::fs::write(&db, &crashed_db).unwrap();
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&journal)
                .unwrap()
                .write_all(&crashed_journal)
                .unwrap();
        }
        // The ordinary reader (and so an ordinary preview) refuses the journal sidecar.
        let pp: LifecyclePreviewParams = serde_json::from_value(
            json!({"action": "prune", "before": "2026-11-02T00:00:00Z", "examples_only": true}),
        )
        .unwrap();
        assert!(
            service
                .lifecycle_preview(&admin("airs-admin-cli", None), &live, &tenant, pp)
                .await
                .is_err()
        );
        // Recovery is reachable: preview, execute, and the job rolls the journal back.
        let (_, job) = lifecycle_job(
            &service,
            &storage,
            &live,
            json!({"action": "recover"}),
            "key-recover-hot0001",
        )
        .await;
        assert_eq!(job.state, State::Succeeded, "{:?}", job.result);
        assert!(!journal.exists(), "the journal was rolled back and removed");
        assert_eq!(runs(&db), before, "exactly the pre-crash contents");
        airs_history_store::inspect::data_generation(
            &storage.history_root,
            &cloud,
            &scm::Session::new(),
        )
        .unwrap();
        std::fs::remove_dir_all(base).unwrap();
    }

    /// Finding 4: a keyed retry after the prune it admitted changed the archive returns
    /// the original job; a preview whose archive changed before its job ran is refused
    /// at execution, inside the prune's transaction, with nothing erased.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn previews_bind_the_generation_at_execution_and_retries_keep_their_binding() {
        let (base, storage, service, live) = admin_service();
        let tenant = live.config.service.tenant.clone();
        seed_archive(&storage, &live);
        let prune =
            json!({"action": "prune", "before": "2026-11-02T00:00:00Z", "examples_only": true});
        let (pv, job) = lifecycle_job(
            &service,
            &storage,
            &live,
            prune.clone(),
            "key-prune-retry0001",
        )
        .await;
        assert_eq!(job.state, State::Succeeded, "{:?}", job.result);
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        let exec: LifecycleExecuteParams = serde_json::from_value(json!({
            "preview_id": pv["preview_id"], "parameter_digest": pv["parameter_digest"]}))
        .unwrap();
        let retry = service
            .lifecycle_execute(
                &proved,
                &live,
                &tenant,
                "key-prune-retry0001",
                "0a1b2c3d",
                exec,
            )
            .await
            .unwrap();
        assert_eq!(
            retry["job"]["job_id"],
            job.job_id.as_str(),
            "the retry returns its job"
        );
        // A preview, then a write before its job runs: refused at execution.
        let (base2, storage2, service2, live2) = admin_service();
        seed_archive(&storage2, &live2);
        let pp: LifecyclePreviewParams = serde_json::from_value(prune).unwrap();
        let pv = service2
            .lifecycle_preview(&admin("airs-admin-cli", None), &live2, &tenant, pp)
            .await
            .unwrap();
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        let exec: LifecycleExecuteParams = serde_json::from_value(json!({
            "preview_id": pv["preview_id"], "parameter_digest": pv["parameter_digest"]}))
        .unwrap();
        let job = service2
            .lifecycle_execute(
                &proved,
                &live2,
                &tenant,
                "key-prune-stale0001",
                "0a1b2c3d",
                exec,
            )
            .await
            .unwrap();
        // A write inside the existing archive after admission.
        let cloud2 = cloud(&live2.config).unwrap();
        let path = storage2
            .history_root
            .join(&cloud2.cloud_id)
            .join("archive.sqlite3");
        {
            let db = rusqlite::Connection::open(&path).unwrap();
            db.execute(
                "UPDATE collection_runs SET ended_ms=coalesce(ended_ms,0)+1",
                [],
            )
            .unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        run_lifecycle(&storage2, &live2);
        let id = job["job"]["job_id"].as_str().unwrap().to_owned();
        let done = storage2.control(|c| c.job(&tenant, &id).unwrap().unwrap());
        assert_eq!(done.state, State::Failed, "{:?}", done.result);
        assert_eq!(
            done.result.as_ref().unwrap()["error"]["code"],
            json!(ErrorCode::Conflict)
        );
        assert_eq!(std::fs::read(&path).unwrap(), before, "nothing erased");
        std::fs::remove_dir_all(&base).unwrap();
        std::fs::remove_dir_all(&base2).unwrap();
    }

    /// Finding 9: an export or import without the service's complete secret registry is
    /// refused `credential_blocked`, and nothing is transferred.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn transfers_fail_closed_without_the_secret_registry() {
        let (base, storage, service, live) = admin_service();
        seed_archive(&storage, &live);
        std::fs::remove_file(base.join("mgmt-client-secret")).unwrap();
        let (_, job) = lifecycle_job(
            &service,
            &storage,
            &live,
            json!({"action": "export"}),
            "key-export-blocked1",
        )
        .await;
        assert_eq!(job.state, State::Failed);
        assert_eq!(
            job.result.as_ref().unwrap()["error"]["code"],
            json!(ErrorCode::CredentialBlocked)
        );
        assert!(!crate::lifecycle::export_path(&storage.admitted.path, &job.job_id).exists());
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M5 review 3 finding 5: an archive whose journal records an unfinished import is
    /// never presented as coverage: the read refuses `maintenance_required` until the
    /// import publishes.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn coverage_refuses_while_an_import_is_unfinished() {
        let (base, storage, service, live) = admin_service();
        let tenant = live.config.service.tenant.clone();
        seed_archive(&storage, &live);
        let p = principal("claude-code");
        let coverage = || -> CoverageParams {
            serde_json::from_value(json!({"range": {"since": "2026-01-01T00:00:00Z", "until": "2026-01-02T00:00:00Z"}}))
                .unwrap()
        };
        assert!(
            service
                .history_coverage(&p, &live, &tenant, coverage())
                .await
                .is_ok()
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        rusqlite::Connection::open(storage.history_root.join(&cloud_id).join("archive.sqlite3"))
            .unwrap()
            .execute(
                "INSERT INTO import_operations VALUES ('job-unfinished',?1,1,3,3,0,0,1)",
                ["a".repeat(64)],
            )
            .unwrap();
        assert_eq!(
            service
                .history_coverage(&p, &live, &tenant, coverage())
                .await
                .unwrap_err(),
            ErrorCode::MaintenanceRequired
        );
        // Status reports the unfinished import instead of hiding it (M5 review 4 minor).
        let status = service.history_status(&live).await.unwrap();
        assert_eq!(status["unfinished_imports"], 1, "{status}");
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// Finding 10 / M5 review 3 finding 7: the M5 exit gate's adversarial transfer cases,
    /// the shared `transfer_cases.rs` the store driver also runs, through the service
    /// adapters: admission, owned chunks, finalization and import execution, into empty
    /// and populated destinations. Every forged upload is refused at finalize, is never importable and
    /// changes no archive; the valid export uploads, finalizes, imports and re-imports
    /// idempotently; a legacy evidence bundle imports through its own kind.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn adversarial_transfers_are_refused_through_the_service_adapters() {
        let (base, storage, service, live) = admin_service();
        let export = seeded_export(&service, &storage, &live).await;
        let lines: Vec<Value> = std::str::from_utf8(&export)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let _ = &lines;
        // The shared exit-gate cases, exactly as the store driver runs them (M5 review 3
        // finding 7), plus the physical-SQLite upload only an upload adapter sees.
        let mut cases: Vec<(&str, Vec<u8>)> = transfer_cases::adversarial(&export);
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let physical =
            std::fs::read(storage.history_root.join(&cloud_id).join("archive.sqlite3")).unwrap();
        cases.push(("physical-sqlite", physical));
        // Each into a fresh destination service, empty and populated.
        for (n, (name, bytes)) in cases
            .into_iter()
            .flat_map(|c| [(false, c.clone()), (true, c)])
            .enumerate()
            .map(|(n, (existing, (name, bytes)))| (n, ((existing, name), bytes)))
        {
            let ((existing, name), bytes) = (name, bytes);
            let (base2, storage2, service2, live2) = admin_service();
            let archive = storage2
                .history_root
                .join(&cloud_id)
                .join("archive.sqlite3");
            let held = if existing {
                let _ = seeded_export(&service2, &storage2, &live2).await;
                Some(std::fs::read(&archive).unwrap())
            } else {
                None
            };
            let begun = admit_upload(
                &service2,
                &live2,
                &bytes,
                "logical_transfer",
                &format!("key-adversarial-{n:03}"),
                json!({}),
            )
            .await
            .unwrap();
            let upload = begun["upload_id"].as_str().unwrap().to_owned();
            send_all(&service2, &live2, &upload, &bytes).await;
            let r = finalize(&service2, &live2, &upload).await.unwrap();
            assert_eq!(r["validated"], false, "{name}: {r}");
            assert!(
                import_job(
                    &service2,
                    &storage2,
                    &live2,
                    &upload,
                    &format!("key-adv-import-{n:03}")
                )
                .await
                .is_err(),
                "{name} is never importable"
            );
            match &held {
                None => assert!(
                    !storage2.history_root.join(&cloud_id).exists(),
                    "{name} created an archive"
                ),
                Some(before) => assert_eq!(
                    &std::fs::read(&archive).unwrap(),
                    before,
                    "{name} changed a populated archive"
                ),
            }
            std::fs::remove_dir_all(&base2).unwrap();
        }
        // The valid export: imported, then re-imported idempotently.
        let (base2, storage2, service2, live2) = admin_service();
        for (n, key) in ["key-adversarial-ok1", "key-adversarial-ok2"]
            .into_iter()
            .enumerate()
        {
            let begun = admit_upload(
                &service2,
                &live2,
                &export,
                "logical_transfer",
                key,
                json!({}),
            )
            .await
            .unwrap();
            let upload = begun["upload_id"].as_str().unwrap().to_owned();
            send_all(&service2, &live2, &upload, &export).await;
            assert_eq!(
                finalize(&service2, &live2, &upload).await.unwrap()["validated"],
                true
            );
            let job = import_job(
                &service2,
                &storage2,
                &live2,
                &upload,
                &format!("key-adv-import-ok{n}"),
            )
            .await
            .unwrap();
            assert_eq!(job.state, State::Succeeded, "{:?}", job.result);
            let imported = job.result.as_ref().unwrap()["imported_records"]
                .as_u64()
                .unwrap();
            if n == 0 {
                assert!(imported > 0);
            } else {
                assert_eq!(imported, 0, "a repeated import changes nothing");
            }
        }
        // A legacy evidence bundle through its own kind.
        let (base3, storage3, service3, live3) = admin_service();
        let mut evidence: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/signal/evidence.json"))
                .unwrap();
        evidence.as_object_mut().unwrap().remove("digest");
        evidence["digest"] = json!(hex(&Sha256::digest(serde_json::to_vec(&evidence).unwrap())));
        let bundle = serde_json::to_vec(&evidence).unwrap();
        let begun = admit_upload(
            &service3,
            &live3,
            &bundle,
            "legacy_bundle",
            "key-legacy-bundle01",
            json!({}),
        )
        .await
        .unwrap();
        let upload = begun["upload_id"].as_str().unwrap().to_owned();
        send_all(&service3, &live3, &upload, &bundle).await;
        assert_eq!(
            finalize(&service3, &live3, &upload).await.unwrap()["validated"],
            true
        );
        let job = import_job(&service3, &storage3, &live3, &upload, "key-legacy-import01")
            .await
            .unwrap();
        assert_eq!(job.state, State::Succeeded, "{:?}", job.result);
        let db = rusqlite::Connection::open_with_flags(
            storage3
                .history_root
                .join(&cloud_id)
                .join("archive.sqlite3"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let events: i64 = db
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(events, 1);
        std::fs::remove_dir_all(&base).unwrap();
        std::fs::remove_dir_all(&base2).unwrap();
        std::fs::remove_dir_all(&base3).unwrap();
    }

    /// Hold the control store for `hold` on another thread, as a long worker
    /// transaction would; returns once the lock is held.
    fn hold_control(storage: &Arc<Storage>, hold: Duration) -> std::thread::JoinHandle<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        let storage = storage.clone();
        let holder = std::thread::spawn(move || {
            storage.control(|_| {
                tx.send(()).unwrap();
                std::thread::sleep(hold);
            })
        });
        rx.recv().unwrap();
        holder
    }

    /// Review M2-2 finding 2 and M3-1 finding 7, through the handlers on a
    /// single-threaded runtime: while the control store is held for longer than the
    /// read budget, an archive read (`history_coverage`, which needs the epoch) is not
    /// delayed at all, a control read (`jobs_list`) is refused `storage_unavailable`
    /// at its 2 s budget, queueing included, a submission waits within its 10 s
    /// acceptance budget, and the runtime keeps serving other work the whole time.
    #[tokio::test(flavor = "current_thread")]
    async fn control_contention_never_blocks_the_runtime_or_escapes_the_budget() {
        let (base, storage, service, live) = contended_service();
        let tenant = live.config.service.tenant.clone();
        let p = principal("claude-code");
        let holder = hold_control(&storage, READ_BUDGET * 2 + Duration::from_millis(800));

        // The runtime stays responsive: a ticker makes progress during every call.
        let ticks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let ticker = {
            let ticks = ticks.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    ticks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            })
        };

        let started = std::time::Instant::now();
        let coverage: CoverageParams = serde_json::from_value(
            json!({"range": {"since": "2026-01-01T00:00:00Z", "until": "2026-01-02T00:00:00Z"}}),
        )
        .unwrap();
        let _ = service.history_coverage(&p, &live, &tenant, coverage).await;
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "the epoch is read without the control store"
        );

        let started = std::time::Instant::now();
        let listed = service
            .jobs_list(
                &p,
                &tenant,
                JobsListParams {
                    page: None,
                    states: None,
                },
            )
            .await;
        assert_eq!(listed, Err(ErrorCode::StorageUnavailable.into()));
        let waited = started.elapsed();
        assert!(
            waited >= READ_BUDGET - Duration::from_millis(50),
            "{waited:?}"
        );
        assert!(
            waited < READ_BUDGET + Duration::from_millis(300),
            "{waited:?}"
        );

        let started = std::time::Instant::now();
        let params: CollectionParams = serde_json::from_value(json!({"mode": "recent"})).unwrap();
        let submitted = service
            .collection_start(
                &p,
                &live,
                &tenant,
                "key-contend-0000001",
                "0a1b2c3d",
                params,
            )
            .await;
        // A submission may queue for its whole 10 s acceptance budget: it is accepted
        // once the store is released (about 4.8 s after the hold began), off the runtime.
        assert!(submitted.is_ok(), "{submitted:?}");
        assert!(started.elapsed() < SUBMIT_BUDGET);

        let progressed = ticks.load(std::sync::atomic::Ordering::Relaxed);
        assert!(
            progressed >= 50,
            "the runtime kept serving: {progressed} ticks"
        );
        ticker.abort();
        holder.join().unwrap();

        // Uncontended, the same calls work; the refused list applied nothing.
        let listed = service
            .jobs_list(
                &p,
                &tenant,
                JobsListParams {
                    page: None,
                    states: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(
            listed["items"].as_array().unwrap().len(),
            1,
            "only the accepted submission"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    fn submit_many(storage: &Storage, tenant: &str, p: &Principal, n: usize, tag: &str) {
        for i in 0..n {
            let key = format!("key-{tag}-{i:012}");
            let params = json!({"n": i, "tag": tag});
            storage
                .control(|c| {
                    c.submit(
                        Submission {
                            tenant,
                            operation: "collection_start",
                            actor: Actor::Principal {
                                sub: p.subject.clone(),
                                client_id: p.client_id.clone(),
                            },
                            idempotency_key: &key,
                            expected_epoch: "0a1b2c3d",
                            params: &params,
                            config_revision: "example-1",
                            credential_revision: None,
                            deadline_ms: airs_runtime_core::time::now_ms() + 3_600_000,
                            queue_tenant: 1_000,
                            queue_principal: 1_000,
                            join_equivalent: false,
                            scheduled_slot: None,
                        },
                        airs_runtime_core::time::now_ms(),
                    )
                })
                .unwrap();
        }
    }

    /// M3 review 1 finding 10: a `jobs_list` page always fits its complete result
    /// bound (a 30-job default page shrinks rather than failing), pages walk the whole
    /// set without repeats, and a change to the set between pages is `conflict`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn jobs_pages_fit_their_bound_and_bind_the_result_set() {
        let (base, storage, service, live) = contended_service();
        let tenant = live.config.service.tenant.clone();
        let p = principal("claude-code");
        submit_many(&storage, &tenant, &p, 40, "page");
        let bound = catalogue::get("jobs_list").unwrap().result_limit;
        let list = |cursor: Option<String>| {
            let params: JobsListParams = serde_json::from_value(match cursor {
                None => json!({}),
                Some(c) => json!({"page": {"cursor": c}}),
            })
            .unwrap();
            service.jobs_list(&p, &tenant, params)
        };
        let mut seen = std::collections::BTreeSet::new();
        let mut cursor = None;
        let mut pages = 0;
        loop {
            let page = list(cursor.clone()).await.unwrap();
            assert!(
                result_bytes(&CallToolResult::structured(page.clone())) <= bound,
                "every page fits its complete bound"
            );
            for item in page["items"].as_array().unwrap() {
                assert!(
                    seen.insert(item["job_id"].as_str().unwrap().to_owned()),
                    "no repeats"
                );
            }
            pages += 1;
            match page["next_cursor"].as_str() {
                Some(c) => cursor = Some(c.to_owned()),
                None => break,
            }
        }
        assert_eq!(seen.len(), 40, "the whole set, in {pages} pages");
        assert!(pages > 1, "a default page shrank to fit");
        // A job inserted between pages is a changed set: `conflict`, never a repeat.
        let first = list(None).await.unwrap();
        let next = first["next_cursor"].as_str().unwrap().to_owned();
        submit_many(&storage, &tenant, &p, 1, "late");
        assert_eq!(list(Some(next)).await, Err(ErrorCode::Conflict.into()));
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M3 review 4 finding 1: a request whose blocking task only starts after its budget
    /// expired (a saturated blocking pool) never runs its transaction, and once the pool
    /// frees nothing is executed late.
    #[test]
    fn a_task_started_after_its_budget_never_runs() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let (base, storage, _service, _live) = contended_service();
        let ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
        runtime.block_on(async {
            // Saturate the one blocking thread for longer than the request's budget.
            let busy =
                tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(600)));
            let flag = ran.clone();
            let refused = storage
                .control_bounded(Duration::from_millis(200), move |_| {
                    flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                })
                .await;
            assert_eq!(refused, Err(ErrorCode::StorageUnavailable.into()));
            busy.await.unwrap();
            tokio::time::sleep(Duration::from_millis(300)).await;
        });
        assert!(
            !ran.load(std::sync::atomic::Ordering::SeqCst),
            "nothing ran after the budget"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M3 review 1 minor: the control store's retry hint reaches the client. A full
    /// principal queue is `overloaded` with `retry_after_seconds` in the error result.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn overloaded_refusals_keep_their_retry_hint() {
        let (base, _storage, service, live) = contended_service();
        let tenant = live.config.service.tenant.clone();
        let p = principal("claude-code");
        let mut last = None;
        for day in 1..=9 {
            let params: CollectionParams = serde_json::from_value(json!({
                "mode": "backfill",
                "range": {"since": format!("2026-01-{day:02}T00:00:00Z"), "until": format!("2026-01-{:02}T00:00:00Z", day + 1)}
            }))
            .unwrap();
            let key = format!("key-overload-{day:06}");
            last = Some(
                service
                    .collection_start(&p, &live, &tenant, &key, "0a1b2c3d", params)
                    .await,
            );
        }
        let refused = last.unwrap().unwrap_err();
        assert_eq!(refused.code, ErrorCode::Overloaded);
        let retry = refused.retry_after_seconds.expect("a retry hint");
        let body = error_result((
            refused.code,
            reason(refused.code),
            refused.retry_after_seconds,
        ));
        let body = body.structured_content.unwrap();
        assert_eq!(body["retry_after_seconds"], json!(retry));
        std::fs::remove_dir_all(&base).unwrap();
    }

    fn receipt(key: &str, scanner: &str, app: Option<&str>) -> Value {
        json!({
            "declared_scanner": scanner,
            "identity_withheld": false,
            "receipt_key": key,
            "received_at": "2026-11-06T00:00:00Z",
            "scan_id": "00000000-0000-4000-8000-0000000000b2",
            "status": "complete",
            "action": "allow",
            "category": "benign",
            "application_id": app,
            "application_name": "never stored",
            "session_id": "session-1",
            "report_id": null,
            "sub_request": 0,
        })
    }

    /// M8 review 2 finding 7: every part of the reserve measurement fails closed. A
    /// failed reservation query, or a store directory that cannot be listed, makes the
    /// level `Unknown` (never a zero reservation or zero bytes), so allocating writes
    /// pause.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_failed_reserve_measurement_is_unknown_not_zero() {
        let (base, storage, _service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-ABCDEFGHIJ");
        let tenant = live.config.service.tenant.clone();
        let plenty: crate::space::Probe = std::sync::Arc::new(|_| Some(u64::MAX / 4));
        storage.set_space_probe_for_tests(plenty);
        assert_eq!(storage.space(&tenant).level, crate::space::Level::Normal);
        // The reservation query fails.
        storage.control(|c| {
            c.conn
                .execute_batch("ALTER TABLE upload_sessions RENAME TO upload_sessions_hidden")
                .unwrap()
        });
        let s = storage.space(&tenant);
        assert_eq!(
            s.level,
            crate::space::Level::Unknown,
            "not a zero reservation"
        );
        assert!(!s.admits("reports_create") && !s.admits("receipts_ingest"));
        storage.control(|c| {
            c.conn
                .execute_batch("ALTER TABLE upload_sessions_hidden RENAME TO upload_sessions")
                .unwrap()
        });
        assert_eq!(storage.space(&tenant).level, crate::space::Level::Normal);
        // A store directory that cannot be listed (root would read it anyway).
        if rustix::process::geteuid().as_raw() != 0 {
            use std::os::unix::fs::PermissionsExt;
            std::fs::create_dir_all(&storage.history_root).unwrap();
            let mode = |m| std::fs::Permissions::from_mode(m);
            std::fs::set_permissions(&storage.history_root, mode(0o000)).unwrap();
            let s = storage.space(&tenant);
            std::fs::set_permissions(&storage.history_root, mode(0o700)).unwrap();
            assert_eq!(s.level, crate::space::Level::Unknown, "not zero bytes");
        }
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M8 review 2 finding 8: the reserve sampler is shared and bounded. While a
    /// measurement stalls, concurrent scrapes and tool calls start no second one; each
    /// returns within its bound with `Unknown`, so allocating tools refuse. When the
    /// measurement finishes, its result is used.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_stalled_reserve_measurement_is_shared_and_bounded() {
        let (base, storage, service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-ABCDEFGHIJ");
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let release = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        // A failing assertion still releases the stalled probes, so the runtime can shut
        // down and the test fails instead of hanging.
        struct Release(std::sync::Arc<std::sync::atomic::AtomicBool>);
        impl Drop for Release {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let _release_on_exit = Release(release.clone());
        let (c2, r2) = (calls.clone(), release.clone());
        storage.set_space_probe_for_tests(std::sync::Arc::new(move |_| {
            c2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            while !r2.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Some(u64::MAX / 4)
        }));
        let bound = SPACE_BUDGET + std::time::Duration::from_millis(400);
        let started = std::time::Instant::now();
        let mut waits = Vec::new();
        for _ in 0..4 {
            let s = storage.clone();
            let t = tenant.clone();
            waits.push(tokio::spawn(async move { s.space_sampled(&t).await }));
        }
        for _ in 0..3 {
            let s = storage.clone();
            let t = tenant.clone();
            waits.push(tokio::spawn(async move {
                let m = crate::metrics::sample(Some(&s), &t, true, false).await;
                (m.space.unwrap(), None)
            }));
        }
        for w in waits {
            let (s, age) = w.await.unwrap();
            assert_eq!(s.level, crate::space::Level::Unknown);
            assert!(age.is_none());
        }
        assert!(started.elapsed() < bound * 2, "{:?}", started.elapsed());
        // A tool call that allocates refuses within its bound while measurement stalls.
        let t0 = std::time::Instant::now();
        let refused = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-space-stall-0001",
                "0a1b2c3d",
                serde_json::from_value(json!({"cloud_id": cloud(&live.config).unwrap().cloud_id,
                    "records": [receipt("rk-stall-0001", "monitor", None)]}))
                .unwrap(),
            )
            .await
            .unwrap_err();
        assert_eq!(refused.code, ErrorCode::StorageUnavailable);
        assert!(t0.elapsed() < bound, "{:?}", t0.elapsed());
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "one measurement in flight, however many callers"
        );
        // The measurement finishes; its result is used and reused.
        release.store(true, std::sync::atomic::Ordering::SeqCst);
        let mut level = crate::space::Level::Unknown;
        for _ in 0..50 {
            level = storage.space_sampled(&tenant).await.0.level;
            if level == crate::space::Level::Normal {
                break;
            }
        }
        assert_eq!(level, crate::space::Level::Normal);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M8 review 1 (fork C gap): the disk reserve gates the synchronous writes. Receipt
    /// ingestion (an archive write) pauses only below the reserve; upload staging pauses
    /// with publication, below reserve + 512 MiB, before any validation or write.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_disk_reserve_pauses_receipts_and_upload_staging() {
        let (base, storage, service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-ABCDEFGHIJ");
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let reserve = storage.space(&tenant).reserve;
        let free =
            |bytes: u64| -> crate::space::Probe { std::sync::Arc::new(move |_| Some(bytes)) };
        let batch = |rk: &str| -> ReceiptsParams {
            serde_json::from_value(
                json!({"cloud_id": cloud_id, "records": [receipt(rk, "monitor", None)]}),
            )
            .unwrap()
        };
        let chunk = || -> UploadChunkParams {
            serde_json::from_value(
                json!({"upload_id": format!("u1_0a1b2c3d_{}", uuid::Uuid::new_v4()),
                "offset": 0, "data_base64": "AA==", "chunk_sha256": hex(&Sha256::digest([0u8]))}),
            )
            .unwrap()
        };
        // Below the reserve: receipts write nothing.
        storage.set_space_probe_for_tests(free(reserve - 1));
        assert_eq!(
            storage.space(&tenant).level,
            crate::space::Level::CollectionPaused
        );
        let refused = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-space-receipts-01",
                "0a1b2c3d",
                batch("rk-space-0001"),
            )
            .await
            .unwrap_err();
        assert_eq!(refused.code, ErrorCode::StorageUnavailable);
        assert!(
            !storage
                .history_root
                .join(&cloud_id)
                .join("archive.sqlite3")
                .exists()
                || rusqlite::Connection::open(
                    storage.history_root.join(&cloud_id).join("archive.sqlite3")
                )
                .unwrap()
                .query_row("SELECT count(*) FROM receipts", [], |r| r.get::<_, i64>(0))
                .unwrap_or(0)
                    == 0,
            "nothing was written"
        );
        // Within the publication headroom: receipts go, upload staging is paused.
        storage.set_space_probe_for_tests(free(reserve + crate::space::MIB));
        assert_eq!(
            storage.space(&tenant).level,
            crate::space::Level::PublicationPaused
        );
        let accepted = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-space-receipts-02",
                "0a1b2c3d",
                batch("rk-space-0002"),
            )
            .await
            .unwrap();
        assert_eq!(accepted["accepted"], 1);
        assert_eq!(
            service
                .upload_chunk(&p, &tenant, chunk())
                .await
                .unwrap_err()
                .code,
            ErrorCode::StorageUnavailable,
            "staging is paused before the upload is even looked up"
        );
        // Normal space (the reserve is measured again: the archive grew): the same
        // chunk reaches validation (an unknown upload).
        let reserve = storage.space(&tenant).reserve;
        storage.set_space_probe_for_tests(free(reserve + crate::space::PUBLICATION_HEADROOM));
        assert_eq!(storage.space(&tenant).level, crate::space::Level::Normal);
        assert_ne!(
            service
                .upload_chunk(&p, &tenant, chunk())
                .await
                .unwrap_err()
                .code,
            ErrorCode::StorageUnavailable
        );
        std::fs::remove_dir_all(base).unwrap();
    }

    /// M4.5 `receipts_ingest`: validation before any write, attribution only through an
    /// explicit association to a configured scanner, idempotency by batch key, archive
    /// duplicates counted, and the 200-record bound.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn receipts_are_ingested_idempotently_and_attributed_only_by_association() {
        let (base, storage, service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-ABCDEFGHIJ");
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let params = |cloud: &str, records: Vec<Value>| -> ReceiptsParams {
            serde_json::from_value(json!({"cloud_id": cloud, "records": records})).unwrap()
        };
        let ingest = |key: &'static str, ps: ReceiptsParams| {
            service.receipts_ingest(&p, &live, &tenant, key, "0a1b2c3d", ps)
        };
        // Another cloud is refused before any write.
        let other = "ab".repeat(32);
        assert_eq!(
            ingest(
                "key-receipts-0000001",
                params(&other, vec![receipt("rk-00000001", "monitor", None)])
            )
            .await
            .unwrap_err()
            .code,
            ErrorCode::InvalidRequest
        );
        // One associated, one not; a client application ID naming another app is not
        // trusted.
        let batch = vec![
            receipt("rk-00000001", "monitor", Some("some-other-app")),
            receipt("rk-00000002", "unknown-scanner", None),
        ];
        let first = ingest("key-receipts-0000002", params(&cloud_id, batch.clone()))
            .await
            .unwrap();
        assert_eq!(first["accepted"], 1);
        assert_eq!(first["duplicates"], 0);
        assert_eq!(first["refused"][0]["index"], 1);
        // After the first batch the archive holds one receipt, unattributed (the
        // client named another app), and no client application name.
        let db = rusqlite::Connection::open_with_flags(
            storage.history_root.join(&cloud_id).join("archive.sqlite3"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let (n, app, name): (i64, Option<String>, Option<String>) = db
            .query_row(
                "SELECT count(*), max(app_id), max(app_name) FROM receipts",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(n, 1);
        assert!(
            app.unwrap_or_default().is_empty(),
            "attribution needs the configured application"
        );
        assert!(
            name.unwrap_or_default().is_empty(),
            "a client application name is never stored"
        );
        // The same key and records: the same result, nothing new written.
        let again = ingest("key-receipts-0000002", params(&cloud_id, batch.clone()))
            .await
            .unwrap();
        assert_eq!(again, first);
        // The same key with other records: a conflict.
        assert_eq!(
            ingest(
                "key-receipts-0000002",
                params(&cloud_id, vec![receipt("rk-00000003", "monitor", None)])
            )
            .await
            .unwrap_err()
            .code,
            ErrorCode::IdempotencyConflict
        );
        // A new batch repeating a receipt key: a duplicate, not a second receipt.
        let dup = ingest(
            "key-receipts-0000003",
            params(&cloud_id, vec![receipt("rk-00000001", "monitor", None)]),
        )
        .await
        .unwrap();
        assert_eq!(
            (dup["accepted"].as_u64(), dup["duplicates"].as_u64()),
            (Some(0), Some(1))
        );
        // 201 records: refused whole.
        let many: Vec<Value> = (0..201)
            .map(|n| receipt(&format!("rk-many-{n:05}"), "monitor", None))
            .collect();
        assert_eq!(
            ingest("key-receipts-0000004", params(&cloud_id, many))
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M4.3 schedule tools: set enables the collection schedule with a stable ID and a
    /// rising revision; the same key repeats its result; other parameters under the key
    /// conflict; disable takes effect from its time without erasing the schedule; the
    /// daily kinds are refused until M6.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn schedule_tools_are_idempotent_and_keep_history() {
        let (base, _storage, service, live) = contended_service();
        let tenant = live.config.service.tenant.clone();
        let p = principal("airs-cli");
        let set = |enabled: bool, at: &str| -> ScheduleSetParams {
            serde_json::from_value(json!({"kind": "collection", "enabled": enabled, "effective_at": at, "interval_minutes": 15})).unwrap()
        };
        let first = service
            .schedules_set(
                &p,
                &tenant,
                "key-sched-00000001",
                "0a1b2c3d",
                set(true, "2026-11-06T00:00:00Z"),
            )
            .await
            .unwrap();
        assert_eq!(first["revision"], 1);
        let id = first["schedule_id"].as_str().unwrap().to_owned();
        assert!(id.starts_with("s1_"));
        let again = service
            .schedules_set(
                &p,
                &tenant,
                "key-sched-00000001",
                "0a1b2c3d",
                set(true, "2026-11-06T00:00:00Z"),
            )
            .await
            .unwrap();
        assert_eq!(again, first, "same key, same result");
        assert_eq!(
            service
                .schedules_set(
                    &p,
                    &tenant,
                    "key-sched-00000001",
                    "0a1b2c3d",
                    set(false, "2026-11-06T00:00:00Z")
                )
                .await
                .unwrap_err()
                .code,
            ErrorCode::IdempotencyConflict
        );
        let second = service
            .schedules_set(
                &p,
                &tenant,
                "key-sched-00000002",
                "0a1b2c3d",
                set(true, "2026-11-06T01:00:00Z"),
            )
            .await
            .unwrap();
        assert_eq!(
            (second["schedule_id"].as_str(), second["revision"].as_i64()),
            (Some(id.as_str()), Some(2))
        );
        let disable: ScheduleDisableParams = serde_json::from_value(
            json!({"schedule_id": id, "effective_at": "2099-01-01T00:00:00Z"}),
        )
        .unwrap();
        let off = service
            .schedules_disable(&p, &tenant, "key-sched-00000003", "0a1b2c3d", disable)
            .await
            .unwrap();
        assert_eq!(off["revision"], 3);
        let listed = service.schedules_list(&tenant).await.unwrap();
        let only = &listed["schedules"][0];
        assert_eq!(only["schedule_id"], id.as_str());
        // The disable takes effect tomorrow: today the schedule is still enabled (M4
        // review 1 finding 10).
        assert_eq!(only["enabled"], true);
        assert_eq!(only["interval_minutes"], 15);
        // M6: the daily kinds are served; an unknown kind is refused.
        let daily: ScheduleSetParams = serde_json::from_value(json!({"kind": "daily_report", "enabled": true, "effective_at": "2026-11-06T00:00:00Z"})).unwrap();
        let d = service
            .schedules_set(&p, &tenant, "key-sched-00000004", "0a1b2c3d", daily)
            .await
            .unwrap();
        assert_ne!(
            d["schedule_id"].as_str(),
            Some(id.as_str()),
            "its own schedule"
        );
        let odd: ScheduleSetParams = serde_json::from_value(
            json!({"kind": "weekly", "enabled": true, "effective_at": "2026-11-06T00:00:00Z"}),
        )
        .unwrap();
        assert_eq!(
            service
                .schedules_set(&p, &tenant, "key-sched-00000005", "0a1b2c3d", odd)
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M3 review 2 minor: a job's recorded gaps reach the wire (`jobs_get`, `jobs_list`).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn recorded_gaps_reach_the_job_view() {
        let (base, storage, service, live) = contended_service();
        let tenant = live.config.service.tenant.clone();
        let p = principal("claude-code");
        submit_many(&storage, &tenant, &p, 1, "gap");
        let id: String = storage.control(|c| {
            let id = c
                .list(
                    &tenant,
                    &Actor::Principal {
                        sub: p.subject.clone(),
                        client_id: p.client_id.clone(),
                    },
                    &[],
                    1,
                    0,
                )
                .unwrap()[0]
                .job_id
                .clone();
            // Past its deadline, the next claim fails it with the gap recorded.
            c.claim_next(&tenant, "o", i64::MAX / 2, |_| true).unwrap();
            id
        });
        let params: JobIdParams = serde_json::from_value(json!({"job_id": id})).unwrap();
        let view = service.jobs_get(&p, &tenant, params).await.unwrap();
        assert_eq!(view["job"]["state"], "failed");
        assert_eq!(
            view["job"]["gaps"],
            json!(["deadline passed before the job could run"])
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// The receipts service with its `[airs]` credentials in private synthetic files.
    fn receipts_service_with_secret(
        secret: &str,
    ) -> (std::path::PathBuf, Arc<Storage>, Service, Arc<Live>) {
        use std::os::unix::fs::PermissionsExt;
        let (base, storage, service, _) = contended_service();
        let write = |name: &str, value: &str| {
            let path = base.join(name);
            std::fs::write(&path, value).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            path.display().to_string()
        };
        let id = write("mgmt-client-id", "synthetic-client-id");
        let key = write("mgmt-client-secret", secret);
        let scanner = write("scanner-monitor", "synthetic-scanner-key");
        let text = include_str!("../config/example.toml")
            .replace("/run/airs-mcp/secrets/mgmt-client-id", &id)
            .replace("/run/airs-mcp/secrets/mgmt-client-secret", &key)
            .replace("/run/airs-mcp/secrets/scanner-monitor", &scanner);
        let live = Live::new(crate::run::parse_config(text.as_bytes()).unwrap()).unwrap();
        (base, storage, service, live)
    }

    /// M4 review 2 finding 8: `history_status` reports the durable state, not the
    /// pre-collection placeholder: the latest collection job, and a collection schedule
    /// with a backlog as falling behind.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn history_status_reports_the_latest_collection_and_falling_behind() {
        let (base, storage, service, live) = contended_service();
        let tenant = live.config.service.tenant.clone();
        let before = service.history_status(&live).await.unwrap();
        assert_eq!(before["last_collection"], Value::Null);
        assert_eq!(before["falling_behind"], false);
        let p = principal("claude-code");
        submit_many(&storage, &tenant, &p, 2, "status");
        let latest =
            storage.control(|c| c.latest_job(&tenant, "collection_start").unwrap().unwrap());
        storage.control(|c| {
            c.schedule_set(&tenant, true, Some(15), 0).unwrap();
            c.conn
                .execute(
                    "INSERT INTO schedule_slots VALUES (?1,'collection','2026-01-01T00:00:00Z',NULL,'pending')",
                    [&tenant],
                )
                .unwrap();
        });
        let after = service.history_status(&live).await.unwrap();
        assert_eq!(after["last_collection"]["job_id"], latest.job_id.as_str());
        assert_eq!(
            after["falling_behind"], true,
            "a scheduled backlog is falling behind"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M4 review 2 finding 7: receipt ingestion fails closed. With one configured secret
    /// file unavailable, the batch is refused before any reservation or archive effect,
    /// and a later batch under the same key is not a conflict.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn receipts_fail_closed_without_the_complete_secret_registry() {
        let (base, storage, service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-KLMNOPQRST");
        std::fs::remove_file(base.join("scanner-monitor")).unwrap();
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let params: ReceiptsParams = serde_json::from_value(
            json!({"cloud_id": cloud_id, "records": [receipt("rk-closed-01", "monitor", None)]}),
        )
        .unwrap();
        let refused = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-closed-00000001",
                "0a1b2c3d",
                params,
            )
            .await
            .unwrap_err();
        assert_eq!(refused.code, ErrorCode::CredentialBlocked);
        assert!(
            !storage.history_root.join(&cloud_id).exists(),
            "no archive effect"
        );
        let reserved: i64 = storage.control(|c| {
            c.conn
                .query_row("SELECT count(*) FROM ingest_results", [], |r| r.get(0))
                .unwrap()
        });
        assert_eq!(reserved, 0, "nothing reserved");
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M4 review 1 findings 7 and 11 and the refusal-only minor: the batch key is
    /// reserved before any effect (other records under it conflict even while the first
    /// is unfinished); a replay after a crash between the archive commit and the recorded
    /// result reports the same result; a refusal-only batch never creates the archive;
    /// receipt fields are normalized against the service's own secrets.
    /// M4 review 3 minor: a maximum-size (200-record) batch is written under the
    /// service's bounded receipt budget (not the CLI's 250 ms), and a replay of the same
    /// batch, as a client retrying after a timeout would, is idempotent: the same
    /// result, no new receipt, and the archive-level re-ingest counts duplicates only.
    /// Rename an archive run as an earlier build would have named it (the receipt and
    /// run tables only; the foreign key is checked off for this rename).
    fn rename_run(storage: &Storage, cloud_id: &str, from: &str, to: &str) {
        let db =
            rusqlite::Connection::open(storage.history_root.join(cloud_id).join("archive.sqlite3"))
                .unwrap();
        db.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        for table in ["receipt_runs", "collection_runs"] {
            db.execute(
                &format!("UPDATE {table} SET run_id=?2 WHERE run_id=?1"),
                rusqlite::params![from, to],
            )
            .unwrap();
        }
    }

    /// M4 review 10: a batch key reused under a later recovery epoch is a new batch with
    /// its own archive run: counts stay its own and within the 200-record schema bound;
    /// the old epoch's key stays `control_restored`; a same-epoch replay is unchanged.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_batch_key_reused_after_a_restore_is_its_own_batch_and_run() {
        let (base, storage, service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-EPOCHKEY01");
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let params = |records: Vec<Value>| -> ReceiptsParams {
            serde_json::from_value(json!({"cloud_id": cloud_id, "records": records})).unwrap()
        };
        let key = "key-receipts-epoch001";
        let first: Vec<Value> = (0..200)
            .map(|n| receipt(&format!("rk-epoch-{n:05}"), "monitor", None))
            .collect();
        let before = service
            .receipts_ingest(&p, &live, &tenant, key, "0a1b2c3d", params(first.clone()))
            .await
            .unwrap();
        assert_eq!(before["accepted"], 200);
        let caller = (
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        // As an earlier build would have written it: the old batch under the epoch-less
        // run ID, so a lookup of that ID under the new epoch would find it.
        rename_run(
            &storage,
            &cloud_id,
            &receipt_run_id(&tenant, caller, "0a1b2c3d", key),
            &legacy_receipt_run_id(&tenant, caller, key),
        );
        // A restore leaves a new epoch pending; the Administrator activates it.
        storage.control(|c| {
            c.conn
                .execute(
                    "INSERT INTO epochs VALUES ('1b2c3d4e', 1, NULL, 'restore', zeroblob(32), 'pending_activation')",
                    [],
                )
                .unwrap();
            c.activate_epoch("1b2c3d4e", 2).unwrap();
        });
        // The old epoch's key is refused, never merged.
        assert_eq!(
            service
                .receipts_ingest(&p, &live, &tenant, key, "0a1b2c3d", params(first))
                .await
                .unwrap_err()
                .code,
            ErrorCode::ControlRestored
        );
        // The same key text under the new epoch is a new one-record batch, here reserved
        // by a process that died before writing anything (so it recovers as pending),
        // with an arbitrary reservation time (no clock is consulted): the reservation's
        // own binding decides, so it never adopts the earlier epoch's epoch-less run.
        let one = vec![receipt("rk-epoch-new-00001", "monitor", None)];
        let digest = crate::control::canonical(&json!({"cloud_id": cloud_id, "records": one})).1;
        let bound = receipt_run_id(&tenant, caller, "1b2c3d4e", key);
        storage.control(|c| {
            let prior = c
                .ingest_reserve(&tenant, caller, "1b2c3d4e", key, &digest, &bound, 1)
                .unwrap();
            assert_eq!(
                prior,
                crate::ingest_store::Prior::Pending(Some(bound.clone()))
            );
        });
        let after = service
            .receipts_ingest(&p, &live, &tenant, key, "1b2c3d4e", params(one.clone()))
            .await
            .unwrap();
        assert_eq!(after["accepted"], 1, "{after}");
        assert_eq!(after["duplicates"], 0, "{after}");
        let replay = service
            .receipts_ingest(&p, &live, &tenant, key, "1b2c3d4e", params(one))
            .await
            .unwrap();
        assert_eq!(
            replay, after,
            "a same-epoch replay returns the recorded result"
        );
        let db = rusqlite::Connection::open_with_flags(
            storage.history_root.join(&cloud_id).join("archive.sqlite3"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let (receipts, runs): (i64, i64) = db
            .query_row(
                "SELECT (SELECT count(*) FROM receipts), (SELECT count(DISTINCT run_id) FROM receipt_runs)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((receipts, runs), (201, 2), "one archive run per batch");
        drop(db);
        // An earlier build's unbound reservation of the same key text under the new
        // epoch, where only the earlier epoch's epoch-less run exists: ownership cannot be
        // proven after a restore, so the batch is refused with no archive effect and its
        // reservation stays unbound for explicit reconciliation.
        let key2 = "key-receipts-epoch002";
        let two = vec![receipt("rk-epoch-amb-00001", "monitor", None)];
        let digest2 = crate::control::canonical(&json!({"cloud_id": cloud_id, "records": two})).1;
        storage.control(|c| {
            c.conn
                .execute(
                    "INSERT INTO ingest_results VALUES (?1,?2,?3,'1b2c3d4e',?4,?5,'{\"pending\":true}',1)",
                    rusqlite::params![tenant, caller.0, caller.1, key2, digest2],
                )
                .unwrap();
        });
        rename_run(
            &storage,
            &cloud_id,
            &legacy_receipt_run_id(&tenant, caller, key),
            &legacy_receipt_run_id(&tenant, caller, key2),
        );
        let archive_before =
            std::fs::read(storage.history_root.join(&cloud_id).join("archive.sqlite3")).unwrap();
        let ambiguous = service
            .receipts_ingest(&p, &live, &tenant, key2, "1b2c3d4e", params(two))
            .await
            .unwrap_err();
        assert_eq!(ambiguous.code, ErrorCode::Conflict);
        assert_eq!(
            std::fs::read(storage.history_root.join(&cloud_id).join("archive.sqlite3")).unwrap(),
            archive_before,
            "an ambiguous batch writes nothing"
        );
        storage.control(|c| {
            let unbound: bool = c
                .conn
                .query_row(
                    "SELECT json_extract(result_json,'$.run.id') IS NULL FROM ingest_results WHERE key=?1",
                    [key2],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(unbound, "an unproven reservation is never bound");
        });
        // The run ID is the canonical scope tuple: the epoch and every field separate.
        let a = receipt_run_id(&tenant, caller, "0a1b2c3d", key);
        assert_ne!(a, receipt_run_id(&tenant, caller, "1b2c3d4e", key));
        assert_ne!(
            receipt_run_id("t", ("a", "b\u{1f}c"), "e", "k"),
            receipt_run_id("t", ("a\u{1f}b", "c"), "e", "k"),
            "no field runs into another"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// A batch an earlier build left pending (its key reserved, its records written
    /// under the epoch-less run ID) is recovered into that run, not re-counted.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_pending_batch_from_an_earlier_build_recovers_into_its_run() {
        let (base, storage, service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-LEGACYRUN1");
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let records = vec![
            receipt("rk-legacy-00001", "monitor", None),
            receipt("rk-legacy-00002", "monitor", None),
        ];
        let params = || -> ReceiptsParams {
            serde_json::from_value(json!({"cloud_id": cloud_id, "records": records})).unwrap()
        };
        let key = "key-receipts-legacy01";
        // Write the batch under the legacy run ID, then leave its reservation pending
        // (a crash after the archive write, before the result was recorded).
        let done = service
            .receipts_ingest(&p, &live, &tenant, key, "0a1b2c3d", params())
            .await
            .unwrap();
        assert_eq!(done["accepted"], 2);
        let caller = (
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let new_run = receipt_run_id(&tenant, caller, "0a1b2c3d", key);
        let legacy = legacy_receipt_run_id(&tenant, caller, key);
        rename_run(&storage, &cloud_id, &new_run, &legacy);
        storage.control(|c| {
            c.conn
                .execute(
                    "UPDATE ingest_results SET result_json='{\"pending\":true}' WHERE key=?1",
                    [key],
                )
                .unwrap();
        });
        // In a store that was never restored (one epoch), the epoch-less run can only be
        // this reservation's: it recovers into it, and the binding is recorded. With the
        // acceptance record the run carries here, its counts are proven.
        let recovered = service
            .receipts_ingest(&p, &live, &tenant, key, "0a1b2c3d", params())
            .await
            .unwrap();
        assert_eq!(recovered["accepted"], 2, "{recovered}");
        assert_eq!(recovered["duplicates"], 0, "{recovered}");
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M4 review 14 minor: a refusal-only batch returns its refusal result without
    /// touching the archive, even while the archive is exclusively locked.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_refusal_only_batch_needs_no_archive() {
        let (base, storage, service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-REFUSEONLY");
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let params = |records: Vec<Value>| -> ReceiptsParams {
            serde_json::from_value(json!({"cloud_id": cloud_id, "records": records})).unwrap()
        };
        // Create the archive, then hold it exclusively.
        service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-refuse01",
                "0a1b2c3d",
                params(vec![receipt("rk-refuse-00001", "monitor", None)]),
            )
            .await
            .unwrap();
        let lock = rusqlite::Connection::open(
            storage.history_root.join(&cloud_id).join("archive.sqlite3"),
        )
        .unwrap();
        lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
        let refused = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-refuse02",
                "0a1b2c3d",
                params(vec![receipt("rk-refuse-00002", "unknown-scanner", None)]),
            )
            .await
            .unwrap();
        assert_eq!(refused["accepted"], 0, "{refused}");
        assert_eq!(refused["refused"].as_array().unwrap().len(), 1, "{refused}");
        lock.execute_batch("ROLLBACK").unwrap();
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M4 review 13: a bound pending batch whose run was written before acceptance
    /// records existed (provenance without `first_accepted`) already holding receipts:
    /// its counts cannot be proven, so recovery is refused `conflict` with no archive
    /// effect, whatever the run timestamps say; the archive writer itself refuses to
    /// extend such a run.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_older_unrecorded_run_is_refused_not_recounted() {
        let (base, storage, service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-OLDERRUN01");
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let caller = (
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let records = vec![
            receipt("rk-older-00001", "monitor", None),
            receipt("rk-older-00002", "monitor", None),
        ];
        let params = || -> ReceiptsParams {
            serde_json::from_value(json!({"cloud_id": cloud_id, "records": records})).unwrap()
        };
        let key = "key-receipts-older01";
        let done = service
            .receipts_ingest(&p, &live, &tenant, key, "0a1b2c3d", params())
            .await
            .unwrap();
        assert_eq!(done["accepted"], 2);
        let run = receipt_run_id(&tenant, caller, "0a1b2c3d", key);
        let path = storage.history_root.join(&cloud_id).join("archive.sqlite3");
        {
            let db = rusqlite::Connection::open(&path).unwrap();
            db.execute(
                "UPDATE collection_runs SET provenance_json=json_remove(provenance_json,'$.first_accepted'), started_ms=started_ms+3600000 WHERE run_id=?1",
                [&run],
            )
            .unwrap();
        }
        storage.control(|c| {
            c.conn
                .execute(
                    "UPDATE ingest_results SET result_json=?2 WHERE key=?1",
                    rusqlite::params![
                        key,
                        json!({"pending": true, "run": {"id": run, "format": 2}}).to_string()
                    ],
                )
                .unwrap();
        });
        let before = std::fs::read(&path).unwrap();
        let refused = service
            .receipts_ingest(&p, &live, &tenant, key, "0a1b2c3d", params())
            .await
            .unwrap_err();
        assert_eq!(refused.code, ErrorCode::Conflict);
        assert_eq!(std::fs::read(&path).unwrap(), before, "no archive effect");
        // The writer refuses to begin a partial record on such a run.
        let config = live.config.clone();
        let scanner = config.airs.scanners[0].clone();
        let extra: ReceiptsParams = serde_json::from_value(
            json!({"cloud_id": cloud_id, "records": [receipt("rk-older-00003", "monitor", None)]}),
        )
        .unwrap();
        let received: Vec<_> = extra.records.iter().map(|r| r.received(&scanner)).collect();
        assert!(
            airs_history_store::repository::ingest_client_receipts(
                &storage.history_root,
                &cloud(&config).unwrap(),
                &run,
                &received,
                &Default::default(),
                &Default::default(),
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before, "no archive effect");
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// The binding of a round-11-shaped reservation: unbound, but its batch written
    /// under the scope-named run. That run proves ownership, so it never switches runs.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_unbound_reservation_with_its_scoped_run_keeps_that_run() {
        let (base, storage, service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-SCOPEDRUN1");
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let records = vec![
            receipt("rk-scoped-00001", "monitor", None),
            receipt("rk-scoped-00002", "monitor", None),
        ];
        let params = || -> ReceiptsParams {
            serde_json::from_value(json!({"cloud_id": cloud_id, "records": records})).unwrap()
        };
        let key = "key-receipts-scoped01";
        let done = service
            .receipts_ingest(&p, &live, &tenant, key, "0a1b2c3d", params())
            .await
            .unwrap();
        assert_eq!(done["accepted"], 2);
        storage.control(|c| {
            c.conn
                .execute(
                    "UPDATE ingest_results SET result_json='{\"pending\":true}' WHERE key=?1",
                    [key],
                )
                .unwrap();
        });
        // An epoch-less run of the same key text also exists (as an earlier build would
        // have named one): the scope-named run still decides.
        {
            let caller = (
                "PLACEHOLDER_CALLER_SUB",
                "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
            );
            let db = rusqlite::Connection::open(
                storage.history_root.join(&cloud_id).join("archive.sqlite3"),
            )
            .unwrap();
            db.execute(
                "INSERT INTO collection_runs SELECT ?2,kind,started_ms,ended_ms,target_since_ms,target_until_ms,state,json_set(provenance_json,'$.first_accepted',json('[]')) FROM collection_runs WHERE run_id=?1",
                rusqlite::params![
                    receipt_run_id(&tenant, caller, "0a1b2c3d", key),
                    legacy_receipt_run_id(&tenant, caller, key)
                ],
            )
            .unwrap();
        }
        let recovered = service
            .receipts_ingest(&p, &live, &tenant, key, "0a1b2c3d", params())
            .await
            .unwrap();
        assert_eq!(recovered, done, "no run switch: the same counts");
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// Counts come from the run's own first-acceptance record: rewriting run start
    /// times (a clock rollback) changes no batch's result, including on recovery.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn receipt_counts_do_not_depend_on_run_timestamps() {
        let (base, storage, service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-ROLLBACK01");
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let caller = (
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let records = vec![
            receipt("rk-clock-00001", "monitor", None),
            receipt("rk-clock-00002", "monitor", None),
        ];
        let params = || -> ReceiptsParams {
            serde_json::from_value(json!({"cloud_id": cloud_id, "records": records})).unwrap()
        };
        let first = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-clock01",
                "0a1b2c3d",
                params(),
            )
            .await
            .unwrap();
        assert_eq!(
            (first["accepted"].clone(), first["duplicates"].clone()),
            (json!(2), json!(0))
        );
        let second = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-clock02",
                "0a1b2c3d",
                params(),
            )
            .await
            .unwrap();
        assert_eq!(
            (second["accepted"].clone(), second["duplicates"].clone()),
            (json!(0), json!(2))
        );
        // The first batch's run now appears to start long after the second's, and both
        // reservations are pending again (crashes before the results were recorded).
        let (run1, run2) = (
            receipt_run_id(&tenant, caller, "0a1b2c3d", "key-receipts-clock01"),
            receipt_run_id(&tenant, caller, "0a1b2c3d", "key-receipts-clock02"),
        );
        {
            let db = rusqlite::Connection::open(
                storage.history_root.join(&cloud_id).join("archive.sqlite3"),
            )
            .unwrap();
            db.execute(
                "UPDATE collection_runs SET started_ms=(SELECT started_ms FROM collection_runs WHERE run_id=?2)+3600000 WHERE run_id=?1",
                rusqlite::params![run1, run2],
            )
            .unwrap();
        }
        for (key, run) in [
            ("key-receipts-clock01", &run1),
            ("key-receipts-clock02", &run2),
        ] {
            storage.control(|c| {
                c.conn
                    .execute(
                        "UPDATE ingest_results SET result_json=?2 WHERE key=?1",
                        rusqlite::params![
                            key,
                            json!({"pending": true, "run": {"id": run, "format": 2}}).to_string()
                        ],
                    )
                    .unwrap();
            });
        }
        let again1 = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-clock01",
                "0a1b2c3d",
                params(),
            )
            .await
            .unwrap();
        let again2 = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-clock02",
                "0a1b2c3d",
                params(),
            )
            .await
            .unwrap();
        assert_eq!(again1, first, "the first batch keeps its acceptances");
        assert_eq!(again2, second, "the later batch keeps its duplicates");
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_maximum_receipt_batch_fits_the_service_budget_and_replays_idempotently() {
        assert!(
            airs_history_store::repository::SERVICE_RECEIPT_BUDGET >= Duration::from_secs(5),
            "the service budget is not the CLI's local 250 ms"
        );
        let (base, storage, service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-MAXBATCH01");
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let records: Vec<Value> = (0..200)
            .map(|n| receipt(&format!("rk-max-{n:05}"), "monitor", None))
            .collect();
        let params = || -> ReceiptsParams {
            serde_json::from_value(json!({"cloud_id": cloud_id, "records": records})).unwrap()
        };
        let first = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-max0001",
                "0a1b2c3d",
                params(),
            )
            .await
            .unwrap();
        assert_eq!(first["accepted"], 200, "{first}");
        let again = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-max0001",
                "0a1b2c3d",
                params(),
            )
            .await
            .unwrap();
        assert_eq!(again, first, "the replay returns the recorded result");
        let db = rusqlite::Connection::open_with_flags(
            storage.history_root.join(&cloud_id).join("archive.sqlite3"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let n: i64 = db
            .query_row("SELECT count(*) FROM receipts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 200, "no receipt was written twice");
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn receipt_batches_reserve_first_replay_stably_and_never_retain_service_secrets() {
        let secret = "SYNTHETIC-SERVICE-SECRET-0123456789";
        let (base, storage, service, live) = receipts_service_with_secret(secret);
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let params = |records: Vec<Value>| -> ReceiptsParams {
            serde_json::from_value(json!({"cloud_id": cloud_id, "records": records})).unwrap()
        };
        let archive = storage.history_root.join(&cloud_id).join("archive.sqlite3");
        // Refusal-only: nothing to retain, so no archive is opened or created.
        let refused_only = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-r00001",
                "0a1b2c3d",
                params(vec![receipt("rk-refused-01", "unknown-scanner", None)]),
            )
            .await
            .unwrap();
        assert_eq!(refused_only["refused"].as_array().unwrap().len(), 1);
        assert!(
            !archive.exists(),
            "a refusal-only batch never creates the archive"
        );
        // A secret-bearing session ID is never retained.
        let mut carrying = receipt("rk-secret-0001", "monitor", None);
        carrying["session_id"] = json!(secret);
        let first = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-s00001",
                "0a1b2c3d",
                params(vec![carrying.clone()]),
            )
            .await
            .unwrap();
        assert_eq!(first["accepted"], 1);
        let bytes = std::fs::read(&archive).unwrap();
        assert!(
            !bytes.windows(secret.len()).any(|w| w == secret.as_bytes()),
            "the service secret is not retained"
        );
        // A crash between the archive commit and the recorded result: the reservation is
        // still pending, with the run it bound. The replay reports exactly the original
        // result.
        let bound = receipt_run_id(
            &tenant,
            (
                "PLACEHOLDER_CALLER_SUB",
                "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
            ),
            "0a1b2c3d",
            "key-receipts-s00001",
        );
        storage.control(|c| {
            c.conn
                .execute(
                    "UPDATE ingest_results SET result_json=?1 WHERE key='key-receipts-s00001'",
                    [json!({"pending": true, "run": {"id": bound, "format": 2}}).to_string()],
                )
                .unwrap()
        });
        let replay = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-s00001",
                "0a1b2c3d",
                params(vec![carrying.clone()]),
            )
            .await
            .unwrap();
        assert_eq!(
            replay, first,
            "a replay after the crash reports the same result"
        );
        // Other records under a key reserved (even pending) conflict before any write.
        storage.control(|c| {
            c.conn
                .execute("UPDATE ingest_results SET result_json='{\"pending\":true}' WHERE key='key-receipts-s00001'", [])
                .unwrap()
        });
        let before = std::fs::read(&archive).unwrap();
        let conflict = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-s00001",
                "0a1b2c3d",
                params(vec![receipt("rk-other-00001", "monitor", None)]),
            )
            .await
            .unwrap_err();
        assert_eq!(conflict.code, ErrorCode::IdempotencyConflict);
        assert_eq!(
            std::fs::read(&archive).unwrap(),
            before,
            "nothing was written"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M4 review 1 finding 7: the key is bound to its digest before any archive effect.
    /// A first attempt whose archive write fails still holds the key, so other records
    /// under it are a conflict (they are never written under the bound key).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_batch_key_is_bound_before_any_archive_effect() {
        // The complete secret registry is present (receipts fail closed without it).
        let (base, storage, service, live) =
            receipts_service_with_secret("SYNTHETIC-SERVICE-SECRET-UVWXYZ0123");
        let tenant = live.config.service.tenant.clone();
        let p = Principal::for_tests(
            "PLACEHOLDER_CALLER_SUB",
            "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID",
        );
        let cloud_id = cloud(&live.config).unwrap().cloud_id;
        let params = |records: Vec<Value>| -> ReceiptsParams {
            serde_json::from_value(json!({"cloud_id": cloud_id, "records": records})).unwrap()
        };
        // A measured reserve first (the shared sampler keeps it for SPACE_MAX_AGE), so
        // both calls pass the reserve gate. An unlistable history root alone would make
        // the reserve unknown and refuse them before the key is bound (fail closed).
        assert_eq!(
            storage.space_bounded(&tenant).await.level,
            crate::space::Level::Normal
        );
        // The archive cannot be written: its root is a regular file.
        std::fs::write(&storage.history_root, b"not a directory").unwrap();
        let failed = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-b00001",
                "0a1b2c3d",
                params(vec![receipt("rk-bound-00001", "monitor", None)]),
            )
            .await
            .unwrap_err();
        assert_eq!(failed.code, ErrorCode::StorageUnavailable);
        let other = service
            .receipts_ingest(
                &p,
                &live,
                &tenant,
                "key-receipts-b00001",
                "0a1b2c3d",
                params(vec![receipt("rk-bound-00002", "monitor", None)]),
            )
            .await
            .unwrap_err();
        assert_eq!(
            other.code,
            ErrorCode::IdempotencyConflict,
            "the key was bound before the failed effect"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// A report job of `tenant` for `report` (the job a published version names).
    fn report_job(storage: &Arc<Storage>, tenant: &str, report: &str) -> String {
        let args = json!({"report_id": report, "kind": "manual", "since_ms": 1, "until_ms": 2});
        storage.control(|c| {
            c.submit(
                crate::control::Submission {
                    tenant,
                    operation: "reports_create",
                    actor: crate::control::Actor::Service("test".into()),
                    idempotency_key: &format!("key-report-job-{}", crate::reports::new_id("k")),
                    expected_epoch: "0a1b2c3d",
                    params: &args,
                    config_revision: "test-1",
                    credential_revision: None,
                    deadline_ms: i64::MAX / 2,
                    queue_tenant: 1_000,
                    queue_principal: 1_000,
                    join_equivalent: false,
                    scheduled_slot: None,
                },
                1,
            )
            .unwrap()
            .job_id
        })
    }

    /// Publish a synthetic version (as the report executor would) with a review.md
    /// long enough to need several pages.
    fn publish_fixture(
        storage: &Arc<Storage>,
        tenant: &str,
    ) -> (crate::reports::Bundles, crate::reports::VersionRow) {
        let dir = crate::storage::private_child(&storage.admitted, "reports").unwrap();
        let bundles = crate::reports::Bundles { root: dir.path };
        let report = crate::reports::new_id("r1");
        let version = crate::reports::new_id("v1");
        let job = report_job(storage, tenant, &report);
        let review = "# Review\n".to_owned() + &"line of review text, with ünïcode\n".repeat(600);
        let files: [(&str, Vec<u8>); 3] = [
            ("report.html", vec![b'<'; 150_000]),
            ("evidence.json", b"{}".to_vec()),
            ("review.md", review.into_bytes()),
        ];
        let manifest = json!({
            "report_id": report, "version_id": version, "version": 0, "kind": "manual", "job_id": job,
            "period": {"since": "2026-11-05T00:00:00Z", "until": "2026-11-06T00:00:00Z"}, "scheduled_at": null, "as_of": "2026-11-06T00:00:00Z",
            "timezone": "America/Chicago", "tzdb_version": "jiff-bundled", "analysis_version": 1, "redaction_version": 1, "completeness": "complete",
            "gaps": [], "predecessor": null,
            "files": files.iter().map(|(n, b)| json!({"name": n, "bytes": b.len(), "sha256": hex(&Sha256::digest(b))})).collect::<Vec<_>>(),
        });
        let staged: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
        let stage = bundles.stage(&staged).unwrap();
        bundles.place(&stage, &report, &version).unwrap();
        let row = crate::reports::VersionRow {
            version_id: version,
            report_id: report.clone(),
            version: 0,
            digest: "ab".repeat(32),
            manifest,
            summary: "1 event".into(),
        };
        storage.control(|c| {
            c.ensure_report(tenant, &report, "manual", (1, 2), &[], "t", None, 1)
                .unwrap();
            let fence = c.fence(tenant).unwrap();
            c.record_version(tenant, &row, "manual", &job, fence, false, 1)
                .unwrap();
        });
        (bundles, row)
    }

    /// Publish one more version of `report` (as the report executor would).
    fn publish_into(
        storage: &Arc<Storage>,
        tenant: &str,
        report: &str,
        version: i64,
    ) -> crate::reports::VersionRow {
        let dir = crate::storage::private_child(&storage.admitted, "reports").unwrap();
        let bundles = crate::reports::Bundles { root: dir.path };
        let version_id = crate::reports::new_id("v1");
        let job = report_job(storage, tenant, report);
        let files: [(&str, Vec<u8>); 2] = [
            (
                "report.html",
                format!("<p>version {version}</p>").into_bytes(),
            ),
            ("review.md", format!("# Review {version}\n").into_bytes()),
        ];
        let manifest = json!({
            "report_id": report, "version_id": version_id, "version": version, "kind": "manual",
            "files": files.iter().map(|(n, b)| json!({"name": n, "bytes": b.len(), "sha256": hex(&Sha256::digest(b))})).collect::<Vec<_>>(),
        });
        let staged: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
        let stage = bundles.stage(&staged).unwrap();
        bundles.place(&stage, report, &version_id).unwrap();
        let row = crate::reports::VersionRow {
            version_id,
            report_id: report.to_owned(),
            version,
            digest: "cd".repeat(32),
            manifest,
            summary: format!("version {version}"),
        };
        storage.control(|c| {
            c.ensure_report(tenant, report, "manual", (1, 2), &[], "t", None, 1)
                .unwrap();
            let fence = c.fence(tenant).unwrap();
            c.record_version(tenant, &row, "manual", &job, fence, false, 1 + version)
                .unwrap();
        });
        row
    }

    /// M6 `artifact_delete` (moved from M5): previewed read-only with exactly what goes
    /// (versions, files, bytes, the latest-pointer move); refused without the direct
    /// proof and for a non-administrator; executed as a durable job that tombstones the
    /// version, moves the latest pointer to the newest remaining version, removes the
    /// bundle, and refuses its retrieval; a keyed retry joins the same job; a stale
    /// preview (the reports fence advanced) is refused.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn report_artifacts_are_deleted_through_the_lifecycle_tools() {
        let (base, storage, service, live) = admin_service();
        let tenant = live.config.service.tenant.clone();
        let report = crate::reports::new_id("r1");
        let older = publish_into(&storage, &tenant, &report, 0);
        let newer = publish_into(&storage, &tenant, &report, 1);
        let bundles = service.bundles().unwrap();
        assert!(bundles.dir(&report, &newer.version_id).exists());
        let preview = |ids: Vec<&str>| -> LifecyclePreviewParams {
            serde_json::from_value(
                json!({"action": "artifact_delete", "artifact_version_ids": ids}),
            )
            .unwrap()
        };
        // A non-administrator is refused both tools at dispatch (the grant gate).
        let reader = principal("claude-code");
        for tool in ["lifecycle_preview", "lifecycle_execute"] {
            assert!(!granted(&reader, catalogue::get(tool).unwrap()), "{tool}");
        }
        // An unknown version is not found.
        let anon = admin("airs-admin-cli", None);
        assert_eq!(
            service
                .lifecycle_preview(
                    &anon,
                    &live,
                    &tenant,
                    preview(vec!["v1_00000000-0000-4000-8000-0000000000ff"])
                )
                .await
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        let pv = service
            .lifecycle_preview(&anon, &live, &tenant, preview(vec![&newer.version_id]))
            .await
            .unwrap();
        assert_eq!(pv["affected"]["report_versions"], 1, "{pv}");
        let summary = pv["summary"].as_str().unwrap();
        assert!(summary.contains("2 file(s)"), "{summary}");
        assert!(
            summary.contains(&format!("latest -> {}", older.version_id)),
            "{summary}"
        );
        assert!(
            bundles.dir(&report, &newer.version_id).exists(),
            "a preview writes nothing"
        );
        let exec = || -> LifecycleExecuteParams {
            serde_json::from_value(
                json!({"preview_id": pv["preview_id"], "parameter_digest": pv["parameter_digest"]}),
            )
            .unwrap()
        };
        assert_eq!(
            service
                .lifecycle_execute(
                    &anon,
                    &live,
                    &tenant,
                    "key-artifact-del-0001",
                    "0a1b2c3d",
                    exec()
                )
                .await
                .unwrap_err()
                .code,
            ErrorCode::Forbidden,
            "no proof, no deletion"
        );
        let proved = admin("airs-admin-cli", pv["proof_nonce"].as_str());
        let job = service
            .lifecycle_execute(
                &proved,
                &live,
                &tenant,
                "key-artifact-del-0001",
                "0a1b2c3d",
                exec(),
            )
            .await
            .unwrap();
        let again = service
            .lifecycle_execute(
                &proved,
                &live,
                &tenant,
                "key-artifact-del-0001",
                "0a1b2c3d",
                exec(),
            )
            .await
            .unwrap();
        assert_eq!(
            again["job"]["job_id"], job["job"]["job_id"],
            "a keyed retry joins the job"
        );
        run_lifecycle(&storage, &live);
        let id = job["job"]["job_id"].as_str().unwrap().to_owned();
        let done = storage.control(|c| c.job(&tenant, &id).unwrap().unwrap());
        assert_eq!(done.state, State::Succeeded, "{:?}", done.result);
        let result = done.result.unwrap();
        assert_eq!(result["action"], "artifact_delete");
        assert_eq!(result["tombstones"], 1);
        // The bundle is gone, the other version stays, and the pointer moved to it.
        assert!(!bundles.dir(&report, &newer.version_id).exists());
        assert!(bundles.dir(&report, &older.version_id).exists());
        let got = service
            .reports_get(
                &tenant,
                serde_json::from_value(json!({"report_id": report})).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(got["latest_version"], older.version_id.as_str());
        let read = service
            .artifacts_read(
                &reader,
                &tenant,
                serde_json::from_value(
                    json!({"name": "review.md", "version_id": newer.version_id}),
                )
                .unwrap(),
            )
            .await;
        assert!(read.is_err(), "a deleted version is never retrievable");
        assert!(storage.control(|c| c.version_deleted(&tenant, &newer.version_id).unwrap()));
        // A preview made before another deletion advanced the fence is stale.
        let stale = service
            .lifecycle_preview(&anon, &live, &tenant, preview(vec![&older.version_id]))
            .await
            .unwrap();
        storage.control(|c| {
            c.advance_fence(&tenant).unwrap();
        });
        let proved = admin("airs-admin-cli", stale["proof_nonce"].as_str());
        assert!(
            service
                .lifecycle_execute(
                    &proved,
                    &live,
                    &tenant,
                    "key-artifact-del-0002",
                    "0a1b2c3d",
                    serde_json::from_value(json!({"preview_id": stale["preview_id"], "parameter_digest": stale["parameter_digest"]})).unwrap(),
                )
                .await
                .is_err(),
            "a stale deletion preview is refused"
        );
        assert!(bundles.dir(&report, &older.version_id).exists());
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// `artifact_delete` crash safety: a deletion whose control transaction committed
    /// but whose files were not yet removed (the process stopped) is completed by
    /// recovery, which never reattaches or presents the deleted version; a leftover
    /// `.deleted-*` directory of an interrupted removal is removed too; a retry of the
    /// deletion (versions already tombstoned) succeeds without changing anything else.
    #[test]
    fn an_interrupted_artifact_deletion_is_completed_by_recovery() {
        let (base, storage, _service, _live) = admin_service();
        let tenant = _live.config.service.tenant.clone();
        let report = crate::reports::new_id("r1");
        let keep = publish_into(&storage, &tenant, &report, 0);
        let gone = publish_into(&storage, &tenant, &report, 1);
        let dir = crate::storage::private_child(&storage.admitted, "reports").unwrap();
        let bundles = crate::reports::Bundles { root: dir.path };
        let job = storage.control(|c| {
            let e = json!({});
            c.submit(
                crate::control::Submission {
                    tenant: &tenant,
                    operation: "lifecycle_execute",
                    actor: crate::control::Actor::Service("test".into()),
                    idempotency_key: "key-artifact-crash-01",
                    expected_epoch: "0a1b2c3d",
                    params: &e,
                    config_revision: "r",
                    credential_revision: None,
                    deadline_ms: i64::MAX / 2,
                    queue_tenant: 64,
                    queue_principal: 64,
                    join_equivalent: false,
                    scheduled_slot: None,
                },
                1,
            )
            .unwrap()
            .job_id
        });
        // Committed in the control store; the process stops before removing files.
        let ids = vec![gone.version_id.clone()];
        let fence = storage.control(|c| c.fence(&tenant).unwrap());
        let (now, already, _) =
            storage.control(|c| c.delete_versions(&tenant, &ids, &job, 2).unwrap());
        assert_eq!((now, already), (1, 0));
        assert!(
            storage.control(|c| c.fence(&tenant).unwrap()) > fence,
            "the deletion advances the reports fence (C7: a render in flight cannot publish)"
        );
        assert!(bundles.dir(&report, &gone.version_id).exists());
        std::fs::create_dir(bundles.root.join(".deleted-leftover")).unwrap();
        // Recovery completes the deletion and never reattaches the version.
        let (reattached, _, _) = storage.control(|c| bundles.recover(c, &tenant, 3).unwrap());
        assert_eq!(reattached, 0);
        assert!(!bundles.dir(&report, &gone.version_id).exists());
        assert!(!bundles.root.join(".deleted-leftover").exists());
        assert!(
            storage
                .control(|c| c.version(&tenant, &gone.version_id).unwrap())
                .is_none()
        );
        assert!(bundles.dir(&report, &keep.version_id).exists());
        // A retry of the same deletion is a no-op success.
        let (now, already, _) =
            storage.control(|c| c.delete_versions(&tenant, &ids, &job, 4).unwrap());
        assert_eq!((now, already), (0, 1));
        let latest =
            storage.control(|c| c.report(&tenant, &report).unwrap().unwrap().latest_version);
        assert_eq!(latest.as_deref(), Some(keep.version_id.as_str()));
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M6.3 retrieval: reports_list/reports_get show the frozen manifest and summary;
    /// artifacts_read pages review.md in bounded segments under a caller- and
    /// version-bound cursor that reassemble to the hashed file; artifacts_download
    /// chunks carry their own and the file's SHA-256 and reassemble exactly; a file that
    /// no longer matches its manifest is refused; another tenant sees nothing.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reports_and_artifacts_are_retrievable_bounded_and_hash_bound() {
        let (base, storage, service, live) = contended_service();
        let tenant = live.config.service.tenant.clone();
        let p = principal("claude-code");
        let (bundles, row) = publish_fixture(&storage, &tenant);
        let listed = service
            .reports_list(&p, &tenant, serde_json::from_value(json!({})).unwrap())
            .await
            .unwrap();
        assert_eq!(listed["items"][0]["version_id"], row.version_id.as_str());
        let got = service
            .reports_get(
                &tenant,
                serde_json::from_value(json!({"report_id": row.report_id})).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(got["latest_version"], row.version_id.as_str());
        assert_eq!(got["summary"], "1 event");
        // Paged review.
        let review = std::fs::read(
            bundles
                .dir(&row.report_id, &row.version_id)
                .join("review.md"),
        )
        .unwrap();
        let mut text = String::new();
        let mut cursor: Option<String> = None;
        let mut pages = 0;
        loop {
            let mut params = json!({"name": "review.md", "version_id": row.version_id});
            if let Some(c) = &cursor {
                params["cursor"] = json!(c);
            }
            let page = service
                .artifacts_read(&p, &tenant, serde_json::from_value(params).unwrap())
                .await
                .unwrap();
            assert!(
                result_bytes(&CallToolResult::structured(page.clone()))
                    <= catalogue::get("artifacts_read").unwrap().result_limit
            );
            assert_eq!(page["sha256"], hex(&Sha256::digest(&review)));
            text.push_str(page["segment"].as_str().unwrap());
            pages += 1;
            match page["next_cursor"].as_str() {
                Some(c) => cursor = Some(c.to_owned()),
                None => break,
            }
        }
        assert!(pages > 1);
        assert_eq!(
            text.as_bytes(),
            review.as_slice(),
            "pages reassemble the file"
        );
        // Another principal cannot continue this caller's cursor.
        let first = service
            .artifacts_read(
                &p,
                &tenant,
                serde_json::from_value(json!({"name": "review.md", "version_id": row.version_id}))
                    .unwrap(),
            )
            .await
            .unwrap();
        let other = principal("other-agent");
        let stolen = json!({"name": "review.md", "version_id": row.version_id, "cursor": first["next_cursor"]});
        assert!(
            service
                .artifacts_read(&other, &tenant, serde_json::from_value(stolen).unwrap())
                .await
                .is_err()
        );
        // Chunked download.
        let html = std::fs::read(
            bundles
                .dir(&row.report_id, &row.version_id)
                .join("report.html"),
        )
        .unwrap();
        let mut got = Vec::new();
        let mut offset = 0u64;
        while (offset as usize) < html.len() {
            let chunk = service.artifacts_download(&tenant, serde_json::from_value(json!({"name": "report.html", "version_id": row.version_id, "offset": offset, "length": 65536})).unwrap()).await.unwrap();
            let data = base64::engine::general_purpose::STANDARD
                .decode(chunk["data_base64"].as_str().unwrap())
                .unwrap();
            assert_eq!(chunk["chunk_sha256"], hex(&Sha256::digest(&data)));
            assert_eq!(chunk["file_sha256"], hex(&Sha256::digest(&html)));
            assert!(
                result_bytes(&CallToolResult::structured(chunk.clone()))
                    <= catalogue::get("artifacts_download").unwrap().result_limit
            );
            offset += data.len() as u64;
            got.extend(data);
        }
        assert_eq!(got, html);
        // Tenant isolation.
        assert_eq!(
            service
                .reports_get(
                    "svc-other",
                    serde_json::from_value(json!({"report_id": row.report_id})).unwrap()
                )
                .await
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        // A tampered file no longer matches its manifest.
        std::fs::write(
            bundles
                .dir(&row.report_id, &row.version_id)
                .join("report.html"),
            b"tampered",
        )
        .unwrap();
        assert_eq!(
            service.artifacts_download(&tenant, serde_json::from_value(json!({"name": "report.html", "version_id": row.version_id, "offset": 0, "length": 10})).unwrap()).await.unwrap_err().code,
            ErrorCode::StorageUnavailable
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M6.4: an assessment is attributed to the caller, bound to the exact review.md it
    /// assessed, kept beside the version, never changing the bundle; a stale review hash
    /// is a conflict; the same key repeats its result.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn assessments_are_attributed_and_never_change_evidence() {
        let (base, storage, service, live) = contended_service();
        let tenant = live.config.service.tenant.clone();
        let p = principal("claude-code");
        let (bundles, row) = publish_fixture(&storage, &tenant);
        let review_sha = row.manifest["files"][2]["sha256"]
            .as_str()
            .unwrap()
            .to_owned();
        let before = std::fs::read(
            bundles
                .dir(&row.report_id, &row.version_id)
                .join("review.md"),
        )
        .unwrap();
        let params = |sha: &str| -> AssessmentParams {
            serde_json::from_value(json!({"version_id": row.version_id, "review_sha256": sha, "markdown": "Looks benign."})).unwrap()
        };
        assert_eq!(
            service
                .assessments_submit(
                    &p,
                    &tenant,
                    "key-assess-00000001",
                    "0a1b2c3d",
                    params(&"cd".repeat(32))
                )
                .await
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
        let ok = service
            .assessments_submit(
                &p,
                &tenant,
                "key-assess-00000002",
                "0a1b2c3d",
                params(&review_sha),
            )
            .await
            .unwrap();
        assert_eq!(ok["attributed_to"]["kind"], "principal");
        assert!(ok["assessment_id"].as_str().unwrap().starts_with("a1_"));
        let again = service
            .assessments_submit(
                &p,
                &tenant,
                "key-assess-00000002",
                "0a1b2c3d",
                params(&review_sha),
            )
            .await
            .unwrap();
        assert_eq!(again, ok);
        assert_eq!(
            std::fs::read(
                bundles
                    .dir(&row.report_id, &row.version_id)
                    .join("review.md")
            )
            .unwrap(),
            before,
            "evidence unchanged"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// Manual report quotas are charged at admission: the hourly per-principal quota
    /// refuses with a retry hint; a replay of an admitted key is never re-charged;
    /// `enrichment_start` resolves a report to its period and is not_found otherwise.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn manual_reports_are_quota_bound_and_enrichment_resolves_reports() {
        let (base, storage, service, live) = contended_service();
        let tenant = live.config.service.tenant.clone();
        let p = principal("airs-cli");
        let create = || -> ReportsCreateParams {
            serde_json::from_value(json!({"range": {"since": "2026-11-05T00:00:00Z", "until": "2026-11-06T00:00:00Z"}})).unwrap()
        };
        let per_hour = live.config.limits.manual_reports_per_principal_hour;
        for n in 0..per_hour {
            service
                .reports_create(
                    &p,
                    &live,
                    &tenant,
                    &format!("key-manual-{n:08}"),
                    "0a1b2c3d",
                    create(),
                )
                .await
                .unwrap();
        }
        let refused = service
            .reports_create(
                &p,
                &live,
                &tenant,
                "key-manual-overflow",
                "0a1b2c3d",
                create(),
            )
            .await
            .unwrap_err();
        assert_eq!(refused.code, ErrorCode::QuotaExhausted);
        assert!(refused.retry_after_seconds.is_some());
        assert!(
            service
                .reports_create(
                    &p,
                    &live,
                    &tenant,
                    "key-manual-00000000",
                    "0a1b2c3d",
                    create()
                )
                .await
                .is_ok(),
            "a replay is not re-charged"
        );
        // Non-hour ranges are refused.
        let odd: ReportsCreateParams = serde_json::from_value(
            json!({"range": {"since": "2026-11-05T00:30:00Z", "until": "2026-11-06T00:00:00Z"}}),
        )
        .unwrap();
        assert_eq!(
            service
                .reports_create(&p, &live, &tenant, "key-manual-odd00001", "0a1b2c3d", odd)
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );
        // Enrichment by report.
        let missing: EnrichmentParams =
            serde_json::from_value(json!({"report_id": crate::reports::new_id("r1")})).unwrap();
        assert_eq!(
            service
                .enrichment_start(
                    &p,
                    &live,
                    &tenant,
                    "key-enrich-00000001",
                    "0a1b2c3d",
                    missing
                )
                .await
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        let (_, row) = publish_fixture(&storage, &tenant);
        let found: EnrichmentParams =
            serde_json::from_value(json!({"report_id": row.report_id, "max_reports": 3})).unwrap();
        let job = service
            .enrichment_start(&p, &live, &tenant, "key-enrich-00000002", "0a1b2c3d", found)
            .await
            .unwrap();
        let id = job["job"]["job_id"].as_str().unwrap().to_owned();
        let stored = storage.control(|c| c.params(&id).unwrap());
        assert_eq!(
            (
                stored["since_ms"].as_i64(),
                stored["until_ms"].as_i64(),
                stored["max_reports"].as_u64()
            ),
            (Some(1), Some(2), Some(3))
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M6 review 3: a manual report's ID is scoped by the key's complete idempotency
    /// scope, epoch included. A same-epoch replay is the same report and job; the same key
    /// text under a later epoch is a different report; the old epoch's key is refused;
    /// an existing report row with other metadata is never silently kept.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn manual_report_ids_are_scoped_by_epoch() {
        let (base, storage, service, live) = contended_service();
        let tenant = live.config.service.tenant.clone();
        let p = principal("airs-cli");
        let range = |day: &str, next: &str| -> ReportsCreateParams {
            serde_json::from_value(json!({"range": {"since": format!("2026-11-{day}T00:00:00Z"), "until": format!("2026-11-{next}T00:00:00Z")}}))
                .unwrap()
        };
        let key = "key-manual-epoch001";
        let first = service
            .reports_create(&p, &live, &tenant, key, "0a1b2c3d", range("03", "04"))
            .await
            .unwrap();
        let replay = service
            .reports_create(&p, &live, &tenant, key, "0a1b2c3d", range("03", "04"))
            .await
            .unwrap();
        assert_eq!(
            first["job"]["job_id"], replay["job"]["job_id"],
            "same-epoch replay"
        );
        let report_of = |job: &Value| {
            let id = job["job"]["job_id"].as_str().unwrap().to_owned();
            storage.control(|c| c.params(&id).unwrap())["report_id"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let old_report = report_of(&first);
        storage.control(|c| {
            c.conn
                .execute(
                    "INSERT INTO epochs VALUES ('1b2c3d4e', 1, NULL, 'restore', zeroblob(32), 'pending_activation')",
                    [],
                )
                .unwrap();
            c.activate_epoch("1b2c3d4e", 2).unwrap();
        });
        assert_eq!(
            service
                .reports_create(&p, &live, &tenant, key, "0a1b2c3d", range("03", "04"))
                .await
                .unwrap_err()
                .code,
            ErrorCode::ControlRestored,
            "the old epoch's key is refused"
        );
        let later = service
            .reports_create(&p, &live, &tenant, key, "1b2c3d4e", range("04", "05"))
            .await
            .unwrap();
        let new_report = report_of(&later);
        assert_ne!(
            new_report, old_report,
            "a new epoch's key names a new report"
        );
        // Both reports' rows initialised (as their executors do): enrichment of the new
        // epoch's report resolves its own period, November 4, not November 3.
        let day =
            |d: &str| airs_runtime_core::dto::utc_ms(&format!("2026-11-{d}T00:00:00Z")).unwrap();
        storage.control(|c| {
            c.ensure_report(
                &tenant,
                &old_report,
                "manual",
                (day("03"), day("04")),
                &[],
                "",
                None,
                3,
            )
            .unwrap();
            c.ensure_report(
                &tenant,
                &new_report,
                "manual",
                (day("04"), day("05")),
                &[],
                "",
                None,
                3,
            )
            .unwrap();
        });
        let enrich: EnrichmentParams =
            serde_json::from_value(json!({"report_id": new_report})).unwrap();
        let job = service
            .enrichment_start(
                &p,
                &live,
                &tenant,
                "key-enrich-epoch001",
                "1b2c3d4e",
                enrich,
            )
            .await
            .unwrap();
        let id = job["job"]["job_id"].as_str().unwrap().to_owned();
        let stored = storage.control(|c| c.params(&id).unwrap());
        assert_eq!(
            (stored["since_ms"].as_i64(), stored["until_ms"].as_i64()),
            (Some(day("04")), Some(day("05"))),
            "enrichment resolves the new epoch's report period"
        );
        // An existing report row describing another report is refused, never kept.
        storage.control(|c| {
            assert_eq!(
                c.ensure_report(
                    &tenant,
                    &old_report,
                    "manual",
                    (1, 7_200_001),
                    &[],
                    "",
                    None,
                    4
                )
                .unwrap_err()
                .code,
                ErrorCode::Conflict
            );
        });
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M6 review 3 minor: the agent-summary ceiling is in bytes, cut at a character
    /// boundary, for generation and recovery alike.
    #[test]
    fn the_summary_ceiling_counts_bytes_at_a_character_boundary() {
        let long = "é".repeat(crate::reports::SUMMARY_MAX);
        let cut = crate::reports::truncate_utf8(long, crate::reports::SUMMARY_MAX);
        assert!(cut.len() <= crate::reports::SUMMARY_MAX);
        assert_eq!(
            cut.len(),
            crate::reports::SUMMARY_MAX,
            "2-byte characters fill it exactly"
        );
        let three = "€".repeat(crate::reports::SUMMARY_MAX);
        let cut = crate::reports::truncate_utf8(three, crate::reports::SUMMARY_MAX);
        assert!(
            cut.len() <= crate::reports::SUMMARY_MAX && cut.len() > crate::reports::SUMMARY_MAX - 3
        );
        assert!(cut.chars().all(|c| c == '€'), "never splits a character");
    }

    #[test]
    fn cursors_bind_their_scope_and_refuse_tampering() {
        let key = [7u8; 32];
        let body = CursorBody {
            d: "scope-a".into(),
            s: "snap".into(),
            p: 50,
        };
        let text = encode_cursor(&key, &body);
        assert!(text.starts_with("c1."));
        let decoded = decode_cursor(&key, &text, "scope-a").unwrap();
        assert_eq!((decoded.s.as_str(), decoded.p), ("snap", 50));
        assert_eq!(
            decode_cursor(&key, &text, "scope-b").err(),
            Some(ErrorCode::InvalidRequest),
            "another principal, tool or parameter set"
        );
        assert_eq!(
            decode_cursor(&[8u8; 32], &text, "scope-a").err(),
            Some(ErrorCode::InvalidRequest),
            "another epoch's key"
        );
        let mut forged = text.clone().into_bytes();
        let i = forged.len() / 2;
        forged[i] = if forged[i] == b'A' { b'B' } else { b'A' };
        assert_eq!(
            decode_cursor(&key, std::str::from_utf8(&forged).unwrap(), "scope-a").err(),
            Some(ErrorCode::InvalidRequest)
        );
        for bad in [
            "c2.AAAAAAAAAAAAAAAAAAAA",
            "c1.short",
            "c1.!!!!!!!!!!!!!!!!!!!!",
        ] {
            assert!(decode_cursor(&key, bad, "scope-a").is_err(), "{bad}");
        }
    }

    fn upload_setup(
        bytes: &[u8],
    ) -> (PathBuf, Arc<Storage>, Service, Arc<Live>, Principal, String) {
        use crate::authority_store::{Attempt, Proof};
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let base = std::env::temp_dir().join(format!("upload-quota-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .unwrap();
        // Disposable synthetic credential files only, never external configuration.
        let mut text = include_str!("../config/example.toml").to_owned();
        for (name, value) in [
            ("mgmt-client-id", "synthetic-client-id"),
            ("mgmt-client-secret", "PUBLIC-SYNTHETIC-MGMT-SECRET"),
            ("scanner-monitor", "public-synthetic-scanner"),
        ] {
            let file = base.join(name);
            std::fs::write(&file, value).unwrap();
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
            text = text.replace(
                &format!("/run/airs-mcp/secrets/{name}"),
                file.to_str().unwrap(),
            );
        }
        let live = Live::new(crate::run::parse_config(text.as_bytes()).unwrap()).unwrap();
        let t = live.config.service.tenant.clone();
        let now = airs_runtime_core::time::now_ms();
        let mut c = Control::open(&base, &t, "0a1b2c3d", &[1; 32], now).unwrap();
        let p = Principal::for_tests("synthetic-admin", "synthetic-client");
        let actor = Service::authority_actor(&p);
        let declared = json!({"action":"upload_admit","upload":{"kind":"logical_transfer","total_bytes":bytes.len(),"sha256":hex(&Sha256::digest(bytes)),"expect_tsg":cloud(&live.config).unwrap().tsg,"expect_environment":cloud(&live.config).unwrap().environment_id}});
        let preview = c
            .preview(&t, &actor, "upload_admit", &declared, 0, now)
            .unwrap();
        let proof = Proof {
            nonce: preview.nonce.clone(),
            auth_time: (now / 1000) as u64,
        };
        let a = Attempt {
            tenant: &t,
            actor: &actor,
            preview_id: &preview.preview_id,
            digest: &preview.digest,
            generation: 0,
            key: "key-upload-quota-01",
            expected_epoch: "0a1b2c3d",
            config_revision: &live.config.revision,
            queue_tenant: 10,
            queue_principal: 10,
        };
        let id = c
            .upload_begin(&a, Some(&proof), 300, bytes.len() as u64, now)
            .unwrap();
        let admitted = crate::storage::Admitted {
            path: base.clone(),
            filesystem: "synthetic",
            uid: rustix::process::geteuid().as_raw(),
        };
        let owner = crate::storage::OwnerRecord {
            owner_id: "synthetic".into(),
            pod: "synthetic".into(),
            node: "synthetic".into(),
            pid: 0,
            acquired_at_unix: 0,
        };
        let storage = Arc::new(Storage::new(admitted, owner, base.join("history"), c));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(base.join("staging"))
            .unwrap();
        let service = Service {
            storage: Some(storage.clone()),
        };
        (base, storage, service, live, p, id)
    }

    /// M5 review 2 finding 6 (the reviewer's probe): a consumed upload keeps reserving its
    /// staging while its job is queued and its bytes remain; once the job ended (here it
    /// failed), the sweep removes the file and only then releases the quota.
    #[tokio::test]
    async fn a_consumed_upload_keeps_its_quota_until_its_staging_is_removed() {
        use crate::authority_store::{Attempt, Proof};
        let bytes = b"synthetic staging for a terminal lifecycle job";
        let (base, storage, service, live, p, id) = upload_setup(bytes);
        let t = live.config.service.tenant.clone();
        let path = crate::lifecycle::upload_path(&base, &id);
        std::fs::write(&path, []).unwrap();
        service
            .upload_chunk(
                &p,
                &t,
                UploadChunkParams {
                    upload_id: id.clone(),
                    offset: 0,
                    data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                    chunk_sha256: hex(&Sha256::digest(bytes)),
                },
            )
            .await
            .unwrap();
        let actor = Service::authority_actor(&p);
        let now = airs_runtime_core::time::now_ms();
        let job = storage.control(|c| {
            c.upload_finalize(&t, &actor, &id, &hex(&Sha256::digest(bytes)), now)
                .unwrap();
            let params = json!({"action":"import_finalize","upload_id":id});
            let preview = c.preview_import(&t, &actor, &id, &params, 0, now).unwrap();
            let proof = Proof {
                nonce: preview.nonce.clone(),
                auth_time: (now / 1000) as u64,
            };
            let a = Attempt {
                tenant: &t,
                actor: &actor,
                preview_id: &preview.preview_id,
                digest: &preview.digest,
                generation: 0,
                key: "key-upload-quota-02",
                expected_epoch: "0a1b2c3d",
                config_revision: &live.config.revision,
                queue_tenant: 10,
                queue_principal: 10,
            };
            c.execute_direct(&a, Some(&proof), 300, now).unwrap()
        });
        // The quota code's own accounting (the same function upload begin consults).
        let reserved = |storage: &Storage| -> i64 {
            storage.control(|c| {
                c.staging_reserved(&t, airs_runtime_core::time::now_ms())
                    .unwrap()
            })
        };
        // Queued: reserved, and the sweep keeps the file even after the upload's expiry.
        storage.control(|c| {
            c.conn
                .execute("UPDATE upload_sessions SET expires_ms=0", [])
                .unwrap()
        });
        assert_eq!(reserved(&storage), bytes.len() as i64);
        service.sweep_uploads(&storage, &t).await;
        assert!(path.exists(), "a queued job's staging is kept");
        // The job ends (this deliberately invalid fixture fails its import).
        let exec = Arc::new(crate::lifecycle::Lifecycle {
            shared: crate::run::Shared::new(live.clone()),
        });
        let worker = crate::worker::Worker {
            storage: storage.clone(),
            owner: "quota-test".into(),
            executors: [(
                "lifecycle_execute".into(),
                exec as Arc<dyn crate::worker::Executor>,
            )]
            .into_iter()
            .collect(),
        };
        worker.step(&t, |_| true).unwrap();
        let state = storage.control(|c| c.job(&t, &job).unwrap().unwrap().state);
        assert!(state.terminal(), "{state:?}");
        assert_eq!(
            reserved(&storage),
            bytes.len() as i64,
            "still reserved: the bytes remain until removed"
        );
        // The sweep (also run by the worker on every pass) removes it, then releases it.
        crate::tools::sweep_staging(&storage, &t);
        assert!(!path.exists(), "the ended job's staging is removed");
        assert_eq!(
            reserved(&storage),
            0,
            "released only after verified removal"
        );
        let state: String = storage.control(|c| {
            c.conn
                .query_row(
                    "SELECT state FROM upload_sessions WHERE upload_id=?1",
                    [&id],
                    |r| r.get(0),
                )
                .unwrap()
        });
        assert_eq!(state, "removed");
        assert!(
            service
                .upload_abort(&p, &t, UploadIdParams { upload_id: id })
                .await
                .is_err(),
            "a consumed upload is never aborted"
        );
        std::fs::remove_dir_all(base).unwrap();
    }
}

/// `receipts_ingest` parameters (frozen schema `receipts_ingest.params`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptsParams {
    cloud_id: String,
    records: Vec<ReceiptRecord>,
}

/// One normalized client-asserted receipt (frozen `$defs/receipt`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptRecord {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    application_id: Option<String>,
    #[serde(default)]
    application_name: Option<String>,
    #[serde(default)]
    category: Option<String>,
    declared_scanner: String,
    identity_withheld: bool,
    receipt_key: String,
    received_at: String,
    #[serde(default)]
    report_id: Option<String>,
    scan_id: String,
    #[serde(default)]
    session_id: Option<String>,
    status: String,
    #[serde(default)]
    sub_request: Option<u64>,
}

impl ReceiptRecord {
    /// The frozen schema's field rules, checked before any effect.
    fn well_formed(&self) -> bool {
        let charset = |s: &str, extra: &str| {
            s.chars()
                .all(|c| c.is_ascii_alphanumeric() || extra.contains(c))
        };
        let len = |s: &Option<String>, n: usize| s.as_ref().is_none_or(|v| v.len() <= n);
        (8..=200).contains(&self.receipt_key.len())
            && charset(&self.receipt_key, "_.:-")
            && (1..=64).contains(&self.declared_scanner.len())
            && self
                .declared_scanner
                .starts_with(|c: char| c.is_ascii_alphanumeric())
            && charset(&self.declared_scanner, "_-")
            && uuid::Uuid::parse_str(&self.scan_id)
                .is_ok_and(|u| u.hyphenated().to_string() == self.scan_id)
            && dto::utc_ms(&self.received_at).is_some()
            && ["complete", "pending", "failed", "timeout"].contains(&self.status.as_str())
            && self
                .action
                .as_deref()
                .is_none_or(|a| ["allow", "block", "unknown"].contains(&a))
            && self
                .category
                .as_deref()
                .is_none_or(|c| ["benign", "malicious", "unknown"].contains(&c))
            && len(&self.application_id, 4096)
            && len(&self.application_name, 4096)
            && len(&self.session_id, 4096)
            && len(&self.report_id, 256)
    }

    /// The archive's received-metadata record for this receipt under `scanner`. The
    /// application comes only from the configured scanner (withheld when the client
    /// marks its identity withheld or names another application); a client application
    /// name is never stored; a failed or timed-out scan is a `failed` action; nothing
    /// unknown becomes benign.
    fn received(
        &self,
        scanner: &crate::config::Scanner,
    ) -> airs_history_store::repository::Received {
        use airs_runtime_core::signal::evidence::{Event, Identity};
        let attributed = !self.identity_withheld
            && self
                .application_id
                .as_deref()
                .is_none_or(|a| a == scanner.application_id);
        let action = match self.status.as_str() {
            "failed" | "timeout" => "failed".to_owned(),
            _ => self.action.clone().unwrap_or_else(|| "unknown".into()),
        };
        airs_history_store::repository::Received {
            receipt_key: self.receipt_key.clone(),
            batch_report_id: None,
            event: Event {
                identity: Identity {
                    app_id: if attributed {
                        scanner.application_id.clone()
                    } else {
                        String::new()
                    },
                    app_name: String::new(),
                    session_id: self.session_id.clone().unwrap_or_default(),
                    scan_id: self.scan_id.clone(),
                    sub_request: self.sub_request,
                },
                timestamp_ms: None,
                profile: None,
                verdict: self.category.clone().unwrap_or_else(|| "unknown".into()),
                action,
                report_ids: self.report_id.iter().cloned().collect(),
                detections: vec![],
                detector_hints: vec![],
                sources: vec!["client_asserted".into()],
                notes: vec![],
            },
            source: "client_asserted",
            endpoint: scanner.endpoint.trim_end_matches('/').to_owned(),
            reference: attributed.then(|| airs_runtime_core::tenant::scanners::ScannerReference {
                name: scanner.name.clone(),
                revision: scanner.revision.clone(),
                application_id: scanner.application_id.clone(),
                endpoint: scanner.endpoint.trim_end_matches('/').to_owned(),
            }),
        }
    }
}

/// `schedules_set` parameters (frozen schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduleSetParams {
    kind: String,
    enabled: bool,
    effective_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    interval_minutes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revision_allowance: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revision_interval_hours: Option<i64>,
}

/// `schedules_disable` parameters (frozen schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduleDisableParams {
    schedule_id: String,
    effective_at: String,
}

/// `lifecycle_preview` parameters (frozen schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecyclePreviewParams {
    action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    allowance: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    app_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    artifact_version_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    backup_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    examples_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    upload: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    upload_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecycleExecuteParams {
    preview_id: String,
    parameter_digest: String,
    #[serde(default)]
    approval_id: Option<String>,
}

/// `admin_confirm` and `transfer_upload_begin` parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfirmParams {
    preview_id: String,
    parameter_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UploadChunkParams {
    upload_id: String,
    offset: u64,
    data_base64: String,
    chunk_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UploadIdParams {
    upload_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportReadParams {
    job_id: String,
    offset: u64,
    length: u64,
}

/// `reports_create` parameters (frozen schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportsCreateParams {
    range: dto::Range,
    #[serde(default)]
    app_ids: Option<Vec<String>>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportsListParams {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    page: Option<dto::Page>,
    #[serde(default)]
    range: Option<dto::Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportsGetParams {
    report_id: String,
    #[serde(default)]
    version_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactsReadParams {
    name: String,
    version_id: String,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactsDownloadParams {
    name: String,
    version_id: String,
    offset: u64,
    length: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssessmentParams {
    version_id: String,
    review_sha256: String,
    markdown: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reviewer_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnrichmentParams {
    report_id: String,
    #[serde(default)]
    max_reports: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryParams {
    range: dto::Range,
    #[serde(default)]
    app_ids: Option<Vec<String>>,
    #[serde(default)]
    group_by: Option<Vec<String>>,
    #[serde(default)]
    page: Option<dto::Page>,
}
