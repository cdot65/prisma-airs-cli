//! Reproducible, privacy-projected Signal Brief evidence.
pub mod analyze;
pub mod bundle;
pub mod collect;
pub mod detectors;
pub mod evidence;
pub mod html;
pub mod privacy;
pub mod review;
pub mod sink;

use crate::error::{CliError, SecretSet};

pub fn invalid() -> CliError {
    CliError::operational(
        "Signal Brief evidence is invalid, unsupported, or exceeds its safety limits",
    )
}

#[cfg(test)]
use html::render_with_spans as render;

pub struct Loaded {
    pub original: evidence::Evidence,
    pub evidence: evidence::Evidence,
    pub changed: bool,
}
pub fn load(
    path: &std::path::Path,
    secrets: &SecretSet,
    payload: &privacy::PayloadSpans,
) -> Result<Loaded, CliError> {
    let text = crate::files::read_text(path.as_os_str(), 32 * 1024 * 1024)?;
    let original = evidence::Evidence::parse(&text)?;
    let mut evidence = original.clone();
    let changed = privacy::project_with_spans(&mut evidence, secrets, payload)?;
    Ok(Loaded {
        original,
        evidence,
        changed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use evidence::*;
    fn example() -> Evidence {
        Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap()
    }
    #[test]
    fn identities_presence_and_counting_are_preserved() {
        let mut e = example();
        let mut other = e.events[0].clone();
        other.identity.app_name = "Another bucket".into();
        other.identity.sub_request = None;
        e.events.push(other);
        e.seal().unwrap();
        assert_eq!(
            Evidence::parse(&serde_json::to_string_pretty(&e).unwrap()).unwrap(),
            e
        );
        assert_ne!(e.events[0].identity, e.events[1].identity);
        e.events[1].identity = e.events[0].identity.clone();
        assert!(e.seal().is_err());
    }
    #[test]
    fn duplicate_keys_tampering_and_dangling_sources_are_refused() {
        let mut e = example();
        e.tenant = "tampered".into();
        assert!(e.validate().is_err());
        e.events[0].sources = vec!["missing".into()];
        assert!(e.seal().is_err());
        let text = include_str!("../../../../tests/fixtures/signal/evidence.json");
        assert!(
            Evidence::parse(&text.replacen(
                "\"schema_version\": 1",
                "\"schema_version\": 1, \"schema_version\": 1",
                1
            ))
            .is_err()
        );
        let mut e = example();
        e.rules_version = 3;
        assert!(e.seal().is_err());
    }
    #[test]
    fn privacy_removes_dlp_values_everywhere_and_escapes_html() {
        let mut e = example();
        let secrets = SecretSet::default();
        secrets.add("FAKE-OAUTH-TOKEN");
        let d = &mut e.events[0].detections[0];
        d.snippets[1] = Snippet {
            family: "dlp".into(),
            state: SnippetState::Present,
            excerpts: vec!["synthetic-dlp-value".into()],
            variants: 1,
            truncated: false,
        };
        d.snippets[0].excerpts =
            vec!["synthetic-dlp-value <script>alert(1)</script> FAKE-OAUTH-TOKEN".into()];
        assert!(privacy::project(&mut e, &secrets).unwrap());
        let html = render(
            &e,
            "<script>title</script>",
            &secrets,
            &privacy::PayloadSpans::default(),
        )
        .unwrap();
        for value in ["synthetic-dlp-value", "FAKE-OAUTH-TOKEN", "<script>"] {
            assert!(!html.contains(value));
        }
        assert_eq!(
            e.events[0].detections[0].snippets[1].state,
            SnippetState::Withheld
        );
        assert!(html.contains("&lt;script&gt;title"));
    }
    #[test]
    fn inconsistent_report_ownership_and_sample_ledgers_are_refused() {
        let mut e = example();
        e.sampling.selected_report_ids.push("phantom".into());
        assert!(e.seal().is_err());
        let mut e = example();
        e.sampling.selected_report_ids.push("report-1".into());
        assert!(e.seal().is_err());
        let mut e = example();
        let duplicate = e.events[0].detections[0].snippets[0].clone();
        e.events[0].detections[0].snippets = vec![duplicate.clone(), duplicate];
        assert!(e.seal().is_err());
        let mut e = example();
        let mut other = e.events[0].clone();
        other.identity.scan_id = "other-scan".into();
        e.events.push(other);
        assert!(e.seal().is_err());
    }
    #[test]
    fn source_window_and_clock_must_be_consistent() {
        let mut e = example();
        e.window.end_ms += 1;
        assert!(e.seal().is_err());
        let mut e = example();
        e.collected_ms = e.window.end_ms - 1;
        assert!(e.seal().is_err());
    }
    #[test]
    fn snippet_absent_empty_and_unknown_detector_survive_safe_projection() {
        let mut e = example();
        e.events[0].detections[0].detector = "future-detector".into();
        privacy::project(&mut e, &SecretSet::default()).unwrap();
        assert_eq!(
            e.events[0].detections[0].snippets[1].state,
            SnippetState::Absent
        );
        assert_eq!(
            e.events[0].detections[0].snippets[2].state,
            SnippetState::Empty
        );
        assert_eq!(e.events[0].detections[0].detector, "future-detector");
    }
}
