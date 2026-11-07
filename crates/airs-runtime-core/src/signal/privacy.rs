//! The shared projection for evidence, views, HTML and review packets.
use super::{
    evidence::{Evidence, SnippetState},
    invalid,
};
use crate::{
    error::{CliError, SecretSet},
    json::OValue,
};

pub const WITHHELD: &str = "[sensitive value withheld]";

pub fn text(input: &str, secrets: &SecretSet, max: usize) -> String {
    let mut value = OValue::String(input.to_owned());
    crate::withhold::apply(&mut value, &crate::withhold::PLAIN, secrets);
    let shown = value.as_str().unwrap_or(WITHHELD);
    let mut out = if secrets.carried_by(shown) {
        WITHHELD.to_owned()
    } else {
        shown.to_owned()
    };
    // Discover and withhold credentials before normalization, then test the exact
    // control-free text that could reach an artifact for personal identifiers.
    out.retain(|c| !c.is_control());
    // Controls can split an authorization scheme or credential. Discover the
    // normalized shape too, before it becomes printable or is truncated.
    let mut normalized = OValue::String(out);
    crate::withhold::apply(&mut normalized, &crate::withhold::PLAIN, secrets);
    let shown = normalized.as_str().unwrap_or(WITHHELD);
    let mut out = if secrets.carried_by(shown) {
        WITHHELD.to_owned()
    } else {
        shown.to_owned()
    };
    // Conservative personal identifier withholding; retain confidential prose warning.
    if out
        .split_whitespace()
        .any(|word| word.contains('@') || contains_ip(word))
    {
        out = WITHHELD.to_owned();
    }
    out.chars().take(max).collect()
}

fn contains_ip(word: &str) -> bool {
    let token = word.trim_matches(|c: char| {
        [
            '\"', '\'', '`', '(', ')', '{', '}', '<', '>', ',', ';', '!', '?',
        ]
        .contains(&c)
    });
    let token = token.trim_end_matches('.');
    let literal = token.trim_matches(['[', ']']);
    literal.parse::<std::net::IpAddr>().is_ok()
        || token.parse::<std::net::SocketAddr>().is_ok()
        || url::Url::parse(token)
            .is_ok_and(|url| matches!(url.host(), Some(url::Host::Ipv4(_) | url::Host::Ipv6(_))))
        || word
            .trim_matches(|c: char| !c.is_ascii_digit() && c != '.')
            .parse::<std::net::IpAddr>()
            .is_ok()
}

/// Payload matches have different semantics from authentication credentials. Short
/// matches apply to free text, but not arbitrary substrings of protocol identities.
#[derive(Clone, Default)]
pub struct PayloadSpans {
    all: SecretSet,
    identifiers: SecretSet,
    state: std::sync::Arc<std::sync::Mutex<PayloadState>>,
}
#[derive(Default)]
struct PayloadState {
    values: std::collections::BTreeSet<String>,
    bytes: usize,
    exceeded: bool,
}
impl PayloadSpans {
    pub fn add(&self, value: &str) {
        self.add_form(value);
        let normalized: String = value.chars().filter(|c| !c.is_control()).collect();
        if normalized != value {
            self.add_form(&normalized);
        }
    }
    fn add_form(&self, value: &str) {
        if value.is_empty() {
            return;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.exceeded || state.values.contains(value) {
            return;
        }
        if state.values.len() >= 10000 || state.bytes.saturating_add(value.len()) > 4 * 1024 * 1024
        {
            state.exceeded = true;
            return;
        }
        state.bytes += value.len();
        state.values.insert(value.to_owned());
        drop(state);
        self.all.add(value);
        if value.chars().count() >= 8 {
            self.identifiers.add(value);
        }
    }
    pub fn exceeded(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .exceeded
    }
    fn redact(&self, value: &str, key: &str) -> String {
        let semantic = match key {
            "confidence" => ["explicit", "unknown"].contains(&value),
            "direction" | "data_type" => [
                "prompt",
                "response",
                "tool",
                "tool-call",
                "tool-response",
                "unknown",
            ]
            .contains(&value),
            "status" => ["complete", "partial", "unavailable"].contains(&value),
            "state" => ["absent", "empty", "present", "withheld", "unavailable"].contains(&value),
            "action" => ["allow", "block", "unknown", "failed"].contains(&value),
            "verdict" => ["malicious", "benign", "unknown", "error"].contains(&value),
            "unit" => ["hour", "hours", "day", "days"].contains(&value),
            "family" => ["pi", "dlp", "dbs", "tc"].contains(&value),
            "detector" | "detection_service" => super::detectors::known(value),
            "detector_hints" => value.split_once('/').is_some_and(|(direction, detector)| {
                ["prompt", "response", "tool", "unknown"].contains(&direction)
                    && super::detectors::known(detector)
            }),
            "sdk_version" => value == "0.2.4",
            "cli_version" => value == crate::VERSION,
            "algorithm" => matches!(value, "stratified-round-robin-v1" | "coverage-greedy-v2"),
            "name" | "sources" | "source" => [
                "sessions",
                "session-details",
                "session-transactions",
                "scan-results",
                "scan-reports",
                "profiles",
                "applications",
                "registered-apps",
                "application-telemetry",
                "event-window",
                "evidence-storage",
            ]
            .contains(&value),
            _ => false,
        };
        if semantic {
            return value.to_owned();
        }
        if [
            "scan_id",
            "report_id",
            "report_ids",
            "selected_report_ids",
            "app_id",
            "app_ids",
            "app_name",
            "profile",
            "name",
            "untrusted_name",
            "id",
            "session_id",
            "timestamp",
            "collection_id",
            "digest",
        ]
        .contains(&key)
        {
            if self.all.contains_exact_form(value) {
                return WITHHELD.into();
            }
            return if self.identifiers.carried_by(value) {
                WITHHELD.into()
            } else {
                value.to_owned()
            };
        }
        if self.all.carried_by(value) {
            WITHHELD.into()
        } else {
            value.to_owned()
        }
    }
}
pub fn text_with_spans(
    input: &str,
    secrets: &SecretSet,
    payload: &PayloadSpans,
    key: &str,
    max: usize,
) -> String {
    let discovered = text(input, secrets, usize::MAX);
    // Discovery sees the original first; raw matching must precede normalization,
    // which may remove a newline or other control inside a sensitive payload span.
    let raw = payload.redact(input, key);
    if raw != input {
        return raw.chars().take(max).collect();
    }
    payload.redact(&discovered, key).chars().take(max).collect()
}
pub fn document(value: &mut serde_json::Value, secrets: &SecretSet, payload: &PayloadSpans) {
    fn walk(v: &mut serde_json::Value, secrets: &SecretSet, payload: &PayloadSpans, key: &str) {
        match v {
            serde_json::Value::String(s) => {
                let shown = text_with_spans(s, secrets, payload, key, 4096);
                *s = if [
                    "action",
                    "verdict",
                    "confidence",
                    "direction",
                    "data_type",
                    "detector",
                    "detection_service",
                    "detector_hints",
                ]
                .contains(&key)
                    && shown != *s
                {
                    "unknown".into()
                } else {
                    shown
                };
            }
            serde_json::Value::Array(a) => {
                a.iter_mut().for_each(|v| walk(v, secrets, payload, key))
            }
            serde_json::Value::Object(o) => {
                let provenance = o.contains_key("unit")
                    && o.contains_key("population")
                    && o.contains_key("status");
                let source_definition = o.contains_key("required") && provenance;
                for (key, v) in o.iter_mut() {
                    let kind = if key == "source" && !provenance {
                        "untrusted_source"
                    } else if key == "name" && !source_definition {
                        "untrusted_name"
                    } else {
                        key.as_str()
                    };
                    walk(v, secrets, payload, kind);
                }
            }
            _ => {}
        }
    }
    let mut discovery = value.clone();
    walk(&mut discovery, secrets, payload, "");
    walk(value, secrets, payload, "");
}

#[cfg(test)]
pub fn project(evidence: &mut Evidence, secrets: &SecretSet) -> Result<bool, CliError> {
    project_with_spans(evidence, secrets, &PayloadSpans::default())
}

/// Correlation strings must preserve equality and inequality after masking. Semantic
/// labels additionally retain their exact meaning; normalization may not invent allow.
fn correlation_keys(e: &Evidence) -> Vec<(&str, bool)> {
    let mut out = vec![];
    out.push((e.window.unit.as_str(), true));
    for s in &e.sources {
        out.push((s.name.as_str(), true));
    }
    for m in &e.metrics {
        out.extend([
            (m.source.as_str(), true),
            (m.name.as_str(), true),
            (m.unit.as_str(), true),
            (m.window.as_str(), true),
            (m.population.as_str(), false),
        ]);
        if let Some(scale) = &m.scale {
            out.push((scale.as_str(), true));
        }
    }
    for id in &e.app_ids {
        out.push((id.as_str(), false));
    }
    for id in &e.sampling.selected_report_ids {
        out.push((id.as_str(), false));
    }
    for a in &e.applications {
        out.extend([(a.id.as_str(), false), (a.name.as_str(), false)]);
        if let Some(p) = &a.profile {
            out.push((p.as_str(), false));
        }
    }
    for p in &e.profiles {
        out.push((p.name.as_str(), false));
        for c in &p.protections {
            out.extend([
                (c.detector.as_str(), true),
                (c.direction.as_str(), true),
                (c.action.as_str(), true),
                (c.confidence.as_str(), true),
            ]);
        }
    }
    for event in &e.events {
        let id = &event.identity;
        out.extend([
            (id.app_id.as_str(), false),
            (id.app_name.as_str(), false),
            (id.session_id.as_str(), false),
            (id.scan_id.as_str(), false),
            (event.action.as_str(), true),
            (event.verdict.as_str(), true),
        ]);
        if let Some(p) = &event.profile {
            out.push((p.as_str(), false));
        }
        for value in &event.sources {
            out.push((value.as_str(), true));
        }
        for value in &event.report_ids {
            out.push((value.as_str(), false));
        }
        for value in &event.detector_hints {
            out.push((value.as_str(), true));
        }
        for d in &event.detections {
            out.extend([
                (d.report_id.as_str(), false),
                (d.detector.as_str(), true),
                (d.direction.as_str(), true),
                (d.action.as_str(), true),
                (d.verdict.as_str(), true),
            ]);
        }
    }
    out
}
/// Returns whether extra projection was necessary, for offline bundle refusal.
pub fn project_with_spans(
    evidence: &mut Evidence,
    secrets: &SecretSet,
    payload: &PayloadSpans,
) -> Result<bool, CliError> {
    let original_keys: Vec<_> = correlation_keys(evidence)
        .into_iter()
        .map(|(s, semantic)| (s.to_owned(), semantic))
        .collect();
    let original = serde_json::to_value(&*evidence).map_err(|_| invalid())?;
    for event in &mut evidence.events {
        for detection in &mut event.detections {
            for snippet in &mut detection.snippets {
                if snippet.family == "dlp" {
                    for excerpt in &snippet.excerpts {
                        payload.add(excerpt);
                    }
                    if snippet.state == SnippetState::Present {
                        snippet.state = SnippetState::Withheld;
                    }
                    snippet.excerpts.clear();
                }
            }
        }
    }
    if payload.exceeded() {
        return Err(invalid());
    }
    let mut value = serde_json::to_value(&*evidence).map_err(|_| invalid())?;
    document(&mut value, secrets, payload);
    *evidence = serde_json::from_value(value).map_err(|_| invalid())?;
    for event in &mut evidence.events {
        for detection in &mut event.detections {
            for snippet in &mut detection.snippets {
                if snippet.excerpts.len() > 20 {
                    snippet.truncated = true;
                    snippet.excerpts.truncate(20);
                }
                for excerpt in &mut snippet.excerpts {
                    if excerpt.chars().count() > 512 {
                        snippet.truncated = true;
                    }
                    *excerpt = excerpt.chars().take(512).collect();
                }
                if snippet.state == SnippetState::Present && snippet.excerpts.is_empty() {
                    snippet.state = SnippetState::Empty;
                }
            }
        }
    }
    let projected_keys = correlation_keys(evidence);
    if original_keys.len() != projected_keys.len() {
        return Err(invalid());
    }
    let mut originals = std::collections::BTreeMap::new();
    for ((raw, semantic), (shown, _)) in original_keys.iter().zip(projected_keys) {
        if (*semantic && raw != shown)
            || originals
                .insert(shown, raw.as_str())
                .is_some_and(|prior| prior != raw)
        {
            return Err(invalid());
        }
    }
    Ok(original != serde_json::to_value(evidence).map_err(|_| invalid())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Evidence {
        Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap()
    }
    #[test]
    fn control_split_authorization_is_discovered_before_publication() {
        let secrets = SecretSet::default();
        let canary = "NORMALIZED-AUTH-CANARY-0123456789";
        let shown = text(
            &format!("Authorization: Bea\u{1}rer {canary}"),
            &secrets,
            512,
        );
        assert!(!shown.contains(canary));
        assert!(secrets.carried_by(canary));
        let mut e = fixture();
        e.events[0].identity.scan_id = canary.into();
        e.events[0]
            .notes
            .push(format!("Authorization: Bea\u{1}rer {canary}"));
        project(&mut e, &SecretSet::default()).unwrap();
        assert!(!serde_json::to_string(&e).unwrap().contains(canary));
    }
    #[test]
    fn masked_profile_collisions_never_create_monitoring_associations() {
        let mut e = fixture();
        e.events[0].profile = Some("blocker@example.invalid".into());
        e.profiles[0].name = "blocker@example.invalid".into();
        e.profiles[0].protections[0].action = "block".into();
        let mut unrelated = e.profiles[0].clone();
        unrelated.name = "monitor@example.invalid".into();
        unrelated.revision = Some(2);
        unrelated.protections[0].action = "allow".into();
        e.profiles.push(unrelated);
        e.seal().unwrap();
        assert!(project(&mut e, &SecretSet::default()).is_err());
        // Equality for one safely masked name is preserved, so legitimate links remain usable.
        let mut e = fixture();
        e.events[0].profile = Some("monitor@example.invalid".into());
        e.profiles[0].name = "monitor@example.invalid".into();
        assert!(project(&mut e, &SecretSet::default()).unwrap());
        assert!(
            super::super::analyze::analyze(&e).findings[0]
                .recommendation
                .contains("support a monitoring interpretation")
        );
    }
    #[test]
    fn masked_scan_or_exact_name_collisions_are_refused() {
        for name_collision in [false, true] {
            let mut e = fixture();
            e.events[0].identity.scan_id = "00000000-0000-4000-8000-000000000001".into();
            if name_collision {
                e.events[0].identity.app_name = "alpha@example.invalid".into();
            }
            let mut other = e.events[0].clone();
            other.identity.scan_id = "00000000-0000-4000-8000-000000000002".into();
            other.identity.session_id = "other-session".into();
            other.report_ids = vec!["report-2".into()];
            other.detections[0].report_id = "report-2".into();
            if name_collision {
                other.identity.app_name = "beta@example.invalid".into();
            }
            e.events.push(other);
            if !name_collision {
                e.events[0].detections[0].snippets[1] = super::super::evidence::Snippet {
                    family: "dlp".into(),
                    state: SnippetState::Present,
                    excerpts: vec!["00000000".into()],
                    variants: 1,
                    truncated: false,
                };
            }
            e.seal().unwrap();
            assert!(
                project(&mut e, &SecretSet::default()).is_err(),
                "name_collision={name_collision}"
            );
        }
    }
    #[test]
    fn untouched_placeholders_and_semantic_normalization_cannot_forge_links_or_allow() {
        let mut e = fixture();
        e.events[0].profile = Some("sensitive@example.invalid".into());
        e.profiles[0].name = WITHHELD.into();
        e.seal().unwrap();
        assert!(project(&mut e, &SecretSet::default()).is_err());
        for confidence in [false, true] {
            let mut e = fixture();
            if confidence {
                e.profiles[0].protections[0].confidence = "expli\ncit".into();
            } else {
                e.events[0].action = "all\now".into();
            }
            e.seal().unwrap();
            assert!(project(&mut e, &SecretSet::default()).is_err());
        }
        let mut value = serde_json::json!({"action":"all\now","verdict":"ma\nlicious","confidence":"expli\ncit"});
        document(&mut value, &SecretSet::default(), &PayloadSpans::default());
        assert_eq!(
            value,
            serde_json::json!({"action":"unknown","verdict":"unknown","confidence":"unknown"})
        );
    }
    #[test]
    fn conservative_ip_withholding_covers_ipv4_ipv6_and_surrounding_punctuation() {
        let secrets = SecretSet::default();
        for value in [
            "192.0.2.7",
            "fdab::abcd",
            "fe80::ffff",
            "2001:db8::1",
            "::1",
            "::ffff:192.0.2.1",
            "[fdab::abcd]",
            "(fe80::ffff),",
            "fdab::abcd.",
            "[fe80::ffff]:443",
            "https://[2001:db8::1]/path?q=example",
            "(192.0.2.7),",
            "[fdab::abcd].",
            "fdab::ab\u{1}cd",
            "192.0.\u{1}2.7",
            "synthetic\u{1}@example.invalid",
        ] {
            assert_eq!(
                text(&format!("Synthetic context {value}"), &secrets, 512),
                WITHHELD,
                "{value}"
            );
        }
        assert_eq!(
            text("Synthetic context without an address", &secrets, 512),
            "Synthetic context without an address"
        );
    }
    #[test]
    fn many_payload_spans_mask_metadata_without_cloning_registry_per_field() {
        let payload = PayloadSpans::default();
        for n in 0..2000 {
            payload.add(&format!("sensitive-value-{n:05}"));
        }
        let auth = SecretSet::default();
        for n in 0..2000 {
            assert_eq!(
                text_with_spans(&format!("opaque-id-{n}"), &auth, &payload, "scan_id", 128),
                format!("opaque-id-{n}")
            );
        }
        assert_eq!(
            text_with_spans(
                "prefix sensitive-value-01999 suffix",
                &auth,
                &payload,
                "source",
                512
            ),
            WITHHELD
        );
        payload.add("1");
        assert_eq!(
            text_with_spans(
                "00000000-0000-4000-8000-000000000001",
                &auth,
                &payload,
                "scan_id",
                512
            ),
            "00000000-0000-4000-8000-000000000001"
        );
        assert_eq!(
            text_with_spans("1", &auth, &payload, "source", 512),
            WITHHELD
        );
        assert_eq!(
            text_with_spans("allow", &auth, &payload, "action", 512),
            "allow"
        );
    }
    #[test]
    fn payload_registry_has_hard_bounds_and_refuses_unsafe_evidence() {
        let payload = PayloadSpans::default();
        payload.add(&"x".repeat(4 * 1024 * 1024 + 1));
        assert!(payload.exceeded());
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        assert!(project_with_spans(&mut e, &SecretSet::default(), &payload).is_err());
    }
}

#[cfg(test)]
mod rc2_protocol_tests {
    use super::*;
    #[test]
    fn sampling_and_fallback_protocol_labels_survive_payload_matches_without_exposing_free_text() {
        let payload = PayloadSpans::default();
        for label in ["coverage-greedy-v2", "session-transactions", "tool"] {
            payload.add(label);
        }
        let mut value = serde_json::json!({"algorithm":"coverage-greedy-v2","sources":["session-transactions"],"direction":"tool","notes":["coverage-greedy-v2 session-transactions tool"]});
        document(&mut value, &SecretSet::default(), &payload);
        assert_eq!(value["algorithm"], "coverage-greedy-v2");
        assert_eq!(
            value["sources"],
            serde_json::json!(["session-transactions"])
        );
        assert_eq!(value["direction"], "tool");
        let text = value["notes"][0].as_str().unwrap();
        for label in ["coverage-greedy-v2", "session-transactions", "tool"] {
            assert!(!text.contains(label));
        }
    }
}

#[cfg(test)]
mod detector_semantics_tests {
    use super::*;
    #[test]
    fn privacy_cannot_normalize_unknown_detector_controls_hints_or_examples_into_known_aliases() {
        for target in ["control", "hint", "report"] {
            let mut e = Evidence::parse(include_str!(
                "../../../../tests/fixtures/signal/evidence-v2.json"
            ))
            .unwrap();
            match target {
                "control" => e.profiles[0].protections[0].detector = "prompt\n-injection".into(),
                "hint" => e.events[0].detector_hints = vec!["prompt/p\ni".into()],
                "report" => e.events[0].detections[0].detector = "p\ni".into(),
                _ => unreachable!(),
            }
            e.seal().unwrap();
            assert!(project(&mut e, &SecretSet::default()).is_err(), "{target}");
        }
        let payload = PayloadSpans::default();
        for text in ["pi", "prompt-injection", "prompt/injection"] {
            payload.add(text);
        }
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence-v2.json"
        ))
        .unwrap();
        project_with_spans(&mut e, &SecretSet::default(), &payload).unwrap();
        assert_eq!(e.events[0].detections[0].detector, "pi");
        assert_eq!(e.events[0].detector_hints, ["prompt/injection"]);
        assert_eq!(e.profiles[0].protections[0].detector, "prompt-injection");
        let mut direct = serde_json::json!({"detection_service":"p\ni","data_type":"pro\nmpt"});
        document(&mut direct, &SecretSet::default(), &PayloadSpans::default());
        assert_eq!(
            direct,
            serde_json::json!({"detection_service":"unknown","data_type":"unknown"})
        );
    }
}
