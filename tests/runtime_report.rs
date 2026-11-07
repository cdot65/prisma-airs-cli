//! Executable acceptance tests for `runtime report` (DESIGN §18.14): the TypeScript
//! `tests/integration/cli/runtime-report.spec.ts` cases against the loopback fixture
//! server (the runtime extras dataset and the standard profiles), plus the Rust
//! additions (the destination parent checked before the tenant, D214; interruption,
//! D224). The collector and renderer specs (`runtime.spec.ts`, `render.spec.ts`) are unit
//! tests in `src/report/runtime_tests.rs`.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use airs_test_harness::{
    Cli, FixtureConfig, FixtureServer, Planned, RecordedRequest, Route, Run, RunOptions, Rx,
    Sandbox, run, spawn,
};
use serde_json::json;

const TSG: &str = "100";
const FAILED_LINE: &str = "  ✗ Report generation failed. Verify Management API configuration and a writable, new output path. Existing files are never overwritten.\n";
const COLLECTING: &str = "  Collecting runtime Signal Brief evidence (read-only)...\n";

/// The seven sources (the profiles source is the management `Route::Profiles`).
const DASHBOARD_SOURCES: [Rx; 5] = [
    Rx::AppsOverview,
    Rx::SessionsOverview,
    Rx::SessionsChart,
    Rx::TopApplications,
    Rx::ViolationsTrend,
];

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

struct Fx {
    server: FixtureServer,
    sandbox: Sandbox,
}

fn fx() -> Fx {
    fx_with(|_| {})
}

fn fx_with(edit: impl FnOnce(&mut FixtureConfig)) -> Fx {
    let server = FixtureServer::start();
    server.seed_runtime();
    let sandbox = Sandbox::new();
    let tenant = sandbox.add_fixture_tenant_with(&server, "prod", TSG, edit);
    sandbox.register(&[&tenant], Some("prod"));
    Fx { server, sandbox }
}

impl Fx {
    /// `runtime report <args>`; writes allowed under `cwd/` (the default name).
    fn report(&self, args: &[&str]) -> Run {
        let mut full = vec!["runtime", "report"];
        full.extend_from_slice(args);
        self.run_raw(&full)
    }

    fn run_raw(&self, args: &[&str]) -> Run {
        let out = run(
            &rust(),
            &self.sandbox,
            args,
            &RunOptions::new().allow_writes_under("cwd"),
        );
        self.server.assert_no_unknown_routes();
        out
    }

    fn api(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .server
            .requests()
            .iter()
            .filter(|request| request.route != Route::Token)
            .map(RecordedRequest::line)
            .collect();
        lines.sort();
        lines
    }

    fn tmp(&self, name: &str) -> String {
        self.sandbox.tmp().join(name).display().to_string()
    }

    fn cwd_files(&self, prefix: &str) -> Vec<PathBuf> {
        self.sandbox
            .cwd_listing()
            .into_iter()
            .filter(|path| path.to_string_lossy().starts_with(prefix))
            .map(|path| self.sandbox.cwd().join(path))
            .collect()
    }
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn no_temporaries(dir: &Path) {
    let left: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(".airs-"))
        .collect();
    assert!(left.is_empty(), "temporaries left: {left:?}");
}

/// Signal Brief HTML despite tenant defaultOutput; bounded read-only SDK discovery,
/// a new private file and stable naming/status conventions.
#[test]
fn writes_signal_html_with_bounded_sdk_reads() {
    let f = fx_with(|config| {
        config.set("defaultOutput", "json");
    });
    let path = f.tmp("daily.html");
    let out = f.report(&["--output-file", &path]);
    out.assert_success();
    let html = std::fs::read_to_string(&path).unwrap();
    assert!(html.starts_with("<!doctype html>\n"));
    assert!(html.ends_with("</body></html>\n"));
    assert_eq!(mode(Path::new(&path)), 0o600);
    no_temporaries(&f.sandbox.tmp());
    assert_eq!(out.stdout, "");
    assert!(
        out.stderr
            .starts_with(&format!("{COLLECTING}  Report → {path} (mode 0600)\n")),
        "{}",
        out.stderr
    );
    let reads = f.api();
    assert!(
        reads.iter().any(|p| p.contains("/sessions/session?")),
        "{reads:?}"
    );
    assert!(reads.iter().any(|p| p.contains("profiles/tsg")));
    assert!(
        !reads
            .iter()
            .any(|p| p.contains("scan-content") || p.starts_with("POST /scan"))
    );
    for needle in ["FAKE-AUTHCODE", "key-1", "pan_session"] {
        assert!(!html.contains(needle));
    }
    assert!(html.contains("AIRS Signal Brief"));
    // The default name: a new timestamped file in the working directory; two runs, two
    // files.
    f.report(&[]).assert_success();
    f.report(&[]).assert_success();
    let files = f.cwd_files("airs-runtime-report-");
    assert_eq!(files.len(), 2, "{files:?}");
    let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        name.len(),
        "airs-runtime-report-2026-10-17T12-00-00.000Z-0a1b2c3d.html".len(),
        "{name}"
    );
    assert!(!name.contains(':') && name.ends_with(".html"));
    assert_eq!(mode(&files[0]), 0o600);
}

/// `--output-file -`: one document on stdout; a closed pipe is the failure line (exit 1).
#[test]
fn stdout_delivery() {
    let f = fx();
    let out = f.run_raw(&["--quiet", "runtime", "report", "--output-file", "-"]);
    out.assert_success();
    assert!(out.stdout.starts_with("<!doctype html>\n"));
    assert_eq!(out.stdout.matches("<!doctype html>").count(), 1);
    assert_eq!(out.stderr, "");
    assert!(f.sandbox.cwd_listing().is_empty());
    let out = run(
        &rust(),
        &f.sandbox,
        &["runtime", "report", "--output-file", "-"],
        &RunOptions::new().close_stdout_after(0),
    );
    out.assert_code(1);
    assert!(out.stderr.ends_with(FAILED_LINE), "{}", out.stderr);
}

/// No 5xx retry (`numRetries: 0`): a failing source is `unavailable`, the report is still
/// useful (exit 0); `--strict` exits 1 after writing.
#[test]
fn a_partial_report_and_strict() {
    let f = fx();
    f.server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(503, &json!({"message": "PRIVATE-ERROR"})).with_retry_after(0),
    );
    let first = f.tmp("partial.html");
    let out = f.report(&["--output-file", &first]);
    out.assert_success();
    assert_eq!(
        f.server.rx_requests(Rx::SessionsOverview).len(),
        1,
        "no retry"
    );
    assert!(
        out.stderr
            .ends_with("  1 source(s) incomplete; review the evidence section.\n"),
        "{}",
        out.stderr
    );
    let html = std::fs::read_to_string(&first).unwrap();
    assert!(html.contains("unavailable"));
    assert!(html.contains("API read unavailable (HTTP 503); no response content retained"));
    assert!(!html.contains("PRIVATE-ERROR"));
    let second = f.tmp("strict.html");
    let out = f.report(&["--strict", "--output-file", &second]);
    out.assert_code(1);
    assert!(!out.stderr.contains('✗'), "{}", out.stderr);
    assert!(
        std::fs::read_to_string(&second)
            .unwrap()
            .contains("unavailable")
    );
}

/// Every source failing: `Unable to assess`, the file still written, no server text,
/// exit 1; each read sent once plus one G7 re-read after a 403.
#[test]
fn no_source_available_still_writes_the_report() {
    let f = fx();
    for source in DASHBOARD_SOURCES
        .iter()
        .map(|rx| Route::Runtime(*rx))
        .chain([Route::Runtime(Rx::CustomerApps), Route::Profiles])
    {
        f.server.set_default(
            source,
            Planned::json(403, &json!({"message": "PRIVATE-ERROR"})),
        );
    }
    let path = f.tmp("unavailable.html");
    let out = f.report(&["--output-file", &path]);
    out.assert_code(1);
    let html = std::fs::read_to_string(&path).unwrap();
    assert!(html.contains("Unable to assess"));
    assert!(html.contains("API read unavailable (HTTP 403)"));
    assert!(!html.contains("PRIVATE-ERROR"));
    assert!(out.stderr.contains("source(s) incomplete"));
    for rx in [Rx::AppsOverview, Rx::SessionsOverview] {
        assert_eq!(
            f.server.rx_requests(rx).len(),
            1,
            "{rx:?}: bounded no-retry read"
        );
    }
}

/// D214: an absent profile list is no record array (`unavailable`), not an empty
/// inventory with the `No security profiles returned` finding.
#[test]
fn an_absent_profile_list_is_unavailable() {
    let f = fx();
    f.server
        .set_default(Route::Profiles, Planned::json(200, &json!({})));
    let out = f.report(&["--output-file", "-"]);
    out.assert_success();
    assert!(
        out.stdout
            .contains("profiles (required)</td><td>unavailable")
    );
    assert!(
        out.stdout
            .contains("Missing profile array is not an empty configuration")
    );
    assert!(!out.stdout.contains("No security profiles returned"));
}

/// An existing path (a dangling symlink included) and a missing parent directory are
/// refused before the tenant and any request (D214): the failure line only.
#[test]
fn the_destination_is_checked_before_any_request() {
    let f = fx();
    let existing = f.sandbox.tmp().join("existing.html");
    std::fs::write(&existing, "KEEP").unwrap();
    let dangling = f.sandbox.tmp().join("dangling.html");
    std::os::unix::fs::symlink(f.sandbox.tmp().join("nowhere"), &dangling).unwrap();
    let missing = f.sandbox.tmp().join("nodir").join("r.html");
    // D226: a path that names no file is refused too (TS writes `<cwd>/sub`).
    let slash = std::path::PathBuf::from(format!("{}/sub/", f.sandbox.tmp().display()));
    let dot = std::path::PathBuf::from(format!("{}/sub/.", f.sandbox.tmp().display()));
    for path in [&existing, &dangling, &missing, &slash, &dot] {
        let out = f.report(&["--output-file", &path.display().to_string()]);
        out.assert_code(1);
        assert_eq!(out.stderr, FAILED_LINE, "{}", path.display());
        assert_eq!(out.stdout, "");
    }
    assert_eq!(std::fs::read_to_string(&existing).unwrap(), "KEEP");
    assert!(!f.sandbox.tmp().join("nowhere").exists());
    assert!(!missing.exists());
    assert!(!f.sandbox.tmp().join("sub").exists());
    assert_eq!(f.server.request_count(), 0);
    // A tenant or credential error: the failure line only, no request.
    let f = fx_with(|config| {
        config.remove("mgmtClientSecret");
    });
    let out = f.report(&["--output-file", "-"]);
    out.assert_code(1);
    assert!(out.stdout.contains("Management configuration unavailable"));
    assert!(out.stderr.contains("source(s) incomplete"));
    assert_eq!(f.server.request_count(), 0);
}

/// Every invalid option exits 2 before the tenant and any request (TS order, D155, D156).
#[test]
fn invalid_options_exit_2_before_any_request() {
    let f = fx();
    let long = "x".repeat(241);
    for (args, message) in [
        (
            vec!["--debug", "runtime", "report"],
            "Disable --debug for environment reports to avoid persisting sensitive traffic content",
        ),
        (
            vec!["--output", "json", "runtime", "report"],
            "The environment report is written as HTML; omit --output",
        ),
        (
            vec!["runtime", "report", "--output", "table"],
            "The environment report is written as HTML; omit --output",
        ),
        (
            vec!["--output", "markdown", "runtime", "report"],
            "Invalid output format 'markdown'. Expected: pretty, table, json",
        ),
        (
            vec!["runtime", "report", "--max-pages", "0"],
            "--max-pages must be an integer from 1 to 100",
        ),
        (
            vec!["runtime", "report", "--max-pages", "1.5"],
            "--max-pages must be an integer from 1 to 100",
        ),
        (
            vec!["runtime", "report", "--max-pages", "101"],
            "--max-pages must be an integer from 1 to 100",
        ),
        (
            vec!["runtime", "report", "--max-pages", "07"],
            "--max-pages must be an integer from 1 to 100",
        ),
        (
            vec!["runtime", "report", "--title", ""],
            "--title must contain 1–240 characters",
        ),
        (
            vec!["runtime", "report", "--title", &long],
            "--title must contain 1–240 characters",
        ),
        (
            vec!["runtime", "report", "--output-file", " "],
            "--output-file cannot be empty",
        ),
    ] {
        let out = f.run_raw(&args);
        out.assert_code(2);
        assert_eq!(out.stderr, format!("\n  ✗ {message}\n\n"), "{args:?}");
        assert_eq!(out.stdout, "");
    }
    assert_eq!(f.server.request_count(), 0);
    assert!(f.sandbox.cwd_listing().is_empty());
}

/// `runtime report` has no `dashboard` alias and its description does not say
/// dashboard (TS `report-naming.spec.ts`); `runtime dashboard` is its own group.
#[test]
fn report_naming() {
    let f = fx();
    let out = f.run_raw(&["runtime", "--help"]);
    out.assert_success();
    let line = out
        .stdout
        .lines()
        .find(|line| line.trim_start().starts_with("report "))
        .unwrap();
    assert!(!line.contains("dashboard"), "{line}");
    assert!(line.contains("Generate an evidence-backed runtime Signal Brief (read-only)"));
    assert!(
        out.stdout
            .lines()
            .any(|line| line.trim_start().starts_with("dashboard "))
    );
    let out = f.run_raw(&["runtime", "report", "--help"]);
    out.assert_success();
    assert!(
        out.stdout
            .contains("$ airs runtime report --output-file ./airs-daily.html")
    );
    assert!(!out.stdout.contains("markdown"));
}

/// A signal during a source read: `Interrupted; no report was written` (D224).
#[test]
fn a_signal_during_collection_writes_no_report() {
    let f = fx();
    f.server
        .push_once(Route::Runtime(Rx::AppsOverview), Planned::hang());
    let path = f.tmp("signal.html");
    let running = spawn(
        &rust(),
        &f.sandbox,
        &["runtime", "report", "--output-file", &path],
        &RunOptions::new().expect_signal(),
    );
    assert!(
        f.server
            .wait_for_route(Route::Runtime(Rx::AppsOverview), 1, Duration::from_secs(20))
    );
    running.send_signal("INT");
    let out = running.wait();
    out.assert_code(130);
    assert_eq!(
        out.stderr,
        format!("{COLLECTING}\n  ✗ Error: Interrupted; no report was written\n\n")
    );
    assert!(!Path::new(&path).exists());
    no_temporaries(&f.sandbox.tmp());
}
