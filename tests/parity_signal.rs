use airs_test_harness::{Cli, RunOptions, Sandbox, require_ts, run};
#[test]
fn d337_offline_evidence_is_rust_only() {
    let Some(ts) = require_ts("d337_offline_evidence_is_rust_only") else {
        return;
    };
    let sandbox = Sandbox::new();
    let input = sandbox.tmp().join("evidence.json");
    std::fs::write(&input, include_str!("fixtures/signal/evidence.json")).unwrap();
    let args = [
        "runtime",
        "report",
        "--from-evidence",
        input.to_str().unwrap(),
        "--output-file",
        "-",
    ];
    let rust = run(
        &Cli::rust(env!("CARGO_BIN_EXE_airs")),
        &sandbox,
        &args,
        &RunOptions::new(),
    );
    rust.assert_success();
    assert!(rust.stdout.contains("prompt_injection"));
    assert!(rust.stdout.contains("<table id=\"profile-enablement\""));
    assert!(rust.stdout.contains("<th scope=\"row\">Monitor"));
    assert!(rust.stdout.contains("data-state=\"enabled\""));
    let reference = run(&ts, &sandbox, &args, &RunOptions::new());
    assert_eq!(reference.code, 1);
    assert!(reference.stdout.is_empty());
    assert!(reference.stderr.contains("unknown option"));
}

#[test]
fn d339_direct_scanner_lookups_are_rust_only() {
    use airs_test_harness::{FixtureServer, Planned, Route};
    use serde_json::json;
    let Some(ts) = require_ts("d339_direct_scanner_lookups_are_rust_only") else {
        return;
    };
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    server.set_default(
        Route::ScanReports,
        Planned::json(
            200,
            &json!([{"report_id":"report-1","detection_results":[]}]),
        ),
    );
    let args = [
        "runtime",
        "scan-logs",
        "reports",
        "--report-id",
        "report-1",
        "--output",
        "json",
    ];
    let r = run(
        &Cli::rust(env!("CARGO_BIN_EXE_airs")),
        &sandbox,
        &args,
        &RunOptions::new(),
    );
    r.assert_success();
    assert!(r.stdout.contains("scan_reports"));
    let before = server.request_count();
    let t = run(&ts, &sandbox, &args, &RunOptions::new());
    t.assert_code(1);
    assert!(t.stderr.contains("unknown command"));
    assert_eq!(server.request_count(), before);
}

#[test]
fn d340_private_report_bundle_is_rust_only() {
    let Some(ts) = airs_test_harness::require_ts("d340_private_report_bundle_is_rust_only") else {
        return;
    };
    for cli in [Cli::rust(env!("CARGO_BIN_EXE_airs")), ts] {
        let sandbox = Sandbox::new();
        let input = sandbox.tmp().join("evidence.json");
        std::fs::write(&input, include_str!("fixtures/signal/evidence.json")).unwrap();
        let dest = sandbox.tmp().join("bundle");
        let out = run(
            &cli,
            &sandbox,
            &[
                "runtime",
                "report",
                "--from-evidence",
                input.to_str().unwrap(),
                "--bundle-dir",
                dest.to_str().unwrap(),
            ],
            &RunOptions::new(),
        );
        if cli.is_ts() {
            out.assert_code(1);
            assert!(out.stderr.contains("unknown option"));
            assert!(!dest.exists());
        } else {
            out.assert_success();
            assert!(dest.join("report.html").is_file());
            assert!(dest.join("review.md").is_file());
        }
    }
}

#[test]
fn d341_review_import_requires_saved_evidence_and_is_rust_only() {
    let Some(ts) = airs_test_harness::require_ts(
        "d341_review_import_requires_saved_evidence_and_is_rust_only",
    ) else {
        return;
    };
    for cli in [Cli::rust(env!("CARGO_BIN_EXE_airs")), ts] {
        let sandbox = Sandbox::new();
        let review = sandbox.tmp().join("review.md");
        std::fs::write(&review, "SYNTHETIC-NOT-A-PACKET").unwrap();
        let out = run(
            &cli,
            &sandbox,
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
        assert!(out.stdout.is_empty());
        if cli.is_ts() {
            out.assert_code(1);
            assert!(out.stderr.contains("unknown option"));
        } else {
            out.assert_code(2);
            assert!(out.stderr.contains("--from-evidence"));
        }
    }
}

#[test]
fn d340_runtime_report_help_describes_signal_brief() {
    let Some(ts) = airs_test_harness::require_ts("d340_runtime_report_help_describes_signal_brief")
    else {
        return;
    };
    for cli in [Cli::rust(env!("CARGO_BIN_EXE_airs")), ts] {
        let sandbox = Sandbox::new();
        let out = run(
            &cli,
            &sandbox,
            &["runtime", "report", "--help"],
            &RunOptions::new(),
        );
        out.assert_success();
        if cli.is_ts() {
            assert!(
                out.stdout
                    .contains("daily AI Runtime Security environment report")
            );
            assert!(!out.stdout.contains("--bundle-dir"));
        } else {
            assert!(out.stdout.contains("Signal Brief"));
            assert!(out.stdout.contains("--bundle-dir") && out.stdout.contains("--max-reports"));
        }
    }
}

#[test]
fn d339_report_lookup_limits_distinct_ids_before_tenant_access() {
    let Some(ts) = require_ts("d339_report_lookup_limits_distinct_ids_before_tenant_access") else {
        return;
    };
    let ids: Vec<_> = (0..11).map(|i| format!("report-{i}")).collect();
    let mut args = vec!["runtime", "scan-logs", "reports"];
    for id in &ids {
        args.extend(["--report-id", id]);
    }
    let sandbox = Sandbox::new();
    let rust = run(
        &Cli::rust(env!("CARGO_BIN_EXE_airs")),
        &sandbox,
        &args,
        &RunOptions::new(),
    );
    rust.assert_code(2);
    assert!(rust.stderr.contains("at most 10 distinct report IDs"));
    let reference = run(&ts, &sandbox, &args, &RunOptions::new());
    reference.assert_code(1);
    assert!(reference.stderr.contains("unknown command"));
}
#[test]
fn d340_report_example_cap_is_ten_before_tenant_access() {
    let Some(ts) = require_ts("d340_report_example_cap_is_ten_before_tenant_access") else {
        return;
    };
    let args = ["runtime", "report", "--max-reports", "11"];
    let sandbox = Sandbox::new();
    let rust = run(
        &Cli::rust(env!("CARGO_BIN_EXE_airs")),
        &sandbox,
        &args,
        &RunOptions::new(),
    );
    rust.assert_code(2);
    assert!(
        rust.stderr
            .contains("--max-reports must be an integer from 0 to 10")
    );
    let reference = run(&ts, &sandbox, &args, &RunOptions::new());
    reference.assert_code(1);
    assert!(reference.stderr.contains("unknown option"));
}
