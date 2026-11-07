//! Entire review round trip uses only synthetic files and no tenant/API clients.
use airs_test_harness::{Cli, RunOptions, Sandbox, run};
use serde_json::{Value, json};
const START: &str = "<!-- airs-assessments:start -->";
const END: &str = "<!-- airs-assessments:end -->";
fn cli() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}
fn setup() -> (Sandbox, std::path::PathBuf, std::path::PathBuf, String) {
    let s = Sandbox::new();
    let input = s.tmp().join("evidence.json");
    std::fs::write(&input, include_str!("fixtures/signal/evidence.json")).unwrap();
    let bundle = s.tmp().join("bundle");
    run(
        &cli(),
        &s,
        &[
            "runtime",
            "report",
            "--from-evidence",
            input.to_str().unwrap(),
            "--bundle-dir",
            bundle.to_str().unwrap(),
        ],
        &RunOptions::new(),
    )
    .assert_success();
    let review = bundle.join("review.md");
    let packet = std::fs::read_to_string(&review).unwrap();
    (s, input, review, packet)
}
fn annotate(packet: &str, kind: &str) -> String {
    let rest = packet.strip_prefix("---\n").unwrap();
    let (front, body) = rest.split_once("\n---\n").unwrap();
    let mut meta: Value = serde_json::from_str(front).unwrap();
    meta["reviewer"] = json!({"kind":kind,"name":"Synthetic reviewer","model":if kind=="agent"{Some("Synthetic model")}else{None},"reviewed_at":"2026-11-02T23:00:00Z"});
    let finding = body
        .split("\"finding_id\": \"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let assessment = json!([{"finding_id":finding,"classification":"possible-test","priority":"low","rationale":"Synthetic assessment; insufficient context for a confirmed attack."}]);
    let (before, after) = body.split_once(START).unwrap();
    let (_, after) = after.split_once(END).unwrap();
    format!(
        "---\n{}\n---\n{before}{START}\n```json\n{}\n```\n{END}{after}",
        serde_json::to_string_pretty(&meta).unwrap(),
        serde_json::to_string_pretty(&assessment).unwrap()
    )
}
fn render(
    s: &Sandbox,
    input: &std::path::Path,
    review: &std::path::Path,
) -> airs_test_harness::Run {
    run(
        &cli(),
        s,
        &[
            "--tenant",
            "absent",
            "runtime",
            "report",
            "--from-evidence",
            input.to_str().unwrap(),
            "--review-file",
            review.to_str().unwrap(),
            "--output-file",
            "-",
        ],
        &RunOptions::new(),
    )
}
#[test]
fn agent_and_human_review_round_trip_is_offline_and_preserves_evidence() {
    let (s, input, review, packet) = setup();
    let original = std::fs::read(&input).unwrap();
    for (kind, label, absent) in [
        ("agent", "Agent-assessed", "Human-reviewed"),
        ("human", "Human-reviewed", "Agent-assessed"),
    ] {
        let annotation = annotate(&packet, kind);
        std::fs::write(&review, &annotation).unwrap();
        let out = render(&s, &input, &review);
        out.assert_success();
        assert!(out.stdout.contains(label) && !out.stdout.contains(absent));
        assert!(
            out.stdout.contains("possible-test") && out.stdout.contains("Proposed priority: low")
        );
        assert!(
            out.stdout.contains("self-attributed")
                && out
                    .stdout
                    .contains("Scan verdict/action: malicious / allow")
        );
        assert!(!out.stderr.contains("Collecting"));
        assert_eq!(std::fs::read(&input).unwrap(), original);
        assert_eq!(std::fs::read_to_string(&review).unwrap(), annotation);
    }
    let rebundle = s.tmp().join("reviewed-bundle");
    run(
        &cli(),
        &s,
        &[
            "runtime",
            "report",
            "--from-evidence",
            input.to_str().unwrap(),
            "--review-file",
            review.to_str().unwrap(),
            "--bundle-dir",
            rebundle.to_str().unwrap(),
        ],
        &RunOptions::new(),
    )
    .assert_success();
    assert_eq!(
        serde_json::from_slice::<Value>(&original).unwrap(),
        serde_json::from_slice::<Value>(&std::fs::read(rebundle.join("evidence.json")).unwrap())
            .unwrap()
    );
    render(&s, &input, &rebundle.join("review.md")).assert_success();
}
#[test]
fn stale_unknown_duplicate_or_invalid_assessments_fail_before_publication() {
    let (s, input, review, packet) = setup();
    let valid = annotate(&packet, "agent");
    let cases = [
        valid.replacen("\"packet_version\": 1", "\"packet_version\": 2", 1),
        valid.replacen(
            "\"packet_version\": 1",
            "\"packet_version\": 1, \"packet_version\": 1",
            1,
        ),
        valid.replace(
            "\"classification\": \"possible-test\"",
            "\"classification\": \"confirmed-attack\"",
        ),
        valid.replace(
            "\"priority\": \"low\"",
            "\"priority\": \"low\", \"action\": \"block\"",
        ),
        valid.replace("\"model\": \"Synthetic model\"", "\"model\": null"),
        valid.replace("2026-11-02T23:00:00Z", "not-a-date"),
        valid.replace(START, &format!("{START}\n{START}")),
        valid.replace("\"kind\": \"agent\"", "\"kind\": \"system\""),
        valid.replace("\"finding_id\": \"", "\"finding_id\": \"unknown-"),
    ];
    for (i, invalid) in cases.iter().enumerate() {
        std::fs::write(&review, invalid).unwrap();
        let out = render(&s, &input, &review);
        assert_eq!(out.code, 1, "case {i}: {}", out.stderr);
        assert!(out.stdout.is_empty());
    }
    let rest = valid.strip_prefix("---\n").unwrap();
    let (front, body) = rest.split_once("\n---\n").unwrap();
    let meta: Value = serde_json::from_str(front).unwrap();
    for key in ["collection_id", "evidence_digest"] {
        let mut bad = meta.clone();
        bad[key] = json!("wrong-collection");
        std::fs::write(&review, format!("---\n{}\n---\n{body}", bad)).unwrap();
        render(&s, &input, &review).assert_code(1);
    }
    let out = run(
        &cli(),
        &s,
        &[
            "runtime",
            "report",
            "--review-file",
            review.to_str().unwrap(),
            "--output-file",
            "-",
        ],
        &RunOptions::new(),
    );
    out.assert_code(2);
}
#[test]
fn annotations_are_masked_escaped_and_never_executed() {
    let (s, input, review, packet) = setup();
    let marker = s.tmp().join("NEVER-CREATE");
    let content=annotate(&packet,"agent").replace("Synthetic assessment; insufficient context for a confirmed attack.",&format!("</pre><script>alert(1)</script> $({}) Authorization: Bearer ANNOTATION-CANARY-0123456789",marker.display()));
    std::fs::write(&review, content).unwrap();
    let out = render(&s, &input, &review);
    out.assert_success();
    assert!(!out.stdout.contains("ANNOTATION-CANARY") && !out.stderr.contains("ANNOTATION-CANARY"));
    assert!(!out.stdout.contains("<script"));
    assert!(!marker.exists());
    let html = annotate(&packet, "agent").replace(
        "Synthetic assessment; insufficient context for a confirmed attack.",
        "</pre><script>synthetic</script> ```json",
    );
    std::fs::write(&review, html).unwrap();
    let out = render(&s, &input, &review);
    out.assert_success();
    assert!(out.stdout.contains("&lt;script&gt;") && !out.stdout.contains("<script"));
}

#[test]
fn reviewer_timestamp_cannot_repeat_a_dlp_match() {
    use sha2::{Digest, Sha256};
    let (s, input, review, packet) = setup();
    let timestamp = "2026-11-02T23:00:00Z";
    let mut e: Value = serde_json::from_slice(&std::fs::read(&input).unwrap()).unwrap();
    e["events"][0]["detections"][0]["snippets"][1] = json!({"family":"dlp","state":"present","excerpts":[timestamp],"variants":1,"truncated":false});
    e.as_object_mut().unwrap().remove("digest");
    e["digest"] = json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&e).unwrap())
    ));
    std::fs::write(&input, serde_json::to_vec(&e).unwrap()).unwrap();
    let annotated = annotate(&packet, "agent");
    let (front, body) = annotated
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---\n")
        .unwrap();
    let mut metadata: Value = serde_json::from_str(front).unwrap();
    metadata["evidence_digest"] = e["digest"].clone();
    std::fs::write(&review, format!("---\n{}\n---\n{body}", metadata)).unwrap();
    let out = render(&s, &input, &review);
    out.assert_success();
    assert!(!out.stdout.contains(timestamp));
    assert!(out.stdout.contains("Time: [sensitive value withheld]"));
    // Sanitized timestamps remain a valid data-only review representation.
    let safe = annotate(&packet, "agent").replace(timestamp, "[sensitive value withheld]");
    std::fs::write(&input, include_str!("fixtures/signal/evidence.json")).unwrap();
    std::fs::write(&review, safe).unwrap();
    render(&s, &input, &review).assert_success();
}
