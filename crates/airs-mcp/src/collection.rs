//! The `collection_start` and enrichment executors (contract §6, plan M4.1–M4.4): one
//! archive collection run per job through `airs_history_store::sync::run`, the
//! collector the CLI used, with the service's AIRS tenant (secret files, `airs_tenant`),
//! the configured tick ceilings and zero Reports calls for polling. The worker thread
//! drives the async collector on a private current-thread runtime.
//!
//! - **C4.** The archive run ID is checkpointed before any archive write or send. A
//!   resumed job looks its run up: no archive row → collected under that ID; a run left
//!   `running` → closed `interrupted`, the job `partial`; a finished run → finished from
//!   its durable record (`succeeded` only when complete, with nothing unconfirmed); a run
//!   that ended in a credential denial → a new run, so a job parked in
//!   `needs_credentials` really collects after a rotation.
//! - **Completeness.** A run is `succeeded` only when its durable record is complete:
//!   state `complete`, no unfinished required source, no unconfirmed read.
//! - **Selector.** An ordinary recent tick starts at the polled sources' oldest last
//!   valid traversal minus the 15-minute overlap (at least one hour), not a fresh 24 h.
//! - **Credentials.** The collector makes exactly one explicit fresh-token replay of a
//!   401 (never a 403; SDK retries stay zero). Outcomes are classified from the durable
//!   journal: API reads all refused (401/403), or the token endpoint refusing the
//!   credentials → a recorded denial (`needs_credentials`, never retried for that
//!   revision); a transient token failure (429/5xx/transport) with no API data →
//!   `waiting_retry`; an API 429 holds the plane.
//! - **Cancellation.** A watcher cancels the collector's session as soon as the job is
//!   `cancel_requested`, so no further send is admitted.
//! - **Accounting (C2).** One control reservation per run, settled `sent` once the run
//!   reached the archive and `uncertain` whenever that cannot be established; the
//!   journal's per-read rows are the classified send record.

use std::sync::Arc;

use serde_json::{Value, json};

use airs_runtime_core::dto::{CollectionMode, ErrorCode};
use airs_runtime_core::error::{CliError, SecretSet};
use airs_runtime_core::scan_logs::{CollectionOptions, Limits};

use crate::control::SendOutcome;
use crate::run::Shared;
use crate::worker::{Executor, JobContext, Outcome};

/// The phase checkpointed (with the run ID) before the collector starts.
pub const COLLECTING: &str = "collecting";

/// Contract §6 tick defaults: 120 s, 200 sends including OAuth, 40 pages per source
/// (the configured `[limits]` values apply; these are the defaults and concurrency).
pub const TICK_LIMITS: Limits = Limits {
    seconds: 120,
    concurrency: 4,
    requests: 200,
};
pub const TICK_PAGES: u64 = 40;
/// How long a management 429 holds the plane (no `Retry-After` reaches the journal).
pub const MANAGEMENT_429_HOLD_MS: i64 = 15 * 60_000;
/// Bounded event rows per run (the collector's existing bound).
pub const TICK_EVENTS: u64 = 10_000;

pub struct Collection {
    pub shared: Arc<Shared>,
}

/// The canonical parameters `collection_start` stored (tools.rs canonical form).
#[derive(serde::Deserialize)]
struct Stored {
    mode: CollectionMode,
    range: Option<StoredRange>,
    #[serde(default)]
    app_ids: Vec<String>,
}

#[derive(serde::Deserialize)]
struct StoredRange {
    since_ms: i64,
    until_ms: i64,
}

fn failure(operation: &str, code: ErrorCode, reason: &str, committed: u64) -> Value {
    json!({
        "operation": operation,
        "committed_facts": committed,
        "uncertain_sends": 0,
        "error": {"schema_version": 1, "code": code, "reason": reason},
    })
}

/// The frozen `result_collection` for one run.
fn result(
    s: &airs_history_store::run_summary::RunSummary,
    sources: Vec<Value>,
    unfinished: usize,
) -> Value {
    json!({
        "operation": "collection_start",
        "new_facts": s.new_facts,
        "duplicate_facts": s.duplicate_facts,
        "sends": {
            "management": s.management_sends.min(1000),
            "oauth": s.oauth_sends.min(1000),
            "reports": 0,
            "results": s.results_sends.min(1000),
        },
        "sources": sources,
        "unfinished_sources": unfinished.min(100),
    })
}

/// A credential denial reaching the executor as an error (a 401/403 after the replay).
fn denied(error: &CliError) -> bool {
    matches!(error, CliError::Operational(f) if matches!(f.status, Some(401 | 403)))
}

/// What one run of the collector is asked to do. The scope (`range`, `target`,
/// `apps`) is resolved once and checkpointed, so every recovery and continuation of the
/// job uses exactly the original scope.
#[derive(Clone)]
struct Plan {
    operation: &'static str,
    target: (i64, i64),
    apps: Vec<String>,
    backfill: bool,
    max_reports: u64,
    /// Durable completeness (partial receipts, incomplete revisits): collection runs;
    /// an enrichment run is judged by its examples.
    strict: bool,
}

/// The durable job scope and history carried in every checkpoint: the resolved scope
/// and every archive run the logical job has already made.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct Scope {
    range: Option<(i64, i64)>,
    target: (i64, i64),
    apps: Vec<String>,
    #[serde(default)]
    runs: Vec<String>,
}

/// Why a job will continue after waiting (checkpoint phase [`CONTINUE`]).
pub const CONTINUE: &str = "continue";

/// How a run ended, classified from its durable record.
enum Ended {
    Canceled,
    NeedsCredentials,
    Retry(i64),
    Done(Option<airs_history_store::run_summary::RunSummary>, bool),
    Failed(Option<airs_history_store::run_summary::RunSummary>),
}

fn summary(
    ctx: &JobContext<'_>,
    cloud: &airs_history_store::identity::Cloud,
    run_id: &str,
) -> Result<Option<airs_history_store::run_summary::RunSummary>, ()> {
    airs_history_store::run_summary::run_summary(
        ctx.history_root(),
        cloud,
        run_id,
        &scm::Session::new(),
    )
    .map_err(|_| ())
}

/// Coverage of the run's target for the result (≤ 100 groups).
fn sources(
    ctx: &JobContext<'_>,
    cloud: &airs_history_store::identity::Cloud,
    target: (i64, i64),
    apps: &[String],
) -> Vec<Value> {
    airs_history_store::inspect::coverage_groups(
        ctx.history_root(),
        cloud,
        target.0,
        target.1,
        apps,
        &scm::Session::new(),
    )
    .ok()
    .flatten()
    .unwrap_or_default()
    .iter()
    .take(100)
    .map(|g| {
        json!({
            "source": g.source,
            "application_id": g.application_id,
            "state": g.state,
            "traversed_as_of": g.traversed_as_of_ms.map(airs_runtime_core::time::iso),
            "gap_seconds": (g.gap_ms.max(0) + 999) / 1000,
        })
    })
    .collect()
}

/// A durable record is complete only when the run completed every required source and
/// confirmed every read.
fn complete(s: &airs_history_store::run_summary::RunSummary) -> bool {
    s.state == "complete" && s.unfinished_sources == 0 && s.unconfirmed_reads == 0
}

/// A run that ended in a credential denial: any terminal API refusal (a 403, or a 401
/// whose single replay was exhausted), whatever other reads returned; any definitive
/// token refusal (an OAuth 401/403 the run never followed with a token); or every API
/// read refused; or the token endpoint refusing the credentials with no API data.
fn credential_denied(s: &airs_history_store::run_summary::RunSummary) -> bool {
    s.terminal_denied > 0
        || s.token_terminal_denied > 0
        || (s.returned == 0 && (s.auth_denied > 0 || s.token_denied > 0))
}

/// What one archive run consumed of the logical job's traffic: API and OAuth sends,
/// Results reads, Reports transports and every unconfirmed read (never proof of no
/// send). Reports are HTTP sends within the same logical job ceiling.
fn consumed(s: &airs_history_store::run_summary::RunSummary) -> u64 {
    s.management_sends + s.oauth_sends + s.results_sends + s.reports_sends + s.unconfirmed_reads
}

/// The job's Reports consumption may be known better to the control store than to the
/// archive: every Reports transport takes a durable reservation at its admission
/// (settled `uncertain`, never released), even when the archive journal never recorded
/// it. The job's total therefore counts the larger of the two.
fn with_reserved_reports(archive_total: u64, archive_reports: u64, reserved_reports: u64) -> u64 {
    archive_total - archive_reports.min(archive_total) + archive_reports.max(reserved_reports)
}

/// The logical job's cumulative consumption over every run it made. A run whose record
/// cannot be read counts as the whole allowance (unknown consumption is spent).
fn job_consumption(
    ctx: &JobContext<'_>,
    cloud: &airs_history_store::identity::Cloud,
    runs: &[String],
    allowance: u64,
) -> u64 {
    let (mut total, mut reports) = (0u64, 0u64);
    for run in runs {
        match summary(ctx, cloud, run) {
            Ok(Some(s)) => {
                total += consumed(&s);
                reports += s.reports_sends;
            }
            Ok(None) => {}
            Err(()) => return allowance,
        }
    }
    let reserved = ctx
        .storage()
        .control(|c| c.plane_sends(&ctx.job.job_id, "reports"))
        .unwrap_or(allowance);
    with_reserved_reports(total, reports, reserved)
}

/// The watcher's poll interval in milliseconds (a test may lengthen it: admission is
/// enforced at every send regardless).
pub static WATCHER_POLL_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(100);

/// Drive one collector run for `plan` under the job: checks, reservation, checkpoint
/// (with the job's scope and run history), the run itself with its admission gate and
/// cancellation watcher, and the durable classification. `requests` is what remains of
/// the logical job's traffic allowance.
#[allow(clippy::too_many_arguments)]
fn drive(
    ctx: &JobContext<'_>,
    config: &crate::config::Config,
    cloud: &airs_history_store::identity::Cloud,
    plan: &Plan,
    scope: &Scope,
    resumed: Option<String>,
    requests: u64,
    reports_gate: Option<airs_runtime_core::read_budget::ReportsGate>,
) -> (String, Ended) {
    let revision = config.airs.management_revision.clone();
    let pending = resumed.clone().unwrap_or_default();
    // A cooldown (a 429, or a recovery hold) is respected before any send. The archive
    // journal is its authority (§6, §10): the hold is derived from it at every admission.
    match management_hold(&ctx.storage(), &ctx.job.tenant, cloud) {
        Ok(Some(until)) => {
            return (
                pending,
                Ended::Retry((until - airs_runtime_core::time::now_ms()).max(1_000)),
            );
        }
        Ok(None) => {}
        Err(_) => return (pending, Ended::Retry(60_000)),
    }
    // A revision already denied is never tried again (§10): no token request, no read.
    // The archive journal is the authority (§6.1); an unreadable record sends nothing.
    match revision_blocked(&ctx.storage(), &ctx.job.tenant, cloud, &revision) {
        Ok(false) => {}
        Ok(true) => return (pending, Ended::NeedsCredentials),
        Err(Unavailable) => return (pending, Ended::Retry(60_000)),
    }
    let secrets = SecretSet::default();
    let Ok(tenant) = crate::airs_tenant::tenant(config, &secrets) else {
        return (pending, Ended::NeedsCredentials);
    };
    let Ok(transport) = ctx.reserve_send(plan.operation, "management", Some(&revision)) else {
        return (pending, Ended::Retry(60_000));
    };
    let run_id = resumed.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut state = serde_json::to_value(scope).unwrap_or_default();
    state["run_id"] = json!(run_id);
    // The run is durably bound to the credential revision it uses, before any send: a
    // recovery reconciles a denial against exactly this revision.
    state["credential_revision"] = json!(revision);
    match ctx.checkpoint(COLLECTING, &state, Some(&run_id)) {
        Ok(false) => {}
        Ok(true) => {
            let _ = ctx.settle(&transport, SendOutcome::NotSent);
            return (run_id, Ended::Canceled);
        }
        Err(_) => {
            let _ = ctx.settle(&transport, SendOutcome::NotSent);
            return (run_id, Ended::Retry(60_000));
        }
    }
    let canceled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // Admission: before every send, the durable job state is checked, so a cancellation
    // committed between two sends (even between watcher polls) stops the next one.
    let gate: airs_runtime_core::read_budget::AdmissionGate = {
        let storage = ctx.storage();
        let (tenant, job) = (ctx.job.tenant.clone(), ctx.job.job_id.clone());
        let canceled = canceled.clone();
        Arc::new(move || {
            let open = storage
                .control(|c| c.job(&tenant, &job))
                .ok()
                .flatten()
                .is_some_and(|j| j.state != crate::jobs::State::CancelRequested);
            if !open {
                canceled.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            open
        })
    };
    let request = airs_history_store::sync::SyncRequest {
        root: ctx.history_root().to_path_buf(),
        // Always the job's checkpointed, concrete target (first collection included), so
        // a recovery or continuation collects exactly the original window.
        range: Some(plan.target),
        options: CollectionOptions {
            limits: Limits {
                seconds: config.limits.tick_seconds,
                concurrency: TICK_LIMITS.concurrency,
                requests,
            },
            interval: 24,
            unit: "hours".into(),
            app_ids: plan.apps.clone(),
            max_pages: config.limits.tick_pages_per_source,
            max_events: TICK_EVENTS,
            max_reports: plan.max_reports,
        },
        backfill: plan.backfill,
        strict: plan.strict,
        run_id: Some(run_id.clone()),
        auth_replay: true,
        reports_gate,
        admission_gate: Some(gate),
        credential_revision: Some(revision.clone()),
        // The in-run fence (§6, §10): every later transport of this run is refused once
        // the journal holds a definitive credential refusal, and an API 429 commits the
        // service's 15-minute minimum as the archive cooldown with the 429 itself.
        send_fence: Some(airs_history_store::sync::SendFence {
            management_hold_floor_seconds: (MANAGEMENT_429_HOLD_MS / 1000) as u64,
            credential: true,
        }),
    };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        let _ = ctx.settle(&transport, SendOutcome::NotSent);
        return (run_id, Ended::Retry(60_000));
    };
    let session = scm::Session::new();
    let spans = Default::default();
    let collect = airs_runtime_core::signal::sink::CollectContext {
        session: &session,
        secrets: &secrets,
        payload_spans: &spans,
        version: airs_runtime_core::VERSION,
    };
    // The watcher interrupts in-flight work as soon as the job is `cancel_requested`.
    let finished = std::sync::atomic::AtomicBool::new(false);
    let report = std::thread::scope(|scope| {
        scope.spawn(|| {
            while !finished.load(std::sync::atomic::Ordering::SeqCst) {
                if ctx.cancel_requested() {
                    canceled.store(true, std::sync::atomic::Ordering::SeqCst);
                    session.cancel();
                    return;
                }
                let poll = WATCHER_POLL_MS.load(std::sync::atomic::Ordering::SeqCst);
                let mut waited = 0;
                while waited < poll && !finished.load(std::sync::atomic::Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(poll.min(20)));
                    waited += poll.min(20);
                }
            }
        });
        let report = runtime.block_on(airs_history_store::sync::run(&collect, &tenant, request));
        finished.store(true, std::sync::atomic::Ordering::SeqCst);
        report
    });
    let recorded = summary(ctx, cloud, &run_id);
    // C2: `sent` once the run is known to have reached the archive; when that cannot be
    // established the reservation stays consumed as `uncertain`, never released.
    let _ = ctx.settle(
        &transport,
        match &recorded {
            Ok(Some(_)) => SendOutcome::Sent,
            _ => SendOutcome::Uncertain,
        },
    );
    let s = recorded.ok().flatten();
    // The run's global outcomes (a revision denial, a 429 hold) are recorded before the
    // job's own fate: a cancellation keeps the job canceled, never the revision usable.
    let denial = s.as_ref().is_some_and(credential_denied);
    if denial {
        let _ = ctx.record_denial("management", &revision);
    }
    if let Some(sum) = &s {
        // An API 429 holds the plane, so the next tick waits instead of multiplying
        // traffic (contract §6, §10). The run's committed data is kept.
        // The control record is a persisted copy: a failed write never shortens the hold,
        // since every admission derives it from the journal again.
        if sum.rate_limited > 0
            && ctx
                .hold("management", MANAGEMENT_429_HOLD_MS, "429")
                .is_err()
        {
            crate::run::log(json!({"event": "management_hold_deferred"}));
        }
    }
    if canceled.load(std::sync::atomic::Ordering::SeqCst) {
        return (run_id, Ended::Canceled);
    }
    if denial {
        return (run_id, Ended::NeedsCredentials);
    }
    if s.as_ref()
        .is_some_and(|sum| sum.returned == 0 && sum.token_transient > 0)
    {
        return (run_id, Ended::Retry(TOKEN_RETRY_MS));
    }
    match report {
        Ok(report) => {
            let whole = !report.incomplete
                && report.incomplete_required_sources == 0
                && s.as_ref().is_some_and(complete);
            (run_id, Ended::Done(s, whole))
        }
        Err(f) if denied(&f.error) => {
            let _ = ctx.record_denial("management", &revision);
            (run_id, Ended::NeedsCredentials)
        }
        Err(_) => (run_id, Ended::Failed(s)),
    }
}

/// What a recovered run's durable record says must happen before another run.
enum Reconciled {
    /// Credential denial of the run's revision, still the configured one: park, no
    /// new attempt (the denial is recorded now if the crash preceded its record).
    Parked,
    /// Credential denial of an older revision: continue with a new run.
    Rotated,
    /// A transient token failure (the crash preceded the continue checkpoint): keep
    /// retry eligibility.
    Retry,
    /// Neither: the run is finished from its record.
    Finish,
}

/// Reconcile a checkpointed run's durable classification (contract §10) before any
/// other run is admitted, whatever the run's archive state (a run left `running` by a
/// process that died before finalizing it is reconciled too). `bound` is the revision
/// the run was bound to (absent in a pre-binding checkpoint: the configured one is
/// assumed, which never reopens a denial).
///
/// Everything the normal path persists after a run is reconstructed here from the
/// run's durable journal before any further work:
/// - a journaled 429 holds the plane until the durable refusal time plus the hold
///   (an existing longer hold is kept);
/// - a definitive denial (a 403, or a 401 whose replay was exhausted) is recorded
///   under the run's bound revision; if that record cannot be written the job still
///   parks (no further attempt), conservatively.
fn reconcile(
    ctx: &JobContext<'_>,
    cloud: &airs_history_store::identity::Cloud,
    run_id: &str,
    s: &airs_history_store::run_summary::RunSummary,
    bound: Option<&str>,
    current: &str,
) -> Reconciled {
    if let Some(at) = s.last_rate_limited_ms {
        let until = at + MANAGEMENT_429_HOLD_MS;
        if until > airs_runtime_core::time::now_ms()
            && ctx.hold_until("management", until, "429").is_err()
        {
            crate::run::log(json!({"event": "management_hold_deferred"}));
        }
    }
    if credential_denied(s) {
        let revision = bound.unwrap_or(current);
        let _ = ctx.record_denial("management", revision);
        if s.state == "running" {
            // The dead process never finalized this run: close it as interrupted.
            let _ = airs_history_store::run_summary::interrupt_run(
                ctx.history_root(),
                cloud,
                run_id,
                &scm::Session::new(),
            );
        }
        return if revision == current {
            Reconciled::Parked
        } else {
            Reconciled::Rotated
        };
    }
    // A transient token failure (OAuth 429, 5xx, transport) with no API data keeps the
    // job's retry eligibility, whether or not the dead process finalized the run: a run
    // left `running` is closed as interrupted, and the caller parks the job for a retry
    // with its scope, its runs (their consumption) and its original deadline.
    if s.returned == 0 && s.token_transient > 0 {
        if s.state == "running" {
            let _ = airs_history_store::run_summary::interrupt_run(
                ctx.history_root(),
                cloud,
                run_id,
                &scm::Session::new(),
            );
        }
        return Reconciled::Retry;
    }
    Reconciled::Finish
}

/// A control store or archive that cannot be read: the caller sends and resumes nothing.
#[derive(Debug)]
pub struct Unavailable;

/// The management hold at admission (contract §6, §10): the longer of the control
/// store's cooldown and the hold the archive derives from its own journal (the latest
/// API 429 plus [`MANAGEMENT_429_HOLD_MS`], or a longer provider cooldown). A journal
/// hold the control store lacks is copied into it (opportunistically: admission always
/// re-derives it). `Err` when either record cannot be read: the caller sends nothing.
pub fn management_hold(
    storage: &crate::tools::Storage,
    tenant: &str,
    cloud: &airs_history_store::identity::Cloud,
) -> Result<Option<i64>, Unavailable> {
    let now = airs_runtime_core::time::now_ms();
    let control = storage
        .control(|c| c.cooldown_until(tenant, "management", now))
        .map_err(|_| Unavailable)?;
    let archive = airs_history_store::run_summary::management_hold_until(
        &storage.history_root,
        cloud,
        MANAGEMENT_429_HOLD_MS,
        now,
        &scm::Session::new(),
    )
    .map_err(|_| Unavailable)?;
    if let Some(until) = archive.filter(|a| control.is_none_or(|c| *a > c)) {
        let _ = storage.control(|c| c.hold(tenant, "management", until, "429"));
    }
    Ok(control.max(archive))
}

/// Reconcile the tenant's `needs_credentials` jobs with the configured management
/// credential `revision` (contract §10): every job parked under another revision is
/// requeued, with its original deadline, scope and consumption, unless `revision` is
/// itself denied (the control record or the archive journal, §6.1). Run at every
/// worker pass (so startup with a new revision, and a resumption whose write failed,
/// are retried) and at a reload that changes the revision. Idempotent: a job parked
/// under the configured revision stays parked. When either record cannot be read
/// nothing resumes and `Err` is returned (admission would refuse the revision's sends
/// regardless). Returns how many jobs resumed.
pub fn credentials_rotated(
    storage: &crate::tools::Storage,
    tenant: &str,
    cloud: &airs_history_store::identity::Cloud,
    revision: &str,
    now_ms: i64,
) -> Result<u64, Unavailable> {
    if revision_blocked(storage, tenant, cloud, revision)? {
        return Ok(0);
    }
    storage
        .control(|c| c.credentials_restored(tenant, "management", revision, now_ms))
        .map_err(|_| Unavailable)
}

/// Whether the management credential `revision` is denied (contract §6.1, §10): the
/// control store records it, or the archive journal holds a definitive denial by a run
/// bound to it. The journal is the authority; the control record is its persisted
/// copy, written here when missing (opportunistically: a failed write never makes the
/// revision usable, since every admission reads the journal again). `Err` when either
/// record cannot be read: the caller sends nothing.
pub fn revision_blocked(
    storage: &crate::tools::Storage,
    tenant: &str,
    cloud: &airs_history_store::identity::Cloud,
    revision: &str,
) -> Result<bool, Unavailable> {
    if storage
        .control(|c| c.denied(tenant, "management", revision))
        .map_err(|_| Unavailable)?
    {
        return Ok(true);
    }
    let bound = storage
        .control(|c| {
            c.bound_runs(
                tenant,
                revision,
                airs_history_store::run_summary::RECENT_REVISION_RUNS,
            )
        })
        .map_err(|_| Unavailable)?;
    let denied = airs_history_store::run_summary::revision_denied(
        &storage.history_root,
        cloud,
        revision,
        &bound,
        &scm::Session::new(),
    )
    .map_err(|_| Unavailable)?;
    if denied {
        let now = airs_runtime_core::time::now_ms();
        let _ = storage.control(|c| c.record_denial(tenant, "management", revision, now));
    }
    Ok(denied)
}

/// How a transient token failure waits before the job is retried.
pub const TOKEN_RETRY_MS: i64 = 60_000;

/// Where a job resumes from its last checkpoint.
enum Resume {
    /// No checkpoint: a fresh job.
    Fresh,
    /// A run was checkpointed (C4): look it up, with the revision it was bound to.
    Run(String, Scope, Option<String>),
    /// The job waited (retry or credentials) and continues with a new run.
    Continue(Scope),
}

fn resume(ctx: &JobContext<'_>) -> Result<Resume, ()> {
    match ctx.resume_point().map_err(|_| ())? {
        Some((phase, state, Some(run))) if phase == COLLECTING => {
            let bound = state["credential_revision"].as_str().map(str::to_owned);
            let scope: Option<Scope> = serde_json::from_value(state).ok();
            Ok(Resume::Run(run, scope.unwrap_or_default(), bound))
        }
        Some((phase, state, _)) if phase == CONTINUE => Ok(serde_json::from_value(state)
            .map(Resume::Continue)
            .unwrap_or(Resume::Fresh)),
        _ => Ok(Resume::Fresh),
    }
}

/// Record that the job continues later (after a retry wait or a credential rotation),
/// with its scope and every run it has made.
fn park(ctx: &JobContext<'_>, scope: &Scope, run_id: &str, reason: &str) {
    let mut scope = scope.clone();
    if !run_id.is_empty() && !scope.runs.iter().any(|r| r == run_id) {
        scope.runs.push(run_id.to_owned());
    }
    let mut state = serde_json::to_value(&scope).unwrap_or_default();
    state["reason"] = json!(reason);
    let _ = ctx.checkpoint(CONTINUE, &state, None);
}

impl Executor for Collection {
    fn run(&self, ctx: &JobContext<'_>) -> Outcome {
        let live = self.shared.current();
        let config = &live.config;
        let Ok(stored) = ctx
            .params()
            .map_err(|_| ())
            .and_then(|v| serde_json::from_value::<Stored>(v).map_err(|_| ()))
        else {
            return Outcome::Failed(failure(
                "collection_start",
                ErrorCode::InvalidRequest,
                "stored parameters are invalid",
                0,
            ));
        };
        let Ok(cloud) = crate::tools::cloud(config) else {
            return Outcome::Failed(failure(
                "collection_start",
                ErrorCode::StorageUnavailable,
                "archive identity unavailable",
                0,
            ));
        };
        let Ok(resumed) = resume(ctx) else {
            return Outcome::RetryLater { after_ms: 60_000 };
        };
        // The scope is resolved once, at the job's first run, and then only recovered.
        let fresh_scope = || {
            let now = airs_runtime_core::time::now_ms();
            let explicit = stored.range.as_ref().map(|r| (r.since_ms, r.until_ms));
            let range = match (explicit, stored.mode) {
                (Some(r), _) => Some(r),
                (None, CollectionMode::Recent) => airs_history_store::inspect::polling_lower_bound(
                    ctx.history_root(),
                    &cloud,
                    &stored.app_ids,
                    now,
                    &scm::Session::new(),
                )
                .ok()
                .flatten()
                .map(|since| (since, now)),
                (None, CollectionMode::Backfill) => None,
            };
            Scope {
                range,
                target: range.unwrap_or((now - 86_400_000, now)),
                apps: stored.app_ids.clone(),
                runs: vec![],
            }
        };
        let (mut scope, mut resume_id, bound) = match &resumed {
            Resume::Fresh => (fresh_scope(), None, None),
            Resume::Run(run, s, b) if s.target != (0, 0) => {
                (s.clone(), Some(run.clone()), b.clone())
            }
            Resume::Run(run, _, b) => (fresh_scope(), Some(run.clone()), b.clone()),
            Resume::Continue(s) => (s.clone(), None, None),
        };
        let current = config.airs.management_revision.clone();
        let plan = Plan {
            operation: "collection",
            target: scope.target,
            apps: scope.apps.clone(),
            backfill: stored.mode == CollectionMode::Backfill,
            max_reports: 0,
            strict: true,
        };
        let build = |s: &airs_history_store::run_summary::RunSummary, unfinished: usize| {
            result(s, sources(ctx, &cloud, plan.target, &plan.apps), unfinished)
        };
        // C4 resume: a checkpointed run is looked up, never collected blind.
        if let Some(run_id) = resume_id.clone() {
            // Reconciled once: a denial is recorded under the run's bound revision.
            let looked = summary(ctx, &cloud, &run_id).map(|found| {
                found.map(|s| {
                    let verdict = reconcile(ctx, &cloud, &run_id, &s, bound.as_deref(), &current);
                    (s, verdict)
                })
            });
            match looked {
                Err(()) => return Outcome::RetryLater { after_ms: 60_000 },
                // No archive row: collected under the checkpointed ID.
                Ok(None) => {}
                Ok(Some((_, Reconciled::Parked))) => {
                    park(ctx, &scope, &run_id, "credentials");
                    return Outcome::NeedsCredentials {
                        revision: current.clone(),
                    };
                }
                Ok(Some((_, Reconciled::Retry))) => {
                    park(ctx, &scope, &run_id, "retry");
                    return Outcome::RetryLater {
                        after_ms: TOKEN_RETRY_MS,
                    };
                }
                // A rotation continues with a new run.
                Ok(Some((_, Reconciled::Rotated))) => {
                    if !scope.runs.contains(&run_id) {
                        scope.runs.push(run_id.clone());
                    }
                    resume_id = None;
                }
                Ok(Some((mut s, Reconciled::Finish))) => {
                    if s.state == "running" {
                        let _ = airs_history_store::run_summary::interrupt_run(
                            ctx.history_root(),
                            &cloud,
                            &run_id,
                            &scm::Session::new(),
                        );
                        s.state = "interrupted".into();
                    }
                    let unfinished =
                        (s.unfinished_sources as usize).max(usize::from(!complete(&s)));
                    let value = build(&s, unfinished);
                    return if complete(&s) {
                        Outcome::Succeeded(value)
                    } else if s.state == "failed" {
                        Outcome::Failed(failure(
                            "collection_start",
                            ErrorCode::UpstreamUnavailable,
                            "the collection failed; committed observations are kept",
                            s.new_facts,
                        ))
                    } else {
                        Outcome::Partial(value)
                    };
                }
            }
        }
        // The logical job's allowance spans every run it makes.
        let allowance = config.limits.tick_sends;
        let used = job_consumption(ctx, &cloud, &scope.runs, allowance);
        if used >= allowance {
            return Outcome::Failed(failure(
                "collection_start",
                ErrorCode::QuotaExhausted,
                "the job's traffic allowance is spent; committed observations are kept",
                0,
            ));
        }
        let (run_id, ended) = drive(
            ctx,
            config,
            &cloud,
            &plan,
            &scope,
            resume_id,
            allowance - used,
            None,
        );
        match ended {
            Ended::Canceled => Outcome::Canceled,
            Ended::NeedsCredentials => {
                park(ctx, &scope, &run_id, "credentials");
                Outcome::NeedsCredentials {
                    revision: current.clone(),
                }
            }
            Ended::Retry(ms) => {
                park(ctx, &scope, &run_id, "retry");
                Outcome::RetryLater { after_ms: ms }
            }
            Ended::Done(s, whole) => {
                let s = s.unwrap_or_default();
                let unfinished = (s.unfinished_sources as usize).max(usize::from(!whole));
                let value = build(&s, unfinished);
                if whole {
                    Outcome::Succeeded(value)
                } else {
                    Outcome::Partial(value)
                }
            }
            Ended::Failed(s) => Outcome::Failed(failure(
                "collection_start",
                ErrorCode::UpstreamUnavailable,
                "the collection failed; committed observations are kept",
                s.map_or(0, |s| s.new_facts),
            )),
        }
    }
}

/// The `enrichment` executor (contract §6 "Reports per job", plan M4.4): sparse
/// strategic examples for a period, through the same collector with Reports enabled.
/// Selection is the collector's deterministic one; cached examples are reused and never
/// fetched again; Reports are serial and stop on a 429.
///
/// Quotas: every Reports transport is charged, at its admission, to the tenant's
/// America/Chicago admission day of that moment (the durable, never-refunded allowance
/// with its high-water guard), with its own durable `reports` reservation; the logical
/// job never exceeds two Reports transports across all its runs. A day already spent
/// blocks the job by policy before any request. Recovery and continuation follow the
/// collection executor (checkpointed scope and run history, continuation after a retry
/// wait or a credential rotation, the checkpointed run ID kept when no archive row
/// exists).
pub struct Enrichment {
    pub shared: Arc<Shared>,
}

/// An enrichment job's canonical parameters.
#[derive(serde::Deserialize)]
struct EnrichParams {
    since_ms: i64,
    until_ms: i64,
    #[serde(default = "default_reports")]
    max_reports: u64,
}

fn default_reports() -> u64 {
    5
}

/// The per-job Reports transport bound (contract §6: two sends per job).
pub const JOB_REPORTS_SENDS: u64 = 2;

/// The frozen `result_enrichment`.
fn enrichment_result(s: &airs_history_store::run_summary::RunSummary) -> Value {
    json!({
        "operation": "enrichment_start",
        "selected_ids": s.selected_report_ids.min(10),
        "cached": s.cached_examples.min(10),
        "sends": s.reports_sends.min(2),
        "stopped_on_429": s.reports_rate_limited,
    })
}

impl Executor for Enrichment {
    fn run(&self, ctx: &JobContext<'_>) -> Outcome {
        let live = self.shared.current();
        let config = &live.config;
        let Ok(p) = ctx
            .params()
            .map_err(|_| ())
            .and_then(|v| serde_json::from_value::<EnrichParams>(v).map_err(|_| ()))
        else {
            return Outcome::Failed(failure(
                "enrichment_start",
                ErrorCode::InvalidRequest,
                "stored parameters are invalid",
                0,
            ));
        };
        let max_reports = p.max_reports.clamp(1, 10);
        let Ok(cloud) = crate::tools::cloud(config) else {
            return Outcome::Failed(failure(
                "enrichment_start",
                ErrorCode::StorageUnavailable,
                "archive identity unavailable",
                0,
            ));
        };
        let Ok(resumed) = resume(ctx) else {
            return Outcome::RetryLater { after_ms: 60_000 };
        };
        let fresh = Scope {
            range: Some((p.since_ms, p.until_ms)),
            target: (p.since_ms, p.until_ms),
            apps: vec![],
            runs: vec![],
        };
        let (mut scope, mut resume_id, bound) = match &resumed {
            Resume::Fresh => (fresh, None, None),
            Resume::Run(run, s, b) if s.target != (0, 0) => {
                (s.clone(), Some(run.clone()), b.clone())
            }
            Resume::Run(run, _, b) => (fresh, Some(run.clone()), b.clone()),
            Resume::Continue(s) => (s.clone(), None, None),
        };
        let current = config.airs.management_revision.clone();
        // C4: a checkpointed run is looked up. No archive row → the run is made under its
        // checkpointed ID; a credential-denied run → continued with a new run after the
        // rotation; otherwise finished from its durable record (success only if complete).
        if let Some(run_id) = resume_id.clone() {
            // Reconciled once: a denial is recorded under the run's bound revision.
            let looked = summary(ctx, &cloud, &run_id).map(|found| {
                found.map(|s| {
                    let verdict = reconcile(ctx, &cloud, &run_id, &s, bound.as_deref(), &current);
                    (s, verdict)
                })
            });
            match looked {
                Err(()) => return Outcome::RetryLater { after_ms: 60_000 },
                Ok(None) => {}
                Ok(Some((_, Reconciled::Parked))) => {
                    park(ctx, &scope, &run_id, "credentials");
                    return Outcome::NeedsCredentials {
                        revision: current.clone(),
                    };
                }
                Ok(Some((_, Reconciled::Retry))) => {
                    park(ctx, &scope, &run_id, "retry");
                    return Outcome::RetryLater {
                        after_ms: TOKEN_RETRY_MS,
                    };
                }
                // A rotation continues with a new run.
                Ok(Some((_, Reconciled::Rotated))) => {
                    if !scope.runs.contains(&run_id) {
                        scope.runs.push(run_id.clone());
                    }
                    resume_id = None;
                }
                Ok(Some((mut s, Reconciled::Finish))) => {
                    if s.state == "running" {
                        let _ = airs_history_store::run_summary::interrupt_run(
                            ctx.history_root(),
                            &cloud,
                            &run_id,
                            &scm::Session::new(),
                        );
                        s.state = "interrupted".into();
                    }
                    return match s.state.as_str() {
                        "complete" => Outcome::Succeeded(enrichment_result(&s)),
                        "failed" => Outcome::Failed(failure(
                            "enrichment_start",
                            ErrorCode::UpstreamUnavailable,
                            "the enrichment failed; committed examples are kept",
                            s.new_facts,
                        )),
                        _ => Outcome::Partial(enrichment_result(&s)),
                    };
                }
            }
        }
        // A day already spent sends nothing (checked before any request).
        let caps = (
            config.limits.reports_daily_sends,
            config.limits.reports_daily_ids,
        );
        let tenant = ctx.job.tenant.clone();
        match ctx
            .storage()
            .control(|c| c.reports_spent(&tenant, airs_runtime_core::time::now_ms()))
        {
            Ok((sends, ids)) if sends >= caps.0 || ids >= caps.1 => return Outcome::Blocked,
            Ok(_) => {}
            Err(_) => return Outcome::RetryLater { after_ms: 60_000 },
        }
        let allowance = config.limits.tick_sends;
        let used = job_consumption(ctx, &cloud, &scope.runs, allowance);
        if used >= allowance {
            return Outcome::Failed(failure(
                "enrichment_start",
                ErrorCode::QuotaExhausted,
                "the job's traffic allowance is spent; committed examples are kept",
                0,
            ));
        }
        // Every Reports transport is charged at its own admission, against the actual
        // admission day, with a durable per-job reservation (never refunded); the job's
        // Reports transports across all its runs stay within the per-job bound.
        let storage = ctx.storage();
        let job = ctx.job.job_id.clone();
        let revision = config.airs.management_revision.clone();
        let gate: airs_runtime_core::read_budget::ReportsGate = Arc::new(move |ids: u64| {
            let now = airs_runtime_core::time::now_ms();
            storage.control(|c| {
                if c.plane_sends(&job, "reports").unwrap_or(u64::MAX) >= JOB_REPORTS_SENDS {
                    return false;
                }
                if c.charge_reports(&tenant, now, (1, ids), caps).is_err() {
                    return false;
                }
                match c.reserve_send(&job, "enrichment", "reports", Some(&revision), now) {
                    Ok(t) => c.settle_send(&t, SendOutcome::Uncertain, now).is_ok(),
                    Err(_) => false,
                }
            })
        });
        let plan = Plan {
            operation: "enrichment",
            target: scope.target,
            apps: scope.apps.clone(),
            backfill: true,
            max_reports,
            strict: false,
        };
        let (run_id, ended) = drive(
            ctx,
            config,
            &cloud,
            &plan,
            &scope,
            resume_id,
            allowance - used,
            Some(gate),
        );
        match ended {
            Ended::Canceled => Outcome::Canceled,
            Ended::NeedsCredentials => {
                park(ctx, &scope, &run_id, "credentials");
                Outcome::NeedsCredentials {
                    revision: current.clone(),
                }
            }
            Ended::Retry(ms) => {
                park(ctx, &scope, &run_id, "retry");
                Outcome::RetryLater { after_ms: ms }
            }
            Ended::Done(Some(s), whole) => {
                if whole || s.state == "complete" {
                    Outcome::Succeeded(enrichment_result(&s))
                } else {
                    Outcome::Partial(enrichment_result(&s))
                }
            }
            Ended::Done(None, _) | Ended::Failed(None) => Outcome::Failed(failure(
                "enrichment_start",
                ErrorCode::StorageUnavailable,
                "the enrichment's record is unavailable; its sends stay consumed",
                0,
            )),
            Ended::Failed(Some(s)) => Outcome::Failed(failure(
                "enrichment_start",
                ErrorCode::UpstreamUnavailable,
                "the enrichment failed; committed examples are kept",
                s.new_facts,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M4 review 3 finding 4: Reports transports are HTTP sends within the logical job's
    /// ceiling (the reviewer's probe: 3 management + 2 OAuth + 1 Results + 2 Reports = 8).
    #[test]
    fn cumulative_traffic_includes_reports() {
        let prior = airs_history_store::run_summary::RunSummary {
            management_sends: 3,
            oauth_sends: 2,
            results_sends: 1,
            reports_sends: 2,
            ..Default::default()
        };
        assert_eq!(consumed(&prior), 8);
        // Durable Reports reservations the archive never journaled are still counted,
        // and are never counted twice.
        assert_eq!(with_reserved_reports(8, 2, 2), 8);
        assert_eq!(with_reserved_reports(8, 2, 3), 9);
        assert_eq!(with_reserved_reports(6, 0, 2), 8);
    }
}
