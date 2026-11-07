//! Submission, polling and result correlation (DESIGN §14.6–§14.8).
//!
//! Every POST is preceded by a durable C1 checkpoint (its items `submitting`, a new
//! attempt `inFlight`) and followed by a durable C3 checkpoint of its outcome before
//! anything else happens. `NotSent` and definitive rejections leave the items
//! `pending`; an outcome that may have reached AIRS leaves them `ambiguous`, and the CLI
//! never submits them again. Polls are idempotent reads, bounded by the TypeScript
//! no-progress rule; results are checkpointed per poll response, with the CSV rewritten
//! after the state (never ahead of it).

use super::checkpoint::{Consequence, StateFile};
use super::lock::JobLock;
use super::output::{OutputFile, Rewritten};
use super::state::{Action, Attempt, Detections, ItemResult, JobState};
use super::{AttemptOutcome, ItemStatus, PROGRAM, is_scan_id, ranges};
use crate::clients::LastStatus;
use crate::error::{
    self, CliError, ErrorFacts, Failure, InterruptContext, Operation, Phase, Plane, SdkContext,
    SecretSet,
};
use crate::output::Output;
use scm::RemoteOutcome;
use scm::airs::scan::Scanner;
use scm::airs::scan::models::{
    AsyncScanObject, AsyncScanResponse, FiniteNumber, Optional, ScanIdResult, ThreatScanReport,
};
use std::collections::BTreeSet;
use std::time::Duration;

/// The deadline of every submission and poll request.
pub const REQUEST_BUDGET: Duration = Duration::from_secs(60);
/// Rate-limit retries per chunk (submissions) or per level (polls).
pub const MAX_RETRIES: u32 = 5;
/// The cap of a server-requested wait (D124).
pub const RETRY_AFTER_CAP: Duration = Duration::from_secs(60);

/// The waits and bounds of the TypeScript poller.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub poll_interval: Duration,
    pub base_delay: Duration,
    pub max_no_progress: u32,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            base_delay: Duration::from_secs(10),
            max_no_progress: 120,
        }
    }
}

/// The safe facts of a failed call.
#[derive(Clone)]
pub struct CallError {
    pub facts: Box<ErrorFacts>,
    pub retry_after: Option<Duration>,
}

impl CallError {
    pub fn of(error: &scm::Error) -> Self {
        Self {
            facts: Box::new(ErrorFacts::of(error)),
            retry_after: error.retry_after(),
        }
    }

    /// An HTTP 429 (also with a body over the client limit, G2; D125: never by text).
    fn rate_limited(&self) -> bool {
        self.facts.http_status() == Some(429)
    }

    fn cancelled(&self) -> bool {
        self.facts.kind == scm::ErrorKind::Cancelled
    }
}

/// The three async scan calls (the SDK scanner; a fake in unit tests).
pub trait Api {
    /// Secondary local metadata may flush only after essential job state is durable.
    fn checkpointed(&self, _state: &JobState) {}
    /// One POST of `objects`, and the HTTP status of its response when one arrived.
    async fn submit(
        &self,
        objects: &[AsyncScanObject],
    ) -> (Result<AsyncScanResponse, CallError>, Option<u16>);
    async fn results(&self, scan_id: &str) -> Result<Vec<ScanIdResult>, CallError>;
    async fn reports(&self, ids: &[String]) -> Result<Vec<ThreatScanReport>, CallError>;
}

/// The SDK scanner with fresh per-call options: a 60 s deadline, and no SDK retry on
/// the polls (TS sends exactly one attempt; the CLI runs its own 429 loop).
pub struct ScannerApi {
    pub scanner: Scanner,
    pub last_status: LastStatus,
}

impl Api for ScannerApi {
    async fn submit(
        &self,
        objects: &[AsyncScanObject],
    ) -> (Result<AsyncScanResponse, CallError>, Option<u16>) {
        let options = scm::RequestOptions::with_timeout(REQUEST_BUDGET).ok();
        let result = self.scanner.async_scan(objects, options.as_ref()).await;
        (
            result.map_err(|e| CallError::of(&e)),
            self.last_status.get(),
        )
    }

    async fn results(&self, scan_id: &str) -> Result<Vec<ScanIdResult>, CallError> {
        let options = read_options();
        self.scanner
            .query_by_scan_ids(&[scan_id.to_owned()], options.as_ref())
            .await
            .map_err(|e| CallError::of(&e))
    }

    async fn reports(&self, ids: &[String]) -> Result<Vec<ThreatScanReport>, CallError> {
        let options = read_options();
        self.scanner
            .query_by_report_ids(ids, options.as_ref())
            .await
            .map_err(|e| CallError::of(&e))
    }
}

fn read_options() -> Option<scm::RequestOptions> {
    scm::RequestOptions::with_timeout(REQUEST_BUDGET)
        .and_then(|options| options.with_retries(0))
        .ok()
}

/// A running job: its state, the state file and the output file.
pub struct Job {
    pub state: JobState,
    pub file: StateFile,
    pub output: OutputFile,
    /// The job lock (§14.11), once the job exists; checked before every C1.
    pub lock: Option<JobLock>,
}

impl Job {
    pub fn state_shown(&self) -> String {
        self.file.shown()
    }

    /// `<s> of <n> prompts were submitted; the job was saved: resume it with …`.
    pub fn saved_context(&self) -> InterruptContext {
        InterruptContext::Bulk(format!(
            "{} of {} prompts were submitted; the job was saved: resume it with '{PROGRAM} runtime resume-poll {}'",
            self.state.submitted_count(),
            self.state.items.len(),
            self.state_shown()
        ))
    }

    /// The detail line of a stop that left nothing in doubt.
    pub fn resume_detail(&self) -> String {
        format!(
            "The job was saved; resume it with '{PROGRAM} runtime resume-poll {}'",
            self.state_shown()
        )
    }

    fn ambiguous_detail(&self, ranges: &str) -> String {
        format!(
            "Prompts {ranges} will not be resubmitted; the job was saved in {}",
            self.state_shown()
        )
    }

    pub fn checkpoint(&mut self, consequence: &Consequence) -> Result<(), CliError> {
        self.file.checkpoint(&mut self.state, consequence)
    }

    /// Stop before a submission when the held job lock is no longer the file at
    /// `<job>.lock` (removed or replaced while held, so another `resume-poll` could take
    /// a new one and run this job too; §14.11). Nothing of this chunk is sent.
    pub fn verify_lock(&self) -> Result<(), CliError> {
        match &self.lock {
            Some(lock) if !lock.still_held(self.file.dir()) => {
                Err(CliError::Operational(Failure {
                    message: lock.removed_message(),
                    status: None,
                    details: vec![self.resume_detail()],
                }))
            }
            _ => Ok(()),
        }
    }

    /// Release the job lock (after the last CSV write).
    pub fn release_lock(&mut self) {
        if let Some(lock) = self.lock.take() {
            lock.release();
        }
    }

    /// A checkpoint that carries a CSV rewrite (§14.10, §14.12): the CSV temporary
    /// first, `output.next` recorded by the checkpoint, then the rename. When the
    /// temporary cannot be written, the results are still checkpointed (without
    /// `output.next`) before the CSV failure is reported, so the state never depends on
    /// whether the CSV is writable and the failure's "results are saved" is true.
    pub fn save_with_output(
        &mut self,
        out: &mut Output,
        secrets: &SecretSet,
    ) -> Result<(), CliError> {
        self.save_with_output_observed(out, secrets, |_| {})
    }

    fn save_with_output_observed(
        &mut self,
        out: &mut Output,
        secrets: &SecretSet,
        after: impl FnOnce(&JobState),
    ) -> Result<(), CliError> {
        let shown = self.state_shown();
        let bytes = super::output::projection(&self.state);
        let prepared = match self.output.prepare(bytes.as_bytes(), &shown) {
            Ok(prepared) => prepared,
            Err(csv_failure) => {
                self.state.output.next = None;
                self.checkpoint(&Consequence::Results)?;
                after(&self.state);
                return Err(csv_failure);
            }
        };
        self.state.output.next = Some(OutputFile::next_record(&prepared));
        if let Err(error) = self.checkpoint(&Consequence::Results) {
            self.output.abandon(prepared);
            return Err(error);
        }
        let rewritten = self.output.commit(prepared, &mut self.state, &shown);
        after(&self.state);
        if let Rewritten::Warning(text) = rewritten? {
            out.warning(secrets.scrub(&text));
        }
        Ok(())
    }

    /// `(total rows, blocked, allowed, failed)`.
    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let mut counts = (0, 0, 0, 0);
        for result in self
            .state
            .items
            .iter()
            .filter_map(|item| item.result.as_ref())
        {
            counts.0 += 1;
            match result.action {
                Action::Block => counts.1 += 1,
                Action::Allow => counts.2 += 1,
                Action::Failed => counts.3 += 1,
            }
        }
        counts
    }
}

/// How a POST attempt ended, as the CLI acts on it (§14.6 table).
enum Outcome {
    Accepted {
        scan_id: String,
        report_id: Option<String>,
    },
    Unusable,
    RateLimited(CallError),
    Rejected(CallError),
    NotSent(CallError),
    Unknown(CallError),
}

fn classify(error: CallError, retries_left: bool) -> Outcome {
    let four_xx = error
        .facts
        .http_status()
        .is_some_and(|status| (400..500).contains(&status));
    if error.cancelled() {
        if error.facts.remote_outcome == RemoteOutcome::NotSent {
            return Outcome::NotSent(error);
        }
        return Outcome::Unknown(error);
    }
    if error.rate_limited() && retries_left {
        return Outcome::RateLimited(error);
    }
    if four_xx || error.facts.remote_outcome == RemoteOutcome::Rejected {
        return Outcome::Rejected(error);
    }
    if error.facts.remote_outcome == RemoteOutcome::NotSent {
        return Outcome::NotSent(error);
    }
    Outcome::Unknown(error)
}

/// `ms / 1000` rounded half up (TS `toFixed(0)`).
fn seconds(delay: Duration) -> String {
    crate::commands::topic_eval::to_fixed(delay.as_millis() as f64 / 1000.0, 0)
}

/// The wait before a rate-limited retry `n` (1-based): the server's `Retry-After`
/// capped at 60 s, else `base × 2^(n−1)`.
pub fn retry_delay(retry_after: Option<Duration>, n: u32, base: Duration) -> Duration {
    match retry_after {
        Some(delay) => delay.min(RETRY_AFTER_CAP),
        None => base.saturating_mul(1u32 << n.saturating_sub(1).min(16)),
    }
}

/// The input index a `req_id` names: a finite number with an integral value in
/// `0..=2^53` (so `2.0` is request 2; A§5 G7).
pub fn request_index(number: &FiniteNumber) -> Option<u64> {
    const LIMIT: u64 = 1 << 53;
    if let Some(value) = number.as_u64() {
        return (value <= LIMIT).then_some(value);
    }
    let value = number.as_f64()?;
    (value.is_finite() && value >= 0.0 && value <= LIMIT as f64 && value.fract() == 0.0)
        .then_some(value as u64)
}

/// TS `String(reqId)` for the report correlation message.
fn request_text(number: &Optional<FiniteNumber>) -> String {
    match number {
        Optional::Value(number) => match (number.as_i64(), number.as_f64()) {
            (Some(value), _) => value.to_string(),
            (None, Some(value)) => crate::render::js_number(value),
            (None, None) => "undefined".to_owned(),
        },
        Optional::Unset => "undefined".to_owned(),
    }
}

/// TS `REPORT_DETECTION_KEYS`.
fn report_key(service: &str) -> Option<&'static str> {
    Some(match service {
        "topic_guardrails" | "topic_violation" => "topic_violation",
        "pi" | "prompt_injection" | "injection" => "injection",
        "tc" | "toxic_content" => "toxic_content",
        "dlp" => "dlp",
        "uf" | "url_filtering" | "url_cats" => "url_cats",
        "mc" | "malicious_code" => "malicious_code",
        "source_code" => "source_code",
        "agent" => "agent",
        _ => return None,
    })
}

/// A failed row (TS `failedBulkResult`).
fn failed_row(scan_id: &str) -> ItemResult {
    ItemResult {
        action: Action::Failed,
        category: "error".to_owned(),
        triggered: false,
        detections: Detections::default(),
        scan_id: scan_id.to_owned(),
        report_id: String::new(),
        error: Some("AIRS async scan failed".to_owned()),
    }
}

/// A per-request result through the §8.3 normaliser, prompt detections only.
fn scan_row(
    response: &scm::airs::scan::models::ScanResponse,
    scan_id: &str,
    secrets: &SecretSet,
) -> ItemResult {
    let normalized = crate::commands::scan::normalize(response, secrets);
    let detections = Detections(
        normalized
            .prompt
            .0
            .iter()
            .map(|(key, value)| ((*key).to_owned(), *value))
            .collect(),
    );
    ItemResult {
        action: match normalized.verdict {
            crate::commands::scan::Verdict::Allow => Action::Allow,
            crate::commands::scan::Verdict::Block => Action::Block,
            crate::commands::scan::Verdict::Failed => Action::Failed,
        },
        category: normalized.category,
        triggered: normalized.prompt.triggered().next().is_some(),
        detections,
        scan_id: scan_id.to_owned(),
        report_id: secrets.scrub(&response.report_id).to_owned(),
        error: normalized.error,
    }
}

/// A threat report as a row (TS `threatReportToResult`).
fn report_row(report: &ThreatScanReport, scan_id: &str, secrets: &SecretSet) -> ItemResult {
    let mut detections = Detections::default();
    let mut blocked = false;
    let mut unexpected: Option<String> = None;
    let text = |value: &Optional<String>| match value {
        Optional::Value(text) => text.to_ascii_lowercase(),
        Optional::Unset => String::new(),
    };
    if let Optional::Value(results) = &report.detection_results {
        for detection in results {
            let service = text(&detection.detection_service);
            let action = text(&detection.action);
            let verdict = text(&detection.verdict);
            let fired = action == "block"
                || matches!(
                    verdict.as_str(),
                    "malicious" | "unsafe" | "violation" | "detected"
                );
            let key = match report_key(&service) {
                Some(key) => key.to_owned(),
                None if !service.is_empty() => service.clone(),
                None if fired => "unknown".to_owned(),
                None => String::new(),
            };
            // The key is lower-cased server text: compare it ignoring case too.
            if !key.is_empty() && !secrets.taints(&key) && !secrets.appears_in_ignoring_case(&key) {
                let previous = detections.get(&key) == Some(true);
                detections.set(&key, previous || fired);
            }
            if action == "block" {
                blocked = true;
            } else if !action.is_empty() && action != "allow" {
                unexpected = Some(action);
            }
        }
    }
    let triggered = detections.0.iter().any(|(_, on)| *on);
    let action = if blocked {
        Action::Block
    } else if unexpected.is_some() {
        Action::Failed
    } else {
        Action::Allow
    };
    let report_id = match &report.report_id {
        Optional::Value(id) => secrets.scrub(id).to_owned(),
        Optional::Unset => String::new(),
    };
    ItemResult {
        action,
        category: match action {
            Action::Failed => "error",
            _ if triggered => "malicious",
            _ => "benign",
        }
        .to_owned(),
        triggered,
        detections,
        scan_id: scan_id.to_owned(),
        report_id,
        error: (action == Action::Failed).then(|| {
            format!(
                "Unknown AIRS action in threat report: {}",
                crate::commands::scan::reason_part(unexpected.as_deref().unwrap_or(""), secrets)
            )
        }),
    }
}

/// What one results response resolved, the report id of a terminal batch row, and the
/// correlation error that stopped it (the rows resolved before it are kept, as
/// TypeScript saves each row before it throws).
#[derive(Debug, Default)]
pub struct Correlated {
    pub resolved: Vec<(u64, ItemResult)>,
    pub report_id: Option<String>,
    pub error: Option<String>,
}

#[cfg(test)]
impl Correlated {
    /// The correlation message that stopped the response.
    fn unwrap_err(self) -> String {
        self.error.expect("a correlation error")
    }

    /// `self`, asserting that no correlation error stopped it.
    fn unwrap(self) -> Self {
        assert_eq!(self.error, None);
        self
    }
}

/// §14.7 step 2 for the rows of one response. A correlation error stops at its row and
/// is returned in `error`, with the rows resolved before it.
pub fn correlate_rows(
    rows: &[ScanIdResult],
    scan_id: &str,
    receipt_report: Option<&str>,
    unresolved: &BTreeSet<u64>,
    secrets: &SecretSet,
) -> Correlated {
    let shown = |id: &str| secrets.scrub(id).to_owned();
    let mut open = unresolved.clone();
    let mut correlated = Correlated::default();
    for row in rows {
        let row_scan = match &row.scan_id {
            Optional::Value(id) => id.as_str(),
            Optional::Unset => scan_id,
        };
        let status = match &row.status {
            Optional::Value(status) => status.to_ascii_lowercase(),
            Optional::Unset => String::new(),
        };
        let terminal = matches!(status.as_str(), "failed" | "complete" | "completed");
        let req_id = match &row.req_id {
            Optional::Value(number) => Some(request_index(number)),
            Optional::Unset => None,
        };
        if req_id.is_none() && terminal && row_scan == scan_id {
            if status == "failed" {
                for index in std::mem::take(&mut open) {
                    correlated.resolved.push((index, failed_row(scan_id)));
                }
                continue;
            }
            let row_report = row
                .extensions
                .get("report_id")
                .and_then(|value| value.as_value().as_str());
            let report = match &row.result {
                Optional::Value(result) => Some(result.report_id.as_str()),
                Optional::Unset => row_report.or(receipt_report),
            };
            match report.filter(|id| !id.is_empty()) {
                Some(id) => correlated.report_id = Some(id.to_owned()),
                None => {
                    correlated.report_id = None;
                    correlated.error = Some(format!(
                        "AIRS result correlation failed for scan {}: terminal row has no request or report ID",
                        shown(row_scan)
                    ));
                    return correlated;
                }
            }
            continue;
        }
        let Some(Some(index)) = req_id else { continue };
        if row_scan != scan_id || !open.contains(&index) {
            continue;
        }
        if status == "failed" {
            open.remove(&index);
            correlated.resolved.push((index, failed_row(scan_id)));
            continue;
        }
        if let ("complete" | "completed", Optional::Value(result)) = (status.as_str(), &row.result)
        {
            if !result.scan_id.is_empty() && result.scan_id != row_scan {
                // A report id recorded by an earlier row is not queried after a stop.
                correlated.report_id = None;
                correlated.error = Some(format!(
                    "AIRS result correlation mismatch: nested scan ID {} does not match {}",
                    shown(&result.scan_id),
                    shown(row_scan)
                ));
                return correlated;
            }
            open.remove(&index);
            correlated
                .resolved
                .push((index, scan_row(result, scan_id, secrets)));
        }
    }
    correlated
}

/// §14.7 step 3 for one reports response: the rows resolved, and the correlation
/// message that stopped it in `error` (the rows resolved before it are kept).
pub fn correlate_reports(
    reports: &[ThreatScanReport],
    scan_id: &str,
    expected_report: &str,
    unresolved: &BTreeSet<u64>,
    secrets: &SecretSet,
) -> Correlated {
    let mut open = unresolved.clone();
    let mut resolved = Vec::new();
    for report in reports {
        let report_scan = match &report.scan_id {
            Optional::Value(id) => id.as_str(),
            Optional::Unset => scan_id,
        };
        let mismatched_report = matches!(&report.report_id, Optional::Value(id) if !id.is_empty() && id != expected_report);
        if report_scan != scan_id || mismatched_report {
            let message = format!(
                "AIRS report correlation mismatch for scan {}, request {}",
                secrets.scrub(scan_id),
                request_text(&report.req_id)
            );
            return Correlated {
                resolved,
                report_id: None,
                error: Some(message),
            };
        }
        let Optional::Value(number) = &report.req_id else {
            continue;
        };
        let Some(index) = request_index(number) else {
            continue;
        };
        if !open.remove(&index) {
            continue;
        }
        resolved.push((index, report_row(report, scan_id, secrets)));
    }
    Correlated {
        resolved,
        report_id: None,
        error: None,
    }
}

/// Submission and polling for one job.
pub struct Engine<'a, A: Api> {
    pub api: &'a A,
    pub session: &'a scm::Session,
    pub secrets: &'a SecretSet,
    pub timing: Timing,
}

impl<A: Api> Engine<'_, A> {
    /// Wait `delay` unless a signal arrives first; `true` when interrupted.
    async fn interrupted_during(&self, delay: Duration) -> bool {
        if self.session.is_cancelled() {
            return true;
        }
        tokio::select! {
            () = tokio::time::sleep(delay) => self.session.is_cancelled(),
            () = self.session.cancelled() => true,
        }
    }

    fn context(&self, operation: Operation) -> SdkContext<'_> {
        SdkContext {
            plane: Plane::Scanner,
            phase: Phase::Api,
            operation,
            timeout: REQUEST_BUDGET,
            secrets: self.secrets,
        }
    }

    /// The §6 failure of `error` with `detail` after its own lines.
    fn failure(&self, error: &CallError, operation: Operation, detail: String) -> CliError {
        match error::describe(&error.facts, &self.context(operation)) {
            CliError::Operational(mut failure) => {
                failure.details.push(detail);
                CliError::Operational(failure)
            }
            other => other,
        }
    }

    /// Submit one chunk (§14.6) with the C1/C3 checkpoints and the 429 loop.
    pub async fn submit_chunk(
        &self,
        job: &mut Job,
        out: &mut Output,
        indices: &[u64],
    ) -> Result<(), CliError> {
        let chunk_ranges = ranges(indices);
        let mut retries = 0u32;
        loop {
            if self.session.is_cancelled() {
                return Err(CliError::Interrupted(job.saved_context()));
            }
            job.verify_lock()?;
            // C1: the items are `submitting` and the attempt `inFlight`, durably.
            let attempt_id = job.state.attempts.len() as u64 + 1;
            job.state.attempts.push(Attempt {
                id: attempt_id,
                indices: indices.to_vec(),
                started_at: super::now_stamp(),
                ended_at: None,
                outcome: AttemptOutcome::InFlight,
                http_status: None,
                error: None,
                scan_id: None,
            });
            for index in indices {
                let item = &mut job.state.items[*index as usize];
                item.status = ItemStatus::Submitting;
                item.attempt = Some(attempt_id);
            }
            job.checkpoint(&Consequence::NothingSubmitted)?;
            let objects = indices
                .iter()
                .map(|index| {
                    let item = &job.state.items[*index as usize];
                    super::plan::scan_object(
                        item.index,
                        &item.prompt,
                        &job.state.profile,
                        &job.state.session_id,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let (result, status) = self.api.submit(&objects).await;
            let outcome = match result {
                Ok(receipt)
                    if is_scan_id(&receipt.scan_id) && !self.secrets.taints(&receipt.scan_id) =>
                {
                    Outcome::Accepted {
                        report_id: match &receipt.report_id {
                            Optional::Value(id) => Some(self.secrets.scrub(id).to_owned()),
                            Optional::Unset => None,
                        },
                        scan_id: receipt.scan_id,
                    }
                }
                Ok(_) => Outcome::Unusable,
                Err(error) => classify(error, retries < MAX_RETRIES),
            };
            let unusable = format!(
                "AIRS accepted prompts {chunk_ranges}, but its receipt has no usable scan id; they will not be resubmitted"
            );
            // C3: the outcome, durably, before anything else.
            let (item_status, attempt_outcome, error_text, http) = match &outcome {
                Outcome::Accepted { .. } => (
                    ItemStatus::Submitted,
                    AttemptOutcome::Accepted,
                    None,
                    status,
                ),
                Outcome::Unusable => (
                    ItemStatus::Ambiguous,
                    AttemptOutcome::UnusableReceipt,
                    Some(unusable.clone()),
                    status,
                ),
                Outcome::RateLimited(error) | Outcome::Rejected(error) => (
                    ItemStatus::Pending,
                    AttemptOutcome::Rejected,
                    Some(error::facts_message(
                        &error.facts,
                        &self.context(Operation::ScanSubmit),
                    )),
                    error.facts.status,
                ),
                Outcome::NotSent(error) => (
                    ItemStatus::Pending,
                    AttemptOutcome::NotSent,
                    Some(error::facts_message(
                        &error.facts,
                        &self.context(Operation::ScanSubmit),
                    )),
                    error.facts.status,
                ),
                Outcome::Unknown(error) => (
                    ItemStatus::Ambiguous,
                    AttemptOutcome::Unknown,
                    Some(error::facts_message(
                        &error.facts,
                        &self.context(Operation::ScanSubmit),
                    )),
                    error.facts.status,
                ),
            };
            let (scan_id, report_id) = match &outcome {
                Outcome::Accepted { scan_id, report_id } => {
                    (Some(scan_id.clone()), report_id.clone())
                }
                _ => (None, None),
            };
            if let Some(attempt) = job.state.attempt_mut(attempt_id) {
                attempt.ended_at = Some(super::now_stamp());
                attempt.outcome = attempt_outcome;
                attempt.http_status = http;
                attempt.error = error_text;
                attempt.scan_id = scan_id.clone();
            }
            for index in indices {
                let item = &mut job.state.items[*index as usize];
                item.status = item_status;
                item.scan_id = scan_id.clone();
                item.receipt_report_id = if scan_id.is_some() {
                    report_id.clone()
                } else {
                    None
                };
            }
            let consequence = match &scan_id {
                Some(scan_id) => Consequence::Accepted {
                    ranges: chunk_ranges.clone(),
                    scan_id: scan_id.clone(),
                },
                None => Consequence::NothingSubmitted,
            };
            job.checkpoint(&consequence)?;
            self.api.checkpointed(&job.state);
            match outcome {
                Outcome::Accepted { .. } => {
                    if self.session.is_cancelled() {
                        return Err(CliError::Interrupted(job.saved_context()));
                    }
                    return Ok(());
                }
                Outcome::Unusable => {
                    return Err(CliError::Operational(Failure {
                        message: unusable,
                        status: None,
                        details: vec![job.ambiguous_detail(&chunk_ranges)],
                    }));
                }
                Outcome::RateLimited(error) => {
                    retries += 1;
                    let delay = retry_delay(error.retry_after, retries, self.timing.base_delay);
                    out.status(&format!(
                        "⚠ Rate limited while submitting — retry {retries} in {}s...",
                        seconds(delay)
                    ));
                    if self.interrupted_during(delay).await {
                        return Err(CliError::Interrupted(job.saved_context()));
                    }
                }
                Outcome::Rejected(error) => {
                    return Err(self.failure(&error, Operation::ScanSubmit, job.resume_detail()));
                }
                Outcome::NotSent(error) if error.cancelled() => {
                    return Err(CliError::Interrupted(job.saved_context()));
                }
                Outcome::NotSent(error) => {
                    return Err(self.failure(&error, Operation::ScanSubmit, job.resume_detail()));
                }
                Outcome::Unknown(error) if error.cancelled() => {
                    return Err(CliError::Interrupted(InterruptContext::Bulk(format!(
                        "prompts {chunk_ranges} may already have been submitted (outcome unknown); the job was saved in {}",
                        job.state_shown()
                    ))));
                }
                Outcome::Unknown(error) => {
                    let detail = job.ambiguous_detail(&chunk_ranges);
                    return Err(self.failure(&error, Operation::ScanSubmit, detail));
                }
            }
        }
    }

    /// Apply resolved rows and checkpoint them with a CSV rewrite.
    fn record(
        &self,
        job: &mut Job,
        out: &mut Output,
        unresolved: &mut BTreeSet<u64>,
        resolved: Vec<(u64, ItemResult)>,
    ) -> Result<(), CliError> {
        if resolved.is_empty() {
            self.api.checkpointed(&job.state);
            return Ok(());
        }
        for (index, result) in resolved {
            unresolved.remove(&index);
            let item = &mut job.state.items[index as usize];
            item.status = if result.action == Action::Failed {
                ItemStatus::Failed
            } else {
                ItemStatus::Complete
            };
            item.result = Some(result);
        }
        job.save_with_output_observed(out, self.secrets, |state| self.api.checkpointed(state))
    }

    fn poll_stop(&self, job: &Job, message: String) -> CliError {
        CliError::Operational(Failure {
            message,
            status: None,
            details: vec![job.resume_detail()],
        })
    }

    /// Poll one receipt until every entry is resolved (§14.7).
    pub async fn poll_receipt(
        &self,
        job: &mut Job,
        out: &mut Output,
        scan_id: &str,
    ) -> Result<(), CliError> {
        let mut unresolved: BTreeSet<u64> = job
            .state
            .items
            .iter()
            .filter(|item| {
                item.status == ItemStatus::Submitted && item.scan_id.as_deref() == Some(scan_id)
            })
            .map(|item| item.index)
            .collect();
        let receipt_report = unresolved
            .first()
            .and_then(|index| job.state.items[*index as usize].receipt_report_id.clone());
        let mut level = 0u32;
        let mut no_progress = 0u32;
        while !unresolved.is_empty() {
            if self.session.is_cancelled() {
                return Err(CliError::Interrupted(job.saved_context()));
            }
            let rows = match self.api.results(scan_id).await {
                Ok(rows) => rows,
                Err(error) if error.cancelled() => {
                    return Err(CliError::Interrupted(job.saved_context()));
                }
                Err(error) if error.rate_limited() && level < MAX_RETRIES => {
                    level += 1;
                    self.rate_limit_wait(job, out, &error, level).await?;
                    continue;
                }
                Err(error) => {
                    return Err(self.failure(&error, Operation::Read, job.resume_detail()));
                }
            };
            let before = unresolved.len();
            let correlated = correlate_rows(
                &rows,
                scan_id,
                receipt_report.as_deref(),
                &unresolved,
                self.secrets,
            );
            // Rows resolved before a correlation error are checkpointed first (TS saves
            // each row as it resolves it), then the error stops the command.
            self.record(job, out, &mut unresolved, correlated.resolved)?;
            if let Some(message) = correlated.error {
                return Err(self.poll_stop(job, message));
            }
            if let Some(report_id) = correlated.report_id {
                let reports = match self.api.reports(std::slice::from_ref(&report_id)).await {
                    Ok(reports) => reports,
                    Err(error) if error.cancelled() => {
                        return Err(CliError::Interrupted(job.saved_context()));
                    }
                    Err(error) if error.rate_limited() && level < MAX_RETRIES => {
                        level += 1;
                        self.rate_limit_wait(job, out, &error, level).await?;
                        continue;
                    }
                    Err(error) => {
                        return Err(self.failure(&error, Operation::Read, job.resume_detail()));
                    }
                };
                let reported =
                    correlate_reports(&reports, scan_id, &report_id, &unresolved, self.secrets);
                self.record(job, out, &mut unresolved, reported.resolved)?;
                if let Some(message) = reported.error {
                    return Err(self.poll_stop(job, message));
                }
            }
            level = level.saturating_sub(1);
            if unresolved.len() == before {
                no_progress += 1;
            } else {
                no_progress = 0;
            }
            if no_progress >= self.timing.max_no_progress {
                return Err(self.poll_stop(
                    job,
                    format!(
                        "AIRS polling made no progress after {no_progress} polls for scan {}",
                        self.secrets.scrub(scan_id)
                    ),
                ));
            }
            if !unresolved.is_empty() && self.interrupted_during(self.timing.poll_interval).await {
                return Err(CliError::Interrupted(job.saved_context()));
            }
        }
        Ok(())
    }

    async fn rate_limit_wait(
        &self,
        job: &Job,
        out: &mut Output,
        error: &CallError,
        level: u32,
    ) -> Result<(), CliError> {
        let delay = retry_delay(error.retry_after, level, self.timing.base_delay);
        out.status(&format!(
            "⚠ Rate limited — retry {level} in {}s...",
            seconds(delay)
        ));
        if self.interrupted_during(delay).await {
            return Err(CliError::Interrupted(job.saved_context()));
        }
        Ok(())
    }

    /// `bulk-scan`'s batches (§14.5): per logical batch, every chunk is submitted, then
    /// every receipt of the batch is polled to completion.
    pub async fn run_new(&self, job: &mut Job, out: &mut Output) -> Result<(), CliError> {
        let batches = super::plan::logical_batches(job.state.items.len(), job.state.batch_size);
        for (number, (start, end)) in batches.into_iter().enumerate() {
            out.status(&format!("Submitting batch {}...", number + 1));
            let items: Vec<(u64, &str)> = job.state.items[start..end]
                .iter()
                .map(|item| (item.index, item.prompt.as_str()))
                .collect();
            let chunks = super::plan::chunks(
                &items,
                &job.state.profile,
                &job.state.session_id,
                super::plan::body_budget(),
            )?;
            for chunk in chunks {
                self.submit_chunk(job, out, &chunk).await?;
            }
            out.status(&format!(
                "Scan IDs saved: {}",
                self.secrets.scrub(&job.state_shown())
            ));
            self.poll_batch(job, out, start, end).await?;
        }
        Ok(())
    }

    /// `resume-poll`'s normal path (§14.13 step 8): per stored logical batch, poll its
    /// `submitted` receipts, submit its `pending` items (ascending, in chunks within
    /// the body budget), then poll again. `submitting` and `ambiguous` items are never
    /// sent (the caller refuses a job that has any).
    pub async fn run_resume(&self, job: &mut Job, out: &mut Output) -> Result<(), CliError> {
        let batches = super::plan::logical_batches(job.state.items.len(), job.state.batch_size);
        for (start, end) in batches {
            self.poll_batch(job, out, start, end).await?;
            let pending: Vec<(u64, &str)> = job.state.items[start..end]
                .iter()
                .filter(|item| item.status == ItemStatus::Pending)
                .map(|item| (item.index, item.prompt.as_str()))
                .collect();
            if pending.is_empty() {
                continue;
            }
            let chunks = super::plan::chunks(
                &pending,
                &job.state.profile,
                &job.state.session_id,
                super::plan::body_budget(),
            )?;
            for chunk in chunks {
                self.submit_chunk(job, out, &chunk).await?;
            }
            self.poll_batch(job, out, start, end).await?;
        }
        Ok(())
    }

    /// `resume-poll`'s ambiguous path (§14.13 step 7): poll every `submitted` receipt
    /// of the job, batch by batch; nothing is submitted.
    pub async fn recover(&self, job: &mut Job, out: &mut Output) -> Result<(), CliError> {
        let batches = super::plan::logical_batches(job.state.items.len(), job.state.batch_size);
        for (start, end) in batches {
            self.poll_batch(job, out, start, end).await?;
        }
        Ok(())
    }

    /// Poll every `submitted` receipt of the items `[start, end)`.
    pub async fn poll_batch(
        &self,
        job: &mut Job,
        out: &mut Output,
        start: usize,
        end: usize,
    ) -> Result<(), CliError> {
        let receipts = super::plan::receipts(
            &job.state.items[start..end]
                .iter()
                .map(|item| (item.index, item.status, item.scan_id.as_deref()))
                .collect::<Vec<_>>(),
        );
        for scan_id in receipts {
            self.poll_receipt(job, out, &scan_id).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::checkpoint::tests::{TempState, tempfile_dir::Dir};
    use super::super::state::tests::sample;
    use super::*;
    use serde_json::{Value, json};
    use std::cell::RefCell;
    use std::collections::VecDeque;

    const SCAN: &str = "00000000-0000-4000-8000-000000007001";

    fn rows(value: Value) -> Vec<ScanIdResult> {
        serde_json::from_value(value).unwrap()
    }

    fn reports(value: Value) -> Vec<ThreatScanReport> {
        serde_json::from_value(value).unwrap()
    }

    fn verdict(action: &str, extra: Value) -> Value {
        let mut value = json!({
            "report_id": format!("R{SCAN}"), "scan_id": SCAN, "category": "benign",
            "action": action, "timeout": false, "error": false, "errors": []
        });
        if let (Value::Object(base), Value::Object(extra)) = (&mut value, extra) {
            base.extend(extra);
        }
        value
    }

    fn open(indices: &[u64]) -> BTreeSet<u64> {
        indices.iter().copied().collect()
    }

    /// int-spec #9 (the correlation half): rows out of order align by request id;
    /// prompt detections only, unknown keys of other scans ignored.
    #[test]
    fn rows_correlate_by_scan_and_request_id() {
        let secrets = SecretSet::default();
        let body = json!([
            {"req_id": 2, "status": "COMPLETED", "scan_id": SCAN, "result": verdict("allow", json!({}))},
            {"req_id": 0, "status": "complete", "result": verdict("block", json!({"category": "malicious", "prompt_detected": {"source_code": true, "injection": false}, "response_detected": {"dlp": true}}))},
            {"req_id": 1, "status": "complete", "scan_id": "00000000-0000-4000-8000-000000009999", "result": verdict("allow", json!({}))},
            {"req_id": 1, "status": "pending"},
            {"req_id": 0, "status": "failed"},
            {"req_id": 9, "status": "complete", "result": verdict("allow", json!({}))}
        ]);
        let correlated =
            correlate_rows(&rows(body), SCAN, None, &open(&[0, 1, 2]), &secrets).unwrap();
        let indices: Vec<u64> = correlated.resolved.iter().map(|(i, _)| *i).collect();
        assert_eq!(
            indices,
            [2, 0],
            "a duplicate and a foreign scan are ignored"
        );
        let block = &correlated.resolved[1].1;
        assert_eq!(block.action, Action::Block);
        assert_eq!(block.category, "malicious");
        assert!(block.triggered);
        assert_eq!(
            block.detections.0,
            [("injection".into(), false), ("source_code".into(), true)]
        );
        assert_eq!(block.report_id, format!("R{SCAN}"));
        assert!(correlated.report_id.is_none());
    }

    /// `req_id` `2.0` is request 2; `-1`, `1.5` and huge values match nothing.
    #[test]
    fn integral_request_ids_only() {
        let secrets = SecretSet::default();
        let body = json!([
            {"req_id": 2.0, "status": "complete", "result": verdict("allow", json!({}))},
            {"req_id": -1, "status": "complete", "result": verdict("allow", json!({}))},
            {"req_id": 1.5, "status": "complete", "result": verdict("allow", json!({}))},
            {"req_id": 1e300, "status": "complete", "result": verdict("allow", json!({}))}
        ]);
        let correlated = correlate_rows(&rows(body), SCAN, None, &open(&[1, 2]), &secrets).unwrap();
        assert_eq!(correlated.resolved.len(), 1);
        assert_eq!(correlated.resolved[0].0, 2);
    }

    /// Fail-closed rows (D6): timeout, `error: true`, unknown action, a failed status.
    #[test]
    fn failed_rows_fail_closed() {
        let secrets = SecretSet::default();
        let body = json!([
            {"req_id": 0, "status": "complete", "result": verdict("allow", json!({"timeout": true}))},
            {"req_id": 1, "status": "complete", "result": verdict("allow", json!({"error": true, "errors": [{"feature": "dlp", "status": "error", "content_type": "prompt"}]}))},
            {"req_id": 2, "status": "complete", "result": verdict("quarantine", json!({}))},
            {"req_id": 3, "status": "failed"}
        ]);
        let correlated =
            correlate_rows(&rows(body), SCAN, None, &open(&[0, 1, 2, 3]), &secrets).unwrap();
        let errors: Vec<(Action, &str, Option<&str>)> = correlated
            .resolved
            .iter()
            .map(|(_, r)| (r.action, r.category.as_str(), r.error.as_deref()))
            .collect();
        assert_eq!(
            errors,
            [
                (Action::Failed, "error", Some("AIRS scan timed out")),
                (Action::Failed, "error", Some("dlp: error: prompt")),
                (
                    Action::Failed,
                    "error",
                    Some("Unknown AIRS action: quarantine")
                ),
                (Action::Failed, "error", Some("AIRS async scan failed")),
            ]
        );
        assert_eq!(correlated.resolved[3].1.report_id, "");
    }

    /// Terminal batch rows: `failed` fails every open entry; `complete` names the
    /// report for the fallback; none is a correlation error; a nested mismatch too.
    #[test]
    fn terminal_rows_and_correlation_errors() {
        let secrets = SecretSet::default();
        let failed = correlate_rows(
            &rows(json!([{"status": "FAILED"}])),
            SCAN,
            None,
            &open(&[3, 4]),
            &secrets,
        )
        .unwrap();
        assert_eq!(
            failed.resolved.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
            [3, 4]
        );
        let other_scan = correlate_rows(
            &rows(json!([{"status": "failed", "scan_id": "00000000-0000-4000-8000-000000009999"}])),
            SCAN,
            None,
            &open(&[3]),
            &secrets,
        )
        .unwrap();
        assert!(other_scan.resolved.is_empty());
        for (body, receipt, expected) in [
            (
                json!([{"status": "complete", "report_id": "R-row"}]),
                Some("R-receipt"),
                "R-row",
            ),
            (
                json!([{"status": "complete"}]),
                Some("R-receipt"),
                "R-receipt",
            ),
            (
                json!([{"status": "complete", "result": verdict("allow", json!({"report_id": "R-nested"}))}]),
                Some("R-receipt"),
                "R-nested",
            ),
        ] {
            let correlated =
                correlate_rows(&rows(body), SCAN, receipt, &open(&[0]), &secrets).unwrap();
            assert_eq!(correlated.report_id.as_deref(), Some(expected));
            assert!(correlated.resolved.is_empty());
        }
        assert_eq!(
            correlate_rows(
                &rows(json!([{"status": "complete"}])),
                SCAN,
                None,
                &open(&[0]),
                &secrets
            )
            .unwrap_err(),
            format!(
                "AIRS result correlation failed for scan {SCAN}: terminal row has no request or report ID"
            )
        );
        assert_eq!(
            correlate_rows(&rows(json!([{"req_id": 0, "status": "complete", "result": verdict("allow", json!({"scan_id": "00000000-0000-4000-8000-000000000bad"}))}])), SCAN, None, &open(&[0]), &secrets).unwrap_err(),
            format!("AIRS result correlation mismatch: nested scan ID 00000000-0000-4000-8000-000000000bad does not match {SCAN}")
        );
    }

    /// A correlation error stops at its row, but the rows resolved before it in the
    /// same response are kept (TS saves each row before it throws), and a report id
    /// recorded before it is not queried.
    #[test]
    fn rows_before_a_correlation_error_are_kept() {
        let secrets = SecretSet::default();
        let body = json!([
            {"req_id": 0, "status": "complete", "result": verdict("allow", json!({}))},
            {"status": "complete", "report_id": "R-row"},
            {"req_id": 1, "status": "complete", "result": verdict("allow", json!({"scan_id": "00000000-0000-4000-8000-00000000ffff"}))},
            {"req_id": 2, "status": "complete", "result": verdict("allow", json!({}))}
        ]);
        let correlated = correlate_rows(&rows(body), SCAN, None, &open(&[0, 1, 2]), &secrets);
        assert_eq!(
            correlated
                .resolved
                .iter()
                .map(|(i, _)| *i)
                .collect::<Vec<_>>(),
            [0]
        );
        assert_eq!(correlated.report_id, None);
        assert_eq!(
            correlated.error.as_deref(),
            Some(
                format!(
                    "AIRS result correlation mismatch: nested scan ID 00000000-0000-4000-8000-00000000ffff does not match {SCAN}"
                )
                .as_str()
            )
        );
        let body = json!([
            {"req_id": 0, "status": "failed"},
            {"status": "complete"}
        ]);
        let correlated = correlate_rows(&rows(body), SCAN, None, &open(&[0, 1]), &secrets);
        assert_eq!(correlated.resolved.len(), 1);
        assert!(correlated.error.is_some());
        let reports = reports(json!([
            {"report_id": "R1", "req_id": 0, "detection_results": []},
            {"report_id": "R2", "req_id": 1, "detection_results": []}
        ]));
        let reported = correlate_reports(&reports, SCAN, "R1", &open(&[0, 1]), &secrets);
        assert_eq!(
            reported
                .resolved
                .iter()
                .map(|(i, _)| *i)
                .collect::<Vec<_>>(),
            [0]
        );
        assert_eq!(
            reported.error.as_deref(),
            Some(format!("AIRS report correlation mismatch for scan {SCAN}, request 1").as_str())
        );
    }

    /// D131: each server reason part in a row's `error` (stored in the state and the
    /// CSV) is cut to 200 characters plus `...`; TypeScript stores the full text.
    #[test]
    fn long_reason_parts_are_cut_in_rows_d131() {
        let secrets = SecretSet::default();
        let long = "f".repeat(250);
        let body = json!([
            {"req_id": 0, "status": "complete", "result": verdict("allow", json!({"error": true, "errors": [{"feature": long, "status": "error", "content_type": "prompt"}]}))},
            {"req_id": 1, "status": "complete", "result": verdict(&"q".repeat(201), json!({}))}
        ]);
        let correlated = correlate_rows(&rows(body), SCAN, None, &open(&[0, 1]), &secrets).unwrap();
        assert_eq!(
            correlated.resolved[0].1.error.as_deref(),
            Some(format!("{}...: error: prompt", "f".repeat(200)).as_str())
        );
        assert_eq!(
            correlated.resolved[1].1.error.as_deref(),
            Some(format!("Unknown AIRS action: {}...", "q".repeat(200)).as_str())
        );
        let report = reports(
            json!([{"req_id": 0, "detection_results": [{"detection_service": "dlp", "action": "x".repeat(230)}]}]),
        );
        let reported = correlate_reports(&report, SCAN, "R", &open(&[0]), &secrets).unwrap();
        assert_eq!(
            reported.resolved[0].1.error.as_deref(),
            Some(
                format!(
                    "Unknown AIRS action in threat report: {}...",
                    "x".repeat(200)
                )
                .as_str()
            )
        );
    }

    /// The report fallback (TS `threatReportToResult`).
    #[test]
    fn reports_become_rows() {
        let secrets = SecretSet::default();
        let body = json!([
            {"report_id": "R1", "scan_id": SCAN, "req_id": 0, "detection_results": [
                {"detection_service": "pi", "verdict": "malicious", "action": "allow"},
                {"detection_service": "dlp", "verdict": "benign", "action": "allow"}
            ]},
            {"report_id": "R1", "req_id": 1, "detection_results": [
                {"detection_service": "uf", "verdict": "benign", "action": "block"},
                {"detection_service": "custom_svc", "verdict": "detected", "action": ""}
            ]},
            {"req_id": 2, "detection_results": [
                {"detection_service": "tc", "verdict": "benign", "action": "quarantine"}
            ]},
            {"req_id": 3, "detection_results": []},
            {"req_id": 0, "detection_results": []}
        ]);
        let resolved =
            correlate_reports(&reports(body), SCAN, "R1", &open(&[0, 1, 2, 3]), &secrets)
                .unwrap()
                .resolved;
        let summary: Vec<(u64, Action, &str, bool)> = resolved
            .iter()
            .map(|(i, r)| (*i, r.action, r.category.as_str(), r.triggered))
            .collect();
        assert_eq!(
            summary,
            [
                (0, Action::Allow, "malicious", true),
                (1, Action::Block, "malicious", true),
                (2, Action::Failed, "error", false),
                (3, Action::Allow, "benign", false),
            ]
        );
        assert_eq!(
            resolved[1].1.detections.0,
            [("url_cats".into(), true), ("custom_svc".into(), true)]
        );
        assert_eq!(
            resolved[2].1.error.as_deref(),
            Some("Unknown AIRS action in threat report: quarantine")
        );
        assert_eq!(resolved[0].1.report_id, "R1");
        assert_eq!(resolved[3].1.report_id, "");
        assert_eq!(
            correlate_reports(
                &reports(json!([{"report_id": "R2", "req_id": 1}])),
                SCAN,
                "R1",
                &open(&[1]),
                &secrets
            )
            .unwrap_err(),
            format!("AIRS report correlation mismatch for scan {SCAN}, request 1")
        );
        assert_eq!(
            correlate_reports(
                &reports(json!([{"scan_id": "00000000-0000-4000-8000-000000009999"}])),
                SCAN,
                "R1",
                &open(&[1]),
                &secrets
            )
            .unwrap_err(),
            format!("AIRS report correlation mismatch for scan {SCAN}, request undefined")
        );
    }

    /// D131: server values carrying a secret are withheld before they are stored.
    #[test]
    fn secrets_are_withheld_from_rows() {
        let secrets = SecretSet::default();
        secrets.add("FAKE-SECRET-VALUE");
        let body = json!([
            {"req_id": 0, "status": "complete", "result": verdict("allow", json!({"category": "x FAKE-SECRET-VALUE", "report_id": "FAKE-SECRET-VALUE"}))},
            {"req_id": 1, "status": "complete", "result": verdict("bad FAKE-SECRET-VALUE", json!({}))}
        ]);
        let correlated = correlate_rows(&rows(body), SCAN, None, &open(&[0, 1]), &secrets).unwrap();
        assert_eq!(correlated.resolved[0].1.category, "[withheld]");
        assert_eq!(correlated.resolved[0].1.report_id, "[withheld]");
        assert_eq!(
            correlated.resolved[1].1.error.as_deref(),
            Some("Unknown AIRS action: [withheld]")
        );
        let report = json!([{"req_id": 0, "detection_results": [{"detection_service": "FAKE-SECRET-VALUE", "verdict": "detected"}]}]);
        let resolved = correlate_reports(&reports(report), SCAN, "R", &open(&[0]), &secrets)
            .unwrap()
            .resolved;
        assert!(resolved[0].1.detections.0.is_empty());
    }

    #[test]
    fn delays_and_classification() {
        let base = Duration::from_secs(10);
        assert_eq!(
            (1..=5)
                .map(|n| retry_delay(None, n, base).as_secs())
                .collect::<Vec<_>>(),
            [10, 20, 40, 80, 160]
        );
        assert_eq!(
            retry_delay(Some(Duration::from_secs(7)), 3, base),
            Duration::from_secs(7)
        );
        // D124: a far-future HTTP date is capped.
        assert_eq!(
            retry_delay(Some(Duration::from_secs(2_300_000_000)), 1, base),
            RETRY_AFTER_CAP
        );
        assert_eq!(seconds(Duration::from_millis(1500)), "2");
        assert_eq!(seconds(Duration::from_millis(1499)), "1");
        assert_eq!(seconds(Duration::ZERO), "0");
        let facts = |kind, status, outcome| CallError {
            facts: Box::new(ErrorFacts {
                kind,
                status,
                error_type: None,
                problem: None,
                service_code: None,
                request_id: None,
                remote_outcome: outcome,
            }),
            retry_after: None,
        };
        use scm::ErrorKind::{Cancelled, Destination, Http, Limit, Transport, Validation};
        let name = |outcome: Outcome| match outcome {
            Outcome::RateLimited(_) => "rate",
            Outcome::Rejected(_) => "rejected",
            Outcome::NotSent(_) => "notSent",
            Outcome::Unknown(_) => "unknown",
            _ => "other",
        };
        for (error, retries_left, expected) in [
            (
                facts(Http, Some(429), RemoteOutcome::Rejected),
                true,
                "rate",
            ),
            (
                facts(Http, Some(429), RemoteOutcome::Rejected),
                false,
                "rejected",
            ),
            (
                facts(Limit, Some(429), RemoteOutcome::Unknown),
                true,
                "rate",
            ),
            (
                facts(Limit, Some(400), RemoteOutcome::Unknown),
                true,
                "rejected",
            ),
            (
                facts(Http, Some(400), RemoteOutcome::Rejected),
                true,
                "rejected",
            ),
            (
                facts(Destination, Some(307), RemoteOutcome::Rejected),
                true,
                "rejected",
            ),
            (
                facts(Http, Some(500), RemoteOutcome::Unknown),
                true,
                "unknown",
            ),
            (
                facts(Transport, None, RemoteOutcome::Unknown),
                true,
                "unknown",
            ),
            (
                facts(Validation, None, RemoteOutcome::NotSent),
                true,
                "notSent",
            ),
            (
                facts(Cancelled, None, RemoteOutcome::NotSent),
                true,
                "notSent",
            ),
            (
                facts(Cancelled, None, RemoteOutcome::Unknown),
                true,
                "unknown",
            ),
        ] {
            assert_eq!(name(classify(error, retries_left)), expected);
        }
    }

    /// A scripted API for the polling loop.
    struct Fake {
        results: RefCell<VecDeque<Result<Vec<ScanIdResult>, CallError>>>,
        calls: RefCell<u32>,
    }

    impl Api for Fake {
        async fn submit(
            &self,
            _: &[AsyncScanObject],
        ) -> (Result<AsyncScanResponse, CallError>, Option<u16>) {
            unreachable!("no submission in this test")
        }
        async fn results(&self, _: &str) -> Result<Vec<ScanIdResult>, CallError> {
            *self.calls.borrow_mut() += 1;
            self.results
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Ok(rows(json!([]))))
        }
        async fn reports(&self, _: &[String]) -> Result<Vec<ThreatScanReport>, CallError> {
            Ok(Vec::new())
        }
    }

    fn submitted_job(temp: &TempState, dir: &Dir) -> Job {
        let mut state = sample();
        let output_path = dir.path().join("out.csv");
        let mut output = OutputFile::new(output_path.clone());
        let (identity, _) = output
            .claim(super::super::output::HEADER.as_bytes())
            .unwrap();
        state.output.path = output_path.to_string_lossy().into_owned();
        state.output.dev = identity.dev.to_string();
        state.output.ino = identity.ino.to_string();
        state.attempts.push(Attempt {
            id: 1,
            indices: vec![0, 1, 2],
            started_at: super::super::now_stamp(),
            ended_at: Some(super::super::now_stamp()),
            outcome: AttemptOutcome::Accepted,
            http_status: Some(200),
            error: None,
            scan_id: Some(SCAN.into()),
        });
        for item in &mut state.items {
            item.status = ItemStatus::Submitted;
            item.attempt = Some(1);
            item.scan_id = Some(SCAN.into());
        }
        let mut file = temp.state_file(&state.job_id);
        file.checkpoint(&mut state, &Consequence::NothingSubmitted)
            .unwrap();
        Job {
            state,
            file,
            output,
            lock: None,
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    /// The SDK refuses an over-long session id locally (`NotSent`, no request), so the
    /// CLI's own check (§14.2) only moves the refusal earlier; and the SDK sends every
    /// chunk the planner builds within its request limit (§14.5).
    #[test]
    fn the_sdk_refuses_before_sending_and_accepts_planned_chunks() {
        use crate::tenant::config::{ConfigValue, TenantConfig};
        let server = airs_test_harness::FixtureServer::start();
        server.add_tenant(
            "100",
            "client-100",
            "FAKE-SECRET-100",
            Some("FAKE-APIKEY-100"),
            None,
        );
        let config = TenantConfig::from_values([
            ("airsApiKey", ConfigValue::Text("FAKE-APIKEY-100".into())),
            ("airsApiEndpoint", ConfigValue::Text(server.url("/scan"))),
        ]);
        let session = scm::Session::new();
        let (scanner, last_status) =
            crate::clients::build_observed_scanner(&config, &session, None).unwrap();
        let api = ScannerApi {
            scanner,
            last_status,
        };
        runtime().block_on(async {
            let long = super::super::plan::scan_object(0, "p", "p", &"s".repeat(101)).unwrap();
            let (result, status) = api.submit(&[long]).await;
            let error = result.expect_err("refused");
            assert_eq!(error.facts.remote_outcome, RemoteOutcome::NotSent);
            assert_eq!(error.facts.kind, scm::ErrorKind::Validation);
            assert_eq!(status, None);
            assert_eq!(server.request_count(), 0);
            let ok = super::super::plan::scan_object(0, "p", "p", &"s".repeat(100)).unwrap();
            let (result, status) = api.submit(&[ok]).await;
            assert!(result.is_ok());
            assert_eq!(status, Some(200));
            let prompts: Vec<String> = (0..5)
                .map(|n| format!("{n}{}", "q".repeat((2 << 20) - 1)))
                .collect();
            let indexed: Vec<(u64, &str)> = prompts
                .iter()
                .enumerate()
                .map(|(i, p)| (i as u64, p.as_str()))
                .collect();
            let chunks =
                super::super::plan::chunks(&indexed, "p", "s", super::super::plan::body_budget())
                    .unwrap();
            assert_eq!(chunks.len(), 2);
            for chunk in chunks {
                let objects: Vec<AsyncScanObject> = chunk
                    .iter()
                    .map(|i| {
                        super::super::plan::scan_object(*i, &prompts[*i as usize], "p", "s")
                            .unwrap()
                    })
                    .collect();
                let (result, _) = api.submit(&objects).await;
                assert!(result.is_ok(), "the SDK sent the planned chunk");
            }
            assert_eq!(server.request_count(), 3);
        });
    }

    /// The no-progress bound (TS 120 polls), with an injected zero interval.
    #[test]
    fn polling_stops_after_the_no_progress_bound() {
        let temp = TempState::new();
        let dir = Dir::new();
        let mut job = submitted_job(&temp, &dir);
        let fake = Fake {
            results: RefCell::new(VecDeque::new()),
            calls: RefCell::new(0),
        };
        let session = scm::Session::new();
        let secrets = SecretSet::default();
        let engine = Engine {
            api: &fake,
            session: &session,
            secrets: &secrets,
            timing: Timing {
                poll_interval: Duration::ZERO,
                base_delay: Duration::ZERO,
                max_no_progress: 120,
            },
        };
        let (mut out, _, _) = crate::output::testing::capture(false);
        let error = runtime()
            .block_on(engine.poll_receipt(&mut job, &mut out, SCAN))
            .unwrap_err();
        assert_eq!(*fake.calls.borrow(), 120);
        let CliError::Operational(failure) = error else {
            panic!("operational")
        };
        assert_eq!(
            failure.message,
            format!("AIRS polling made no progress after 120 polls for scan {SCAN}")
        );
        assert_eq!(failure.details, [job.resume_detail()]);
    }

    /// A CSV temporary that cannot be written does not skip the results checkpoint:
    /// the rows are durable (without `output.next`) before the failure says so, and the
    /// CSV is left as it was.
    #[test]
    fn a_failed_csv_temporary_still_checkpoints_the_results() {
        let temp = TempState::new();
        let dir = Dir::new();
        let mut job = submitted_job(&temp, &dir);
        let resolved = correlate_rows(
            &rows(
                json!([{"req_id": 1, "status": "complete", "result": verdict("allow", json!({}))}]),
            ),
            SCAN,
            None,
            &open(&[0, 1, 2]),
            &SecretSet::default(),
        )
        .unwrap()
        .resolved;
        for (index, result) in resolved {
            let item = &mut job.state.items[index as usize];
            item.status = ItemStatus::Complete;
            item.result = Some(result);
        }
        let (mut out, _, _) = crate::output::testing::capture(false);
        super::super::checkpoint::fail_at(Some(super::super::checkpoint::Step::OutputTemp));
        let error = job.save_with_output(&mut out, &SecretSet::default());
        super::super::checkpoint::fail_at(None);
        let message = error.unwrap_err().message();
        assert_eq!(
            message,
            format!(
                "Cannot write {}; the results are saved in {}: run 'airs runtime resume-poll {}' to write it again",
                job.output.path().display(),
                job.state_shown(),
                job.state_shown()
            )
        );
        let saved = super::super::state::parse(&std::fs::read(job.file.path()).unwrap()).unwrap();
        assert_eq!(saved.items[1].status, ItemStatus::Complete);
        assert!(saved.output.next.is_none());
        assert_eq!(
            std::fs::read_to_string(job.output.path()).unwrap(),
            super::super::output::HEADER
        );
    }

    /// int-spec #20: rows are checkpointed (with the CSV) as each response resolves
    /// them; progress resets the no-progress count.
    #[test]
    fn results_are_checkpointed_per_response() {
        let temp = TempState::new();
        let dir = Dir::new();
        let mut job = submitted_job(&temp, &dir);
        let responses = VecDeque::from([
            Ok(rows(
                json!([{"req_id": 1, "status": "complete", "result": verdict("allow", json!({}))}]),
            )),
            Ok(rows(json!([{"req_id": 0, "status": "pending"}]))),
            Ok(rows(json!([
                {"req_id": 0, "status": "complete", "result": verdict("block", json!({"prompt_detected": {"injection": true}}))},
                {"req_id": 2, "status": "failed"}
            ]))),
        ]);
        let fake = Fake {
            results: RefCell::new(responses),
            calls: RefCell::new(0),
        };
        let session = scm::Session::new();
        let secrets = SecretSet::default();
        let engine = Engine {
            api: &fake,
            session: &session,
            secrets: &secrets,
            timing: Timing {
                poll_interval: Duration::ZERO,
                base_delay: Duration::ZERO,
                max_no_progress: 2,
            },
        };
        let (mut out, _, _) = crate::output::testing::capture(false);
        let sequence = job.state.sequence;
        runtime()
            .block_on(engine.poll_receipt(&mut job, &mut out, SCAN))
            .unwrap();
        assert_eq!(*fake.calls.borrow(), 3);
        assert_eq!(
            job.state.sequence,
            sequence + 2,
            "one checkpoint per resolving response"
        );
        let saved = super::super::state::parse(&std::fs::read(job.file.path()).unwrap()).unwrap();
        let statuses: Vec<ItemStatus> = saved.items.iter().map(|item| item.status).collect();
        assert_eq!(
            statuses,
            [
                ItemStatus::Complete,
                ItemStatus::Complete,
                ItemStatus::Failed
            ]
        );
        let csv = std::fs::read_to_string(job.output.path()).unwrap();
        assert_eq!(csv.lines().count(), 4);
        assert_eq!(csv, super::super::output::projection(&saved));
    }
}
