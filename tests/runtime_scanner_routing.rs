//! Credential ownership and recovery are tested only against guarded loopback APIs.
use airs_test_harness::{Cli, FixtureServer, Planned, Route, RunOptions, Sandbox, fixtures, run};
use serde_json::{Value, json};
use std::{fs, path::Path};
const KEY_A: &str = "FAKE-NAMED-SCANNER-A";
const KEY_B: &str = "FAKE-NAMED-SCANNER-B";
const SCAN_A: &str = "00000000-0000-4000-8000-0000000000a1";
const SCAN_B: &str = "00000000-0000-4000-8000-0000000000b2";
fn cli() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}
fn options() -> RunOptions {
    RunOptions::new().secret(KEY_A).secret(KEY_B)
}
fn configure(s: &Sandbox, name: &str, app: &str, endpoint: &str, key: &str) {
    let input = format!(
        "applicationId = {}\nendpoint = {}\napiKey = {}\n",
        json!(app),
        json!(endpoint),
        json!(key)
    );
    run(
        &cli(),
        s,
        &[
            "tenant",
            "scanners",
            "set",
            "prod",
            name,
            "--stdin",
            "--replace",
        ],
        &options()
            .secret(key)
            .allow_writes_under("store")
            .stdin(input),
    )
    .assert_success();
}
fn timestamp() -> String {
    String::from_utf8(
        std::process::Command::new("date")
            .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .into()
}
fn response(scan: &str, session: &str, report: &str, action: &str) -> Value {
    let mut result = fixtures::scan_block("monitor");
    result["scan_id"] = json!(scan);
    result["report_id"] = json!(report);
    result["session_id"] = json!(session);
    result["action"] = json!(action);
    json!([{"scan_id":scan,"req_id":0,"status":"COMPLETED","result":result}])
}
fn report(scan: &str, session: &str, id: &str, action: &str) -> Value {
    json!([{"scan_id":scan,"session_id":session,"report_id":id,"req_id":0,
        "detection_results":[{"data_type":"prompt","detection_service":"prompt_injection","verdict":"malicious","action":action,
            "result_detail":{"pi_snippets":[format!("Synthetic example echoing {KEY_A} and {KEY_B}")]}}]}])
}
fn routed(unbound: bool, rate_limit: bool) -> (FixtureServer, FixtureServer, Sandbox) {
    let a = FixtureServer::start();
    a.seed_runtime();
    let b = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&a, "prod", "100");
    configure(&s, "alpha", "app-a", &a.url("/scan"), KEY_A);
    configure(&s, "beta", "app-b", &b.url("/scan"), KEY_B);
    let mut sessions = vec![];
    let now = timestamp();
    for (app, name, session, scan) in [
        ("app-a", "Alpha", "s-a", SCAN_A),
        ("app-b", "Beta", "s-b", SCAN_B),
        (
            "app-c",
            "No credential",
            "s-c",
            "00000000-0000-4000-8000-0000000000c3",
        ),
    ] {
        if app == "app-c" && !unbound {
            continue;
        }
        sessions.push(fixtures::runtime::session_item(app, name, session));
        let mut detail = fixtures::runtime::session(session);
        detail["application"] = json!({"id":app,"name":name});
        let event = &mut detail["session_actions"][0];
        event["application_id"] = json!(app);
        event["application_name"] = json!(name);
        event["scan_id"] = json!(scan);
        event["start_time"] = json!(now);
        a.runtime(|data| {
            data.session_records
                .insert((app.into(), name.into(), session.into()), detail);
        });
    }
    a.runtime(|data| data.sessions = sessions);
    a.set_default(
        Route::ScanResults,
        Planned::json(200, &response(SCAN_A, "s-a", "report-a", "allow"))
            .with_required_header("x-pan-token", KEY_A),
    );
    b.set_default(
        Route::ScanResults,
        Planned::json(200, &response(SCAN_B, "s-b", "report-b", "block"))
            .with_required_header("x-pan-token", KEY_B),
    );
    let details = if rate_limit {
        Planned::empty_json(429)
    } else {
        Planned::json(200, &report(SCAN_A, "s-a", "report-a", "allow"))
    };
    a.set_default(
        Route::ScanReports,
        details.with_required_header("x-pan-token", KEY_A),
    );
    b.set_default(
        Route::ScanReports,
        Planned::json(200, &report(SCAN_B, "s-b", "report-b", "block"))
            .with_required_header("x-pan-token", KEY_B),
    );
    (a, b, s)
}

#[test]
fn report_routes_each_app_to_its_key_and_keeps_unbound_management_evidence() {
    let (a, b, s) = routed(true, false);
    let bundle = s.tmp().join("brief");
    let out = run(
        &cli(),
        &s,
        &[
            "runtime",
            "report",
            "--strict",
            "--bundle-dir",
            bundle.to_str().unwrap(),
        ],
        &options(),
    );
    out.assert_code(1);
    let bytes = fs::read_to_string(bundle.join("evidence.json")).unwrap();
    assert!(!bytes.contains(KEY_A));
    assert!(!bytes.contains(KEY_B));
    let evidence: Value = serde_json::from_str(&bytes).unwrap();
    let events = evidence["events"].as_array().unwrap();
    assert_eq!(events.len(), 3);
    for (app, verdict, action) in [
        ("app-a", "malicious", "allow"),
        ("app-b", "malicious", "block"),
        ("app-c", "unknown", "unknown"),
    ] {
        let event = events
            .iter()
            .find(|e| e["identity"]["app_id"] == app)
            .unwrap();
        assert_eq!(event["verdict"], verdict);
        assert_eq!(event["action"], action);
    }
    for (server, key) in [(&a, KEY_A), (&b, KEY_B)] {
        for route in [Route::ScanResults, Route::ScanReports] {
            let requests = server.requests_for(route);
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].header("x-pan-token"), Some(key));
            assert!(requests[0].header("authorization").is_none());
            assert!(requests[0].header("x-tsg-id").is_none());
        }
        server.assert_no_unknown_routes();
    }
    assert!(
        b.requests()
            .iter()
            .all(|r| matches!(r.route, Route::ScanResults | Route::ScanReports))
    );
    assert_eq!(
        evidence["sampling"]["selected_report_ids"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let source = evidence["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "scan-reports")
        .unwrap();
    assert_eq!(source["pages"], 2);
    assert_eq!(source["required"], false);
    let results = evidence["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "scan-results")
        .unwrap();
    assert_eq!(results["pages"], 2);
    for app in ["app-a", "app-b"] {
        assert!(
            results["notes"].as_array().unwrap().iter().any(|note| note
                .as_str()
                .is_some_and(|note| note
                    .starts_with(&format!("Application {app}: requested IDs 1;"))
                    && note.contains("actual sends 1")))
        );
    }
}

#[test]
fn unbound_action_violations_keep_positive_metadata_and_aggregate_sessions_visible() {
    let (a, b, s) = routed(true, false);
    a.runtime(|data| {
        let mut item = fixtures::runtime::session_item("app-c", "No credential", "s-c");
        item["detection_type_violation_breakdown"] = json!([{"detection_type":"tc","violation_breakdown":{"total":1}}]);
        data.sessions = vec![item];
        let detail = data.session_records.get_mut(&("app-c".into(), "No credential".into(), "s-c".into())).unwrap();
        let mut second = detail["session_actions"][0].clone();
        detail["session_actions"][0]["detection_type_violation_breakdown"] = json!([{"detection_type":"pi","violation_breakdown":{"high":1},"action":"block","data_type":"prompt"}]);
        second["scan_sub_req_id"] = json!(1);
        second["detection_type_violation_breakdown"] = json!([{"detection_type":"dlp","violation_breakdown":{"total":2},"action":"allow","data_type":"response"}]);
        detail["session_actions"].as_array_mut().unwrap().push(second);
        detail["pagination"]["total_items"] = json!(2);
        let mut overview = fixtures::runtime::overview_item("app-c", "No credential");
        overview["sessions_total"] = json!(256);
        overview["sessions_violated"] = json!(33);
        data.apps = vec![overview];
    });
    let bundle = s.tmp().join("management-positive");
    run(
        &cli(),
        &s,
        &[
            "runtime",
            "report",
            "--strict",
            "--bundle-dir",
            bundle.to_str().unwrap(),
        ],
        &options(),
    )
    .assert_code(1);
    let evidence: Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence.json")).unwrap()).unwrap();
    let events = evidence["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| e["action"] == "unknown"
        && e["verdict"] == "unknown"
        && e["report_ids"] == json!([])
        && e["detections"] == json!([])));
    let hints: Vec<_> = events
        .iter()
        .flat_map(|e| e["detector_hints"].as_array().unwrap())
        .collect();
    assert!(hints.contains(&&json!("unknown/pi")));
    assert!(hints.contains(&&json!("unknown/dlp")));
    assert!(
        !hints.contains(&&json!("unknown/tc")),
        "Overview evidence is session-level, not a scan join"
    );
    let html = fs::read_to_string(bundle.join("report.html")).unwrap();
    let summary = html.split("id=\"findings\"").next().unwrap();
    assert!(summary.contains("Management session activity"));
    assert!(summary.contains("Violated sessions"));
    assert!(summary.contains(">33</td>"));
    assert!(html.contains("Detector-positive metadata observations"));
    for server in [&a, &b] {
        assert_eq!(server.route_count(Route::ScanResults), 0);
        assert_eq!(server.route_count(Route::ScanReports), 0);
    }
}

#[test]
fn collection_diagnostics_distinguish_http_failure_empty_success_and_pending_rows() {
    for pending in [false, true] {
        let (a, b, s) = routed(false, false);
        a.set_default(
            Route::ScanResults,
            if pending {
                Planned::json(
                    200,
                    &json!([{"scan_id":SCAN_A,"req_id":0,"status":"PENDING"}]),
                )
            } else {
                Planned::empty_json(500)
            },
        );
        b.set_default(Route::ScanResults, Planned::json(200, &json!([])));
        let bundle = s.tmp().join("diagnostics");
        run(
            &cli(),
            &s,
            &[
                "runtime",
                "report",
                "--strict",
                "--bundle-dir",
                bundle.to_str().unwrap(),
            ],
            &options(),
        )
        .assert_code(1);
        let evidence: Value =
            serde_json::from_slice(&fs::read(bundle.join("evidence.json")).unwrap()).unwrap();
        let source = evidence["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == "scan-results")
            .unwrap();
        assert_eq!(source["pages"], 2);
        let notes = source["notes"].as_array().unwrap();
        let app = |id: &str| {
            notes
                .iter()
                .filter_map(Value::as_str)
                .find(|note| note.starts_with(&format!("Application {id}: requested IDs")))
                .unwrap()
        };
        assert!(app("app-a").contains(if pending {
            "pending rows 1"
        } else {
            "failed SDK reads 1"
        }));
        assert!(app("app-a").contains("actual sends 1"));
        assert!(
            app("app-b")
                .contains("empty successful responses 1; failed SDK reads 0; actual sends 1")
        );
        assert!(
            evidence["events"]
                .as_array()
                .unwrap()
                .iter()
                .all(|e| e["action"] == "unknown" && e["verdict"] == "unknown")
        );
        assert_eq!(a.route_count(Route::ScanResults), 1);
        assert_eq!(b.route_count(Route::ScanResults), 1);
        assert_eq!(
            a.route_count(Route::ScanReports) + b.route_count(Route::ScanReports),
            0
        );
    }
}

#[test]
fn scanner_direction_subsumes_the_same_management_hint_without_double_counting() {
    let (a, _, s) = routed(false, false);
    a.runtime(|data| {
        let detail = data
            .session_records
            .get_mut(&("app-a".into(), "Alpha".into(), "s-a".into()))
            .unwrap();
        detail["session_actions"][0]["detection_type_violation_breakdown"] =
            json!([{"detection_type":"pi","violation_breakdown":{"total":1}}]);
    });
    let bundle = s.tmp().join("dedup-hints");
    run(
        &cli(),
        &s,
        &[
            "runtime",
            "report",
            "--bundle-dir",
            bundle.to_str().unwrap(),
        ],
        &options(),
    )
    .assert_success();
    let evidence: Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence.json")).unwrap()).unwrap();
    let event = evidence["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["identity"]["app_id"] == "app-a")
        .unwrap();
    assert!(
        event["detector_hints"]
            .as_array()
            .unwrap()
            .contains(&json!("unknown/pi"))
    );
    assert!(
        event["detector_hints"]
            .as_array()
            .unwrap()
            .contains(&json!("prompt/injection"))
    );
    let html = fs::read_to_string(bundle.join("report.html")).unwrap();
    assert_eq!(html.matches("<h3>prompt-injection · Alpha</h3>").count(), 1);
    assert!(
        event["sources"]
            .as_array()
            .unwrap()
            .contains(&json!("session-details"))
    );
    assert!(
        event["sources"]
            .as_array()
            .unwrap()
            .contains(&json!("scan-results"))
    );
    assert_eq!(event["action"], "allow");
    assert_eq!(event["verdict"], "malicious");
}

#[test]
fn positive_sessions_get_priority_without_spending_every_page_on_one_application() {
    for cap in [1, 2] {
        let server = FixtureServer::start();
        server.seed_runtime();
        let s = Sandbox::new();
        s.single_tenant(&server, "prod", "100");
        server.runtime(|data| {
            data.sessions = (0..8)
                .map(|n| fixtures::runtime::session_item("app-a", "A", &format!("benign-{n}")))
                .collect();
            let mut late = fixtures::runtime::session_item("app-z", "Z", "z-positive");
            late["violation_breakdown"] = json!({"high":1});
            data.sessions.push(late);
            if cap == 2 {
                let mut within = fixtures::runtime::session_item("app-a", "A", "a-positive");
                within["detection_type_violation_breakdown"] =
                    json!([{"detection_type":"pi","violation_breakdown":{"total":1}}]);
                data.sessions.push(within);
            }
        });
        let cap_text = cap.to_string();
        run(
            &cli(),
            &s,
            &[
                "runtime",
                "scan-logs",
                "query",
                "--interval",
                "24",
                "--unit",
                "hours",
                "--max-pages",
                &cap_text,
                "--concurrency",
                "1",
                "--output",
                "json",
            ],
            &options(),
        )
        .assert_code(1);
        let sessions: std::collections::BTreeSet<_> = server
            .rx_requests(airs_test_harness::Rx::Session)
            .iter()
            .filter_map(|r| r.query_value("session_id").map(str::to_owned))
            .collect();
        assert!(sessions.contains("z-positive"));
        assert_eq!(sessions.len(), cap);
        if cap == 2 {
            assert!(sessions.contains("a-positive"));
        }
    }
}

#[test]
fn one_app_rate_limit_stops_all_later_report_clients_without_failing_strict_metadata() {
    let (a, b, s) = routed(false, true);
    let out = run(
        &cli(),
        &s,
        &["runtime", "report", "--strict", "--output-file", "-"],
        &options(),
    );
    out.assert_success();
    assert_eq!(a.route_count(Route::ScanReports), 1);
    assert_eq!(b.route_count(Route::ScanReports), 0);
    assert_eq!(a.route_count(Route::ScanResults), 1);
    assert_eq!(b.route_count(Route::ScanResults), 1);
    assert!(out.stdout.contains("rate") || out.stdout.contains("429"));
}

#[test]
fn later_application_dlp_match_withholds_earlier_long_example_before_truncation() {
    let (a, b, s) = routed(false, false);
    let sensitive = "Synthetic confidential phrase repeated for privacy coverage. ".repeat(16);
    let mut first = report(SCAN_A, "s-a", "report-a", "allow");
    first[0]["detection_results"][0]["result_detail"]["pi_snippets"] = json!([sensitive]);
    let mut later = report(SCAN_B, "s-b", "report-b", "block");
    later[0]["detection_results"][0]["result_detail"]["dlp_snippets"] =
        json!({"snippets":[sensitive]});
    for (server, key, body) in [(&a, KEY_A, first), (&b, KEY_B, later)] {
        server.set_default(
            Route::ScanReports,
            Planned::json(200, &body).with_required_header("x-pan-token", key),
        );
    }
    let bundle = s.tmp().join("cross-app-privacy");
    run(
        &cli(),
        &s,
        &[
            "runtime",
            "report",
            "--strict",
            "--bundle-dir",
            bundle.to_str().unwrap(),
        ],
        &options(),
    )
    .assert_success();
    for name in ["evidence.json", "report.html"] {
        let artifact = fs::read_to_string(bundle.join(name)).unwrap();
        assert!(!artifact.contains("Synthetic confidential phrase"));
    }
    assert_eq!(a.route_count(Route::ScanReports), 1);
    assert_eq!(b.route_count(Route::ScanReports), 1);
}

#[test]
fn scan_preview_masks_an_unselected_named_credential() {
    let (a, _, s) = routed(false, false);
    a.set_default(
        Route::ScanSync,
        Planned::json(200, &fixtures::scan_allow("monitor")),
    );
    let out = run(
        &cli(),
        &s,
        &[
            "runtime",
            "scan",
            "--profile",
            "monitor",
            "--scanner",
            "alpha",
            KEY_B,
        ],
        &options(),
    );
    out.assert_success();
    assert!(!out.stderr.contains(KEY_B));
    assert_eq!(a.route_count(Route::ScanSync), 1);
}

#[test]
fn direct_selection_never_falls_back_to_flat_or_another_named_credential() {
    let (a, b, s) = routed(false, false);
    let args = [
        "runtime",
        "scan-logs",
        "results",
        "--scan-id",
        SCAN_B,
        "--scanner",
        "beta",
        "--output",
        "json",
    ];
    let out = run(&cli(), &s, &args, &options());
    out.assert_success();
    assert_eq!(
        b.requests_for(Route::ScanResults)[0].header("x-pan-token"),
        Some(KEY_B)
    );
    assert_eq!(a.route_count(Route::ScanResults), 0);
    configure(&s, "beta", "app-b", &b.url("/scan"), "FAKE-WRONG-KEY");
    let out = run(&cli(), &s, &args, &options().secret("FAKE-WRONG-KEY"));
    out.assert_code(1);
    assert_eq!(b.route_count(Route::ScanResults), 2);
    assert_eq!(a.route_count(Route::ScanResults), 0);
    assert!(out.stdout.contains("403"));
    let out = run(
        &cli(),
        &s,
        &[
            "runtime",
            "scan-logs",
            "results",
            "--scan-id",
            SCAN_B,
            "--scanner",
            "missing",
            "--output",
            "json",
        ],
        &options(),
    );
    out.assert_code(1);
    assert_eq!(b.route_count(Route::ScanResults), 2);
}

#[test]
fn named_token_uses_only_bearer_without_flat_key_or_management_headers() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let token = "FAKE-NAMED-BEARER-TOKEN";
    let input = format!(
        "applicationId = \"app-a\"\nendpoint = {}\napiToken = {}\n",
        json!(server.url("/scan")),
        json!(token)
    );
    run(
        &cli(),
        &s,
        &["tenant", "scanners", "set", "prod", "token", "--stdin"],
        &options()
            .secret(token)
            .allow_writes_under("store")
            .stdin(input),
    )
    .assert_success();
    server.set_default(
        Route::ScanResults,
        Planned::json(200, &response(SCAN_A, "s-a", "report-a", "allow"))
            .with_required_header("authorization", &format!("Bearer {token}")),
    );
    run(
        &cli(),
        &s,
        &[
            "runtime",
            "scan-logs",
            "results",
            "--scan-id",
            SCAN_A,
            "--scanner",
            "token",
            "--output",
            "json",
        ],
        &options().secret(token),
    )
    .assert_success();
    let requests = server.requests_for(Route::ScanResults);
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].header("authorization"),
        Some(format!("Bearer {token}").as_str())
    );
    assert!(requests[0].header("x-pan-token").is_none());
    assert!(requests[0].header("x-tsg-id").is_none());
    assert_eq!(server.route_count(Route::Token), 0);
}

#[test]
fn named_bulk_existing_output_error_masks_a_credential_in_the_path() {
    let (a, b, s) = routed(false, false);
    let input = s.tmp().join("synthetic.txt");
    fs::write(&input, "Synthetic prompt\n").unwrap();
    let output = s.tmp().join(format!("{KEY_A}.csv"));
    fs::write(&output, "preserve me\n").unwrap();
    let out = run(
        &cli(),
        &s,
        &[
            "runtime",
            "bulk-scan",
            "--scanner",
            "alpha",
            "--profile",
            "monitor",
            "--file",
            input.to_str().unwrap(),
            "--output-file",
            output.to_str().unwrap(),
        ],
        &options(),
    );
    out.assert_code(1);
    assert!(!out.stderr.contains(KEY_A));
    assert!(out.stderr.contains("withheld"));
    assert_eq!(fs::read_to_string(output).unwrap(), "preserve me\n");
    assert!(a.requests().is_empty());
    assert!(b.requests().is_empty());
}

#[test]
fn named_bulk_and_resume_mask_unused_credential_profile_and_paths() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    configure(
        &s,
        "monitor",
        "app-a",
        &server.url("/scan"),
        "FAKE-APIKEY-100",
    );
    configure(&s, "unused", "app-b", &server.url("/scan"), KEY_B);
    let input = s.tmp().join("synthetic.txt");
    fs::write(&input, "Synthetic prompt\n").unwrap();
    let state_root = s.tmp().join(KEY_B);
    let opts = options().env("XDG_STATE_HOME", state_root.to_str().unwrap());
    for format in ["json", "pretty", "table"] {
        let output = s.tmp().join(format!("{KEY_B}-{format}.csv"));
        let out = run(
            &cli(),
            &s,
            &[
                "runtime",
                "bulk-scan",
                "--scanner",
                "monitor",
                "--profile",
                KEY_B,
                "--file",
                input.to_str().unwrap(),
                "--output-file",
                output.to_str().unwrap(),
                "--output",
                format,
            ],
            &opts,
        );
        out.assert_success();
        assert!(!out.stdout.contains(KEY_B));
        assert!(!out.stderr.contains(KEY_B));
        let state = fs::read_dir(state_root.join("airs-cli/bulk-scans"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.to_string_lossy().ends_with(".bulk-scan.json")
                    && read_state(p)["output"]["path"] == output.to_str().unwrap()
            })
            .unwrap();
        let out = run(
            &cli(),
            &s,
            &[
                "runtime",
                "resume-poll",
                state.to_str().unwrap(),
                "--output",
                format,
            ],
            &opts,
        );
        out.assert_success();
        assert!(!out.stdout.contains(KEY_B));
        assert!(!out.stderr.contains(KEY_B));
    }
    assert_eq!(server.route_count(Route::ScanAsync), 3);
}

#[test]
fn same_scan_in_two_exact_name_buckets_is_not_attributed_by_position() {
    let (a, _, s) = routed(false, false);
    let now = timestamp();
    a.runtime(|data| {
        data.sessions.clear();
        data.session_records.clear();
        for name in ["Alpha one", "Alpha two"] {
            data.sessions
                .push(fixtures::runtime::session_item("app-a", name, "s-a"));
            let mut detail = fixtures::runtime::session("s-a");
            detail["application"] = json!({"id":"app-a","name":name});
            detail["session_actions"][0]["application_id"] = json!("app-a");
            detail["session_actions"][0]["application_name"] = json!(name);
            detail["session_actions"][0]["scan_id"] = json!(SCAN_A);
            detail["session_actions"][0]["start_time"] = json!(now);
            data.session_records
                .insert(("app-a".into(), name.into(), "s-a".into()), detail);
        }
    });
    let bundle = s.tmp().join("ambiguous");
    let out = run(
        &cli(),
        &s,
        &[
            "runtime",
            "report",
            "--strict",
            "--bundle-dir",
            bundle.to_str().unwrap(),
        ],
        &options(),
    );
    out.assert_code(1);
    let evidence: Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence.json")).unwrap()).unwrap();
    let events = evidence["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert!(
        events
            .iter()
            .all(|e| e["action"] == "unknown" && e["verdict"] == "unknown")
    );
    assert_eq!(a.route_count(Route::ScanResults), 1);
    assert_eq!(a.route_count(Route::ScanReports), 0);
}

fn read_state(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn named_only_scan_requires_selection_and_doctor_never_guesses_a_probe() {
    let server = FixtureServer::start();
    server.seed_runtime();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    configure(
        &s,
        "monitor",
        "app-a",
        &server.url("/scan"),
        "FAKE-APIKEY-100",
    );
    let bytes = fs::read_to_string(s.store_tenant_path("prod"))
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("airsApiKey =") && !line.starts_with("airsApiToken ="))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    s.write_tenant_file_raw("prod", bytes.as_bytes(), 0o600);
    server.set_default(
        Route::ScanSync,
        Planned::json(200, &fixtures::scan_allow("monitor"))
            .with_required_header("x-pan-token", "FAKE-APIKEY-100"),
    );
    let out = run(
        &cli(),
        &s,
        &[
            "runtime",
            "scan",
            "--profile",
            "monitor",
            "--scanner",
            "monitor",
            "Synthetic sample",
            "--output",
            "json",
        ],
        &options(),
    );
    out.assert_success();
    assert_eq!(server.route_count(Route::ScanSync), 1);
    let out = run(
        &cli(),
        &s,
        &[
            "runtime",
            "scan",
            "--profile",
            "monitor",
            "Synthetic sample",
        ],
        &options(),
    );
    out.assert_code(1);
    assert!(out.stderr.contains("select a named scanner"));
    assert_eq!(server.route_count(Route::ScanSync), 1);
    let out = run(&cli(), &s, &["doctor", "--output", "json"], &options());
    let doc: Value = serde_json::from_str(&out.stdout).unwrap();
    let checks = doc.as_array().unwrap();
    let credential = checks
        .iter()
        .find(|v| v["name"] == "Scanner credentials")
        .unwrap();
    let probe = checks.iter().find(|v| v["name"] == "Scanner API").unwrap();
    assert_eq!(credential["status"], "pass");
    assert_eq!(probe["status"], "skip");
    assert!(
        probe["detail"]
            .as_str()
            .unwrap()
            .contains("explicit selection")
    );
    assert_eq!(server.route_count(Route::ScanResults), 0);
}

#[test]
fn named_bulk_state_refuses_removed_binding_and_resumes_after_explicit_same_app_rotation() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    configure(
        &s,
        "monitor",
        "app-a",
        &server.url("/scan"),
        "FAKE-APIKEY-100",
    );
    let input = s.tmp().join("prompts.txt");
    fs::write(&input, "Synthetic sample\n").unwrap();
    let output = s.tmp().join("results.csv");
    let state_root = s.tmp().join("state");
    let opts = options().env("XDG_STATE_HOME", state_root.to_str().unwrap());
    server.push_once(Route::ScanResults, Planned::empty_json(500));
    let out = run(
        &cli(),
        &s,
        &[
            "runtime",
            "bulk-scan",
            "--profile",
            "monitor",
            "--scanner",
            "monitor",
            "--file",
            input.to_str().unwrap(),
            "--output-file",
            output.to_str().unwrap(),
        ],
        &opts,
    );
    out.assert_code(1);
    let dir = state_root.join("airs-cli/bulk-scans");
    let state = fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|v| v.path())
        .find(|p| p.to_string_lossy().ends_with(".bulk-scan.json"))
        .unwrap();
    let original = read_state(&state);
    assert_eq!(original["version"], 2);
    assert_eq!(original["scannerReference"]["name"], "monitor");
    assert_eq!(original["scannerReference"]["applicationId"], "app-a");
    assert!(
        !fs::read_to_string(&state)
            .unwrap()
            .contains("FAKE-APIKEY-100")
    );
    let resume_args = ["runtime", "resume-poll", state.to_str().unwrap()];
    for (app, endpoint) in [
        ("app-b", server.url("/scan")),
        ("app-a", server.url("/different")),
    ] {
        configure(&s, "monitor", app, &endpoint, "FAKE-APIKEY-100");
        let calls = server.requests().len();
        let out = run(&cli(), &s, &resume_args, &opts);
        out.assert_code(1);
        assert!(out.stderr.contains("association or endpoint changed"));
        assert_eq!(server.requests().len(), calls);
        assert_eq!(read_state(&state), original);
    }
    run(
        &cli(),
        &s,
        &["tenant", "scanners", "remove", "prod", "monitor", "--force"],
        &options().allow_writes_under("store"),
    )
    .assert_success();
    let calls = server.requests().len();
    let args = ["runtime", "resume-poll", state.to_str().unwrap()];
    let out = run(&cli(), &s, &args, &opts);
    out.assert_code(1);
    assert!(out.stderr.contains("Saved scanner is not configured"));
    assert_eq!(server.requests().len(), calls);
    assert_eq!(read_state(&state), original);
    configure(
        &s,
        "monitor",
        "app-a",
        &server.url("/scan"),
        "FAKE-APIKEY-100",
    );
    let out = run(&cli(), &s, &args, &opts);
    out.assert_success();
    assert!(
        out.stderr
            .contains("historical lookup access is unqualified")
    );
    assert_ne!(
        read_state(&state)["scannerReference"]["revision"],
        original["scannerReference"]["revision"]
    );
    assert_eq!(
        server.route_count(Route::ScanAsync),
        1,
        "resume must not submit again"
    );
}
