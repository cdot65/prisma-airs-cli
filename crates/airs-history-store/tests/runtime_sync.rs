//! Explicit collection uses loopback only, with actual archived work assertions.
use airs_test_harness::{
    Cli, FixtureServer, Planned, Route, RunOptions, Rx, Sandbox, fixtures, run, spawn,
};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
const SCAN: &str = "00000000-0000-4000-8000-0000000000b2";
fn cli() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs-history-driver"))
}
fn fixture() -> (FixtureServer, Sandbox) {
    let server = FixtureServer::start();
    server.seed_runtime();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    sandbox.bind_fixture_scanner("prod", "fixture", "uuid-shared", "key");
    let item = fixtures::runtime::session_item("uuid-shared", "A & B", "session-1");
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items":[item],"pagination":{"limit":25,"skip":0,"total_items":1}}),
        ),
    );
    let mut session = fixtures::runtime::session("session-1");
    session["session_actions"][0]["scan_id"] = json!(SCAN);
    let at = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .unwrap();
    session["session_actions"][0]["start_time"] =
        json!(std::str::from_utf8(&at.stdout).unwrap().trim());
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
    (server, sandbox)
}
fn archive(s: &Sandbox) -> Connection {
    let root = s.tmp().join("history-data/airs-cli/history");
    let status = run(
        &cli(),
        s,
        &[
            "runtime",
            "history",
            "status",
            "--history-dir",
            root.to_str().unwrap(),
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    status.assert_success();
    let v: Value = serde_json::from_str(&status.stdout).unwrap();
    Connection::open_with_flags(
        root.join(v["cloud_id"].as_str().unwrap())
            .join("archive.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap()
}
/// What `airs tenant scanners set prod fixture --replace --stdin` leaves behind: the
/// `fixture` binding replaced by `input` (application, endpoint and key) with a fresh
/// revision. The driver has no tenant commands, so the test writes the file directly.
fn rebind(s: &Sandbox, input: &str) {
    let path = s.store_tenant_path("prod");
    let text = std::fs::read_to_string(&path).unwrap();
    let start = text
        .find("[scanners.\"fixture\"]")
        .or_else(|| text.find("[scanners.fixture]"))
        .expect("fixture binding");
    let end = text[start + 1..]
        .find("\n[")
        .map_or(text.len(), |i| start + 1 + i + 1);
    let mut bytes = format!("{}{}", &text[..start], &text[end..]);
    let fields: toml::Table = input.parse().unwrap();
    let field = |key: &str| airs_runtime_core::store::toml_string(fields[key].as_str().unwrap());
    bytes.push_str(&format!(
        "\n[scanners.fixture]\napplicationId = {}\nendpoint = {}\nnumRetries = 0\nrevision = \"{}\"\napiKey = {}\n",
        field("applicationId"),
        field("endpoint"),
        uuid::Uuid::new_v4(),
        field("apiKey"),
    ));
    s.write_tenant_file_raw("prod", bytes.as_bytes(), 0o600);
}
#[test]
fn ordinary_sync_retains_monitoring_and_work_without_reports_and_reuses_cache() {
    let (server, s) = fixture();
    let out = run(
        &cli(),
        &s,
        &["runtime", "sync", "--output", "json"],
        &RunOptions::new(),
    );
    out.assert_success();
    let v: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(v["observed_events"], 1);
    assert!(server.requests_for(Route::ScanReports).is_empty());
    assert_eq!(server.requests_for(Route::ScanResults).len(), 1);
    let db = archive(&s);
    let management_sends: i64 = db
        .query_row(
            "SELECT coalesce(sum(actual_sends),0) FROM management_reads",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(management_sends as usize, server.requests().len() - 1);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM management_reads WHERE outcome='started'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    let (verdict, action): (String, String) = db
        .query_row("SELECT verdict,action FROM events", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((verdict.as_str(), action.as_str()), ("malicious", "allow"));
    let attempt: (i64, String) = db
        .query_row("SELECT ordinal,outcome FROM work_attempts", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(attempt, (1, "returned".into()));
    assert!(
        db.query_row(
            "SELECT count(*) FROM coverage_segments WHERE state='traversed_as_of'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap()
            > 0
    );
    drop(db);
    let out = run(
        &cli(),
        &s,
        &["runtime", "sync", "--output", "json"],
        &RunOptions::new(),
    );
    out.assert_success();
    assert_eq!(server.requests_for(Route::ScanResults).len(), 1);
    assert!(server.requests_for(Route::ScanReports).is_empty());
    let db = archive(&s);
    assert_eq!(
        db.query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM work_attempts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn cached_sync_after_rebinding_never_invents_current_scanner_provenance() {
    let (server, s) = fixture();
    run(&cli(), &s, &["runtime", "sync"], &RunOptions::new()).assert_success();
    let old: String=archive(&s).query_row("SELECT json_extract(fields_json,'$.scanner_endpoint') FROM event_facts WHERE json_extract(fields_json,'$.scanner_endpoint') IS NOT NULL LIMIT 1",[],|r|r.get(0)).unwrap();
    let endpoint = server.url("/different-scanner");
    let input = format!(
        "applicationId='uuid-shared'\nendpoint={}\napiKey='SYNTHETIC_ROTATED_KEY'\n",
        json!(endpoint)
    );
    rebind(&s, &input);
    run(&cli(), &s, &["runtime", "sync"], &RunOptions::new()).assert_success();
    assert_eq!(server.requests_for(Route::ScanResults).len(), 1);
    let db = archive(&s);
    assert_eq!(db.query_row("SELECT count(*) FROM event_facts WHERE json_extract(fields_json,'$.scanner_endpoint') IS NOT NULL AND json_extract(fields_json,'$.scanner_endpoint')!=?1",[old],|r|r.get::<_,i64>(0)).unwrap(),0);
}

#[test]
fn a_report_transport_failure_cannot_inherit_a_results_http_status() {
    let (server, s) = fixture();
    server.set_default(Route::ScanReports, Planned::disconnect());
    run(
        &cli(),
        &s,
        &["runtime", "sync", "--with-examples"],
        &RunOptions::new(),
    )
    .assert_success();
    assert_eq!(server.requests_for(Route::ScanReports).len(), 1);
    let db = archive(&s);
    let status: Option<i64> = db
        .query_row(
            "SELECT status FROM work_attempts WHERE source='reports'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, None);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM service_cooldowns WHERE service='reports'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn five_selected_examples_share_one_transport_and_cached_examples_are_not_refetched() {
    let (server, s) = fixture();
    let mut session = fixtures::runtime::session("session-1");
    let template = session["session_actions"][0].clone();
    let at = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .unwrap();
    let at = std::str::from_utf8(&at.stdout).unwrap().trim();
    let mut actions = vec![];
    let mut results = vec![];
    let mut reports = vec![];
    for n in 1..=5 {
        let scan = format!("00000000-0000-4000-8000-{n:012x}");
        let report = format!("report-{n}");
        let mut action = template.clone();
        action["scan_id"] = json!(scan);
        action["start_time"] = json!(at);
        actions.push(action);
        let mut result = fixtures::scan_block("monitor");
        result["scan_id"] = json!(scan);
        result["report_id"] = json!(report);
        result["session_id"] = json!("session-1");
        result["action"] = json!("allow");
        results.push(json!({"scan_id":scan,"req_id":0,"status":"COMPLETED","result":result}));
        reports.push(json!({"report_id":report,"scan_id":scan,"req_id":0,"session_id":"session-1","detection_results":[{"detection_service":"prompt_injection","data_type":"prompt","verdict":"malicious","action":"allow","result_detail":{"pi_snippets":["Synthetic strategic example"]}}]}));
    }
    session["session_actions"] = json!(actions);
    session["pagination"]["total_items"] = json!(5);
    server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
    server.set_default(Route::ScanResults, Planned::json(200, &json!(results)));
    server.set_default(Route::ScanReports, Planned::json(200, &json!(reports)));
    run(
        &cli(),
        &s,
        &["runtime", "sync", "--with-examples"],
        &RunOptions::new(),
    )
    .assert_success();
    assert_eq!(server.requests_for(Route::ScanReports).len(), 1);
    let db = archive(&s);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM work_attempts WHERE source='reports' AND outcome='returned'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        5
    );
    assert_eq!(
        db.query_row(
            "SELECT count(DISTINCT transport_id) FROM work_attempts WHERE source='reports'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    drop(db);
    run(
        &cli(),
        &s,
        &["runtime", "sync", "--with-examples"],
        &RunOptions::new(),
    )
    .assert_success();
    assert_eq!(server.requests_for(Route::ScanReports).len(), 1);
}

#[test]
fn report_rate_limit_persists_once_and_never_affects_ordinary_metadata_sync() {
    let (server, s) = fixture();
    server.set_default(
        Route::ScanReports,
        Planned::status(429).with_retry_after(120),
    );
    run(
        &cli(),
        &s,
        &["runtime", "sync", "--with-examples"],
        &RunOptions::new(),
    )
    .assert_success();
    let db = archive(&s);
    assert_eq!(
        db.query_row(
            "SELECT until_ms-observed_ms FROM service_cooldowns WHERE service='reports'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        120_000
    );
    drop(db);
    for args in [
        vec!["runtime", "sync"],
        vec!["runtime", "sync", "--with-examples"],
    ] {
        run(&cli(), &s, &args, &RunOptions::new()).assert_success();
    }
    assert_eq!(server.requests_for(Route::ScanReports).len(), 1);
    assert_eq!(
        archive(&s)
            .query_row(
                "SELECT count(*) FROM work_attempts WHERE source='reports'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn explicit_retry_revisits_exact_old_work_without_widening_recent_traversal() {
    let (server, s) = fixture();
    let mut session = fixtures::runtime::session("session-1");
    session["session_actions"][0]["scan_id"] = json!(SCAN);
    session["session_actions"][0]["start_time"] = json!("2025-01-01T12:00:00Z");
    server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
    server.push_once(Route::ScanResults, Planned::json(200, &json!([])));
    run(
        &cli(),
        &s,
        &[
            "runtime",
            "sync",
            "--since",
            "2025-01-01T00:00:00Z",
            "--until",
            "2025-01-02T00:00:00Z",
            "--backfill",
        ],
        &RunOptions::new(),
    )
    .assert_success();
    assert_eq!(
        archive(&s)
            .query_row(
                "SELECT count(*) FROM work_attempts WHERE outcome='missing'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    let root = s.tmp().join("history-data/airs-cli/history");
    run(
        &cli(),
        &s,
        &[
            "runtime",
            "history",
            "retry",
            "--source",
            "results",
            "--force",
            "--history-dir",
            root.to_str().unwrap(),
        ],
        &RunOptions::new(),
    )
    .assert_success();
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items":[],"pagination":{"limit":25,"skip":0,"total_items":0}}),
        ),
    );
    let out = run(
        &cli(),
        &s,
        &["runtime", "sync", "--output", "json"],
        &RunOptions::new(),
    );
    out.assert_success();
    let summary: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(summary["observed_events"], 0);
    assert_eq!(summary["actual_rolling_query"]["interval"], 24);
    assert_eq!(server.requests_for(Route::ScanResults).len(), 2);
    let db = archive(&s);
    let result: (String, String) = db
        .query_row("SELECT verdict,action FROM events", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(result, ("malicious".into(), "allow".into()));
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM work_attempts WHERE retry_epoch=1 AND outcome='returned'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM event_facts WHERE source='native_sync/individual_work_revisit'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(db.query_row("SELECT count(*) FROM coverage_segments c JOIN collection_runs r ON r.run_id=c.run_id WHERE c.run_id=?1 AND c.state='traversed_as_of' AND c.since_ms<r.target_since_ms",[summary["run_id"].as_str().unwrap()],|r|r.get::<_,i64>(0)).unwrap(),0);
}

#[test]
fn requested_absolute_cohort_is_filtered_before_any_scanner_admission() {
    let (server, s) = fixture();
    let mut session = fixtures::runtime::session("session-1");
    session["session_actions"][0]["scan_id"] = json!(SCAN);
    session["session_actions"][0]["start_time"] = json!("2025-01-03T12:00:00Z");
    server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
    let out = run(
        &cli(),
        &s,
        &[
            "runtime",
            "sync",
            "--since",
            "2025-01-01T00:00:00Z",
            "--until",
            "2025-01-02T00:00:00Z",
            "--backfill",
            "--with-examples",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    assert_eq!(
        serde_json::from_str::<Value>(&out.stdout).unwrap()["observed_events"],
        0
    );
    assert!(server.requests_for(Route::ScanResults).is_empty());
    assert!(server.requests_for(Route::ScanReports).is_empty());
    let db = archive(&s);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sync_work WHERE kind IN ('results','reports')",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn already_withheld_bucket_identity_is_never_an_exact_join_or_scanner_request() {
    let (server, s) = fixture();
    let name = "[sensitive value withheld]";
    let item = fixtures::runtime::session_item("uuid-shared", name, "session-1");
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items":[item],"pagination":{"limit":25,"skip":0,"total_items":1}}),
        ),
    );
    let mut session = fixtures::runtime::session("session-1");
    session["application"]["name"] = json!(name);
    session["session_actions"][0]["application_name"] = json!(name);
    session["session_actions"][0]["scan_id"] = json!(SCAN);
    session["session_actions"][0]["start_time"] = json!("2025-01-01T12:00:00Z");
    server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
    run(
        &cli(),
        &s,
        &[
            "runtime",
            "sync",
            "--since",
            "2025-01-01T00:00:00Z",
            "--until",
            "2025-01-02T00:00:00Z",
            "--backfill",
        ],
        &RunOptions::new(),
    )
    .assert_success();
    assert!(server.requests_for(Route::ScanResults).is_empty());
    let db = archive(&s);
    assert_eq!(
        db.query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(
        db.query_row("SELECT count(*) FROM receipts", [], |r| r.get::<_, i64>(0))
            .unwrap()
            > 0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM work_attempts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn positive_flags_derive_malicious_while_preserving_the_contradictory_api_category() {
    let (server, s) = fixture();
    let mut response = fixtures::scan_block("monitor");
    response["category"] = json!("benign");
    response["action"] = json!("allow");
    response["scan_id"] = json!(SCAN);
    response["session_id"] = json!("session-1");
    server.set_default(
        Route::ScanResults,
        Planned::json(
            200,
            &json!([{"scan_id":SCAN,"req_id":0,"status":"COMPLETED","result":response}]),
        ),
    );
    run(&cli(), &s, &["runtime", "sync"], &RunOptions::new()).assert_success();
    let db = archive(&s);
    let verdict: String = db
        .query_row("SELECT verdict FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(verdict, "malicious");
    let facts: String = db
        .query_row(
            "SELECT group_concat(fields_json,'') FROM event_facts",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(facts.contains("API scan category: benign"));
    assert!(facts.contains("derived from detector flags"));
}

#[test]
fn optional_old_example_revisit_uses_the_shared_selection_and_keeps_old_event_time() {
    let (server, s) = fixture();
    let mut session = fixtures::runtime::session("session-1");
    session["session_actions"][0]["scan_id"] = json!(SCAN);
    session["session_actions"][0]["start_time"] = json!("2025-01-01T12:00:00Z");
    server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
    server.push_once(Route::ScanReports, Planned::json(200, &json!([])));
    run(
        &cli(),
        &s,
        &[
            "runtime",
            "sync",
            "--since",
            "2025-01-01T00:00:00Z",
            "--until",
            "2025-01-02T00:00:00Z",
            "--backfill",
            "--with-examples",
        ],
        &RunOptions::new(),
    )
    .assert_success();
    let root = s.tmp().join("history-data/airs-cli/history");
    run(
        &cli(),
        &s,
        &[
            "runtime",
            "history",
            "retry",
            "--source",
            "reports",
            "--force",
            "--history-dir",
            root.to_str().unwrap(),
        ],
        &RunOptions::new(),
    )
    .assert_success();
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items":[],"pagination":{"limit":25,"skip":0,"total_items":0}}),
        ),
    );
    server.set_default(Route::ScanReports,Planned::json(200,&json!([{"report_id":"report-1","scan_id":SCAN,"req_id":0,"session_id":"session-1","detection_results":[{"detection_service":"prompt_injection","data_type":"prompt","verdict":"malicious","action":"allow","result_detail":{"pi_snippets":["Synthetic older strategic example"]}}]}])));
    let out = run(
        &cli(),
        &s,
        &["runtime", "sync", "--with-examples", "--output", "json"],
        &RunOptions::new(),
    );
    out.assert_success();
    let summary: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(summary["observed_events"], 0);
    assert_eq!(summary["progress"]["revisited_work_attempts"], 1);
    assert_eq!(server.requests_for(Route::ScanReports).len(), 2);
    let db = archive(&s);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM examples WHERE state='present'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT event_ms FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1735732800000
    );
    assert_eq!(db.query_row("SELECT count(*) FROM work_attempts WHERE source='reports' AND retry_epoch=1 AND outcome='returned'",[],|r|r.get::<_,i64>(0)).unwrap(),1);
}

#[test]
fn a_changed_same_endpoint_revision_revisits_old_ineligible_work_without_flat_fallback() {
    let (server, s) = fixture();
    let mut session = fixtures::runtime::session("session-1");
    session["session_actions"][0]["scan_id"] = json!(SCAN);
    session["session_actions"][0]["start_time"] = json!("2025-01-01T12:00:00Z");
    server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
    server.push_once(Route::ScanResults, Planned::status(403));
    run(
        &cli(),
        &s,
        &[
            "runtime",
            "sync",
            "--since",
            "2025-01-01T00:00:00Z",
            "--until",
            "2025-01-02T00:00:00Z",
            "--backfill",
        ],
        &RunOptions::new(),
    )
    .assert_success();
    let db = archive(&s);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sync_work WHERE kind='results' AND state='ineligible'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    drop(db);
    let rotated = "FAKE-APIKEY-ROTATED-SYNC-100";
    let input = format!(
        "applicationId='uuid-shared'\nendpoint={}\napiKey={}\n",
        json!(server.url("/scan")),
        json!(rotated)
    );
    rebind(&s, &input);
    server.add_tenant("100", "client-100", "FAKE-SECRET-100", Some(rotated), None);
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items":[],"pagination":{"limit":25,"skip":0,"total_items":0}}),
        ),
    );
    run(
        &cli(),
        &s,
        &["runtime", "sync"],
        &RunOptions::new().secret(rotated),
    )
    .assert_success();
    assert_eq!(server.requests_for(Route::ScanResults).len(), 2);
    let db = archive(&s);
    assert_eq!(
        db.query_row("SELECT verdict FROM events", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "malicious"
    );
    assert_eq!(
        db.query_row(
            "SELECT count(DISTINCT binding_revision) FROM work_attempts",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM work_attempts WHERE outcome='auth_denied'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM work_attempts WHERE outcome='returned'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn held_network_calls_leave_readers_free_refuse_a_second_sync_and_preserve_signal_progress() {
    for (signal, code) in [("INT", 130), ("TERM", 143)] {
        let (server, s) = fixture();
        server.push_once(Route::ScanResults, Planned::held());
        let running = spawn(&cli(), &s, &["runtime", "sync"], &RunOptions::new());
        assert!(server.wait_for_route(Route::ScanResults, 1, std::time::Duration::from_secs(10)));
        let db = archive(&s);
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM work_attempts WHERE outcome='started' AND ordinal=1",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        drop(db);
        let requests = server.request_count();
        let started = std::time::Instant::now();
        let second = run(&cli(), &s, &["runtime", "sync"], &RunOptions::new());
        second.assert_code(1);
        assert!(second.stderr.contains("active sync or maintenance"));
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
        assert_eq!(server.request_count(), requests);
        running.send_signal(signal);
        let out = running.wait();
        out.assert_code(code);
        server.release_held();
        let db = archive(&s);
        assert_eq!(
            db.query_row(
                "SELECT state FROM collection_runs WHERE kind='sync' ORDER BY started_ms LIMIT 1",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "interrupted"
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM work_attempts WHERE outcome='started' AND ended_ms IS NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        drop(db);
        run(&cli(), &s, &["runtime", "sync"], &RunOptions::new()).assert_success();
        assert_eq!(server.requests_for(Route::ScanResults).len(), 1);
    }
}

#[test]
fn report_http_date_retry_after_is_not_shortened_to_the_fallback() {
    let (server, s) = fixture();
    let target = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
    let header = httpdate::fmt_http_date(target);
    server.set_default(
        Route::ScanReports,
        Planned::status(429).with_header("retry-after", &header),
    );
    run(
        &cli(),
        &s,
        &["runtime", "sync", "--with-examples"],
        &RunOptions::new(),
    )
    .assert_success();
    let db = archive(&s);
    let (until, observed): (i64, i64) = db
        .query_row(
            "SELECT until_ms,observed_ms FROM service_cooldowns WHERE service='reports'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let target_ms = (target
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        * 1000) as i64;
    assert!(until >= target_ms);
    assert!((3_500_000..=3_601_000).contains(&(until - observed)));
}

#[test]
fn unavailable_primary_collection_exits_operationally_with_qualified_json() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    let tenant = s.add_fixture_tenant_with(&server, "prod", "100", |c| {
        c.remove("mgmtClientId");
        c.remove("mgmtClientSecret");
    });
    s.register(&[&tenant], Some("prod"));
    let out = run(
        &cli(),
        &s,
        &["runtime", "sync", "--output", "json"],
        &RunOptions::new(),
    );
    out.assert_code(1);
    let value: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(value["collection_unavailable"], true);
    assert_eq!(value["state"], "partial");
    assert_eq!(server.request_count(), 0);
}
#[test]
fn sync_usage_is_pure_and_retry_requires_explicit_source_and_force() {
    let s = Sandbox::new();
    for args in [
        vec!["runtime", "sync", "--backfill"],
        vec![
            "runtime",
            "sync",
            "--backfill",
            "--since",
            "2999-01-01T00:00:00Z",
            "--until",
            "2999-01-02T00:00:00Z",
        ],
        vec!["runtime", "sync", "--since", "2026-01-01T00:00:00Z"],
        vec!["runtime", "sync", "--max-reports", "5"],
        vec!["runtime", "sync", "--with-examples", "--max-reports", "11"],
        vec!["runtime", "sync", "--with-examples", "--with-examples"],
        vec!["runtime", "sync", "--history-dir", "relative"],
        vec!["runtime", "history", "retry", "--source", "results"],
        vec![
            "runtime", "history", "retry", "--source", "unknown", "--force",
        ],
    ] {
        let out = run(&cli(), &s, &args, &RunOptions::new());
        assert_eq!(out.code, 2, "{}", out.stderr);
    }
}

#[test]
fn shifting_application_pages_preserve_valid_observations_but_not_traversal_claims() {
    let (server, s) = fixture();
    server.set_default(Route::Runtime(Rx::AppsOverview), Planned::json(200, &json!({
        "items":[{"id":"uuid-shared","name":"A & B","sessions_total":8,"sessions_violated":3}],
        "pagination":{"limit":100,"skip":100,"total_items":101}
    })));
    let out = run(
        &cli(),
        &s,
        &["runtime", "sync", "--output", "json"],
        &RunOptions::new(),
    );
    out.assert_success();
    let db = archive(&s);
    assert_eq!(db.query_row("SELECT count(*) FROM aggregate_observations WHERE source='applications' AND json_extract(fields_json,'$.metric.value')=8", [], |r|r.get::<_,i64>(0)).unwrap(), 1);
    assert_eq!(db.query_row("SELECT count(*) FROM coverage_segments WHERE source='applications' AND state='traversed_as_of'", [], |r|r.get::<_,i64>(0)).unwrap(), 0);
    assert_eq!(
        server.requests_for(Route::Runtime(Rx::AppsOverview)).len(),
        1
    );
    drop(db);
    run(&cli(), &s, &["runtime", "sync"], &RunOptions::new()).assert_success();
    for request in server.requests_for(Route::Runtime(Rx::AppsOverview)) {
        assert_eq!(request.query_value("offset"), Some("0"));
    }
}

#[test]
fn management_rate_limit_blocks_all_subsequent_management_sends_including_oauth() {
    let (server, s) = fixture();
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::status(429).with_retry_after(180),
    );
    run(&cli(), &s, &["runtime", "sync"], &RunOptions::new()).assert_success();
    let db = archive(&s);
    assert_eq!(
        db.query_row(
            "SELECT until_ms-observed_ms FROM service_cooldowns WHERE service='management'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        180_000
    );
    drop(db);
    let before = server.request_count();
    let out = run(
        &cli(),
        &s,
        &["runtime", "sync", "--output", "json"],
        &RunOptions::new(),
    );
    out.assert_code(1);
    let v: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(v["collection_unavailable"], true);
    assert_eq!(server.request_count(), before);
}

#[test]
fn timed_out_management_read_is_never_recorded_as_proven_not_sent() {
    let (server, s) = fixture();
    server.push_once(Route::Runtime(Rx::SessionsOverview), Planned::held());
    let running = spawn(
        &cli(),
        &s,
        &["runtime", "sync", "--timeout", "5", "--output", "json"],
        &RunOptions::new(),
    );
    assert!(server.wait_for_route(
        Route::Runtime(Rx::SessionsOverview),
        1,
        std::time::Duration::from_secs(10)
    ));
    let out = running.wait();
    server.release_held();
    let document: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(document["state"], "partial");
    // Unrelated primary reads may also time out under emulation. Availability
    // determines the exit code; it cannot turn the received held read into no-send.
    let unavailable = document["collection_unavailable"]
        .as_bool()
        .expect("explicit primary availability");
    out.assert_code(if unavailable { 1 } else { 0 });
    let db = archive(&s);
    let (started,not_sent):(i64,i64)=db.query_row("SELECT count(*),coalesce(sum(outcome='not_sent'),0) FROM management_reads WHERE source LIKE '%sessions%overview%'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(started, 1);
    assert_eq!(not_sent, 0);
    assert_eq!(
        server
            .requests_for(Route::Runtime(Rx::SessionsOverview))
            .len(),
        1
    );
}

#[test]
fn failed_primary_http_sources_are_unavailable_even_with_empty_optional_telemetry() {
    let (server, s) = fixture();
    for route in [
        Route::Profiles,
        Route::Runtime(Rx::CustomerApps),
        Route::Runtime(Rx::AppsOverview),
        Route::Runtime(Rx::SessionsOverview),
    ] {
        server.set_default(route, Planned::status(500));
    }
    let out = run(
        &cli(),
        &s,
        &["runtime", "sync", "--output", "json"],
        &RunOptions::new(),
    );
    out.assert_code(1);
    let v: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(v["collection_unavailable"], true);
    assert_eq!(v["observed_events"], 0);
    assert!(server.requests_for(Route::ScanResults).is_empty());
    assert!(server.requests_for(Route::ScanReports).is_empty());
}

#[test]
fn report_transport_cap_is_shared_across_three_named_application_scanners() {
    let (server, s) = fixture();
    let mut inventory = vec![];
    let mut results = vec![];
    let mut reports = vec![];
    let at = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .unwrap();
    let at = std::str::from_utf8(&at.stdout).unwrap().trim();
    for n in 1..=3 {
        let app = format!("uuid-app-{n}");
        let session_id = format!("session-{n}");
        let scan = format!("00000000-0000-4000-8000-{n:012x}");
        let report = format!("report-app-{n}");
        s.bind_fixture_scanner("prod", &format!("scanner-{n}"), &app, "key");
        inventory.push(fixtures::runtime::session_item(
            &app,
            "Exact application",
            &session_id,
        ));
        let mut session = fixtures::runtime::session(&session_id);
        session["application"] = json!({"id":app,"name":"Exact application"});
        session["session_actions"][0]["application_id"] = json!(app);
        session["session_actions"][0]["application_name"] = json!("Exact application");
        session["session_actions"][0]["scan_id"] = json!(scan);
        session["session_actions"][0]["start_time"] = json!(at);
        server.push_once(Route::Runtime(Rx::Session), Planned::json(200, &session));
        let mut result = fixtures::scan_block("monitor");
        result["scan_id"] = json!(scan);
        result["session_id"] = json!(session_id);
        result["report_id"] = json!(report);
        result["action"] = json!("allow");
        results.push(json!({"scan_id":scan,"req_id":0,"status":"COMPLETED","result":result}));
        reports.push(json!({"scan_id":scan,"req_id":0,"report_id":report,"session_id":session_id,"detection_results":[{"detection_service":"prompt_injection","data_type":"prompt","verdict":"malicious","action":"allow","result_detail":{"pi_snippets":["Synthetic multi-application evidence"]}}]}));
    }
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items":inventory,"pagination":{"limit":25,"skip":0,"total_items":3}}),
        ),
    );
    server.set_default(Route::ScanResults, Planned::json(200, &json!(results)));
    server.set_default(Route::ScanReports, Planned::json(200, &json!(reports)));
    let out = run(
        &cli(),
        &s,
        &["runtime", "sync", "--with-examples", "--concurrency", "1"],
        &RunOptions::new(),
    );
    out.assert_success();
    assert_eq!(server.requests_for(Route::ScanResults).len(), 3);
    assert_eq!(server.requests_for(Route::ScanReports).len(), 2);
    let db = archive(&s);
    assert_eq!(db.query_row("SELECT count(DISTINCT transport_id) FROM work_attempts WHERE source='reports' AND outcome!='not_sent'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM work_attempts WHERE source='reports' AND outcome='not_sent'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn privacy_erased_application_name_cannot_promote_into_an_existing_full_bucket() {
    for name in ["private@example.invalid", "[sensitive value withheld]"] {
        let (server, s) = fixture();
        run(&cli(), &s, &["runtime", "sync"], &RunOptions::new()).assert_success();
        let item = fixtures::runtime::session_item("uuid-shared", name, "session-1");
        server.set_default(
            Route::Runtime(Rx::SessionsOverview),
            Planned::json(
                200,
                &json!({"items":[item],"pagination":{"limit":25,"skip":0,"total_items":1}}),
            ),
        );
        let mut session = fixtures::runtime::session("session-1");
        session["application"]["name"] = json!(name);
        session["session_actions"][0]["application_name"] = json!(name);
        session["session_actions"][0]["scan_id"] = json!(SCAN);
        let at = std::process::Command::new("date")
            .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
            .output()
            .unwrap();
        session["session_actions"][0]["start_time"] =
            json!(std::str::from_utf8(&at.stdout).unwrap().trim());
        server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
        run(&cli(), &s, &["runtime", "sync"], &RunOptions::new()).assert_success();
        assert_eq!(server.requests_for(Route::ScanResults).len(), 1);
        assert!(server.requests_for(Route::ScanReports).is_empty());
        let db = archive(&s);
        assert_eq!(
            db.query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert!(db.query_row("SELECT count(*) FROM receipts WHERE json_extract(binding_json,'$.identity_withheld')=1 AND promoted_event_id IS NULL",[],|r|r.get::<_,i64>(0)).unwrap()>0);
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM receipts WHERE state='promoted'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        let facts: String = db
            .query_row(
                "SELECT group_concat(fields_json,'') FROM receipt_facts",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!facts.contains("private@example.invalid"));
    }
}

/// Events archived while their application had no scanner binding must all become due
/// once a binding appears: repeated syncs look every one of them up, a budget's worth at a
/// time, and none is stranded. The events lie outside the later runs' window, so only the
/// retained-work revisit can reach them (production, 2026-11-09: a long bulk session whose
/// scanner key was bound afterwards).
#[test]
fn a_new_binding_eventually_revisits_every_event_archived_while_unbound() {
    const APP: &str = "uuid-late";
    const NAME: &str = "Late";
    const SESSIONS: usize = 40;
    const PER_SESSION: usize = 25;
    let (server, s) = fixture();
    let date = |args: &[&str]| {
        let out = std::process::Command::new("date")
            .args(args)
            .output()
            .unwrap();
        std::str::from_utf8(&out.stdout).unwrap().trim().to_owned()
    };
    // The events happened three hours ago: inside the dashboard's rolling 24 hours, but
    // outside the later runs' collection window, so only the retained-work revisit can
    // reach them (as for a scheduled tick after a long bulk run).
    let now = date(&["-u", "-d", "-3 hours", "+%Y-%m-%dT%H:%M:%SZ"]);
    let since = date(&["-u", "-d", "-1 hour", "+%Y-%m-%dT%H:%M:%SZ"]);
    let until = date(&["-u", "+%Y-%m-%dT%H:%M:%SZ"]);
    let mut scan_ids = Vec::new();
    server.clear_overrides(Route::Runtime(Rx::SessionsOverview));
    server.clear_overrides(Route::Runtime(Rx::Session));
    server.runtime(|data| {
        data.sessions.clear();
        data.session_records.clear();
        for n in 0..SESSIONS {
            let session = format!("late-session-{n:02}");
            let mut item = fixtures::runtime::session_item(APP, NAME, &session);
            item["last_session_activity"] = json!(now);
            data.sessions.push(item);
            let mut record = fixtures::runtime::session(&session);
            record["application"] = json!({"id": APP, "name": NAME});
            let mut actions = Vec::new();
            for k in 0..PER_SESSION {
                let scan = format!(
                    "00000000-0000-4000-8000-{:012x}",
                    0x1000 + n * PER_SESSION + k
                );
                scan_ids.push(scan.clone());
                let mut action = record["session_actions"][0].clone();
                action["application_id"] = json!(APP);
                action["application_name"] = json!(NAME);
                action["scan_id"] = json!(scan);
                action["start_time"] = json!(now);
                actions.push(action);
            }
            record["session_actions"] = json!(actions);
            record["pagination"] = json!({"limit": 25, "skip": 0, "total_items": PER_SESSION});
            data.session_records
                .insert((APP.into(), NAME.into(), session), record);
        }
    });
    server.set_default(Route::ScanResults, Planned::json(200, &json!([])));
    // Collect while the application has no binding (the fixture binds only uuid-shared).
    let collected = run(
        &cli(),
        &s,
        &["runtime", "sync"],
        &RunOptions::new().timeout(std::time::Duration::from_secs(120)),
    );
    eprintln!(
        "[DEBUG-sync-1000] initial collection: {:?}",
        collected.elapsed
    );
    collected.assert_success();
    let events: i64 = archive(&s)
        .query_row("SELECT count(*) FROM events WHERE app_id=?1", [APP], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        events as usize,
        SESSIONS * PER_SESSION,
        "every event archived"
    );
    assert!(
        server.requests_for(Route::ScanResults).is_empty(),
        "nothing looked up unbound"
    );
    // Bind the application, then sync repeatedly; each run has a bounded Results budget.
    let input = format!(
        "applicationId={}\nendpoint={}\napiKey={}\n",
        json!(APP),
        json!(server.url("/scan")),
        json!("key")
    );
    rebind(&s, &input);
    let mut requested = std::collections::BTreeSet::new();
    for _ in 0..30 {
        run(
            &cli(),
            &s,
            &[
                "runtime",
                "sync",
                "--since",
                &since,
                "--until",
                &until,
                "--backfill",
            ],
            &RunOptions::new(),
        )
        .assert_success();
        for request in server.requests_for(Route::ScanResults) {
            for id in request
                .query_value("scan_ids")
                .unwrap_or_default()
                .split(',')
            {
                requested.insert(id.to_owned());
            }
        }
        if requested.len() == scan_ids.len() {
            break;
        }
    }
    let missing: Vec<_> = scan_ids
        .iter()
        .filter(|id| !requested.contains(*id))
        .collect();
    assert!(
        missing.is_empty(),
        "{} of {} events were never looked up after the binding appeared (first: {:?})",
        missing.len(),
        scan_ids.len(),
        missing.first()
    );
}
