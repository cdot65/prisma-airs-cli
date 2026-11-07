//! The historical report page: golden snapshot, determinism, CSP hashes, injection
//! fixtures, detail/byte thresholds and fallbacks, disabled filters, the no-script and
//! print views, and the filter script's arithmetic (run under Node when installed).
use super::*;
use airs_history_store::report::{Cell, TimelineEntry};
use airs_runtime_core::signal::evidence::Evidence;

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/report.html");
const DAY: (i64, i64) = (1_783_141_200_000, 1_783_227_600_000); // 2026-07-04 Chicago

fn evidence() -> Evidence {
    Evidence::parse(include_str!("../../../tests/fixtures/signal/evidence.json")).unwrap()
}

fn cell(key: &[&str], events: u64) -> Cell {
    Cell {
        key: key.iter().map(|k| (*k).to_owned()).collect(),
        events,
    }
}

/// A full-cohort summary larger than (and distributed differently from) the detail.
fn summary() -> Summary {
    Summary {
        dimensions: vec!["day", "application", "verdict", "action"],
        cells: vec![
            cell(&["2026-07-04", "app-1", "benign", "allow"], 9_000),
            cell(&["2026-07-04", "app-1", "malicious", "allow"], 40),
            cell(&["2026-07-04", "app-2", "malicious", "block"], 1_200),
            cell(&["2026-07-04", "app-2", "unknown", "unknown"], 7),
        ],
        detector_cells: Some(vec![
            cell(
                &[
                    "2026-07-04",
                    "app-1",
                    "malicious",
                    "allow",
                    "prompt-injection",
                ],
                40,
            ),
            cell(&["2026-07-04", "app-2", "malicious", "block", "dlp"], 1_000),
            cell(
                &[
                    "2026-07-04",
                    "app-2",
                    "malicious",
                    "block",
                    "prompt-injection",
                ],
                300,
            ),
        ]),
        disabled_filters: vec![],
        events: 10_247,
        cell_budget: 50_000,
    }
}

fn input() -> ReportInput {
    ReportInput {
        evidence: evidence(),
        digest: "0".repeat(64),
        gaps: vec!["results coverage is partial for app-2".into()],
        complete: false,
        summary: summary(),
        timeline: vec![
            TimelineEntry {
                profile: "Default".into(),
                revision: Some(1),
                first_observed_ms: DAY.0 - 3_600_000,
                last_observed_ms: DAY.0 + 3_600_000,
                observation: "a1b2c3d4e5f6".into(),
                at_start: true,
            },
            TimelineEntry {
                profile: "Default".into(),
                revision: Some(2),
                first_observed_ms: DAY.0 + 7_200_000,
                last_observed_ms: DAY.0 + 7_200_000,
                observation: "0f1e2d3c4b5a".into(),
                at_start: false,
            },
        ],
        timeline_truncated: false,
    }
}

fn page() -> Page<'static> {
    Page {
        title: "Daily report",
        kind: "daily",
        version: 0,
        period: DAY,
        cutoff_ms: DAY.1 + 4 * 3_600_000,
    }
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find(open) {
        let tail = &rest[i + open.len()..];
        let j = tail.find(close).unwrap();
        out.push(&tail[..j]);
        rest = &tail[j..];
    }
    out
}

/// The golden page. Regenerate deliberately with
/// `AIRS_UPDATE_GOLDEN=1 cargo test -p airs-mcp --lib report_html` and review the diff.
#[test]
fn the_report_page_matches_its_golden_snapshot() {
    let html = render(&page(), &input(), Limits::default()).unwrap();
    if std::env::var_os("AIRS_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(std::path::Path::new(GOLDEN).parent().unwrap()).unwrap();
        std::fs::write(GOLDEN, &html).unwrap();
    }
    let golden = std::fs::read_to_string(GOLDEN).expect("golden snapshot is checked in");
    assert!(
        html == golden,
        "report.html differs from tests/golden/report.html"
    );
    assert_eq!(
        html,
        render(&page(), &input(), Limits::default()).unwrap(),
        "rendering is deterministic"
    );
}

#[test]
fn the_policy_admits_exactly_the_page_style_and_script() {
    let html = render(&page(), &input(), Limits::default()).unwrap();
    let policy = policy();
    assert!(html.contains(&h(&policy)));
    let styles = between(&html, "<style>", "</style>");
    assert_eq!(styles, vec![CSS]);
    let scripts = between(&html, "<script>", "</script>");
    assert_eq!(
        scripts,
        vec![SCRIPT],
        "one executable script, the hashed one"
    );
    for (body, directive) in [(CSS, "style-src"), (SCRIPT, "script-src")] {
        let hash = base64(&Sha256::digest(body.as_bytes()));
        assert!(policy.contains(&format!("{directive} 'sha256-{hash}'")));
    }
    for forbidden in [
        " style=",
        " onclick",
        " onerror",
        " onload",
        "javascript:",
        "innerHTML",
        "eval(",
        "http://",
        "https://",
        "<link",
        "<img",
        "<iframe",
    ] {
        assert!(!html.contains(forbidden), "{forbidden}");
    }
    assert!(policy.starts_with("default-src 'none'"));
    assert!(policy.contains("require-trusted-types-for 'script'"));
}

#[test]
fn injected_evidence_text_cannot_escape_any_context() {
    let attack = "</script><script>alert(1)</script>\"><img src=x onerror=alert(2)>\u{2028}";
    let mut i = input();
    i.evidence.tenant = attack.into();
    i.evidence.events[0].identity.app_id = attack.into();
    i.evidence.events[0].profile = Some(attack.into());
    i.evidence.events[0].detector_hints = vec![format!("prompt/{attack}")];
    i.evidence.profiles[0].name = attack.into();
    i.evidence.seal().unwrap();
    i.summary.cells[0].key[1] = attack.into();
    i.summary.detector_cells.as_mut().unwrap()[0].key[4] = attack.into();
    i.timeline[0].profile = attack.into();
    i.gaps.push(attack.into());
    let mut p = page();
    p.title = attack;
    let html = render(&p, &i, Limits::default()).unwrap();
    assert!(!html.contains("<script>alert"));
    assert!(!html.contains("<img"));
    assert_eq!(
        html.matches("</script>").count(),
        2,
        "data block and the script only"
    );
    let data = between(
        &html,
        "<script type=\"application/json\" id=\"report-data\">",
        "</script>",
    );
    assert_eq!(data.len(), 1);
    assert!(!data[0].contains('<') && !data[0].contains('\u{2028}'));
    let parsed: serde_json::Value = serde_json::from_str(data[0]).unwrap();
    assert_eq!(
        parsed["cells"][0][1], attack,
        "data is escaped, not altered"
    );
}

fn many(n: usize) -> ReportInput {
    let mut i = input();
    let base = i.evidence.events[0].clone();
    i.evidence.events = (0..n)
        .map(|k| {
            let mut e = base.clone();
            e.identity.scan_id = format!("00000000-0000-4000-8000-{k:012x}");
            e
        })
        .collect();
    i
}

#[test]
fn detail_is_capped_at_ten_thousand_rows() {
    for (n, rows) in [
        (MAX_DETAIL_ROWS, MAX_DETAIL_ROWS),
        (MAX_DETAIL_ROWS + 1, MAX_DETAIL_ROWS),
    ] {
        let html = render(&page(), &many(n), Limits::default()).unwrap();
        let body = between(&html, "<tbody id=\"detail-rows\">", "</tbody>")[0];
        assert_eq!(body.matches("<tr ").count(), rows, "{n} events");
    }
}

#[test]
fn the_byte_ceiling_drops_the_detail_then_refuses_at_its_exact_boundary() {
    let i = many(50);
    let full = render(&page(), &i, Limits::default()).unwrap();
    let exact = Limits {
        detail_rows: MAX_DETAIL_ROWS,
        html_bytes: full.len(),
    };
    assert_eq!(
        render(&page(), &i, exact).unwrap(),
        full,
        "exactly at the ceiling"
    );
    let under = Limits {
        html_bytes: full.len() - 1,
        ..exact
    };
    let reduced = render(&page(), &i, under).unwrap();
    assert!(reduced.len() < full.len());
    assert!(reduced.contains("The detail table is omitted"));
    assert!(!reduced.contains("id=\"detail-rows\""));
    assert!(
        reduced.contains("id=\"base-summary\""),
        "the summary is kept"
    );
    let tiny = Limits {
        html_bytes: reduced.len() - 1,
        ..exact
    };
    assert_eq!(render(&page(), &i, tiny), Err(Refusal::TooLarge));
    assert_eq!(MAX_HTML_BYTES, 32 * 1024 * 1024);
}

#[test]
fn a_disabled_filter_is_absent_and_stated() {
    let mut i = input();
    i.summary.detector_cells = None;
    i.summary.disabled_filters = vec!["detector"];
    let html = render(&page(), &i, Limits::default()).unwrap();
    assert!(!html.contains("data-dim=\"detector\""));
    assert!(html.contains("The detector filter(s) are disabled"));
    assert!(html.contains("\"det\":null"));
    let full = render(&page(), &input(), Limits::default()).unwrap();
    assert!(full.contains("data-dim=\"detector\""));
}

#[test]
fn no_script_and_print_views_keep_the_base_scope() {
    let html = render(&page(), &input(), Limits::default()).unwrap();
    assert!(
        html.contains("<form id=\"filters\" hidden"),
        "controls appear only with script"
    );
    assert!(html.contains("<div id=\"live-summary\" hidden></div>"));
    assert!(html.contains("<noscript>"));
    // The base summary carries full-cohort values, not the detail's.
    let base = between(&html, "<div id=\"base-summary\">", "<script type")[0];
    assert!(base.contains("<strong>10247</strong><span>Events</span>"));
    assert!(base.contains("<strong>1240</strong><span>Malicious verdicts</span>"));
    assert!(base.contains("<td>dlp</td><td class=\"n\">1000</td>"));
    for rule in [
        "#live-summary{display:none!important}",
        "#base-summary{display:block!important}",
        "#detail-rows tr{display:table-row!important}",
        "form,",
    ] {
        assert!(
            CSS.contains(rule) || CSS.contains(&rule.replace("form,", "form,#filter")),
            "{rule}"
        );
    }
    assert!(
        html.contains("<label>Report date<select data-dim=\"day\""),
        "labelled controls"
    );
    assert!(html.contains("role=\"status\" aria-live=\"polite\""));
}

/// The script's arithmetic matches the server's totals with no filter and computes
/// filtered full-cohort counts from the summary (never the detail), with detector
/// counts from detector-positive cells. Needs Node; skipped (and said) without it.
#[test]
fn the_filter_script_counts_the_full_cohort() {
    if std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("node is not installed: the filter-script arithmetic test is skipped");
        return;
    }
    let html = render(&page(), &input(), Limits::default()).unwrap();
    let data = between(
        &html,
        "<script type=\"application/json\" id=\"report-data\">",
        "</script>",
    )[0];
    let t = totals(&summary());
    let program = format!(
        "const vm=require('vm');const c={{}};vm.createContext(c);vm.runInContext({script},c);\
         const D=JSON.parse({data});const r=c.airsReport;\
         const all=r.tally(D,{{}});\
         const app2=r.tally(D,{{application:'app-2'}});\
         const pi=r.tally(D,{{detector:'prompt-injection'}});\
         const piBlock=r.tally(D,{{detector:'prompt-injection',action:'block'}});\
         const attr=n=>({{day:'2026-07-04',application:'app-1',verdict:'malicious',action:'allow',detector:'dlp\\nprompt-injection'}})[n]??null;\
         console.log(JSON.stringify([all.total,all.by.verdict.malicious,all.by.action.block,all.detectors,\
           app2.total,app2.by.verdict.malicious,pi.total,pi.by.application,piBlock.total,\
           r.rowMatches(attr,{{detector:'prompt-injection'}}),r.rowMatches(attr,{{detector:'dlp',application:'app-2'}}),r.rowMatches(attr,{{}})]));",
        script = serde_json::to_string(SCRIPT).unwrap(),
        data = serde_json::to_string(data).unwrap(),
    );
    let out = std::process::Command::new("node")
        .arg("-e")
        .arg(program)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let got: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(got[0], t.total);
    assert_eq!(got[1], t.by["verdict"]["malicious"]);
    assert_eq!(got[2], t.by["action"]["block"]);
    assert_eq!(
        got[3],
        serde_json::to_value(t.detectors.as_ref().unwrap()).unwrap()
    );
    assert_eq!(got[4], 1_207);
    assert_eq!(got[5], 1_200);
    assert_eq!(got[6], 340, "detector-positive events, not all events");
    assert_eq!(got[7], serde_json::json!({"app-1": 40, "app-2": 300}));
    assert_eq!(got[8], 300);
    assert_eq!(got[9], true);
    assert_eq!(got[10], false);
    assert_eq!(got[11], true);
}

#[test]
fn review_agent_summary_preserves_full_cohort_counts() {
    let input = input();
    let text = crate::reports::summary(&input);
    assert!(
        text.starts_with("10247 events: 1240 malicious"),
        "the summary read by agents must carry the same cohort counts as the HTML: {text}"
    );
}

/// M6 review 1 minor: application IDs such as `__proto__` and `constructor` are ordinary
/// counter keys (null-prototype objects), never inherited properties. Needs Node.
#[test]
fn the_filter_script_counts_prototype_named_applications() {
    if std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("node is not installed: the prototype-key test is skipped");
        return;
    }
    let data = serde_json::json!({
        "dims": ["application", "verdict"],
        "cells": [["__proto__", "benign", 2], ["constructor", "benign", 3], ["toString", "malicious", 1]],
        "det": [["__proto__", "benign", "constructor", 2]],
    });
    let program = format!(
        "const vm=require('vm');const c={{}};vm.createContext(c);vm.runInContext({script},c);\
         const t=c.airsReport.tally({data},{{}});\
         console.log(JSON.stringify([t.total,t.by.application['__proto__'],t.by.application['constructor'],\
           t.by.application['toString'],Object.keys(t.by.application).sort(),t.detectors['constructor']]));",
        script = serde_json::to_string(SCRIPT).unwrap(),
        data = data,
    );
    let out = std::process::Command::new("node")
        .arg("-e")
        .arg(program)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let got: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        got,
        serde_json::json!([6, 2, 3, 1, ["__proto__", "constructor", "toString"], 2])
    );
}
