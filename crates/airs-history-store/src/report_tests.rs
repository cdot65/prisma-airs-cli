//! Report input (plan M6.1): full-cohort summaries beyond the detail bound, the
//! summary-cell budget and its fallback, Chicago dates across DST, the evidence
//! cutoff (late data) and profile posture as of the period end.
use super::*;
use crate::database::Writer;
use crate::repository::seed;
use crate::test_support::TestDir;
use airs_runtime_core::signal::evidence::{Identity, Protection};
use airs_runtime_core::tenant::{Selected, TenantContext, config::TenantConfig};
use std::time::Duration;

const HOUR: i64 = 3_600_000;

fn cloud() -> Cloud {
    Cloud::from_tenant(&TenantContext {
        selected: Selected {
            name: "fixture".into(),
            tsg_id: "100".into(),
            by_flag: false,
        },
        config: TenantConfig::default(),
    })
    .unwrap()
}

/// Chicago day bounds (from the core's own DST test: 8 March 23 h, 1 November 25 h).
fn chicago_day(y: i16, m: i8, d: i8) -> (i64, i64) {
    let utc = |t: &str| airs_runtime_core::dto::utc_ms(t).unwrap();
    match (y, m, d) {
        (2026, 1, 1) => (utc("2026-01-01T06:00:00Z"), utc("2026-01-02T06:00:00Z")),
        (2026, 3, 8) => (utc("2026-03-08T06:00:00Z"), utc("2026-03-09T05:00:00Z")),
        (2026, 3, 9) => (utc("2026-03-09T05:00:00Z"), utc("2026-03-10T05:00:00Z")),
        (2026, 7, 4) => (utc("2026-07-04T05:00:00Z"), utc("2026-07-05T05:00:00Z")),
        (2026, 11, 1) => (utc("2026-11-01T05:00:00Z"), utc("2026-11-02T06:00:00Z")),
        _ => unreachable!("add the day"),
    }
}

#[derive(Clone)]
pub(crate) struct Synthetic {
    pub app: String,
    pub scan: u64,
    pub at: Option<i64>,
    pub verdict: &'static str,
    pub action: &'static str,
    pub detector: Option<String>,
}

fn event(s: &Synthetic) -> Event {
    Event {
        identity: Identity {
            app_id: s.app.clone(),
            app_name: format!("Application {}", s.app),
            session_id: format!("session-{}", s.scan / 20),
            scan_id: format!("00000000-0000-4000-8000-{:012x}", s.scan),
            sub_request: Some(0),
        },
        timestamp_ms: s.at,
        profile: Some("profile-a".into()),
        verdict: s.verdict.into(),
        action: s.action.into(),
        report_ids: vec![],
        detections: vec![],
        detector_hints: s.detector.iter().map(|d| format!("prompt/{d}")).collect(),
        sources: vec!["scan-results".into()],
        notes: vec![],
    }
}

/// An archive with the given events (first seen at `seen`), in one transaction.
pub(crate) fn archive(root: &Path, events: &[Synthetic], seen: i64) {
    let c = cloud();
    let w = Writer::open(
        root,
        &c,
        Duration::from_secs(3600),
        true,
        &scm::Session::new(),
    )
    .unwrap();
    w.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    w.conn
        .execute(
            "INSERT OR IGNORE INTO collection_runs VALUES ('seed','sync',1,1,NULL,NULL,'complete','{}')",
            [],
        )
        .unwrap();
    for s in events {
        seed::event(&w.conn, event(s), "seed", seen).unwrap();
    }
    w.conn.execute_batch("COMMIT").unwrap();
    w.check().unwrap();
}

fn with_run(root: &Path, run: &str, f: impl FnOnce(&rusqlite::Connection)) {
    let c = cloud();
    let w = Writer::open(
        root,
        &c,
        Duration::from_secs(60),
        true,
        &scm::Session::new(),
    )
    .unwrap();
    w.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    w.conn
        .execute(
            "INSERT OR IGNORE INTO collection_runs VALUES (?1,'sync',1,1,NULL,NULL,'complete','{}')",
            [run],
        )
        .unwrap();
    f(&w.conn);
    w.conn.execute_batch("COMMIT").unwrap();
    w.check().unwrap();
}

fn input(root: &Path, period: (i64, i64), cutoff: i64, budget: usize) -> ReportInput {
    report_evidence_within(
        root,
        &cloud(),
        "fixture",
        period,
        &[],
        cutoff,
        "test",
        budget,
        &scm::Session::new(),
    )
    .unwrap()
    .unwrap()
}

fn total(s: &Summary, dim: &str, value: &str) -> u64 {
    let i = s.dimensions.iter().position(|d| *d == dim).unwrap();
    s.cells
        .iter()
        .filter(|c| c.key[i] == value)
        .map(|c| c.events)
        .sum()
}

/// Plan B4/B5 item 4: a cohort above the 10,000-event detail bound whose capped detail
/// has a different verdict, application and detector distribution from the whole.
#[test]
fn full_cohort_summary_is_exact_beyond_the_detail_bound() {
    let dir = TestDir::new("report-cohort");
    let (since, until) = chicago_day(2026, 7, 4);
    let mut events: Vec<Synthetic> = (0..10_000)
        .map(|n| Synthetic {
            app: "app-early".into(),
            scan: n,
            at: Some(since + n as i64),
            verdict: "benign",
            action: "allow",
            detector: None,
        })
        .collect();
    events.extend((0..2_000).map(|n| Synthetic {
        app: "app-late".into(),
        scan: 20_000 + n,
        at: Some(since + 20 * HOUR + n as i64),
        verdict: "malicious",
        action: "block",
        detector: Some("prompt-injection".into()),
    }));
    // No observed time: never placed in the period or the summary.
    events.push(Synthetic {
        app: "app-late".into(),
        scan: 99_999,
        at: None,
        verdict: "malicious",
        action: "block",
        detector: None,
    });
    archive(dir.path(), &events, since + HOUR);
    let r = input(dir.path(), (since, until), until, MAX_SUMMARY_CELLS);
    assert_eq!(r.evidence.events.len(), MAX_EVENTS, "detail is capped");
    assert!(
        r.evidence.events.iter().all(|e| e.verdict == "benign"),
        "the capped detail holds none of the malicious events"
    );
    assert!(r.gaps.iter().any(|g| g.contains("more than 10000 events")));
    let s = &r.summary;
    assert_eq!(s.dimensions, DIMENSIONS.to_vec());
    assert!(s.disabled_filters.is_empty());
    assert_eq!(s.events, 12_000, "the whole cohort, not the detail");
    assert_eq!(total(s, "verdict", "malicious"), 2_000);
    assert_eq!(total(s, "action", "block"), 2_000);
    assert_eq!(total(s, "application", "app-early"), 10_000);
    assert_eq!(total(s, "day", "2026-07-04"), 12_000);
    let detectors = s.detector_cells.as_ref().unwrap();
    assert_eq!(
        detectors
            .iter()
            .filter(|c| c.key[4] == "prompt-injection")
            .map(|c| c.events)
            .sum::<u64>(),
        2_000
    );
}

/// The budget's fallback order, at exact boundaries: detector, then date, then
/// application; verdict and action always fit.
#[test]
fn the_summary_budget_disables_filters_in_order_at_its_exact_boundaries() {
    let dir = TestDir::new("report-budget");
    let (since, until) = chicago_day(2026, 7, 4);
    let mut events = Vec::new();
    // 3 apps × 2 local days × (malicious/block with a detector, benign/allow without).
    for app in 0..3u64 {
        for day in 0..2i64 {
            for (k, (verdict, action, detector)) in [
                ("malicious", "block", Some("dlp".to_owned())),
                ("benign", "allow", None),
            ]
            .into_iter()
            .enumerate()
            {
                events.push(Synthetic {
                    app: format!("app-{app}"),
                    scan: app * 100 + day as u64 * 10 + k as u64,
                    at: Some(since + day * 24 * HOUR + HOUR),
                    verdict,
                    action,
                    detector,
                });
            }
        }
    }
    archive(dir.path(), &events, since);
    let period = (since, until + 24 * HOUR);
    // Full plan: 12 event cells + 6 detector cells.
    let full = input(dir.path(), period, until + 24 * HOUR, 18);
    assert_eq!(full.summary.cells.len(), 12);
    assert_eq!(full.summary.detector_cells.as_ref().unwrap().len(), 6);
    assert!(full.summary.disabled_filters.is_empty());
    let over = input(dir.path(), period, until + 24 * HOUR, 17);
    assert_eq!(over.summary.disabled_filters, vec!["detector"]);
    assert_eq!(over.summary.detector_cells, None);
    assert_eq!(over.summary.cells.len(), 12);
    let date = input(dir.path(), period, until + 24 * HOUR, 11);
    assert_eq!(date.summary.disabled_filters, vec!["detector", "date"]);
    assert_eq!(date.summary.cells.len(), 6);
    let app = input(dir.path(), period, until + 24 * HOUR, 5);
    assert_eq!(
        app.summary.disabled_filters,
        vec!["detector", "date", "application"]
    );
    assert_eq!(app.summary.dimensions, vec!["verdict", "action"]);
    assert_eq!(app.summary.cells.len(), 2);
    for r in [&full, &over, &date, &app] {
        assert_eq!(r.summary.events, 12, "every plan counts the whole cohort");
    }
    assert!(
        app.gaps
            .iter()
            .any(|g| g.contains("detector, date, application") && g.contains("narrower")),
        "the fallback is stated"
    );
    // The digest binds the summary plan: a different budget is different evidence.
    assert_ne!(full.digest, over.digest);
}

/// The production budget at its exact boundary: 50,000 cells fit, 50,001 do not.
#[test]
fn the_production_cell_budget_is_exact() {
    let dir = TestDir::new("report-50k");
    let (since, until) = chicago_day(2026, 7, 4);
    // 25,000 apps with one detector-positive event each: 25,000 + 25,000 cells.
    let mut events: Vec<Synthetic> = (0..25_000u64)
        .map(|n| Synthetic {
            app: format!("app-{n:05}"),
            scan: n,
            at: Some(since + HOUR),
            verdict: "malicious",
            action: "block",
            detector: Some("dlp".into()),
        })
        .collect();
    archive(dir.path(), &events, since);
    let at = input(dir.path(), (since, until), until, MAX_SUMMARY_CELLS);
    assert_eq!(
        at.summary.cells.len() + at.summary.detector_cells.as_ref().unwrap().len(),
        MAX_SUMMARY_CELLS
    );
    assert!(at.summary.disabled_filters.is_empty());
    // One more application without a detector: 50,001 cells.
    events.clear();
    events.push(Synthetic {
        app: "app-extra".into(),
        scan: 99_000,
        at: Some(since + HOUR),
        verdict: "benign",
        action: "allow",
        detector: None,
    });
    archive(dir.path(), &events, since);
    let over = input(dir.path(), (since, until), until, MAX_SUMMARY_CELLS);
    assert_eq!(over.summary.disabled_filters, vec!["detector"]);
    assert_eq!(over.summary.cells.len(), 25_001);
    assert_eq!(over.summary.events, 25_001);
}

/// Report dates are Chicago dates: the 25-hour fall-back day is one date, and the
/// spring-forward boundary splits on local midnight.
#[test]
fn summary_dates_follow_chicago_across_dst() {
    let dir = TestDir::new("report-dst");
    let (fall_since, fall_until) = chicago_day(2026, 11, 1);
    let (spring_since, _) = chicago_day(2026, 3, 8);
    let (_, spring_until) = chicago_day(2026, 3, 9);
    let at = |scan, ms| Synthetic {
        app: "app-1".into(),
        scan,
        at: Some(ms),
        verdict: "benign",
        action: "allow",
        detector: None,
    };
    archive(
        dir.path(),
        &[
            at(1, fall_since),
            at(2, fall_since + 24 * HOUR + 30 * 60_000), // the 25th hour
            at(3, fall_until),                           // next day, outside
            at(4, spring_since + 22 * HOUR + 59 * 60_000), // last minute of 8 March (23 h day)
            at(5, spring_since + 23 * HOUR),             // 00:00 9 March
        ],
        fall_since - 400 * 24 * HOUR,
    );
    let fall = input(dir.path(), (fall_since, fall_until), fall_until, 100);
    assert_eq!(fall.summary.events, 2);
    assert_eq!(total(&fall.summary, "day", "2026-11-01"), 2);
    let spring = input(dir.path(), (spring_since, spring_until), spring_until, 100);
    assert_eq!(total(&spring.summary, "day", "2026-03-08"), 1);
    assert_eq!(total(&spring.summary, "day", "2026-03-09"), 1);
}

/// Late data: evidence first seen after a revision's cutoff is in neither the
/// summary nor the detail of that revision, and is in a later one's.
#[test]
fn late_data_enters_only_revisions_whose_cutoff_covers_it() {
    let dir = TestDir::new("report-late");
    let (since, until) = chicago_day(2026, 7, 4);
    let on_time = Synthetic {
        app: "app-1".into(),
        scan: 1,
        at: Some(since + HOUR),
        verdict: "benign",
        action: "allow",
        detector: None,
    };
    archive(dir.path(), &[on_time], until);
    let late = Synthetic {
        app: "app-1".into(),
        scan: 2,
        at: Some(since + 2 * HOUR),
        verdict: "malicious",
        action: "allow",
        detector: Some("dlp".into()),
    };
    archive(dir.path(), &[late], until + 6 * HOUR);
    let at4 = input(dir.path(), (since, until), until + 4 * HOUR, 100);
    assert_eq!(at4.summary.events, 1);
    assert_eq!(at4.evidence.events.len(), 1);
    let at8 = input(dir.path(), (since, until), until + 8 * HOUR, 100);
    assert_eq!(at8.summary.events, 2);
    assert_eq!(total(&at8.summary, "verdict", "malicious"), 1);
    assert_eq!(at8.evidence.events.len(), 2);
    assert_ne!(at4.digest, at8.digest, "late data makes a new revision");
}

fn profile(revision: u64, active: bool) -> Profile {
    Profile {
        name: "profile-a".into(),
        revision: Some(revision),
        protections: vec![Protection {
            detector: "prompt-injection".into(),
            direction: "prompt".into(),
            action: "block".into(),
            active,
            location: "model-protection".into(),
            confidence: "explicit".into(),
        }],
    }
}

/// Plan B4/B5 item 6: A, B, A reobservation selects A again by observation link, and
/// a later observation is never used for an earlier period. The timeline states each
/// change.
#[test]
fn profiles_are_taken_as_of_the_period_end_by_observation_link() {
    let dir = TestDir::new("report-asof");
    let (since, until) = chicago_day(2026, 7, 4);
    archive(dir.path(), &[], since);
    with_run(dir.path(), "r1", |c| {
        seed::profile(c, profile(1, true), "r1", since - HOUR).unwrap()
    });
    with_run(dir.path(), "r2", |c| {
        seed::profile(c, profile(2, false), "r2", since + 2 * HOUR).unwrap()
    });
    with_run(dir.path(), "r3", |c| {
        seed::profile(c, profile(1, true), "r3", since + 5 * HOUR).unwrap()
    });
    with_run(dir.path(), "r4", |c| {
        seed::profile(c, profile(3, false), "r4", until + HOUR).unwrap()
    });
    let day = input(dir.path(), (since, until), until, 100);
    assert_eq!(day.evidence.profiles.len(), 1);
    assert_eq!(
        day.evidence.profiles[0].revision,
        Some(1),
        "A seen again is current again"
    );
    let shape: Vec<(Option<u64>, bool)> = day
        .timeline
        .iter()
        .map(|t| (t.revision, t.at_start))
        .collect();
    assert_eq!(
        shape,
        vec![(Some(1), true), (Some(2), false), (Some(1), false)]
    );
    assert!(!day.timeline_truncated);
    // A period ending between B and A: B, never the later A or the future revision 3.
    let early = input(dir.path(), (since, since + 4 * HOUR), until, 100);
    assert_eq!(early.evidence.profiles[0].revision, Some(2));
    assert!(early.timeline.iter().all(|t| t.revision != Some(3)));
}

/// Opt-in 1M-scan qualification (plan M6.1, contract §6 "30/90-day qualified at ≥1M
/// scans"). Builds `AIRS_QUALIFY_SCANS` (default 1,000,000) distinct synthetic scans
/// over 90 Chicago days and checks that the 30- and 90-day report inputs are exact,
/// fit their budgets and finish within the report read budget. Run with
/// `cargo test --release -p airs-history-store qualify_ -- --ignored --nocapture`.
#[test]
#[ignore = "opt-in qualification: about a million archive writes"]
fn qualify_30_and_90_day_reports_on_a_million_scans() {
    let scans: u64 = std::env::var("AIRS_QUALIFY_SCANS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);
    let dir = TestDir::new("report-1m");
    let (start, _) = chicago_day(2026, 1, 1);
    let span = 90 * 24 * HOUR;
    let started = std::time::Instant::now();
    let batch = 1_000u64;
    let mut n = 0u64;
    while n < scans {
        let events: Vec<Synthetic> = (n..(n + batch).min(scans))
            .map(|i| Synthetic {
                app: format!("app-{:02}", i % 40),
                scan: i,
                at: Some(start + (i as i64 * span / scans as i64)),
                verdict: if i % 10 == 0 { "malicious" } else { "benign" },
                action: if i % 7 == 0 { "block" } else { "allow" },
                detector: (i % 10 == 0).then(|| {
                    ["dlp", "prompt-injection", "toxic-content"][(i % 3) as usize].to_owned()
                }),
            })
            .collect();
        archive(dir.path(), &events, start);
        n += batch;
    }
    eprintln!("seeded {scans} scans in {:?}", started.elapsed());
    for days in [30i64, 90] {
        let period = (start, start + days * 24 * HOUR);
        let t = std::time::Instant::now();
        let r = input(dir.path(), period, period.1, MAX_SUMMARY_CELLS);
        let elapsed = t.elapsed();
        let expected = (0..scans)
            .filter(|i| start + (*i as i64 * span / scans as i64) < period.1)
            .count() as u64;
        eprintln!(
            "{days}-day report input: {} events, {} + {} cells, disabled {:?}, {:?}",
            r.summary.events,
            r.summary.cells.len(),
            r.summary.detector_cells.as_ref().map_or(0, Vec::len),
            r.summary.disabled_filters,
            elapsed
        );
        assert_eq!(r.summary.events, expected, "{days}-day cohort is exact");
        assert_eq!(r.evidence.events.len(), MAX_EVENTS.min(expected as usize));
        assert!(
            r.summary.cells.len() + r.summary.detector_cells.as_ref().map_or(0, Vec::len)
                <= MAX_SUMMARY_CELLS
        );
        assert!(
            elapsed < REPORT_READ_BUDGET,
            "{days}-day input within the read budget"
        );
    }
}

fn source(name: &str, status: Status) -> Source {
    Source {
        name: name.into(),
        status,
        required: true,
        records: 1,
        pages: 1,
        unit: "records".into(),
        population: "tenant".into(),
        started_ms: 0,
        ended_ms: 0,
        notes: vec![],
    }
}

/// The golden coverage snapshot: the report input's summary, timeline, coverage and
/// gaps for a seeded archive (complete results, partial sessions). Regenerate
/// deliberately with `AIRS_UPDATE_GOLDEN=1 cargo test -p airs-history-store --lib
/// golden` and review the diff.
#[test]
fn the_report_input_matches_its_golden_coverage_snapshot() {
    const GOLDEN: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/report-input.json"
    );
    let dir = TestDir::new("report-golden");
    let (since, until) = chicago_day(2026, 11, 1);
    let mk = |scan, app: &str, at, verdict, action, detector: Option<&str>| Synthetic {
        app: app.into(),
        scan,
        at: Some(since + at * HOUR),
        verdict,
        action,
        detector: detector.map(str::to_owned),
    };
    archive(
        dir.path(),
        &[
            mk(1, "app-1", 0, "benign", "allow", None),
            mk(
                2,
                "app-1",
                24,
                "malicious",
                "allow",
                Some("prompt-injection"),
            ),
            mk(3, "app-2", 3, "malicious", "block", Some("dlp")),
            mk(4, "app-2", 4, "unknown", "unknown", None),
        ],
        since,
    );
    with_run(dir.path(), "cov", |c| {
        seed::coverage(
            c,
            source("scan-results", Status::Complete),
            (since, until),
            "cov",
            until,
        )
        .unwrap();
        seed::coverage(
            c,
            source("sessions", Status::Partial),
            (since, until),
            "cov",
            until,
        )
        .unwrap();
        seed::profile(c, profile(1, true), "cov", since - HOUR).unwrap();
    });
    let r = input(dir.path(), (since, until), until, MAX_SUMMARY_CELLS);
    let snapshot = serde_json::json!({
        "complete": r.complete,
        "gaps": r.gaps,
        "sources": r.evidence.sources,
        "summary": r.summary,
        "timeline": r.timeline,
        "detail_events": r.evidence.events.len(),
        "profiles": r.evidence.profiles,
        "digest": r.digest,
    });
    let text = serde_json::to_string_pretty(&snapshot).unwrap() + "\n";
    if std::env::var_os("AIRS_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(Path::new(GOLDEN).parent().unwrap()).unwrap();
        std::fs::write(GOLDEN, &text).unwrap();
    }
    let golden = std::fs::read_to_string(GOLDEN).expect("golden snapshot is checked in");
    assert!(
        text == golden,
        "report input differs from tests/golden/report-input.json:\n{text}"
    );
}

/// One `/proc/self` reading: peak resident memory (kB) and bytes read and written to
/// storage (`/proc/self/io`), for the M8.2 qualification record.
fn proc_sample() -> (u64, u64, u64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let hwm = status
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
        .unwrap_or(0);
    let io = std::fs::read_to_string("/proc/self/io").unwrap_or_default();
    let field = |name: &str| {
        io.lines()
            .find_map(|l| l.strip_prefix(name))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    };
    (hwm, field("read_bytes:"), field("write_bytes:"))
}

fn archive_file(root: &Path) -> std::path::PathBuf {
    for e in std::fs::read_dir(root).unwrap().flatten() {
        let p = e.path().join("archive.sqlite3");
        if p.exists() {
            return p;
        }
    }
    panic!("no archive under {root:?}");
}

/// Opt-in M8.2 qualification on local disk (plan M8.2; contract §9.4, §9.5): seeds
/// `AIRS_QUALIFY_SCANS` (default 1,000,000) scans, then
/// - **contention:** four report readers (30-day inputs) run while a writer appends
///   `AIRS_QUALIFY_WRITES` (default 50,000) more scans in 1,000-scan transactions;
///   every read and write succeeds, each read within the report read budget;
/// - **restore/volume:** a `VACUUM INTO` backup copy (the §9.4 content shape) is restored
///   into a fresh root, `PRAGMA integrity_check` is `ok`, and the event count matches;
/// - records peak RSS and storage I/O from `/proc/self`, archive and backup bytes, and
///   timings to stderr (and to `AIRS_QUALIFY_OUT` as JSON when set).
///
/// Run with `cargo test --release -p airs-history-store qualify_restore -- --ignored
/// --nocapture`. Production storage (Longhorn, encrypted class) and node loss are the
/// user-run M8.2 steps; this qualifies the software on the local filesystem only.
#[test]
#[ignore = "opt-in M8.2 qualification: about a million archive writes"]
fn qualify_restore_and_contention_at_scale() {
    let env = |k: &str, d: u64| {
        std::env::var(k)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(d)
    };
    let scans = env("AIRS_QUALIFY_SCANS", 1_000_000);
    let writes = env("AIRS_QUALIFY_WRITES", 50_000);
    let dir = TestDir::new("qualify-m8");
    let (start, _) = chicago_day(2026, 1, 1);
    let span = 90 * 24 * HOUR;
    let synth = move |i: u64, total: u64| Synthetic {
        app: format!("app-{:02}", i % 40),
        scan: i,
        at: Some(start + (i as i64 * span / total as i64)),
        verdict: if i.is_multiple_of(10) {
            "malicious"
        } else {
            "benign"
        },
        action: if i.is_multiple_of(7) {
            "block"
        } else {
            "allow"
        },
        detector: i.is_multiple_of(10).then(|| "dlp".to_owned()),
    };
    let t = std::time::Instant::now();
    let mut n = 0u64;
    while n < scans {
        let events: Vec<_> = (n..(n + 1000).min(scans))
            .map(|i| synth(i, scans))
            .collect();
        archive(dir.path(), &events, start);
        n += 1000;
    }
    let seeded = t.elapsed();
    let root = dir.path().to_path_buf();
    // Contention: readers alongside one writer.
    let t = std::time::Instant::now();
    let readers: Vec<_> = (0..4)
        .map(|r| {
            let root = root.clone();
            std::thread::spawn(move || {
                let mut slowest = Duration::ZERO;
                for k in 0..3 {
                    let day = start + ((r * 3 + k) as i64 % 60) * 24 * HOUR;
                    let t = std::time::Instant::now();
                    let input = input(
                        &root,
                        (day, day + 30 * 24 * HOUR),
                        day + 30 * 24 * HOUR,
                        MAX_SUMMARY_CELLS,
                    );
                    assert!(input.summary.events > 0);
                    slowest = slowest.max(t.elapsed());
                }
                slowest
            })
        })
        .collect();
    let writer = {
        let root = root.clone();
        std::thread::spawn(move || {
            let mut n = 0u64;
            while n < writes {
                let events: Vec<_> = (scans + n..scans + (n + 1000).min(writes))
                    .map(|i| synth(i, scans + writes))
                    .collect();
                archive(&root, &events, start);
                n += 1000;
            }
        })
    };
    let slowest_read = readers
        .into_iter()
        .map(|h| h.join().unwrap())
        .max()
        .unwrap();
    writer.join().unwrap();
    let contention = t.elapsed();
    assert!(
        slowest_read < REPORT_READ_BUDGET,
        "reads within budget under a writer: {slowest_read:?}"
    );
    // Restore/volume: a consistent copy, restored and verified.
    let live = archive_file(&root);
    let archive_bytes = std::fs::metadata(&live).unwrap().len();
    let backup = TestDir::new("qualify-m8-backup");
    let copy = backup.path().join("archive.sqlite3");
    let t = std::time::Instant::now();
    rusqlite::Connection::open_with_flags(&live, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap()
        .execute("VACUUM INTO ?1", [copy.to_str().unwrap()])
        .unwrap();
    let backed_up = t.elapsed();
    let restored_root = TestDir::new("qualify-m8-restore");
    let target = restored_root
        .path()
        .join(live.parent().unwrap().file_name().unwrap());
    std::fs::create_dir_all(&target).unwrap();
    let t = std::time::Instant::now();
    std::fs::copy(&copy, target.join("archive.sqlite3")).unwrap();
    let restored = rusqlite::Connection::open_with_flags(
        target.join("archive.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let integrity: String = restored
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    let events: i64 = restored
        .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
        .unwrap();
    let events = events as u64;
    let verified = t.elapsed();
    assert_eq!(integrity, "ok");
    assert_eq!(events, scans + writes, "every event restored");
    let backup_bytes = std::fs::metadata(&copy).unwrap().len();
    let (hwm_kb, read_bytes, write_bytes) = proc_sample();
    let record = serde_json::json!({
        "scans": scans, "contention_writes": writes,
        "seed_s": seeded.as_secs_f64(), "contention_s": contention.as_secs_f64(),
        "slowest_30d_read_under_writer_s": slowest_read.as_secs_f64(),
        "backup_vacuum_into_s": backed_up.as_secs_f64(), "restore_verify_s": verified.as_secs_f64(),
        "archive_bytes": archive_bytes, "backup_bytes": backup_bytes,
        "peak_rss_kb": hwm_kb, "io_read_bytes": read_bytes, "io_write_bytes": write_bytes,
    });
    eprintln!("M8.2 qualification: {record}");
    if let Ok(out) = std::env::var("AIRS_QUALIFY_OUT") {
        std::fs::write(out, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
    }
}

#[test]
fn review_later_facts_do_not_change_an_earlier_cutoff() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(crate::database::SCHEMA).unwrap();
    conn.execute_batch(
        "INSERT INTO collection_runs VALUES ('seed','sync',1,1,NULL,NULL,'complete','{}')",
    )
    .unwrap();
    let (since, until) = chicago_day(2026, 7, 4);
    let first = Synthetic {
        app: "app-1".into(),
        scan: 1,
        at: Some(since + HOUR),
        verdict: "benign",
        action: "allow",
        detector: None,
    };
    seed::event(&conn, event(&first), "seed", until).unwrap();
    let before = summarize(&conn, (since, until), "[]", until + 4 * HOUR, 100).unwrap();
    let later = Synthetic {
        detector: Some("dlp".into()),
        verdict: "malicious",
        ..first
    };
    seed::event(&conn, event(&later), "seed", until + 26 * HOUR).unwrap();
    let after = summarize(&conn, (since, until), "[]", until + 4 * HOUR, 100).unwrap();
    assert_eq!(
        after, before,
        "a fact first observed at +26h must not alter the +4h summary"
    );
}

/// M6 review 1 finding 1, through the real report read: a later conflicting fact for an
/// existing event changes neither the summary nor the detail of an earlier cutoff; a
/// later cutoff sees it.
#[test]
fn an_earlier_cutoff_keeps_its_summary_and_detail_when_a_later_fact_arrives() {
    let dir = TestDir::new("report-asof");
    let (since, until) = chicago_day(2026, 7, 4);
    let first = Synthetic {
        app: "app-1".into(),
        scan: 1,
        at: Some(since + HOUR),
        verdict: "benign",
        action: "allow",
        detector: None,
    };
    archive(dir.path(), std::slice::from_ref(&first), until);
    let later = Synthetic {
        detector: Some("dlp".into()),
        verdict: "malicious",
        ..first.clone()
    };
    archive(dir.path(), &[later], until + 26 * HOUR);
    let at4 = input(dir.path(), (since, until), until + 4 * HOUR, 100);
    assert_eq!(
        total(&at4.summary, "verdict", "benign"),
        1,
        "{:?}",
        at4.summary
    );
    assert_eq!(total(&at4.summary, "verdict", "malicious"), 0);
    assert_eq!(
        at4.summary.detector_cells.as_deref().map(<[_]>::len),
        Some(0)
    );
    assert_eq!(at4.evidence.events.len(), 1);
    assert_eq!(at4.evidence.events[0].verdict, "benign");
    assert!(
        at4.evidence.events[0].detections.is_empty(),
        "the later fact's detection is not in the earlier detail"
    );
    let at30 = input(dir.path(), (since, until), until + 30 * HOUR, 100);
    assert_ne!(
        at30.evidence.events[0].verdict, "benign",
        "a later cutoff sees the later fact"
    );
    assert_ne!(at4.digest, at30.digest);
}

#[test]
fn review2_a_later_receipt_demotion_does_not_change_an_earlier_summary() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(crate::database::SCHEMA).unwrap();
    conn.execute_batch(
        "INSERT INTO collection_runs VALUES ('seed','sync',1,1,NULL,NULL,'complete','{}')",
    )
    .unwrap();
    let (since, until) = chicago_day(2026, 7, 4);
    let first = Synthetic {
        app: "app-1".into(),
        scan: 1,
        at: Some(since + HOUR),
        verdict: "benign",
        action: "allow",
        detector: None,
    };
    seed::event(&conn, event(&first), "seed", until).unwrap();
    let mut receipt = event(&first);
    receipt.identity.app_name.clear();
    receipt.verdict = "malicious".into();
    receipt.detector_hints.push("prompt/dlp".into());
    seed::probe_receipt(&conn, receipt, "seed", until + HOUR);
    let state: String = conn
        .query_row("SELECT state FROM receipts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(state, "promoted");
    let before = summarize(&conn, (since, until), "[]", until + 4 * HOUR, 100).unwrap();
    let mut competing = event(&first);
    competing.identity.app_name = "Another exact name".into();
    seed::event(&conn, competing, "seed", until + 26 * HOUR).unwrap();
    let state: String = conn
        .query_row("SELECT state FROM receipts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(state, "ambiguous");
    let after = summarize(&conn, (since, until), "[]", until + 4 * HOUR, 100).unwrap();
    assert_eq!(
        after, before,
        "+26h receipt demotion must not change the +4h cohort"
    );
}

#[test]
fn review2_later_small_facts_do_not_remove_earlier_detail() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(crate::database::SCHEMA).unwrap();
    conn.execute_batch(
        "INSERT INTO collection_runs VALUES ('seed','sync',1,1,NULL,NULL,'complete','{}')",
    )
    .unwrap();
    let (since, until) = chicago_day(2026, 7, 4);
    let first = Synthetic {
        app: "app-1".into(),
        scan: 1,
        at: Some(since + HOUR),
        verdict: "benign",
        action: "allow",
        detector: None,
    };
    let early = event(&first);
    seed::event(&conn, early.clone(), "seed", until).unwrap();
    let before = crate::repository::cached_event(&conn, &early.identity)
        .unwrap()
        .unwrap();
    for i in 0..400 {
        let mut late = early.clone();
        late.notes.push(format!("{}-{i}", "x".repeat(300)));
        seed::event(&conn, late, "seed", until + 26 * HOUR).unwrap();
    }
    let (reconstructed, receipts) = late_as_of(&conn, until + 4 * HOUR).unwrap();
    let projection = &reconstructed.values().next().unwrap().projection;
    let after = crate::repository::cached_event_as_of(
        &conn,
        &early.identity,
        until + 4 * HOUR,
        projection,
        &receipts,
    )
    .unwrap();
    assert_eq!(
        after,
        Some(before),
        "later bytes must not remove earlier detail"
    );
}

/// M6 review 2 finding 1, the other direction: a receipt first received (and promoted)
/// after the cutoff changes neither the earlier summary nor that event's detail.
#[test]
fn a_receipt_promoted_only_after_the_cutoff_does_not_change_it() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(crate::database::SCHEMA).unwrap();
    conn.execute_batch(
        "INSERT INTO collection_runs VALUES ('seed','sync',1,1,NULL,NULL,'complete','{}')",
    )
    .unwrap();
    let (since, until) = chicago_day(2026, 7, 4);
    let first = Synthetic {
        app: "app-1".into(),
        scan: 1,
        at: Some(since + HOUR),
        verdict: "benign",
        action: "allow",
        detector: None,
    };
    seed::event(&conn, event(&first), "seed", until).unwrap();
    let before = summarize(&conn, (since, until), "[]", until + 4 * HOUR, 100).unwrap();
    let mut receipt = event(&first);
    receipt.identity.app_name.clear();
    receipt.verdict = "malicious".into();
    receipt.detector_hints.push("prompt/dlp".into());
    seed::probe_receipt(&conn, receipt, "seed", until + 26 * HOUR);
    let state: String = conn
        .query_row("SELECT state FROM receipts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(state, "promoted", "the receipt is promoted now");
    let after = summarize(&conn, (since, until), "[]", until + 4 * HOUR, 100).unwrap();
    assert_eq!(
        after, before,
        "a later promotion does not reach the earlier cutoff"
    );
    let (late, receipts) = late_as_of(&conn, until + 4 * HOUR).unwrap();
    let l = late
        .values()
        .next()
        .expect("the event is rebuilt as of the cutoff");
    assert_eq!(l.projection.verdict, "benign");
    let detail = crate::repository::cached_event_as_of(
        &conn,
        &event(&first).identity,
        until + 4 * HOUR,
        &l.projection,
        &receipts,
    )
    .unwrap()
    .unwrap();
    assert_eq!(detail.verdict, "benign");
}
