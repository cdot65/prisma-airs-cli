//! The M7.2/M7.3 service adapters of the optional runtime MCP client
//! (RUNTIME_MCP_CONTRACT §3, §5, §7; DESIGN D355–D360):
//!
//! - `runtime report --mcp` and `runtime history query|reports|download`: archive-only
//!   reports and reads; downloads are hash-verified chunk by chunk and published
//!   without clobbering into a new private directory;
//! - `runtime history prune|compact|backup|recover|restore`: a preview (recorded
//!   privately), then `--execute PREVIEW_ID` on an admin-role connection (a fresh,
//!   nonce-bound sign-in, never a cached session) or with `--approval` on the
//!   previewing connection; `airs mcp confirm` issues that approval;
//! - `runtime history export|import`: the whole admin flow in one command (preview,
//!   fresh proof, execute; chunked `export_read` or a staged upload);
//! - `runtime receipts upload` and `--record-history`: already-received bulk/scan
//!   metadata, batched at 200 records/256 KiB under deterministic batch keys; never an
//!   AIRS call, never a resubmission, never an interactive sign-in.

use std::path::{Path, PathBuf};

use super::Invocation;
use super::mcp_client::{
    self as mc, Auth, connection, connection_any, emit, epoch, new_key, refused, trust,
};
use crate::{
    error::{CliError, InterruptContext},
    output::Format,
};
use airs_mcp_client::{Called, McpConnection, Role, previews, receipts};
use serde_json::{Value, json};

/// Chunk size of byte transfers (contract §5: 64 KiB raw).
pub const CHUNK: u64 = 65_536;
/// Attempts per chunk before a transfer stops (each from the same verified offset).
const CHUNK_ATTEMPTS: usize = 3;
/// Receipt batch bounds (contract §4.3 `receipts_ingest`).
pub const BATCH_RECORDS: usize = 200;
pub const BATCH_BYTES: usize = 256 * 1024;
/// The whole automatic retention wait of one `--record-history` command (plan M7.3).
pub const RECORD_BUDGET: std::time::Duration = std::time::Duration::from_secs(2);

fn op(e: impl std::fmt::Display) -> CliError {
    CliError::operational(e.to_string())
}

/// The token of one command on `c`: its private session (ordinary connections) or one
/// fresh sign-in (admin connections keep no session, contract §3 option B).
async fn auth_for(
    c: &McpConnection,
    sign_in: &SignIn,
    extra: &[(&str, &str)],
) -> Result<Auth, CliError> {
    match c.role {
        Role::Ordinary => Ok(Auth::Session),
        Role::Admin => {
            let t = trust(c)?;
            let token =
                airs_mcp_client::fresh_token(&t, &c.client_id, sign_in.port, sign_in.open, extra)
                    .await
                    .map_err(|e| CliError::operational(format!("Sign-in failed: {e}")))?;
            mc::protect(&token);
            Ok(Auth::Token(token))
        }
    }
}

/// The fresh administrative proof of one preview: a sign-in that asks for a fresh
/// authentication and carries the preview's nonce (the gateway signs both into the
/// identity assertion, contract §3 option B).
async fn proof_for(c: &McpConnection, sign_in: &SignIn, nonce: &str) -> Result<Auth, CliError> {
    auth_for(c, sign_in, &[("max_age", "0"), ("airs_proof_nonce", nonce)]).await
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SignIn {
    pub open: bool,
    pub port: u16,
}

async fn call(
    c: &McpConnection,
    auth: &Auth,
    tool: &str,
    params: Value,
    key: Option<(&str, &str)>,
) -> Result<Value, CliError> {
    match mc::outcome(c, auth, tool, params, key).await? {
        Called::Ok(v) => Ok(v),
        Called::Refused(r) => Err(refused(r)),
    }
}

// ---------------------------------------------------------------------------------------
// Keyed submissions with durable operation receipts (D354) and one bounded
// authentication recovery (contract §7.4).
// ---------------------------------------------------------------------------------------

/// A submission admitted on the service, and its receipt (removed only once the
/// terminal result has been written to stdout, M7 review 1 finding 2).
pub struct Admitted {
    pub accepted: Value,
    pub job_id: String,
    book: receipts::Receipts,
    key: String,
}

impl Admitted {
    /// Deliver the terminal result already emitted to `inv`'s output: write stdout now,
    /// and remove the receipt only when that write succeeded. A closed pipe keeps the
    /// receipt (nobody received the result) and is otherwise success, as for every
    /// command; a failed or interrupted write keeps it and is reported. A strict or
    /// failed-job exit returned afterwards cannot discard the written result.
    pub fn deliver(self, inv: &mut Invocation) -> Result<(), CliError> {
        match inv.output.commit() {
            Ok(()) => {
                let _ = self.book.remove(&self.key);
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
            Err(_) if inv.output.pending_signal().is_some() => {
                Err(CliError::Interrupted(InterruptContext::OutputIncomplete))
            }
            Err(_) => Err(crate::output::stdout_write_failed()),
        }
    }
}

/// Submit `tool(params)` under a new key minted in the service's current epoch (a job
/// submission, or a synchronous keyed mutation such as a schedule change): the
/// receipt is durable before the first send; a transport or authentication failure
/// is retried once, with the same key, after one token refresh (session connections);
/// a refusal created nothing, so its receipt is removed.
pub async fn submit(
    c: &McpConnection,
    name: &str,
    auth: &Auth,
    tool: &str,
    params: Value,
) -> Result<Admitted, CliError> {
    submit_tracked(c, name, auth, tool, params)
        .await
        .map_err(|(error, _)| error)
}

/// Whether a failed submission can have reached the service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sent {
    /// Nothing was sent (the epoch read or the local receipt failed).
    No,
    /// The service refused it: nothing was created.
    Refused,
    /// It may have been admitted (a lost response, a failure after admission): the
    /// receipt stays for reconciliation.
    Uncertain,
}

/// [`submit`], telling a caller that must clean up (an import's staged upload, M7
/// review 2 finding 3) whether the failure can have reached the service.
pub async fn submit_tracked(
    c: &McpConnection,
    name: &str,
    auth: &Auth,
    tool: &str,
    params: Value,
) -> Result<Admitted, (CliError, Sent)> {
    let not_sent = |e: CliError| (e, Sent::No);
    let uncertain = |e: CliError| (e, Sent::Uncertain);
    // One bounded authentication recovery per submission (contract §7.4): a session
    // whose token the gateway no longer accepts is refreshed once (never a sign-in),
    // whether that shows at the epoch read or at the submission itself.
    let mut recovered: Option<Auth> = None;
    let e = match (epoch(c, auth).await, auth) {
        (Ok(e), _) => e,
        (Err(error), Auth::Session) => {
            let fresh = refresh(c).await.ok_or(error).map_err(not_sent)?;
            let e = epoch(c, &fresh).await.map_err(not_sent)?;
            recovered = Some(fresh);
            e
        }
        (Err(error), Auth::Token(_)) => return Err(not_sent(error)),
    };
    let key = new_key();
    let root = airs_mcp_client::state_root()
        .map_err(op)
        .map_err(not_sent)?;
    let book = receipts::Receipts::open(&root, name)
        .map_err(op)
        .map_err(not_sent)?;
    let mut receipt = receipts::Receipt {
        format: receipts::FORMAT.into(),
        operation: tool.into(),
        params_digest: airs_mcp_client::sha256_hex(
            &serde_json::to_vec(&params).unwrap_or_default(),
        ),
        idempotency_key: key.clone(),
        epoch: e.clone(),
        created_at: airs_runtime_core::time::now_ms(),
        job_id: None,
        connection_revision: c.revision.clone(),
    };
    book.put(&receipt).map_err(op).map_err(not_sent)?;
    let first = mc::outcome(
        c,
        recovered.as_ref().unwrap_or(auth),
        tool,
        params.clone(),
        Some((&key, &e)),
    )
    .await;
    let called = match (first, auth, &recovered) {
        (Ok(called), _, _) => called,
        (Err(error), Auth::Session, None) => {
            // The recovery: refresh, then replay the same key (never a new one).
            let fresh = refresh(c).await.ok_or(error).map_err(uncertain)?;
            mc::outcome(c, &fresh, tool, params, Some((&key, &e)))
                .await
                .map_err(uncertain)?
        }
        (Err(error), _, _) => return Err(uncertain(error)),
    };
    let accepted = match called {
        Called::Ok(v) => v,
        Called::Refused(r) => {
            let _ = book.remove(&key);
            return Err((refused(r), Sent::Refused));
        }
    };
    let job_id = accepted["job"]["job_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    // A job must carry a well-formed ID before it is persisted or shown; the key-only
    // receipt stays for reconciliation.
    // A well-formed ID can still carry a secret (a token as its UUID part, M7 review 3
    // finding 2): it is never persisted, so a later invocation cannot print it.
    if accepted.get("job").is_some() && (!mc::valid_job_id(&job_id) || mc::carries_secret(&job_id))
    {
        return Err(uncertain(CliError::operational(
            "The MCP service returned an invalid job ID; the submission's receipt is kept for reconciliation",
        )));
    }
    // A synchronous keyed mutation (a schedule change) names no job: its receipt keeps
    // the key alone until the result is delivered (M7 review 1 finding 3).
    if !job_id.is_empty() {
        receipt.job_id = Some(job_id.clone());
        book.put(&receipt).map_err(op).map_err(uncertain)?;
    }
    Ok(Admitted {
        accepted,
        job_id,
        book,
        key,
    })
}

/// A refreshed access token for `c`'s session (the one recovery), if the refresh
/// succeeds.
async fn refresh(c: &McpConnection) -> Option<Auth> {
    let t = trust(c).ok()?;
    let store = mc::store_for(c, &t).ok()?;
    let token = airs_mcp_client::refreshed_token(&t, &store).await.ok()?;
    mc::protect(&token);
    Some(Auth::Token(token))
}

const TERMINAL: [&str; 4] = ["succeeded", "partial", "failed", "canceled"];

/// Wait for `job_id` to reach a terminal state (an interruption prints the job ID and
/// leaves the job running; the timeout reports where it continues).
pub async fn wait(
    inv: &mut Invocation,
    c: &McpConnection,
    name: &str,
    auth: &Auth,
    job_id: &str,
    timeout: u64,
    what: &str,
) -> Result<Value, CliError> {
    if !mc::valid_job_id(job_id) {
        return Err(CliError::operational(
            "The MCP service returned an invalid job ID",
        ));
    }
    inv.interrupt = Some(InterruptContext::Context(format!(
        "the {what} job continues on the service (job {job_id}); check it with 'airs runtime jobs get --mcp {name} {job_id}'"
    )));
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout);
    let session = inv.session.clone();
    let interrupted = || {
        CliError::Interrupted(InterruptContext::Context(format!(
            "the {what} job continues on the service (job {job_id}); it was not canceled"
        )))
    };
    let mut state = String::from("queued");
    let timed_out = |state: &str| {
        CliError::operational(format!(
            "The {what} job is still {state} after {timeout} seconds (job {job_id}); it continues on the service"
        ))
    };
    loop {
        // The whole poll (authentication, connection and `jobs_get`) races the absolute
        // deadline and cancellation: a stalled request cannot outlast `--timeout` or
        // delay an interruption (M7 review 1 finding 4).
        let job = tokio::select! {
            () = session.cancelled() => return Err(interrupted()),
            () = tokio::time::sleep_until(deadline) => return Err(timed_out(&state)),
            job = call(c, auth, "jobs_get", json!({"job_id": job_id}), None) => job?,
        };
        state = job["job"]["state"].as_str().unwrap_or_default().to_owned();
        if TERMINAL.contains(&state.as_str()) {
            return Ok(job);
        }
        tokio::select! {
            () = session.cancelled() => return Err(interrupted()),
            () = tokio::time::sleep_until(deadline) => return Err(timed_out(&state)),
            () = tokio::time::sleep(std::time::Duration::from_millis(500)) => {}
        }
    }
}

fn ended(job: &Value, what: &str) -> Result<(), CliError> {
    match job["job"]["state"].as_str() {
        Some(state @ ("failed" | "canceled")) => Err(CliError::operational(format!(
            "The {what} job ended {state}"
        ))),
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------------------
// Hash-verified byte transfer (contract §5.2, §5.5).
// ---------------------------------------------------------------------------------------

/// One verified chunk of `tool` (`artifacts_download` or `export_read`): its offset and
/// length, the chunk hash and the file hash all checked.
async fn chunk(
    c: &McpConnection,
    auth: &Auth,
    tool: &str,
    params: Value,
    offset: u64,
    file_sha: Option<&str>,
) -> Result<(Vec<u8>, u64, String), CliError> {
    let mut last = CliError::operational("The transfer could not read a chunk");
    for _ in 0..CHUNK_ATTEMPTS {
        let v = match mc::outcome(c, auth, tool, params.clone(), None).await {
            Ok(Called::Ok(v)) => v,
            // A refusal is the service's answer: never retried blindly.
            Ok(Called::Refused(r)) => return Err(refused(r)),
            Err(e) => {
                last = e;
                continue;
            }
        };
        let bytes = v["data_base64"]
            .as_str()
            .and_then(airs_mcp_client::b64_decode)
            .ok_or_else(|| CliError::operational("The service sent an invalid chunk"))?;
        let total = v["total_bytes"].as_u64().unwrap_or(u64::MAX);
        let sha = v["file_sha256"].as_str().unwrap_or_default().to_owned();
        if v["offset"].as_u64() != Some(offset)
            || v["length"].as_u64() != Some(bytes.len() as u64)
            || v["chunk_sha256"].as_str() != Some(airs_mcp_client::sha256_hex(&bytes).as_str())
            || file_sha.is_some_and(|f| f != sha)
            || offset + bytes.len() as u64 > total
            || (bytes.is_empty() && offset < total)
        {
            return Err(CliError::operational(
                "A transferred chunk failed its hash or offset check; nothing was published",
            ));
        }
        return Ok((bytes, total, sha));
    }
    Err(last)
}

/// Fetch a whole file of at most `cap` bytes, verifying each chunk and the end-to-end
/// SHA-256 (`expected` when the manifest names it).
#[allow(clippy::too_many_arguments)]
async fn fetch(
    inv: &Invocation,
    c: &McpConnection,
    auth: &Auth,
    tool: &str,
    base: Value,
    expected: Option<&str>,
    cap: u64,
    mut sink: impl FnMut(&[u8]) -> Result<(), CliError>,
) -> Result<(u64, String), CliError> {
    let mut offset = 0;
    let mut file_sha: Option<String> = expected.map(str::to_owned);
    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    // No transferred byte that carries a registered secret is ever written: each chunk
    // is screened together with the tail of the one before it, so a secret split across
    // chunks is caught too (M7 review 7 finding 1). The overlap is taken again for every
    // chunk, since a chunk request can register a longer token, and never below
    // MIN_OVERLAP (M7 review 8 finding 1). Callers also re-screen the whole file against
    // the final registry before publishing it.
    let mut tail: Vec<u8> = Vec::new();
    loop {
        if inv.session.is_cancelled() {
            return Err(CliError::Interrupted(InterruptContext::NoDataWritten));
        }
        let mut params = base.clone();
        params["offset"] = json!(offset);
        params["length"] = json!(CHUNK);
        let (bytes, total, sha) = chunk(c, auth, tool, params, offset, file_sha.as_deref()).await?;
        if total > cap {
            return Err(CliError::operational("The file exceeds its transfer bound"));
        }
        file_sha.get_or_insert(sha);
        let overlap = mc::longest_secret().max(MIN_OVERLAP);
        if !screened_window(&mut tail, &bytes, overlap, mc::carries_secret) {
            return Err(CliError::operational(CARRIES));
        }
        sha2::Digest::update(&mut hasher, &bytes);
        sink(&bytes)?;
        offset += bytes.len() as u64;
        if offset >= total {
            let got = format!("{:x}", sha2::Digest::finalize(hasher));
            if Some(got.as_str()) != file_sha.as_deref() {
                return Err(CliError::operational(
                    "The transferred file failed its end-to-end hash check; nothing was published",
                ));
            }
            return Ok((total, got));
        }
    }
}

/// The least overlap a transfer keeps between chunks (a token registered later in the
/// transfer, up to this length, is still found across a boundary).
const MIN_OVERLAP: usize = 16 * 1024;

/// Whether a whole file, already in memory, carries no registered secret (the final
/// screen before publication, against the registry as it is once every chunk is read).
fn clean(bytes: &[u8]) -> bool {
    !mc::carries_secret(&String::from_utf8_lossy(bytes))
}

const CARRIES: &str =
    "The transferred file carries a credential this command uses; nothing was published";

/// Whether `bytes`, read after `tail` (the end of the previous chunk), carries no secret
/// by `carries`; `tail` keeps the last `overlap - 1` bytes for the next chunk, so a
/// secret of at most `overlap` bytes split across chunks is still found.
fn screened_window(
    tail: &mut Vec<u8>,
    bytes: &[u8],
    overlap: usize,
    carries: impl Fn(&str) -> bool,
) -> bool {
    if overlap == 0 {
        return true;
    }
    let mut window = std::mem::take(tail);
    window.extend_from_slice(bytes);
    if carries(&String::from_utf8_lossy(&window)) {
        return false;
    }
    *tail = window[window.len().saturating_sub(overlap - 1)..].to_vec();
    true
}

/// Publish one verified report file, only when it is clean against the registry as it
/// is now (M7 review 8 finding 1).
fn publish_single(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    if !clean(bytes) {
        return Err(CliError::operational(CARRIES));
    }
    crate::files::publish_new(path, bytes, "--output-file")?;
    Ok(())
}

/// Link a streamed export into place. When a token was registered while it streamed
/// (`registered` was the registry size at its start), the written temporary is first
/// re-screened against the final registry (M7 review 8 finding 1).
fn finish_export(
    reservation: crate::files::Reservation,
    file: &Path,
    registered: usize,
) -> Result<(), CliError> {
    if mc::registry_len() != registered && !rescan(reservation.temp_path())? {
        return Err(CliError::operational(CARRIES));
    }
    match reservation.finish_written(file) {
        Ok(_) => Ok(()),
        Err(crate::files::LinkError::Exists(_)) => {
            Err(crate::files::already_exists(file, "--file"))
        }
        Err(_) => Err(crate::files::cannot_write(file)),
    }
}

/// Whether the file at `path` carries no registered secret, read in bounded chunks.
fn rescan(path: &Path) -> Result<bool, CliError> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|_| crate::files::cannot_write(path))?;
    let mut tail = Vec::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|_| crate::files::cannot_write(path))?;
        if n == 0 {
            return Ok(true);
        }
        let overlap = mc::longest_secret().max(MIN_OVERLAP);
        if !screened_window(&mut tail, &buf[..n], overlap, mc::carries_secret) {
            return Ok(false);
        }
    }
}

/// A new private directory (0700); an existing path is refused.
fn new_private_dir(dir: &Path, flag: &str) -> Result<(), CliError> {
    use std::os::unix::fs::DirBuilderExt;
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(crate::files::already_exists(dir, flag))
        }
        Err(_) => Err(crate::files::cannot_write(dir)),
    }
}

/// Download `names` of `version` into the new private directory `dir`: each file is
/// verified against the version's manifest and published without clobbering.
pub async fn download(
    inv: &mut Invocation,
    c: &McpConnection,
    auth: &Auth,
    version: &str,
    names: &[String],
    dir: &Path,
    flag: &str,
) -> Result<Vec<PathBuf>, CliError> {
    let manifest = version_manifest(c, auth, version).await?;
    new_private_dir(dir, flag)?;
    // Every file is read and verified before any is published, and each is screened
    // against the final registry, so a token registered while a later file was read
    // cannot be in an earlier, already published one (M7 review 8 finding 1).
    let mut fetched: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    for name in names {
        let (sha, length) = match manifest_entry(&manifest, name) {
            Ok(e) => e,
            Err(e) => {
                let _ = std::fs::remove_dir(dir);
                return Err(e);
            }
        };
        let mut bytes = Vec::new();
        let result = fetch(
            inv,
            c,
            auth,
            "artifacts_download",
            json!({"version_id": version, "name": name}),
            Some(&sha),
            REPORT_FILE_CAP,
            |b| {
                bytes.extend_from_slice(b);
                Ok(())
            },
        )
        .await;
        if let Err(e) = result {
            let _ = std::fs::remove_dir(dir);
            return Err(e);
        }
        if bytes.len() as u64 != length {
            let _ = std::fs::remove_dir(dir);
            return Err(CliError::operational(
                "The downloaded file does not match its manifest; nothing was published",
            ));
        }
        fetched.push((dir.join(name), bytes));
    }
    publish_bundle(dir, fetched, flag)
}

/// Publish a verified bundle into its new directory `dir`: nothing is written unless
/// every file is clean against the registry as it is now (M7 review 8 finding 1).
fn publish_bundle(
    dir: &Path,
    fetched: Vec<(PathBuf, Vec<u8>)>,
    flag: &str,
) -> Result<Vec<PathBuf>, CliError> {
    if !fetched.iter().all(|(_, b)| clean(b)) {
        let _ = std::fs::remove_dir(dir);
        return Err(CliError::operational(CARRIES));
    }
    let mut written = Vec::new();
    for (path, bytes) in fetched {
        crate::files::publish_new(&path, &bytes, flag)?;
        written.push(path);
    }
    Ok(written)
}

/// The bound on one report file (the server's per-file publication limit).
const REPORT_FILE_CAP: u64 = 64 * 1024 * 1024;

/// The manifest entry for `name`: required, with a SHA-256 of 64 lowercase hex digits
/// and a byte length within the file bound, before anything is downloaded (M7 review 1
/// finding 5). Single-file and bundle retrieval share it.
fn manifest_entry(manifest: &Value, name: &str) -> Result<(String, u64), CliError> {
    let entry = manifest["files"]
        .as_array()
        .and_then(|f| f.iter().find(|e| e["name"] == name))
        .ok_or_else(|| CliError::operational("The report version has no such file"))?;
    let sha = entry["sha256"]
        .as_str()
        .filter(|h| {
            h.len() == 64
                && h.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .ok_or_else(|| {
            CliError::operational("The report manifest names no valid hash for the file")
        })?;
    let bytes = entry["bytes"]
        .as_u64()
        .filter(|b| *b <= REPORT_FILE_CAP)
        .ok_or_else(|| {
            CliError::operational("The report manifest names no valid length for the file")
        })?;
    Ok((sha.to_owned(), bytes))
}

/// The manifest of a report version (`reports_list` paged until found is avoided:
/// `reports_get` needs the report ID, so the version is looked up through the list).
async fn version_manifest(
    c: &McpConnection,
    auth: &Auth,
    version: &str,
) -> Result<Value, CliError> {
    let mut cursor: Option<String> = None;
    for _ in 0..200 {
        let mut page = json!({"limit": 50});
        if let Some(cur) = &cursor {
            page["cursor"] = json!(cur);
        }
        let list = call(c, auth, "reports_list", json!({"page": page}), None).await?;
        if let Some(m) = list["items"]
            .as_array()
            .and_then(|i| i.iter().find(|m| m["version_id"] == version))
        {
            return Ok(m.clone());
        }
        match list["next_cursor"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => {
                return Err(CliError::operational(
                    "No such report version on the service",
                ));
            }
        }
    }
    // The page budget ended with more pages to read: the search is incomplete, which
    // is not evidence that the version is absent (M7 review 1 minor).
    Err(CliError::operational(
        "The report version was not found within the 200-page search budget; the search is incomplete",
    ))
}

// ---------------------------------------------------------------------------------------
// `runtime report --mcp` (contract §7.2).
// ---------------------------------------------------------------------------------------

pub enum ReportDest {
    None,
    Bundle(PathBuf),
    File(PathBuf),
}

pub struct ReportJob {
    pub connection: String,
    pub params: Value,
    pub wait: bool,
    pub timeout: u64,
    pub strict: bool,
    pub dest: ReportDest,
}

pub async fn report(inv: &mut Invocation, job: ReportJob, format: Format) -> Result<(), CliError> {
    let c = connection(inv, &job.connection)?;
    let auth = Auth::Session;
    let admitted = submit(&c, &job.connection, &auth, "reports_create", job.params).await?;
    if !job.wait {
        emit(inv, &admitted.accepted, format)?;
        return Ok(());
    }
    let id = admitted.job_id.clone();
    let done = wait(inv, &c, &job.connection, &auth, &id, job.timeout, "report").await?;
    if let Err(e) = ended(&done, "report") {
        // The failed job's result is delivered before its failure exit.
        emit(inv, &done, format)?;
        admitted.deliver(inv)?;
        return Err(e);
    }
    let result = &done["job"]["result"];
    let version = result["version_id"].as_str().unwrap_or_default().to_owned();
    let partial = result["completeness"] == "partial";
    match &job.dest {
        ReportDest::None => {}
        ReportDest::Bundle(dir) => {
            let names = ["report.html", "evidence.json", "review.md"].map(String::from);
            download(inv, &c, &auth, &version, &names, dir, "--bundle-dir").await?;
            inv.output.status(&format!(
                "Report bundle: {}",
                inv.secrets.scrub(&crate::store::shown(dir))
            ));
        }
        ReportDest::File(path) => {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let manifest = version_manifest(&c, &auth, &version).await?;
            let (sha, length) = manifest_entry(&manifest, "report.html")?;
            let mut bytes = Vec::new();
            fetch(
                inv,
                &c,
                &auth,
                "artifacts_download",
                json!({"version_id": version, "name": "report.html"}),
                Some(&sha),
                REPORT_FILE_CAP,
                |b| {
                    bytes.extend_from_slice(b);
                    Ok(())
                },
            )
            .await?;
            if bytes.len() as u64 != length {
                return Err(CliError::operational(
                    "The downloaded file does not match its manifest; nothing was published",
                ));
            }
            let _ = parent;
            publish_single(path, &bytes)?;
            inv.output.status(&format!(
                "Report: {}",
                inv.secrets.scrub(&crate::store::shown(path))
            ));
        }
    }
    emit(inv, &done, format)?;
    admitted.deliver(inv)?;
    if partial && job.strict {
        return Err(CliError::operational(
            "The report is partial (--strict); its gaps are listed in the result",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Lifecycle previews, execution and confirmation (contract §3, §5.1).
// ---------------------------------------------------------------------------------------

pub struct LifecycleJob {
    pub connection: String,
    /// The preview parameters (`action` and its fields).
    pub params: Value,
    pub execute: Option<String>,
    pub approval: Option<String>,
    pub wait: bool,
    pub sign_in: SignIn,
}

fn record(c: &McpConnection, v: &Value) -> previews::PreviewRecord {
    previews::PreviewRecord {
        format: previews::FORMAT.into(),
        preview_id: v["preview_id"].as_str().unwrap_or_default().into(),
        action: v["action"].as_str().unwrap_or_default().into(),
        parameter_digest: v["parameter_digest"].as_str().unwrap_or_default().into(),
        proof_nonce: v["proof_nonce"].as_str().map(str::to_owned),
        expires_at: v["expires_at"].as_str().unwrap_or_default().into(),
        connection_revision: c.revision.clone(),
    }
}

/// `lifecycle_preview` on `c`, recorded privately for `--execute`/`confirm`.
async fn preview(
    c: &McpConnection,
    name: &str,
    auth: &Auth,
    params: Value,
) -> Result<(Value, previews::PreviewRecord), CliError> {
    let v = call(c, auth, "lifecycle_preview", params, None).await?;
    let r = record(c, &v);
    // Never persist a malformed or secret-bearing record: a later invocation (`mcp
    // confirm`) would show it without knowing this invocation's tokens (M7 review 4).
    let fields = [
        r.preview_id.as_str(),
        r.action.as_str(),
        r.parameter_digest.as_str(),
        r.proof_nonce.as_deref().unwrap_or_default(),
        r.expires_at.as_str(),
    ];
    if !previews::well_formed(&r) || fields.iter().any(|f| mc::carries_secret(f)) {
        return Err(CliError::operational(
            "The MCP service returned an invalid preview; nothing was recorded",
        ));
    }
    let root = airs_mcp_client::state_root().map_err(op)?;
    previews::Previews::open(&root, name)
        .and_then(|p| p.put(&r))
        .map_err(op)?;
    Ok((v, r))
}

/// The local record of `id` made through `name` (an `--execute` must name a preview
/// this connection made, under its current revision).
fn recorded(c: &McpConnection, name: &str, id: &str) -> Result<previews::PreviewRecord, CliError> {
    let root = airs_mcp_client::state_root().map_err(op)?;
    let r = previews::Previews::open(&root, name)
        .and_then(|p| p.get(id))
        .map_err(op)?
        .ok_or_else(|| {
            CliError::operational(
                "No preview record for this ID on this connection; run the command without --execute first",
            )
        })?;
    if !previews::well_formed(&r) {
        return Err(CliError::operational(
            "The local preview record is invalid and was not used; remove it and preview again",
        ));
    }
    if r.connection_revision != c.revision {
        return Err(CliError::operational(
            "The preview was made under another revision of this connection; preview again",
        ));
    }
    Ok(r)
}

pub async fn lifecycle(
    inv: &mut Invocation,
    job: LifecycleJob,
    format: Format,
) -> Result<(), CliError> {
    let c = connection_any(inv, &job.connection)?;
    let Some(id) = &job.execute else {
        let auth = auth_for(&c, &job.sign_in, &[]).await?;
        let (v, _) = preview(&c, &job.connection, &auth, job.params).await?;
        inv.output.status(&format!(
            "Preview only: nothing changed. Execute with --execute {} (expires {})",
            mc::screened(v["preview_id"].as_str().unwrap_or_default()),
            mc::screened(v["expires_at"].as_str().unwrap_or_default())
        ));
        return emit(inv, &v, format);
    };
    let r = recorded(&c, &job.connection, id)?;
    if Some(r.action.as_str()) != job.params["action"].as_str() {
        return Err(CliError::usage(
            "--execute names a preview of another action",
        ));
    }
    let mut params = json!({"preview_id": r.preview_id, "parameter_digest": r.parameter_digest});
    let auth = match (&job.approval, c.role) {
        (Some(approval), Role::Ordinary) => {
            params["approval_id"] = json!(approval);
            Auth::Session
        }
        (None, Role::Admin) => {
            let nonce = r.proof_nonce.clone().ok_or_else(|| {
                CliError::operational("The preview carries no proof nonce; preview again")
            })?;
            proof_for(&c, &job.sign_in, &nonce).await?
        }
        (Some(_), Role::Admin) => {
            return Err(CliError::usage(
                "--approval executes an approved preview on the connection that made it, not an admin connection",
            ));
        }
        (None, Role::Ordinary) => {
            return Err(CliError::usage(
                "--execute needs an admin-role connection, or --approval from 'airs mcp confirm' on the connection that made the preview",
            ));
        }
    };
    let admitted = submit(&c, &job.connection, &auth, "lifecycle_execute", params).await?;
    let root = airs_mcp_client::state_root().map_err(op)?;
    if let Ok(p) = previews::Previews::open(&root, &job.connection) {
        let _ = p.remove(id);
    }
    if !job.wait {
        emit(inv, &admitted.accepted, format)?;
        return Ok(());
    }
    let id = admitted.job_id.clone();
    let done = wait(inv, &c, &job.connection, &auth, &id, 600, "lifecycle").await?;
    emit(inv, &done, format)?;
    admitted.deliver(inv)?;
    ended(&done, "lifecycle")
}

pub struct ConfirmJob {
    pub preview_id: String,
    pub connection: String,
    pub digest: Option<String>,
    pub nonce: Option<String>,
    pub sign_in: SignIn,
}

/// `airs mcp confirm`: a one-use approval of another actor's preview, under a fresh,
/// nonce-bound administrative sign-in.
pub async fn confirm(
    inv: &mut Invocation,
    job: ConfirmJob,
    format: Format,
) -> Result<(), CliError> {
    let c = connection_any(inv, &job.connection)?;
    if c.role != Role::Admin {
        return Err(CliError::usage(
            "'airs mcp confirm' needs an admin-role connection (--role admin)",
        ));
    }
    let root = airs_mcp_client::state_root().map_err(op)?;
    let local = previews::find(&root, &job.preview_id).map_err(op)?;
    if local.as_ref().is_some_and(|r| !previews::well_formed(r)) {
        return Err(CliError::operational(
            "The local preview record is invalid and was not used; remove it and preview again",
        ));
    }
    let digest = job
        .digest
        .clone()
        .or_else(|| local.as_ref().map(|r| r.parameter_digest.clone()))
        .ok_or_else(|| {
            CliError::usage("No local record of this preview: pass --digest and --proof-nonce")
        })?;
    let nonce = job
        .nonce
        .clone()
        .or_else(|| local.as_ref().and_then(|r| r.proof_nonce.clone()))
        .ok_or_else(|| {
            CliError::usage("No local record of this preview: pass --digest and --proof-nonce")
        })?;
    if let Some(r) = &local {
        inv.output.status(&format!(
            "Approving {} preview {} (expires {})",
            mc::screened(&r.action),
            mc::screened(&r.preview_id),
            mc::screened(&r.expires_at)
        ));
    }
    let auth = proof_for(&c, &job.sign_in, &nonce).await?;
    let v = call(
        &c,
        &auth,
        "admin_confirm",
        json!({"preview_id": job.preview_id, "parameter_digest": digest}),
        None,
    )
    .await?;
    emit(inv, &v, format)
}

// ---------------------------------------------------------------------------------------
// Export and import (contract §5.1, §5.4).
// ---------------------------------------------------------------------------------------

pub async fn export(
    inv: &mut Invocation,
    name: &str,
    file: &Path,
    sign_in: SignIn,
    format: Format,
) -> Result<(), CliError> {
    let c = admin(inv, name)?;
    crate::files::check_new(file, "--file")?;
    let first = auth_for(&c, &sign_in, &[]).await?;
    let (_, r) = preview(&c, name, &first, json!({"action": "export"})).await?;
    let nonce = r.proof_nonce.clone().unwrap_or_default();
    let auth = proof_for(&c, &sign_in, &nonce).await?;
    let admitted = submit(
        &c,
        name,
        &auth,
        "lifecycle_execute",
        json!({"preview_id": r.preview_id, "parameter_digest": r.parameter_digest}),
    )
    .await?;
    let id = admitted.job_id.clone();
    let done = wait(inv, &c, name, &auth, &id, 3600, "export").await?;
    if let Err(e) = ended(&done, "export") {
        // The failed job's result is delivered before its failure exit (M7 review 2).
        emit(inv, &done, format)?;
        admitted.deliver(inv)?;
        return Err(e);
    }
    let sha = done["job"]["result"]["sha256"].as_str().map(str::to_owned);
    let mut reservation = crate::files::Reservation::reserve_new(file)?;
    let registered = mc::registry_len();
    fetch(
        inv,
        &c,
        &auth,
        "export_read",
        json!({"job_id": id}),
        sha.as_deref(),
        16 * 1024 * 1024 * 1024,
        |b| {
            reservation
                .append(b)
                .map_err(|_| crate::files::cannot_write(file))
        },
    )
    .await?;
    finish_export(reservation, file, registered)?;
    inv.output.status(&format!(
        "Export: {}",
        inv.secrets.scrub(&crate::store::shown(file))
    ));
    emit(inv, &done, format)?;
    admitted.deliver(inv)?;
    Ok(())
}

pub struct ImportJob {
    pub connection: String,
    pub file: PathBuf,
    pub expect_tsg: String,
    pub expect_environment: String,
    pub sign_in: SignIn,
}

pub async fn import(inv: &mut Invocation, job: ImportJob, format: Format) -> Result<(), CliError> {
    let c = admin(inv, &job.connection)?;
    let name = job.connection.as_str();
    let bytes =
        airs_runtime_core::files::read_bytes(job.file.as_os_str(), 16 * 1024 * 1024 * 1024)?;
    if bytes.is_empty() {
        return Err(CliError::usage("--file is empty"));
    }
    let sha = airs_mcp_client::sha256_hex(&bytes);
    let first = auth_for(&c, &job.sign_in, &[]).await?;
    let (_, admit) = preview(
        &c,
        name,
        &first,
        json!({"action": "upload_admit", "upload": {
            "kind": "logical_transfer", "total_bytes": bytes.len(), "sha256": sha,
            "expect_tsg": job.expect_tsg, "expect_environment": job.expect_environment}}),
    )
    .await?;
    let proof = proof_for(
        &c,
        &job.sign_in,
        admit.proof_nonce.as_deref().unwrap_or_default(),
    )
    .await?;
    let e = epoch(&c, &proof).await?;
    let key = new_key();
    let begun = call(
        &c,
        &proof,
        "transfer_upload_begin",
        json!({"preview_id": admit.preview_id, "parameter_digest": admit.parameter_digest}),
        Some((&key, &e)),
    )
    .await?;
    let upload = begun["upload_id"].as_str().unwrap_or_default().to_owned();
    inv.interrupt = Some(InterruptContext::Context(format!(
        "the staged upload {upload} was not imported; it expires on the service"
    )));
    let staged = async {
        let mut offset = 0usize;
        while offset < bytes.len() {
            if inv.session.is_cancelled() {
                return Err(CliError::Interrupted(InterruptContext::Context(format!(
                    "the staged upload {upload} was not imported"
                ))));
            }
            let end = (offset + CHUNK as usize).min(bytes.len());
            let part = &bytes[offset..end];
            let got = call(
                &c,
                &proof,
                "transfer_upload_chunk",
                json!({"upload_id": upload, "offset": offset, "data_base64": airs_mcp_client::b64_encode(part),
                       "chunk_sha256": airs_mcp_client::sha256_hex(part)}),
                None,
            )
            .await?;
            if got["received_bytes"].as_u64() != Some(end as u64) {
                return Err(CliError::operational("The service did not acknowledge the chunk"));
            }
            offset = end;
        }
        call(&c, &proof, "transfer_upload_finalize", json!({"upload_id": upload}), None).await
    }
    .await;
    let finalized = match staged {
        Ok(v) => v,
        Err(error) => {
            abort_upload(&c, &proof, &upload).await;
            return Err(error);
        }
    };
    if finalized["validated"] != true {
        abort_upload(&c, &proof, &upload).await;
        return Err(CliError::operational(
            "The service refused the staged file; nothing was imported",
        ));
    }
    // Every failure before execution (the final preview, the fresh sign-in) aborts the
    // staged upload too, rather than leaving it until expiry (D358; M7 review 1
    // finding 6). Execution itself is not aborted: its outcome is reconciled.
    let prepared = async {
        let (_, fin) = preview(
            &c,
            name,
            &proof,
            json!({"action": "import_finalize", "upload_id": upload}),
        )
        .await?;
        let execute = proof_for(
            &c,
            &job.sign_in,
            fin.proof_nonce.as_deref().unwrap_or_default(),
        )
        .await?;
        Ok::<_, CliError>((fin, execute))
    }
    .await;
    let (fin, execute) = match prepared {
        Ok(p) => p,
        Err(error) => {
            abort_upload(&c, &proof, &upload).await;
            return Err(error);
        }
    };
    // A submission that cannot have reached the service (not sent, or refused) aborts
    // the staged upload; an uncertain one is reconciled from its receipt (M7 review 2
    // finding 3).
    let admitted = match submit_tracked(
        &c,
        name,
        &execute,
        "lifecycle_execute",
        json!({"preview_id": fin.preview_id, "parameter_digest": fin.parameter_digest}),
    )
    .await
    {
        Ok(a) => a,
        Err((error, Sent::No | Sent::Refused)) => {
            abort_upload(&c, &proof, &upload).await;
            return Err(error);
        }
        Err((error, Sent::Uncertain)) => return Err(error),
    };
    let id = admitted.job_id.clone();
    let done = wait(inv, &c, name, &execute, &id, 3600, "import").await?;
    emit(inv, &done, format)?;
    admitted.deliver(inv)?;
    ended(&done, "import")
}

/// Release a staged upload on the service (best effort: it otherwise expires).
async fn abort_upload(c: &McpConnection, proof: &Auth, upload: &str) {
    let _ = call(
        c,
        proof,
        "transfer_upload_abort",
        json!({"upload_id": upload}),
        None,
    )
    .await;
}

fn admin(inv: &mut Invocation, name: &str) -> Result<McpConnection, CliError> {
    let c = connection_any(inv, name)?;
    if c.role != Role::Admin {
        return Err(CliError::usage(
            "This command needs an admin-role connection (--role admin): it executes an administrative action",
        ));
    }
    Ok(c)
}

// ---------------------------------------------------------------------------------------
// Receipt retention (plan M7.3; contract §4.3 `receipts_ingest`).
// ---------------------------------------------------------------------------------------

/// Normalized receipts of a bulk checkpoint, and what it cannot retain.
pub fn bulk_receipts(state: &crate::bulk::state::JobState) -> (Vec<Value>, Vec<String>) {
    use crate::bulk::ItemStatus;
    let mut out = Vec::new();
    let mut gaps = Vec::new();
    let Some(scanner) = &state.scanner_reference else {
        gaps.push(
            "the checkpoint names no scanner (--scanner); its results cannot be attributed".into(),
        );
        return (out, gaps);
    };
    let mut unreceived = 0;
    for item in &state.items {
        let (ItemStatus::Complete, Some(result)) = (item.status, &item.result) else {
            if !matches!(item.status, ItemStatus::Complete) {
                unreceived += 1;
            } else {
                gaps.push(format!("item {}: no retained result metadata", item.index));
            }
            continue;
        };
        let scan_id = result.scan_id.to_ascii_lowercase();
        if !crate::bulk::is_scan_id(&scan_id) {
            gaps.push(format!("item {}: no scan ID", item.index));
            continue;
        }
        let received_at = item
            .attempt
            .and_then(|a| state.attempts.iter().find(|x| x.id == a))
            .and_then(|a| a.ended_at.clone())
            .unwrap_or_else(|| state.updated_at.clone());
        let (status, action) = match result.action {
            crate::bulk::state::Action::Allow => ("complete", json!("allow")),
            crate::bulk::state::Action::Block => ("complete", json!("block")),
            crate::bulk::state::Action::Failed => ("failed", Value::Null),
        };
        out.push(receipt(
            &format!("bulk:{}:{}", state.job_id, item.req_id),
            &scanner.name,
            &scanner.application_id,
            &scan_id,
            &result.report_id,
            &result.category,
            status,
            action,
            &received_at,
            Some(&state.session_id),
            Some(item.req_id),
        ));
    }
    if unreceived > 0 {
        gaps.push(format!("{unreceived} item(s) have no received result yet"));
    }
    (out, gaps)
}

/// One `client_asserted` receipt (contract `$defs/receipt`): never an application
/// name; withheld or empty text is null.
#[allow(clippy::too_many_arguments)]
pub fn receipt(
    key: &str,
    scanner: &str,
    application: &str,
    scan_id: &str,
    report_id: &str,
    category: &str,
    status: &str,
    action: Value,
    received_at: &str,
    session_id: Option<&str>,
    sub_request: Option<u64>,
) -> Value {
    let text =
        |t: &str| (!t.is_empty() && t != airs_runtime_core::error::WITHHELD).then(|| t.to_owned());
    let category = match category {
        "benign" | "malicious" => category,
        _ => "unknown",
    };
    json!({
        "receipt_key": key,
        "declared_scanner": scanner,
        "application_id": text(application),
        "application_name": null,
        "identity_withheld": false,
        "scan_id": scan_id,
        "report_id": text(report_id),
        "session_id": session_id.and_then(text),
        "sub_request": sub_request,
        "status": status,
        "category": category,
        "action": action,
        "received_at": received_at,
    })
}

/// Room left for what the transport adds beyond the JSON-RPC request this module
/// measures (a progress token or other `_meta`).
const REQUEST_SLACK: usize = 2 * 1024;

/// The complete serialized `tools/call` request that carries `params` under `key`: the
/// JSON-RPC wrapper, the call envelope (schema version, tenant, key, epoch) and the
/// parameters, as sent (M7 review 9).
fn request_bytes(tenant: &str, key: &str, epoch: &str, params: &Value) -> usize {
    let arguments = airs_mcp_client::envelope(tenant, params.clone(), Some((key, epoch)));
    serde_json::to_vec(
        &json!({"jsonrpc": "2.0", "id": u64::MAX, "method": "tools/call",
        "params": {"name": "receipts_ingest", "arguments": arguments}}),
    )
    .map_or(usize::MAX, |b| b.len())
}

/// Batches of at most 200 records whose *complete request* (with [`REQUEST_SLACK`])
/// fits the service's 256 KiB request limit, each under a deterministic key, so a retry
/// of the same batch is the same submission, never a duplicate. A record too large to be
/// sent even alone is never sent; the second value counts them (M7 review 9).
///
/// Linear time (M7 review 10): each record is serialized once, and a batch's request
/// size is the fixed framing (measured once, with a key of the final length) plus its
/// records and their separators, exactly, since the parameters are embedded as JSON
/// rather than as an escaped string. Keys are computed only when a batch is final.
pub fn batch_plan(
    tenant: &str,
    epoch: &str,
    cloud_id: &str,
    records: &[Value],
) -> (Vec<(String, Value)>, u64) {
    let key_of = |params: &Value| {
        let digest = airs_mcp_client::sha256_hex(&serde_json::to_vec(params).unwrap_or_default());
        format!("rcpt-{}", &digest[..40])
    };
    // The request with an empty records array, under a key of the final length.
    let empty = json!({"cloud_id": cloud_id, "records": []});
    let framing = request_bytes(tenant, &format!("rcpt-{}", "0".repeat(40)), epoch, &empty);
    let budget = BATCH_BYTES.saturating_sub(REQUEST_SLACK);
    let mut out = Vec::new();
    let mut oversized = 0;
    let mut current: Vec<Value> = Vec::new();
    let mut size = framing;
    let flush = |current: &mut Vec<Value>, out: &mut Vec<(String, Value)>| {
        if current.is_empty() {
            return;
        }
        let params = json!({"cloud_id": cloud_id, "records": std::mem::take(current)});
        out.push((key_of(&params), params));
    };
    for r in records {
        let n = serde_json::to_vec(r).map_or(usize::MAX, |b| b.len());
        if framing.saturating_add(n) > budget {
            oversized += 1;
            continue;
        }
        // A separator before every record but the first.
        let added = n + usize::from(!current.is_empty());
        if current.len() == BATCH_RECORDS || size + added > budget {
            flush(&mut current, &mut out);
            size = framing;
            current.push(r.clone());
            size += n;
        } else {
            current.push(r.clone());
            size += added;
        }
    }
    flush(&mut current, &mut out);
    (out, oversized)
}

/// Totals of an upload.
#[derive(Default, Debug)]
pub struct Retained {
    pub accepted: u64,
    pub duplicates: u64,
    pub refused: u64,
    pub unsent: u64,
    /// Records never sent because they carry a known secret (M7 review 2 finding 2).
    pub withheld: u64,
    /// Records never sent because one alone exceeds the request limit (M7 review 9).
    pub oversized: u64,
}

/// Send `records` of the selected tenant's cloud on `c` (no interactive sign-in;
/// `deadline` bounds the whole wait).
async fn retain(
    inv: &mut Invocation,
    c: &McpConnection,
    records: &[Value],
    deadline: Option<std::time::Instant>,
) -> Result<Retained, CliError> {
    let records_in = records.len();
    let context = inv.load_tenant()?;
    let (tsg, _env, cloud_id) = airs_runtime_core::cloud::of_tenant(&context)?;
    if tsg != c.expected_tsg {
        return Err(CliError::operational(
            "The selected tenant's TSG is not the connection's expected TSG; nothing was sent",
        ));
    }
    let mut totals = Retained::default();
    // No outgoing record may carry a secret the invocation knows (a credential that
    // came back as a scan or report ID): it is withheld, counted, and never sent, as
    // stdout withholds it (M7 review 2 finding 2).
    let records: Vec<Value> = records
        .iter()
        .filter(|r| !mc::carries_secret(&r.to_string()))
        .cloned()
        .collect();
    let records = records.as_slice();
    totals.withheld = (records_in - records.len()) as u64;
    let e = epoch(c, &Auth::Session).await?;
    let (all, oversized) = batch_plan(&c.service_tenant, &e, &cloud_id, records);
    totals.oversized = oversized;
    let mut sent = 0;
    for (key, params) in &all {
        let n = params["records"].as_array().map_or(0, Vec::len) as u64;
        let send = mc::outcome(
            c,
            &Auth::Session,
            "receipts_ingest",
            params.clone(),
            Some((key, &e)),
        );
        let called = match deadline {
            Some(d) => match tokio::time::timeout_at(d.into(), send).await {
                Ok(r) => r?,
                Err(_) => break,
            },
            None => send.await?,
        };
        match called {
            Called::Ok(v) => {
                totals.accepted += v["accepted"].as_u64().unwrap_or(0);
                totals.duplicates += v["duplicates"].as_u64().unwrap_or(0);
                totals.refused += v["refused"].as_array().map_or(0, Vec::len) as u64;
            }
            Called::Refused(r) => return Err(refused(r)),
        }
        sent += n;
    }
    totals.unsent = records.len() as u64 - oversized - sent;
    Ok(totals)
}

/// `runtime receipts upload --from-bulk-state PATH`.
pub async fn upload(
    inv: &mut Invocation,
    name: &str,
    path: &Path,
    format: Format,
) -> Result<(), CliError> {
    let c = connection(inv, name)?;
    let bytes = airs_runtime_core::files::read_bytes(path.as_os_str(), 256 * 1024 * 1024)?;
    let state = crate::bulk::state::parse(&bytes)
        .map_err(|_| CliError::operational("The bulk-scan state file is not a valid checkpoint"))?;
    let (records, gaps) = bulk_receipts(&state);
    let totals = if records.is_empty() {
        Retained::default()
    } else {
        retain(inv, &c, &records, None).await?
    };
    emit(
        inv,
        &json!({"records": records.len(), "accepted": totals.accepted, "duplicates": totals.duplicates,
                "refused": totals.refused, "unsent": totals.unsent, "withheld": totals.withheld,
                "oversized": totals.oversized, "gaps": gaps}),
        format,
    )
}

/// After `scan`/`bulk-scan`/`resume-poll --record-history CONNECTION`: retain what the
/// command received, within [`RECORD_BUDGET`]; a failure warns once and never changes
/// the command's outcome.
pub async fn record_history(inv: &mut Invocation, name: &str) {
    mc::bind_guard(inv);
    let records = std::mem::take(&mut inv.retained);
    if records.is_empty() {
        return;
    }
    let deadline = std::time::Instant::now() + RECORD_BUDGET;
    let result = async {
        let c = connection(inv, name)?;
        tokio::time::timeout_at(deadline.into(), retain(inv, &c, &records, Some(deadline)))
            .await
            .unwrap_or(Ok(Retained {
                unsent: records.len() as u64,
                ..Retained::default()
            }))
    }
    .await;
    let message = match result {
        Ok(t) if t.unsent == 0 && t.refused == 0 && t.withheld == 0 && t.oversized == 0 => return,
        Ok(t) => format!(
            "{} result(s) were not retained on the MCP service ({} unsent, {} refused, {} withheld because they carry a credential, {} too large to send); retry with 'airs runtime receipts upload'",
            t.unsent + t.refused + t.withheld + t.oversized,
            t.unsent,
            t.refused,
            t.withheld,
            t.oversized
        ),
        Err(e) => format!(
            "The results were not retained on the MCP service: {}",
            e.message()
        ),
    };
    let message = inv.secrets.scrub(&message).to_owned();
    inv.output.warning(&message);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tests that bind the process-wide secret registry run one at a time.
    static REGISTRY: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// M7 review 9 minor: each publication step refuses a file carrying a token that was
    /// registered only after its bytes were read, and writes nothing. Bundle (a later
    /// file revealed the token), single report file, and streamed export (its rescan).
    #[test]
    fn publication_refuses_a_token_registered_after_the_bytes_were_read() {
        let _serial = REGISTRY
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let set = airs_runtime_core::secrets::SecretSet::default();
        mc::bind_secrets_for_tests(set.clone());
        let base = std::env::temp_dir().join(format!("airs-pub-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&base).unwrap();
        let token = format!("late-token-{}", "q".repeat(40));
        let early = format!("<html>{token}</html>").into_bytes();
        // Bundle: the token is registered after every file was fetched.
        let dir = base.join("bundle");
        std::fs::create_dir(&dir).unwrap();
        let fetched = vec![
            (dir.join("report.html"), early.clone()),
            (dir.join("review.md"), b"ok".to_vec()),
        ];
        set.add(&token);
        assert!(publish_bundle(&dir, fetched, "--bundle-dir").is_err());
        assert!(!dir.exists(), "no file and no directory remain");
        // Single report file.
        let single = base.join("report.html");
        assert!(publish_single(&single, &early).is_err());
        assert!(!single.exists());
        // Streamed export: registered while it streamed.
        let other = format!("export-token-{}", "w".repeat(40));
        let file = base.join("archive.jsonl");
        let mut reservation = crate::files::Reservation::reserve_new(&file).unwrap();
        let registered = mc::registry_len();
        reservation
            .append(format!("{{\"x\":\"{other}\"}}\n").as_bytes())
            .unwrap();
        set.add(&other);
        assert!(finish_export(reservation, &file, registered).is_err());
        assert!(!file.exists());
        // Clean controls publish.
        let clean_dir = base.join("clean");
        std::fs::create_dir(&clean_dir).unwrap();
        assert!(
            publish_bundle(
                &clean_dir,
                vec![(clean_dir.join("review.md"), b"ok".to_vec())],
                "--bundle-dir"
            )
            .is_ok()
        );
        assert!(publish_single(&base.join("clean.html"), b"<html>ok</html>").is_ok());
        let ok_file = base.join("ok.jsonl");
        let mut r = crate::files::Reservation::reserve_new(&ok_file).unwrap();
        let before = mc::registry_len();
        r.append(b"{}\n").unwrap();
        set.add(&format!("unrelated-{}", "u".repeat(40)));
        assert!(finish_export(r, &ok_file, before).is_ok());
        assert!(ok_file.exists());
        mc::bind_secrets_for_tests(airs_runtime_core::secrets::SecretSet::default());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// M7 review 8 finding 1: a longer token registered only after the first chunk was
    /// screened is still found across the boundary (the tail keeps MIN_OVERLAP bytes),
    /// and the final whole-file screen sees the registry as it is at publication.
    #[test]
    fn a_token_registered_mid_transfer_is_still_found() {
        let _serial = REGISTRY
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let set = airs_runtime_core::secrets::SecretSet::default();
        mc::bind_secrets_for_tests(set.clone());
        let token = format!("late-{}", "t".repeat(2000));
        let (a, b) = token.split_at(1500);
        let first = format!("{}{a}", "x".repeat(70_000));
        let second = format!("{b}{}", "y".repeat(100));
        let mut tail = Vec::new();
        // The first chunk is screened before the token exists.
        let overlap = mc::longest_secret().max(MIN_OVERLAP);
        assert!(screened_window(
            &mut tail,
            first.as_bytes(),
            overlap,
            mc::carries_secret
        ));
        // A chunk request registers the longer token.
        set.add(&token);
        let overlap = mc::longest_secret().max(MIN_OVERLAP);
        assert!(!screened_window(
            &mut tail,
            second.as_bytes(),
            overlap,
            mc::carries_secret
        ));
        // The whole-file screen before publication, against the final registry.
        let whole = format!("{first}{second}");
        assert!(!clean(whole.as_bytes()));
        assert!(clean(b"benign report"));
        mc::bind_secrets_for_tests(airs_runtime_core::secrets::SecretSet::default());
    }

    #[test]
    fn a_secret_split_across_chunks_is_found() {
        let secret = "0f0e0d0c-split-token";
        let carries = |t: &str| t.contains(secret);
        for cut in 1..secret.len() {
            let (a, b) = secret.split_at(cut);
            let first = format!("{}{a}", "x".repeat(100));
            let second = format!("{b}{}", "y".repeat(100));
            let mut tail = Vec::new();
            assert!(screened_window(
                &mut tail,
                first.as_bytes(),
                secret.len(),
                carries
            ));
            assert!(
                !screened_window(&mut tail, second.as_bytes(), secret.len(), carries),
                "split at {cut}"
            );
        }
        // Clean chunks pass, and the tail stays bounded.
        let mut tail = Vec::new();
        for _ in 0..10 {
            assert!(screened_window(
                &mut tail,
                &[b'z'; 1000],
                secret.len(),
                carries
            ));
            assert_eq!(tail.len(), secret.len() - 1);
        }
    }

    #[test]
    fn a_manifest_entry_needs_a_valid_hash_and_length() {
        let sha = "a".repeat(64);
        let m = |entry: Value| json!({"files": [entry]});
        assert_eq!(
            manifest_entry(
                &m(json!({"name": "report.html", "sha256": sha, "bytes": 10})),
                "report.html"
            )
            .unwrap(),
            (sha.clone(), 10)
        );
        for bad in [
            json!({"name": "other.html", "sha256": sha, "bytes": 10}),
            json!({"name": "report.html", "bytes": 10}),
            json!({"name": "report.html", "sha256": "A".repeat(64), "bytes": 10}),
            json!({"name": "report.html", "sha256": "a".repeat(63), "bytes": 10}),
            json!({"name": "report.html", "sha256": sha}),
            json!({"name": "report.html", "sha256": sha, "bytes": -1}),
            json!({"name": "report.html", "sha256": sha, "bytes": REPORT_FILE_CAP + 1}),
        ] {
            assert!(
                manifest_entry(&m(bad.clone()), "report.html").is_err(),
                "{bad}"
            );
        }
    }

    fn record(i: usize, pad: usize) -> Value {
        receipt(
            &format!("bulk:job:{i}"),
            "monitor",
            "app-a",
            "00000000-0000-4000-8000-000000000001",
            &"r".repeat(pad),
            "malicious",
            "complete",
            json!("block"),
            "2026-11-06T00:00:00.000Z",
            None,
            Some(i as u64),
        )
    }

    /// Batches hold at most 200 records, cover every record once in order, and their
    /// keys are deterministic (a retry is the same submission).
    #[test]
    fn batches_are_bounded_ordered_and_deterministically_keyed() {
        let plan = |cloud: &str, r: &[Value]| batch_plan("tenant-a", "0a1b2c3d", cloud, r);
        let records: Vec<Value> = (0..450).map(|i| record(i, 10)).collect();
        let (b, oversized) = plan(&"c".repeat(64), &records);
        assert_eq!(oversized, 0);
        assert_eq!(
            b.iter()
                .map(|(_, p)| p["records"].as_array().unwrap().len())
                .collect::<Vec<_>>(),
            vec![200, 200, 50]
        );
        assert_eq!(
            (b.clone(), 0),
            plan(&"c".repeat(64), &records),
            "deterministic keys"
        );
        assert!(
            b.iter()
                .all(|(k, _)| k.len() == 45 && k.starts_with("rcpt-"))
        );
        assert_ne!(
            plan(&"d".repeat(64), &records).0[0].0,
            b[0].0,
            "the cloud is part of the key"
        );
    }

    /// M7 review 9: every batch's complete serialized request (JSON-RPC wrapper and call
    /// envelope included) fits the 256 KiB limit with its slack, even when the records
    /// array alone is just under it; a record too large to send alone is counted, never
    /// sent, and the rest still go.
    /// M7 review 10: planning is linear. The largest supported input (10,000 records,
    /// each near the per-record limits) plans well inside the 2 s retention budget, and
    /// every planned request still fits.
    #[test]
    fn planning_the_largest_input_fits_the_retention_budget() {
        let records: Vec<Value> = (0..10_000).map(|i| record(i, 1500)).collect();
        // The cost of serializing every record once, on this machine and build: linear
        // planning is a small multiple of it (the earlier, quadratic planner was about a
        // hundred times it), so the bound holds under load and in debug builds.
        let started = std::time::Instant::now();
        let once: usize = records
            .iter()
            .map(|r| serde_json::to_vec(r).unwrap().len())
            .sum();
        let unit = started.elapsed();
        assert!(once > 0);
        let started = std::time::Instant::now();
        let (b, oversized) = batch_plan("tenant-a", "0a1b2c3d", &"c".repeat(64), &records);
        let took = started.elapsed();
        assert!(
            took < unit * 8 + std::time::Duration::from_millis(100),
            "{took:?} vs {unit:?}"
        );
        assert_eq!(oversized, 0);
        assert_eq!(
            b.iter()
                .map(|(_, p)| p["records"].as_array().unwrap().len())
                .sum::<usize>(),
            10_000
        );
        for (key, params) in &b {
            assert!(
                request_bytes("tenant-a", key, "0a1b2c3d", params) + REQUEST_SLACK <= BATCH_BYTES
            );
        }
    }

    #[test]
    fn every_batch_request_fits_the_limit_and_oversized_records_are_counted() {
        let (tenant, epoch, cloud) = ("tenant-a", "0a1b2c3d", "c".repeat(64));
        // Records whose array alone lands just under the limit.
        let mut near: Vec<Value> = Vec::new();
        while serde_json::to_vec(&near).unwrap().len() < BATCH_BYTES - 4 * 1024 {
            near.push(record(near.len(), 2000));
        }
        while serde_json::to_vec(&near).unwrap().len() < BATCH_BYTES - 1000 {
            near.push(record(near.len(), 10));
        }
        let array = serde_json::to_vec(&near).unwrap().len();
        assert!(array < BATCH_BYTES, "{array}");
        let total = near.len();
        assert!(total <= BATCH_RECORDS, "the split is by size, not by count");
        let (b, oversized) = batch_plan(tenant, epoch, &cloud, &near);
        assert_eq!(oversized, 0);
        assert!(
            b.len() >= 2,
            "the array fits but the request does not: split"
        );
        for (key, params) in &b {
            assert!(request_bytes(tenant, key, epoch, params) + REQUEST_SLACK <= BATCH_BYTES);
        }
        assert_eq!(
            b.iter()
                .map(|(_, p)| p["records"].as_array().unwrap().len())
                .sum::<usize>(),
            total
        );
        // One record alone over the limit, between two small ones.
        let mixed = vec![record(0, 10), record(1, 300 * 1024), record(2, 10)];
        let (b, oversized) = batch_plan(tenant, epoch, &cloud, &mixed);
        assert_eq!(oversized, 1);
        let sent: Vec<&Value> = b
            .iter()
            .flat_map(|(_, p)| p["records"].as_array().unwrap())
            .collect();
        assert_eq!(sent.len(), 2);
        assert!(sent.iter().all(|r| r["receipt_key"] != "bulk:job:1"));
    }

    /// A receipt never carries an application name; withheld or empty text is null;
    /// an unrecognised category is unknown, never benign.
    #[test]
    fn receipts_are_normalized_client_assertions() {
        let r = receipt(
            "scan:x1234567",
            "monitor",
            "",
            "00000000-0000-4000-8000-000000000001",
            airs_runtime_core::error::WITHHELD,
            "suspicious",
            "complete",
            json!("allow"),
            "2026-11-06T00:00:00.000Z",
            Some(""),
            None,
        );
        assert_eq!(r["application_name"], Value::Null);
        assert_eq!(r["application_id"], Value::Null);
        assert_eq!(r["report_id"], Value::Null);
        assert_eq!(r["session_id"], Value::Null);
        assert_eq!(r["category"], "unknown");
        assert_eq!(r["identity_withheld"], false);
    }
}
