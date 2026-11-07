//! Shared collection options and normalized scanner lookups.
use super::read_budget::ReadBudget;
use crate::{
    error::CliError,
    signal::{evidence::Snippet, evidence::SnippetState, privacy},
};
use scm::airs::scan::Scanner;
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    pub seconds: u64,
    pub concurrency: u64,
    pub requests: u64,
}
impl Limits {
    /// The `--timeout`, `--concurrency` and `--max-requests` values (DESIGN §17).
    pub fn parse(
        timeout: Option<&str>,
        concurrency: Option<&str>,
        max_requests: Option<&str>,
    ) -> Result<Self, CliError> {
        Ok(Self {
            seconds: number(timeout, 120, 1, 3600, "--timeout")?,
            concurrency: number(concurrency, 4, 1, 16, "--concurrency")?,
            requests: number(max_requests, 1000, 1, 10000, "--max-requests")?,
        })
    }
    pub fn budget(&self, session: scm::Session) -> Arc<ReadBudget> {
        ReadBudget::new(self.seconds, self.requests, self.concurrency, session)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionOptions {
    pub limits: Limits,
    pub interval: u64,
    pub unit: String,
    pub app_ids: Vec<String>,
    pub max_pages: u64,
    pub max_events: u64,
    pub max_reports: u64,
}
pub fn number(
    value: Option<&str>,
    default: u64,
    min: u64,
    max: u64,
    flag: &str,
) -> Result<u64, CliError> {
    let Some(value) = value else {
        return Ok(default);
    };
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CliError::usage(format!(
            "{flag} must be an integer from {min} to {max}"
        )));
    }
    value
        .parse::<u64>()
        .ok()
        .filter(|v| *v >= min && *v <= max)
        .ok_or_else(|| CliError::usage(format!("{flag} must be an integer from {min} to {max}")))
}
pub fn is_scan_id(id: &str) -> bool {
    id.len() == 36 && uuid::Uuid::parse_str(id).is_ok()
}
pub fn is_report_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 1024 && !id.contains(',') && !id.chars().any(char::is_control)
}
pub fn valid_ids(ids: &[String], scans: bool) -> Result<(), CliError> {
    if ids.is_empty()
        || ids.len() > 1000
        || ids.iter().any(|id| {
            id.is_empty()
                || id.len() > 1024
                || id.contains(',')
                || id.chars().any(char::is_control)
                || (scans && !is_scan_id(id))
        })
    {
        return Err(CliError::usage(
            "Supply 1–1000 valid scan UUIDs or opaque report IDs (at most 1024 bytes, no commas or controls)",
        ));
    }
    if !scans && ids.iter().collect::<BTreeSet<_>>().len() > 10 {
        return Err(CliError::usage(
            "Supply at most 10 distinct report IDs per invocation",
        ));
    }
    Ok(())
}
pub fn note(error: &scm::Error) -> String {
    match error.status() {
        Some(status) => {
            format!("API read unavailable (HTTP {status}); no response content retained")
        }
        None => match error.kind() {
            scm::ErrorKind::Deadline => "Collection deadline reached".into(),
            scm::ErrorKind::Limit => "Collection request or response limit reached".into(),
            scm::ErrorKind::Cancelled => "Collection interrupted".into(),
            _ => "API read unavailable or response did not match its contract".into(),
        },
    }
}
/// Complete wrapper plus clean explicit result. Failed/unknown outcomes never normalize to allow.
pub fn usable_result(row: &Value) -> bool {
    let status = row
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let Some(result) = row.get("result") else {
        return false;
    };
    matches!(status.as_str(), "complete" | "completed")
        && result.get("error").and_then(Value::as_bool) == Some(false)
        && result.get("timeout").and_then(Value::as_bool) == Some(false)
        && result
            .get("errors")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        && result
            .get("action")
            .and_then(Value::as_str)
            .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "allow" | "block"))
        && result
            .get("category")
            .and_then(Value::as_str)
            .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "benign" | "malicious"))
}
/// Typed field allowlist. This never serializes arbitrary SDK extensions or masked content.
pub fn result_view(value: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for key in ["scan_id", "req_id", "status", "source"] {
        if let Some(v) = value.get(key) {
            out.insert(key.into(), v.clone());
        }
    }
    if let Some(result) = value.get("result").and_then(Value::as_object) {
        let mut row = serde_json::Map::new();
        for key in [
            "report_id",
            "scan_id",
            "session_id",
            "tr_id",
            "profile_id",
            "profile_name",
            "category",
            "action",
            "timeout",
            "error",
            "created_at",
            "completed_at",
        ] {
            if let Some(v) = result.get(key) {
                row.insert(key.into(), v.clone());
            }
        }
        for name in ["prompt_detected", "response_detected", "tool_detected"] {
            if let Some(flags) = result.get(name).and_then(Value::as_object) {
                let mut safe = serde_json::Map::new();
                for key in crate::scan_verdict::PROMPT_KEYS
                    .iter()
                    .chain(crate::scan_verdict::RESPONSE_KEYS.iter())
                {
                    if let Some(v) = flags.get(*key).filter(|v| v.is_boolean()) {
                        safe.insert((*key).into(), v.clone());
                    }
                }
                row.insert(name.into(), Value::Object(safe));
            }
        }
        if !usable_result(value) {
            row.insert("action".into(), json!("failed"));
            row.insert("category".into(), json!("unknown"));
        }
        out.insert("result".into(), Value::Object(row));
    }
    Value::Object(out)
}
fn report_view(value: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for key in [
        "scan_id",
        "report_id",
        "req_id",
        "session_id",
        "transaction_id",
        "source",
    ] {
        if let Some(v) = value.get(key) {
            out.insert(key.into(), v.clone());
        }
    }
    let detections = value
        .get("detection_results")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let mut rows = Vec::new();
    for (index, d) in detections.iter().enumerate().take(1000) {
        let mut row = serde_json::Map::new();
        row.insert("row".into(), json!(index));
        for key in ["data_type", "detection_service", "verdict", "action"] {
            if let Some(v) = d.get(key) {
                row.insert(key.into(), v.clone());
            }
        }
        let detail = d.get("result_detail");
        let mut snippets = Vec::new();
        for (family, field) in [
            ("pi", "pi_snippets"),
            ("dlp", "dlp_snippets"),
            ("dbs", "dbs_snippets"),
            ("tc", "tc_snippets"),
        ] {
            let raw = detail.and_then(|v| v.get(field)).and_then(|v| {
                if family == "dlp" {
                    v.get("snippets")
                } else {
                    Some(v)
                }
            });
            let (state, excerpts) = match raw {
                None => (SnippetState::Absent, vec![]),
                Some(Value::Array(a)) if a.is_empty() => (SnippetState::Empty, vec![]),
                Some(Value::Array(a)) if a.iter().all(Value::is_string) => (
                    SnippetState::Present,
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect(),
                ),
                _ => (SnippetState::Unavailable, vec![]),
            };
            let variants = excerpts.iter().collect::<BTreeSet<_>>().len() as u64;
            let truncated = excerpts.len() > 20;
            let excerpts: Vec<_> = excerpts.into_iter().take(20).collect();
            snippets.push(Snippet {
                family: family.into(),
                state: if family == "dlp" && state == SnippetState::Present {
                    SnippetState::Withheld
                } else {
                    state
                },
                excerpts: if family == "dlp" { vec![] } else { excerpts },
                variants,
                truncated,
            });
        }
        row.insert(
            "snippets".into(),
            serde_json::to_value(snippets).unwrap_or(Value::Null),
        );
        rows.push(Value::Object(row));
    }
    out.insert("detections".into(), Value::Array(rows));
    Value::Object(out)
}
/// DLP matched text is payload data, not authentication material: keep its span
/// registry local to excerpts so a match such as "1" cannot corrupt UUIDs or schema tags.
pub fn report_views(values: &[Value], payload: &privacy::PayloadSpans) -> Vec<Value> {
    register_report_payloads(values, payload);
    values.iter().map(report_view).collect()
}

fn register_report_payloads(values: &[Value], payload: &privacy::PayloadSpans) {
    fn register(value: &Value, payload: &privacy::PayloadSpans) {
        match value {
            Value::String(text) => payload.add(text),
            Value::Array(a) => a.iter().for_each(|v| register(v, payload)),
            Value::Object(o) => o.values().for_each(|v| register(v, payload)),
            _ => {}
        }
    }
    for value in values {
        if let Some(rows) = value.get("detection_results").and_then(Value::as_array) {
            for row in rows {
                if let Some(dlp) = row.pointer("/result_detail/dlp_snippets") {
                    register(dlp, payload);
                }
            }
        }
    }
}

/// Discover all strings before projection; key allowlists are applied before this step.
pub fn project_document(
    value: &mut Value,
    secrets: &crate::error::SecretSet,
    payload: &privacy::PayloadSpans,
) {
    privacy::document(value, secrets, payload);
    fn cap(value: &mut Value) {
        match value {
            Value::Object(o) => {
                if let Some(Value::Array(a)) = o.get_mut("excerpts") {
                    a.truncate(20);
                    for v in a {
                        if let Value::String(s) = v {
                            *s = s.chars().take(512).collect();
                        }
                    }
                }
                for v in o.values_mut() {
                    cap(v)
                }
            }
            Value::Array(a) => a.iter_mut().for_each(cap),
            _ => {}
        }
    }
    cap(value);
}

#[derive(Default)]
pub struct LookupDiagnostics {
    pub requested_ids: u64,
    pub returned_rows: u64,
    pub pending_rows: u64,
    pub rejected_rows: u64,
    pub missing_ids: u64,
    pub empty_responses: u64,
    pub failed_reads: u64,
}

pub async fn lookup(
    scanner: &Scanner,
    budget: &Arc<ReadBudget>,
    ids: &[String],
    reports: bool,
    secrets: &crate::error::SecretSet,
    payload: Option<&privacy::PayloadSpans>,
) -> (Vec<Value>, Vec<String>, Vec<String>, LookupDiagnostics) {
    let mut requested = BTreeSet::new();
    let unique: Vec<_> = ids
        .iter()
        .filter(|id| requested.insert((*id).clone()))
        .cloned()
        .collect();
    let mut rows: Vec<Value> = Vec::new();
    let mut notes = Vec::new();
    let mut diagnostics = LookupDiagnostics {
        requested_ids: unique.len() as u64,
        ..Default::default()
    };
    let mut seen = BTreeSet::new();
    let mut ambiguous = BTreeSet::new();
    let identity = |row: &Value| {
        (
            row.get(if reports { "report_id" } else { "scan_id" })
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            if reports {
                None
            } else {
                row.get("req_id").and_then(Value::as_u64)
            },
        )
    };
    'batches: for chunk in unique.chunks(5) {
        if !budget.lookup_capacity() {
            notes.push("Aggregate lookup row/byte cap reached; remaining IDs uncollected".into());
            break;
        }
        let result = if reports {
            budget
                .run(|opts| async move {
                    scanner
                        .query_by_report_ids(chunk, Some(&opts))
                        .await
                        .and_then(|v| {
                            serde_json::to_value(v)
                                .map_err(|_| scm::Error::new(scm::ErrorKind::Decode))
                        })
                })
                .await
        } else {
            budget
                .run(|opts| async move {
                    scanner
                        .query_by_scan_ids(chunk, Some(&opts))
                        .await
                        .and_then(|v| {
                            serde_json::to_value(v)
                                .map_err(|_| scm::Error::new(scm::ErrorKind::Decode))
                        })
                })
                .await
        };
        match result {
            Ok(Value::Array(values)) => {
                diagnostics.returned_rows += values.len() as u64;
                diagnostics.empty_responses += u64::from(values.is_empty());
                if reports && let Some(payload) = payload {
                    register_report_payloads(&values, payload);
                }
                // Discover secret-shaped values in every received row before a
                // storage cap can discard later rows or abort this traversal.
                let safe: Vec<_> = values
                    .iter()
                    .map(|row| {
                        if let Ok(mut discovery) = crate::json::to_ordered(&row) {
                            crate::withhold::apply(
                                &mut discovery,
                                &crate::withhold::PLAIN,
                                secrets,
                            );
                            true
                        } else {
                            notes.push("Response could not be safely projected".into());
                            false
                        }
                    })
                    .collect();
                for (row, safe) in values.into_iter().zip(safe) {
                    if !safe {
                        continue;
                    }
                    let key = identity(&row);
                    if !chunk.contains(&key.0)
                        || (!reports
                            && row
                                .pointer("/result/scan_id")
                                .and_then(Value::as_str)
                                .is_some_and(|id| id != key.0))
                    {
                        notes.push("Unrequested or conflicting response identity excluded".into());
                        continue;
                    }
                    if !seen.insert(key.clone()) {
                        ambiguous.insert(key);
                        notes.push(
                            "Duplicate response identity is unresolved; conflicting rows excluded"
                                .into(),
                        );
                        continue;
                    }
                    let size = serde_json::to_vec(&row).map_or(usize::MAX, |v| v.len());
                    if !budget.retain_lookup_row(size) {
                        notes.push(
                            "Aggregate lookup row/byte cap reached; remaining IDs uncollected"
                                .into(),
                        );
                        break 'batches;
                    }
                    if reports {
                        match row.get("detection_results").and_then(Value::as_array) {
                            None => notes.push("Detailed report detector array unavailable".into()),
                            Some(a) if a.len() > 1000 => {
                                notes.push("Detailed report detector-row view cap reached".into())
                            }
                            _ => {}
                        }
                    }
                    if !reports && !usable_result(&row) {
                        notes.push("Scan result pending, failed, timed out, or has an unknown outcome; not normalized as allow".into());
                    }
                    rows.push(row);
                }
            }
            Ok(_) => {
                diagnostics.failed_reads += 1;
                notes.push("API returned no usable row array".into());
            }
            Err(error) => {
                diagnostics.failed_reads += 1;
                notes.push(note(&error));
                if (reports && error.status() == Some(429))
                    || matches!(
                        error.kind(),
                        scm::ErrorKind::Deadline
                            | scm::ErrorKind::Limit
                            | scm::ErrorKind::Cancelled
                    )
                {
                    break;
                }
            }
        }
    }
    rows.retain(|row| !ambiguous.contains(&identity(row)));
    notes.sort();
    notes.dedup();
    let key = if reports { "report_id" } else { "scan_id" };
    let missing: Vec<_> = unique
        .into_iter()
        .filter(|id| {
            !rows
                .iter()
                .any(|row| row.get(key).and_then(Value::as_str) == Some(id.as_str()))
        })
        .collect();
    diagnostics.missing_ids = missing.len() as u64;
    diagnostics.pending_rows = if reports {
        0
    } else {
        rows.iter()
            .filter(|row| {
                row.get("status")
                    .and_then(Value::as_str)
                    .is_some_and(|status| {
                        ["PENDING", "RUNNING", "IN_PROGRESS", "QUEUED", "PROCESSING"]
                            .iter()
                            .any(|known| status.eq_ignore_ascii_case(known))
                    })
            })
            .count() as u64
    };
    let usable = rows
        .iter()
        .filter(|row| {
            if reports {
                row.get("detection_results").is_some_and(Value::is_array)
            } else {
                usable_result(row)
            }
        })
        .count() as u64;
    diagnostics.rejected_rows = diagnostics
        .returned_rows
        .saturating_sub(usable + diagnostics.pending_rows);
    (rows, missing, notes, diagnostics)
}
