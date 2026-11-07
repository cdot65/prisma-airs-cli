//! Explicit equivalences across scanner flags, report services and profile controls.
//! Unknown names are retained verbatim; no substring or case-folded guesses.
pub fn canonical(name: &str) -> &str {
    match name {
        "pi" | "injection" | "prompt_injection" | "prompt-injection" => "prompt-injection",
        "tc" | "toxic_content" | "toxic-content" => "toxic-content",
        "uf" | "url_cats" | "url_filtering" | "url-filtering" => "url-filtering",
        "dbs" | "db_security" | "database-security" => "database-security",
        "mc" | "malicious_code" | "malicious-code" => "malicious-code",
        "source_code" | "source-code" => "source-code",
        "topic_guardrails" | "topic_violation" | "topic-violation" => "topic-violation",
        _ => name,
    }
}

/// Known public protocol labels may survive payload-span withholding unchanged.
/// This predicate never accepts a normalized/control-stripped spelling.
pub fn known(name: &str) -> bool {
    matches!(
        canonical(name),
        "prompt-injection"
            | "toxic-content"
            | "url-filtering"
            | "database-security"
            | "malicious-code"
            | "source-code"
            | "topic-violation"
            | "dlp"
            | "agent"
            | "ungrounded"
    )
}

/// Compare observations without mutating their raw provenance. A known positive
/// direction subsumes the same detector's unknown-direction hint for counting.
pub fn observation_hints(hints: &[String]) -> std::collections::BTreeSet<(&str, &str)> {
    let mut observations: std::collections::BTreeSet<_> = hints
        .iter()
        .filter_map(|hint| {
            let (direction, detector) = hint.split_once('/')?;
            (["prompt", "response", "tool", "unknown"].contains(&direction) && !detector.is_empty())
                .then(|| (direction, canonical(detector)))
        })
        .collect();
    let directional: std::collections::BTreeSet<_> = observations
        .iter()
        .filter(|(direction, _)| *direction != "unknown")
        .map(|(_, detector)| *detector)
        .collect();
    observations
        .retain(|(direction, detector)| *direction != "unknown" || !directional.contains(detector));
    observations
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comparison_subsumes_unknown_direction_and_preserves_raw_hints() {
        let raw = vec![
            "unknown/pi".into(),
            "prompt/injection".into(),
            "response/prompt_injection".into(),
            "unknown/dlp".into(),
        ];
        let compared = observation_hints(&raw);
        assert_eq!(compared.len(), 3);
        assert!(compared.contains(&("prompt", "prompt-injection")));
        assert!(compared.contains(&("response", "prompt-injection")));
        assert!(compared.contains(&("unknown", "dlp")));
        assert!(!compared.contains(&("unknown", "prompt-injection")));
        assert_eq!(raw[0], "unknown/pi");
        assert_eq!(
            observation_hints(&["unknown/pi".into()]),
            [("unknown", "prompt-injection")].into_iter().collect()
        );
    }
    #[test]
    fn known_equivalences_are_explicit_and_unknown_controls_are_not_guessed() {
        for (a, b) in [
            ("pi", "prompt-injection"),
            ("tc", "toxic-content"),
            ("uf", "url-filtering"),
            ("dbs", "database-security"),
            ("mc", "malicious-code"),
            ("topic_guardrails", "topic-violation"),
            ("url_filtering", "url-filtering"),
        ] {
            assert_eq!(canonical(a), canonical(b));
        }
        assert_eq!(canonical("sql-injection"), "sql-injection");
        assert_eq!(canonical("PI"), "PI");
        assert_eq!(canonical("future/pi"), "future/pi");
    }
}
