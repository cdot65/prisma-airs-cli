//! The scan verdict: a synchronous scan response normalised to allow/block/failed with
//! its detection flags (DESIGN §8.3), shared by `runtime scan` and evidence collection.

use crate::error::SecretSet;
use crate::signal::evidence::Event;
use scm::airs::scan::models::ScanResponse;
use scm::data::Optional;
use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};

/// Longest server-provided `errors[]` part shown in a failure reason, in characters.
pub const MAX_REASON_PART: usize = 200;

/// Prompt detection keys, in report order.
pub const PROMPT_KEYS: [&str; 8] = [
    "topic_violation",
    "injection",
    "toxic_content",
    "dlp",
    "url_cats",
    "malicious_code",
    "source_code",
    "agent",
];

/// Response detection keys, in report order.
pub const RESPONSE_KEYS: [&str; 8] = [
    "topic_violation",
    "toxic_content",
    "dlp",
    "url_cats",
    "malicious_code",
    "agent",
    "db_security",
    "ungrounded",
];

/// The CLI verdict. Anything but a clean allow/block fails closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Block,
    Failed,
}

impl Verdict {
    pub fn name(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Block => "block",
            Self::Failed => "failed",
        }
    }
}

/// Detection flags in fixed key order (serialised as an ordered object).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Flags(pub Vec<(&'static str, bool)>);

impl Flags {
    pub fn triggered(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.0.iter().filter(|(_, on)| *on).map(|(key, _)| *key)
    }
}

impl Serialize for Flags {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

/// The normalised scan result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Normalized {
    pub verdict: Verdict,
    pub category: String,
    pub triggered: bool,
    pub prompt: Flags,
    pub response: Flags,
    pub error: Option<String>,
}

fn flag(value: &Optional<bool>) -> Option<bool> {
    value.as_ref().into_option().copied()
}

fn prompt_flags(response: &ScanResponse) -> Flags {
    let Optional::Value(detected) = &response.prompt_detected else {
        return Flags::default();
    };
    // The explicit length ties this list to PROMPT_KEYS: adding a key to one and not
    // the other does not compile (the order is pinned by the all-detections tests).
    let values: [&Optional<bool>; PROMPT_KEYS.len()] = [
        &detected.topic_violation,
        &detected.injection,
        &detected.toxic_content,
        &detected.dlp,
        &detected.url_cats,
        &detected.malicious_code,
        &detected.source_code,
        &detected.agent,
    ];
    Flags(
        PROMPT_KEYS
            .into_iter()
            .zip(values)
            .filter_map(|(key, value)| flag(value).map(|on| (key, on)))
            .collect(),
    )
}

fn response_flags(response: &ScanResponse) -> Flags {
    let Optional::Value(detected) = &response.response_detected else {
        return Flags::default();
    };
    let values: [&Optional<bool>; RESPONSE_KEYS.len()] = [
        &detected.topic_violation,
        &detected.toxic_content,
        &detected.dlp,
        &detected.url_cats,
        &detected.malicious_code,
        &detected.agent,
        &detected.db_security,
        &detected.ungrounded,
    ];
    Flags(
        RESPONSE_KEYS
            .into_iter()
            .zip(values)
            .filter_map(|(key, value)| flag(value).map(|on| (key, on)))
            .collect(),
    )
}

/// One server-provided reason part: withheld when it carries a secret, bounded.
pub fn reason_part(text: &str, secrets: &SecretSet) -> String {
    let text = secrets.scrub(text);
    match text.char_indices().nth(MAX_REASON_PART) {
        Some((end, _)) => format!("{}...", &text[..end]),
        None => text.to_owned(),
    }
}

fn error_reason(response: &ScanResponse, secrets: &SecretSet) -> String {
    let entries: Vec<String> = response
        .errors
        .iter()
        .map(|entry| {
            [&entry.feature, &entry.status, &entry.content_type]
                .into_iter()
                .filter_map(|part| part.as_ref().into_option())
                .filter(|part| !part.is_empty())
                .map(|part| reason_part(part, secrets))
                .collect::<Vec<_>>()
                .join(": ")
        })
        .filter(|entry| !entry.is_empty())
        .collect();
    if !entries.is_empty() {
        entries.join("; ")
    } else if response.timeout {
        "AIRS scan timed out".to_owned()
    } else if response.error {
        "AIRS scan failed".to_owned()
    } else {
        format!(
            "Unknown AIRS action: {}",
            reason_part(&response.action, secrets)
        )
    }
}

/// Normalise a response: lower-cased allow/block pass through; error, timeout or any
/// other action fails closed with a reason. Server text that carries a configured
/// secret is withheld (DESIGN §8.3).
pub fn normalize(response: &ScanResponse, secrets: &SecretSet) -> Normalized {
    let action = response.action.to_lowercase();
    let failed = response.error || response.timeout || (action != "allow" && action != "block");
    let prompt = prompt_flags(response);
    let response_detections = response_flags(response);
    let triggered =
        prompt.triggered().next().is_some() || response_detections.triggered().next().is_some();
    Normalized {
        verdict: if failed {
            Verdict::Failed
        } else if action == "block" {
            Verdict::Block
        } else {
            Verdict::Allow
        },
        category: if failed {
            "error".to_owned()
        } else {
            secrets.scrub(&response.category).to_owned()
        },
        triggered,
        prompt,
        response: response_detections,
        error: failed.then(|| error_reason(response, secrets)),
    }
}

/// Fold a typed scan result into an evidence event: the API category note, the
/// fail-closed verdict and action, and the triggered detector hints (DESIGN §22.6).
pub fn apply_response(event: &mut Event, response: &ScanResponse, secrets: &SecretSet) {
    let normal = normalize(response, secrets);
    event.notes.push(format!(
        "API scan category: {}",
        if response.category.eq_ignore_ascii_case("benign") {
            "benign"
        } else if response.category.eq_ignore_ascii_case("malicious") {
            "malicious"
        } else {
            "unknown"
        }
    ));
    if response.timeout {
        event.notes.push("Scanner reported timeout".into());
    }
    if response.error {
        event.notes.push("Scanner reported an error".into());
    }
    event.report_ids = if response.report_id.len() <= 4096 {
        vec![response.report_id.clone()]
    } else {
        vec![]
    };
    event.verdict = if normal.verdict == Verdict::Failed {
        "error"
    } else if normal.triggered || response.category.eq_ignore_ascii_case("malicious") {
        "malicious"
    } else if response.category.eq_ignore_ascii_case("benign") {
        "benign"
    } else {
        "unknown"
    }
    .into();
    if event.verdict == "malicious" && !response.category.eq_ignore_ascii_case("malicious") {
        event.notes.push("Overall malicious classification is derived from detector flags; it is not the API category".into());
    }
    event.action = match normal.verdict {
        Verdict::Failed => "failed",
        Verdict::Allow => "allow",
        Verdict::Block => "block",
    }
    .into();
    for (direction, flags) in [("prompt", &normal.prompt), ("response", &normal.response)] {
        event
            .detector_hints
            .extend(flags.triggered().map(|key| format!("{direction}/{key}")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn response(value: serde_json::Value) -> ScanResponse {
        serde_json::from_value(value).unwrap()
    }

    fn base(extra: serde_json::Value) -> ScanResponse {
        let mut value = json!({
            "report_id": "R1", "scan_id": "S1", "category": "benign", "action": "allow",
            "timeout": false, "error": false, "errors": []
        });
        for (key, item) in extra.as_object().unwrap() {
            value[key] = item.clone();
        }
        response(value)
    }

    #[test]
    fn allow_and_block() {
        let none = SecretSet::default();
        let allow = normalize(&base(json!({})), &none);
        assert_eq!(allow.verdict, Verdict::Allow);
        assert_eq!(allow.category, "benign");
        assert!(!allow.triggered);
        assert_eq!(allow.error, None);

        let block = normalize(
            &base(json!({
                "action": "BLOCK", "category": "malicious",
                "prompt_detected": {"toxic_content": true, "injection": true, "dlp": false},
                "response_detected": {"ungrounded": true, "dlp": false}
            })),
            &none,
        );
        assert_eq!(block.verdict, Verdict::Block);
        assert_eq!(
            block.prompt.0,
            [("injection", true), ("toxic_content", true), ("dlp", false)]
        );
        assert_eq!(block.response.0, [("dlp", false), ("ungrounded", true)]);
        assert!(block.triggered);
    }

    #[test]
    fn response_detections_alone_trigger() {
        let result = normalize(
            &base(json!({"response_detected": {"db_security": true}})),
            &SecretSet::default(),
        );
        assert!(result.triggered);
        assert_eq!(result.verdict, Verdict::Allow);
    }

    #[test]
    fn failures_close() {
        let none = SecretSet::default();
        let failed = normalize(
            &base(json!({
                "error": true,
                "errors": [{"content_type": "prompt", "feature": "dlp", "status": "error"}, {"feature": "tc"}, {}]
            })),
            &none,
        );
        assert_eq!(failed.verdict, Verdict::Failed);
        assert_eq!(failed.category, "error");
        assert_eq!(failed.error.as_deref(), Some("dlp: error: prompt; tc"));
        let timeout = normalize(&base(json!({"timeout": true, "action": "block"})), &none);
        assert_eq!(timeout.verdict, Verdict::Failed);
        assert_eq!(timeout.error.as_deref(), Some("AIRS scan timed out"));
        let error = normalize(&base(json!({"error": true})), &none);
        assert_eq!(error.error.as_deref(), Some("AIRS scan failed"));
        let unknown = normalize(&base(json!({"action": "Alert"})), &none);
        assert_eq!(unknown.verdict, Verdict::Failed);
        assert_eq!(unknown.error.as_deref(), Some("Unknown AIRS action: Alert"));
    }
}
