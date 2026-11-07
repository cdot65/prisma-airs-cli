//! Paired frozen-reference checks for explicitly Rust-only credential behavior.
use airs_test_harness::{
    Cli, FixtureServer, Planned, Route, RunOptions, Rx, Sandbox, fixtures, require_ts, run,
};
use serde_json::json;
fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

#[test]
fn d342_named_scanner_management_and_selection_are_rust_only() {
    let Some(ts) = require_ts("d342_named_scanner_management_and_selection_are_rust_only") else {
        return;
    };
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    s.bind_fixture_scanner("prod", "monitor", "app-a", "key");
    let args = ["tenant", "scanners", "list", "prod", "--output", "json"];
    let r = run(&rust(), &s, &args, &RunOptions::new());
    r.assert_success();
    assert!(r.stdout.contains("hasApiKey"));
    assert!(!r.stdout.contains("FAKE-APIKEY"));
    let t = run(&ts, &s, &args, &RunOptions::new());
    t.assert_code(1);
    assert!(t.stderr.contains("unknown command"));
    let args = [
        "runtime",
        "scan",
        "--profile",
        "monitor",
        "--scanner",
        "monitor",
        "Synthetic sample",
    ];
    server.set_default(
        Route::ScanSync,
        Planned::json(200, &fixtures::scan_allow("monitor")),
    );
    let r = run(&rust(), &s, &args, &RunOptions::new());
    r.assert_success();
    assert_eq!(server.route_count(Route::ScanSync), 1);
    let t = run(&ts, &s, &args, &RunOptions::new());
    t.assert_code(1);
    assert!(t.stderr.contains("unknown option"));
    assert_eq!(server.route_count(Route::ScanSync), 1);
}

#[test]
fn d343_unbound_strict_report_keeps_metadata_and_never_probes_flat_keys() {
    let Some(ts) =
        require_ts("d343_unbound_strict_report_keeps_metadata_and_never_probes_flat_keys")
    else {
        return;
    };
    for cli in [rust(), ts] {
        let server = FixtureServer::start();
        server.seed_runtime();
        let s = Sandbox::new();
        s.single_tenant(&server, "prod", "100");
        let item = fixtures::runtime::session_item("uuid-shared", "A & B", "session-1");
        server.set_default(
            Route::Runtime(Rx::SessionsOverview),
            Planned::json(
                200,
                &json!({"items":[item],"pagination":{"limit":25,"skip":0,"total_items":1}}),
            ),
        );
        let mut detail = fixtures::runtime::session("session-1");
        let scan = "00000000-0000-4000-8000-0000000000a1";
        detail["session_actions"][0]["scan_id"] = json!(scan);
        server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &detail));
        server.set_default(Route::ScanResults,Planned::json(200,&json!([{"scan_id":scan,"req_id":0,"status":"COMPLETED","result":fixtures::scan_allow("monitor")}])));
        let opts = if cli.is_ts() {
            RunOptions::new().allow_secret_output()
        } else {
            RunOptions::new()
        };
        let out = run(
            &cli,
            &s,
            &["runtime", "report", "--strict", "--output-file", "-"],
            &opts,
        );
        if cli.is_ts() {
            out.assert_success();
            assert!(out.stdout.contains("Report schema 2"));
            assert!(!out.stdout.contains("bind-default"));
        } else {
            out.assert_code(1);
            assert!(out.stdout.contains("bind-default"));
            assert!(out.stdout.contains("management evidence retained"));
        }
        assert_eq!(server.route_count(Route::ScanResults), 0);
        assert_eq!(server.route_count(Route::ScanReports), 0);
        server.assert_no_unknown_routes();
    }
}

#[test]
fn d344_management_detector_observations_and_session_aggregates_remain_distinct() {
    let Some(ts) =
        require_ts("d344_management_detector_observations_and_session_aggregates_remain_distinct")
    else {
        return;
    };
    let now = String::from_utf8(
        std::process::Command::new("date")
            .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();
    for cli in [rust(), ts] {
        let server = FixtureServer::start();
        server.seed_runtime();
        let s = Sandbox::new();
        s.single_tenant(&server, "prod", "100");
        server.runtime(|data| {
            data.sessions = vec![fixtures::runtime::session_item(
                "uuid-shared",
                "A & B",
                "session-1",
            )]
        });
        let mut detail = fixtures::runtime::session("session-1");
        detail["session_actions"][0]["start_time"] = json!(now);
        detail["session_actions"][0]["detection_type_violation_breakdown"] = json!([{"detection_type":"pi","violation_breakdown":{"total":1},"action":"block","data_type":"prompt"}]);
        server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &detail));
        let opts = if cli.is_ts() {
            RunOptions::new().allow_secret_output()
        } else {
            RunOptions::new()
        };
        let out = run(
            &cli,
            &s,
            &["runtime", "report", "--output-file", "-"],
            &opts,
        );
        out.assert_success();
        if cli.is_ts() {
            assert!(out.stdout.contains("Report schema 2"));
            assert!(
                !out.stdout
                    .contains("Detector-positive metadata observations")
            );
            assert!(!out.stdout.contains("Positive management action breakdown"));
        } else {
            assert!(
                out.stdout
                    .contains("Detector-positive metadata observations")
            );
            assert!(out.stdout.contains("Positive management action breakdown"));
            assert!(out.stdout.contains("Management session activity"));
            assert!(out.stdout.contains("Violated sessions"));
            assert!(out.stdout.contains("unknown"));
        }
        assert_eq!(server.route_count(Route::ScanResults), 0);
        assert_eq!(server.route_count(Route::ScanReports), 0);
    }
}
