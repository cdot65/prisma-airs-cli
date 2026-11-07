//! Receipt mapping: scan API responses already received, normalised into bounded
//! archive observations (plan M1.3, M4.5). No HTTP client and no CLI I/O: the caller
//! hands over typed SDK responses and decides how to report the outcome. Receipts are
//! client assertions; the archive keeps their source (`cli_*`) so server-observed
//! corroboration decides authority (RUNTIME_MCP_CONTRACT §4.3).
use super::{
    identity::Cloud,
    repository::{self, Received},
};
use airs_runtime_core::{
    error::SecretSet,
    signal::{
        evidence::{Detection, Event, Identity},
        privacy::PayloadSpans,
    },
    tenant::scanners::ScannerReference,
};
use scm::airs::scan::models::{
    AsyncScanObject, AsyncScanResponse, FiniteNumber, Optional, ScanIdResult, ScanResponse,
    ThreatScanReport,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct Receipts(Arc<Inner>);
struct Inner {
    enabled: bool,
    context: Option<(PathBuf, Cloud)>,
    reference: Option<ScannerReference>,
    endpoint: String,
    run: String,
    kind: &'static str,
    secrets: SecretSet,
    payload: PayloadSpans,
    counts: Mutex<Counts>,
    job: Mutex<Option<JobScope>>,
    pending: Mutex<(Vec<Received>, usize)>,
}
struct JobScope {
    id: String,
    items: BTreeMap<u64, Option<String>>,
}
#[derive(Default)]
struct Counts {
    received: usize,
    unconfirmed: usize,
    retained: bool,
    finished: bool,
}
impl Receipts {
    /// A recorder for one run of `kind` (`scan`, `bulk`, `resume`). `archive` is the
    /// admitted root and public cloud identity; `None` while `enabled` means the archive
    /// could not be admitted, so every received observation counts as unconfirmed.
    pub fn prepare(
        enabled: bool,
        archive: Option<(PathBuf, Cloud)>,
        reference: Option<ScannerReference>,
        endpoint: &str,
        kind: &'static str,
        (secrets, payload): (&SecretSet, &PayloadSpans),
    ) -> Self {
        let context = archive.filter(|_| enabled);
        Self(Arc::new(Inner {
            enabled,
            context,
            reference,
            endpoint: endpoint.trim_end_matches('/').into(),
            run: uuid::Uuid::new_v4().to_string(),
            kind,
            secrets: secrets.clone(),
            payload: payload.clone(),
            counts: Mutex::new(Counts::default()),
            job: Mutex::new(None),
            pending: Mutex::new((Vec::new(), 0)),
        }))
    }
    pub fn payload(&self, text: &str) {
        if self.0.enabled {
            self.0.payload.add(text);
        }
    }
    /// Scope later receipts to a bulk job: its ID and each request index's scan ID.
    pub fn job(&self, id: &str, items: impl IntoIterator<Item = (u64, Option<String>)>) {
        *self
            .0
            .job
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(JobScope {
            id: id.to_owned(),
            items: items.into_iter().collect(),
        });
    }
    fn counts(&self) -> std::sync::MutexGuard<'_, Counts> {
        self.0
            .counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn event(
        &self,
        scan: &str,
        request: Option<u64>,
        session: Option<&str>,
        profile: Option<&str>,
        source: &'static str,
    ) -> Received {
        let job = self
            .0
            .job
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let receipt_key = match (&*job, request) {
            (Some(job), Some(request))
                if job.items.get(&request).is_some_and(|id| {
                    source == "cli_submit"
                        || id.as_ref().is_some_and(|id| {
                            uuid::Uuid::parse_str(id)
                                .ok()
                                .zip(uuid::Uuid::parse_str(scan).ok())
                                .is_some_and(|(a, b)| a == b)
                        })
                }) =>
            {
                format!("job/{}/item/{request}", job.id)
            }
            (None, _) if source == "cli_scan" => format!("invocation/{}/item/0", self.0.run),
            _ => format!("unlinked/{}", uuid::Uuid::new_v4()),
        };
        Received {
            receipt_key,
            batch_report_id: None,
            event: Event {
                identity: Identity {
                    app_id: self
                        .0
                        .reference
                        .as_ref()
                        .map(|r| r.application_id.clone())
                        .unwrap_or_default(),
                    app_name: String::new(),
                    session_id: session
                        .filter(|s| s.len() <= 4096)
                        .unwrap_or_default()
                        .into(),
                    scan_id: if scan.len() <= 4096 {
                        scan.into()
                    } else {
                        String::new()
                    },
                    sub_request: request,
                },
                timestamp_ms: None,
                profile: profile.filter(|s| s.len() <= 4096).map(str::to_owned),
                verdict: "unknown".into(),
                action: "unknown".into(),
                report_ids: vec![],
                detections: vec![],
                detector_hints: vec![],
                sources: vec![source.into()],
                notes: vec![],
            },
            source,
            endpoint: self.0.endpoint.clone(),
            reference: self.0.reference.clone(),
        }
    }
    fn capture(&self, inputs: impl IntoIterator<Item = Received>) {
        if !self.0.enabled {
            return;
        }
        let mut pending = self
            .0
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for input in inputs {
            // Only normalized allowlisted models are serialized, never SDK bodies.
            let size = serde_json::to_vec(&input.event)
                .map(|v| v.len())
                .unwrap_or(usize::MAX / 2)
                .saturating_add(
                    input
                        .reference
                        .as_ref()
                        .and_then(|r| serde_json::to_vec(r).ok())
                        .map(|v| v.len())
                        .unwrap_or(0),
                )
                .saturating_add(input.endpoint.len())
                .saturating_add(input.batch_report_id.as_ref().map(String::len).unwrap_or(0))
                .saturating_add(2048);
            if size > 256 * 1024
                || pending.0.len() >= 10000
                || pending.1.saturating_add(size) > 4 * 1024 * 1024
            {
                let mut c = self.counts();
                c.received = c.received.saturating_add(1);
                c.unconfirmed = c.unconfirmed.saturating_add(1);
                continue;
            }
            pending.1 += size;
            pending.0.push(input);
        }
    }
    /// Write the captured observations (after the caller's own checkpoint).
    pub fn flush(&self) {
        let inputs = {
            let mut pending = self
                .0
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            pending.1 = 0;
            std::mem::take(&mut pending.0)
        };
        let mut batch = Vec::new();
        let mut bytes = 0;
        for input in inputs {
            let size = serde_json::to_vec(&input.event)
                .map(|v| v.len())
                .unwrap_or(256 * 1024)
                + input.endpoint.len()
                + input.batch_report_id.as_ref().map(String::len).unwrap_or(0)
                + input
                    .reference
                    .as_ref()
                    .and_then(|r| serde_json::to_vec(r).ok())
                    .map(|v| v.len())
                    .unwrap_or(0)
                + 2048;
            if !batch.is_empty() && (batch.len() >= 200 || bytes + size > 256 * 1024) {
                self.batch(&batch);
                batch.clear();
                bytes = 0;
            }
            bytes += size;
            batch.push(input);
        }
        if !batch.is_empty() {
            self.batch(&batch);
        }
    }
    fn batch(&self, batch: &[Received]) {
        let ok = self.0.context.as_ref().is_some_and(|(root, cloud)| {
            repository::retain_received(
                root,
                cloud,
                &self.0.run,
                self.0.kind,
                batch,
                &self.0.secrets,
                &self.0.payload,
            )
            .is_ok()
        });
        let mut c = self.counts();
        c.received = c.received.saturating_add(batch.len());
        if ok {
            c.retained = true;
        } else {
            c.unconfirmed = c.unconfirmed.saturating_add(batch.len());
        }
    }
    /// Close the run: abandoned unflushed observations are unconfirmed, and the run
    /// state is `interrupted`, `partial` or `complete`. `None` when disabled or already
    /// finished; the caller reports [`Finished::warning`].
    pub fn finish(&self, session: &scm::Session, failed: bool) -> Option<Finished> {
        if !self.0.enabled {
            return None;
        }
        // Unflushed metadata means the essential bulk checkpoint did not succeed.
        // Never write the secondary archive ahead of the resumable state.
        let abandoned = {
            let mut p = self
                .0
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            p.1 = 0;
            std::mem::take(&mut p.0).len()
        };
        let (retained, unconfirmed) = {
            let mut c = self.counts();
            if c.finished {
                return None;
            }
            c.finished = true;
            c.received = c.received.saturating_add(abandoned);
            c.unconfirmed = c.unconfirmed.saturating_add(abandoned);
            (c.retained, c.unconfirmed)
        };
        let state = if session.is_cancelled() {
            "interrupted"
        } else if failed || unconfirmed > 0 {
            "partial"
        } else {
            "complete"
        };
        let finalization_failed = retained
            && self.0.context.as_ref().is_none_or(|(root, cloud)| {
                repository::finish_received(root, cloud, &self.0.run, state).is_err()
            });
        Some(Finished {
            state,
            unconfirmed,
            finalization_failed,
        })
    }
    pub fn sync_response(&self, response: &ScanResponse, profile: &str) {
        if !self.0.enabled {
            return;
        }
        let mut input = self.event(
            &response.scan_id,
            None,
            text(&response.session_id),
            text(&response.profile_name).or(Some(profile)),
            "cli_scan",
        );
        airs_runtime_core::scan_verdict::apply_response(
            &mut input.event,
            response,
            &self.0.secrets,
        );
        if text(&response.profile_name).is_none() {
            input.event.notes.push(
                "Profile is explicit CLI request context, not an observed policy revision".into(),
            );
        }
        self.capture([input]);
        self.flush();
    }
    pub fn submission(&self, objects: &[AsyncScanObject], response: &AsyncScanResponse) {
        self.capture(objects.iter().map(|object| {
            let mut input = self.event(
                &response.scan_id,
                integer(&object.req_id),
                text(&object.scan_req.session_id),
                text(&object.scan_req.ai_profile.profile_name),
                "cli_submit",
            );
            input.batch_report_id = text(&response.report_id)
                .filter(|s| s.len() <= 4096)
                .map(str::to_owned);
            input
        }));
    }
    pub fn results(&self, requested: &str, rows: &[ScanIdResult]) {
        self.capture(rows.iter().map(|row| {
            let scan = text(&row.scan_id).unwrap_or(requested);
            let mut input = self.event(
                scan,
                optional_integer(&row.req_id),
                None,
                None,
                "cli_results",
            );
            let status = text(&row.status).unwrap_or_default().to_ascii_uppercase();
            input.event.notes.push(format!(
                "Scanner lookup status: {}",
                match status.as_str() {
                    "COMPLETE" | "COMPLETED" => "complete",
                    "PENDING" | "QUEUED" | "RUNNING" => "pending",
                    "FAILED" | "ERROR" => "failed",
                    _ => "unknown",
                }
            ));
            if let Optional::Value(response) = &row.result {
                // Refuse unsupported request/row/nested identity joins. Preserve a
                // separate unknown observation; no result is attributed by guessing.
                if scan == requested
                    && (response.scan_id.is_empty() || response.scan_id == requested)
                {
                    input.event.identity.session_id =
                        text(&response.session_id).unwrap_or_default().into();
                    input.event.profile = text(&response.profile_name).map(str::to_owned);
                    if ["COMPLETE", "COMPLETED"].contains(&status.as_str()) {
                        airs_runtime_core::scan_verdict::apply_response(
                            &mut input.event,
                            response,
                            &self.0.secrets,
                        );
                    }
                } else {
                    input.event.identity.scan_id.clear();
                    input.event.identity.sub_request = None;
                    input.receipt_key = format!("unlinked/{}", uuid::Uuid::new_v4());
                }
            }
            if ["FAILED", "ERROR"].contains(&status.as_str()) {
                input.event.verdict = "error".into();
                input.event.action = "failed".into();
            }
            if scan != requested {
                input.event.identity.scan_id.clear();
                input.event.identity.sub_request = None;
                input.receipt_key = format!("unlinked/{}", uuid::Uuid::new_v4());
            }
            input
        }));
    }
    pub fn reports(&self, requested: &[String], rows: &[ThreatScanReport]) {
        self.capture(rows.iter().map(|row| {
            let report = text(&row.report_id).filter(|s| requested.iter().any(|r| r == s));
            let mut input = self.event(
                if report.is_some() {
                    text(&row.scan_id).unwrap_or_default()
                } else {
                    ""
                },
                optional_integer(&row.req_id),
                text(&row.session_id),
                None,
                "cli_reports",
            );
            if let Some(report) = report {
                input.event.report_ids.push(report.into());
                let mut complete=true;
                if let Optional::Value(detections) = &row.detection_results {
                    complete=detections.len()<=1000;
                    for (index, d) in detections.iter().take(1000).enumerate() {
                        let detector=match text(&d.detection_service).filter(|s|!s.is_empty()&&s.len()<=4096){Some(s)=>s,None=>{complete=false;"unknown"}};
                        let lower=detector.to_ascii_lowercase();
                        let normalized=if airs_runtime_core::signal::detectors::known(&lower){airs_runtime_core::signal::detectors::canonical(&lower).to_owned()}else{detector.to_owned()};
                        if normalized!=detector {input.event.notes.push(format!("Raw report detector service: {detector}"));}
                        let direction = match text(&d.data_type).unwrap_or_default().to_ascii_lowercase().as_str() {
                            "prompt" => "prompt",
                            "response" => "response",
                            "tool" => "tool",
                            _ => "unknown",
                        };
                        let verdict = match text(&d.verdict).unwrap_or_default().to_ascii_lowercase().as_str() {
                            "malicious" | "unsafe" | "violation" | "detected" => "malicious",
                            "benign" => "benign",
                            "error" | "failed" => "error",
                            _ => "unknown",
                        };
                        let action = match text(&d.action).unwrap_or_default().to_ascii_lowercase().as_str() {
                            "allow" => "allow",
                            "block" => "block",
                            _ => "unknown",
                        };
                        input.event.detections.push(Detection {
                            detector: normalized,
                            direction: direction.into(),
                            verdict: verdict.into(),
                            action: action.into(),
                            report_id: report.into(),
                            row: index as u64,
                            snippets: vec![],
                        });
                    }
                }
                input.event.verdict=if input.event.detections.iter().any(|d|d.verdict=="error"){"error"}else if input.event.detections.iter().any(|d|d.verdict=="malicious"){"malicious"}else if complete && !input.event.detections.is_empty()&&input.event.detections.iter().all(|d|d.verdict=="benign"){"benign"}else{"unknown"}.into();
                let actions:std::collections::BTreeSet<_>=input.event.detections.iter().map(|d|d.action.as_str()).collect();
                input.event.action=if input.event.verdict=="error"{"failed"}else if complete && actions.len()==1{actions.first().copied().unwrap_or("unknown")}else{"unknown"}.into();
                if !complete{input.event.notes.push("Detector metadata is incomplete after the bounded projection".into());}
                input.event.notes.push("Overall report outcome derives from the available detector rows; missing actions remain unknown".into());
            } else {
                input.receipt_key=format!("unlinked/{}",uuid::Uuid::new_v4());
                input.event.identity.sub_request=None;
            }
            input
        }));
    }
}
/// The outcome of one recorded run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finished {
    pub state: &'static str,
    pub unconfirmed: usize,
    pub finalization_failed: bool,
}

impl Finished {
    /// The one warning a run with unconfirmed observations or an unfinished run record
    /// reports, or `None`.
    pub fn warning(&self) -> Option<String> {
        if self.unconfirmed == 0 && !self.finalization_failed {
            return None;
        }
        let detail = if self.unconfirmed > 0 {
            format!(
                "retention could not be confirmed for {} received observation(s)",
                self.unconfirmed
            )
        } else {
            "run completion could not be confirmed; retained observations remain available".into()
        };
        Some(format!(
            "Runtime history: {detail}{}",
            if self.finalization_failed {
                "; the run remains incomplete"
            } else {
                ""
            }
        ))
    }
}

/// A JSON request index (`req_id`) as an exact integer up to 2^53.
fn request_index(number: &FiniteNumber) -> Option<u64> {
    const LIMIT: u64 = 1 << 53;
    if let Some(value) = number.as_u64() {
        return (value <= LIMIT).then_some(value);
    }
    let value = number.as_f64()?;
    (value.is_finite() && value >= 0.0 && value <= LIMIT as f64 && value.fract() == 0.0)
        .then_some(value as u64)
}

fn text(value: &Optional<String>) -> Option<&str> {
    value.as_ref().into_option().map(String::as_str)
}
fn integer(value: &FiniteNumber) -> Option<u64> {
    request_index(value)
}

fn optional_integer(value: &Optional<FiniteNumber>) -> Option<u64> {
    value.as_ref().into_option().and_then(integer)
}

#[cfg(test)]
#[path = "receipts_tests.rs"]
mod tests;
