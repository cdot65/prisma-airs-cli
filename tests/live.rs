//! The live comparison tier (DESIGN §10, tier 3) and its loopback self-tests.
//!
//! `live_tenant_comparison` runs only with `AIRS_RS_LIVE=1 AIRS_E2E_TENANT=<name>`
//! (optionally `AIRS_E2E_PROFILE=<profile>` for scans and `AIRS_RS_LIVE_MAX_SCANS=<n>`,
//! default 8, at most 20); otherwise it prints one `LIVE SKIPPED` line to the real stderr
//! and passes. Run it with
//!
//! ```sh
//! AIRS_RS_LIVE=1 AIRS_E2E_TENANT=<name> [AIRS_E2E_PROFILE=<profile>] \
//!   cargo test --test live -- --nocapture --test-threads=1
//! ```
//!
//! It resolves the real registry the way both CLIs do, copies only the selected entry
//! into an isolated registry, imports that entry with `airs tenant import` into a
//! private 0700 store under `target/live/` (removed when the flow ends, also on failure
//! or panic; leftovers older than an hour are swept first), runs both CLIs with a
//! cleared environment, compares
//! sanitized projections, prints the summary to stderr, writes it to
//! `target/live/summary.json`, and fails on any mismatch after every check ran. A
//! changed tenant file, a leaked secret or an exhausted scan budget stops it at once.
//!
//! The other tests run the same flow (`airs_test_harness::live::run_live`) against the
//! loopback fixture server with a sandbox registry; they need no opt-in. Several plan a
//! divergent answer for one CLI (the Rust run of each check comes first, so a one-shot
//! response reaches it) to prove that each comparison rule reports a mismatch.

use std::io::Write;

use airs_test_harness::live::bulk::{
    BULK_PROFILE, BULK_PROMPTS, BulkGate, DEFAULT_BULK_PROMPTS, MAX_BULK_PROMPTS, MIN_BULK_PROMPTS,
    TS_SKIPPED, Trigger, run_live_bulk, run_live_bulk_with,
};
use airs_test_harness::live::cleanup::{
    MAX_DELETES, cleanup_gate, run_live_cleanup, run_live_cleanup_at,
};
use airs_test_harness::live::eval::{EVAL_PROFILE, EvalGate, run_live_eval};
use airs_test_harness::live::ledger::{self, now_unix};
use airs_test_harness::live::writes::{RESOURCE_PREFIX, run_live_writes, writes_gate};
use airs_test_harness::live::{
    BENCH_CHECKS, BENCH_WARMUP, BENIGN_PROMPT, INJECTION_PROMPT, LiveAbort, LiveGate, LivePlan,
    LiveReport, LiveRow, LiveStore, LiveTarget, MISSING_PROFILE, MISSING_PROFILE_PROMPT,
    PAIR_PROMPT, PLANNED_SCANS, ResourceCounts, STALE_STORE_AGE, real_registry_path, run_live,
    run_live_bench, sweep_stale_stores,
};
use airs_test_harness::{
    AsyncRule, Cli, FixtureServer, FixtureTenant, Planned, RegistryEntry, Route, Sandbox, fixtures,
    reference, require_ts, workspace_root,
};
use serde_json::json;

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

/// Writes to the process stderr, which libtest does not capture.
fn notice(text: &str) {
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "{text}");
}

#[test]
fn live_tenant_comparison() {
    let settings = match LiveGate::from_env() {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1 and AIRS_E2E_TENANT=<name> to compare both CLIs against a real tenant)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let ts = reference().unwrap_or_else(|reason| {
        panic!("live tier: the TypeScript reference CLI is unusable: {reason}")
    });
    let registry = real_registry_path().unwrap_or_else(|problem| panic!("live tier: {problem}"));
    let target = LiveTarget::real(registry, &settings.tenant)
        .unwrap_or_else(|problem| panic!("live tier: {problem}"));
    let mut plan = LivePlan::new(target, rust(), ts);
    plan.scan_profile = settings.profile.clone();
    plan.max_scans = settings.max_scans;
    if settings.profile.is_none() {
        notice("LIVE: AIRS_E2E_PROFILE is not set; scans are skipped");
    }
    let report = match run_live(&plan) {
        Ok(report) => report,
        Err(abort) => panic!("live tier stopped: {abort}"),
    };
    notice(&report.table());
    let summary = workspace_root().join("target/live/summary.json");
    if let Some(parent) = summary.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(&report.to_json()) {
        Ok(text) => match std::fs::write(&summary, text + "\n") {
            Ok(()) => notice(&format!("LIVE: summary written to {}", summary.display())),
            Err(err) => notice(&format!("LIVE: cannot write {}: {err}", summary.display())),
        },
        Err(err) => notice(&format!("LIVE: cannot serialise the summary: {err}")),
    }
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private airs-cli-rs store under target/live could not be removed; \
         it holds a copy of the tenant's secrets, remove it by hand"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: the CLIs disagree on {mismatched:?} (see the summary above)"
    );
}

// ---------------------------------------------------------------------------------------
// Self-tests (loopback; no opt-in)
// ---------------------------------------------------------------------------------------

const PROFILE: &str = "live-fixture-profile";
const TOPIC_DESCRIPTION: &str = "fixture topic description that must stay out of summaries";

/// Fixture tenant `prod` (TSG 100) with profiles, topics and scan rules.
fn fixture() -> (FixtureServer, Sandbox, FixtureTenant) {
    fixture_with(|_| {})
}

/// [`fixture`], with `adjust` applied after the profiles and topics are set and before
/// the standard scan rules are added (scan rules are tried in insertion order, so a
/// rule `adjust` adds wins).
fn fixture_with(adjust: impl FnOnce(&FixtureServer)) -> (FixtureServer, Sandbox, FixtureTenant) {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let tenant = sandbox.single_tenant(&server, "prod", "100");
    server.set_profiles(
        "100",
        vec![
            fixtures::profile(1, "alpha", 1),
            fixtures::profile(2, "alpha", 2),
            fixtures::profile(3, PROFILE, 1),
        ],
    );
    server.set_topics(
        "100",
        vec![
            fixtures::topic(11, "pii", 1, TOPIC_DESCRIPTION),
            fixtures::topic(12, "pii", 2, TOPIC_DESCRIPTION),
            fixtures::topic(13, "weapons", 1, TOPIC_DESCRIPTION),
        ],
    );
    adjust(&server);
    server.add_scan_verdict(INJECTION_PROMPT, &fixtures::scan_block(PROFILE));
    server.add_scan_rule(
        MISSING_PROFILE_PROMPT,
        Planned::json(
            400,
            &json!({"error": {"message": "Profile not found"}, "status": 400}),
        ),
    );
    (server, sandbox, tenant)
}

fn plan(sandbox: &Sandbox, ts: Cli) -> LivePlan {
    LivePlan::new(LiveTarget::loopback(sandbox, "prod"), rust(), ts)
}

fn row<'a>(report: &'a LiveReport, name: &str) -> &'a LiveRow {
    report
        .rows
        .iter()
        .find(|row| row.check == name)
        .unwrap_or_else(|| panic!("{name} missing:\n{}", report.table()))
}

/// Asserts that exactly `expected` rows mismatch.
fn assert_mismatches(report: &LiveReport, expected: &[&str]) {
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert_eq!(mismatched, expected, "{}", report.table());
}

/// Whether `route` received a request whose user agent contains `marker`.
fn route_seen_by(server: &FixtureServer, route: Route, marker: &str) -> bool {
    server.requests_for(route).iter().any(|request| {
        request
            .header("user-agent")
            .is_some_and(|ua| ua.contains(marker))
    })
}

/// The summary (table and JSON) carries no secret, token, ID, path or description.
fn assert_sanitized(report: &LiveReport, sandbox: &Sandbox, tenant: &FixtureTenant) {
    let text = format!(
        "{}\n{}",
        report.table(),
        serde_json::to_string(&report.to_json()).unwrap_or_default()
    );
    for secret in sandbox.secrets() {
        assert!(
            !text.contains(&secret),
            "the summary carries a fixture secret"
        );
    }
    let root = sandbox.root().display().to_string();
    let config = tenant.config_path.display().to_string();
    let forbidden = [
        "FAKE-",
        "mock-token",
        "client-100",
        "0000000000a1",
        "0000000000b2",
        "R00000000",
        "00000000-0000-4000-8000",
        "creator@example.com",
        "example",
        TOPIC_DESCRIPTION,
        BENIGN_PROMPT,
        INJECTION_PROMPT,
        PROFILE,
        "127.0.0.1",
        root.as_str(),
        config.as_str(),
        "tenants.json",
    ];
    for needle in forbidden {
        assert!(
            !text.contains(needle),
            "the summary carries {needle:?}:\n{text}"
        );
    }
}

#[test]
fn live_flow_self_test_against_fixture() {
    let Some(ts) = require_ts("live_flow_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = fixture();
    let before = sandbox.full_snapshot();
    let mut plan = plan(&sandbox, ts);
    plan.scan_profile = Some(PROFILE.into());
    let report = run_live(&plan).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    // Captured by libtest; shown with --nocapture or on failure.
    println!("{table}");
    assert!(
        report.mismatches().is_empty(),
        "self-test mismatches:\n{table}"
    );
    assert_eq!(report.scan_invocations, PLANNED_SCANS);
    assert_eq!(
        server.requests_for(Route::ScanSync).len(),
        PLANNED_SCANS as usize
    );
    // Both binaries ran every check: equal invocation counts, and every route the
    // management, doctor and scan checks use was reached by both SDKs.
    assert_eq!(report.rust_runs, report.ts_runs, "{table}");
    assert_eq!(report.rust_runs as usize, report.rows.len(), "{table}");
    for route in [
        Route::Profiles,
        Route::Topics,
        Route::ScanResults,
        Route::ScanSync,
    ] {
        for marker in ["rust-sdk", "typescript-sdk"] {
            assert!(
                route_seen_by(&server, route, marker),
                "{} got no request from the {marker}",
                route.label()
            );
        }
    }
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "tenant read --output json",
        "tenant get airsApiKey",
        "doctor --output json",
        "doctor --output pretty",
        "profiles list --all --max 500",
        "profiles list --all --max 500 --output table",
        "profiles get <first>",
        "profiles get <missing>",
        "topics list --limit 20",
        "topics list --limit 20 --output table",
        "topics get <first>",
        "scan benign",
        "scan injection",
        "scan prompt+response",
        "scan <missing profile>",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    let row = |name: &str| row(&report, name);
    assert_eq!(
        row("scan benign").rust_verdict.as_deref(),
        Some("allow/benign/no")
    );
    assert_eq!(
        row("scan injection").rust_verdict.as_deref(),
        Some("block/malicious/yes")
    );
    assert_eq!(
        row("scan injection").ts_verdict.as_deref(),
        Some("block/malicious/yes")
    );
    assert!(row("scan injection").note.contains("D7"), "{table}");
    assert_eq!(row("scan <missing profile>").rust_exit, "1");
    assert_eq!(row("profiles list --all --max 500").note, "2 rows");
    assert_eq!(
        row("profiles list --all --max 500 --output table").note,
        "2 rows"
    );
    assert!(
        row("topics list --limit 20 --output table").agree,
        "{table}"
    );
    assert!(row("tenant read --output json").note.contains("redacted"));
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.starts_with("injection prompt blocked")),
        "{table}"
    );
    assert_sanitized(&report, &sandbox, &tenant);
    assert!(
        Sandbox::changes_under(sandbox.root(), &before).is_empty(),
        "the flow changed the sandbox tenant files"
    );
    server.assert_no_unknown_routes();
}

#[test]
fn live_flow_without_profile_skips_scans_visibly() {
    let Some(ts) = require_ts("live_flow_without_profile_skips_scans_visibly") else {
        return;
    };
    let (server, sandbox, tenant) = fixture();
    let report = run_live(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    assert!(report.mismatches().is_empty(), "{}", report.table());
    assert_eq!(report.scan_invocations, 0);
    assert!(server.requests_for(Route::ScanSync).is_empty());
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.starts_with("scans skipped")),
        "{}",
        report.table()
    );
    assert!(report.table().contains("note: scans skipped"));
    assert_sanitized(&report, &sandbox, &tenant);
}

#[test]
fn live_flow_refuses_a_budget_below_the_plan_before_any_request() {
    let (server, sandbox, _tenant) = fixture();
    // The flow must stop before running either CLI, so a placeholder TS entry is enough.
    let mut plan = plan(&sandbox, rust());
    plan.scan_profile = Some(PROFILE.into());
    plan.max_scans = PLANNED_SCANS - 1;
    assert!(matches!(run_live(&plan), Err(LiveAbort::Budget(_))));
    plan.max_scans = 21;
    assert!(matches!(run_live(&plan), Err(LiveAbort::Budget(_))));
    assert_eq!(server.request_count(), 0);
}

#[test]
fn live_flow_reports_a_mismatch_without_stopping() {
    let Some(ts) = require_ts("live_flow_reports_a_mismatch_without_stopping") else {
        return;
    };
    let (server, sandbox, _tenant) = fixture();
    // Every profiles listing answers differently for the two CLIs: the Rust run (first)
    // gets the planned page, the TS run the fixture data.
    server.push_once(
        Route::Profiles,
        Planned::json(
            200,
            &json!({"ai_profiles": [fixtures::profile(9, "zeta", 7)]}),
        ),
    );
    let report = run_live(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.contains(&"profiles list --all --max 500"),
        "{}",
        report.table()
    );
    // Later checks still ran.
    assert!(
        report
            .rows
            .iter()
            .any(|row| row.check == "topics get <first>")
    );
    assert!(!report.table().contains("zeta"));
}

/// Each row here is wrong in exactly one way that a single comparison rule catches:
/// - `doctor --output json`: the Rust Scanner API probe is rejected (HTTP 401), the TS
///   probe passes, so the statuses differ;
/// - `profiles list --all --max 500 --output table`: the Rust JSON, TS JSON and Rust
///   table listings get a one-profile page, the TS table the three fixture profiles;
/// - `profiles get <missing>`: the "missing" name exists, so both CLIs exit 0;
/// - `scan benign`: both CLIs get the same block verdict, so only the benign rule fails;
/// - `scan <missing profile>`: the service allows the scan, so both CLIs exit 0.
#[test]
fn live_flow_flags_a_mismatch_for_each_rule() {
    let Some(ts) = require_ts("live_flow_flags_a_mismatch_for_each_rule") else {
        return;
    };
    let (server, sandbox, tenant) = fixture_with(|server| {
        server.set_profiles(
            "100",
            vec![
                fixtures::profile(1, "alpha", 1),
                fixtures::profile(3, PROFILE, 1),
                fixtures::profile(4, MISSING_PROFILE, 1),
            ],
        );
        server.add_scan_verdict(
            MISSING_PROFILE_PROMPT,
            &fixtures::scan_allow(MISSING_PROFILE),
        );
    });
    // The first probe of the scanner results route is the Rust doctor's.
    server.push_once(Route::ScanResults, Planned::empty_json(401));
    // Profiles listings run Rust JSON, TS JSON, Rust table, TS table: only the last one
    // sees the fixture data.
    for _ in 0..3 {
        server.push_once(
            Route::Profiles,
            Planned::json(
                200,
                &json!({"ai_profiles": [fixtures::profile(1, "alpha", 1)]}),
            ),
        );
    }
    // The first two scans are the benign prompt on Rust, then on TS.
    for _ in 0..2 {
        server.push_once(
            Route::ScanSync,
            Planned::json(200, &fixtures::scan_block(PROFILE)),
        );
    }
    let mut plan = plan(&sandbox, ts);
    plan.scan_profile = Some(PROFILE.into());
    let report = run_live(&plan).unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(
        &report,
        &[
            "doctor --output json",
            "profiles list --all --max 500 --output table",
            "profiles get <missing>",
            "scan benign",
            "scan <missing profile>",
        ],
    );
    assert_eq!(
        row(&report, "profiles list --all --max 500 --output table").note,
        "differs: table rows (1 vs 3)"
    );
    assert!(
        row(&report, "doctor --output json")
            .note
            .contains("differs: Scanner API"),
        "{}",
        report.table()
    );
    let benign = row(&report, "scan benign");
    assert_eq!(benign.rust_verdict, benign.ts_verdict);
    assert!(benign.note.starts_with("benign prompt not allowed"));
    let missing = row(&report, "scan <missing profile>");
    assert_eq!(
        (missing.rust_exit.as_str(), missing.ts_exit.as_str()),
        ("0", "0")
    );
    assert_eq!(report.scan_invocations, PLANNED_SCANS);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// A content scan with no verdict on either CLI (HTTP 400 for both) is a mismatch, and
/// the scan series stops there: the missing-profile scan is not run.
#[test]
fn live_flow_flags_a_scan_without_verdicts_and_stops_the_scans() {
    let Some(ts) = require_ts("live_flow_flags_a_scan_without_verdicts_and_stops_the_scans") else {
        return;
    };
    let (server, sandbox, _tenant) = fixture_with(|server| {
        server.add_scan_rule(
            PAIR_PROMPT,
            Planned::json(
                400,
                &json!({"error": {"message": "Bad request"}, "status": 400}),
            ),
        );
    });
    let mut plan = plan(&sandbox, ts);
    plan.scan_profile = Some(PROFILE.into());
    let report = run_live(&plan).unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &["scan prompt+response"]);
    let pair = row(&report, "scan prompt+response");
    assert_eq!((pair.rust_exit.as_str(), pair.ts_exit.as_str()), ("1", "1"));
    assert!(
        pair.note.contains("no verdict from either CLI (exit 1/1)"),
        "{}",
        pair.note
    );
    assert!(
        !report
            .rows
            .iter()
            .any(|row| row.check == "scan <missing profile>")
    );
    assert!(
        report.notices.iter().any(|notice| notice
            == "scans stopped after scan prompt+response did not complete; not run: scan <missing profile>"),
        "{}",
        report.table()
    );
    assert_eq!(report.scan_invocations, 6);
    assert_eq!(server.requests_for(Route::ScanSync).len(), 6);
}

/// A failed scan stops the series even where D6 makes the two outputs agree (Rust
/// FAILED, exit 1; TS ALLOW, exit 0): one failing tenant costs one scan per CLI, not
/// one per planned case.
#[test]
fn live_flow_stops_the_scans_at_the_first_failed_scan() {
    let Some(ts) = require_ts("live_flow_stops_the_scans_at_the_first_failed_scan") else {
        return;
    };
    let (server, sandbox, _tenant) = fixture_with(|server| {
        server.add_scan_verdict(BENIGN_PROMPT, &fixtures::scan_failed());
    });
    let mut plan = plan(&sandbox, ts);
    plan.scan_profile = Some(PROFILE.into());
    let report = run_live(&plan).unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &["scan benign"]);
    let benign = row(&report, "scan benign");
    assert_eq!(benign.rust_exit, "1");
    assert!(
        benign
            .note
            .contains("scan did not complete; later scans not run"),
        "{}",
        benign.note
    );
    assert!(
        report.notices.iter().any(|notice| notice
            == "scans stopped after scan benign did not complete; not run: \
                scan injection, scan prompt+response, scan <missing profile>"),
        "{}",
        report.table()
    );
    assert_eq!(report.scan_invocations, 2);
    assert_eq!(server.requests_for(Route::ScanSync).len(), 2);
}

/// The live benchmark flow (`examples/bench.rs --live`) against the fixture: every
/// check times both CLIs, nothing is scanned, the tenant files stay unchanged, and the
/// report carries only fixed labels, exit codes and durations.
#[test]
fn live_bench_self_test_against_fixture() {
    let Some(ts) = require_ts("live_bench_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = fixture();
    let before = sandbox.full_snapshot();
    let iterations = 2;
    let report = run_live_bench(
        LiveTarget::loopback(&sandbox, "prod"),
        rust(),
        ts,
        iterations,
    )
    .unwrap_or_else(|abort| panic!("{abort}"));
    assert_eq!(report.rows.len(), BENCH_CHECKS.len() * 2, "{report:?}");
    for row in &report.rows {
        assert_eq!(row.exits, vec!["0"; iterations as usize], "{row:?}");
        assert_eq!(row.wall_ms.len(), iterations as usize);
        assert!(row.wall_ms.iter().all(|ms| *ms > 0.0), "{row:?}");
    }
    let runs = BENCH_CHECKS.len() as u32 * (BENCH_WARMUP + iterations);
    assert_eq!((report.rust_runs, report.ts_runs), (runs, runs));
    assert!(server.requests_for(Route::ScanSync).is_empty());
    for route in [Route::Profiles, Route::Topics] {
        for marker in ["rust-sdk", "typescript-sdk"] {
            assert!(route_seen_by(&server, route, marker), "{marker}");
        }
    }
    assert!(Sandbox::changes_under(sandbox.root(), &before).is_empty());
    let text = serde_json::to_string(&report.to_json()).unwrap_or_default();
    let root = sandbox.root().display().to_string();
    for needle in [
        "FAKE-",
        "mock-token",
        "client-100",
        "00000000-0000-4000-8000",
        "example",
        "alpha",
        "pii",
        "\"prod\"",
        TOPIC_DESCRIPTION,
        PROFILE,
        "127.0.0.1",
        root.as_str(),
        "tenants.json",
    ] {
        assert!(
            !text.contains(needle),
            "the report carries {needle:?}:\n{text}"
        );
    }
    for secret in sandbox.secrets() {
        assert!(
            !text.contains(&secret),
            "the report carries a fixture secret"
        );
    }
    let _ = tenant;
}

#[test]
fn real_target_requires_the_opt_in() {
    if std::env::var("AIRS_RS_LIVE").as_deref() == Ok("1") {
        notice("LIVE: real_target_requires_the_opt_in has nothing to prove with AIRS_RS_LIVE=1");
        return;
    }
    // Refused before any file is touched: the path does not exist.
    let refused = LiveTarget::real("/nonexistent/airs-live/tenants.json", "prod");
    assert_eq!(
        refused.err().as_deref(),
        Some("the live tier needs AIRS_RS_LIVE=1 in the test process")
    );
}

#[test]
fn loopback_target_refuses_a_non_loopback_config() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let tenant =
        sandbox.add_fixture_tenant_allowing_external_endpoints(&server, "prod", "100", |config| {
            config.set("mgmtEndpoint", "https://api.example.invalid/aisec");
        });
    sandbox.register(&[&tenant], Some("prod"));
    let plan = plan(&sandbox, rust());
    match run_live(&plan) {
        Err(LiveAbort::Setup(problem)) => assert!(problem.contains("loopback"), "{problem}"),
        other => panic!("expected a setup refusal, got {other:?}"),
    }
    assert_eq!(server.request_count(), 0);
}

#[test]
fn live_flow_rejects_an_unregistered_tenant() {
    let (server, sandbox, _tenant) = fixture();
    let plan = LivePlan::new(LiveTarget::loopback(&sandbox, "staging"), rust(), rust());
    match run_live(&plan) {
        Err(LiveAbort::Setup(problem)) => {
            assert!(problem.contains("not registered") && problem.contains("1 registered"));
        }
        other => panic!("expected a setup refusal, got {other:?}"),
    }
    assert_eq!(server.request_count(), 0);
}

// ---------------------------------------------------------------------------------------
// The private airs-cli-rs store (DESIGN §10): `tenant import` into
// `<store root>/store-<run id>/`, removed when the flow ends
// ---------------------------------------------------------------------------------------

/// The `store-*` entries under `root`, sorted.
fn stores_under(root: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with("store-"))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// A directory whose modification time is `age` ago.
fn aged_dir(path: &std::path::Path, age: std::time::Duration) {
    std::fs::create_dir_all(path.join("tenants")).unwrap();
    std::fs::write(path.join("config.toml"), "version = 1\n").unwrap();
    let past = std::time::SystemTime::now() - age;
    std::fs::File::open(path)
        .unwrap()
        .set_modified(past)
        .unwrap();
}

#[test]
fn live_flow_imports_into_a_private_store_and_removes_it() {
    let Some(ts) = require_ts("live_flow_imports_into_a_private_store_and_removes_it") else {
        return;
    };
    let (server, sandbox, tenant) = fixture();
    let root = sandbox.tmp().join("live-stores");
    let before = sandbox.full_snapshot();
    let mut plan = plan(&sandbox, ts);
    plan.store_root = root.clone();
    let report = run_live(&plan).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert!(report.mismatches().is_empty(), "{table}");
    assert_eq!(report.store_removed, Some(true), "{table}");
    assert_eq!(report.stale_stores_removed, 0);
    assert!(
        table.contains("note: private airs-cli-rs store removed"),
        "{table}"
    );
    assert_eq!(report.to_json()["storeRemoved"], json!(true));
    assert!(root.is_dir(), "the store root is created");
    assert_eq!(
        stores_under(&root),
        Vec::<String>::new(),
        "the store is gone"
    );
    // The Rust runs used the imported tenant: its credentials reached the server.
    assert!(route_seen_by(&server, Route::Profiles, "rust-sdk"));
    // The import is not a check run, and only read the fixture registry and config.
    assert_eq!(report.rust_runs, report.ts_runs, "{table}");
    assert!(
        Sandbox::changes_under(sandbox.root(), &before)
            .iter()
            .all(|change| change.contains("tmp/")),
        "the flow changed the fixture tenant files"
    );
    assert_sanitized(&report, &sandbox, &tenant);
}

#[test]
fn live_flow_removes_the_store_when_the_import_fails() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let tenant = sandbox.add_fixture_tenant(&server, "prod", "100");
    // The registry pins another TSG: the import skips the tenant, so the flow stops.
    sandbox.write_registry(
        &[RegistryEntry::new("prod", &tenant.config_path, "999")],
        Some("prod"),
    );
    let root = sandbox.tmp().join("live-stores");
    let mut plan = plan(&sandbox, rust());
    plan.store_root = root.clone();
    match run_live(&plan) {
        Err(LiveAbort::Setup(problem)) => {
            assert!(
                problem.contains("tenant import did not import"),
                "{problem}"
            );
            assert!(!problem.contains("FAKE-"), "{problem}");
        }
        other => panic!("expected a setup failure, got {other:?}"),
    }
    assert_eq!(
        stores_under(&root),
        Vec::<String>::new(),
        "the store is gone"
    );
    assert_eq!(server.request_count(), 0);
}

/// A loopback flow keeps its private stores inside its sandbox: by default under
/// `<sandbox>/tmp/live-stores` (never the shared `target/live`), and a store root outside
/// the sandbox is refused before anything is created or swept.
#[test]
fn loopback_flows_keep_their_stores_inside_the_sandbox() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let tenant = sandbox.add_fixture_tenant(&server, "prod", "100");
    sandbox.write_registry(
        &[RegistryEntry::new("prod", &tenant.config_path, "999")],
        Some("prod"),
    );
    let plan = plan(&sandbox, rust());
    assert_eq!(plan.store_root, sandbox.tmp().join("live-stores"));
    // The flow stops at the import (pin mismatch); its store lived in the sandbox.
    assert!(matches!(run_live(&plan), Err(LiveAbort::Setup(_))));
    assert!(plan.store_root.is_dir(), "the store root is in the sandbox");
    assert_eq!(stores_under(&plan.store_root), Vec::<String>::new());
    let outside = Sandbox::new();
    let root = outside.tmp().join("live-stores");
    aged_dir(
        &root.join("store-killed-run"),
        std::time::Duration::from_secs(7200),
    );
    let mut plan = plan;
    plan.store_root = root.clone();
    match run_live(&plan) {
        Err(LiveAbort::Setup(problem)) => assert!(
            problem.contains("store root lies outside its harness sandbox"),
            "{problem}"
        ),
        other => panic!("expected a setup refusal, got {other:?}"),
    }
    assert_eq!(
        stores_under(&root),
        ["store-killed-run"],
        "nothing swept or created"
    );
    assert_eq!(server.request_count(), 0);
}

#[test]
fn live_flow_sweeps_only_stale_stores() {
    let Some(ts) = require_ts("live_flow_sweeps_only_stale_stores") else {
        return;
    };
    let (_server, sandbox, _tenant) = fixture();
    let root = sandbox.tmp().join("live-stores");
    let hour = std::time::Duration::from_secs(3600);
    aged_dir(&root.join("store-killed-run"), hour * 2);
    aged_dir(
        &root.join("store-concurrent-run"),
        std::time::Duration::from_secs(60),
    );
    aged_dir(&root.join("summary-old"), hour * 2);
    let mut plan = plan(&sandbox, ts);
    plan.store_root = root.clone();
    let report = run_live(&plan).unwrap_or_else(|abort| panic!("{abort}"));
    assert_eq!(report.stale_stores_removed, 1, "{}", report.table());
    assert_eq!(stores_under(&root), ["store-concurrent-run"]);
    assert!(root.join("summary-old").is_dir(), "other names are kept");
    assert!(
        report
            .table()
            .contains("1 leftover store(s) older than 1 hour removed")
    );
}

#[test]
fn live_store_is_private_exclusive_and_removed_on_panic() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new();
    let root = sandbox.tmp().join("live-stores");
    let store = LiveStore::create(&root).unwrap();
    let mode = std::fs::metadata(store.path())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700);
    let other = LiveStore::create(&root).unwrap();
    assert_ne!(
        store.path(),
        other.path(),
        "every flow gets its own directory"
    );
    drop(other);
    let path = store.path().to_owned();
    std::fs::write(path.join("config.toml"), "version = 1\n").unwrap();
    // A panic while the guard is alive (as in a failing check) removes the store.
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = store;
        panic!("a failing live check");
    }));
    assert!(unwound.is_err());
    assert!(
        !path.exists(),
        "the store is removed while the panic unwinds"
    );
    assert_eq!(stores_under(&root), Vec::<String>::new());
    assert_eq!(
        sweep_stale_stores(&root.join("missing"), STALE_STORE_AGE),
        0
    );
}

// ---------------------------------------------------------------------------------------
// M4: the write flow, the sweeper and the eval flow (DESIGN §10)
// ---------------------------------------------------------------------------------------

/// Writes a live summary under `target/live/` (real runs only).
fn write_summary(file: &str, report: &LiveReport) {
    let summary = workspace_root().join("target/live").join(file);
    if let Some(parent) = summary.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(&report.to_json()) {
        Ok(text) => match std::fs::write(&summary, text + "\n") {
            Ok(()) => notice(&format!("LIVE: summary written to {}", summary.display())),
            Err(err) => notice(&format!("LIVE: cannot write {}: {err}", summary.display())),
        },
        Err(err) => notice(&format!("LIVE: cannot serialise the summary: {err}")),
    }
}

/// The real target of a gated M4 flow.
fn real_plan(tenant: &str) -> LivePlan {
    let ts = reference().unwrap_or_else(|reason| {
        panic!("live tier: the TypeScript reference CLI is unusable: {reason}")
    });
    let registry = real_registry_path().unwrap_or_else(|problem| panic!("live tier: {problem}"));
    let target =
        LiveTarget::real(registry, tenant).unwrap_or_else(|problem| panic!("live tier: {problem}"));
    LivePlan::new(target, rust(), ts)
}

/// The M4 write flow against the selected tenant: run-created `airs-rs-e2e-<run id>-…`
/// resources only, at most 10, journalled to `target/live/ledger.jsonl`, torn down at
/// the end. Needs `AIRS_RS_LIVE_WRITES=1` as well.
#[test]
fn live_write_flow() {
    let settings = match writes_gate(|name| std::env::var(name).ok()) {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1, AIRS_E2E_TENANT=<name> and AIRS_RS_LIVE_WRITES=1 for the write flow)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.tenant);
    let report = match run_live_writes(&plan) {
        Ok(report) => report,
        Err(abort) => panic!(
            "live write flow stopped: {abort} (see target/live/ledger.jsonl; run the sweeper with AIRS_RS_LIVE_CLEANUP=1 after an hour)"
        ),
    };
    notice(&report.table());
    write_summary("summary-writes.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    let left = report.resources.map_or(1, |resources| resources.left);
    assert_eq!(
        left, 0,
        "live tier: resources were left behind; see target/live/ledger.jsonl and run the sweeper"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The sweeper: deletes only `airs-rs-e2e-…` resources older than an hour (at most 20
/// per run, profiles first, plain deletes). Safe to re-run. Needs
/// `AIRS_RS_LIVE_CLEANUP=1` as well.
#[test]
fn live_cleanup() {
    let settings = match cleanup_gate(|name| std::env::var(name).ok()) {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1, AIRS_E2E_TENANT=<name> and AIRS_RS_LIVE_CLEANUP=1 to sweep leftovers)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.tenant);
    let report = match run_live_cleanup(&plan) {
        Ok(report) => report,
        Err(abort) => panic!("live sweeper stopped: {abort}"),
    };
    notice(&report.table());
    write_summary("summary-cleanup.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
}

/// The eval flow: both CLIs evaluate the `topics sample` CSV against "Prisma AIRS
/// Harness" (4–8 scans). Needs `AIRS_RS_LIVE_EVAL=1` and that profile.
#[test]
fn live_eval() {
    let settings = match EvalGate::from_env() {
        Ok(EvalGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1, AIRS_E2E_TENANT=<name>, AIRS_RS_LIVE_EVAL=1 and AIRS_E2E_PROFILE=\"{EVAL_PROFILE}\" to run the eval)"
            ));
            return;
        }
        Ok(EvalGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let mut plan = real_plan(&settings.tenant);
    plan.scan_profile = Some(EVAL_PROFILE.into());
    let report = match run_live_eval(&plan, settings.max_scans) {
        Ok(report) => report,
        Err(abort) => panic!("live eval stopped: {abort}"),
    };
    notice(&report.table());
    write_summary("summary-eval.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The ids of rows whose `name_key` starts with the prefix.
fn ids_of(rows: &[serde_json::Value], id_key: &str) -> std::collections::BTreeSet<String> {
    rows.iter()
        .filter_map(|row| row.get(id_key).and_then(serde_json::Value::as_str))
        .map(str::to_owned)
        .collect()
}

/// A profile whose topic guardrails reference topic `topic_id`.
fn referencing_profile(n: u64, name: &str, topic_id: &str, topic_name: &str) -> serde_json::Value {
    let mut profile = fixtures::profile(n, name, 1);
    profile["policy"]["ai-security-profiles"][0]["model-configuration"]["model-protection"] = json!([
        {"name": "topic-guardrails", "action": "allow", "options": [], "topic-list": [
            {"action": "block", "topic": [{"topic_id": topic_id, "topic_name": topic_name, "revision": 1}]}
        ]}
    ]);
    profile
}

/// The ledger of a loopback flow (`<sandbox>/tmp/live-stores/ledger.jsonl`).
fn ledger_lines(sandbox: &Sandbox) -> Vec<serde_json::Value> {
    ledger::read(&sandbox.tmp().join("live-stores").join(ledger::LEDGER_FILE))
}

/// Checks the ledger rules: run-created names only, UUID ids, fixed fields, each
/// result line preceded by its `pending` line, nothing tenant-specific.
fn assert_ledger_clean(lines: &[serde_json::Value], sandbox: &Sandbox) {
    assert!(!lines.is_empty(), "the ledger is empty");
    let text = serde_json::to_string(lines).unwrap();
    for secret in sandbox.secrets() {
        assert!(
            !text.contains(&secret),
            "the ledger carries a fixture secret"
        );
    }
    for needle in [
        "mock-token",
        "127.0.0.1",
        "\"prod\"",
        TOPIC_DESCRIPTION,
        "creator@",
    ] {
        assert!(!text.contains(needle), "the ledger carries {needle:?}");
    }
    let mut open: Vec<(String, String, String)> = Vec::new();
    for line in lines {
        assert_eq!(line["v"], 1);
        let name = line["name"].as_str().unwrap_or_default();
        assert!(name.starts_with(RESOURCE_PREFIX), "{line}");
        if let Some(id) = line["id"].as_str() {
            let dlp = matches!(line["kind"].as_str(), Some("pattern" | "dictionary"));
            assert!(
                ledger::is_uuid(id) || (dlp && ledger::is_dlp_id(id)),
                "{line}"
            );
        }
        let key = (
            line["op"].as_str().unwrap_or_default().to_owned(),
            name.to_owned(),
            line["phase"].as_str().unwrap_or_default().to_owned(),
        );
        match line["outcome"].as_str() {
            Some("pending") => open.push(key),
            // A planned write that was not needed (a reused or verified restore copy)
            // closes its pending line; otherwise `skipped` stands alone (a TS skip).
            Some("skipped") => {
                if let Some(at) = open.iter().position(|item| *item == key) {
                    open.remove(at);
                }
            }
            Some("ok" | "failed" | "unknown") => {
                let at = open.iter().position(|item| *item == key);
                assert!(at.is_some(), "a result without its pending line: {line}");
                open.remove(at.unwrap_or_default());
            }
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    assert!(open.is_empty(), "pending lines without a result: {open:?}");
}

#[test]
fn live_write_flow_self_test_against_fixture() {
    let Some(ts) = require_ts("live_write_flow_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = fixture();
    // Pre-existing resources, including names of an older run and look-alikes: the
    // flow must not touch any of them.
    let mut topics = server.topics("100");
    topics.push(fixtures::topic(
        51,
        "airs-rs-e2e-1000000000-1-0-topic-rs",
        1,
        TOPIC_DESCRIPTION,
    ));
    server.set_topics("100", topics);
    let mut profiles = server.profiles("100");
    profiles.push(fixtures::profile(
        50,
        "airs-rs-e2e-1000000000-1-0-profile-rs",
        1,
    ));
    profiles.push(fixtures::profile(52, "airs-rs-e2e-precious", 1));
    server.set_profiles("100", profiles);
    let topics_before = server.topics("100");
    let profiles_before = server.profiles("100");
    let before_ids: std::collections::BTreeSet<String> = ids_of(&topics_before, "topic_id")
        .into_iter()
        .chain(ids_of(&profiles_before, "profile_id"))
        .collect();
    let report = run_live_writes(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    assert_eq!(
        report.resources,
        Some(ResourceCounts {
            created: 3,
            deleted: 3,
            left: 0
        }),
        "{table}"
    );
    assert_eq!(report.store_removed, Some(true));
    // Nothing that existed before changed, and nothing the run made is left.
    assert_eq!(server.topics("100"), topics_before);
    assert_eq!(server.profiles("100"), profiles_before);
    // Every write named a run-created resource.
    for request in server.requests() {
        if !request.route.is_write() {
            continue;
        }
        if let Some(body) = request.body_json() {
            for key in ["topic_name", "profile_name"] {
                if let Some(name) = body.get(key).and_then(serde_json::Value::as_str) {
                    assert!(name.starts_with(RESOURCE_PREFIX), "{}", request.line());
                    assert!(
                        !name.starts_with("airs-rs-e2e-1000000000"),
                        "{}",
                        request.line()
                    );
                }
            }
        }
        for id in &before_ids {
            assert!(
                !request.path.contains(id.as_str()),
                "touched {}",
                request.line()
            );
        }
    }
    // TypeScript topics create/apply/revert never ran against a loopback target (D93).
    for route in [Route::TopicCreate, Route::TopicUpdate, Route::TopicDelete] {
        assert!(
            !route_seen_by(&server, route, "typescript-sdk"),
            "{}",
            route.label()
        );
    }
    assert!(route_seen_by(
        &server,
        Route::ProfileCreate,
        "typescript-sdk"
    ));
    assert!(
        report.notices.iter().any(|notice| notice.contains("D93")),
        "{table}"
    );
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "topics create (rs)",
        "topics get after create (rs)",
        "topics create again (rs)",
        "topics update (rs)",
        "profiles create (rs)",
        "profiles create (ts)",
        "profiles update (rs)",
        "profiles update (ts)",
        "topics apply (rs)",
        "topics apply guardrails",
        "topics revert --force (rs)",
        "teardown profiles delete",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    let lines = ledger_lines(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    let creates = lines
        .iter()
        .filter(|line| {
            line["outcome"] == "pending"
                && matches!(
                    line["op"].as_str(),
                    Some("topics create" | "profiles create")
                )
        })
        .count();
    assert_eq!(creates, 4, "topic-rs twice, profile-rs and profile-ts");
    assert!(
        lines
            .iter()
            .any(|line| line["outcome"] == "skipped" && line["note"] == "D93"),
        "the skipped TypeScript steps are journalled"
    );
    assert!(lines.iter().any(|line| line["phase"] == "teardown"));
    assert_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

#[test]
fn live_write_flow_tears_down_after_an_unknown_outcome() {
    let Some(ts) = require_ts("live_write_flow_tears_down_after_an_unknown_outcome") else {
        return;
    };
    let (server, sandbox, _) = fixture();
    let profiles_before = server.profiles("100");
    let topics_before = server.topics("100");
    // The first profile update (airs-cli-rs, profile-rs) has an unknown outcome.
    server.push_once(
        Route::ProfileUpdate,
        Planned::json(503, &json!({})).with_retry_after(0),
    );
    let report = run_live_writes(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert_mismatches(&report, &["profiles update (rs)"]);
    assert_eq!(report.resources.map(|r| r.left), Some(0), "{table}");
    assert_eq!(server.profiles("100"), profiles_before);
    assert_eq!(server.topics("100"), topics_before);
    let lines = ledger_lines(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    assert!(
        lines
            .iter()
            .any(|line| line["op"] == "profiles update" && line["outcome"] == "unknown"),
        "{lines:?}"
    );
}

/// Seeds `count` old prefixed topics (created 2 h + n s ago) and returns their ids.
fn old_topics(now: u64, first: u64, count: u64) -> Vec<serde_json::Value> {
    (0..count)
        .map(|n| {
            fixtures::topic(
                first + n,
                &format!("airs-rs-e2e-{}-7-{n}-topic-rs", now - 7200 - n),
                1,
                TOPIC_DESCRIPTION,
            )
        })
        .collect()
}

fn delete_lines(server: &FixtureServer) -> Vec<String> {
    server
        .requests()
        .iter()
        .filter(|request| {
            matches!(
                request.route,
                Route::TopicDelete
                    | Route::ProfileDelete
                    | Route::TopicForceDelete
                    | Route::ProfileForceDelete
            )
        })
        .map(|request| request.line())
        .collect()
}

#[test]
fn live_cleanup_deletes_only_old_prefixed_resources_and_is_idempotent() {
    let (server, sandbox, tenant) = fixture();
    let now = now_unix();
    let old = |kind: &str| format!("airs-rs-e2e-{}-7-0-{kind}-rs", now - 7300);
    let referenced = fixtures::uuid(61);
    server.set_topics(
        "100",
        vec![
            fixtures::topic(11, "pii", 1, TOPIC_DESCRIPTION),
            fixtures::topic(61, &old("topic"), 1, TOPIC_DESCRIPTION),
            fixtures::topic(
                62,
                &format!("airs-rs-e2e-{}-7-1-topic-ts", now - 7200),
                1,
                TOPIC_DESCRIPTION,
            ),
            fixtures::topic(
                63,
                &format!("airs-rs-e2e-{}-7-2-topic-rs", now - 60),
                1,
                TOPIC_DESCRIPTION,
            ),
            fixtures::topic(
                64,
                &format!("airs-rs-e2e-{}-7-3-profile-rs", now - 7200),
                1,
                TOPIC_DESCRIPTION,
            ),
            fixtures::topic(65, "airs-rs-e2e-precious", 1, TOPIC_DESCRIPTION),
        ],
    );
    server.set_profiles(
        "100",
        vec![
            fixtures::profile(1, "alpha", 1),
            referencing_profile(2, "keeper", &referenced, &old("topic")),
            fixtures::profile(71, &old("profile"), 1),
            fixtures::profile(72, &format!("airs-rs-e2e-{}-7-4-profile-ts", now - 30), 1),
        ],
    );
    let mut plan = plan(&sandbox, rust());
    plan.scan_profile = None;
    let report = run_live_cleanup_at(&plan, now).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert_eq!(
        delete_lines(&server),
        [
            format!("DELETE /aisec/v1/mgmt/profile/{}", fixtures::uuid(71)),
            format!("DELETE /aisec/v1/mgmt/topic/{referenced}"),
            format!("DELETE /aisec/v1/mgmt/topic/{}", fixtures::uuid(62)),
        ],
        "profiles first, then topics oldest first; plain deletes only:\n{table}"
    );
    let topic_ids: Vec<String> = ids_of(&server.topics("100"), "topic_id")
        .into_iter()
        .collect();
    let profile_ids: Vec<String> = ids_of(&server.profiles("100"), "profile_id")
        .into_iter()
        .collect();
    assert_eq!(
        topic_ids,
        [11, 61, 63, 64, 65].map(fixtures::uuid),
        "the referenced topic (409) and everything young, foreign or malformed are kept"
    );
    assert_eq!(profile_ids, [1, 2, 72].map(fixtures::uuid));
    assert!(
        report
            .notices
            .iter()
            .any(|n| n.contains("3 selected, 2 deleted, 1 failed or referenced (kept)")),
        "{table}"
    );
    assert_eq!(report.store_removed, Some(true));
    let lines = ledger_lines(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    assert!(lines.iter().all(|line| line["phase"] == "cleanup"));
    assert_eq!(lines.len(), 6);
    assert_sanitized(&report, &sandbox, &tenant);
    // A re-run finds only what is left: the referenced topic, which fails again.
    server.clear_requests();
    run_live_cleanup_at(&plan, now).unwrap_or_else(|abort| panic!("{abort}"));
    assert_eq!(
        delete_lines(&server),
        [format!("DELETE /aisec/v1/mgmt/topic/{referenced}")]
    );
    assert_eq!(ids_of(&server.topics("100"), "topic_id").len(), 5);
    assert_eq!(ids_of(&server.profiles("100"), "profile_id").len(), 3);
}

#[test]
fn live_cleanup_deletes_at_most_twenty_per_run() {
    let (server, sandbox, _) = fixture();
    let now = now_unix();
    server.set_topics("100", old_topics(now, 300, 22));
    server.set_profiles("100", vec![fixtures::profile(1, "alpha", 1)]);
    let plan = plan(&sandbox, rust());
    for (round, expected) in [(0, MAX_DELETES), (1, 2), (2, 0)] {
        server.clear_requests();
        let report = run_live_cleanup_at(&plan, now).unwrap_or_else(|abort| panic!("{abort}"));
        assert_eq!(
            delete_lines(&server).len(),
            expected,
            "round {round}:\n{}",
            report.table()
        );
    }
    // The oldest went first: the two youngest were the ones left for round two.
    assert!(server.topics("100").is_empty());
    assert_eq!(server.profiles("100").len(), 1);
}

#[test]
fn live_eval_self_test_against_fixture() {
    let Some(ts) = require_ts("live_eval_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = fixture();
    let mut blocked = fixtures::scan_block(EVAL_PROFILE);
    blocked["prompt_detected"]["topic_violation"] = json!(true);
    server.add_scan_verdict("bomb", &blocked);
    let mut plan = plan(&sandbox, ts);
    plan.scan_profile = Some(EVAL_PROFILE.into());
    let report = run_live_eval(&plan, 4).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert!(report.mismatches().is_empty(), "{table}");
    assert_eq!(report.scan_invocations, 4);
    assert_eq!(
        server.requests_for(Route::ScanSync).len(),
        4,
        "2 rows on each CLI"
    );
    assert_eq!(
        row(&report, "topics eval (sample CSV)").note,
        "equal TP/TN/FP/FN"
    );
    // Nothing was written: no management write and no profile change.
    assert!(
        server
            .requests()
            .iter()
            .all(|request| !request.route.is_write())
    );
    assert_eq!(report.store_removed, Some(true));
    assert_sanitized(&report, &sandbox, &tenant);
}

#[test]
fn live_eval_accepts_the_d111_stop() {
    let Some(ts) = require_ts("live_eval_accepts_the_d111_stop") else {
        return;
    };
    let (server, sandbox, _) = fixture();
    server.add_scan_verdict("weather", &fixtures::scan_failed());
    let mut plan = plan(&sandbox, ts);
    plan.scan_profile = Some(EVAL_PROFILE.into());
    let report = run_live_eval(&plan, 8).unwrap_or_else(|abort| panic!("{abort}"));
    let eval = row(&report, "topics eval (sample CSV)");
    assert!(eval.agree, "{}", report.table());
    assert!(eval.note.starts_with("D111"), "{}", eval.note);
    assert_eq!(eval.rust_exit, "1");
}

#[test]
fn live_eval_refuses_a_bad_budget_or_profile_before_any_request() {
    let (server, sandbox, _) = fixture();
    let root = sandbox.tmp().join("live-stores");
    for (profile, budget) in [
        (Some(EVAL_PROFILE), 3),
        (Some(EVAL_PROFILE), 9),
        (Some(PROFILE), 4),
        (None, 4),
    ] {
        let mut plan = plan(&sandbox, rust());
        plan.scan_profile = profile.map(str::to_owned);
        let abort = run_live_eval(&plan, budget).err();
        assert!(
            matches!(abort, Some(LiveAbort::Budget(_) | LiveAbort::Setup(_))),
            "{profile:?} {budget}: {abort:?}"
        );
    }
    assert_eq!(server.request_count(), 0, "no request");
    assert_eq!(
        stores_under(&root),
        Vec::<String>::new(),
        "no store was created"
    );
}

/// A service whose PUTs create a new revision with a new id: the flow follows the new
/// ids and the teardown deletes every revision listed under the planned names.
#[test]
fn live_write_flow_deletes_every_revision_when_puts_create_new_ids() {
    let Some(ts) = require_ts("live_write_flow_deletes_every_revision_when_puts_create_new_ids")
    else {
        return;
    };
    let (server, sandbox, _) = fixture();
    server.set_put_appends_new_record(true);
    let profiles_before = server.profiles("100");
    let topics_before = server.topics("100");
    let report = run_live_writes(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert!(report.mismatches().is_empty(), "{table}");
    assert_eq!(row(&report, "topics create again (rs)").note, "id changed");
    let resources = report.resources.unwrap_or_default();
    assert_eq!(resources.left, 0, "{table}");
    assert!(resources.created > 3, "{table}");
    assert_eq!(resources.created, resources.deleted, "{table}");
    assert_eq!(server.profiles("100"), profiles_before);
    assert_eq!(server.topics("100"), topics_before);
    assert_ledger_clean(&ledger_lines(&sandbox), &sandbox);
}

// ---------------------------------------------------------------------------------------
// The M5 bulk flow (DESIGN §10, "M5 bulk flow")
// ---------------------------------------------------------------------------------------

/// The bulk flow: an interrupted airs-cli-rs bulk-scan, its recovery by resume-poll
/// without a new submission, and (when its state can be sandboxed) one TypeScript
/// bulk-scan, against "Prisma AIRS Harness" with at most 8 prompts in all. Needs
/// `AIRS_RS_LIVE_BULK=1` and that profile; `AIRS_RS_LIVE_BULK_PROMPTS` (2-8, default 4)
/// sets the prompts per CLI.
#[test]
fn live_bulk() {
    let settings = match BulkGate::from_env() {
        Ok(BulkGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1, AIRS_E2E_TENANT=<name>, AIRS_RS_LIVE_BULK=1 and AIRS_E2E_PROFILE=\"{BULK_PROFILE}\" to run the bulk flow)"
            ));
            return;
        }
        Ok(BulkGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let mut plan = real_plan(&settings.tenant);
    plan.scan_profile = Some(BULK_PROFILE.into());
    let report = match run_live_bulk(&plan, settings.prompts) {
        Ok(report) => report,
        Err(abort) => panic!("live bulk flow stopped: {abort}"),
    };
    for text in &report.notices {
        notice(&format!("LIVE NOTICE: {text}"));
    }
    notice(&report.table());
    write_summary("summary-bulk.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The `req_id`s of every POST whose session id ends with `suffix`.
fn bulk_posts(server: &FixtureServer, suffix: &str) -> Vec<Vec<u64>> {
    server
        .requests_for(Route::ScanAsync)
        .iter()
        .filter_map(|post| post.body_json())
        .filter(|body| {
            body[0]["scan_req"]["session_id"]
                .as_str()
                .is_some_and(|session| session.ends_with(suffix))
        })
        .map(|body| {
            body.as_array()
                .map(|objects| {
                    objects
                        .iter()
                        .filter_map(|object| object["req_id"].as_u64())
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect()
}

/// A fixture for the bulk flow: the injection probes are blocked; `adjust` runs first.
fn bulk_fixture(adjust: impl FnOnce(&FixtureServer)) -> (FixtureServer, Sandbox, FixtureTenant) {
    let (server, sandbox, tenant) = fixture_with(adjust);
    let mut blocked = fixtures::scan_block(BULK_PROFILE);
    blocked["prompt_detected"]["injection"] = json!(true);
    for marker in [INJECTION_PROMPT, BULK_PROMPTS[3]] {
        server.add_async_rule(marker, AsyncRule::verdict(blocked.clone()));
    }
    (server, sandbox, tenant)
}

fn bulk_plan(sandbox: &Sandbox, ts: Cli) -> LivePlan {
    let mut plan = plan(sandbox, ts);
    plan.scan_profile = Some(BULK_PROFILE.into());
    plan
}

/// Branch (a), on the real trigger (`Scan IDs saved:`) and a fixture event: batch 1's
/// first poll is held until the signal, so R1 is always interrupted while its receipt is
/// polled, however late the signal lands. R2 submits only batch 2, R3 sends nothing;
/// every prompt was submitted once by airs-cli-rs; TypeScript runs with its state
/// inside the session sandbox and agrees; nothing outside the sandboxes changed.
#[test]
fn live_bulk_self_test_branch_a_against_fixture() {
    let Some(ts) = require_ts("live_bulk_self_test_branch_a_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = bulk_fixture(|server| {
        server.push_once(Route::ScanResults, Planned::held());
    });
    let held = || server.route_count(Route::ScanResults) >= 1;
    let before = sandbox.snapshot_with(&[&sandbox.home(), &sandbox.cwd()]);
    let report = run_live_bulk_with(
        &bulk_plan(&sandbox, ts),
        DEFAULT_BULK_PROMPTS,
        &Trigger::ScanIdsSavedAnd(&held),
    )
    .unwrap_or_else(|abort| panic!("{abort}"));
    server.release_held();
    let table = report.table();
    assert!(report.mismatches().is_empty(), "{table}");
    let counts = report.bulk.clone().unwrap_or_default();
    assert_eq!(counts.branch, "a", "{table}");
    assert_eq!(counts.prompts_submitted, 4, "{table}");
    assert_eq!(
        counts.async_records,
        [("R1".into(), 1), ("R2".into(), 1), ("R3".into(), 0)],
        "{table}"
    );
    assert!(counts.ts_ran);
    assert_eq!(counts.verdicts_agree, Some(true), "{table}");
    for check in [
        "bulk-scan R1 (interrupted)",
        "resume-poll R2",
        "resume-poll R3 (no-op)",
        "bulk-scan T1 (TypeScript)",
        "bulk verdicts (rs vs ts)",
    ] {
        assert!(row(&report, check).agree, "{check}: {table}");
    }
    assert!(
        row(&report, "bulk-scan R1 (interrupted)")
            .note
            .starts_with("interrupted; branch a")
    );
    // Recovery without resubmission: each prompt in exactly one airs-cli-rs POST.
    assert_eq!(bulk_posts(&server, "-rs"), [vec![0, 1], vec![2, 3]]);
    assert_eq!(bulk_posts(&server, "-ts"), [vec![0, 1], vec![2, 3]]);
    assert_eq!(report.scan_invocations, 8);
    assert_eq!(report.max_scans, 8);
    assert_eq!((report.rust_runs, report.ts_runs), (3, 1));
    assert_eq!(report.store_removed, Some(true));
    sandbox.assert_unchanged(&before);
    assert!(
        !workspace_root()
            .join("Prisma-AIRS-Harness-bulk-scan.csv")
            .exists(),
        "nothing was written into the repository"
    );
    assert_sanitized(&report, &sandbox, &tenant);
}

/// Branch (b), on a fixture event: batch 2's POST is held until the signal, so its
/// chunk is ambiguous; R2 refuses without a submission and the Rust part passes with
/// a note.
#[test]
fn live_bulk_self_test_branch_b_against_fixture() {
    let Some(ts) = require_ts("live_bulk_self_test_branch_b_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = bulk_fixture(|server| {
        server.push_once(Route::ScanAsync, Planned::builtin());
        server.push_once(Route::ScanAsync, Planned::held());
    });
    let held = || server.route_count(Route::ScanAsync) >= 2;
    let report = run_live_bulk_with(
        &bulk_plan(&sandbox, ts),
        DEFAULT_BULK_PROMPTS,
        &Trigger::When(&held),
    )
    .unwrap_or_else(|abort| panic!("{abort}"));
    server.release_held();
    let table = report.table();
    assert!(report.mismatches().is_empty(), "{table}");
    let counts = report.bulk.clone().unwrap_or_default();
    assert_eq!(counts.branch, "b", "{table}");
    assert_eq!(
        counts.async_records,
        [("R1".into(), 2), ("R2".into(), 0)],
        "{table}"
    );
    assert!(row(&report, "resume-poll R2 (ambiguous)").agree, "{table}");
    assert!(
        !report
            .rows
            .iter()
            .any(|row| row.check == "resume-poll R3 (no-op)"),
        "{table}"
    );
    assert_eq!(bulk_posts(&server, "-rs"), [vec![0, 1], vec![2, 3]]);
    assert_eq!(counts.verdicts_agree, None);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// A `dataDir` TypeScript would resolve outside the session sandbox: the TypeScript
/// step is skipped with the notice, so a larger airs-cli-rs budget fits; TypeScript is
/// never started.
#[test]
fn live_bulk_skips_typescript_when_its_state_cannot_be_sandboxed() {
    for data_dir in ["/nonexistent-airs-live-bulk/runs", "relative/runs"] {
        let server = FixtureServer::start();
        let sandbox = Sandbox::new();
        let tenant = sandbox.add_fixture_tenant_with(&server, "prod", "100", |config| {
            config.set("dataDir", data_dir);
        });
        sandbox.register(&[&tenant], Some("prod"));
        let report = run_live_bulk(&bulk_plan(&sandbox, rust()), 6)
            .unwrap_or_else(|abort| panic!("{abort}"));
        let table = report.table();
        assert!(report.mismatches().is_empty(), "{table}");
        assert_eq!(report.notices, [TS_SKIPPED], "{table}");
        assert_eq!(report.ts_runs, 0);
        assert_eq!(report.max_scans, 6);
        assert!(!report.bulk.unwrap_or_default().ts_ran);
        assert!(bulk_posts(&server, "-ts").is_empty());
        assert!(!std::path::Path::new(data_dir).exists() || data_dir.starts_with("relative"));
    }
}

/// The profile and every budget rule are checked before anything is created or sent:
/// a budget out of range, a profile other than the harness one, and a plan whose
/// TypeScript step would take the total over 8.
#[test]
fn live_bulk_refuses_a_bad_budget_or_profile_before_any_request() {
    let (server, sandbox, _) = bulk_fixture(|_| {});
    let root = sandbox.tmp().join("live-stores");
    for (profile, prompts) in [
        (Some(BULK_PROFILE), MIN_BULK_PROMPTS - 1),
        (Some(BULK_PROFILE), MAX_BULK_PROMPTS + 1),
        (Some(BULK_PROFILE), 5),
        (Some(PROFILE), 4),
        (None, 4),
    ] {
        let mut plan = plan(&sandbox, rust());
        plan.scan_profile = profile.map(str::to_owned);
        let abort = run_live_bulk(&plan, prompts).err();
        assert!(
            matches!(abort, Some(LiveAbort::Budget(_) | LiveAbort::Setup(_))),
            "{profile:?} {prompts}: {abort:?}"
        );
    }
    assert_eq!(server.request_count(), 0, "no request");
    assert_eq!(
        stores_under(&root),
        Vec::<String>::new(),
        "no store was created"
    );
}

// ---------------------------------------------------------------------------------------
// M6a: the red team reads, write flow and scan flow (DESIGN §10)
// ---------------------------------------------------------------------------------------

use airs_test_harness::Rt;
use airs_test_harness::fixtures::redteam as rtfx;
use airs_test_harness::live::redteam::run_live_redteam_reads;
use airs_test_harness::live::redteam_scan::{ScanGate, ScanSettings, run_live_redteam_scan};
use airs_test_harness::live::redteam_writes::{redteam_writes_gate, run_live_redteam_writes};

/// The red team reads against the selected tenant (read-only on both CLIs). Needs only
/// `AIRS_RS_LIVE=1` and `AIRS_E2E_TENANT`.
#[test]
fn live_redteam_reads() {
    let settings = match LiveGate::from_env() {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1 and AIRS_E2E_TENANT=<name> for the red team reads)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.tenant);
    let report = run_live_redteam_reads(&plan)
        .unwrap_or_else(|abort| panic!("live red team reads stopped: {abort}"));
    notice(&report.table());
    write_summary("summary-redteam.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The red team write flow: two run-created draft adapters, journalled and deleted.
/// Needs `AIRS_RS_LIVE_REDTEAM_WRITES=1` as well.
#[test]
fn live_redteam_writes() {
    let settings = match redteam_writes_gate(|name| std::env::var(name).ok()) {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1, AIRS_E2E_TENANT=<name> and AIRS_RS_LIVE_REDTEAM_WRITES=1 for the red team write flow)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.tenant);
    let report = run_live_redteam_writes(&plan).unwrap_or_else(|abort| {
        panic!("live red team write flow stopped: {abort} (see target/live/ledger.jsonl; run the sweeper with AIRS_RS_LIVE_CLEANUP=1 after an hour)")
    });
    notice(&report.table());
    write_summary("summary-redteam-writes.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    assert_eq!(
        report.resources.map_or(1, |r| r.left),
        0,
        "live tier: adapters were left behind; see target/live/ledger.jsonl and run the sweeper"
    );
    assert!(
        report
            .resources
            .is_some_and(|r| r.created == r.deleted && r.created > 0),
        "live tier: not every created adapter was deleted ({:?}); see target/live/ledger.jsonl",
        report.resources
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The red team scan flow: one billed STATIC scan of one sub-category against the
/// target the user names. Needs `AIRS_RS_LIVE_REDTEAM_SCAN=1` and
/// `AIRS_E2E_REDTEAM_TARGET=<uuid>`; never run without asking the user.
#[test]
fn live_redteam_scan() {
    let settings = match ScanGate::from_lookup(|name| std::env::var(name).ok()) {
        Ok(ScanGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1, AIRS_E2E_TENANT=<name>, AIRS_RS_LIVE_REDTEAM_SCAN=1 and AIRS_E2E_REDTEAM_TARGET=<target uuid> for the billed red team scan flow)"
            ));
            return;
        }
        Ok(ScanGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let mut plan = real_plan(&settings.live.tenant);
    plan.timeout = std::time::Duration::from_secs(120);
    let report = run_live_redteam_scan(&plan, &settings).unwrap_or_else(|abort| {
        panic!("live red team scan flow stopped: {abort} (see target/live/ledger.jsonl)")
    });
    notice(&report.table());
    write_summary("summary-redteam-scan.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The standard fixture with the red team dataset.
fn rt_fixture() -> (FixtureServer, Sandbox, FixtureTenant) {
    let (server, sandbox, tenant) = fixture();
    server.seed_redteam();
    (server, sandbox, tenant)
}

/// Neither the summary nor the ledger carries a fixture credential marker, a target or
/// adapter name, or an id that is not the run's own.
fn assert_rt_sanitized(report: &LiveReport, sandbox: &Sandbox, tenant: &FixtureTenant) {
    assert_sanitized(report, sandbox, tenant);
    let text = format!(
        "{}\n{}",
        report.table(),
        serde_json::to_string(&report.to_json()).unwrap()
    );
    for marker in rtfx::markers() {
        assert!(!text.contains(&marker), "the summary carries a marker");
    }
    for needle in [
        "Rest Target One",
        "adapter-one",
        "fixture-set",
        "fixture-scan",
        "FAKE-REGISTRY",
    ] {
        assert!(!text.contains(needle), "the summary carries {needle:?}");
    }
}

#[test]
fn live_redteam_reads_self_test_against_fixture() {
    let Some(ts) = require_ts("live_redteam_reads_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = rt_fixture();
    // A completed job exists (job 1), so status and report are compared too.
    let before = sandbox.snapshot();
    let report =
        run_live_redteam_reads(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "redteam categories",
        "redteam eula status",
        "redteam languages",
        "redteam languages --management",
        "redteam properties list",
        "redteam network-broker stats",
        "redteam channels list",
        "redteam adapter list",
        "redteam targets metadata",
        "redteam list --limit 5",
        "redteam targets list",
        "redteam targets list --output table",
        "redteam prompt-sets list",
        "redteam prompt-sets get <first>",
        "redteam prompts list <first> --limit 5",
        "redteam status <completed>",
        "redteam report <completed>",
        "redteam targets get <first> (rs)",
        "redteam targets backup (rs)",
        "redteam report (environment)",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    assert_eq!(report.store_removed, Some(true));
    // TypeScript never read a target's settings nor wrote a backup; nothing was written,
    // and the backup and reports stayed in the run's own sandbox, now removed.
    assert!(!route_seen_by(
        &server,
        Route::RedTeam(Rt::TargetGet),
        "typescript-sdk"
    ));
    assert!(route_seen_by(
        &server,
        Route::RedTeam(Rt::TargetGet),
        "rust-sdk"
    ));
    assert!(
        server
            .requests()
            .iter()
            .all(|request| !request.route.is_write()),
        "read-only"
    );
    assert!(server.rt_requests(Rt::QuotaPost).is_empty());
    sandbox.assert_unchanged(&before);
    assert_rt_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

#[test]
fn live_redteam_reads_agree_on_a_tenant_with_empty_lists() {
    // D146: TypeScript prints `  No adapters found`, `  No prompt sets found`, … in json
    // and table; airs-cli-rs prints `[]` and nothing. The comparisons count that as equal.
    let Some(ts) = require_ts("live_redteam_reads_agree_on_a_tenant_with_empty_lists") else {
        return;
    };
    let (server, sandbox, tenant) = rt_fixture();
    server.redteam(|data| {
        data.adapters.clear();
        data.prompt_sets.clear();
        data.prompts.clear();
        data.targets.clear();
        data.channels.clear();
        data.scans.clear();
    });
    let report =
        run_live_redteam_reads(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "redteam adapter list",
        "redteam prompt-sets list",
        "redteam channels list",
        "redteam list --limit 5",
        "redteam targets list",
        "redteam targets list --output table",
        "redteam targets backup (rs)",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    for skipped in [
        "prompt-sets get and prompts list skipped",
        "targets get skipped",
    ] {
        assert!(
            report.notices.iter().any(|n| n.starts_with(skipped)),
            "{skipped}:\n{table}"
        );
    }
    assert_eq!(report.store_removed, Some(true));
    assert_rt_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

#[test]
fn live_redteam_reads_note_a_tenant_without_red_team_access() {
    let Some(ts) = require_ts("live_redteam_reads_note_a_tenant_without_red_team_access") else {
        return;
    };
    let (server, sandbox, tenant) = rt_fixture();
    server.set_default(
        Route::RedTeam(Rt::Categories),
        Planned::json(403, &json!({})),
    );
    let report =
        run_live_redteam_reads(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    assert!(report.mismatches().is_empty(), "{}", report.table());
    assert!(
        report
            .notices
            .iter()
            .any(|n| n.starts_with("redteam unavailable")),
        "{}",
        report.table()
    );
    assert!(report.rows.is_empty());
    let red_team: Vec<_> = server
        .requests()
        .into_iter()
        .filter(|request| matches!(request.route, Route::RedTeam(_)))
        .collect();
    assert!(
        red_team
            .iter()
            .all(|request| request.route == Route::RedTeam(Rt::Categories)),
        "only the first read (and the SDK's one replay of a 403)"
    );
    assert_rt_sanitized(&report, &sandbox, &tenant);
}

/// The ledger lines of the red team flows (`adapter` and `scan` kinds).
fn rt_ledger(sandbox: &Sandbox) -> Vec<serde_json::Value> {
    ledger_lines(sandbox)
}

#[test]
fn live_redteam_writes_self_test_against_fixture() {
    let Some(ts) = require_ts("live_redteam_writes_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = rt_fixture();
    // Pre-existing adapters, including a prefixed one of an older run and a look-alike.
    server.redteam(|data| {
        let mut old = rtfx::adapter(7, "airs-rs-e2e-1000000000-1-0-adapter-rs", "DRAFT");
        old["uuid"] = json!(rtfx::adapter_id(7));
        data.adapters.push(old);
        let mut precious = rtfx::adapter(8, "airs-rs-e2e-precious", "DRAFT");
        precious["uuid"] = json!(rtfx::adapter_id(8));
        data.adapters.push(precious);
    });
    let before = server.redteam_data().adapters;
    let report =
        run_live_redteam_writes(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    assert_eq!(
        report.resources,
        Some(ResourceCounts {
            created: 2,
            deleted: 2,
            left: 0
        }),
        "{table}"
    );
    assert_eq!(report.store_removed, Some(true));
    assert_eq!(
        server.redteam_data().adapters,
        before,
        "the adapter set is back to its seed"
    );
    for request in server.rt_requests(Rt::AdapterCreate) {
        let body = request.body_json().unwrap();
        let name = body["name"].as_str().unwrap_or_default();
        assert!(
            name.starts_with(RESOURCE_PREFIX) && !name.starts_with("airs-rs-e2e-1000000000"),
            "{name}"
        );
        assert!(
            request
                .query
                .iter()
                .any(|(key, value)| key == "validate" && value == "false"),
            "a draft"
        );
        assert!(body.get("network_broker_channel_uuid").is_none());
    }
    for request in server
        .rt_requests(Rt::AdapterDelete)
        .iter()
        .chain(&server.rt_requests(Rt::AdapterUpdate))
    {
        for old in [7, 8, 1, 2] {
            assert!(
                !request.path.contains(&rtfx::adapter_id(old)),
                "touched {}",
                request.line()
            );
        }
    }
    assert_eq!(
        server.rt_requests(Rt::AdapterDelete).len(),
        2,
        "Rust deletes only"
    );
    assert!(server.rt_requests(Rt::AdapterValidate).is_empty());
    for route in [
        Rt::TargetCreate,
        Rt::TargetProbe,
        Rt::TargetValidateAuth,
        Rt::EulaAccept,
        Rt::ScanCreate,
        Rt::ScanAbort,
        Rt::PromptSetCreate,
        Rt::PropertyCreate,
        Rt::ChannelCreate,
        Rt::RegistryCredentials,
    ] {
        assert!(server.rt_requests(route).is_empty(), "{route:?}");
    }
    let lines = rt_ledger(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    let creates = lines
        .iter()
        .filter(|line| line["op"] == "adapter create" && line["outcome"] == "ok")
        .count();
    let deletes = lines
        .iter()
        .filter(|line| line["op"] == "adapter delete" && line["outcome"] == "ok")
        .count();
    assert_eq!((creates, deletes), (2, 2));
    assert!(lines.iter().all(|line| line["kind"] == "adapter"));
    assert_rt_sanitized(&report, &sandbox, &tenant);
}

/// A teardown listing that does not show the run's adapters (not yet indexed, or a
/// search that ignores names): the ids captured at create time are deleted by id, the
/// miss is a mismatch, and nothing is left behind.
#[test]
fn live_redteam_writes_delete_created_ids_the_listing_misses() {
    let Some(ts) = require_ts("live_redteam_writes_delete_created_ids_the_listing_misses") else {
        return;
    };
    let (server, sandbox, _tenant) = rt_fixture();
    let before = server.redteam_data().adapters;
    // Preflight, teardown and the teardown check all answer an empty listing.
    for _ in 0..3 {
        server.push_once(
            Route::RedTeam(Rt::AdapterList),
            Planned::json(200, &json!({"data": [], "pagination": {"total_items": 0}})),
        );
    }
    let report =
        run_live_redteam_writes(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert_eq!(
        report.resources,
        Some(ResourceCounts {
            created: 2,
            deleted: 2,
            left: 0
        }),
        "{table}"
    );
    assert_eq!(
        server.redteam_data().adapters,
        before,
        "both drafts were deleted by id: {table}"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert_eq!(
        mismatched,
        ["teardown listing shows every created adapter"],
        "{table}"
    );
    assert_eq!(server.rt_requests(Rt::AdapterDelete).len(), 2);
    let lines = rt_ledger(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    let deletes = lines
        .iter()
        .filter(|line| line["op"] == "adapter delete" && line["outcome"] == "ok")
        .count();
    assert_eq!(deletes, 2);
}

#[test]
fn live_redteam_writes_need_their_own_opt_in() {
    let lookup = |name: &str| match name {
        "AIRS_RS_LIVE" => Some("1".to_owned()),
        "AIRS_E2E_TENANT" => Some("prod".to_owned()),
        _ => None,
    };
    let (server, _sandbox, _tenant) = rt_fixture();
    assert!(
        matches!(redteam_writes_gate(lookup), Ok(LiveGate::Skip(reason)) if reason.contains("redteam writes skipped"))
    );
    assert_eq!(
        server.request_count(),
        0,
        "the gate decides before anything runs"
    );
}

#[test]
fn live_cleanup_sweeps_only_old_prefixed_adapters() {
    let (server, sandbox, _) = rt_fixture();
    let now = now_unix();
    server.redteam(|data| {
        for (n, name) in [
            (11, format!("airs-rs-e2e-{}-7-0-adapter-rs", now - 7200)),
            (12, format!("airs-rs-e2e-{}-7-1-adapter-ts", now - 60)),
            (13, "airs-rs-e2e-precious-adapter-rs".to_owned()),
        ] {
            let mut adapter = rtfx::adapter(n, &name, "DRAFT");
            adapter["uuid"] = json!(rtfx::adapter_id(n));
            data.adapters.push(adapter);
        }
    });
    let report =
        run_live_cleanup_at(&plan(&sandbox, rust()), now).unwrap_or_else(|abort| panic!("{abort}"));
    let deletes: Vec<String> = server
        .rt_requests(Rt::AdapterDelete)
        .iter()
        .map(|request| request.line())
        .collect();
    assert_eq!(
        deletes,
        [format!(
            "DELETE /redteam/mgmt/v1/adapters/{}",
            rtfx::adapter_id(11)
        )],
        "{}",
        report.table()
    );
    let left: Vec<String> = server
        .redteam_data()
        .adapters
        .iter()
        .map(|adapter| adapter["uuid"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(left, [1, 2, 12, 13].map(rtfx::adapter_id));
    let lines = ledger_lines(&sandbox);
    assert!(lines.iter().any(|line| line["kind"] == "adapter"
        && line["phase"] == "cleanup"
        && line["outcome"] == "ok"));
}

/// Settings for a loopback scan flow on target 1 (job 2, a foreign RUNNING job, is on
/// the same target and must never be aborted).
fn scan_settings(deadline: std::time::Duration) -> ScanSettings {
    ScanSettings {
        live: airs_test_harness::live::LiveSettings {
            tenant: "prod".into(),
            profile: None,
            max_scans: 0,
        },
        target: rtfx::target_id(1),
        deadline,
        interval: std::time::Duration::from_millis(10),
    }
}

/// The created scan's id (the first record the write routes create).
fn created_scan() -> String {
    fixtures::uuid(181)
}

fn scan_fixture() -> (FixtureServer, Sandbox, FixtureTenant) {
    let (server, sandbox, tenant) = rt_fixture();
    server.redteam(|data| {
        data.static_reports
            .insert(created_scan(), rtfx::static_report());
        data.attacks.insert(created_scan(), rtfx::attacks());
        data.scan_polls
            .push_back(json!({"status": "RUNNING", "completed": 5}));
    });
    (server, sandbox, tenant)
}

#[test]
fn live_redteam_scan_self_test_against_fixture() {
    let Some(ts) = require_ts("live_redteam_scan_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = scan_fixture();
    let settings = scan_settings(std::time::Duration::from_secs(60));
    let report = run_live_redteam_scan(&plan(&sandbox, ts), &settings)
        .unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    let scans = server.rt_requests(Rt::ScanCreate);
    assert_eq!(scans.len(), 1, "one scan, the smallest kind");
    let body = scans[0].body_json().unwrap();
    assert_eq!(body["job_type"], "STATIC");
    assert_eq!(
        body["job_metadata"]["categories"],
        json!({"SECURITY": ["JAILBREAK"]})
    );
    assert!(body["name"].as_str().unwrap().starts_with(RESOURCE_PREFIX));
    assert!(
        !route_seen_by(&server, Route::RedTeam(Rt::ScanCreate), "typescript"),
        "TypeScript never starts a scan"
    );
    assert!(
        server.rt_requests(Rt::ScanAbort).is_empty(),
        "a completed scan is not aborted"
    );
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "redteam scan (rs, one STATIC sub-category)",
        "redteam status <scan>",
        "redteam report <scan>",
        "redteam report <scan> --attacks",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    assert!(
        report
            .notices
            .iter()
            .any(|n| n == "scan final status: COMPLETED"),
        "{table}"
    );
    let lines = ledger_lines(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    assert!(lines.iter().any(|line| line["kind"] == "scan"
        && line["op"] == "redteam scan"
        && line["outcome"] == "ok"));
    assert_rt_sanitized(&report, &sandbox, &tenant);
    assert!(
        !table.contains(&created_scan()),
        "the summary records the status, not the id"
    );
}

#[test]
fn live_redteam_scan_aborts_its_own_scan_at_the_deadline() {
    let (server, sandbox, _) = scan_fixture();
    server.redteam(|data| {
        for _ in 0..10 {
            data.scan_polls.push_back(json!({"status": "RUNNING"}));
        }
    });
    let report = run_live_redteam_scan(
        &plan(&sandbox, rust()),
        &scan_settings(std::time::Duration::ZERO),
    )
    .unwrap_or_else(|abort| panic!("{abort}"));
    let aborts: Vec<String> = server
        .rt_requests(Rt::ScanAbort)
        .iter()
        .map(|request| request.path.clone())
        .collect();
    assert_eq!(
        aborts,
        [format!("/redteam/data/v1/scan/{}/abort", created_scan())],
        "{}",
        report.table()
    );
    assert!(
        !aborts.iter().any(|path| path.contains(&rtfx::job_id(2))),
        "the foreign RUNNING job is never aborted"
    );
    assert!(
        report
            .notices
            .iter()
            .any(|n| n == "after the abort the scan reads ABORTED"),
        "{}",
        report.table()
    );
    let lines = ledger_lines(&sandbox);
    assert!(lines.iter().any(|line| line["op"] == "redteam abort"
        && line["phase"] == "teardown"
        && line["outcome"] == "ok"));
}

#[test]
fn live_redteam_scan_resolves_an_unknown_start_by_its_name() {
    let (server, sandbox, _) = scan_fixture();
    server.push_once(
        Route::RedTeam(Rt::ScanCreate),
        Planned::applied_then_dropped(),
    );
    for _ in 0..10 {
        server.redteam(|data| data.scan_polls.push_back(json!({"status": "RUNNING"})));
    }
    let report = run_live_redteam_scan(
        &plan(&sandbox, rust()),
        &scan_settings(std::time::Duration::ZERO),
    )
    .unwrap_or_else(|abort| panic!("{abort}"));
    assert_eq!(
        server.rt_requests(Rt::ScanCreate).len(),
        1,
        "never a second scan"
    );
    assert!(
        report
            .notices
            .iter()
            .any(|n| n.contains("found by its exact name")),
        "{}",
        report.table()
    );
    let lines = ledger_lines(&sandbox);
    assert!(
        lines
            .iter()
            .any(|line| line["op"] == "redteam scan" && line["outcome"] == "unknown")
    );
    let aborts: Vec<String> = server
        .rt_requests(Rt::ScanAbort)
        .iter()
        .map(|request| request.path.clone())
        .collect();
    assert_eq!(
        aborts,
        [format!("/redteam/data/v1/scan/{}/abort", created_scan())]
    );
    assert_eq!(
        report.mismatches().len(),
        1,
        "the unknown start is a mismatch"
    );
}

/// An unknown start whose first listing does not show the scan yet (not indexed, or a
/// failing list): the lookup is retried, the scan found and aborted at the deadline.
#[test]
fn live_redteam_scan_retries_the_lookup_of_an_unknown_start() {
    let (server, sandbox, _) = scan_fixture();
    server.push_once(
        Route::RedTeam(Rt::ScanCreate),
        Planned::applied_then_dropped(),
    );
    server.push_once(
        Route::RedTeam(Rt::ScanList),
        Planned::json(200, &json!({"data": [], "pagination": {"total_items": 0}})),
    );
    server.push_once(Route::RedTeam(Rt::ScanList), Planned::json(500, &json!({})));
    for _ in 0..10 {
        server.redteam(|data| data.scan_polls.push_back(json!({"status": "RUNNING"})));
    }
    let report = run_live_redteam_scan(
        &plan(&sandbox, rust()),
        &scan_settings(std::time::Duration::ZERO),
    )
    .unwrap_or_else(|abort| panic!("{abort}"));
    assert_eq!(
        server.rt_requests(Rt::ScanCreate).len(),
        1,
        "never a second scan"
    );
    assert!(
        server.rt_requests(Rt::ScanList).len() >= 3,
        "an empty and a failing listing were retried"
    );
    let aborts: Vec<String> = server
        .rt_requests(Rt::ScanAbort)
        .iter()
        .map(|request| request.path.clone())
        .collect();
    assert_eq!(
        aborts,
        [format!("/redteam/data/v1/scan/{}/abort", created_scan())],
        "{}",
        report.table()
    );
}

/// An unknown start that no listing ever shows fails the run with an explicit
/// "abort it manually" message; nothing else is sent (no second scan, no abort of a
/// scan the run cannot name).
#[test]
fn live_redteam_scan_fails_loudly_when_an_unknown_start_stays_unresolved() {
    let (server, sandbox, _) = scan_fixture();
    server.push_once(
        Route::RedTeam(Rt::ScanCreate),
        Planned::applied_then_dropped(),
    );
    server.set_default(
        Route::RedTeam(Rt::ScanList),
        Planned::json(200, &json!({"data": [], "pagination": {"total_items": 0}})),
    );
    let abort = run_live_redteam_scan(
        &plan(&sandbox, rust()),
        &scan_settings(std::time::Duration::ZERO),
    )
    .expect_err("an unresolved billed scan fails the run");
    let text = abort.to_string();
    assert!(
        text.contains("a billed scan named airs-rs-e2e-") && text.contains("abort it manually"),
        "{text}"
    );
    assert_eq!(server.rt_requests(Rt::ScanCreate).len(), 1);
    assert_eq!(
        server.rt_requests(Rt::ScanList).len(),
        airs_test_harness::live::redteam_scan::RESOLVE_ATTEMPTS as usize,
        "bounded retries"
    );
    assert!(server.rt_requests(Rt::ScanAbort).is_empty());
    let lines = ledger_lines(&sandbox);
    assert!(
        lines
            .iter()
            .any(|line| line["op"] == "redteam scan" && line["outcome"] == "unknown")
    );
}

#[test]
fn live_redteam_scan_gates_refuse_before_any_request() {
    let (server, _sandbox, _) = scan_fixture();
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    };
    static NO_OPT_IN: [(&str, &str); 2] = [("AIRS_RS_LIVE", "1"), ("AIRS_E2E_TENANT", "prod")];
    assert!(matches!(
        ScanGate::from_lookup(env(&NO_OPT_IN)),
        Ok(ScanGate::Skip(_))
    ));
    static NO_TARGET: [(&str, &str); 3] = [
        ("AIRS_RS_LIVE", "1"),
        ("AIRS_E2E_TENANT", "prod"),
        ("AIRS_RS_LIVE_REDTEAM_SCAN", "1"),
    ];
    assert!(ScanGate::from_lookup(env(&NO_TARGET)).is_err());
    static BAD_TARGET: [(&str, &str); 4] = [
        ("AIRS_RS_LIVE", "1"),
        ("AIRS_E2E_TENANT", "prod"),
        ("AIRS_RS_LIVE_REDTEAM_SCAN", "1"),
        ("AIRS_E2E_REDTEAM_TARGET", "target-1"),
    ];
    assert!(ScanGate::from_lookup(env(&BAD_TARGET)).is_err());
    static BAD_MINUTES: [(&str, &str); 5] = [
        ("AIRS_RS_LIVE", "1"),
        ("AIRS_E2E_TENANT", "prod"),
        ("AIRS_RS_LIVE_REDTEAM_SCAN", "1"),
        (
            "AIRS_E2E_REDTEAM_TARGET",
            "00000000-0000-4000-8000-000000000201",
        ),
        ("AIRS_RS_LIVE_REDTEAM_SCAN_MINUTES", "90"),
    ];
    assert!(ScanGate::from_lookup(env(&BAD_MINUTES)).is_err());
    assert_eq!(server.request_count(), 0);
}

// ---------------------------------------------------------------------------------------
// M6b: the DLP reads, the DLP write flow and the sweeper's DLP kinds (DESIGN §10)
// ---------------------------------------------------------------------------------------

use airs_test_harness::fixtures::dlp as dlpfx;
use airs_test_harness::live::dlp::run_live_dlp_reads;
use airs_test_harness::live::dlp_writes::{DlpWriteSettings, dlp_writes_gate, run_live_dlp_writes};
use airs_test_harness::{Collection, Dl};

/// The DLP reads against the selected tenant (read-only on both CLIs). Needs only
/// `AIRS_RS_LIVE=1` and `AIRS_E2E_TENANT`.
#[test]
fn live_dlp_reads() {
    let settings = match LiveGate::from_env() {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1 and AIRS_E2E_TENANT=<name> for the DLP reads)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.tenant);
    let report =
        run_live_dlp_reads(&plan).unwrap_or_else(|abort| panic!("live DLP reads stopped: {abort}"));
    notice(&report.table());
    write_summary("summary-dlp.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The DLP write flow: run-created custom patterns and dictionaries only (at most six,
/// journalled, deleted at the end). Needs `AIRS_RS_LIVE_DLP_WRITES=1` as well;
/// `AIRS_E2E_DLP_REGION` names the dictionary region when no dictionary lists one.
#[test]
fn live_dlp_writes() {
    let settings = match dlp_writes_gate(|name| std::env::var(name).ok()) {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1, AIRS_E2E_TENANT=<name> and AIRS_RS_LIVE_DLP_WRITES=1 for the DLP write flow)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.tenant);
    let write_settings = DlpWriteSettings {
        region: std::env::var("AIRS_E2E_DLP_REGION")
            .ok()
            .filter(|region| !region.trim().is_empty()),
    };
    let report = run_live_dlp_writes(&plan, &write_settings).unwrap_or_else(|abort| {
        panic!("live DLP write flow stopped: {abort} (see target/live/ledger.jsonl; run the sweeper with AIRS_RS_LIVE_CLEANUP=1 after an hour)")
    });
    if report
        .notices
        .iter()
        .any(|text| text.starts_with("no DLP region known"))
    {
        notice("LIVE NOTICE: no DLP region known; dictionary writes skipped");
    }
    notice(&report.table());
    write_summary("summary-dlp-writes.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    assert_eq!(
        report.resources.map_or(1, |r| r.left),
        0,
        "live tier: DLP records were left behind; see target/live/ledger.jsonl and run the sweeper"
    );
    assert!(
        report
            .resources
            .is_some_and(|r| r.created == r.deleted && r.created > 0),
        "live tier: not every created DLP record was deleted ({:?}); see target/live/ledger.jsonl",
        report.resources
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The standard fixture with the DLP dataset.
fn dlp_fixture() -> (FixtureServer, Sandbox, FixtureTenant) {
    let (server, sandbox, tenant) = fixture();
    server.seed_dlp();
    (server, sandbox, tenant)
}

/// Neither the summary nor the ledger carries a DLP content marker, a seeded resource
/// name, a region or a TSG.
fn assert_dlp_sanitized(report: &LiveReport, sandbox: &Sandbox, tenant: &FixtureTenant) {
    assert_sanitized(report, sandbox, tenant);
    let text = format!(
        "{}\n{}",
        report.table(),
        serde_json::to_string(&report.to_json()).unwrap()
    );
    for marker in dlpfx::markers() {
        assert!(!text.contains(&marker), "the summary carries a marker");
    }
    for needle in [
        "My Pattern",
        "My Dict",
        "My Profile",
        "Credit Card Number",
        "United States",
        "airs-rs-e2e-kw",
        "\"100\"",
    ] {
        assert!(!text.contains(needle), "the summary carries {needle:?}");
    }
}

#[test]
fn live_dlp_reads_self_test_against_fixture() {
    let Some(ts) = require_ts("live_dlp_reads_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = dlp_fixture();
    let before = sandbox.snapshot();
    let report = run_live_dlp_reads(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "dlp patterns list --limit 1 (rs)",
        "dlp filtering-profiles list --limit 50",
        "dlp patterns list --include-predefined --limit 50",
        "dlp profiles list --include-predefined --limit 50",
        "dlp dictionaries list --include-predefined --limit 50",
        "dlp patterns list",
        "dlp profiles list",
        "dlp dictionaries list",
        "dlp filtering-profiles get <first>",
        "dlp patterns get <first>",
        "dlp profiles get <first>",
        "dlp dictionaries get <first>",
        "dlp backup",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    assert_eq!(report.store_removed, Some(true));
    assert!(
        server
            .requests()
            .iter()
            .all(|request| !request.route.is_write()),
        "read-only"
    );
    // No keyword list was asked for by a read (the backups read them per dictionary or
    // in their own listing).
    for request in server.dl_requests(Dl::Get(Collection::Dictionaries)) {
        let ua = request.header("user-agent").unwrap_or_default().to_owned();
        assert!(
            request.query_value("keywords").is_none() || ua.contains("rust-sdk"),
            "{}",
            request.line()
        );
    }
    sandbox.assert_unchanged(&before);
    assert_dlp_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

#[test]
fn live_dlp_reads_note_a_tenant_without_dlp_access() {
    let Some(ts) = require_ts("live_dlp_reads_note_a_tenant_without_dlp_access") else {
        return;
    };
    let (server, sandbox, tenant) = dlp_fixture();
    server.set_default(
        Route::Dlp(Dl::List(Collection::Patterns)),
        Planned::json(403, &json!({})),
    );
    let report = run_live_dlp_reads(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    assert!(report.mismatches().is_empty(), "{}", report.table());
    assert!(
        report
            .notices
            .iter()
            .any(|n| n.starts_with("dlp unavailable")),
        "{}",
        report.table()
    );
    assert!(report.rows.is_empty());
    assert!(
        server
            .requests()
            .iter()
            .filter(|request| matches!(request.route, Route::Dlp(_)))
            .all(|request| request.route == Route::Dlp(Dl::List(Collection::Patterns))),
        "only the first read"
    );
    assert_dlp_sanitized(&report, &sandbox, &tenant);
}

/// The ids of the seeded DLP records (never written by the flow).
const SEEDED_DLP_IDS: [&str; 7] = [
    "p-del-1", "p-pre-1", "p-dis-1", "p-cus-1", "p-par-1", "d-cus-1", "d-pre-1",
];

/// The DLP write rules on the request log: run-prefixed create names, no filtering
/// profile PUT, no data-profile write, no write to a seeded id.
fn assert_dlp_writes_owned(server: &FixtureServer) {
    for request in server.requests() {
        if !request.route.is_write() {
            continue;
        }
        let line = request.line();
        assert!(!line.contains("data-filtering-profiles"), "{line}");
        assert!(!line.contains("data-profiles"), "{line}");
        for id in SEEDED_DLP_IDS {
            assert!(!request.path.ends_with(&format!("/{id}")), "{line}");
        }
        if let Route::Dlp(Dl::Create(collection)) = request.route {
            let name = match airs_test_harness::multipart(&request) {
                Some(parts) => parts
                    .part("json")
                    .and_then(|part| part.json.clone())
                    .unwrap_or_default()["name"]
                    .clone(),
                None => request.body_json().unwrap_or_default()["name"].clone(),
            };
            let name = name.as_str().unwrap_or_default().to_owned();
            assert!(name.starts_with(RESOURCE_PREFIX), "{collection:?}: {name}");
        }
    }
}

/// The patterns and dictionaries are back to their seed, apart from the run's own
/// soft-deleted patterns.
fn assert_dlp_seed_restored(server: &FixtureServer) {
    let data = server.dlp_data();
    let seed = airs_test_harness::DlpData::seeded();
    let patterns: Vec<&serde_json::Value> = data
        .patterns
        .iter()
        .filter(|pattern| {
            let name = pattern["name"].as_str().unwrap_or_default();
            if name.starts_with(RESOURCE_PREFIX) {
                assert_eq!(pattern["status"], "deleted", "{name}");
                false
            } else {
                true
            }
        })
        .collect();
    assert_eq!(patterns, seed.patterns.iter().collect::<Vec<_>>());
    assert_eq!(data.dictionaries, seed.dictionaries);
    assert_eq!(data.profiles, seed.profiles);
}

#[test]
fn live_dlp_writes_self_test_against_fixture() {
    let Some(ts) = require_ts("live_dlp_writes_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = dlp_fixture();
    let before = sandbox.snapshot();
    let report = run_live_dlp_writes(&plan(&sandbox, ts), &DlpWriteSettings::default())
        .unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    assert_eq!(
        report.resources,
        Some(ResourceCounts {
            created: 6,
            deleted: 6,
            left: 0
        }),
        "{table}"
    );
    assert_eq!(report.store_removed, Some(true));
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "dlp patterns create (rs)",
        "dlp patterns create (ts)",
        "dlp patterns get (rs pattern)",
        "dlp patterns patch (ts)",
        "dlp patterns replace (rs)",
        "dlp patterns get after replace (ts)",
        "dlp dictionaries create (ts)",
        "dlp dictionaries get --keywords (rs dictionary)",
        "dlp dictionaries replace (rs)",
        "dlp restore --dry-run (rs)",
        "dlp restore (rs)",
        "teardown every created DLP record reads back deleted",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    assert_dlp_writes_owned(&server);
    assert_dlp_seed_restored(&server);
    let restored: Vec<String> = server
        .dl_requests(Dl::Create(Collection::Patterns))
        .iter()
        .map(|request| {
            request.body_json().unwrap()["name"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(restored.len(), 3);
    assert!(restored[2].ends_with("-pattern-rr"), "{restored:?}");
    let lines = ledger_lines(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    let count = |op: &str, outcome: &str| {
        lines
            .iter()
            .filter(|line| line["op"] == op && line["outcome"] == outcome)
            .count()
    };
    assert_eq!(
        count("patterns create", "ok") + count("dictionaries create", "ok"),
        4
    );
    assert_eq!(count("dlp restore", "ok"), 2);
    assert_eq!(
        count("patterns delete", "ok") + count("dictionaries delete", "ok"),
        6
    );
    assert!(
        lines
            .iter()
            .all(|line| matches!(line["kind"].as_str(), Some("pattern" | "dictionary")))
    );
    sandbox.assert_unchanged(&before);
    assert_dlp_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

/// An injected 500 on a patch: the flow reports it and still tears everything down.
#[test]
fn live_dlp_writes_tear_down_after_a_failed_patch() {
    let Some(ts) = require_ts("live_dlp_writes_tear_down_after_a_failed_patch") else {
        return;
    };
    let (server, sandbox, _tenant) = dlp_fixture();
    server.push_once(
        Route::Dlp(Dl::Patch(Collection::Patterns)),
        Planned::json(500, &json!({"title": "boom"})),
    );
    let report = run_live_dlp_writes(&plan(&sandbox, ts), &DlpWriteSettings::default())
        .unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert_eq!(
        mismatched,
        [
            "dlp patterns patch (rs)",
            "dlp patterns get after patch (rs)"
        ],
        "{table}"
    );
    let resources = report.resources.unwrap();
    assert_eq!((resources.created, resources.left), (6, 0), "{table}");
    assert_dlp_seed_restored(&server);
    assert_ledger_clean(&ledger_lines(&sandbox), &sandbox);
}

/// Every pattern create answered 500: TypeScript exits 2 (as for any failure of its
/// pattern writes) and airs-cli-rs reports an unknown outcome; the ledger journals both
/// `unknown`, never `failed` (a 5xx may have been applied), and the flow still tears
/// down what it created.
#[test]
fn live_dlp_writes_journal_a_5xx_create_as_unknown() {
    let Some(ts) = require_ts("live_dlp_writes_journal_a_5xx_create_as_unknown") else {
        return;
    };
    let (server, sandbox, _tenant) = dlp_fixture();
    server.set_default(
        Route::Dlp(Dl::Create(Collection::Patterns)),
        Planned::json(500, &json!({"title": "boom"})).with_retry_after(0),
    );
    let report = run_live_dlp_writes(&plan(&sandbox, ts), &DlpWriteSettings::default())
        .unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    let lines = ledger_lines(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    let creates: Vec<(&str, &str)> = lines
        .iter()
        .filter(|line| line["op"] == "patterns create" && line["outcome"] != "pending")
        .map(|line| {
            (
                line["cli"].as_str().unwrap_or_default(),
                line["outcome"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(creates, [("rs", "unknown"), ("ts", "unknown")], "{table}");
    assert!(
        lines.iter().any(|line| line["op"] == "patterns create"
            && line["cli"] == "ts"
            && line["exit"] == 2),
        "TypeScript's exit 2: {table}"
    );
    let resources = report.resources.unwrap();
    assert_eq!(resources.left, 0, "{table}");
    assert!(
        !server
            .dl_requests(Dl::Create(Collection::Patterns))
            .is_empty()
    );
    assert_dlp_writes_owned(&server);
}

/// A dry run that does not plan exactly the two creates (here: an inconsistent
/// destination inventory) ends the restore step with a mismatch and writes nothing.
#[test]
fn live_dlp_writes_restore_refuses_another_plan() {
    let Some(ts) = require_ts("live_dlp_writes_restore_refuses_another_plan") else {
        return;
    };
    let (server, sandbox, _tenant) = dlp_fixture();
    // The dictionaries lists of the flow: preflight, the region, the backup, the dry run.
    for _ in 0..3 {
        server.push_once(
            Route::Dlp(Dl::List(Collection::Dictionaries)),
            Planned::builtin(),
        );
    }
    server.push_once(
        Route::Dlp(Dl::List(Collection::Dictionaries)),
        Planned::json(200, &json!({"content": [], "totalPages": 5})),
    );
    let report = run_live_dlp_writes(&plan(&sandbox, ts), &DlpWriteSettings::default())
        .unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert_eq!(mismatched, ["dlp restore --dry-run (rs)"], "{table}");
    assert!(
        !report
            .rows
            .iter()
            .any(|row| row.check == "dlp restore (rs)"),
        "{table}"
    );
    assert_eq!(
        report.resources,
        Some(ResourceCounts {
            created: 4,
            deleted: 4,
            left: 0
        }),
        "{table}"
    );
    for request in server.dl_requests(Dl::Create(Collection::Patterns)) {
        let name = request.body_json().unwrap()["name"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(!name.ends_with("-rr"), "{name}");
    }
    assert_dlp_seed_restored(&server);
}

#[test]
fn live_dlp_writes_need_their_own_opt_in() {
    let lookup = |name: &str| match name {
        "AIRS_RS_LIVE" => Some("1".to_owned()),
        "AIRS_E2E_TENANT" => Some("prod".to_owned()),
        _ => None,
    };
    let (server, _sandbox, _tenant) = dlp_fixture();
    assert!(
        matches!(dlp_writes_gate(lookup), Ok(LiveGate::Skip(reason)) if reason.contains("dlp writes skipped"))
    );
    assert_eq!(
        server.request_count(),
        0,
        "the gate decides before anything runs"
    );
}

#[test]
fn live_cleanup_sweeps_only_old_prefixed_dlp_records() {
    let (server, sandbox, _) = dlp_fixture();
    let now = now_unix();
    let old = |kind: &str, cli: &str| format!("airs-rs-e2e-{}-7-0-{kind}-{cli}", now - 7200);
    server.dlp(|data| {
        let pattern = |id: &str, name: &str, status: &str| {
            dlpfx::pattern(id, name, "custom", status, "regex", 1, None)
        };
        data.patterns
            .push(pattern("p-old-1", &old("pattern", "rr"), "active"));
        data.patterns
            .push(pattern("p-gone-1", &old("pattern", "rs"), "deleted"));
        data.patterns.push(pattern(
            "p-young-1",
            &format!("airs-rs-e2e-{}-7-1-pattern-ts", now - 60),
            "active",
        ));
        data.patterns.push(pattern(
            "p-foreign-1",
            "airs-rs-e2e-precious-pattern-rs",
            "active",
        ));
        data.dictionaries.push(dlpfx::dictionary(
            "d-old-1",
            &old("dictionary", "ts"),
            "custom",
            "active",
            1,
            None,
        ));
    });
    let report =
        run_live_cleanup_at(&plan(&sandbox, rust()), now).unwrap_or_else(|abort| panic!("{abort}"));
    let deletes: Vec<String> = server
        .requests()
        .iter()
        .filter(|request| matches!(request.route, Route::Dlp(Dl::Delete(_))))
        .map(|request| request.line())
        .collect();
    assert_eq!(
        deletes,
        [
            "DELETE /dlp/v2/api/data-patterns/p-old-1".to_owned(),
            "DELETE /dlp/v2/api/dictionaries/d-old-1".to_owned()
        ],
        "{}",
        report.table()
    );
    let data = server.dlp_data();
    for id in ["p-gone-1", "p-young-1", "p-foreign-1"] {
        let record = data.patterns.iter().find(|p| p["id"] == id).unwrap();
        assert_ne!(record["name"], "", "{id} kept");
    }
    assert_eq!(
        data.patterns
            .iter()
            .find(|p| p["id"] == "p-young-1")
            .unwrap()["status"],
        "active"
    );
    let lines = ledger_lines(&sandbox);
    assert!(lines.iter().any(|line| line["kind"] == "pattern"
        && line["phase"] == "cleanup"
        && line["outcome"] == "ok"));
    assert_ledger_clean(&lines, &sandbox);
}

// ---------------------------------------------------------------------------------------
// M6c: model security (DESIGN §10)
// ---------------------------------------------------------------------------------------

use airs_test_harness::fixtures::modelsec as msfx;
use airs_test_harness::live::modelsec::run_live_modelsec_reads;
use airs_test_harness::live::modelsec_scan::{MsScanGate, MsScanSettings, run_live_modelsec_scan};
use airs_test_harness::live::modelsec_writes::{
    MsWriteSettings, modelsec_writes_gate, run_live_modelsec_writes,
};
use airs_test_harness::{GroupDeleteMode, Ms};

/// The model-security reads against the selected tenant (read-only on both CLIs), with
/// the Rust-only `pypi-auth` check. Needs only `AIRS_RS_LIVE=1` and `AIRS_E2E_TENANT`.
#[test]
fn live_modelsec_reads() {
    let settings = match LiveGate::from_env() {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1 and AIRS_E2E_TENANT=<name> for the model-security reads)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.tenant);
    let report = run_live_modelsec_reads(&plan)
        .unwrap_or_else(|abort| panic!("live model-security reads stopped: {abort}"));
    notice(&report.table());
    write_summary("summary-modelsec.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The model-security write flow: run-created security groups only (one per CLI,
/// journalled, deleted at the end). Needs `AIRS_RS_LIVE_MS_WRITES=1` as well.
#[test]
fn live_modelsec_writes() {
    let settings = match modelsec_writes_gate(|name| std::env::var(name).ok()) {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1, AIRS_E2E_TENANT=<name> and AIRS_RS_LIVE_MS_WRITES=1 for the model-security write flow)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.tenant);
    let report = run_live_modelsec_writes(&plan, &MsWriteSettings::default()).unwrap_or_else(|abort| {
        panic!("live model-security write flow stopped: {abort} (see target/live/ledger.jsonl; run the sweeper with AIRS_RS_LIVE_CLEANUP=1 after an hour)")
    });
    notice(&report.table());
    write_summary("summary-modelsec-writes.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    let unavailable = report
        .notices
        .iter()
        .any(|text| text.starts_with("model-security unavailable"));
    if !unavailable {
        assert_eq!(
            report.resources.map_or(1, |r| r.left),
            0,
            "live tier: groups were left behind; see target/live/ledger.jsonl and run the sweeper"
        );
        assert!(
            report
                .resources
                .is_some_and(|r| r.created == r.deleted && r.created > 0),
            "live tier: not every created group was confirmed deleted ({:?})",
            report.resources
        );
    }
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The billed model-security scan flow (budget 1, Rust creates the scan, labels only on
/// it). Needs `AIRS_RS_LIVE_MS_WRITES=1`, `AIRS_RS_LIVE_MS_SCAN=1` and
/// `AIRS_E2E_MS_MODEL_URI=<uri>` as well; the
/// orchestrator asks the user first.
#[test]
fn live_modelsec_scan() {
    let settings = match MsScanGate::from_lookup(|name| std::env::var(name).ok()) {
        Ok(MsScanGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1, AIRS_E2E_TENANT=<name>, AIRS_RS_LIVE_MS_WRITES=1, AIRS_RS_LIVE_MS_SCAN=1 and AIRS_E2E_MS_MODEL_URI=<uri> for the billed model-security scan flow)"
            ));
            return;
        }
        Ok(MsScanGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.live.tenant);
    let report = run_live_modelsec_scan(&plan, &settings).unwrap_or_else(|abort| {
        panic!("live model-security scan flow stopped: {abort} (see target/live/ledger.jsonl)")
    });
    notice(&report.table());
    write_summary("summary-modelsec-scan.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    assert_eq!(
        report.resources.map_or(0, |r| r.left),
        0,
        "live tier: the scan flow's group was left behind (a scan may reference it); see target/live/ledger.jsonl and run the sweeper after an hour"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The standard fixture with the model-security dataset (the live service's behaviour:
/// created records are timestamped now, the group list hides tombstones, a new group
/// holds one rule instance).
fn ms_fixture() -> (FixtureServer, Sandbox, FixtureTenant) {
    let (server, sandbox, tenant) = fixture();
    server.seed_modelsec();
    server.modelsec(|data| {
        data.created_at = Some("now".into());
        data.list_hides_tombstones = true;
        data.instances_for_created_groups = true;
    });
    (server, sandbox, tenant)
}

/// Neither the summary nor the ledger carries a model-security marker, the PyPI
/// credential or host, a seeded name, a URI or a TSG.
fn assert_ms_sanitized(report: &LiveReport, sandbox: &Sandbox, tenant: &FixtureTenant) {
    assert_sanitized(report, sandbox, tenant);
    let text = format!(
        "{}\n{}",
        report.table(),
        serde_json::to_string(&report.to_json()).unwrap()
    );
    for needle in [
        "FAKE-PYPI-SECRET",
        "FAKE-MS-SECRET",
        msfx::PYPI_HOST,
        "oauth2accesstoken",
        "Default LOCAL",
        "hf://",
        "11111111-1111-4111-8111",
        "\"100\"",
    ] {
        assert!(
            !text.contains(needle),
            "the summary carries {needle:?}:\n{text}"
        );
    }
}

fn ms_plan(sandbox: &Sandbox, ts: Cli) -> LivePlan {
    plan(sandbox, ts)
}

/// The seeded groups the flows must never write to.
const SEEDED_GROUPS: [u64; 2] = [1, 2];

/// The write rules on the request log: run-prefixed group names, no write to a seeded
/// group, rule-instance updates only inside the run's groups, no label write outside the
/// run's scan, at most `scans` billed `POST /v1/scans`.
fn assert_ms_writes_owned(server: &FixtureServer, scans: usize) {
    let created: Vec<String> = (101..=110).map(msfx::uuid).collect();
    let run_scans: Vec<String> = (451..=460).map(msfx::uuid).collect();
    for request in server.requests() {
        if !request.route.is_write() {
            continue;
        }
        let line = request.line();
        for n in SEEDED_GROUPS {
            assert!(!request.path.contains(&msfx::uuid(n)), "{line}");
        }
        match request.route {
            Route::ModelSecurity(Ms::GroupCreate) => {
                let name = request.body_json().unwrap_or_default()["name"].clone();
                let name = name.as_str().unwrap_or_default().to_owned();
                assert!(name.starts_with(RESOURCE_PREFIX), "{name}");
            }
            Route::ModelSecurity(Ms::RuleInstanceUpdate) => {
                assert!(
                    created.iter().any(|id| request.path.contains(id.as_str())),
                    "{line}"
                );
            }
            Route::ModelSecurity(Ms::LabelsAdd | Ms::LabelsSet | Ms::LabelsDelete) => {
                assert!(
                    run_scans
                        .iter()
                        .any(|id| request.path.contains(id.as_str())),
                    "{line}"
                );
            }
            _ => {}
        }
    }
    assert!(server.ms_requests(Ms::ScanCreate).len() <= scans);
}

#[test]
fn live_modelsec_reads_self_test_against_fixture() {
    let Some(ts) = require_ts("live_modelsec_reads_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = ms_fixture();
    let before = sandbox.snapshot();
    let report =
        run_live_modelsec_reads(&ms_plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "model-security groups list --limit 1 (rs)",
        "model-security groups list --limit 20",
        "model-security rules list --limit 20",
        "model-security scans list --limit 20",
        "model-security models list --limit 20",
        "model-security groups get <first>",
        "model-security rules get <first>",
        "model-security scans get <first>",
        "model-security models get <first>",
        "model-security rule-instances list <first group> --limit 20",
        "model-security rule-instances get <first group> <first instance>",
        "model-security models versions <first model> --limit 20",
        "model-security models version <first version>",
        "model-security models files <first version> --limit 20",
        "model-security scans evaluations <first scan> --limit 20",
        "model-security scans violations <first scan> --limit 20",
        "model-security scans files <first scan> --limit 20",
        "model-security labels keys",
        "model-security labels values <first key>",
        "model-security groups list --limit 20 (table)",
        "model-security scans list --limit 20 (table)",
        "model-security pypi-auth (rs: json, pretty, table, --url-file, --debug)",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    assert!(
        report
            .notices
            .iter()
            .any(|n| n == "pypi-auth credential withheld: true (formats: json, pretty, table)"),
        "{table}"
    );
    assert_eq!(report.store_removed, Some(true));
    assert!(
        server
            .requests()
            .iter()
            .all(|request| !request.route.is_write()),
        "read-only"
    );
    // The TypeScript `pypi-auth` never ran (only the Rust SDK asked for the URL).
    let pypi = server.ms_requests(Ms::PypiAuth);
    assert_eq!(
        pypi.len(),
        4,
        "json, pretty and table, each with its own --url-file, and pretty without one"
    );
    assert!(pypi.iter().all(|request| {
        !request
            .header("user-agent")
            .unwrap_or_default()
            .contains("typescript")
    }));
    // The URL file and its debug log are gone with the run.
    assert!(!sandbox.tmp().join("live-stores").join("pypi").exists());
    sandbox.assert_unchanged(&before);
    assert_ms_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

#[test]
fn live_modelsec_reads_note_a_tenant_without_access() {
    let Some(ts) = require_ts("live_modelsec_reads_note_a_tenant_without_access") else {
        return;
    };
    let (server, sandbox, tenant) = ms_fixture();
    server.set_default(
        Route::ModelSecurity(Ms::GroupList),
        Planned::json(403, &json!({})),
    );
    let report =
        run_live_modelsec_reads(&ms_plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    assert!(report.mismatches().is_empty(), "{}", report.table());
    assert!(
        report
            .notices
            .iter()
            .any(|n| n.starts_with("model-security unavailable")),
        "{}",
        report.table()
    );
    assert!(report.rows.is_empty());
    assert!(
        server
            .requests()
            .iter()
            .filter(|request| matches!(request.route, Route::ModelSecurity(_)))
            .all(|request| request.route == Route::ModelSecurity(Ms::GroupList)),
        "only the first read"
    );
    assert_ms_sanitized(&report, &sandbox, &tenant);
}

/// The live tier refuses the TypeScript `pypi-auth` and any `install` outright.
#[test]
fn live_runs_refuse_the_credential_printing_commands() {
    let rust_cli = rust();
    assert!(
        airs_test_harness::live::model_security_refusal(&rust_cli, &["model-security", "install"])
            .is_some()
    );
    assert!(
        airs_test_harness::live::model_security_refusal(
            &rust_cli,
            &["--quiet", "model-security", "install", "--dry-run"]
        )
        .is_some()
    );
    assert!(
        airs_test_harness::live::model_security_refusal(
            &rust_cli,
            &["model-security", "pypi-auth"]
        )
        .is_none()
    );
    if let Ok(ts) = reference() {
        assert!(
            airs_test_harness::live::model_security_refusal(&ts, &["model-security", "pypi-auth"])
                .is_some()
        );
        assert!(
            airs_test_harness::live::model_security_refusal(
                &ts,
                &["model-security", "groups", "list"]
            )
            .is_none()
        );
    }
}

fn fast_writes() -> MsWriteSettings {
    MsWriteSettings {
        recheck_interval: std::time::Duration::from_millis(10),
    }
}

#[test]
fn live_modelsec_writes_self_test_against_fixture() {
    let Some(ts) = require_ts("live_modelsec_writes_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = ms_fixture();
    let before = sandbox.snapshot();
    let report = run_live_modelsec_writes(&ms_plan(&sandbox, ts), &fast_writes())
        .unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    assert_eq!(
        report.resources,
        Some(ResourceCounts {
            created: 2,
            deleted: 2,
            left: 0
        }),
        "{table}"
    );
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "model-security groups create (rs)",
        "model-security groups create (ts)",
        "model-security groups get (rs group)",
        "model-security groups get (ts group)",
        "model-security groups update (rs)",
        "model-security groups update (ts)",
        "model-security rule-instances list (rs group)",
        "model-security rule-instances get (ts group)",
        "model-security rule-instances update (rs)",
        "model-security rule-instances update (ts)",
        "flow model-security groups delete (rs)",
        "flow model-security groups delete (ts)",
        "teardown every created model-security group is gone",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    assert_ms_writes_owned(&server, 0);
    assert!(server.ms_requests(Ms::ScanCreate).is_empty());
    // The group set is back to its seed.
    assert_eq!(
        server.modelsec_data().groups,
        airs_test_harness::ModelSecData::seeded().groups
    );
    let lines = ledger_lines(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    let count = |op: &str, outcome: &str| {
        lines
            .iter()
            .filter(|line| line["op"] == op && line["outcome"] == outcome)
            .count()
    };
    assert_eq!(count("groups create", "ok"), 2);
    assert_eq!(
        count("groups check", "ok"),
        2,
        "every create has a confirmed delete"
    );
    assert_eq!(count("rule-instance update", "ok"), 2);
    assert!(lines.iter().all(|line| line["kind"] == "group"));
    sandbox.assert_unchanged(&before);
    assert_ms_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

/// A soft delete the service finishes later: the re-check confirms it, and nothing is
/// deleted twice.
#[test]
fn live_modelsec_writes_confirm_a_soft_delete_by_the_recheck() {
    let Some(ts) = require_ts("live_modelsec_writes_confirm_a_soft_delete_by_the_recheck") else {
        return;
    };
    let (server, sandbox, _tenant) = ms_fixture();
    server.modelsec(|data| data.group_delete = GroupDeleteMode::Lagging(2));
    let report = run_live_modelsec_writes(&ms_plan(&sandbox, ts), &fast_writes())
        .unwrap_or_else(|abort| panic!("{abort}"));
    let resources = report.resources.unwrap();
    assert_eq!(
        (resources.created, resources.deleted, resources.left),
        (2, 2, 0),
        "{}",
        report.table()
    );
    assert_eq!(
        server.ms_requests(Ms::GroupDelete).len(),
        2,
        "never deleted twice"
    );
    assert_ledger_clean(&ledger_lines(&sandbox), &sandbox);
}

/// An injected 500 on a `groups update`: the flow reports it and still tears down.
#[test]
fn live_modelsec_writes_tear_down_after_a_failed_update() {
    let Some(ts) = require_ts("live_modelsec_writes_tear_down_after_a_failed_update") else {
        return;
    };
    let (server, sandbox, _tenant) = ms_fixture();
    server.push_once(
        Route::ModelSecurity(Ms::GroupUpdate),
        Planned::json(500, &json!({"message": "boom"})).with_retry_after(0),
    );
    let report = run_live_modelsec_writes(&ms_plan(&sandbox, ts), &fast_writes())
        .unwrap_or_else(|abort| panic!("{abort}"));
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert_eq!(
        mismatched,
        ["model-security groups update (rs)"],
        "{}",
        report.table()
    );
    assert_eq!(report.resources.unwrap().left, 0);
    assert_eq!(
        server.modelsec_data().groups,
        airs_test_harness::ModelSecData::seeded().groups
    );
    assert_ledger_clean(&ledger_lines(&sandbox), &sandbox);
}

/// A teardown delete that aborts (here the tenant-integrity check after it) never stops
/// the teardown: every later run group is still deleted.
#[test]
fn live_modelsec_writes_teardown_continues_after_an_aborted_delete() {
    let Some(ts) = require_ts("live_modelsec_writes_teardown_continues_after_an_aborted_delete")
    else {
        return;
    };
    let (server, sandbox, _tenant) = ms_fixture();
    // A JWT-shaped text in the first `groups get` body trips the leak check, so the flow
    // aborts before its deletes and the teardown holds both run groups.
    server.push_once(
        Route::ModelSecurity(Ms::GroupGet),
        Planned::json(
            200,
            &msfx::group(
                101,
                "airs-rs-e2e-shown",
                "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.sig",
                "LOCAL",
                "ACTIVE",
            ),
        ),
    );
    // The teardown's first delete is held while the guarded tenant registry changes, so
    // the integrity check after that delete aborts it.
    server.push_once(Route::ModelSecurity(Ms::GroupDelete), Planned::held());
    let registry = sandbox.registry_path();
    let outcome = std::thread::scope(|scope| {
        let tamper = scope.spawn(|| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
            while server.ms_requests(Ms::GroupDelete).is_empty() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "no teardown delete arrived"
                );
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&registry)
                .unwrap();
            file.write_all(b"\n").unwrap();
            drop(file);
            server.release_held();
        });
        let outcome = run_live_modelsec_writes(&ms_plan(&sandbox, ts), &fast_writes());
        tamper.join().unwrap();
        outcome
    });
    assert!(
        matches!(outcome, Err(LiveAbort::Leak(_))),
        "the flow aborted before its deletes: {:?}",
        outcome.err()
    );
    assert_eq!(
        server.ms_requests(Ms::GroupDelete).len(),
        2,
        "the second run group is deleted after the first delete aborted"
    );
    assert!(
        !server
            .modelsec_data()
            .groups
            .iter()
            .any(|group| group["name"]
                .as_str()
                .is_some_and(|name| name.starts_with(RESOURCE_PREFIX))),
        "no run group is left"
    );
}

#[test]
fn live_modelsec_writes_need_their_own_opt_in() {
    let lookup = |name: &str| match name {
        "AIRS_RS_LIVE" => Some("1".to_owned()),
        "AIRS_E2E_TENANT" => Some("prod".to_owned()),
        _ => None,
    };
    let (server, _sandbox, _tenant) = ms_fixture();
    assert!(
        matches!(modelsec_writes_gate(lookup), Ok(LiveGate::Skip(reason)) if reason.contains("model-security writes skipped"))
    );
    assert_eq!(
        server.request_count(),
        0,
        "the gate decides before anything runs"
    );
}

fn ms_scan_settings() -> MsScanSettings {
    MsScanSettings {
        live: airs_test_harness::live::LiveSettings {
            tenant: "prod".into(),
            profile: None,
            max_scans: 1,
        },
        model_uri: "hf://org/live-model".into(),
        deadline: std::time::Duration::from_secs(5),
        interval: std::time::Duration::from_millis(10),
        groups: fast_writes(),
    }
}

#[test]
fn live_modelsec_scan_self_test_against_fixture() {
    let Some(ts) = require_ts("live_modelsec_scan_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = ms_fixture();
    server.modelsec(|data| data.created_scans_finish_as = Some("ALLOWED".into()));
    let before = sandbox.snapshot();
    let report = run_live_modelsec_scan(&ms_plan(&sandbox, ts), &ms_scan_settings())
        .unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "model-security groups create (rs)",
        "model-security scans create (rs, budget 1)",
        "model-security labels add (rs, ts) and scans get",
        "model-security labels set --force (rs)",
        "model-security labels set (ts) and scans get (rs)",
        "model-security labels delete (ts, rs) and scans get (rs)",
        "model-security scans evaluations <run scan>",
        "model-security scans violations <run scan>",
        "model-security scans files <run scan>",
        "teardown every created model-security group is gone",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    assert_eq!(
        server.ms_requests(Ms::ScanCreate).len(),
        1,
        "budget 1, Rust only"
    );
    let create = &server.ms_requests(Ms::ScanCreate)[0];
    assert!(
        !create
            .header("user-agent")
            .unwrap_or_default()
            .contains("typescript")
    );
    assert_ms_writes_owned(&server, 1);
    assert_eq!(
        report.resources,
        Some(ResourceCounts {
            created: 1,
            deleted: 1,
            left: 0
        }),
        "{table}"
    );
    assert!(
        report
            .notices
            .iter()
            .any(|n| n.contains("has no scan delete"))
    );
    assert!(
        report
            .notices
            .iter()
            .any(|n| n == "model-security scan final outcome: ALLOWED")
    );
    // The run's labels are gone from its scan; every seeded scan is untouched.
    let data = server.modelsec_data();
    let seeded = airs_test_harness::ModelSecData::seeded();
    assert_eq!(&data.scans[..seeded.scans.len()], &seeded.scans[..]);
    let lines = ledger_lines(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    assert!(
        lines
            .iter()
            .any(|line| line["op"] == "scan kept" && line["note"] == "undeletable")
    );
    sandbox.assert_unchanged(&before);
    assert_ms_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

/// An unknown `scans create` outcome is resolved by the read-only listing; the POST is
/// sent exactly once.
#[test]
fn live_modelsec_scan_resolves_an_unknown_create_by_the_listing() {
    let Some(ts) = require_ts("live_modelsec_scan_resolves_an_unknown_create_by_the_listing")
    else {
        return;
    };
    let (server, sandbox, _tenant) = ms_fixture();
    server.push_once(
        Route::ModelSecurity(Ms::ScanCreate),
        Planned::applied_then_dropped(),
    );
    let report = run_live_modelsec_scan(&ms_plan(&sandbox, ts), &ms_scan_settings())
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert_eq!(server.ms_requests(Ms::ScanCreate).len(), 1);
    assert!(
        report
            .rows
            .iter()
            .any(|row| row.check == "model-security scans create (rs, budget 1)" && row.agree),
        "{}",
        report.table()
    );
    let lines = ledger_lines(&sandbox);
    assert!(
        lines
            .iter()
            .any(|line| line["op"] == "scans create" && line["outcome"] == "unknown")
    );
    assert!(
        lines
            .iter()
            .any(|line| line["op"] == "scans lookup" && line["outcome"] == "ok")
    );
    assert_ledger_clean(&lines, &sandbox);
}

/// Every gate refuses before anything is opened: no store, no request.
#[test]
fn live_modelsec_scan_gates_refuse_before_any_request() {
    let (server, sandbox, _tenant) = ms_fixture();
    let base = |extra: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            [
                ("AIRS_RS_LIVE", "1"),
                ("AIRS_E2E_TENANT", "prod"),
                ("AIRS_RS_LIVE_MS_WRITES", "1"),
            ]
            .iter()
            .chain(extra.iter())
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
        }
    };
    // The scan flow writes (a group, labels): without the write opt-in it is skipped,
    // however complete the scan variables are.
    let no_writes = |name: &str| {
        [
            ("AIRS_RS_LIVE", "1"),
            ("AIRS_E2E_TENANT", "prod"),
            ("AIRS_RS_LIVE_MS_SCAN", "1"),
            ("AIRS_E2E_MS_MODEL_URI", "hf://org/m"),
        ]
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| (*value).to_owned())
    };
    assert!(matches!(
        MsScanGate::from_lookup(no_writes),
        Ok(MsScanGate::Skip(reason)) if reason.contains("AIRS_RS_LIVE_MS_WRITES")
    ));
    assert!(matches!(
        MsScanGate::from_lookup(base(&[])),
        Ok(MsScanGate::Skip(_))
    ));
    assert!(MsScanGate::from_lookup(base(&[("AIRS_RS_LIVE_MS_SCAN", "1")])).is_err());
    assert!(
        MsScanGate::from_lookup(base(&[
            ("AIRS_RS_LIVE_MS_SCAN", "1"),
            ("AIRS_E2E_MS_MODEL_URI", "s3://u:FAKE-MS-SECRET-01@bucket/m")
        ]))
        .is_err()
    );
    assert!(
        MsScanGate::from_lookup(base(&[
            ("AIRS_RS_LIVE_MS_SCAN", "1"),
            ("AIRS_E2E_MS_MODEL_URI", "hf://org/m"),
            ("AIRS_RS_LIVE_MS_SCAN_MINUTES", "0")
        ]))
        .is_err()
    );
    let mut settings = ms_scan_settings();
    settings.model_uri = "https://u:FAKE-MS-SECRET-01@h.example/m?sig=1".into();
    let refused = run_live_modelsec_scan(&ms_plan(&sandbox, rust()), &settings);
    assert!(
        matches!(refused, Err(LiveAbort::Setup(message)) if !message.contains("FAKE-MS-SECRET"))
    );
    assert_eq!(server.request_count(), 0);
    assert!(stores_under(&sandbox.tmp().join("live-stores")).is_empty());
}

#[test]
fn live_cleanup_sweeps_only_old_prefixed_groups() {
    let (server, sandbox, _) = ms_fixture();
    let now = now_unix();
    let old = |cli: &str| format!("airs-rs-e2e-{}-7-0-group-{cli}", now - 7200);
    server.modelsec(|data| {
        let group = |n: u64, name: &str| msfx::group(n, name, "", "LOCAL", "ACTIVE");
        data.groups.push(group(120, &old("sc")));
        data.groups.push(group(
            121,
            &format!("airs-rs-e2e-{}-7-1-group-ts", now - 60),
        ));
        data.groups
            .push(group(122, "airs-rs-e2e-precious-group-rs"));
        let mut tombstoned = group(123, &old("rs"));
        tombstoned["is_tombstone"] = json!(true);
        data.groups.push(tombstoned);
    });
    let report =
        run_live_cleanup_at(&plan(&sandbox, rust()), now).unwrap_or_else(|abort| panic!("{abort}"));
    let deletes: Vec<String> = server
        .ms_requests(Ms::GroupDelete)
        .iter()
        .map(|request| request.line())
        .collect();
    assert_eq!(
        deletes,
        [format!(
            "DELETE /aims/mgmt/v1/security-groups/{}",
            msfx::uuid(120)
        )],
        "{}",
        report.table()
    );
    let data = server.modelsec_data();
    for n in [121, 122, 123] {
        assert!(
            data.groups.iter().any(|g| g["uuid"] == msfx::uuid(n)),
            "{n} kept"
        );
    }
    let lines = ledger_lines(&sandbox);
    assert!(lines.iter().any(|line| line["kind"] == "group"
        && line["phase"] == "cleanup"
        && line["outcome"] == "ok"));
    assert_ledger_clean(&lines, &sandbox);
}

// ---------------------------------------------------------------------------------------
// M6d: the runtime-extras reads and the profile-transfer flow (DESIGN §10)
// ---------------------------------------------------------------------------------------

use airs_test_harness::Rx;
use airs_test_harness::fixtures::runtime as rxfx;
use airs_test_harness::live::profile_transfer::{
    TransferHooks, profile_transfer_gate, run_live_profile_transfer, run_live_profile_transfer_with,
};
use airs_test_harness::live::runtime::{GET_FORBIDDEN, run_live_runtime_reads};

/// The runtime-extras reads against the selected tenant (read-only on both CLIs). Needs
/// only `AIRS_RS_LIVE=1` and `AIRS_E2E_TENANT`.
#[test]
fn live_runtime_reads() {
    let settings = match LiveGate::from_env() {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1 and AIRS_E2E_TENANT=<name> for the runtime-extras reads)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.tenant);
    let report = run_live_runtime_reads(&plan)
        .unwrap_or_else(|abort| panic!("live runtime-extras reads stopped: {abort}"));
    notice(&report.table());
    write_summary("summary-runtime.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The profile-transfer flow: one run-created topic and profile, backed up, deleted,
/// restored, verified and deleted (at most four resources, journalled to
/// `target/live/ledger.jsonl`). Needs `AIRS_RS_LIVE_PROFILE_TRANSFER=1` as well.
#[test]
fn live_profile_transfer() {
    let settings = match profile_transfer_gate(|name| std::env::var(name).ok()) {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1, AIRS_E2E_TENANT=<name> and AIRS_RS_LIVE_PROFILE_TRANSFER=1 for the profile-transfer flow)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.tenant);
    let report = run_live_profile_transfer(&plan).unwrap_or_else(|abort| {
        panic!("live profile-transfer flow stopped: {abort} (see target/live/ledger.jsonl; run the sweeper with AIRS_RS_LIVE_CLEANUP=1 after an hour)")
    });
    notice(&report.table());
    write_summary("summary-profile-transfer.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    assert_eq!(
        report.resources.map_or(1, |r| r.left),
        0,
        "live tier: resources were left behind; see target/live/ledger.jsonl and run the sweeper"
    );
    assert!(
        report
            .resources
            .is_some_and(|r| r.created == r.deleted && r.created > 0),
        "live tier: not every created resource was deleted ({:?}); see target/live/ledger.jsonl",
        report.resources
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The standard fixture with the runtime-extras dataset, an `api_key` on a listed key
/// row (D205) and the topic the first profile references.
fn rx_fixture() -> (FixtureServer, Sandbox, FixtureTenant) {
    let (server, sandbox, tenant) = fixture();
    server.seed_runtime();
    server.runtime(|data| {
        data.api_keys[0]["api_key"] = json!(rxfx::key_secret("100", "list"));
        // A key inside a customer app's record (D205): TS prints it in every get.
        data.customer_apps[0]["api_keys_dp_info"][0]["api_key"] =
            json!(rxfx::key_secret("100", "app"));
    });
    (server, sandbox, tenant)
}

/// The routes no M6d live flow may reach.
const RX_FORBIDDEN: [Rx; 6] = [
    Rx::ApiKeyCreate,
    Rx::ApiKeyRegenerate,
    Rx::ApiKeyDelete,
    Rx::CustomerAppUpdate,
    Rx::CustomerAppDelete,
    Rx::ScanLogs,
];

/// No forbidden route was reached, and neither summary nor ledger carries an M6d marker.
fn assert_rx_clean(server: &FixtureServer, report: &LiveReport, sandbox: &Sandbox) {
    for rx in RX_FORBIDDEN {
        assert!(server.rx_requests(rx).is_empty(), "{rx:?} was reached");
    }
    let text = format!(
        "{}\n{}\n{}",
        report.table(),
        serde_json::to_string(&report.to_json()).unwrap(),
        serde_json::to_string(&ledger_lines(sandbox)).unwrap()
    );
    for marker in [rxfx::KEY_MARKER, rxfx::AUTHCODE_MARKER]
        .into_iter()
        .chain(rxfx::CONTENT_MARKERS)
    {
        assert!(
            !text.contains(marker),
            "a summary or the ledger carries {marker}"
        );
    }
    for needle in [
        "chatbot",
        "litellm",
        "pan_session",
        "app-1",
        "uuid-shared",
        "user-1",
    ] {
        assert!(
            !text.contains(needle),
            "the summary carries tenant content {needle:?}"
        );
    }
    for dir in ["rs", "ts"] {
        assert!(
            !sandbox.tmp().join(dir).exists(),
            "the run files were removed"
        );
    }
}

#[test]
fn live_runtime_reads_self_test_against_fixture() {
    let Some(ts) = require_ts("live_runtime_reads_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = rx_fixture();
    let report =
        run_live_runtime_reads(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    assert_eq!(report.store_removed, Some(true));
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "api-keys list --limit 20 (table)",
        "api-keys list --limit 20 (rs json)",
        "customer-apps list --limit 20 (table)",
        "customer-apps list --limit 20 (rs json)",
        "customer-apps get <first> (rs json)",
        "deployment-profiles list",
        "scan-logs query",
        "dashboard applications --limit 5",
        "dashboard top-applications",
        "dashboard violations-trend",
        "dashboard apps-list",
        "dashboard application <first bucket> --interval 7",
        "dashboard application-violations <first bucket> --interval 7",
        "sessions chart",
        "sessions list --limit 5",
        "sessions get <first session>",
        "sessions transaction <first action>",
        "sessions scan-content --output-file",
        "customer-apps consumption <first bucket> --time-interval 7",
        "runtime report --output-file",
        "profiles backup <first profile>",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    // The TypeScript json key listing never ran; nothing was written.
    assert!(
        server
            .requests()
            .iter()
            .all(|request| !request.route.is_write())
    );
    // Choice 70: the TypeScript customer-apps get (whose every format prints `raw`, keys
    // included) never ran: one get, the Rust json.
    assert_eq!(server.rx_requests(Rx::CustomerApp).len(), 1);
    assert_rx_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

#[test]
fn live_runtime_reads_note_unavailable_access() {
    let Some(ts) = require_ts("live_runtime_reads_note_unavailable_access") else {
        return;
    };
    let (server, sandbox, tenant) = rx_fixture();
    server.set_default(Route::Runtime(Rx::ApiKeys), Planned::empty_json(403));
    server.set_default(Route::Runtime(Rx::AppsOverview), Planned::empty_json(403));
    let report =
        run_live_runtime_reads(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert!(report.mismatches().is_empty(), "{table}");
    for note in ["api-keys unavailable", "dashboard unavailable"] {
        assert!(
            report.notices.iter().any(|notice| notice.starts_with(note)),
            "{note}: {:?}",
            report.notices
        );
    }
    // Direct dashboard checks were skipped. The bounded replacement discovery and
    // report still independently read session details (D338, D340), never content.
    assert!(server.rx_requests(Rx::Session).len() <= 2);
    assert!(server.rx_requests(Rx::ScanContent).is_empty());
    assert!(server.rx_requests(Rx::Application).is_empty());
    assert_rx_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// The live finding of 2026-10-18: the service account may list customer apps but not
/// read one. A 403 on both gets is noted as agreeing; the TypeScript get ran once with
/// `--output table` and its stdout was never kept.
#[test]
fn live_runtime_reads_note_a_get_forbidden_on_both_clis() {
    let Some(ts) = require_ts("live_runtime_reads_note_a_get_forbidden_on_both_clis") else {
        return;
    };
    let (server, sandbox, tenant) = rx_fixture();
    server.set_default(Route::Runtime(Rx::CustomerApp), Planned::empty_json(403));
    let report =
        run_live_runtime_reads(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert!(report.mismatches().is_empty(), "{table}");
    let get = row(&report, "customer-apps get <first> (rs json)");
    assert!(get.agree, "{table}");
    assert_eq!((get.rust_exit.as_str(), get.ts_exit.as_str()), ("1", "1"));
    assert_eq!(get.note, GET_FORBIDDEN);
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.starts_with("customer-apps get unavailable: HTTP 403")),
        "{:?}",
        report.notices
    );
    assert!(route_seen_by(
        &server,
        Route::Runtime(Rx::CustomerApp),
        "typescript-sdk"
    ));
    assert_rx_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// A 403 on the Rust get alone stays the failure, and the successful TypeScript get's
/// stdout (the app's `raw`, its `api_key` included) reaches neither the row nor the
/// summary.
#[test]
fn live_runtime_reads_keep_a_rust_only_get_failure() {
    let Some(ts) = require_ts("live_runtime_reads_keep_a_rust_only_get_failure") else {
        return;
    };
    let (server, sandbox, tenant) = rx_fixture();
    // The Rust read re-reads a 403 once with a fresh token (G7): both attempts refused.
    for _ in 0..2 {
        server.push_once(Route::Runtime(Rx::CustomerApp), Planned::empty_json(403));
    }
    let report =
        run_live_runtime_reads(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &["customer-apps get <first> (rs json)"]);
    let get = row(&report, "customer-apps get <first> (rs json)");
    assert_eq!((get.rust_exit.as_str(), get.ts_exit.as_str()), ("1", "0"));
    assert_ne!(get.note, GET_FORBIDDEN);
    // The TypeScript get was answered with the record (its key included).
    let agents: Vec<String> = server
        .rx_requests(Rx::CustomerApp)
        .iter()
        .map(|request| request.header("user-agent").unwrap_or_default().to_owned())
        .collect();
    assert_eq!(
        agents.len(),
        3,
        "two Rust attempts (403), one TypeScript get (200)"
    );
    assert!(agents[2].contains("typescript-sdk"), "{agents:?}");
    assert_rx_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

#[test]
fn live_profile_transfer_needs_its_own_opt_in() {
    let lookup = |name: &str| match name {
        "AIRS_RS_LIVE" => Some("1".to_owned()),
        "AIRS_E2E_TENANT" => Some("prod".to_owned()),
        _ => None,
    };
    let (server, _sandbox, _tenant) = rx_fixture();
    assert!(
        matches!(profile_transfer_gate(lookup), Ok(LiveGate::Skip(reason)) if reason.contains("profile transfer skipped"))
    );
    assert_eq!(
        server.request_count(),
        0,
        "the gate decides before anything runs"
    );
}

/// Every write of a transfer flow named a run-created resource, no seeded id was
/// written, at most ten creates; the fixture's sets equal their seed.
fn assert_transfer_owned(
    server: &FixtureServer,
    profiles_before: &[serde_json::Value],
    topics_before: &[serde_json::Value],
) {
    let seeded: std::collections::BTreeSet<String> = ids_of(profiles_before, "profile_id")
        .into_iter()
        .chain(ids_of(topics_before, "topic_id"))
        .collect();
    let mut creates = 0;
    for request in server.requests() {
        if !request.route.is_write() {
            continue;
        }
        if matches!(request.route, Route::ProfileCreate | Route::TopicCreate) {
            creates += 1;
            let body = request.body_json().unwrap_or_default();
            let name = body
                .get("profile_name")
                .or_else(|| body.get("topic_name"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            assert!(name.starts_with(RESOURCE_PREFIX), "{}", request.line());
        }
        for id in &seeded {
            assert!(
                !request.path.contains(id.as_str()),
                "touched {}",
                request.line()
            );
        }
    }
    assert!(creates <= 10, "{creates} creates");
    assert_eq!(server.profiles("100"), profiles_before);
    assert_eq!(server.topics("100"), topics_before);
}

#[test]
fn live_profile_transfer_self_test_against_fixture() {
    let Some(ts) = require_ts("live_profile_transfer_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = rx_fixture();
    let profiles_before = server.profiles("100");
    let topics_before = server.topics("100");
    let report =
        run_live_profile_transfer(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    assert_eq!(report.store_removed, Some(true));
    let resources = report.resources.expect("counts");
    assert_eq!(resources.left, 0, "{table}");
    assert_eq!(resources.created, resources.deleted, "{table}");
    assert_eq!(resources.created, 4, "T, P and their copies: {table}");
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "topics create (rs)",
        "profiles create (rs)",
        "topics apply (rs)",
        "profiles backup <own profile>",
        "profiles restore --dry-run (rs)",
        "profiles restore (rs)",
        "profiles get <restored>",
        "profiles restore --on-conflict verify --dry-run (rs)",
        "profiles restore --on-conflict verify (rs)",
        "profiles restore <typescript file> (rs)",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    assert_transfer_owned(&server, &profiles_before, &topics_before);
    // The verify re-run wrote nothing: two profile creates and two topic creates in all.
    assert_eq!(server.route_count(Route::ProfileCreate), 2);
    assert_eq!(server.route_count(Route::TopicCreate), 2);
    let lines = ledger_lines(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    let restored = lines
        .iter()
        .filter(|line| line["op"] == "profiles restore" && line["outcome"] == "ok")
        .count();
    assert_eq!(restored, 2, "{lines:?}");
    // The verify re-run is journalled as a write for both copies (pending, then skipped:
    // verified and reused, nothing created).
    for outcome in ["pending", "skipped"] {
        let count = lines
            .iter()
            .filter(|line| line["op"] == "profiles restore verify" && line["outcome"] == outcome)
            .count();
        assert_eq!(count, 2, "{outcome}: {lines:?}");
    }
    // Every create has a confirmed delete.
    let ok_deletes = lines
        .iter()
        .filter(|line| {
            matches!(
                line["op"].as_str(),
                Some("profiles delete" | "topics delete")
            ) && line["outcome"] == "ok"
        })
        .count();
    assert!(ok_deletes >= 4, "{lines:?}");
    // A non-prefixed name is refused by the ledger itself.
    let mut ledger = ledger::Ledger::open(&sandbox.tmp().join("ledger-check"), "run").unwrap();
    assert!(
        ledger
            .record(&ledger::Entry {
                phase: ledger::Phase::Flow,
                cli: "rs",
                op: "profiles create",
                kind: ledger::Kind::Profile,
                name: "Production",
                id: None,
                outcome: ledger::Outcome::Pending,
                exit: None,
                note: "",
            })
            .is_err()
    );
    assert_rx_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

#[test]
fn live_profile_transfer_tears_down_after_an_unknown_restore_create() {
    let Some(ts) = require_ts("live_profile_transfer_tears_down_after_an_unknown_restore_create")
    else {
        return;
    };
    let (server, sandbox, _) = rx_fixture();
    let profiles_before = server.profiles("100");
    let topics_before = server.topics("100");
    // The first profile create (P) is served; the restore's copy is applied, then the
    // connection drops (an unknown outcome).
    server.push_once(Route::ProfileCreate, Planned::builtin());
    server.push_once(Route::ProfileCreate, Planned::applied_then_dropped());
    let report =
        run_live_profile_transfer(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert_mismatches(&report, &["profiles restore (rs)"]);
    assert_eq!(report.resources.map(|r| r.left), Some(0), "{table}");
    assert_eq!(server.route_count(Route::ProfileCreate), 2, "never re-sent");
    assert_transfer_owned(&server, &profiles_before, &topics_before);
    let lines = ledger_lines(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    assert!(
        lines.iter().any(|line| line["op"] == "profiles restore"
            && line["kind"] == "profile"
            && line["outcome"] == "unknown"),
        "{lines:?}"
    );
}

/// A topic delete whose outcome is unknown but which was applied: the restore may then
/// create the topic as well as the profile. Every restore is journalled for both copies
/// whatever the dry run said, so the topic copy has its `pending` and `ok` lines and the
/// teardown removes it (choice 69).
#[test]
fn live_profile_transfer_journals_every_copy_a_restore_creates() {
    let Some(ts) = require_ts("live_profile_transfer_journals_every_copy_a_restore_creates") else {
        return;
    };
    let (server, sandbox, _) = rx_fixture();
    let profiles_before = server.profiles("100");
    let topics_before = server.topics("100");
    server.push_once(Route::TopicDelete, Planned::applied_then_dropped());
    let report =
        run_live_profile_transfer(&plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    // Only the delete whose outcome was unknown disagrees.
    assert_mismatches(&report, &["flow topics delete"]);
    assert_eq!(report.resources.map(|r| r.left), Some(0), "{table}");
    assert_eq!(server.route_count(Route::TopicCreate), 2, "T and its copy");
    assert_transfer_owned(&server, &profiles_before, &topics_before);
    let lines = ledger_lines(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    for kind in ["topic", "profile"] {
        assert!(
            lines.iter().any(|line| line["op"] == "profiles restore"
                && line["kind"] == kind
                && line["outcome"] == "ok"
                && line["id"].is_string()),
            "{kind}: {lines:?}"
        );
    }
}

#[test]
fn live_profile_transfer_tears_down_after_a_panic() {
    let Some(ts) = require_ts("live_profile_transfer_tears_down_after_a_panic") else {
        return;
    };
    let (server, sandbox, _) = rx_fixture();
    let profiles_before = server.profiles("100");
    let topics_before = server.topics("100");
    let hooks = TransferHooks {
        after_creates: Some(|| panic!("injected after the creates")),
    };
    let plan = plan(&sandbox, ts);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_live_profile_transfer_with(&plan, hooks)
    }));
    assert!(
        outcome.is_err(),
        "the panic is re-raised after the teardown"
    );
    assert_transfer_owned(&server, &profiles_before, &topics_before);
    let lines = ledger_lines(&sandbox);
    assert_ledger_clean(&lines, &sandbox);
    assert!(
        lines
            .iter()
            .any(|line| line["phase"] == "teardown" && line["outcome"] == "ok")
    );
    assert!(stores_under(&sandbox.tmp().join("live-stores")).is_empty());
}

// ---------------------------------------------------------------------------------------
// M6e Gateway reads (DESIGN §10 M6e)
// ---------------------------------------------------------------------------------------

use airs_test_harness::Gw;
use airs_test_harness::fixtures::gateway as gwfx;
use airs_test_harness::live::gateway::{reads_only, run_live_gateway_reads};

/// The gateway reads against the selected tenant: read-only on both CLIs, never a gateway
/// write and never inference. Needs only `AIRS_RS_LIVE=1` and `AIRS_E2E_TENANT`.
#[test]
fn live_gateway_reads() {
    let settings = match LiveGate::from_env() {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1 and AIRS_E2E_TENANT=<name> for the gateway reads)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.tenant);
    let report = run_live_gateway_reads(&plan)
        .unwrap_or_else(|abort| panic!("live gateway reads stopped: {abort}"));
    notice(&report.table());
    write_summary("summary-gateway.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The seeded gateway dataset on a single fixture tenant; the plan forbids every
/// `FAKE-GW-SECRET-*` marker on every stream of every run (both CLIs).
fn gw_fixture() -> (FixtureServer, Sandbox, FixtureTenant) {
    let server = FixtureServer::start();
    server.seed_gateway();
    let sandbox = Sandbox::new();
    let tenant = sandbox.single_tenant(&server, "prod", "100");
    (server, sandbox, tenant)
}

fn gw_plan(sandbox: &Sandbox, ts: Cli) -> LivePlan {
    let mut plan = plan(sandbox, ts);
    plan.extra_secrets = gwfx::markers();
    plan
}

/// No write and no inference reached the fixture; the summary holds no marker and no
/// tenant content; the report files were removed.
fn assert_gateway_clean(server: &FixtureServer, report: &LiveReport, sandbox: &Sandbox) {
    reads_only(&server.requests()).unwrap_or_else(|problem| panic!("{problem}"));
    for gw in Gw::INFERENCE {
        assert!(server.gw_requests(gw).is_empty(), "{gw:?} was reached");
    }
    assert!(
        server
            .requests()
            .iter()
            .all(|request| !request.route.is_write())
    );
    let text = format!(
        "{}\n{}",
        report.table(),
        serde_json::to_string(&report.to_json()).unwrap()
    );
    for marker in gwfx::markers() {
        assert!(!text.contains(&marker), "the summary carries {marker}");
    }
    for needle in [
        "ws-main-a-349e0e",
        "Main A",
        "11111111-",
        "main_airs_workspace",
        "Example Org",
        "gpt-x",
    ] {
        assert!(
            !text.contains(needle),
            "the summary carries tenant content {needle:?}"
        );
    }
    for dir in ["gw-rs", "gw-ts"] {
        assert!(
            !sandbox.tmp().join(dir).exists(),
            "the report files were removed"
        );
    }
}

#[test]
fn live_gateway_reads_self_test_against_fixture() {
    let Some(ts) = require_ts("live_gateway_reads_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = gw_fixture();
    let report =
        run_live_gateway_reads(&gw_plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    assert_eq!(report.store_removed, Some(true));
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    for expected in [
        "workspaces list --output json",
        "workspaces list --output table",
        "workspaces list --plane admin --output json",
        "workspaces list --plane admin --output table",
        "workspaces get <first data-plane slug>",
        "scopes list --output json",
        "telemetry requests <pinned window>",
        "configs list (table)",
        "guardrails list (table)",
        "api-keys service list (table)",
        "api-keys user list (table)",
        "configs get <first> (rs json)",
        "guardrails get <first> (rs json)",
        "api-keys service get <first> (rs json)",
        "api-keys user get <first> (rs json)",
        "providers list (rs json)",
        "providers get <first> (rs json)",
        "telemetry logs list --page-size 5 (rs json)",
        "integrations list (table)",
        "integrations providers (table)",
        "mcp integrations list (table)",
        "deployments list (table)",
        "plugins list (table)",
        "admin-guardrails list --page-size 20 (table)",
        "guardrails catalog (table)",
        "integrations get <first> (rs json)",
        "mcp integrations get <first> (rs json)",
        "deployments get <first> (rs json)",
        "admin-guardrails get <first> (rs json)",
        "audit-logs list (rs json)",
        "organisations self get (rs json)",
        "organisations auth-settings get (rs json)",
        "report --output-file",
        "doctor Gateway API",
    ] {
        assert!(checks.contains(&expected), "{expected} missing:\n{table}");
    }
    // Choice 156 (live): the catalog arrives in the service's member order, which TypeScript
    // keeps and airs-cli-rs sorts (D35); its JSON cell agrees as JSON, and the note says so.
    let catalog = report
        .rows
        .iter()
        .find(|row| row.check == "guardrails catalog (table)")
        .unwrap();
    assert!(catalog.agree, "{table}");
    assert_eq!(
        catalog.note, "cells equal (D238); JSON cells equal apart from key order: 1 (D35)",
        "{table}"
    );
    // The pinned window reached both CLIs' telemetry requests as the same instants.
    let windows: std::collections::BTreeSet<(String, String)> = server
        .gw_requests(Gw::Chart)
        .iter()
        .map(|request| {
            (
                request
                    .query_value("timeOfGenerationMin")
                    .unwrap()
                    .to_owned(),
                request
                    .query_value("timeOfGenerationMax")
                    .unwrap()
                    .to_owned(),
            )
        })
        .collect();
    assert_eq!(windows.len(), 1, "{windows:?}");
    let (min, max) = windows.into_iter().next().unwrap();
    assert!(
        min.ends_with(":00+00:00") && max.ends_with(":00+00:00"),
        "{min} {max}"
    );
    assert_gateway_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

/// A 403 on every data-plane route on both CLIs: the agreeing `gateway unavailable`
/// notice, the data-plane checks skipped, the admin checks still run.
#[test]
fn live_gateway_reads_note_an_unavailable_data_plane() {
    let Some(ts) = require_ts("live_gateway_reads_note_an_unavailable_data_plane") else {
        return;
    };
    let (server, sandbox, tenant) = gw_fixture();
    for gw in [
        Gw::DataWorkspaces,
        Gw::DataWorkspaceGet,
        Gw::Chart,
        Gw::Logs,
        Gw::Configs,
        Gw::ServiceKeys,
        Gw::UserKeys,
        Gw::Providers,
        Gw::Guardrails,
    ] {
        server.set_default(
            Route::Gateway(gw),
            Planned::json(403, &json!({"message": "Forbidden"})),
        );
    }
    let report =
        run_live_gateway_reads(&gw_plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert!(report.mismatches().is_empty(), "{table}");
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.starts_with("gateway unavailable: HTTP 403 on both CLIs")),
        "{:?}",
        report.notices
    );
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    assert!(!checks.contains(&"configs list (table)"), "{table}");
    assert!(!checks.contains(&"report --output-file"), "{table}");
    assert!(checks.contains(&"integrations list (table)"), "{table}");
    assert!(server.gw_requests(Gw::Chart).is_empty());
    assert_gateway_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// A 403 on the admin routes only: `gateway admin plane unavailable`, the data-plane
/// checks still pass.
#[test]
fn live_gateway_reads_note_an_unavailable_admin_plane() {
    let Some(ts) = require_ts("live_gateway_reads_note_an_unavailable_admin_plane") else {
        return;
    };
    let (server, sandbox, tenant) = gw_fixture();
    for gw in [
        Gw::AdminWorkspaces,
        Gw::Integrations,
        Gw::CatalogProviders,
        Gw::McpIntegrations,
        Gw::Deployments,
        Gw::Plugins,
        Gw::AdminGuardrails,
        Gw::GuardrailCatalog,
        Gw::AuditLogs,
        Gw::OrganisationSelf,
        Gw::OrganisationInfo,
        Gw::AuthSettings,
    ] {
        server.set_default(
            Route::Gateway(gw),
            Planned::json(403, &json!({"message": "Forbidden"})),
        );
    }
    let report =
        run_live_gateway_reads(&gw_plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert!(report.mismatches().is_empty(), "{table}");
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.starts_with("gateway admin plane unavailable: HTTP 403")),
        "{:?}",
        report.notices
    );
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    assert!(checks.contains(&"configs list (table)"), "{table}");
    assert!(!checks.contains(&"integrations list (table)"), "{table}");
    assert_gateway_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// A 403 on the Rust gate alone (both Rust attempts refused) stays a mismatch.
#[test]
fn live_gateway_reads_keep_a_rust_only_gate_failure() {
    let Some(ts) = require_ts("live_gateway_reads_keep_a_rust_only_gate_failure") else {
        return;
    };
    let (server, sandbox, tenant) = gw_fixture();
    for _ in 0..2 {
        server.push_once(
            Route::Gateway(Gw::DataWorkspaces),
            Planned::json(403, &json!({"message": "Forbidden"})),
        );
    }
    let report =
        run_live_gateway_reads(&gw_plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &["workspaces list (gate)"]);
    assert_gateway_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// Choices 149 and 155: a table mismatch is diagnosable from the summary alone. A catalog
/// whose `api_key` carries a value and whose member is named after it (airs-cli-rs withholds
/// the one and drops the other, D47) mismatches with a note of shapes and counts: the row
/// counts, the equal header, the count of TypeScript-only fields (never their names), one
/// cell withheld by airs-cli-rs only, the JSON cell equal apart from key order (D35); never
/// the value or any other tenant content. (Not a fixture marker:
/// TypeScript prints it, and the session's leak check stops a run that prints a marker.)
#[test]
fn live_gateway_reads_note_a_table_mismatch_by_shape() {
    let Some(ts) = require_ts("live_gateway_reads_note_a_table_mismatch_by_shape") else {
        return;
    };
    let (server, sandbox, tenant) = gw_fixture();
    server.gateway(|data| {
        let catalog = data.inventory.get_mut(&Gw::GuardrailCatalog).unwrap();
        // Letters only: a printable name, which the note must still not print (choice 155).
        catalog["api_key"] = json!("LIVENOTEVALUESEVEN");
        catalog["LIVENOTEVALUESEVEN"] = json!("shown");
    });
    let report =
        run_live_gateway_reads(&gw_plan(&sandbox, ts)).unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &["guardrails catalog (table)"]);
    let row = report
        .rows
        .iter()
        .find(|row| row.check == "guardrails catalog (table)")
        .unwrap();
    assert_eq!(
        row.note,
        "tables differ: rows 3/4 (airs-cli-rs/TypeScript); header equal; fields TypeScript only: 1; cells withheld by airs-cli-rs only: 1 (1 placeholders); JSON cells equal apart from key order: 1; other differing cells: 0",
        "{}",
        report.table()
    );
    let summary = format!(
        "{}\n{}",
        report.table(),
        serde_json::to_string(&report.to_json()).unwrap()
    );
    assert!(!summary.contains("LIVENOTEVALUESEVEN"), "{summary}");
    assert!(!summary.contains("shown"), "{summary}");
    assert_gateway_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// The live guard refuses every gateway write and every inference command on either CLI
/// (DESIGN §10 M6e), whatever flow asks.
#[test]
fn live_runs_refuse_gateway_writes_and_inference() {
    use airs_test_harness::live::gateway_refusal;
    for args in [
        &["gateway", "workspaces", "create", "--name", "x"][..],
        &["aigateway", "configs", "delete", "x", "--force"],
        &["gateway", "inference", "embeddings", "hi"],
        &["aigateway", "inference", "chat", "hi"],
        &[
            "gateway",
            "admin-guardrails",
            "mcp-servers",
            "sync",
            "x",
            "--force",
        ],
        &[
            "gateway",
            "--output",
            "json",
            "mcp",
            "integrations",
            "workspaces",
            "set",
            "x",
            "--force",
        ],
        &[
            "gateway",
            "--tenant",
            "t",
            "--output",
            "json",
            "admin-guardrails",
            "mcp-servers",
            "sync",
            "x",
            "--force",
        ],
        &["gateway", "--output", "json", "inference", "chat", "hi"],
    ] {
        assert!(gateway_refusal(args).is_some(), "{args:?}");
    }
    assert!(gateway_refusal(&["gateway", "workspaces", "list"]).is_none());
    assert!(gateway_refusal(&["gateway", "--output", "json", "workspaces", "get", "w"]).is_none());
}

// ---------------------------------------------------------------------------------------
// M6f Supplychain reads (DESIGN §10 M6f)
// ---------------------------------------------------------------------------------------

use airs_test_harness::Sc;
use airs_test_harness::fixtures::supplychain as scfx;
use airs_test_harness::live::supplychain::{
    reads_only as sc_reads_only, run_live_supplychain_reads,
};

/// The supplychain reads against the selected tenant: one page of each read on both CLIs,
/// the Rust-only `--include-content` run checked in memory, the report on both; a tenant
/// without AI Supply Chain access records `supplychain unavailable` and passes. Needs only
/// `AIRS_RS_LIVE=1` and `AIRS_E2E_TENANT`.
#[test]
fn live_supplychain_reads() {
    let settings = match LiveGate::from_env() {
        Ok(LiveGate::Skip(reason)) => {
            notice(&format!(
                "LIVE SKIPPED: {reason} (set AIRS_RS_LIVE=1 and AIRS_E2E_TENANT=<name> for the supplychain reads)"
            ));
            return;
        }
        Ok(LiveGate::Run(settings)) => settings,
        Err(problem) => panic!("live tier: {problem}"),
    };
    let plan = real_plan(&settings.tenant);
    let report = run_live_supplychain_reads(&plan)
        .unwrap_or_else(|abort| panic!("live supplychain reads stopped: {abort}"));
    notice(&report.table());
    write_summary("summary-supplychain.json", &report);
    assert_eq!(
        report.store_removed,
        Some(true),
        "live tier: the private store was not removed"
    );
    let mismatched: Vec<&str> = report
        .mismatches()
        .iter()
        .map(|row| row.check.as_str())
        .collect();
    assert!(
        mismatched.is_empty(),
        "live tier: mismatches {mismatched:?}"
    );
}

/// The seeded supplychain dataset (scans created an hour ago) on a single fixture tenant.
fn sc_fixture() -> (FixtureServer, Sandbox, FixtureTenant) {
    let server = FixtureServer::start();
    server.seed_supplychain();
    server.supplychain(|data| data.scans = scfx::recent_scans());
    let sandbox = Sandbox::new();
    let tenant = sandbox.single_tenant(&server, "prod", "100");
    (server, sandbox, tenant)
}

/// The plan forbids the `PRIVATE-` marker on every stream of every run, except the stdout of
/// the Rust `--include-content` run.
fn sc_plan(sandbox: &Sandbox, ts: Cli) -> LivePlan {
    let mut plan = plan(sandbox, ts);
    plan.content_markers = vec![scfx::MARKER.to_owned()];
    plan
}

/// Only `GET`s reached the fixture; the summary holds no marker and no tenant content; the
/// report and debug directories were removed.
fn assert_supplychain_clean(server: &FixtureServer, report: &LiveReport, sandbox: &Sandbox) {
    sc_reads_only(&server.requests()).unwrap_or_else(|problem| panic!("{problem}"));
    let text = format!(
        "{}\n{}",
        report.table(),
        serde_json::to_string(&report.to_json()).unwrap()
    );
    for needle in [
        scfx::MARKER,
        "11111111-",
        "SECRET_EXPOSURE",
        "Secret exposure",
        "UNLISTED",
        "COMPLETED",
        "BLOCKING",
        "password_reset",
        "os.environ",
        "2026-",
        "30_DAYS",
    ] {
        assert!(
            !text.contains(needle),
            "the summary carries tenant content {needle:?}:\n{text}"
        );
    }
    for dir in ["sc-rs", "sc-ts", "sc-debug"] {
        assert!(!sandbox.tmp().join(dir).exists(), "{dir} was removed");
    }
}

#[test]
fn live_supplychain_reads_self_test_against_fixture() {
    let Some(ts) = require_ts("live_supplychain_reads_self_test_against_fixture") else {
        return;
    };
    let (server, sandbox, tenant) = sc_fixture();
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    assert_eq!(report.store_removed, Some(true));
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    assert_eq!(
        checks,
        [
            "stats (gate)",
            "stats --output json",
            "stats --output table",
            "scans list --limit 10 --output json",
            "scans list --limit 10 --output table",
            "scans vulnerabilities <first scan> --output json",
            "scans vulnerabilities <first scan> --include-content (rs, in memory)",
            "rules list --limit 10 --output json",
            "rules list --limit 10 --output table",
            "report --max-pages 3 --output-file",
        ],
        "{table}"
    );
    let content = row(
        &report,
        "scans vulnerabilities <first scan> --include-content (rs, in memory)",
    );
    assert!(
        content
            .note
            .starts_with("content on stdout only; the D321 notice"),
        "{}",
        content.note
    );
    assert_eq!(content.ts_exit, "-", "Rust only");
    // The rules catalog arrives with passthrough members in the service's order, which
    // airs-cli-rs sorts (D35): equal as JSON.
    assert_eq!(
        row(&report, "rules list --limit 10 --output json").note,
        "documents equal (D35)"
    );
    // TypeScript never ran with --include-content; the content route was read by the two
    // projections and the Rust content run only.
    assert_eq!(server.sc_requests(Sc::Vulnerabilities).len(), 3);
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.starts_with("doctor: no supplychain check exists")),
        "{:?}",
        report.notices
    );
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

/// A 403 on every supplychain route on both CLIs: the agreeing `supplychain unavailable`
/// notice, nothing else run.
#[test]
fn live_supplychain_reads_note_an_unavailable_service() {
    let Some(ts) = require_ts("live_supplychain_reads_note_an_unavailable_service") else {
        return;
    };
    let (server, sandbox, tenant) = sc_fixture();
    for sc in Sc::ALL {
        server.set_default(
            Route::Supplychain(sc),
            Planned::json(403, &json!({"message": "Forbidden"})),
        );
    }
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert!(report.mismatches().is_empty(), "{table}");
    assert!(
        report.notices.iter().any(|notice| notice
            == "supplychain unavailable: HTTP 403 on both CLIs (no AI Supply Chain access)"),
        "{:?}",
        report.notices
    );
    let checks: Vec<&str> = report.rows.iter().map(|row| row.check.as_str()).collect();
    assert_eq!(checks, ["stats (gate)"], "{table}");
    for sc in [Sc::Scans, Sc::Vulnerabilities, Sc::Rules] {
        assert!(server.sc_requests(sc).is_empty(), "{sc:?}");
    }
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// A 403 on the Rust gate alone (both Rust attempts refused, G63) stays a mismatch.
#[test]
fn live_supplychain_reads_keep_a_rust_only_gate_failure() {
    let Some(ts) = require_ts("live_supplychain_reads_keep_a_rust_only_gate_failure") else {
        return;
    };
    let (server, sandbox, tenant) = sc_fixture();
    for _ in 0..2 {
        server.push_once(
            Route::Supplychain(Sc::Stats),
            Planned::json(403, &json!({"message": "Forbidden"})),
        );
    }
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &["stats (gate)"]);
    assert_eq!(
        row(&report, "stats (gate)").note,
        "the gate failed on one side only"
    );
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// The service's data rolls: a check whose two documents differ is run once more on both
/// sides; a second agreement passes (noted), a second difference fails with a masked note.
#[test]
fn live_supplychain_reads_reread_once_when_the_data_rolls() {
    let Some(ts) = require_ts("live_supplychain_reads_reread_once_when_the_data_rolls") else {
        return;
    };
    let rolled = || {
        let mut stats = scfx::stats();
        stats.set(
            "unique_skills_scanned",
            scfx::obj(vec![
                ("count", scfx::w(json!(5))),
                ("percent_change", scfx::w(json!(1.5))),
            ]),
        );
        Planned::raw(200, "application/json", stats.text())
    };
    // The gate, then the Rust run of `stats --output json` sees a rolled document once.
    let (server, sandbox, tenant) = sc_fixture();
    server.push_once(Route::Supplychain(Sc::Stats), Planned::builtin());
    server.push_once(Route::Supplychain(Sc::Stats), rolled());
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts.clone()))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert!(report.mismatches().is_empty(), "{}", report.table());
    assert_eq!(
        row(&report, "stats --output json").note,
        "documents equal (D35) (after one re-read)"
    );
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
    // A second difference fails; the note names shapes only.
    let (server, sandbox, tenant) = sc_fixture();
    server.push_once(Route::Supplychain(Sc::Stats), Planned::builtin());
    server.push_once(Route::Supplychain(Sc::Stats), rolled());
    server.push_once(Route::Supplychain(Sc::Stats), Planned::builtin());
    server.push_once(Route::Supplychain(Sc::Stats), rolled());
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &["stats --output json"]);
    assert_eq!(
        row(&report, "stats --output json").note,
        "json differs: records 1/1 (airs-cli-rs/TypeScript); the same members; values differ"
    );
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// The findings roll between the plain read and the content run: the Rust plain read and
/// the content run run once more and the second pair decides (choice 60); a second pair
/// that agrees passes (noted), one that differs again fails with the check's own note.
#[test]
fn live_supplychain_reads_reread_the_content_run_once_when_the_findings_roll() {
    let Some(ts) =
        require_ts("live_supplychain_reads_reread_the_content_run_once_when_the_findings_roll")
    else {
        return;
    };
    let check = "scans vulnerabilities <first scan> --include-content (rs, in memory)";
    let rolled = || {
        Planned::raw(
            200,
            "application/json",
            scfx::vulnerability_list_body(&scfx::findings()[..2], 2, true),
        )
    };
    // The plain pair sees three findings, the content run two; the re-read pair sees two.
    let (server, sandbox, tenant) = sc_fixture();
    for planned in [
        Planned::builtin(),
        Planned::builtin(),
        rolled(),
        rolled(),
        rolled(),
    ] {
        server.push_once(Route::Supplychain(Sc::Vulnerabilities), planned);
    }
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts.clone()))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert!(report.mismatches().is_empty(), "{}", report.table());
    let note = &row(&report, check).note;
    assert!(
        note.starts_with("content on stdout only; the D321 notice")
            && note.ends_with(" (after one re-read)"),
        "{note}"
    );
    // The plain pair, the content run, then one Rust plain read and one content run.
    assert_eq!(server.sc_requests(Sc::Vulnerabilities).len(), 5);
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
    // The re-read plain read sees three findings and the content run two again: a mismatch.
    let (server, sandbox, tenant) = sc_fixture();
    for planned in [
        Planned::builtin(),
        Planned::builtin(),
        rolled(),
        Planned::builtin(),
        rolled(),
    ] {
        server.push_once(Route::Supplychain(Sc::Vulnerabilities), planned);
    }
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &[check]);
    assert_eq!(
        row(&report, check).note,
        "content check failed: exit ok true, same findings false, notice true, stderr clean true, debug log clean true"
    );
    assert_eq!(server.sc_requests(Sc::Vulnerabilities).len(), 5);
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// The data rolls under the report: a report pair that differs runs once more on both CLIs
/// and the second pair decides (choice 61); a second agreement passes (noted), a second
/// difference fails.
#[test]
fn live_supplychain_reads_reread_the_report_once_when_the_data_rolls() {
    let Some(ts) = require_ts("live_supplychain_reads_reread_the_report_once_when_the_data_rolls")
    else {
        return;
    };
    let check = "report --max-pages 3 --output-file";
    let missing = || Planned::json(404, &json!({"message": "Not found"}));
    // The gate and the two stats pairs read the statistics, then the first Rust report
    // finds none (its Statistics source unavailable) while TypeScript's reads them.
    let (server, sandbox, tenant) = sc_fixture();
    for _ in 0..5 {
        server.push_once(Route::Supplychain(Sc::Stats), Planned::builtin());
    }
    server.push_once(Route::Supplychain(Sc::Stats), missing());
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts.clone()))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert!(report.mismatches().is_empty(), "{}", report.table());
    assert_eq!(
        row(&report, check).note,
        "0600 files; sections, sources, methods and statuses equal (D322); aggregates only (after one re-read)"
    );
    assert_eq!(server.sc_requests(Sc::Stats).len(), 9, "two report pairs");
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
    // The re-read Rust report finds no statistics either: a mismatch.
    let (server, sandbox, tenant) = sc_fixture();
    for _ in 0..5 {
        server.push_once(Route::Supplychain(Sc::Stats), Planned::builtin());
    }
    for planned in [missing(), Planned::builtin(), missing()] {
        server.push_once(Route::Supplychain(Sc::Stats), planned);
    }
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &[check]);
    assert_eq!(row(&report, check).note, "the reports differ");
    assert_eq!(server.sc_requests(Sc::Stats).len(), 9);
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// The live guard refuses a TypeScript `agentguard` run with `--include-content` anywhere
/// after the group, in every live flow (DESIGN §10 M6f); the Rust run is allowed.
#[test]
fn live_runs_refuse_typescript_supplychain_content() {
    use airs_test_harness::live::supplychain_content_refusal;
    // The guard reads only the CLI's kind and the arguments; nothing is run.
    let ts = Cli::Ts {
        node: "node".into(),
        entry: "index.js".into(),
    };
    for args in [
        &[
            "agentguard",
            "scans",
            "vulnerabilities",
            "u",
            "--include-content",
        ][..],
        &[
            "--output",
            "json",
            "agentguard",
            "scans",
            "vulnerabilities",
            "u",
            "--include-content",
        ],
        &[
            "agentguard",
            "--include-content",
            "scans",
            "vulnerabilities",
            "u",
        ],
        &[
            "agentguard",
            "scans",
            "vulnerabilities",
            "--",
            "u",
            "--include-content",
        ],
        &[
            "agentguard",
            "scans",
            "vulnerabilities",
            "u",
            "--include-content=true",
        ],
    ] {
        assert!(supplychain_content_refusal(&ts, args).is_some(), "{args:?}");
    }
    assert!(
        supplychain_content_refusal(&ts, &["agentguard", "scans", "vulnerabilities", "u"])
            .is_none()
    );
    assert!(
        supplychain_content_refusal(
            &rust(),
            &[
                "supplychain",
                "scans",
                "vulnerabilities",
                "u",
                "--include-content"
            ]
        )
        .is_none()
    );
}

/// The content markers are forbidden outside the Rust `--include-content` stdout: a rule
/// text carrying one stops the flow at the first run that prints it (a leak), and the
/// private store is still removed.
#[test]
fn live_supplychain_reads_stop_on_a_marker_outside_the_content_run() {
    let Some(ts) = require_ts("live_supplychain_reads_stop_on_a_marker_outside_the_content_run")
    else {
        return;
    };
    let (server, sandbox, _tenant) = sc_fixture();
    server.supplychain(|data| {
        data.rules[0].set(
            "description",
            scfx::w(json!(format!("{}RULE", scfx::MARKER))),
        );
    });
    let plan = sc_plan(&sandbox, ts);
    match run_live_supplychain_reads(&plan) {
        Err(LiveAbort::Leak(problem)) => {
            assert!(
                problem.contains("rules list --limit 10 --output json")
                    && problem.contains("appeared in stdout"),
                "{problem}"
            );
            assert!(!problem.contains(scfx::MARKER), "{problem}");
        }
        other => panic!("expected a leak stop, got {other:?}"),
    }
    assert!(
        std::fs::read_dir(&plan.store_root)
            .map(|entries| entries.count())
            .unwrap_or(0)
            == 0,
        "the private store was removed"
    );
}

/// Scanned code may hold a JWT-shaped string: the `--include-content` stdout is checked for
/// the plan's secret forms only (the JWT shape outside the finding rows still stops the
/// flow), so the content row agrees and the flow runs to the end; the summary never holds
/// the string.
#[test]
fn live_supplychain_reads_accept_a_jwt_shape_in_scanned_code() {
    let Some(ts) = require_ts("live_supplychain_reads_accept_a_jwt_shape_in_scanned_code") else {
        return;
    };
    const JWT: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.abc";
    let (server, sandbox, tenant) = sc_fixture();
    server.supplychain(|data| {
        let rows = data
            .findings
            .get_mut(&scfx::uuid(scfx::SCAN_WITH_FINDINGS))
            .expect("the seeded findings");
        rows[2].set("original_code", scfx::w(json!(format!("AUTH = '{JWT}'"))));
    });
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert!(report.mismatches().is_empty(), "{table}");
    assert!(
        row(
            &report,
            "scans vulnerabilities <first scan> --include-content (rs, in memory)"
        )
        .note
        .starts_with("content on stdout only; the D321 notice"),
        "{table}"
    );
    row(&report, "report --max-pages 3 --output-file");
    assert!(!table.contains("eyJ"), "{table}");
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// A disagreement whose re-read is unavailable on both CLIs stays a mismatch (the first
/// note kept), and the flow goes on with the first Rust listing.
#[test]
fn live_supplychain_reads_keep_a_difference_whose_reread_is_unavailable() {
    let Some(ts) =
        require_ts("live_supplychain_reads_keep_a_difference_whose_reread_is_unavailable")
    else {
        return;
    };
    let (server, sandbox, tenant) = sc_fixture();
    let rows = scfx::recent_scans();
    server.push_once(
        Route::Supplychain(Sc::Scans),
        Planned::raw(200, "application/json", scfx::scan_list_body(&rows[..1], 1)),
    );
    server.push_once(Route::Supplychain(Sc::Scans), Planned::builtin());
    for _ in 0..2 {
        server.push_once(
            Route::Supplychain(Sc::Scans),
            Planned::json(404, &json!({"message": "Not found"})),
        );
    }
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &["scans list --limit 10 --output json"]);
    assert_eq!(
        row(&report, "scans list --limit 10 --output json").note,
        "differed, then unavailable: HTTP 404 on both CLIs on re-read (first: json differs: records 1/10 (airs-cli-rs/TypeScript); the same members; values differ)"
    );
    // The vulnerabilities and the content run still ran on the first listing's scan.
    row(&report, "scans vulnerabilities <first scan> --output json");
    row(
        &report,
        "scans vulnerabilities <first scan> --include-content (rs, in memory)",
    );
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// A content run refused after the plain read of the same scan succeeded is a mismatch,
/// not `unavailable`: the D321 policy was not checked.
#[test]
fn live_supplychain_reads_fail_a_refused_content_run() {
    let Some(ts) = require_ts("live_supplychain_reads_fail_a_refused_content_run") else {
        return;
    };
    let (server, sandbox, tenant) = sc_fixture();
    server.push_once(Route::Supplychain(Sc::Vulnerabilities), Planned::builtin());
    server.push_once(Route::Supplychain(Sc::Vulnerabilities), Planned::builtin());
    server.push_once(
        Route::Supplychain(Sc::Vulnerabilities),
        Planned::json(404, &json!({"message": "Not found"})),
    );
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    let check = "scans vulnerabilities <first scan> --include-content (rs, in memory)";
    assert_mismatches(&report, &[check]);
    assert_eq!(
        row(&report, check).note,
        "content run refused after the plain read succeeded: HTTP 404"
    );
    assert!(
        !report
            .notices
            .iter()
            .any(|notice| notice.contains("unavailable")),
        "{:?}",
        report.notices
    );
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// D318 is tolerated only in its shape: with an array member, the Rust field rows must
/// equal the statistics document and TypeScript's table must be the array's record table;
/// a Rust table with another count is a mismatch, whatever the array.
#[test]
fn live_supplychain_reads_tolerate_d318_only_in_its_shape() {
    let Some(ts) = require_ts("live_supplychain_reads_tolerate_d318_only_in_its_shape") else {
        return;
    };
    let with_array = |count: u64| {
        let mut stats = scfx::stats();
        stats.set(
            "unique_skills_scanned",
            scfx::obj(vec![
                ("count", scfx::w(json!(count))),
                ("percent_change", scfx::w(json!(12.5))),
            ]),
        );
        stats.set(
            "zz_list",
            scfx::w(json!([{"k": 1, "skill_name": "a"}, {"k": 2}])),
        );
        stats
    };
    let (server, sandbox, tenant) = sc_fixture();
    server.supplychain(|data| data.stats = Some(with_array(4)));
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts.clone()))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert!(report.mismatches().is_empty(), "{}", report.table());
    assert_eq!(
        row(&report, "stats --output table").note,
        "D318 tolerated: the field rows equal the statistics document; TypeScript prints an array member's record table"
    );
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
    // The Rust table runs (first and re-read) see another count.
    let (server, sandbox, tenant) = sc_fixture();
    server.supplychain(|data| data.stats = Some(with_array(4)));
    let rolled = || Planned::raw(200, "application/json", with_array(999).text());
    for planned in [
        Planned::builtin(),
        Planned::builtin(),
        Planned::builtin(),
        rolled(),
        Planned::builtin(),
        rolled(),
        Planned::builtin(),
    ] {
        server.push_once(Route::Supplychain(Sc::Stats), planned);
    }
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &["stats --output table"]);
    assert_eq!(
        row(&report, "stats --output table").note,
        "D318 shape not met: field rows differing from the statistics document: 1"
    );
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// G71 (D324): the scan rows hold the `null`s the service sends (the observed
/// `"fingerprint": null` and every other member the tolerant read accepts as `null`).
/// TypeScript refuses both scan checks with its status-less request line, on the first read
/// and on the re-read (choice 85), and airs-cli-rs prints them: both rows pass with the G71
/// note; the vulnerabilities checks read the
/// first scan airs-cli-rs listed on both CLIs (and the content run); the report agrees but
/// for TypeScript's unavailable Scans source.
#[test]
fn live_supplychain_reads_accept_rows_the_shared_contract_refuses() {
    use airs_test_harness::live::supplychain::{G71_NOTE, G71_REPORT_NOTE};
    let Some(ts) = require_ts("live_supplychain_reads_accept_rows_the_shared_contract_refuses")
    else {
        return;
    };
    let (server, sandbox, tenant) = sc_fixture();
    server.supplychain(|data| {
        data.scans = scfx::recent_null_scans();
        data.findings.insert(
            scfx::uuid(113),
            vec![
                scfx::finding(341, 113, "SECRET_EXPOSURE", 1),
                scfx::ordinary_finding(342, 113),
            ],
        );
    });
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    println!("{table}");
    assert!(report.mismatches().is_empty(), "{table}");
    for check in [
        "scans list --limit 10 --output json",
        "scans list --limit 10 --output table",
    ] {
        let row = row(&report, check);
        assert_eq!(row.note, G71_NOTE, "{table}");
        assert_eq!((row.rust_exit.as_str(), row.ts_exit.as_str()), ("0", "1"));
    }
    assert_eq!(
        row(&report, "scans vulnerabilities <first scan> --output json").note,
        "documents equal (4-key projection, content_included false)"
    );
    assert!(
        row(
            &report,
            "scans vulnerabilities <first scan> --include-content (rs, in memory)"
        )
        .note
        .starts_with("content on stdout only; the D321 notice"),
        "{table}"
    );
    assert_eq!(
        row(&report, "report --max-pages 3 --output-file").note,
        G71_REPORT_NOTE
    );
    // G71 held on both reads of each scan check (choice 85): the json and table checks ran
    // twice on both CLIs (8 GETs), then one report page on each CLI.
    assert_eq!(server.sc_requests(Sc::Scans).len(), 10);
    // The listed scan is G71's observed row: both plain reads and the content run read it.
    let reads = server.sc_requests(Sc::Vulnerabilities);
    assert_eq!(reads.len(), 3);
    assert!(
        reads
            .iter()
            .all(|request| request.path.contains(&scfx::uuid(113)))
    );
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
    server.assert_no_unknown_routes();
}

/// G71 is only TypeScript's status-less request line against a Rust success: a scan page
/// both CLIs refuse (a scan without a `uuid`: the contract line here, TypeScript's request
/// line there) and a TypeScript failure with an HTTP status stay mismatches.
#[test]
fn live_supplychain_reads_keep_other_scan_failures_as_mismatches() {
    let Some(ts) = require_ts("live_supplychain_reads_keep_other_scan_failures_as_mismatches")
    else {
        return;
    };
    let scan_checks = [
        "scans list --limit 10 --output json",
        "scans list --limit 10 --output table",
    ];
    // Both refuse the page.
    let (server, sandbox, tenant) = sc_fixture();
    server.supplychain(|data| {
        data.scans = scfx::recent_null_scans();
        data.scans[1].remove("uuid");
    });
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts.clone()))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &scan_checks);
    for check in scan_checks {
        assert!(row(&report, check).note.contains("a run failed"), "{check}");
    }
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice
                == "scans vulnerabilities skipped: the scan listing returned no scan"),
        "{:?}",
        report.notices
    );
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
    // The Rust runs read the page, the TypeScript runs get an HTTP 500 (first and re-read).
    let (server, sandbox, tenant) = sc_fixture();
    for _ in 0..4 {
        server.push_once(Route::Supplychain(Sc::Scans), Planned::builtin());
        server.push_once(
            Route::Supplychain(Sc::Scans),
            Planned::json(500, &json!({"message": "x"})),
        );
    }
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &scan_checks);
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// A body TypeScript refuses with its status-less request line (not JSON), while the
/// fixture's own page goes to the other side.
fn ts_unreadable() -> Planned {
    Planned::raw(200, "application/json", "not json")
}

/// G71 needs evidence (choice 85): an empty Rust listing shows no row the shared contract
/// could refuse, so TypeScript's status-less line against it is a mismatch on both scan
/// checks, never a G71 pass.
#[test]
fn live_supplychain_reads_need_rust_rows_for_g71() {
    use airs_test_harness::live::supplychain::G71_NOTE;
    let Some(ts) = require_ts("live_supplychain_reads_need_rust_rows_for_g71") else {
        return;
    };
    let (server, sandbox, tenant) = sc_fixture();
    server.supplychain(|data| data.scans = Vec::new());
    // json and table, first read and re-read: the Rust run the fixture's empty page, the
    // TypeScript run a body it cannot read.
    for _ in 0..4 {
        server.push_once(Route::Supplychain(Sc::Scans), Planned::builtin());
        server.push_once(Route::Supplychain(Sc::Scans), ts_unreadable());
    }
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    let scan_checks = [
        "scans list --limit 10 --output json",
        "scans list --limit 10 --output table",
    ];
    assert_mismatches(&report, &scan_checks);
    for check in scan_checks {
        let row = row(&report, check);
        assert!(row.note.ends_with("a run failed"), "{check}: {}", row.note);
        assert_ne!(row.note, G71_NOTE);
        assert_eq!((row.rust_exit.as_str(), row.ts_exit.as_str()), ("0", "1"));
    }
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}

/// G71 needs the refusal on both reads and no TypeScript success (choice 85). A TypeScript
/// failure on the first json read only (its status-less line, the service answering on
/// the re-read) is rolling data: the re-read decides, the row passes `(after one re-read)`,
/// and the listing is not G71, so a report whose TypeScript Scans source then fails the
/// same way (both report reads) is a mismatch, not G71's tolerance. A refusal on the
/// re-read alone fails with its own note.
#[test]
fn live_supplychain_reads_need_the_refusal_twice_for_g71() {
    use airs_test_harness::live::supplychain::{G71_NOTE, G71_ONCE_NOTE, G71_REPORT_NOTE};
    let Some(ts) = require_ts("live_supplychain_reads_need_the_refusal_twice_for_g71") else {
        return;
    };
    let json_check = "scans list --limit 10 --output json";
    let table_check = "scans list --limit 10 --output table";
    let report_check = "report --max-pages 3 --output-file";
    let (server, sandbox, tenant) = sc_fixture();
    let scans = Route::Supplychain(Sc::Scans);
    // json: Rust, TypeScript (refused), then both answered.
    server.push_once(scans, Planned::builtin());
    server.push_once(scans, ts_unreadable());
    server.push_once(scans, Planned::builtin());
    server.push_once(scans, Planned::builtin());
    // table: both answered.
    server.push_once(scans, Planned::builtin());
    server.push_once(scans, Planned::builtin());
    // report, first pair and re-read: the two Rust pages (10 rows each), then a first
    // TypeScript page it refuses (its Scans source unavailable).
    for _ in 0..2 {
        server.push_once(scans, Planned::builtin());
        server.push_once(scans, Planned::builtin());
        server.push_once(scans, ts_unreadable());
    }
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts.clone()))
        .unwrap_or_else(|abort| panic!("{abort}"));
    let table = report.table();
    assert_eq!(
        row(&report, json_check).note,
        "pagination and projections equal (9 keys, no sensitive member) (after one re-read)",
        "{table}"
    );
    assert_eq!(
        row(&report, table_check).note,
        "cells equal (D53)",
        "{table}"
    );
    let report_row = row(&report, report_check);
    assert_ne!(report_row.note, G71_REPORT_NOTE, "{table}");
    assert_eq!(report_row.note, "the reports differ", "{table}");
    assert_mismatches(&report, &[report_check]);
    assert!(
        report.rows.iter().all(|row| row.note != G71_NOTE),
        "{table}"
    );
    // Each scan request went where it was planned: 6 listing runs, then 2 + 1 report pages
    // twice.
    assert_eq!(server.sc_requests(Sc::Scans).len(), 12, "{table}");
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
    // The refusal on the re-read alone: the first pair differs (TypeScript reads a shorter
    // page), the re-read is refused.
    let (server, sandbox, tenant) = sc_fixture();
    let short = scfx::scan_list_body(&scfx::recent_scans()[..2], 2);
    server.push_once(scans, Planned::builtin());
    server.push_once(scans, Planned::raw(200, "application/json", short));
    server.push_once(scans, Planned::builtin());
    server.push_once(scans, ts_unreadable());
    let report = run_live_supplychain_reads(&sc_plan(&sandbox, ts))
        .unwrap_or_else(|abort| panic!("{abort}"));
    assert_mismatches(&report, &[json_check]);
    assert_eq!(row(&report, json_check).note, G71_ONCE_NOTE);
    assert_eq!(
        row(&report, report_check).note,
        "0600 files; sections, sources, methods and statuses equal (D322); aggregates only"
    );
    assert_supplychain_clean(&server, &report, &sandbox);
    assert_sanitized(&report, &sandbox, &tenant);
}
