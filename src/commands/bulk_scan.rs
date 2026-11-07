//! `runtime bulk-scan` and `runtime resume-poll` (DESIGN §14): planning checks, job
//! setup and its clean-up, the location and tenant checks of a resume, the status
//! lines, the summary renderers and the interruption contexts.

use super::Invocation;
use crate::bulk::checkpoint::{Consequence, StateFile};
use crate::bulk::engine::{Engine, Job, ScannerApi, Timing};
use crate::bulk::lock::JobLock;
use crate::bulk::output::{self as csv, OutputFile, Rewritten};
use crate::bulk::state::{JobState, NewJob};
use crate::bulk::{self, MAX_BATCH_SIZE, StateDir};
use crate::bulk::{AttemptOutcome, ItemStatus};
use crate::cli::{BulkScanArgs, ResumePollArgs};
use crate::clients::{self, DEFAULT_SCAN_ENDPOINT};
use crate::error::{CliError, InterruptContext};
use crate::output::{Format, Output, terminal_safe};
use crate::render::pretty;
use crate::render::style::Style;
use crate::store::PROGRAM;
use crate::tenant::scanners::{self, ScannerReference};
use serde::Serialize;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The deprecated-alias warning of `--input` (TS text, DESIGN §14.1).
pub const INPUT_DEPRECATED: &str = "⚠ --input is deprecated and will be removed in v4 — use --file";

/// The interruption context before the first checkpoint (§14.16).
pub fn nothing_submitted() -> InterruptContext {
    InterruptContext::Bulk("no prompt was submitted".to_owned())
}

/// A validated `bulk-scan`, its input file read and checked (§14.2, §14.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulkPlan {
    pub scanner: Option<String>,
    pub profile: String,
    pub prompts: Vec<String>,
    pub output_file: Option<OsString>,
    pub session_id: Option<String>,
    pub batch_size: u64,
    pub format: Format,
}

fn payload_error(field: &str) -> CliError {
    CliError::usage(format!(
        "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request body for POST /v1/scan/async/request did not match schema: {field}"
    ))
}

/// `^[1-9][0-9]*$`, at most `2^53 − 1` (TS `parsePositiveInteger`).
fn batch_size(value: &str) -> Option<u64> {
    let bytes = value.as_bytes();
    if bytes
        .first()
        .is_none_or(|first| !(b'1'..=b'9').contains(first))
        || !bytes.iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    value
        .parse::<u64>()
        .ok()
        .filter(|size| *size <= MAX_BATCH_SIZE)
}

/// §14.2 steps 4–9 (clap, the `--input` warning, `--output` and `--tenant` come first).
pub fn plan(args: &BulkScanArgs, format: Format) -> Result<BulkPlan, CliError> {
    if let Some(name) = &args.scanner {
        scanners::validate_name(name)?;
    }
    let file = args
        .file
        .as_ref()
        .or(args.input.as_ref())
        .filter(|file| !file.is_empty())
        .ok_or_else(|| CliError::usage("--file <file> is required"))?;
    let batch_size = batch_size(&args.batch_size)
        .ok_or_else(|| CliError::usage("--batch-size must be a positive integer"))?;
    if args.profile.is_empty() {
        return Err(payload_error("scan_req.ai_profile: custom"));
    }
    if args.profile.encode_utf16().count() > 100 {
        return Err(payload_error("scan_req.ai_profile.profile_name: too_big"));
    }
    if args
        .session_id
        .as_ref()
        .is_some_and(|session| session.encode_utf16().count() > 100)
    {
        return Err(payload_error("scan_req.session_id: too_big"));
    }
    if args
        .output_file
        .as_ref()
        .is_some_and(|path| path.is_empty())
    {
        return Err(CliError::usage("--output-file needs a path"));
    }
    let prompts = bulk::input::load(file)?;
    // D132: every prompt must fit a request body once JSON-encoded (the default session
    // has a fixed length, so the check holds for the session the job will use).
    let session = args.session_id.clone().unwrap_or_else(default_session);
    bulk::plan::check_encoded(&prompts, &args.profile, &session, bulk::plan::body_budget())?;
    Ok(BulkPlan {
        scanner: args.scanner.clone(),
        profile: args.profile.clone(),
        prompts,
        output_file: args.output_file.clone(),
        session_id: args.session_id.clone(),
        batch_size,
        format,
    })
}

/// `prisma-airs-cli-bulk-<Unix ms in lowercase base 36>` (the TS default).
fn default_session() -> String {
    let mut ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    let mut digits = Vec::new();
    loop {
        let digit = (ms % 36) as u32;
        digits.push(char::from_digit(digit, 36).unwrap_or('0'));
        ms /= 36;
        if ms == 0 {
            break;
        }
    }
    format!(
        "prisma-airs-cli-bulk-{}",
        digits.iter().rev().collect::<String>()
    )
}

/// What one setup step created, removed in reverse order on failure (§14.4, D123).
struct Created {
    lock: Option<JobLock>,
    output: Option<(OutputFile, bulk::Identity)>,
}

impl Created {
    fn undo(self, file: &StateFile) {
        file.remove_if_ours();
        if let Some((output, identity)) = &self.output {
            output.remove_if(*identity);
        }
        if let Some(lock) = self.lock {
            lock.remove(file.dir());
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Summary<'a> {
    profile: &'a str,
    session_id: &'a str,
    state: String,
    output: String,
    prompts: usize,
    total: usize,
    blocked: usize,
    allowed: usize,
    failed: usize,
}

/// The summary (§14.14): pretty (TS bytes), json or table.
fn render_summary(
    out: &mut Output,
    format: Format,
    title: &str,
    summary: &Summary<'_>,
) -> Result<(), CliError> {
    match format {
        Format::Json => out.json(summary),
        Format::Table => {
            let value = crate::json::to_ordered(summary).map_err(|_| {
                CliError::operational("AISEC_SDK_ERROR:Output could not be serialized as JSON")
            })?;
            let rows = value
                .as_object()
                .unwrap_or_default()
                .iter()
                .map(|(key, item)| (key.clone(), crate::render::cell_text(Some(item))))
                .collect();
            out.data_line(&crate::render::table::key_value_table(rows));
            Ok(())
        }
        Format::Pretty => {
            pretty::banner(out, title, None);
            let blocked = out.paint(Style::Red, &summary.blocked.to_string());
            let allowed = out.paint(Style::Green, &summary.allowed.to_string());
            let failed = out.paint(Style::Red, &summary.failed.to_string());
            let output = out.paint(Style::Cyan, &terminal_safe(&summary.output));
            pretty::key_value(
                out,
                &[
                    ("Total", summary.total.to_string()),
                    ("Blocked", blocked),
                    ("Allowed", allowed),
                    ("Failed", failed),
                    ("Output", output),
                ],
            );
            Ok(())
        }
    }
}

/// Print the summary, then the failed-rows line and exit 1 when a row failed.
pub fn finish(
    invocation: &mut Invocation,
    job: &Job,
    format: Format,
    title: &str,
) -> Result<(), CliError> {
    let (total, blocked, allowed, failed) = job.counts();
    // What `--record-history` may retain: the checkpoint's received results.
    invocation.retained = super::mcp_ops::bulk_receipts(&job.state).0;
    let summary = Summary {
        profile: invocation.secrets.scrub(&job.state.profile),
        session_id: invocation.secrets.scrub(&job.state.session_id),
        state: invocation
            .secrets
            .scrub(&job.file.path().to_string_lossy())
            .to_owned(),
        output: invocation.secrets.scrub(&job.state.output.path).to_owned(),
        prompts: job.state.items.len(),
        total,
        blocked,
        allowed,
        failed,
    };
    render_summary(&mut invocation.output, format, title, &summary)?;
    if failed == 0 {
        return Ok(());
    }
    // The summary is data: write it, then report the failed rows (TS `ui.error`).
    let _ = invocation.output.commit();
    let line = invocation.output.paint_err(
        Style::Red,
        &format!("✗ {failed} prompt(s) failed; successful results were preserved."),
    );
    invocation.output.write_stderr(&format!("  {line}\n"));
    Err(CliError::Reported(crate::error::EXIT_OPERATIONAL))
}

fn cannot_write(path: &Path) -> CliError {
    CliError::operational(format!(
        "Cannot write {}; check that its directory exists and is writable",
        crate::store::shown(path)
    ))
}

/// The tenant, its scanner credentials, its admitted scan endpoint and the scanner
/// client (§14.2 step 11; `resume-poll` the same before its state checks).
struct Scanning {
    tenant: String,
    endpoint: String,
    api: ScannerApi,
    reference: Option<ScannerReference>,
}

fn connect(invocation: &mut Invocation, scanner: Option<&str>) -> Result<Scanning, CliError> {
    let tenant = invocation.load_tenant()?;
    connect_tenant(invocation, &tenant, scanner)
}

fn connect_tenant(
    invocation: &mut Invocation,
    tenant: &crate::tenant::TenantContext,
    scanner: Option<&str>,
) -> Result<Scanning, CliError> {
    let (scanner_config, reference) =
        scanners::select(&tenant.config, scanner, Some(tenant.name()))?;
    let endpoint = clients::admit(
        "airsApiEndpoint",
        scanner_config.effective_str("airsApiEndpoint"),
        DEFAULT_SCAN_ENDPOINT,
    )?;
    let (scanner, last_status) = clients::build_observed_scanner(
        &scanner_config,
        &invocation.session,
        invocation.debug.as_ref(),
    )?;
    Ok(Scanning {
        tenant: tenant.name().to_owned(),
        endpoint: endpoint.url().as_str().to_owned(),
        api: ScannerApi {
            scanner,
            last_status,
        },
        reference,
    })
}

fn working_directory(invocation: &Invocation, name: &Path) -> Result<PathBuf, CliError> {
    invocation
        .env
        .cwd
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| cannot_write(name))
}

pub async fn run(invocation: &mut Invocation, plan: &BulkPlan) -> Result<(), CliError> {
    let Scanning {
        tenant,
        endpoint,
        api,
        reference,
    } = connect(invocation, plan.scanner.as_deref())?;
    let session = invocation.session.clone();
    let interrupted = || CliError::Interrupted(nothing_submitted());

    // §14.4 steps 1–2: the session id and the output path (nothing is created yet).
    let session_id = plan.session_id.clone().unwrap_or_else(default_session);
    let name = match &plan.output_file {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(csv::default_name(&plan.profile)),
    };
    let cwd = working_directory(invocation, &name)?;
    let output_path = csv::absolute(&name, &cwd);
    let output_text = output_path
        .to_str()
        .ok_or_else(|| cannot_write(&output_path))?
        .to_owned();
    csv::refuse_existing(&output_path)?;
    if session.is_cancelled() {
        return Err(interrupted());
    }

    // Steps 3–6: the state directory, the job lock, the output claim, the first
    // checkpoint; a failure removes what this run created.
    let dir = StateDir::ensure(&bulk::state_dir_path(
        invocation.env.xdg_state_home.as_deref(),
        invocation.env.home.as_deref(),
    )?)?;
    let job_id = bulk::new_job_id(std::time::SystemTime::now());
    let file = StateFile::new(dir, &job_id);
    let mut created = Created {
        lock: None,
        output: None,
    };
    created.lock = Some(JobLock::create(file.dir(), &job_id, &file.shown())?);
    let mut output = OutputFile::new(output_path);
    let claimed = if session.is_cancelled() {
        Err(interrupted())
    } else {
        output.claim(csv::HEADER.as_bytes())
    };
    let (identity, claim_report) = match claimed {
        Ok(claimed) => claimed,
        Err(error) => {
            created.undo(&file);
            return Err(error);
        }
    };
    let mut state = JobState::new(NewJob {
        job_id: &job_id,
        tenant: &tenant,
        scan_endpoint: &endpoint,
        profile: &plan.profile,
        session_id: &session_id,
        batch_size: plan.batch_size,
        output_path: &output_text,
        output: identity,
        prompts: plan.prompts.clone(),
    });
    if reference.is_some() {
        state.version = bulk::state::NAMED_VERSION;
        state.scanner_reference = reference;
    }
    let mut job = Job {
        state,
        file,
        output,
        lock: None,
    };
    let first = if session.is_cancelled() {
        Err(interrupted())
    } else {
        job.checkpoint(&Consequence::NothingSubmitted)
    };
    if let Err(error) = first {
        created.output = Some((OutputFile::new(job.output.path().to_path_buf()), identity));
        created.undo(&job.file);
        return Err(error);
    }
    job.lock = created.lock.take();
    if let Rewritten::Warning(text) = claim_report {
        invocation.output.warning(invocation.secrets.scrub(&text));
    }
    invocation.interrupt = Some(job.saved_context());

    // Step 7: the status lines, then the batches.
    let out = &mut invocation.output;
    out.status("Prisma AIRS Bulk Scan");
    out.status(&format!(
        "Profile:  {}",
        terminal_safe(invocation.secrets.scrub(&plan.profile))
    ));
    out.status(&format!(
        "Session:  {}",
        terminal_safe(invocation.secrets.scrub(&session_id))
    ));
    out.status(&format!("Prompts:  {}", plan.prompts.len()));
    out.status(&format!(
        "Batches:  {} (size {})",
        bulk::plan::batch_count(plan.prompts.len(), plan.batch_size),
        plan.batch_size
    ));
    out.status(&format!(
        "State:    {}",
        invocation.secrets.scrub(&job.state_shown())
    ));
    let secrets = invocation.secrets.clone();
    let engine = Engine {
        api: &api,
        session: &session,
        secrets: &secrets,
        timing: Timing::default(),
    };
    let ran = engine.run_new(&mut job, out).await;
    invocation.interrupt = Some(job.saved_context());
    ran?;
    job.save_with_output(&mut invocation.output, &invocation.secrets)?;
    invocation.interrupt = Some(job.saved_context());
    job.release_lock();
    finish(invocation, &job, plan.format, "Bulk Scan Complete")
}

// ---------------------------------------------------------------------------------------
// `runtime resume-poll <stateFile>` (§14.13)
// ---------------------------------------------------------------------------------------

/// A validated `resume-poll` (§14.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResumePlan {
    pub state_file: OsString,
    pub output_file: Option<OsString>,
    pub format: Format,
}

/// §14.2 for `resume-poll` (clap, `--output` and `--tenant` come first).
pub fn plan_resume(args: &ResumePollArgs, format: Format) -> Result<ResumePlan, CliError> {
    if args
        .output_file
        .as_ref()
        .is_some_and(|path| path.is_empty())
    {
        return Err(CliError::usage("--output-file needs a path"));
    }
    Ok(ResumePlan {
        state_file: args.state_file.clone(),
        output_file: args.output_file.clone(),
        format,
    })
}

/// The interruption context before the job lock is held (§14.16).
pub fn nothing_changed() -> InterruptContext {
    InterruptContext::NothingChanged
}

/// `<job>` of a `<job>.bulk-scan.json` file name.
fn job_of(name: &std::ffi::OsStr) -> Option<&str> {
    name.to_str()?
        .strip_suffix(bulk::STATE_SUFFIX)
        .filter(|job| bulk::is_job_id(job))
}

/// Whether the file at `path` is a TypeScript state (read once, bounded, read-only and
/// not locked; only to choose the message).
/// Only a regular file is opened: the path is checked first, and the open is
/// non-blocking (a FIFO swapped in afterwards never blocks it) and re-checked on the
/// descriptor (§14.13 "Details").
fn is_typescript_state(path: &Path) -> bool {
    use std::os::unix::fs::OpenOptionsExt;
    let regular = |metadata: &std::fs::Metadata| {
        metadata.is_file() && metadata.len() <= bulk::MAX_STATE_BYTES
    };
    if !std::fs::metadata(path).is_ok_and(|metadata| regular(&metadata)) {
        return false;
    }
    let Ok(file) = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(path)
    else {
        return false;
    };
    if !file.metadata().is_ok_and(|metadata| regular(&metadata)) {
        return false;
    }
    let mut bytes = Vec::new();
    if std::io::Read::read_to_end(
        &mut std::io::Read::take(file, bulk::MAX_STATE_BYTES + 1),
        &mut bytes,
    )
    .is_err()
        || bytes.len() as u64 > bulk::MAX_STATE_BYTES
    {
        return false;
    }
    bulk::state::parse(&bytes) == Err(bulk::state::LoadError::TypeScript)
}

/// §14.13 steps 1–2 (D127): the canonical argument must be `<job>.bulk-scan.json` in
/// the canonical state directory; returns the job id. Anything else is refused, a
/// TypeScript state with its own message (D118).
fn locate(typed: &Path, cwd: &Path, dir: &Path) -> Result<String, CliError> {
    let shown_typed = crate::store::shown(typed);
    let canonical = std::fs::canonicalize(cwd.join(typed)).map_err(|_| {
        CliError::operational(format!(
            "Cannot read {shown_typed} (missing, not a regular file, unreadable or larger than 128 MiB)"
        ))
    })?;
    let canonical_dir = std::fs::canonicalize(dir).ok();
    let job = match (canonical.parent(), canonical.file_name(), &canonical_dir) {
        (Some(parent), Some(name), Some(dir)) if parent == dir => job_of(name),
        _ => None,
    };
    if let Some(job) = job {
        return Ok(job.to_owned());
    }
    if is_typescript_state(&canonical) {
        return Err(bulk::checkpoint::load_error(
            typed,
            bulk::state::LoadError::TypeScript,
        ));
    }
    let shown_dir = crate::store::shown(canonical_dir.as_deref().unwrap_or(dir));
    Err(CliError::operational(format!(
        "{shown_typed} is not an {PROGRAM} bulk-scan job in {shown_dir}; resume-poll resumes only jobs in its state directory (a moved or copied state cannot be resumed)"
    )))
}

/// §14.13 step 5 (D127): a job resumes only under the tenant and the scan endpoint that
/// started it.
fn check_tenant(job: &Job, tenant: &str, endpoint: &str) -> Result<(), CliError> {
    let state = job.state_shown();
    let recorded = &job.state.tenant;
    if recorded != tenant {
        return Err(CliError::operational(format!(
            "{state} was started with tenant {recorded}; select that tenant (for example with --tenant {recorded}) to resume it"
        )));
    }
    if job.state.scan_endpoint != endpoint {
        return Err(CliError::operational(format!(
            "{state} was started against {}, but tenant {tenant} now uses {}; restore its airsApiEndpoint to resume it",
            terminal_safe(&job.state.scan_endpoint),
            terminal_safe(endpoint)
        )));
    }
    Ok(())
}

/// The job's output after §14.13 step 6.
struct ResolvedOutput {
    output: OutputFile,
    /// The file this run claimed (removed again when the checkpoint fails).
    claimed: Option<bulk::Identity>,
    changed: bool,
    warning: Option<String>,
}

/// §14.12 "resume-poll and --output-file": every check that can refuse comes before
/// anything is written; then a finished rewrite is promoted (`output.next`), a leftover
/// temporary removed, and an absent or re-pointed output claimed with the projection
/// of the loaded state.
fn resolve_output(
    state: &mut JobState,
    requested: Option<PathBuf>,
    state_shown: &str,
) -> Result<ResolvedOutput, CliError> {
    let recorded = PathBuf::from(&state.output.path);
    let target = requested.unwrap_or_else(|| recorded.clone());
    let repoint = target != recorded;
    let at = csv::at_path(&state.output);
    if repoint {
        csv::refuse_existing(&target)?;
    } else if at == csv::AtPath::Foreign {
        return Err(csv::foreign_error(&recorded, state_shown));
    }
    let text = target
        .to_str()
        .ok_or_else(|| cannot_write(&target))?
        .to_owned();
    if at == csv::AtPath::Next
        && let Some(next) = state.output.next.clone()
    {
        state.output.dev = next.dev;
        state.output.ino = next.ino;
    }
    csv::remove_leftover_temp(&state.output);
    let changed = state.output.next.take().is_some();
    let mut output = OutputFile::new(target);
    if !repoint && at != csv::AtPath::Absent {
        // The job's own file stays; its rewrites need a temporary next to it.
        if !output.probe() {
            return Err(output.unwritable_error(state_shown));
        }
        return Ok(ResolvedOutput {
            output,
            claimed: None,
            changed,
            warning: None,
        });
    }
    let (identity, report) = output.claim(csv::projection(state).as_bytes())?;
    state.output = bulk::state::OutputRecord {
        path: text,
        dev: identity.dev.to_string(),
        ino: identity.ino.to_string(),
        next: None,
    };
    Ok(ResolvedOutput {
        output,
        claimed: Some(identity),
        changed: true,
        warning: match report {
            Rewritten::Warning(text) => Some(text),
            Rewritten::Durable => None,
        },
    })
}

/// §14.13 step 6: a durable `submitting` (the process died between C1 and C3) becomes
/// `ambiguous`, its attempt `unknown`. Returns whether anything changed.
pub fn settle_submitting(state: &mut JobState) -> bool {
    let mut attempts = BTreeSet::new();
    for item in &mut state.items {
        if item.status == ItemStatus::Submitting {
            item.status = ItemStatus::Ambiguous;
            attempts.extend(item.attempt);
        }
    }
    let now = bulk::now_stamp();
    for id in &attempts {
        if let Some(attempt) = state.attempt_mut(*id)
            && attempt.outcome == AttemptOutcome::InFlight
        {
            attempt.outcome = AttemptOutcome::Unknown;
            attempt.ended_at = Some(now.clone());
            attempt.error = Some("the process stopped before the outcome was recorded".to_owned());
        }
    }
    !attempts.is_empty()
}

/// The ambiguous refusal (§14.13 step 7): the TypeScript message (the lowest ambiguous
/// index) and the Rust-only counts line.
fn ambiguous_refusal(job: &Job) -> Option<CliError> {
    let items = &job.state.items;
    let lowest = items
        .iter()
        .find(|item| item.status == ItemStatus::Ambiguous)?;
    let count = |status: ItemStatus| items.iter().filter(|item| item.status == status).count();
    Some(CliError::Operational(crate::error::Failure {
        message: format!(
            "Cannot safely resubmit prompt {}: its submission outcome is ambiguous. Known accepted results were preserved; inspect {} before taking manual action.",
            lowest.index,
            job.state_shown()
        ),
        status: None,
        details: vec![format!(
            "{} prompt(s) have an unknown submission outcome and {} were not submitted; nothing will be resubmitted automatically.",
            count(ItemStatus::Ambiguous),
            count(ItemStatus::Pending)
        )],
    }))
}

pub async fn resume(invocation: &mut Invocation, plan: &ResumePlan) -> Result<(), CliError> {
    let session = invocation.session.clone();
    let typed = PathBuf::from(&plan.state_file);
    let cwd = working_directory(invocation, &typed)?;

    // Steps 1–3: the location (D127), the checked directory (not created), the lock;
    // before the tenant, as TypeScript resolves the state path and takes its lock
    // before it loads the config.
    let dir_path = bulk::state_dir_path(
        invocation.env.xdg_state_home.as_deref(),
        invocation.env.home.as_deref(),
    )?;
    let job_id = locate(&typed, &cwd, &dir_path)?;
    let dir = StateDir::open(&dir_path)?;
    let state_shown =
        crate::store::shown(&dir.path().join(format!("{job_id}{}", bulk::STATE_SUFFIX)));
    if session.is_cancelled() {
        return Err(CliError::Interrupted(nothing_changed()));
    }
    let lock = JobLock::acquire(&dir, &job_id, &state_shown)?;

    // Preserve legacy tenant/credential-before-state validation for old configs.
    // Named configs must read the saved reference before selecting a scanner.
    let tenant_context = invocation.load_tenant()?;
    let legacy = if tenant_context.config.scanners().is_none() {
        Some(connect_tenant(invocation, &tenant_context, None)?)
    } else {
        None
    };

    // Step 4: the state, read and validated under the lock.
    let (file, mut state) = StateFile::load(dir, &job_id)?;
    let selected = if let Some(saved) = &state.scanner_reference {
        let current = tenant_context
            .config
            .scanners()
            .and_then(|map| map.get(&saved.name))
            .ok_or_else(|| {
                CliError::operational(
                    "Saved scanner is not configured; restore the original binding before resuming",
                )
            })?;
        if current.application_id != saved.application_id || current.endpoint != saved.endpoint {
            return Err(CliError::operational(
                "Saved scanner association or endpoint changed; restore the original binding before resuming",
            ));
        }
        connect_tenant(invocation, &tenant_context, Some(&saved.name))?
    } else if let Some(legacy) = legacy {
        legacy
    } else {
        connect_tenant(invocation, &tenant_context, None)?
    };
    let Scanning {
        tenant,
        endpoint,
        api,
        reference,
    } = selected;
    let reference_changed = state.scanner_reference != reference;
    if reference_changed {
        invocation.output.warning(invocation.secrets.scrub(
            "Scanner credentials changed since submission; historical lookup access is unqualified",
        ));
        state.scanner_reference = reference;
    }
    let mut job = Job {
        output: OutputFile::new(PathBuf::from(&state.output.path)),
        state: state.clone(),
        file,
        lock: Some(lock),
    };
    invocation.interrupt = Some(job.saved_context());

    // Step 5: the tenant and the scan endpoint that started the job (D127).
    check_tenant(&job, &tenant, &endpoint)?;

    // Step 6: the output (its refusals before anything is written), stale
    // temporaries, `submitting` → `ambiguous`, a checkpoint.
    let requested = plan
        .output_file
        .as_ref()
        .map(|path| csv::absolute(Path::new(path), &cwd));
    let resolved = resolve_output(&mut state, requested, &state_shown)?;
    job.file.sweep_temps();
    let settled = settle_submitting(&mut state);
    job.state = state;
    job.output = resolved.output;
    if (resolved.changed || settled || reference_changed)
        && let Err(error) = job.checkpoint(&Consequence::NothingSubmitted)
    {
        if let Some(identity) = resolved.claimed {
            job.output.remove_if(identity);
        }
        return Err(error);
    }
    if let Some(text) = resolved.warning {
        invocation.output.warning(invocation.secrets.scrub(&text));
    }
    invocation.interrupt = Some(job.saved_context());
    let secrets = invocation.secrets.clone();
    let engine = Engine {
        api: &api,
        session: &session,
        secrets: &secrets,
        timing: Timing::default(),
    };

    // Step 7: the ambiguous path recovers the known receipts' results, then refuses.
    if let Some(refusal) = ambiguous_refusal(&job) {
        let ran = engine.recover(&mut job, &mut invocation.output).await;
        invocation.interrupt = Some(job.saved_context());
        ran?;
        job.save_with_output(&mut invocation.output, &invocation.secrets)?;
        job.release_lock();
        return Err(refusal);
    }

    // Step 8: the normal path.
    let out = &mut invocation.output;
    let scan_ids: BTreeSet<&str> = job
        .state
        .items
        .iter()
        .filter_map(|item| item.scan_id.as_deref())
        .collect();
    out.status("Prisma AIRS Resume Poll");
    out.status(&format!(
        "Profile:  {}",
        terminal_safe(invocation.secrets.scrub(&job.state.profile))
    ));
    out.status(&format!("Scan IDs: {}", scan_ids.len()));
    out.status(&format!("Prompts:  {}", job.state.items.len()));
    let ran = engine.run_resume(&mut job, out).await;
    invocation.interrupt = Some(job.saved_context());
    ran?;

    // Step 9: the final checkpoint and CSV rewrite, then the summary.
    job.save_with_output(&mut invocation.output, &invocation.secrets)?;
    invocation.interrupt = Some(job.saved_context());
    job.release_lock();
    finish(invocation, &job, plan.format, "Resume Poll Complete")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::FromArgMatches;

    fn args(extra: &[&str]) -> BulkScanArgs {
        let mut argv = vec!["airs", "runtime", "bulk-scan"];
        argv.extend_from_slice(extra);
        let matches = crate::cli::command().try_get_matches_from(argv).unwrap();
        match crate::cli::Cli::from_arg_matches(&matches).unwrap().command {
            Some(crate::cli::TopCommand::Runtime(crate::cli::RuntimeGroup {
                command: Some(crate::cli::RuntimeCommand::BulkScan(args)),
            })) => args,
            _ => panic!("bulk-scan"),
        }
    }

    fn usage(extra: &[&str]) -> String {
        match plan(&args(extra), Format::Pretty) {
            Err(CliError::Usage(message)) => message,
            other => panic!("{other:?}"),
        }
    }

    /// int-spec #21 and §14.2: argument checks in order, before any file is read.
    #[test]
    fn argument_checks_run_in_the_documented_order() {
        assert_eq!(usage(&["--profile", "p"]), "--file <file> is required");
        assert_eq!(
            usage(&["--profile", "p", "--file", ""]),
            "--file <file> is required"
        );
        for size in [
            "0",
            "-1",
            "1.5",
            "abc",
            "007",
            "9007199254740992",
            "",
            "+1",
            " 1",
        ] {
            assert_eq!(
                usage(&[
                    "--profile",
                    "p",
                    "--file",
                    "missing.txt",
                    "--batch-size",
                    size
                ]),
                "--batch-size must be a positive integer",
                "{size:?}"
            );
        }
        assert_eq!(batch_size("9007199254740991"), Some(9_007_199_254_740_991));
        assert_eq!(
            usage(&["--profile", "", "--file", "missing.txt"]),
            "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request body for POST /v1/scan/async/request did not match schema: scan_req.ai_profile: custom"
        );
        let long = "p".repeat(101);
        assert_eq!(
            usage(&["--profile", &long, "--file", "missing.txt"]),
            "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request body for POST /v1/scan/async/request did not match schema: scan_req.ai_profile.profile_name: too_big"
        );
        assert_eq!(
            usage(&[
                "--profile",
                "p",
                "--file",
                "missing.txt",
                "--session-id",
                &long
            ]),
            "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request body for POST /v1/scan/async/request did not match schema: scan_req.session_id: too_big"
        );
        assert_eq!(
            usage(&[
                "--profile",
                "p",
                "--file",
                "missing.txt",
                "--output-file",
                ""
            ]),
            "--output-file needs a path"
        );
        // 100 UTF-16 units pass; the missing file is then the (operational) error.
        let hundred = "é".repeat(100);
        assert!(matches!(
            plan(
                &args(&[
                    "--profile",
                    &hundred,
                    "--file",
                    "/nonexistent/x.txt",
                    "--session-id",
                    &hundred
                ]),
                Format::Pretty
            ),
            Err(CliError::Operational(_))
        ));
    }

    /// §14.14 and §2.8: the TS summary colours (keys dim, Blocked and Failed red,
    /// Allowed green, Output cyan, Total plain).
    #[test]
    fn the_pretty_summary_has_the_typescript_colours() {
        let (mut out, stdout, _) = crate::output::testing::capture_colored(false);
        let summary = Summary {
            profile: "p",
            session_id: "s",
            state: "/s/j.bulk-scan.json".into(),
            output: "/o.csv".into(),
            prompts: 3,
            total: 3,
            blocked: 1,
            allowed: 2,
            failed: 0,
        };
        render_summary(&mut out, Format::Pretty, "Bulk Scan Complete", &summary).unwrap();
        out.commit().unwrap();
        let key = |k: &str| format!("\u{1b}[2m{k}\u{1b}[22m");
        assert_eq!(
            stdout.text(),
            format!(
                "\n  \u{1b}[1mBulk Scan Complete\u{1b}[22m\n\n  {}  3\n  {}  \u{1b}[31m1\u{1b}[39m\n  {}  \u{1b}[32m2\u{1b}[39m\n  {}  \u{1b}[31m0\u{1b}[39m\n  {}  \u{1b}[36m/o.csv\u{1b}[39m\n",
                key("Total  "),
                key("Blocked"),
                key("Allowed"),
                key("Failed "),
                key("Output ")
            )
        );
    }

    /// §14.13 step 6: a durable `submitting` becomes `ambiguous` with its attempt
    /// `unknown`; the result is a valid state and nothing else changes.
    #[test]
    fn a_durable_submitting_becomes_ambiguous() {
        let mut state = JobState::new(NewJob {
            job_id: "20261017T120000Z-3f9c0a1b2c3d4e5f",
            tenant: "prod",
            scan_endpoint: "http://127.0.0.1:1/scan",
            profile: "p",
            session_id: "s",
            batch_size: 25,
            output_path: "/abs/out.csv",
            output: bulk::Identity { dev: 1, ino: 2 },
            prompts: vec!["a".into(), "b".into(), "c".into()],
        });
        assert!(!settle_submitting(&mut state), "nothing to settle");
        state.attempts.push(crate::bulk::state::Attempt {
            id: 1,
            indices: vec![0, 1],
            started_at: bulk::now_stamp(),
            ended_at: None,
            outcome: AttemptOutcome::InFlight,
            http_status: None,
            error: None,
            scan_id: None,
        });
        for item in &mut state.items[..2] {
            item.status = ItemStatus::Submitting;
            item.attempt = Some(1);
        }
        state.validate().unwrap();
        assert!(settle_submitting(&mut state));
        state.validate().unwrap();
        let statuses: Vec<ItemStatus> = state.items.iter().map(|item| item.status).collect();
        assert_eq!(
            statuses,
            [
                ItemStatus::Ambiguous,
                ItemStatus::Ambiguous,
                ItemStatus::Pending
            ]
        );
        let attempt = &state.attempts[0];
        assert_eq!(attempt.outcome, AttemptOutcome::Unknown);
        assert!(attempt.ended_at.is_some());
        assert_eq!(
            attempt.error.as_deref(),
            Some("the process stopped before the outcome was recorded")
        );
    }

    /// §14.2: `resume-poll --output-file ""` is a usage error; everything else is
    /// checked after the tenant.
    #[test]
    fn resume_planning_checks_only_the_output_file() {
        let matches = crate::cli::command()
            .try_get_matches_from([
                "airs",
                "runtime",
                "resume-poll",
                "s.json",
                "--output-file",
                "",
            ])
            .unwrap();
        let Some(crate::cli::TopCommand::Runtime(crate::cli::RuntimeGroup {
            command: Some(crate::cli::RuntimeCommand::ResumePoll(args)),
        })) = crate::cli::Cli::from_arg_matches(&matches).unwrap().command
        else {
            panic!("resume-poll")
        };
        assert!(matches!(
            plan_resume(&args, Format::Pretty),
            Err(CliError::Usage(message)) if message == "--output-file needs a path"
        ));
        let mut args = args;
        args.output_file = None;
        assert_eq!(
            plan_resume(&args, Format::Json).unwrap(),
            ResumePlan {
                state_file: "s.json".into(),
                output_file: None,
                format: Format::Json
            }
        );
    }

    #[test]
    fn the_hidden_input_alias_yields_to_file() {
        let both = args(&["--profile", "p", "--input", "a.txt", "--file", "b.txt"]);
        assert_eq!(both.file.as_deref(), Some("b.txt".as_ref()));
        assert_eq!(both.input.as_deref(), Some("a.txt".as_ref()));
        assert_eq!(args(&["--profile", "p"]).batch_size, "25");
    }

    #[test]
    fn default_sessions_are_base36_milliseconds() {
        let session = default_session();
        let digits = session.strip_prefix("prisma-airs-cli-bulk-").unwrap();
        assert!(digits.len() >= 8, "{session}");
        assert!(
            digits
                .bytes()
                .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
        );
        let ms = u128::from_str_radix(digits, 36).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        assert!(now - ms < 60_000);
    }
}
