use airs_test_harness::{
    Cli, FixtureConfig, FixtureServer, Planned, Route, RunOptions, Rx, Sandbox, fixtures, run,
};
use serde_json::{Value, json};
const SCAN: &str = "00000000-0000-4000-8000-0000000000b2";
fn cli() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}
fn report() -> Value {
    json!({"report_id":"report-1","scan_id":SCAN,"req_id":0,"session_id":"session-1","detection_results":[{"detection_service":"prompt_injection","data_type":"prompt","verdict":"malicious","action":"allow","result_detail":{"pi_snippets":["Synthetic prompt example"],"dbs_snippets":[],"tc_snippets":["Synthetic toxic example"],"dlp_snippets":{"snippets":["SYNTHETIC-DLP-MATCH"]}}}]})
}
fn setup(edit: impl FnOnce(&mut FixtureConfig)) -> (FixtureServer, Sandbox) {
    let server = FixtureServer::start();
    server.seed_runtime();
    let sandbox = Sandbox::new();
    let tenant = sandbox.add_fixture_tenant_with(&server, "prod", "100", edit);
    sandbox.register(&[&tenant], Some("prod"));
    sandbox.bind_fixture_scanner("prod", "fixture", "uuid-shared", "key");
    (server, sandbox)
}
fn seed_event(server: &FixtureServer) {
    let mut item = fixtures::runtime::session_item("uuid-shared", "A & B", "session-1");
    item["violation_status"] = json!("violated");
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items":[item],"pagination":{"limit":25,"skip":0,"total_items":1}}),
        ),
    );
    let mut session = fixtures::runtime::session("session-1");
    session["session_actions"][0]["scan_id"] = json!(SCAN);
    let at = String::from_utf8(
        std::process::Command::new("date")
            .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    session["session_actions"][0]["start_time"] = json!(at.trim());
    server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
    let mut result = fixtures::scan_block("monitor");
    result["scan_id"] = json!(SCAN);
    result["report_id"] = json!("report-1");
    result["session_id"] = json!("session-1");
    result["action"] = json!("allow");
    server.set_default(
        Route::ScanResults,
        Planned::json(
            200,
            &json!([{"scan_id":SCAN,"req_id":0,"status":"COMPLETED","result":result}]),
        ),
    );
    server.set_default(Route::ScanReports, Planned::json(200, &json!([report()])));
}
#[test]
fn direct_reports_need_only_scanner_credentials_and_mask_all_families() {
    let (server, sandbox) = setup(|c| {
        c.remove("mgmtClientSecret");
        c.remove("mgmtClientId");
    });
    let mut r = report();
    r["detection_results"][0]["result_detail"]["pi_snippets"] =
        json!(["SYNTHETIC-DLP-MATCH repeated here"]);
    server.set_default(Route::ScanReports, Planned::json(200, &json!([r])));
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "reports",
            "--report-id",
            "report-1",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    let value: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(value["complete"], true);
    assert!(!out.stdout.contains("SYNTHETIC-DLP-MATCH"));
    assert!(out.stdout.contains("withheld"));
    assert!(out.stdout.contains("empty"));
    assert!(
        server
            .requests()
            .iter()
            .all(|r| r.route == Route::ScanReports)
    );
}
#[test]
fn discovery_resolves_scan_results_without_legacy_or_content_reads() {
    let (server, sandbox) = setup(|_| {});
    seed_event(&server);
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "query",
            "--interval",
            "24",
            "--unit",
            "hours",
            "--filter",
            "threat",
            "--app-id",
            "uuid-shared",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    let value: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(value["items"].as_array().unwrap().len(), 1);
    assert_eq!(value["items"][0]["action"], "allow");
    assert_eq!(value["items"][0]["verdict"], "malicious");
    assert!(server.requests().iter().all(|r| !matches!(
        r.route,
        Route::ScanSync
            | Route::ScanAsync
            | Route::ScanReports
            | Route::Runtime(Rx::ScanLogs | Rx::ScanContent)
    )));
    server.assert_no_unknown_routes();
}
#[test]
fn missing_reports_and_request_caps_return_partial_data_with_failure() {
    let (server, sandbox) = setup(|_| {});
    server.set_default(Route::ScanReports, Planned::json(200, &json!([])));
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "reports",
            "--report-id",
            "missing",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    let v: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(v["complete"], false);
    assert_eq!(v["missing_ids"], json!(["missing"]));
    let mut args = vec![
        "runtime",
        "scan-logs",
        "reports",
        "--max-requests",
        "1",
        "--output",
        "json",
    ];
    let ids: Vec<_> = (0..6).map(|n| format!("report-{n}")).collect();
    for id in &ids {
        args.extend(["--report-id", id]);
    }
    let before = server.request_count();
    let out = run(&cli(), &sandbox, &args, &RunOptions::new());
    out.assert_code(1);
    assert_eq!(server.request_count() - before, 1);
}
#[test]
fn invalid_options_fail_before_tenant_or_requests() {
    let sandbox = Sandbox::new();
    for args in [
        vec!["runtime", "scan-logs", "results", "--scan-id", "not-a-uuid"],
        vec![
            "runtime",
            "scan-logs",
            "query",
            "--interval",
            "24",
            "--unit",
            "hours",
            "--all",
            "--limit",
            "2",
        ],
        vec![
            "runtime",
            "scan-logs",
            "reports",
            "--report-id",
            "report-1",
            "--concurrency",
            "17",
        ],
        vec![
            "runtime",
            "scan-logs",
            "query",
            "--interval",
            "24",
            "--unit",
            "hours",
            "--page",
            "2",
        ],
    ] {
        let out = run(&cli(), &sandbox, &args, &RunOptions::new());
        out.assert_code(2);
        assert!(!out.stderr.contains("No tenant"));
    }
}
#[test]
fn denied_or_incoherent_discovery_is_never_a_clean_empty_success() {
    let (server, sandbox) = setup(|_| {});
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::empty_json(403),
    );
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "query",
            "--interval",
            "1",
            "--unit",
            "day",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    let value: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(value["complete"], false);
    assert!(out.stdout.contains("unavailable"));
}
#[test]
fn absent_request_identity_is_not_sub_request_zero() {
    let (server, sandbox) = setup(|_| {});
    seed_event(&server);
    let result = fixtures::scan_block("monitor");
    server.set_default(
        Route::ScanResults,
        Planned::json(
            200,
            &json!([{"scan_id":SCAN,"status":"COMPLETED","result":result}]),
        ),
    );
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "query",
            "--interval",
            "24",
            "--unit",
            "hours",
            "--filter",
            "benign",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    let value: Value = serde_json::from_str(&out.stdout).unwrap();
    assert!(value["items"].as_array().unwrap().is_empty());
    assert!(out.stdout.contains("ambiguous"));
}

#[test]
fn repeated_scalar_options_are_rejected_while_ids_are_repeatable() {
    let sandbox = Sandbox::new();
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "reports",
            "--report-id",
            "x",
            "--timeout",
            "1",
            "--timeout",
            "2",
        ],
        &RunOptions::new(),
    );
    out.assert_code(2);
}
#[test]
fn scanner_lookup_deadline_cancels_a_hanging_read() {
    let (server, sandbox) = setup(|_| {});
    server.set_default(Route::ScanReports, Planned::hang());
    let started = std::time::Instant::now();
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "reports",
            "--report-id",
            "x",
            "--timeout",
            "1",
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    // Without the deadline the hung read waits for the harness's 30 s kill; the bound
    // leaves room for process start-up on a contended CI runner.
    assert!(started.elapsed() < std::time::Duration::from_secs(15));
    assert!(out.stdout.contains("deadline"));
}
#[test]
fn unrequested_and_duplicate_report_rows_remain_unresolved() {
    let (server, sandbox) = setup(|_| {});
    let r = report();
    server.set_default(Route::ScanReports, Planned::json(200, &json!([r, r])));
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "reports",
            "--report-id",
            "report-1",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    assert!(out.stdout.contains("Duplicate response identity"));
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "reports",
            "--report-id",
            "other",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    assert!(out.stdout.contains("Unrequested"));
}

#[test]
fn failed_timed_out_or_unknown_scan_outcomes_never_pass_benign_filters() {
    for case in ["failed", "timeout", "errors", "pending", "unknown"] {
        let (server, sandbox) = setup(|_| {});
        seed_event(&server);
        let mut result = fixtures::scan_allow("monitor");
        result["scan_id"] = json!(SCAN);
        let status = match case {
            "pending" => "PENDING",
            "unknown" => "UNFAMILIAR",
            _ => "COMPLETED",
        };
        if case == "failed" {
            result["error"] = json!(true);
        }
        if case == "timeout" {
            result["timeout"] = json!(true);
        }
        if case == "errors" {
            result["errors"] = json!([{"content_type":"prompt","feature":"dlp","status":"error"}]);
        }
        server.set_default(
            Route::ScanResults,
            Planned::json(
                200,
                &json!([{"scan_id":SCAN,"req_id":0,"status":status,"result":result}]),
            ),
        );
        let args = [
            "runtime",
            "scan-logs",
            "query",
            "--interval",
            "24",
            "--unit",
            "hours",
            "--filter",
            "benign",
            "--output",
            "json",
        ];
        let out = run(&cli(), &sandbox, &args, &RunOptions::new());
        out.assert_code(1);
        let v: Value = serde_json::from_str(&out.stdout).unwrap();
        assert_eq!(v["complete"], false);
        assert!(v["items"].as_array().unwrap().is_empty(), "{case}");
        let out = run(
            &cli(),
            &sandbox,
            &[
                "runtime",
                "scan-logs",
                "results",
                "--scan-id",
                SCAN,
                "--output",
                "json",
            ],
            &RunOptions::new(),
        );
        out.assert_code(1);
        let v: Value = serde_json::from_str(&out.stdout).unwrap();
        assert_eq!(v["items"][0]["result"]["action"], "failed");
    }
}
#[test]
fn dlp_metadata_echo_is_masked_without_corrupting_short_match_protocol_values() {
    let (server, sandbox) = setup(|_| {});
    let canary = "SENSITIVE-DLP-CANARY-123456";
    let mut r = report();
    r["source"] = json!(canary);
    r["detection_results"][0]["result_detail"]["dlp_snippets"]["snippets"] = json!([canary, "1"]);
    r["detection_results"][0]["result_detail"]["pi_snippets"] = json!([format!("{canary} and 1")]);
    server.set_default(Route::ScanReports, Planned::json(200, &json!([r])));
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "reports",
            "--report-id",
            "report-1",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    let v: Value = serde_json::from_str(&out.stdout).unwrap();
    assert!(!out.stdout.contains(canary));
    assert_eq!(v["items"][0]["scan_id"], SCAN);
    assert_eq!(v["items"][0]["report_id"], "report-1");
    assert_eq!(v["items"][0]["detections"][0]["action"], "allow");
}
#[test]
fn noncanonical_dashboard_uuid_does_not_poison_a_valid_result_batch() {
    let (server, sandbox) = setup(|_| {});
    seed_event(&server);
    let item = fixtures::runtime::session_item("uuid-shared", "A & B", "session-1");
    let mut session = fixtures::runtime::session("session-1");
    session["session_actions"][0]["scan_id"] = json!(SCAN);
    let at = String::from_utf8(
        std::process::Command::new("date")
            .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    session["session_actions"][0]["start_time"] = json!(at.trim());
    let mut other = session["session_actions"][0].clone();
    other["scan_id"] = json!(SCAN.replace('-', ""));
    session["session_actions"]
        .as_array_mut()
        .unwrap()
        .push(other);
    session["pagination"]["total_items"] = json!(2);
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items":[item],"pagination":{"limit":25,"skip":0,"total_items":1}}),
        ),
    );
    server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "query",
            "--interval",
            "24",
            "--unit",
            "hours",
            "--filter",
            "threat",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    let v: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    for r in server.requests_for(Route::ScanResults) {
        assert_eq!(r.query_value("scan_ids"), Some(SCAN));
    }
}
#[test]
fn session_pages_are_fair_across_application_buckets() {
    let (server, sandbox) = setup(|_| {});
    server.runtime(|data| {
        data.sessions = (0..10)
            .map(|n| fixtures::runtime::session_item("app-a", "A", &format!("a-{n}")))
            .chain(std::iter::once(fixtures::runtime::session_item(
                "app-b", "B", "b-1",
            )))
            .collect();
    });
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "query",
            "--interval",
            "24",
            "--unit",
            "hours",
            "--max-pages",
            "2",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    let reads = server.rx_requests(Rx::Session);
    assert_eq!(reads.len(), 2);
    // Concurrent reads may reach the server in either order; both application
    // buckets must receive a page before the aggregate page allowance is spent.
    let apps: std::collections::BTreeSet<_> = reads
        .iter()
        .filter_map(|r| r.query_value("app_id"))
        .collect();
    assert_eq!(apps, std::collections::BTreeSet::from(["app-a", "app-b"]));
}
#[test]
fn report_lookup_preserves_priority_order_under_request_exhaustion() {
    let (server, sandbox) = setup(|_| {});
    server.set_default(Route::ScanReports, Planned::json(200, &json!([])));
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "reports",
            "--report-id",
            "z",
            "--report-id",
            "y",
            "--report-id",
            "x",
            "--report-id",
            "w",
            "--report-id",
            "v",
            "--report-id",
            "a",
            "--max-requests",
            "1",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    assert_eq!(
        server.requests_for(Route::ScanReports)[0].query_value("report_ids"),
        Some("z,y,x,w,v")
    );
}

#[test]
fn live_signal_report_bundles_masked_details_and_strict_gaps() {
    let (server, sandbox) = setup(|_| {});
    seed_event(&server);
    server.set_default(
        Route::Profiles,
        Planned::json(
            200,
            &json!({"ai_profiles":[fixtures::runtime::directional_profile()],"next_offset":0}),
        ),
    );
    let path = sandbox.tmp().join("brief");
    let out = run(
        &cli(),
        &sandbox,
        &["runtime", "report", "--bundle-dir", path.to_str().unwrap()],
        &RunOptions::new(),
    );
    out.assert_success();
    assert!(out.stdout.is_empty());
    let html = std::fs::read_to_string(path.join("report.html")).unwrap();
    let json = std::fs::read_to_string(path.join("evidence.json")).unwrap();
    assert!(html.contains("Synthetic prompt example"));
    assert!(html.contains("allow") && html.contains("malicious"));
    assert!(!html.contains("SYNTHETIC-DLP-MATCH") && !json.contains("SYNTHETIC-DLP-MATCH"));
    assert!(html.contains("tenant-wide current inventory"));
    assert!(html.contains("<table id=\"profile-enablement\""));
    server.assert_no_unknown_routes();
    assert!(server.requests().iter().all(|r| !matches!(
        r.route,
        Route::ScanSync | Route::ScanAsync | Route::Runtime(Rx::ScanLogs | Rx::ScanContent)
    )));
    server.set_default(
        Route::Profiles,
        Planned::json(503, &json!({"message":"PRIVATE-ERROR"})),
    );
    let strict = sandbox.tmp().join("strict");
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "report",
            "--strict",
            "--bundle-dir",
            strict.to_str().unwrap(),
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    assert!(strict.join("evidence.json").is_file());
    assert!(!out.stderr.contains("PRIVATE-ERROR") && !out.stderr.contains('✗'));
    // Explicitly omitting detail sampling is not an incomplete detail source.
    let omitted = sandbox.tmp().join("omitted");
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "report",
            "--max-reports",
            "0",
            "--bundle-dir",
            omitted.to_str().unwrap(),
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(omitted.join("evidence.json")).unwrap())
            .unwrap();
    assert_eq!(saved["sampling"]["limit"], 0);
    assert!(
        saved["sampling"]["selected_report_ids"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn report_required_sources_distinguish_empty_success_from_missing_scanner() {
    let (server, sandbox) = setup(|_| {});
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items":[],"pagination":{"limit":25,"skip":0,"total_items":0}}),
        ),
    );
    let out = run(
        &cli(),
        &sandbox,
        &["runtime", "report", "--strict", "--output-file", "-"],
        &RunOptions::new(),
    );
    out.assert_success();
    assert!(
        out.stdout
            .contains("application-telemetry (supplemental)</td><td>unavailable")
    );
    assert!(!out.stderr.contains("source(s) incomplete"));
    let (server, sandbox) = setup(|c| {
        c.remove("airsApiKey");
        c.remove("airsApiToken");
    });
    seed_event(&server);
    for (strict, code) in [(false, 0), (true, 1)] {
        let mut args = vec!["runtime", "report", "--output-file", "-"];
        if strict {
            args.push("--strict");
        }
        let out = run(&cli(), &sandbox, &args, &RunOptions::new());
        out.assert_code(code);
        assert!(out.stdout.contains("no usable application-bound scanner"));
        assert!(out.stdout.contains("bind-default"));
        assert!(out.stdout.contains("Compare configured protections"));
        assert!(!out.stdout.contains("Synthetic prompt example"));
    }
    assert!(
        server
            .requests()
            .iter()
            .all(|r| !matches!(r.route, Route::ScanResults | Route::ScanReports))
    );
}

#[test]
fn control_normalization_cannot_expose_dlp_values_in_other_snippet_families() {
    let (server, sandbox) = setup(|_| {});
    let sensitive = "SYNTHETIC-CONTROL-CANARY-A\nSYNTHETIC-CONTROL-CANARY-B\u{1b}";
    let normalized: String = sensitive.chars().filter(|c| !c.is_control()).collect();
    let mut r = report();
    r["detection_results"][0]["result_detail"]["dlp_snippets"]["snippets"] = json!([sensitive]);
    r["detection_results"][0]["result_detail"]["pi_snippets"] = json!([sensitive, normalized]);
    server.set_default(Route::ScanReports, Planned::json(200, &json!([r])));
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "reports",
            "--report-id",
            "report-1",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    assert!(!out.stdout.contains("SYNTHETIC-CONTROL-CANARY"));
}

#[test]
fn malformed_detector_semantics_never_normalize_to_allow_or_malicious() {
    let (server, sandbox) = setup(|_| {});
    let mut r = report();
    r["detection_results"][0]["action"] = json!("all\now");
    r["detection_results"][0]["verdict"] = json!("ma\nlicious");
    r["detection_results"][0]["detection_service"] = json!("p\ni");
    r["detection_results"][0]["data_type"] = json!("pro\nmpt");
    server.set_default(Route::ScanReports, Planned::json(200, &json!([r])));
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "reports",
            "--report-id",
            "report-1",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    let value: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(value["items"][0]["detections"][0]["action"], "unknown");
    assert_eq!(value["items"][0]["detections"][0]["verdict"], "unknown");
    assert_eq!(
        value["items"][0]["detections"][0]["detection_service"],
        "unknown"
    );
    assert_eq!(value["items"][0]["detections"][0]["data_type"], "unknown");
}

#[test]
fn report_configuration_is_not_starved_by_hung_session_walk_and_details_use_concurrency() {
    let (server, sandbox) = setup(|_| {});
    let items: Vec<_> = (0..4)
        .map(|n| fixtures::runtime::session_item("uuid-shared", "A & B", &format!("session-{n}")))
        .collect();
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items":items,"pagination":{"limit":25,"skip":0,"total_items":4}}),
        ),
    );
    server.set_default(Route::Runtime(Rx::Session), Planned::hang());
    let dest = sandbox.tmp().join("scheduled");
    let at = std::time::Instant::now();
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "report",
            "--strict",
            "--timeout",
            "2",
            "--bundle-dir",
            dest.to_str().unwrap(),
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    // Room for process start-up on a contended runner; a missed deadline hangs to 30 s.
    assert!(at.elapsed() < std::time::Duration::from_secs(15));
    let e: Value =
        serde_json::from_str(&std::fs::read_to_string(dest.join("evidence.json")).unwrap())
            .unwrap();
    for name in ["profiles", "registered-apps", "applications"] {
        let source = e["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == name)
            .unwrap();
        assert_eq!(source["status"], "complete", "{name}");
        assert!(source["pages"].as_u64().unwrap() > 0);
    }
    assert_eq!(server.requests_for(Route::Runtime(Rx::Session)).len(), 4);
    assert_eq!(
        e["budgets"]["attempts"].as_u64(),
        Some(server.request_count() as u64)
    );
    server.assert_no_unknown_routes();
}
#[test]
fn report_serial_configuration_gives_other_sources_time_after_one_hangs() {
    let (server, sandbox) = setup(|_| {});
    server.set_default(Route::Profiles, Planned::hang());
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items":[],"pagination":{"limit":25,"skip":0,"total_items":0}}),
        ),
    );
    let dest = sandbox.tmp().join("fair");
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "report",
            "--strict",
            "--concurrency",
            "1",
            "--timeout",
            "3",
            "--bundle-dir",
            dest.to_str().unwrap(),
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    let e: Value =
        serde_json::from_str(&std::fs::read_to_string(dest.join("evidence.json")).unwrap())
            .unwrap();
    for name in ["registered-apps", "applications", "sessions"] {
        assert_eq!(
            e["sources"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["name"] == name)
                .unwrap()["status"],
            "complete",
            "{name}"
        );
    }
    server.assert_no_unknown_routes();
}

#[test]
fn report_metadata_findings_survive_disabled_or_rate_limited_examples() {
    let (server, sandbox) = setup(|_| {});
    seed_event(&server);
    let mut baseline = None;
    for (name, cap) in [("disabled", "0"), ("rate-limited", "5")] {
        server.clear_requests();
        server.set_default(
            Route::ScanReports,
            Planned::json(429, &json!({"message":"PRIVATE-REPORT-LIMIT"})).with_retry_after(3600),
        );
        let dest = sandbox.tmp().join(name);
        let out = run(
            &cli(),
            &sandbox,
            &[
                "runtime",
                "report",
                "--strict",
                "--max-reports",
                cap,
                "--bundle-dir",
                dest.to_str().unwrap(),
            ],
            &RunOptions::new(),
        );
        out.assert_success();
        let e: Value =
            serde_json::from_str(&std::fs::read_to_string(dest.join("evidence.json")).unwrap())
                .unwrap();
        assert_eq!(e["rules_version"], 2);
        let packet = std::fs::read_to_string(dest.join("review.md")).unwrap();
        let block: String = packet
            .split("## Evidence index\n\n")
            .nth(1)
            .unwrap()
            .lines()
            .skip(1)
            .take_while(|line| !line.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n");
        let findings: Value = serde_json::from_str(&block).unwrap();
        let keys: Vec<_> = findings
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                (
                    f["finding_id"].clone(),
                    f["event_count"].clone(),
                    f["distinct_scans"].clone(),
                    f["metadata_observations"].clone(),
                )
            })
            .collect();
        assert!(!keys.is_empty());
        if let Some(expected) = &baseline {
            assert_eq!(&keys, expected);
        } else {
            baseline = Some(keys);
        }
        assert!(
            e["events"]
                .as_array()
                .unwrap()
                .iter()
                .all(|event| event["detections"].as_array().unwrap().is_empty())
        );
        assert_eq!(
            server.requests_for(Route::ScanReports).len(),
            if cap == "0" { 0 } else { 1 }
        );
        let html = std::fs::read_to_string(dest.join("report.html")).unwrap();
        let packet = std::fs::read_to_string(dest.join("review.md")).unwrap();
        assert!(html.contains("Metadata observation:"));
        assert!(html.contains("prompt-injection"));
        assert!(packet.contains("metadata_evidence"));
        assert!(!html.contains("PRIVATE-REPORT-LIMIT"));
        if cap != "0" {
            let source = e["sources"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["name"] == "scan-reports")
                .unwrap();
            assert_eq!(source["required"], false);
            assert_eq!(source["pages"], 1);
            assert!(
                source["notes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|n| n.as_str().unwrap().contains("429"))
            );
        }
    }
    server.assert_no_unknown_routes();
}

#[test]
fn report_examples_default_to_five_with_hard_ten_limit_and_two_batches() {
    for cap in [None, Some("10")] {
        let (server, sandbox) = setup(|_| {});
        seed_event(&server);
        let mut session = fixtures::runtime::session("session-1");
        let base = session["session_actions"][0].clone();
        let at = std::process::Command::new("date")
            .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
            .output()
            .unwrap();
        let at = String::from_utf8(at.stdout).unwrap();
        let scans: Vec<_> = (0..15)
            .map(|i| format!("00000000-0000-4000-8000-{i:012x}"))
            .collect();
        session["session_actions"] = json!(
            scans
                .iter()
                .map(|scan| {
                    let mut row = base.clone();
                    row["scan_id"] = json!(scan);
                    row["start_time"] = json!(at.trim());
                    row
                })
                .collect::<Vec<_>>()
        );
        session["pagination"]["total_items"] = json!(15);
        server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
        for (batch, chunk) in scans.chunks(5).enumerate() {
            let rows: Vec<_> = chunk
                .iter()
                .enumerate()
                .map(|(i, scan)| {
                    let mut result = fixtures::scan_block("monitor");
                    result["scan_id"] = json!(scan);
                    result["session_id"] = json!("session-1");
                    result["report_id"] = json!(format!("example-{}", batch * 5 + i));
                    result["action"] = json!(if i % 2 == 0 { "allow" } else { "block" });
                    json!({"scan_id":scan,"req_id":0,"status":"COMPLETED","result":result})
                })
                .collect();
            server.push_once(Route::ScanResults, Planned::json(200, &json!(rows)));
        }
        server.set_default(Route::ScanReports, Planned::json(200, &json!([])));
        let dest = sandbox.tmp().join("examples");
        let mut args = vec![
            "runtime",
            "report",
            "--strict",
            "--bundle-dir",
            dest.to_str().unwrap(),
        ];
        if let Some(cap) = cap {
            args.extend(["--max-reports", cap]);
        }
        let out = run(&cli(), &sandbox, &args, &RunOptions::new());
        out.assert_success();
        let e: Value =
            serde_json::from_str(&std::fs::read_to_string(dest.join("evidence.json")).unwrap())
                .unwrap();
        let expected = if cap.is_some() { 10 } else { 5 };
        assert_eq!(e["sampling"]["limit"], expected);
        let ids = e["sampling"]["selected_report_ids"].as_array().unwrap();
        assert_eq!(ids.len(), expected);
        let actions: std::collections::BTreeSet<_> = e["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| {
                event["report_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|id| ids.contains(id))
            })
            .map(|event| event["action"].as_str().unwrap())
            .collect();
        assert_eq!(
            actions,
            std::collections::BTreeSet::from(["allow", "block"])
        );
        let reads = server.requests_for(Route::ScanReports);
        assert_eq!(reads.len(), expected / 5);
        let sent: std::collections::BTreeSet<_> = reads
            .iter()
            .flat_map(|r| r.query_value("report_ids").unwrap().split(','))
            .collect();
        assert_eq!(sent.len(), expected);
        assert_eq!(
            e["budgets"]["attempts"].as_u64(),
            Some(server.request_count() as u64)
        );
        server.assert_no_unknown_routes();
    }
}

#[test]
fn direct_reports_stop_on_rate_limit_and_reject_more_than_ten_before_tenant() {
    let (server, sandbox) = setup(|_| {});
    server.set_default(
        Route::ScanReports,
        Planned::json(429, &json!({"message":"PRIVATE-LIMIT"})).with_retry_after(3600),
    );
    let ids: Vec<_> = (0..11).map(|i| format!("example-{i}")).collect();
    let mut args = vec!["runtime", "scan-logs", "reports", "--output", "json"];
    for id in ids.iter().take(10) {
        args.extend(["--report-id", id]);
    }
    let at = std::time::Instant::now();
    let out = run(&cli(), &sandbox, &args, &RunOptions::new());
    out.assert_code(1);
    // Room for process start-up on a contended runner; a missed deadline hangs to 30 s.
    assert!(at.elapsed() < std::time::Duration::from_secs(15));
    assert_eq!(server.requests_for(Route::ScanReports).len(), 1);
    assert!(!out.stdout.contains("PRIVATE-LIMIT") && !out.stderr.contains("PRIVATE-LIMIT"));
    args.extend(["--report-id", &ids[10]]);
    let before = server.request_count();
    let out = run(&cli(), &Sandbox::new(), &args, &RunOptions::new());
    out.assert_code(2);
    assert!(out.stderr.contains("at most 10 distinct report IDs"));
    assert_eq!(server.request_count(), before);
}

#[test]
fn missing_scan_results_recover_only_exact_transaction_metadata_without_fabricating_outcomes() {
    for mismatch in [
        None,
        Some("application_id"),
        Some("application_name"),
        Some("session_id"),
        Some("scan_id"),
        Some("scan_sub_req_id"),
        Some("absent-sub-request"),
        Some("timeout"),
    ] {
        let (server, sandbox) = setup(|_| {});
        seed_event(&server);
        server.set_default(Route::ScanResults, Planned::json(200, &json!([])));
        let mut transaction = fixtures::runtime::transaction("session-1", SCAN, 0);
        transaction["status"] = json!("passed");
        transaction["profile_name"] = json!("Recovered profile");
        transaction["timeout"] = json!(false);
        transaction["detection_type_violation_breakdown"] =
            json!([{"detection_type":"pi","violation_breakdown":{"total":2}}]);
        transaction["action"] = json!("allow");
        transaction["category"] = json!("benign");
        transaction["report_id"] = json!("DO-NOT-INFER-REPORT");
        if mismatch == Some("timeout") {
            transaction["timeout"] = json!(true);
        } else if mismatch == Some("absent-sub-request") {
            transaction
                .as_object_mut()
                .unwrap()
                .remove("scan_sub_req_id");
        } else if let Some(key) = mismatch {
            transaction[key] = if key == "scan_sub_req_id" {
                json!(1)
            } else {
                json!("wrong-identity")
            };
        }
        server.set_default(
            Route::Runtime(Rx::SessionTransaction),
            Planned::json(200, &transaction),
        );
        let dest = sandbox.tmp().join("fallback");
        let out = run(
            &cli(),
            &sandbox,
            &[
                "runtime",
                "report",
                "--strict",
                "--bundle-dir",
                dest.to_str().unwrap(),
            ],
            &RunOptions::new(),
        );
        out.assert_code(1);
        let saved = std::fs::read_to_string(dest.join("evidence.json")).unwrap();
        let e: Value = serde_json::from_str(&saved).unwrap();
        let event = &e["events"][0];
        assert_eq!(event["verdict"], "unknown");
        assert_eq!(event["action"], "unknown");
        assert!(event["report_ids"].as_array().unwrap().is_empty());
        assert!(event["detections"].as_array().unwrap().is_empty());
        assert!(!saved.contains("DO-NOT-INFER-REPORT"));
        assert_eq!(server.requests_for(Route::ScanReports).len(), 0);
        let reads = server.rx_requests(Rx::SessionTransaction);
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0].query_value("scan_sub_req_id"), Some("0"));
        let sources = e["sources"].as_array().unwrap();
        assert_ne!(
            sources
                .iter()
                .find(|s| s["name"] == "scan-results")
                .unwrap()["status"],
            "complete"
        );
        let fallback = sources
            .iter()
            .find(|s| s["name"] == "session-transactions")
            .unwrap();
        if mismatch.is_none() || mismatch == Some("timeout") {
            assert_eq!(event["profile"], "Recovered profile");
            assert_eq!(
                event["detector_hints"],
                if mismatch.is_none() {
                    json!(["unknown/pi"])
                } else {
                    json!([])
                }
            );
            assert_eq!(fallback["records"], 1);
            let html = std::fs::read_to_string(dest.join("report.html")).unwrap();
            if mismatch.is_none() {
                assert!(html.contains("prompt-injection"));
                assert!(html.contains("Scan enforcement: unknown"));
            } else {
                assert!(!html.contains("Scan enforcement: allow"));
            }
        } else {
            assert_eq!(event["profile"], Value::Null);
            assert!(event["detector_hints"].as_array().unwrap().is_empty());
            assert_eq!(fallback["records"], 0);
        }
        server.assert_no_unknown_routes();
    }
}

#[test]
fn large_slow_session_inventory_keeps_configuration_and_enforces_aggregate_and_fallback_caps() {
    use std::time::Duration;
    let (server, sandbox) = setup(|_| {});
    server.runtime(|data| {
        data.sessions = (0..258)
            .map(|i| {
                fixtures::runtime::session_item("uuid-shared", "A & B", &format!("session-{i}"))
            })
            .collect()
    });
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::builtin().with_delay(Duration::from_millis(100)),
    );
    server.set_default(
        Route::Runtime(Rx::Session),
        Planned::builtin().with_delay(Duration::from_millis(80)),
    );
    let dest = sandbox.tmp().join("large");
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "report",
            "--strict",
            "--timeout",
            "5",
            "--bundle-dir",
            dest.to_str().unwrap(),
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    let e: Value =
        serde_json::from_str(&std::fs::read_to_string(dest.join("evidence.json")).unwrap())
            .unwrap();
    let sources = e["sources"].as_array().unwrap();
    for name in ["profiles", "registered-apps", "applications", "sessions"] {
        assert_eq!(
            sources.iter().find(|s| s["name"] == name).unwrap()["status"],
            "complete",
            "{name}"
        );
    }
    assert_eq!(
        sources.iter().find(|s| s["name"] == "sessions").unwrap()["records"],
        258
    );
    let details = sources
        .iter()
        .find(|s| s["name"] == "session-details")
        .unwrap();
    assert_eq!(details["status"], "partial");
    assert_eq!(details["records"], 40);
    assert_eq!(server.rx_requests(Rx::Session).len(), 40);
    assert_eq!(server.rx_requests(Rx::SessionTransaction).len(), 10);
    assert_eq!(server.requests_for(Route::ScanReports).len(), 0);
    assert_eq!(
        e["budgets"]["attempts"].as_u64(),
        Some(server.request_count() as u64)
    );
    server.assert_no_unknown_routes();
}

#[test]
fn transaction_fallback_deadline_cancels_hung_reads_without_spending_report_requests() {
    let (server, sandbox) = setup(|_| {});
    seed_event(&server);
    server.set_default(Route::ScanResults, Planned::json(200, &json!([])));
    server.set_default(Route::Runtime(Rx::SessionTransaction), Planned::hang());
    let dest = sandbox.tmp().join("bounded-fallback");
    let at = std::time::Instant::now();
    let out = run(
        &cli(),
        &sandbox,
        &[
            "runtime",
            "report",
            "--strict",
            "--timeout",
            "30",
            "--bundle-dir",
            dest.to_str().unwrap(),
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    // The fallback deadline is a tenth of --timeout (3 s here): it must cancel the hung
    // transaction read well before the 30 s overall timeout, with start-up room.
    assert!(at.elapsed() < std::time::Duration::from_secs(15));
    let e: Value =
        serde_json::from_str(&std::fs::read_to_string(dest.join("evidence.json")).unwrap())
            .unwrap();
    let s = e["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "session-transactions")
        .unwrap();
    assert_eq!(s["status"], "unavailable");
    assert!(
        s["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().contains("deadline"))
    );
    assert_eq!(server.rx_requests(Rx::SessionTransaction).len(), 1);
    assert_eq!(server.requests_for(Route::ScanReports).len(), 0);
    assert_eq!(e["events"][0]["action"], "unknown");
    assert_eq!(
        e["budgets"]["attempts"].as_u64(),
        Some(server.request_count() as u64)
    );
    server.assert_no_unknown_routes();
}
