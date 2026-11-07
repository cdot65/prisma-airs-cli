//! M4.1 collection executor against the loopback AIRS fixture: a real collection run in
//! service context, its frozen result, zero Reports calls, and the C4 resume rule (a
//! checkpointed run that already reached the archive finishes with no new send).

use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::sync::Arc;

use airs_mcp::collection::{COLLECTING, Collection};
use airs_mcp::control::{Actor, Control, Submission};
use airs_mcp::jobs::State;
use airs_mcp::run::{Live, Shared, parse_config};
use airs_mcp::storage;
use airs_mcp::tools::Storage;
use airs_mcp::worker::{Executor, Worker};
use airs_test_harness::{FixtureServer, Route, Sandbox, paths};
use serde_json::{Value, json};

struct Dir(std::path::PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn private_dir(tag: &str) -> Dir {
    let d = std::env::temp_dir().join(format!("airs-mcp-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new().mode(0o700).create(&d).unwrap();
    Dir(d.canonicalize().unwrap())
}

fn secret(dir: &std::path::Path, name: &str, value: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, value).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    path.display().to_string()
}

/// The example service configuration pointed at the loopback fixture, with the
/// fixture tenant's synthetic credentials in private secret files.
fn service_config_for(server: &FixtureServer, base: &std::path::Path, app: &str) -> String {
    let url = |p: &str| format!("{}{p}", server.base_url().trim_end_matches('/'));
    let secrets = base.join("secrets");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&secrets)
        .unwrap();
    let id = secret(&secrets, "mgmt-client-id", "client-100");
    let key = secret(&secrets, "mgmt-client-secret", "FAKE-SECRET-100");
    let scanner = secret(&secrets, "scanner-monitor", "FAKE-APIKEY-100");
    include_str!("../config/example.toml")
        .replace("listen = \"0.0.0.0:8443\"", "listen = \"127.0.0.1:0\"")
        .replace("health_listen = \"0.0.0.0:8081\"\n", "")
        .replace("[server.tls]\ncert_file = \"/etc/airs-mcp/tls/tls.crt\"\nkey_file = \"/etc/airs-mcp/tls/tls.key\"\n", "")
        .replace("data_dir = \"/data/airs\"", &format!("data_dir = \"{}\"", base.join("data").display()))
        .replace("tsg = \"1234567890\"", "tsg = \"100\"")
        .replace("management_endpoint = \"https://api.apps.paloaltonetworks.com/aisec\"", &format!("management_endpoint = \"{}\"", url(paths::MGMT_PREFIX)))
        .replace("dashboard_endpoint = \"https://api.apps.paloaltonetworks.com/aisec\"", &format!("dashboard_endpoint = \"{}\"", url("/dashboard")))
        .replace("token_endpoint = \"https://auth.apps.paloaltonetworks.com/oauth2/access_token\"", &format!("token_endpoint = \"{}\"", url(paths::TOKEN)))
        .replace("management_client_id_file = \"/run/airs-mcp/secrets/mgmt-client-id\"", &format!("management_client_id_file = \"{id}\""))
        .replace("management_client_secret_file = \"/run/airs-mcp/secrets/mgmt-client-secret\"", &format!("management_client_secret_file = \"{key}\""))
        .replace("endpoint = \"https://service.api.aisecurity.paloaltonetworks.com\"", &format!("endpoint = \"{}\"", url(paths::SCAN_PREFIX)))
        .replace("api_key_file = \"/run/airs-mcp/secrets/scanner-monitor\"", &format!("api_key_file = \"{scanner}\""))
        .replace("applicationId = \"app-example\"", &format!("applicationId = \"{app}\""))
}

struct Service {
    _base: Dir,
    storage: Arc<Storage>,
    shared: Arc<Shared>,
    tenant: String,
}

fn service(server: &FixtureServer) -> Service {
    service_with(server, "app-example")
}

fn service_with(server: &FixtureServer, app: &str) -> Service {
    let base = private_dir("collect");
    let text = service_config_for(server, &base.0, app);
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(base.0.join("data"))
        .unwrap();
    let live = Live::new(parse_config(text.as_bytes()).expect("config")).unwrap();
    let tenant = live.config.service.tenant.clone();
    let admitted = storage::admit(&base.0.join("data")).unwrap();
    let control_dir = storage::private_child(&admitted, "control").unwrap();
    let history = storage::private_child(&admitted, "history").unwrap();
    let control = Control::open(&control_dir.path, &tenant, "0a1b2c3d", &[1u8; 32], 1_000).unwrap();
    let owner = storage::OwnerRecord {
        owner_id: "o".into(),
        pod: "p".into(),
        node: "n".into(),
        pid: 0,
        acquired_at_unix: 0,
    };
    let storage = Arc::new(Storage::new(admitted, owner, history.path.clone(), control));
    Service {
        _base: base,
        storage,
        shared: Shared::new(live),
        tenant,
    }
}

fn submit(s: &Service, key: &str) -> String {
    let params = json!({"mode": "recent", "range": null, "app_ids": []});
    s.storage.control(|c| {
        c.submit(
            Submission {
                tenant: &s.tenant,
                operation: "collection_start",
                actor: Actor::Principal {
                    sub: "PLACEHOLDER_CALLER_SUB".into(),
                    client_id: "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID".into(),
                },
                idempotency_key: key,
                expected_epoch: "0a1b2c3d",
                params: &params,
                config_revision: "example-1",
                credential_revision: None,
                deadline_ms: airs_runtime_core::time::now_ms() + 3_600_000,
                queue_tenant: 64,
                queue_principal: 8,
                join_equivalent: false,
                scheduled_slot: None,
            },
            airs_runtime_core::time::now_ms(),
        )
        .unwrap()
        .job_id
    })
}

fn worker(s: &Service) -> Worker {
    let mut executors: std::collections::BTreeMap<String, Arc<dyn Executor>> = Default::default();
    executors.insert(
        "collection_start".into(),
        Arc::new(Collection {
            shared: s.shared.clone(),
        }),
    );
    Worker {
        storage: s.storage.clone(),
        owner: "owner-test".into(),
        executors,
    }
}

fn job(s: &Service, id: &str) -> airs_mcp::control::Job {
    s.storage
        .control(|c| c.job(&s.tenant, id).unwrap().unwrap())
}

fn fixture() -> (FixtureServer, Sandbox) {
    let server = FixtureServer::start();
    server.seed_runtime();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    (server, sandbox)
}

/// A collection job runs the shared collector in service context and records the
/// frozen `result_collection`: management traffic, zero Reports calls.
#[test]
fn a_collection_job_collects_into_the_archive_with_zero_reports_calls() {
    let (server, _sandbox) = fixture();
    let s = service(&server);
    let id = submit(&s, "key-collect-000000001");
    let (stepped, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(stepped, id);
    assert!(
        matches!(state, State::Succeeded | State::Partial),
        "{state:?}"
    );
    let result: Value = job(&s, &id).result.unwrap();
    assert_eq!(result["operation"], "collection_start");
    assert_eq!(result["sends"]["reports"], 0);
    assert!(
        result["sends"]["management"].as_u64().unwrap() > 0,
        "{result}"
    );
    for key in [
        "new_facts",
        "duplicate_facts",
        "sources",
        "unfinished_sources",
    ] {
        assert!(result.get(key).is_some(), "{key}");
    }
    assert!(
        server.requests_for(Route::ScanReports).is_empty(),
        "polling makes no Reports call"
    );
    let tokens = server.requests_for(Route::Token).len() as u64;
    assert!(tokens > 0);
    assert_eq!(
        result["sends"]["oauth"], tokens,
        "every token request is reported"
    );
    // The checkpoint named the archive run before anything was written (C4).
    let (phase, _, op) = s
        .storage
        .control(|c| c.last_checkpoint(&id).unwrap().unwrap());
    assert_eq!(phase, COLLECTING);
    assert!(uuid::Uuid::parse_str(op.as_deref().unwrap()).is_ok());
}

/// Crash point C4: a job whose checkpoint names a run that already reached the archive
/// (the process died after the archive commit, before the job finished) finishes from
/// the recorded run with no new AIRS request.
#[test]
fn a_resumed_job_finishes_from_its_recorded_run_without_sending() {
    let (server, _sandbox) = fixture();
    let s = service(&server);
    let first = submit(&s, "key-collect-000000002");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let (_, _, run) = s
        .storage
        .control(|c| c.last_checkpoint(&first).unwrap().unwrap());
    let run = run.unwrap();
    // A second job, claimed and checkpointed with that run ID, then "restarted".
    let second = submit(&s, "key-collect-000000003");
    s.storage.control(|c| {
        c.claim_next(
            &s.tenant,
            "owner-dead",
            airs_runtime_core::time::now_ms(),
            |_| true,
        )
        .unwrap()
        .unwrap();
        c.checkpoint(
            &second,
            COLLECTING,
            &json!({"run_id": run}),
            Some(&run),
            airs_runtime_core::time::now_ms(),
        )
        .unwrap();
        c.recover(airs_runtime_core::time::now_ms()).unwrap();
    });
    let before = server.requests().len();
    let (stepped, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(stepped, second);
    assert!(
        matches!(state, State::Succeeded | State::Partial),
        "{state:?}"
    );
    assert_eq!(
        server.requests().len(),
        before,
        "no new AIRS request on resume"
    );
}

/// M4.2: a denied management credential parks the job in `needs_credentials` and
/// records the denial; every later job under the same revision stops before any
/// request (repeated 401s cannot multiply traffic); a new revision resumes the parked
/// jobs, and the denied revision resumes nothing.
#[test]
fn a_denied_credential_is_never_retried_and_a_new_revision_resumes() {
    use airs_test_harness::Planned;
    let (server, _sandbox) = fixture();
    server.set_default(
        Route::Token,
        Planned::json(401, &json!({"error": "invalid_client"})),
    );
    let s = service(&server);
    let first = submit(&s, "key-denied-0000000001");
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(state, State::NeedsCredentials);
    let sent = server.requests().len();
    assert!(
        sent > 0 && sent <= 2,
        "one token request and at most its single replay: {sent}"
    );
    assert!(server.requests_for(Route::ScanReports).is_empty());
    let revision = s.shared.current().config.airs.management_revision.clone();
    assert!(
        s.storage
            .control(|c| c.denied(&s.tenant, "management", &revision).unwrap())
    );
    // Another job under the denied revision: no request at all.
    let second = submit(&s, "key-denied-0000000002");
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(state, State::NeedsCredentials);
    assert_eq!(
        server.requests().len(),
        sent,
        "a denied revision sends nothing"
    );
    // The denied revision resumes nothing; a new one resumes both, deadlines intact.
    let now = airs_runtime_core::time::now_ms();
    let deadline = job(&s, &first).deadline_ms;
    assert_eq!(
        s.storage.control(|c| c
            .credentials_restored(&s.tenant, "management", &revision, now)
            .unwrap()),
        0
    );
    let rotated = "00000000-0000-4000-8000-0000000000aa";
    assert_eq!(
        s.storage.control(|c| c
            .credentials_restored(&s.tenant, "management", rotated, now)
            .unwrap()),
        2
    );
    for id in [&first, &second] {
        let j = job(&s, id);
        assert_eq!(j.state, State::Queued);
        assert_eq!(job(&s, &first).deadline_ms, deadline);
    }
}

/// Five sessions with reports, as the history store's enrichment test seeds them.
fn seed_examples(server: &FixtureServer) {
    use airs_test_harness::{Planned, Rx, fixtures};
    // The fixture sessions belong to `uuid-shared`; the test scanner is bound to it.
    let item = fixtures::runtime::session_item("uuid-shared", "A & B", "session-1");
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items": [item], "pagination": {"limit": 25, "skip": 0, "total_items": 1}}),
        ),
    );
    let mut session = fixtures::runtime::session("session-1");
    let template = session["session_actions"][0].clone();
    let at = airs_runtime_core::time::iso(airs_runtime_core::time::now_ms() - 60_000);
    let (mut actions, mut results, mut reports) = (vec![], vec![], vec![]);
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
        results
            .push(json!({"scan_id": scan, "req_id": 0, "status": "COMPLETED", "result": result}));
        reports.push(json!({"report_id": report, "scan_id": scan, "req_id": 0, "session_id": "session-1", "detection_results": [{"detection_service": "prompt_injection", "data_type": "prompt", "verdict": "malicious", "action": "allow", "result_detail": {"pi_snippets": ["Synthetic strategic example"]}}]}));
    }
    session["session_actions"] = json!(actions);
    session["pagination"]["total_items"] = json!(5);
    server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
    server.set_default(Route::ScanResults, Planned::json(200, &json!(results)));
    server.set_default(Route::ScanReports, Planned::json(200, &json!(reports)));
}

fn submit_enrichment(s: &Service, key: &str) -> String {
    let now = airs_runtime_core::time::now_ms();
    let params = json!({"since_ms": now - 86_400_000, "until_ms": now, "max_reports": 5});
    s.storage.control(|c| {
        c.submit(
            Submission {
                tenant: &s.tenant,
                operation: "enrichment_start",
                actor: Actor::Service("scheduler".into()),
                idempotency_key: key,
                expected_epoch: "0a1b2c3d",
                params: &params,
                config_revision: "example-1",
                credential_revision: None,
                deadline_ms: now + 3_600_000,
                queue_tenant: 64,
                queue_principal: 8,
                join_equivalent: false,
                scheduled_slot: None,
            },
            now,
        )
        .unwrap()
        .job_id
    })
}

fn enrichment_worker(s: &Service) -> Worker {
    let mut executors: std::collections::BTreeMap<String, Arc<dyn Executor>> = Default::default();
    executors.insert(
        "enrichment_start".into(),
        Arc::new(airs_mcp::collection::Enrichment {
            shared: s.shared.clone(),
        }),
    );
    Worker {
        storage: s.storage.clone(),
        owner: "owner-test".into(),
        executors,
    }
}

/// M4.4: one enrichment job selects its examples in one serial Reports transport and
/// records the frozen `result_enrichment`; the tenant's Chicago-day allowance is charged
/// durably before any send, so a second job the same day is blocked with no request.
#[test]
fn enrichment_is_sparse_and_charged_to_the_daily_allowance() {
    let (server, _sandbox) = fixture();
    seed_examples(&server);
    let s = service_with(&server, "uuid-shared");
    let first = submit_enrichment(&s, "key-enrich-000000001");
    let (_, state) = enrichment_worker(&s)
        .step(&s.tenant, |_| true)
        .unwrap()
        .unwrap();
    // The fixture leaves one required management source unfinished, so the run's
    // durable record is partial: the job says so (M4 review 1 finding 4).
    assert!(
        matches!(state, State::Succeeded | State::Partial),
        "{:?}",
        job(&s, &first).result
    );
    let result = job(&s, &first).result.unwrap();
    assert_eq!(result["operation"], "enrichment_start");
    assert_eq!(result["sends"], 1, "{result}");
    assert_eq!(result["selected_ids"], 5);
    assert_eq!(result["stopped_on_429"], false);
    assert_eq!(server.requests_for(Route::ScanReports).len(), 1);
    // M4 review 1 finding 8: the allowance is charged per actual Reports transport, at
    // its admission, to that moment's Chicago day: one send, five IDs, not a whole
    // allowance at job start; each transport has its own durable reservation.
    let now = airs_runtime_core::time::now_ms();
    let spent = s
        .storage
        .control(|c| c.reports_spent(&s.tenant, now).unwrap());
    assert_eq!(spent, (1, 5), "charged per transport");
    let reserved = s.storage.control(|c| c.consumed_sends(&first).unwrap());
    assert!(
        reserved >= 2,
        "a run reservation plus the Reports transport's own"
    );
    // Another job spends the rest of the day; the next enrichment sends nothing.
    s.storage
        .control(|c| c.charge_reports(&s.tenant, now, (1, 1), (2, 10)).unwrap());
    let second = submit_enrichment(&s, "key-enrich-000000002");
    let before = server.requests().len();
    let (_, state) = enrichment_worker(&s)
        .step(&s.tenant, |_| true)
        .unwrap()
        .unwrap();
    assert_eq!(state, State::BlockedPolicy);
    assert_eq!(server.requests().len(), before, "a spent day sends nothing");
    let _ = second;
}

fn archive_path(s: &Service) -> std::path::PathBuf {
    let cloud = airs_mcp::tools::cloud_for_tests(&s.shared.current().config);
    s.storage.history_root.join(cloud).join("archive.sqlite3")
}

/// Claim a fresh job as a now-dead owner, checkpoint it at `run`, and recover.
fn crashed_job(s: &Service, key: &str, run: &str) -> String {
    let id = submit(s, key);
    s.storage.control(|c| {
        let now = airs_runtime_core::time::now_ms();
        c.claim_next(&s.tenant, "owner-dead", now, |_| true)
            .unwrap()
            .unwrap();
        c.checkpoint(&id, COLLECTING, &json!({"run_id": run}), Some(run), now)
            .unwrap();
        c.recover(now).unwrap();
    });
    id
}

/// Crash point C4, death before the archive row: the checkpointed run never reached the
/// archive, so the resumed job collects under that same run ID (no second run, no
/// orphaned ID).
#[test]
fn a_run_that_never_reached_the_archive_is_collected_under_its_checkpointed_id() {
    let (server, _sandbox) = fixture();
    let s = service(&server);
    let run = uuid::Uuid::new_v4().to_string();
    let id = crashed_job(&s, "key-collect-000000010", &run);
    let (stepped, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(stepped, id);
    assert!(
        matches!(state, State::Succeeded | State::Partial),
        "{state:?}"
    );
    let db = rusqlite::Connection::open_with_flags(
        archive_path(&s),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let (runs, mine): (i64, i64) = db
        .query_row(
            "SELECT count(*), sum(run_id=?1) FROM collection_runs",
            [&run],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((runs, mine), (1, 1), "the checkpointed ID is the run");
}

/// Crash point C4, death mid-run: a run left `running` is closed `interrupted`, the job
/// ends `partial` with what committed, and nothing is sent again.
#[test]
fn a_run_left_running_is_interrupted_and_the_job_ends_partial_without_sending() {
    let (server, _sandbox) = fixture();
    let s = service(&server);
    let first = submit(&s, "key-collect-000000011");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let (_, _, run) = s
        .storage
        .control(|c| c.last_checkpoint(&first).unwrap().unwrap());
    let run = run.unwrap();
    // As if the process had died before the run's final status was written.
    let db = rusqlite::Connection::open(archive_path(&s)).unwrap();
    db.execute(
        "UPDATE collection_runs SET state='running', ended_ms=NULL WHERE run_id=?1",
        [&run],
    )
    .unwrap();
    drop(db);
    let second = crashed_job(&s, "key-collect-000000012", &run);
    let before = server.requests().len();
    let (stepped, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!((stepped.as_str(), state), (second.as_str(), State::Partial));
    assert_eq!(server.requests().len(), before, "nothing is sent again");
    let db = rusqlite::Connection::open_with_flags(
        archive_path(&s),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let state: String = db
        .query_row(
            "SELECT state FROM collection_runs WHERE run_id=?1",
            [&run],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(state, "interrupted");
}

/// M4 exit gate: a 429 on management reads keeps the run's data (`partial`) and holds
/// the plane, so the next job waits (`waiting_retry`) without any request; repeated
/// 429s cannot multiply traffic.
#[test]
fn a_management_429_holds_the_plane_and_the_next_job_sends_nothing() {
    use airs_test_harness::{Planned, Rx};
    let (server, _sandbox) = fixture();
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::status(429).with_retry_after(120),
    );
    let s = service(&server);
    let first = submit(&s, "key-limited-000000001");
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert!(
        matches!(state, State::Partial | State::Succeeded),
        "{state:?}"
    );
    let _ = first;
    let sent = server.requests().len();
    let _second = submit(&s, "key-limited-000000002");
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(state, State::WaitingRetry);
    assert_eq!(
        server.requests().len(),
        sent,
        "the held plane sends nothing"
    );
}

const SCAN: &str = "00000000-0000-4000-8000-0000000000b2";

/// The runtime_sync fixture's one scan: a session action and its scan result.
fn seed_scan(server: &FixtureServer) {
    use airs_test_harness::{Planned, Rx, fixtures};
    let item = fixtures::runtime::session_item("uuid-shared", "A & B", "session-1");
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::json(
            200,
            &json!({"items": [item], "pagination": {"limit": 25, "skip": 0, "total_items": 1}}),
        ),
    );
    let mut session = fixtures::runtime::session("session-1");
    session["session_actions"][0]["scan_id"] = json!(SCAN);
    session["session_actions"][0]["start_time"] = json!(airs_runtime_core::time::iso(
        airs_runtime_core::time::now_ms() - 60_000
    ));
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
            &json!([{"scan_id": SCAN, "req_id": 0, "status": "COMPLETED", "result": result}]),
        ),
    );
}

/// A client-asserted receipt for the fixture scan, as `receipts_ingest` builds it.
fn ingest_receipt(s: &Service, key: &str) -> (u64, u64) {
    ingest_claim(s, key, "unknown", "allow")
}

/// A client-asserted receipt claiming `verdict`/`action` for the fixture scan.
fn ingest_claim(s: &Service, key: &str, verdict: &str, action: &str) -> (u64, u64) {
    use airs_runtime_core::signal::evidence::{Event, Identity};
    let config = s.shared.current().config.clone();
    let scanner = &config.airs.scanners[0];
    let endpoint = scanner.endpoint.trim_end_matches('/').to_owned();
    let received = airs_history_store::repository::Received {
        receipt_key: key.into(),
        batch_report_id: None,
        event: Event {
            identity: Identity {
                app_id: scanner.application_id.clone(),
                app_name: String::new(),
                session_id: "session-1".into(),
                scan_id: SCAN.into(),
                sub_request: Some(0),
            },
            timestamp_ms: None,
            profile: None,
            verdict: verdict.into(),
            action: action.into(),
            report_ids: vec![],
            detections: vec![],
            detector_hints: vec![],
            sources: vec!["client_asserted".into()],
            notes: vec![],
        },
        source: "client_asserted",
        endpoint: endpoint.clone(),
        reference: Some(airs_runtime_core::tenant::scanners::ScannerReference {
            name: scanner.name.clone(),
            revision: scanner.revision.clone(),
            application_id: scanner.application_id.clone(),
            endpoint,
        }),
    };
    let cloud = airs_history_store::identity::Cloud::from_parts(
        &config.airs.tsg,
        [
            config.airs.management_endpoint.clone(),
            config.airs.dashboard_endpoint.clone(),
            config.airs.token_endpoint.clone(),
        ],
    )
    .unwrap();
    let run = uuid::Uuid::new_v4().to_string();
    let root = s.storage.history_root.clone();
    let counts = airs_history_store::repository::ingest_client_receipts(
        &root,
        &cloud,
        &run,
        &[received],
        &airs_runtime_core::error::SecretSet::default(),
        &Default::default(),
    )
    .unwrap();
    airs_history_store::repository::finish_received_within(
        &root,
        &cloud,
        &run,
        "complete",
        airs_history_store::repository::SERVICE_RECEIPT_BUDGET,
    )
    .unwrap();
    counts
}

fn receipt_state(s: &Service) -> String {
    let db = rusqlite::Connection::open_with_flags(
        archive_path(s),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    db.query_row("SELECT state FROM receipts", [], |r| r.get(0))
        .unwrap()
}

/// M4 exit gate: late and duplicate receipts reconcile in either order. A receipt
/// ingested before the collector saw its scan stays partial until the collection
/// promotes it; one ingested after the scan was collected promotes at once; a repeated
/// receipt key is a duplicate, not a second receipt.
#[test]
fn receipts_reconcile_with_collection_in_either_order() {
    // Receipt first, then collection.
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    assert_eq!(ingest_receipt(&s, "receipt-before-0001"), (1, 0));
    assert_eq!(receipt_state(&s), "partial", "no collected scan yet");
    submit(&s, "key-order-before-0001");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(
        receipt_state(&s),
        "promoted",
        "the collection promotes the early receipt"
    );
    assert_eq!(
        ingest_receipt(&s, "receipt-before-0001"),
        (0, 1),
        "a repeated key is a duplicate"
    );
    // Collection first, then the receipt.
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    submit(&s, "key-order-after-00001");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(ingest_receipt(&s, "receipt-after-00001"), (1, 0));
    assert_eq!(
        receipt_state(&s),
        "promoted",
        "a late receipt promotes against the collected scan"
    );
}

fn projection(s: &Service) -> (String, String, i64) {
    let db = rusqlite::Connection::open_with_flags(
        archive_path(s),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    db.query_row("SELECT verdict, action, conflicting FROM events", [], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })
    .unwrap()
}

/// M4 review 1 finding 1: a client-asserted receipt that contradicts the provider never
/// changes the authoritative projection (nor makes it conflicting), in either order.
#[test]
fn client_claims_never_change_the_provider_projection() {
    // Collection first, then a contradicting claim.
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    submit(&s, "key-claim-after-00001");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let provider = projection(&s);
    let forged = if provider.0 == "benign" {
        "malicious"
    } else {
        "benign"
    };
    let forged_action = if provider.1 == "block" {
        "allow"
    } else {
        "block"
    };
    ingest_claim(&s, "claim-after-000001", forged, forged_action);
    assert_eq!(receipt_state(&s), "promoted");
    assert_eq!(projection(&s), provider, "a late claim changes nothing");
    // A contradicting claim first, then the collection.
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    ingest_claim(&s, "claim-before-00001", forged, forged_action);
    submit(&s, "key-claim-before-0001");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(receipt_state(&s), "promoted");
    assert_eq!(projection(&s), provider, "an early claim changes nothing");
}

/// M4 review 1 finding 2: an API 401 gets exactly one fresh-token replay at the failed
/// read (a new token request, then the read again); a second 401 is final and recorded;
/// a 403 is never replayed.
#[test]
fn a_401_gets_exactly_one_fresh_token_replay_and_a_403_none() {
    use airs_test_harness::{Planned, Rx};
    let overview = Route::Runtime(Rx::SessionsOverview);
    // One 401: replayed with a fresh token, and the read succeeds.
    let (server, _sandbox) = fixture();
    server.push_once(overview, Planned::status(401));
    let s = service(&server);
    submit(&s, "key-replay-0000000001");
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert!(
        matches!(state, State::Succeeded | State::Partial),
        "{state:?}"
    );
    assert_eq!(
        server.requests_for(overview).len(),
        2,
        "the failed read, then its one replay"
    );
    assert_eq!(
        server.requests_for(Route::Token).len(),
        2,
        "the replay carried a fresh token"
    );
    let revision = s.shared.current().config.airs.management_revision.clone();
    assert!(
        !s.storage
            .control(|c| c.denied(&s.tenant, "management", &revision).unwrap())
    );
    // Two 401s: exactly one replay, then final (no third read).
    let (server, _sandbox) = fixture();
    server.set_default(overview, Planned::status(401));
    let s = service(&server);
    submit(&s, "key-replay-0000000002");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(
        server.requests_for(overview).len(),
        2,
        "never more than one replay"
    );
    // A 403: no replay.
    let (server, _sandbox) = fixture();
    server.push_once(overview, Planned::status(403));
    let s = service(&server);
    submit(&s, "key-replay-0000000003");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(
        server.requests_for(overview).len(),
        1,
        "a 403 is never replayed"
    );
}

/// M4 review 1 finding 3: a transient token failure (503) is a bounded retry
/// (`waiting_retry`), never a credential denial or a terminal failure; after a
/// credential rotation a parked job really collects again (a new run, new requests).
#[test]
fn token_outcomes_are_classified_and_a_rotation_resumes_collecting() {
    use airs_test_harness::Planned;
    let (server, _sandbox) = fixture();
    server.set_default(Route::Token, Planned::status(503));
    let s = service(&server);
    submit(&s, "key-token-5030000001");
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(
        state,
        State::WaitingRetry,
        "a transient token failure retries"
    );
    let revision = s.shared.current().config.airs.management_revision.clone();
    assert!(
        !s.storage
            .control(|c| c.denied(&s.tenant, "management", &revision).unwrap())
    );
    // Credentials refused: parked; rotation requeues; the job then collects anew.
    let (server, _sandbox) = fixture();
    server.set_default(
        Route::Token,
        Planned::json(401, &json!({"error": "invalid_client"})),
    );
    let s = service(&server);
    let parked = submit(&s, "key-token-4010000001");
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(state, State::NeedsCredentials);
    let (_, _, first_run) = s
        .storage
        .control(|c| c.last_checkpoint(&parked).unwrap().unwrap());
    server.set_default(Route::Token, Planned::builtin());
    let rotated = "00000000-0000-4000-8000-0000000000bb";
    let now = airs_runtime_core::time::now_ms();
    assert_eq!(
        s.storage.control(|c| c
            .credentials_restored(&s.tenant, "management", rotated, now)
            .unwrap()),
        1
    );
    // The service now runs with the rotated revision.
    let text = std::fs::read_to_string(s._base.0.join("config.toml")).ok();
    let _ = text;
    let rotated_live = {
        let current = s.shared.current();
        let mut config = (*current.config).clone();
        config.airs.management_revision = rotated.into();
        Live::new(config).unwrap()
    };
    s.shared.swap(rotated_live);
    let before = server.requests().len();
    let (stepped, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(stepped, parked);
    assert!(
        matches!(state, State::Succeeded | State::Partial),
        "{state:?}"
    );
    assert!(
        server.requests().len() > before,
        "the resumed job collected"
    );
    let (_, _, second_run) = s
        .storage
        .control(|c| c.last_checkpoint(&parked).unwrap().unwrap());
    assert_ne!(first_run, second_run, "a new run, not the failed one");
}

/// M4 review 1 finding 5: an ordinary recent tick after a valid traversal asks only for
/// the narrowest selector covering the last traversal minus the overlap (1 h here), not
/// a fresh 24 h.
#[test]
fn a_later_tick_reads_only_since_the_last_traversal() {
    use airs_test_harness::Rx;
    let (server, _sandbox) = fixture();
    let s = service(&server);
    submit(&s, "key-window-000000001");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let interval = |r: &airs_test_harness::RecordedRequest| {
        r.query
            .iter()
            .find(|(k, _)| k == "time_interval")
            .map(|(_, v)| v.clone())
    };
    let first: Vec<_> = server
        .requests_for(Route::Runtime(Rx::SessionsOverview))
        .iter()
        .filter_map(interval)
        .collect();
    // The first collection's target is the 24 h resolved (and checkpointed) when the job
    // starts; the collector, a moment later, needs the narrowest whole-hour selector that
    // still covers that target's start: 24 h, or 25 h when the hour boundary was crossed
    // (it always is by a few milliseconds). The archive bounds are the exact target.
    assert!(
        matches!(first.first().map(String::as_str), Some("24" | "25")),
        "the first collection reads its 24 h target: {first:?}"
    );
    submit(&s, "key-window-000000002");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let all: Vec<_> = server
        .requests_for(Route::Runtime(Rx::SessionsOverview))
        .iter()
        .filter_map(interval)
        .collect();
    assert_eq!(
        all.last().map(String::as_str),
        Some("1"),
        "the next tick reads only the last hour: {all:?}"
    );
}

/// M4 review 1 finding 6: cancellation during a delayed response stops the collector:
/// no request is admitted after the cancellation, and the job ends `canceled`.
#[test]
fn cancellation_stops_further_sends() {
    use airs_test_harness::{Planned, Rx};
    let (server, _sandbox) = fixture();
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::builtin().with_delay(std::time::Duration::from_millis(1500)),
    );
    let s = service(&server);
    let id = submit(&s, "key-cancel-0000000001");
    let storage = s.storage.clone();
    let tenant = s.tenant.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        storage.control(|c| {
            c.cancel(&tenant, &id, airs_runtime_core::time::now_ms())
                .unwrap()
        });
    });
    let w = worker(&s);
    let (_, state) = w.step(&s.tenant, |_| true).unwrap().unwrap();
    canceller.join().unwrap();
    assert_eq!(state, State::Canceled);
    let after = server.requests().len();
    std::thread::sleep(std::time::Duration::from_millis(1800));
    assert_eq!(
        server.requests().len(),
        after,
        "nothing is sent after the cancellation"
    );
    assert!(
        server.requests_for(Route::Runtime(Rx::Session)).is_empty(),
        "no session read after cancel"
    );
}

fn cloud_of(s: &Service) -> airs_history_store::identity::Cloud {
    let config = s.shared.current().config.clone();
    airs_history_store::identity::Cloud::from_parts(
        &config.airs.tsg,
        [
            config.airs.management_endpoint.clone(),
            config.airs.dashboard_endpoint.clone(),
            config.airs.token_endpoint.clone(),
        ],
    )
    .unwrap()
}

/// What one archive run consumed (API + OAuth + Results + unconfirmed).
fn consumption(s: &Service, run: &str) -> u64 {
    let r = airs_history_store::run_summary::run_summary(
        &s.storage.history_root,
        &cloud_of(s),
        run,
        &scm::Session::new(),
    )
    .unwrap()
    .unwrap();
    r.management_sends + r.oauth_sends + r.results_sends + r.unconfirmed_reads
}

/// Claim a fresh job as a dead owner and leave it at `phase` with `state` (C4 / a
/// continuation), then recover.
fn job_at(s: &Service, key: &str, phase: &str, state: Value, op: Option<&str>) -> String {
    let id = submit(s, key);
    s.storage.control(|c| {
        let now = airs_runtime_core::time::now_ms();
        c.claim_next(&s.tenant, "owner-dead", now, |_| true)
            .unwrap()
            .unwrap();
        c.checkpoint(&id, phase, &state, op, now).unwrap();
        c.recover(now).unwrap();
    });
    id
}

fn with_tick_sends(s: &Service, sends: u64) {
    let current = s.shared.current();
    let mut config = (*current.config).clone();
    config.limits.tick_sends = sends;
    s.shared.swap(Live::new(config).unwrap());
}

/// M4 review 2 finding 2: a terminal API refusal (a 403 here) is a credential denial
/// even when another read of the same run returned data; the revision is recorded and
/// a later tick makes no request.
#[test]
fn a_peer_success_never_masks_a_terminal_denial() {
    use airs_test_harness::{Planned, Rx};
    let (server, _sandbox) = fixture();
    server.set_default(Route::Runtime(Rx::Session), Planned::status(403));
    let s = service(&server);
    submit(&s, "key-peer-403-000001");
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert!(
        !server
            .requests_for(Route::Runtime(Rx::SessionsOverview))
            .is_empty(),
        "a peer read returned data"
    );
    assert_eq!(state, State::NeedsCredentials);
    let revision = s.shared.current().config.airs.management_revision.clone();
    assert!(
        s.storage
            .control(|c| c.denied(&s.tenant, "management", &revision).unwrap())
    );
    let before = server.requests().len();
    submit(&s, "key-peer-403-000002");
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(state, State::NeedsCredentials);
    assert_eq!(
        server.requests().len(),
        before,
        "a denied revision is never retried"
    );
}

/// M4 review 2 finding 3: a job that waited for a transient token failure actually
/// retries its unfinished work when due (a new run), instead of finishing from the
/// failed run.
#[test]
fn a_transient_token_failure_retries_the_unfinished_work_when_due() {
    use airs_test_harness::Planned;
    let (server, _sandbox) = fixture();
    server.set_default(Route::Token, Planned::status(503));
    let s = service(&server);
    let id = submit(&s, "key-retry-000000001");
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(state, State::WaitingRetry);
    server.set_default(Route::Token, Planned::builtin());
    let later = airs_runtime_core::time::now_ms() + 120_000;
    s.storage
        .control(|c| c.scheduler_pass(&s.tenant, later).unwrap());
    let before = server
        .requests_for(Route::Runtime(airs_test_harness::Rx::SessionsOverview))
        .len();
    let (stepped, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(stepped, id);
    assert!(
        matches!(state, State::Succeeded | State::Partial),
        "{state:?}"
    );
    assert!(
        server
            .requests_for(Route::Runtime(airs_test_harness::Rx::SessionsOverview))
            .len()
            > before,
        "the retry collected"
    );
    let r = job(&s, &id).result.unwrap();
    assert!(r["sends"]["management"].as_u64().unwrap_or(0) > 0, "{r}");
}

/// M4 review 2 finding 3 (enrichment): a checkpointed enrichment run that never
/// reached the archive is made under its checkpointed ID.
#[test]
fn an_enrichment_run_without_an_archive_row_keeps_its_checkpointed_id() {
    let (server, _sandbox) = fixture();
    seed_examples(&server);
    let s = service_with(&server, "uuid-shared");
    let run = uuid::Uuid::new_v4().to_string();
    let id = submit_enrichment(&s, "key-enrich-norow-001");
    s.storage.control(|c| {
        let now = airs_runtime_core::time::now_ms();
        c.claim_next(&s.tenant, "owner-dead", now, |_| true)
            .unwrap()
            .unwrap();
        c.checkpoint(&id, COLLECTING, &json!({"run_id": run}), Some(&run), now)
            .unwrap();
        c.recover(now).unwrap();
    });
    enrichment_worker(&s)
        .step(&s.tenant, |_| true)
        .unwrap()
        .unwrap();
    let db = rusqlite::Connection::open_with_flags(
        archive_path(&s),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let mine: i64 = db
        .query_row(
            "SELECT count(*) FROM collection_runs WHERE run_id=?1",
            [&run],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(mine, 1, "the checkpointed ID is the run");
}

/// M4 review 2 finding 4: a logical job's traffic allowance spans every run it makes. A
/// continuation after earlier runs gets only what remains; a spent allowance sends
/// nothing.
#[test]
fn continuation_runs_get_only_the_jobs_remaining_allowance() {
    let (server, _sandbox) = fixture();
    let s = service(&server);
    let first = submit(&s, "key-budget-00000001");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let run1 = s
        .storage
        .control(|c| c.last_checkpoint(&first).unwrap().unwrap())
        .2
        .unwrap();
    let used = consumption(&s, &run1);
    assert!(used > 2, "the first run made several sends: {used}");
    let now = airs_runtime_core::time::now_ms();
    let scope = json!({"range": null, "target": [now - 86_400_000, now], "apps": [], "runs": [run1], "reason": "credentials"});
    // Two sends remain: the continuation run stays within them.
    with_tick_sends(&s, used + 2);
    let cont = job_at(
        &s,
        "key-budget-00000002",
        airs_mcp::collection::CONTINUE,
        scope.clone(),
        None,
    );
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let run2 = s
        .storage
        .control(|c| c.last_checkpoint(&cont).unwrap().unwrap())
        .2
        .unwrap();
    assert_ne!(run2, run1);
    assert!(
        consumption(&s, &run2) <= 2,
        "only the remaining allowance: {}",
        consumption(&s, &run2)
    );
    // Nothing remains: refused before any request.
    with_tick_sends(&s, used);
    let before = server.requests().len();
    let spent = job_at(
        &s,
        "key-budget-00000003",
        airs_mcp::collection::CONTINUE,
        scope,
        None,
    );
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(state, State::Failed);
    assert_eq!(
        job(&s, &spent).result.unwrap()["error"]["code"],
        "quota_exhausted"
    );
    assert_eq!(
        server.requests().len(),
        before,
        "a spent allowance sends nothing"
    );
}

/// M4 review 2 finding 5: the resolved scope is checkpointed; a recovered run is made
/// against the original bounds, not ones recomputed at recovery time.
#[test]
fn recovery_keeps_the_original_scope() {
    let (server, _sandbox) = fixture();
    let s = service(&server);
    let run = uuid::Uuid::new_v4().to_string();
    let now = airs_runtime_core::time::now_ms();
    let (since, until) = (now - 2 * 3_600_000, now - 3_600_000);
    let state = json!({"run_id": run, "range": [since, until], "target": [since, until], "apps": [], "runs": []});
    job_at(&s, "key-scope-000000001", COLLECTING, state, Some(&run));
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let db = rusqlite::Connection::open_with_flags(
        archive_path(&s),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let (a, b): (i64, i64) = db
        .query_row(
            "SELECT target_since_ms, target_until_ms FROM collection_runs WHERE run_id=?1",
            [&run],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((a, b), (since, until), "the original bounds");
}

/// M4 review 2 finding 6: cancellation is enforced at every outbound admission, not
/// only when the watcher polls: with the watcher effectively idle, a cancellation
/// committed while the first response is in flight admits no further send.
#[test]
fn a_cancellation_between_polls_admits_no_further_send() {
    use airs_test_harness::{Planned, Rx};
    let (server, _sandbox) = fixture();
    server.set_default(
        Route::Runtime(Rx::SessionsOverview),
        Planned::builtin().with_delay(std::time::Duration::from_millis(800)),
    );
    let s = service(&server);
    let id = submit(&s, "key-cancel-poll-0001");
    let storage = s.storage.clone();
    let tenant = s.tenant.clone();
    let previous =
        airs_mcp::collection::WATCHER_POLL_MS.swap(60_000, std::sync::atomic::Ordering::SeqCst);
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(250));
        storage.control(|c| {
            c.cancel(&tenant, &id, airs_runtime_core::time::now_ms())
                .unwrap()
        });
    });
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    airs_mcp::collection::WATCHER_POLL_MS.store(previous, std::sync::atomic::Ordering::SeqCst);
    canceller.join().unwrap();
    assert_eq!(state, State::Canceled);
    assert!(
        server.requests_for(Route::Runtime(Rx::Session)).is_empty(),
        "no read was admitted after the cancellation"
    );
}

/// Leave an already submitted job at `phase` with `state`, claimed by a dead owner, then
/// recover (crash point C4).
fn crash_job(s: &Service, id: &str, phase: &str, state: Value, op: Option<&str>) {
    s.storage.control(|c| {
        let now = airs_runtime_core::time::now_ms();
        c.claim_next(&s.tenant, "owner-dead", now, |_| true)
            .unwrap()
            .unwrap();
        c.checkpoint(id, phase, &state, op, now).unwrap();
        c.recover(now).unwrap();
    });
}

/// A checkpoint scope bound to `revision`, for the last 24 hours.
fn bound_scope(revision: &str) -> Value {
    let now = airs_runtime_core::time::now_ms();
    json!({"range": [now - 86_400_000, now], "target": [now - 86_400_000, now], "apps": [], "runs": [], "credential_revision": revision})
}

fn seed(s: &Service, run: &str, reads: &[(&str, &str, i64)]) {
    let now = airs_runtime_core::time::now_ms();
    airs_history_store::run_summary::seed_run_for_tests(
        &s.storage.history_root,
        &cloud_of(s),
        run,
        (now - 86_400_000, now),
        "partial",
        reads,
    )
    .unwrap();
}

/// M4 review 3 finding 2 (the reviewer's probe): a process died after the archive
/// recorded a terminal denial (a 403) of the run's revision but before the control store
/// did. Recovery records the denial, parks the job `needs_credentials` and makes no new
/// attempt (no reservation, no request) while that revision is configured.
#[test]
fn a_crash_after_a_denial_parks_the_job_without_a_new_attempt() {
    let (server, _sandbox) = fixture();
    let s = service(&server);
    let revision = s.shared.current().config.airs.management_revision.clone();
    let run = uuid::Uuid::new_v4().to_string();
    seed(
        &s,
        &run,
        &[(
            "airs.management.dashboard.sessions_overview",
            "auth_denied",
            403,
        )],
    );
    let id = job_at(
        &s,
        "key-crash-denial-0001",
        COLLECTING,
        bound_scope(&revision),
        Some(&run),
    );
    let before = server.requests().len();
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(state, State::NeedsCredentials);
    assert_eq!(
        s.storage.control(|c| c.consumed_sends(&id).unwrap()),
        0,
        "no new reservation"
    );
    assert_eq!(server.requests().len(), before, "no new request");
    assert!(
        s.storage
            .control(|c| c.denied(&s.tenant, "management", &revision).unwrap())
    );
    // A run bound to an older revision is a rotation: the job continues with a new run.
    let run = uuid::Uuid::new_v4().to_string();
    seed(
        &s,
        &run,
        &[(
            "airs.management.dashboard.sessions_overview",
            "auth_denied",
            403,
        )],
    );
    let rotated = job_at(
        &s,
        "key-crash-denial-0002",
        COLLECTING,
        bound_scope("00000000-0000-4000-8000-0000000000ee"),
        Some(&run),
    );
    let (stepped, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(stepped, rotated);
    // The configured revision is now denied (above), so the continuation parks again
    // without sending; what matters is that it did not finish from the old run.
    assert_eq!(state, State::NeedsCredentials);
}

/// M4 review 3 finding 2 (the reviewer's probe): a process died after recording a
/// transient token failure but before the continue checkpoint. Recovery keeps the
/// job's retry eligibility (`waiting_retry`), instead of finishing it `partial`.
#[test]
fn a_crash_after_a_transient_token_failure_keeps_retry_eligibility() {
    let (server, _sandbox) = fixture();
    let s = service(&server);
    let revision = s.shared.current().config.airs.management_revision.clone();
    let run = uuid::Uuid::new_v4().to_string();
    seed(&s, &run, &[("oauth.token", "failed", 503)]);
    job_at(
        &s,
        "key-crash-transient-01",
        COLLECTING,
        bound_scope(&revision),
        Some(&run),
    );
    let before = server.requests().len();
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(state, State::WaitingRetry);
    assert_eq!(server.requests().len(), before);
}

/// M4 review 3 finding 2, enrichment: the same reconciliation applies to the enrichment
/// executor (a crash after a recorded denial parks it without a new attempt).
#[test]
fn an_enrichment_crash_after_a_denial_parks_without_a_new_attempt() {
    let (server, _sandbox) = fixture();
    let s = service(&server);
    let revision = s.shared.current().config.airs.management_revision.clone();
    let run = uuid::Uuid::new_v4().to_string();
    seed(
        &s,
        &run,
        &[(
            "airs.management.dashboard.sessions_overview",
            "auth_denied",
            403,
        )],
    );
    let id = submit_enrichment(&s, "key-enrich-crash-0001");
    crash_job(&s, &id, COLLECTING, bound_scope(&revision), Some(&run));
    let before = server.requests().len();
    let (_, state) = enrichment_worker(&s)
        .step(&s.tenant, |_| true)
        .unwrap()
        .unwrap();
    assert_eq!(state, State::NeedsCredentials);
    assert_eq!(server.requests().len(), before);
}

/// M4 review 3 finding 3 (the reviewer's probe): a first collection checkpoints a null
/// range and its concrete target; recovered an hour later with no archive row, the run
/// collects exactly that target, not a window from the recovery clock.
#[test]
fn a_null_range_recovery_collects_the_checkpointed_target() {
    let (server, _sandbox) = fixture();
    let s = service(&server);
    let revision = s.shared.current().config.airs.management_revision.clone();
    let run = uuid::Uuid::new_v4().to_string();
    let old = airs_runtime_core::time::now_ms() - 3_600_000;
    let scope = json!({"range": null, "target": [old - 86_400_000, old], "apps": [], "runs": [], "credential_revision": revision});
    job_at(&s, "key-null-range-000001", COLLECTING, scope, Some(&run));
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let db = rusqlite::Connection::open_with_flags(
        archive_path(&s),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let bounds: (i64, i64) = db
        .query_row(
            "SELECT target_since_ms, target_until_ms FROM collection_runs WHERE run_id=?1",
            [&run],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        bounds,
        (old - 86_400_000, old),
        "the checkpointed target is the run's"
    );
}

/// A recovered run seeded with `reads` in archive state `state`, recovered by the
/// collection or the enrichment executor: its final job state, whether the bound
/// revision is denied, the management hold, the job's consumed reservations and the
/// run's archive state afterwards.
fn recover_seeded(
    state: &str,
    reads: &[(&str, &str, i64)],
    enrich: bool,
) -> (State, bool, Option<i64>, u64, String) {
    let (server, _sandbox) = fixture();
    let s = service_with(&server, "app-example");
    let rev = s.shared.current().config.airs.management_revision.clone();
    let run = uuid::Uuid::new_v4().to_string();
    let now = airs_runtime_core::time::now_ms();
    airs_history_store::run_summary::seed_run_for_tests(
        &s.storage.history_root,
        &cloud_of(&s),
        &run,
        (now - 86_400_000, now),
        state,
        reads,
    )
    .unwrap();
    let id = if enrich {
        let id = submit_enrichment(&s, "key-recover-seeded-01");
        crash_job(&s, &id, COLLECTING, bound_scope(&rev), Some(&run));
        id
    } else {
        job_at(
            &s,
            "key-recover-seeded-01",
            COLLECTING,
            bound_scope(&rev),
            Some(&run),
        )
    };
    let w = if enrich {
        enrichment_worker(&s)
    } else {
        worker(&s)
    };
    let (_, out) = w.step(&s.tenant, |_| true).unwrap().unwrap();
    let denied = s
        .storage
        .control(|c| c.denied(&s.tenant, "management", &rev).unwrap());
    let hold = s
        .storage
        .control(|c| c.cooldown_until(&s.tenant, "management", now).unwrap());
    let used = s.storage.control(|c| c.consumed_sends(&id).unwrap());
    let db = rusqlite::Connection::open_with_flags(
        archive_path(&s),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let archived: String = db
        .query_row(
            "SELECT state FROM collection_runs WHERE run_id=?1",
            [&run],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(server.requests().len(), 0, "recovery itself sends nothing");
    (out, denied, hold, used, archived)
}

/// M4 review 4 finding 2 (the reviewer's probe): a process died before finalizing a
/// run whose journal holds a 403. Both executors reconcile the denial before the
/// running-run shortcut: the bound revision is recorded denied, the job waits for new
/// credentials with no new reservation, and the dead run is closed `interrupted`.
#[test]
fn a_running_run_with_a_journaled_denial_parks_both_executors() {
    for enrich in [false, true] {
        let (out, denied, _, used, archived) = recover_seeded(
            "running",
            &[(
                "airs.management.dashboard.sessions_overview",
                "auth_denied",
                403,
            )],
            enrich,
        );
        assert_eq!(out, State::NeedsCredentials, "enrichment={enrich}");
        assert!(
            denied,
            "the revision block is persisted (enrichment={enrich})"
        );
        assert_eq!(used, 0, "no further attempt (enrichment={enrich})");
        assert_eq!(archived, "interrupted", "enrichment={enrich}");
    }
}

/// M4 review 4 finding 3 (the reviewer's probe): recovery of a finished run whose
/// journal holds a management 429 restores the plane's hold from the durable refusal
/// time, for both executors, before any further work.
#[test]
fn a_recovered_429_restores_the_management_hold_both_executors() {
    for enrich in [false, true] {
        let (_, _, hold, _, _) = recover_seeded(
            "partial",
            &[(
                "airs.management.dashboard.sessions_overview",
                "rate_limited",
                429,
            )],
            enrich,
        );
        let until = hold.expect("the hold is restored");
        assert!(
            until > airs_runtime_core::time::now_ms() + 14 * 60_000,
            "held for the 15-minute window from the refusal (enrichment={enrich})"
        );
    }
}

/// M4 review 4 finding 3, mixed outcomes: a run with both a 403 and a 429 leaves the
/// denial recorded and the hold restored.
#[test]
fn mixed_denial_and_429_restore_both_on_recovery() {
    for enrich in [false, true] {
        let (out, denied, hold, used, _) = recover_seeded(
            "running",
            &[
                (
                    "airs.management.dashboard.sessions_overview",
                    "rate_limited",
                    429,
                ),
                ("airs.management.dashboard.session", "auth_denied", 403),
            ],
            enrich,
        );
        assert_eq!(out, State::NeedsCredentials, "enrichment={enrich}");
        assert!(denied && hold.is_some() && used == 0, "enrichment={enrich}");
    }
}

/// The control store's own path, for fault injection and deadline probes.
fn control_db(s: &Service) -> rusqlite::Connection {
    s.storage
        .control(|c| rusqlite::Connection::open(c.path()).unwrap())
}

/// A journaled 403 in a run the dead process left `running`, bound to the configured
/// revision through its job's checkpoint (as the reviewer seeded it: no archive-side
/// binding) or through the run's provenance, and that job, crashed in `collecting`.
fn crashed_denial(s: &Service, enrich: bool, key: &str, archive_bound: bool) -> (String, String) {
    crashed_with(
        s,
        enrich,
        key,
        archive_bound,
        &[(
            "airs.management.dashboard.sessions_overview",
            "auth_denied",
            403,
        )],
    )
}

/// [`crashed_denial`] with the run's journal holding `reads`.
fn crashed_with(
    s: &Service,
    enrich: bool,
    key: &str,
    archive_bound: bool,
    reads: &[(&str, &str, i64)],
) -> (String, String) {
    let revision = s.shared.current().config.airs.management_revision.clone();
    let run = uuid::Uuid::new_v4().to_string();
    let now = airs_runtime_core::time::now_ms();
    airs_history_store::run_summary::seed_bound_run_for_tests(
        &s.storage.history_root,
        &cloud_of(s),
        &run,
        (now - 86_400_000, now),
        "running",
        reads,
        archive_bound.then_some(revision.as_str()),
    )
    .unwrap();
    crash_seeded(s, enrich, key, archive_bound, &run)
}

/// A job of either executor crashed in `collecting` on the already seeded `run`, bound
/// to the configured revision through its checkpoint or (`archive_bound`) only through
/// the run's provenance.
fn crash_seeded(
    s: &Service,
    enrich: bool,
    key: &str,
    archive_bound: bool,
    run: &str,
) -> (String, String) {
    let revision = s.shared.current().config.airs.management_revision.clone();
    let run = run.to_owned();
    let id = if enrich {
        submit_enrichment(s, key)
    } else {
        submit(s, key)
    };
    let scope = if archive_bound {
        // A checkpoint that carries no binding: the archive's is the only one.
        let mut v = bound_scope(&revision);
        v.as_object_mut().unwrap().remove("credential_revision");
        v
    } else {
        bound_scope(&revision)
    };
    crash_job(s, &id, COLLECTING, scope, Some(&run));
    (id, run)
}

fn blocked(s: &Service) -> bool {
    let revision = s.shared.current().config.airs.management_revision.clone();
    airs_mcp::collection::revision_blocked(&s.storage, &s.tenant, &cloud_of(s), &revision).unwrap()
}

fn recorded(s: &Service) -> bool {
    let revision = s.shared.current().config.airs.management_revision.clone();
    s.storage
        .control(|c| c.denied(&s.tenant, "management", &revision).unwrap())
}

/// A later job of either executor is refused before any send: `needs_credentials`, no
/// reservation, no request.
fn next_job_makes_no_attempt(s: &Service, server: &FixtureServer, key: &str, context: &str) {
    for enrich in [false, true] {
        let key = format!("{key}-{}", if enrich { "e" } else { "c" });
        let next = if enrich {
            submit_enrichment(s, &key)
        } else {
            submit(s, &key)
        };
        let before = server.requests().len();
        let w = if enrich {
            enrichment_worker(s)
        } else {
            worker(s)
        };
        let (stepped, state) = w.step(&s.tenant, |_| true).unwrap().unwrap();
        assert_eq!(stepped, next, "{context}");
        assert_eq!(
            state,
            State::NeedsCredentials,
            "{context} next enrichment={enrich}"
        );
        assert_eq!(
            s.storage.control(|c| c.consumed_sends(&next).unwrap()),
            0,
            "no reservation ({context} next enrichment={enrich})"
        );
        assert_eq!(server.requests().len(), before, "no request ({context})");
    }
}

/// M4 review 5 finding 1 (the reviewer's probe): a job whose run journaled a 403 and
/// whose process died is canceled, or expires (at its claim, or waiting in the
/// scheduler pass), before its executor reconciles the run. The job keeps its canceled
/// or failed state; the revision is blocked from the archive journal at once and
/// recorded by the worker tick's reconciliation pass; the next job of either executor
/// makes no attempt. Both executors, both kinds of run binding.
#[test]
fn a_canceled_or_expired_job_keeps_its_state_and_the_revision_stays_blocked() {
    for enrich in [false, true] {
        for archive_bound in [false, true] {
            for how in ["cancel", "expire_at_claim", "expire_in_pass"] {
                let context = format!("enrichment={enrich} archive_bound={archive_bound} {how}");
                let (server, _sandbox) = fixture();
                let s = service(&server);
                let (id, _) = crashed_denial(&s, enrich, "key-terminal-denial-01", archive_bound);
                let now = airs_runtime_core::time::now_ms();
                let w = if enrich {
                    enrichment_worker(&s)
                } else {
                    worker(&s)
                };
                match how {
                    "cancel" => {
                        s.storage.control(|c| {
                            c.cancel(&s.tenant, &id, now).unwrap();
                            c.scheduler_pass(&s.tenant, now).unwrap();
                        });
                        assert_eq!(job(&s, &id).state, State::Canceled, "{context}");
                    }
                    "expire_at_claim" => {
                        control_db(&s)
                            .execute(
                                "UPDATE jobs SET deadline_ms=?1 WHERE job_id=?2",
                                rusqlite::params![now - 1, id],
                            )
                            .unwrap();
                        assert!(w.step(&s.tenant, |_| true).unwrap().is_none(), "{context}");
                        assert_eq!(job(&s, &id).state, State::Failed, "{context}");
                    }
                    _ => {
                        control_db(&s)
                            .execute(
                                "UPDATE jobs SET state='waiting_retry', retry_after_ms=?1, deadline_ms=?1 WHERE job_id=?2",
                                rusqlite::params![now - 1, id],
                            )
                            .unwrap();
                        s.storage
                            .control(|c| c.scheduler_pass(&s.tenant, now).unwrap());
                        assert_eq!(job(&s, &id).state, State::Failed, "{context}");
                    }
                }
                assert!(!recorded(&s), "no executor reconciled the run ({context})");
                // The worker tick's reconciliation pass persists the journal's denial and
                // leaves the terminal job as it is.
                airs_mcp::run::tick_passes_for_tests(&w, &s.shared);
                assert!(recorded(&s), "the revision block is recorded ({context})");
                let terminal = job(&s, &id).state;
                assert!(
                    matches!(terminal, State::Canceled | State::Failed),
                    "{context}: {terminal:?}"
                );
                next_job_makes_no_attempt(&s, &server, "key-terminal-denial-02", &context);
                assert_eq!(job(&s, &id).state, terminal, "{context}");
                assert_eq!(server.requests().len(), 0, "{context}");
            }
        }
    }
}

/// M4 review 5 finding 1 without the reconciliation pass: admission itself reads the
/// archive journal, so the next job is refused even before any tick persisted it.
#[test]
fn admission_reads_the_journal_before_any_reconciliation() {
    for enrich in [false, true] {
        let (server, _sandbox) = fixture();
        let s = service(&server);
        let (id, _) = crashed_denial(&s, enrich, "key-journal-admission-01", false);
        let now = airs_runtime_core::time::now_ms();
        s.storage.control(|c| {
            c.cancel(&s.tenant, &id, now).unwrap();
            c.scheduler_pass(&s.tenant, now).unwrap();
        });
        assert!(!recorded(&s));
        next_job_makes_no_attempt(
            &s,
            &server,
            "key-journal-admission-02",
            &format!("enrichment={enrich}"),
        );
        assert!(
            blocked(&s),
            "the journal blocks the revision (enrichment={enrich})"
        );
        assert_eq!(job(&s, &id).state, State::Canceled);
    }
}

/// M4 review 5 finding 2 (the reviewer's probe): the denial insertion fails (an
/// injected SQLite fault). The first job parks; every other job is still refused
/// admission, from the archive journal, while the fault lasts; once it clears, the
/// worker tick's reconciliation pass records the denial. Both executors.
#[test]
fn a_failed_denial_write_still_refuses_every_job_until_reconciled() {
    for enrich in [false, true] {
        let context = format!("enrichment={enrich}");
        let (server, _sandbox) = fixture();
        let s = service(&server);
        let (id, _) = crashed_denial(&s, enrich, "key-failed-denial-01", false);
        control_db(&s)
            .execute_batch("CREATE TRIGGER fail_denial BEFORE INSERT ON credential_denials BEGIN SELECT RAISE(ABORT, 'injected failure'); END;")
            .unwrap();
        let w = if enrich {
            enrichment_worker(&s)
        } else {
            worker(&s)
        };
        let (stepped, state) = w.step(&s.tenant, |_| true).unwrap().unwrap();
        assert_eq!(
            (stepped.as_str(), state),
            (id.as_str(), State::NeedsCredentials)
        );
        assert!(!recorded(&s), "the fault refused the record ({context})");
        next_job_makes_no_attempt(&s, &server, "key-failed-denial-02", &context);
        // The pass cannot persist it while the fault lasts.
        airs_mcp::run::tick_passes_for_tests(&w, &s.shared);
        assert!(!recorded(&s), "{context}");
        assert!(blocked(&s), "{context}");
        control_db(&s)
            .execute_batch("DROP TRIGGER fail_denial;")
            .unwrap();
        airs_mcp::run::tick_passes_for_tests(&w, &s.shared);
        assert!(
            recorded(&s),
            "reconciled once the fault cleared ({context})"
        );
        next_job_makes_no_attempt(&s, &server, "key-failed-denial-03", &context);
        assert_eq!(server.requests().len(), 0, "{context}");
    }
}

/// M4 review 5 finding 3 (the reviewer's probe): a run the dead process left `running`
/// whose journal holds a transient OAuth failure (a 429, a 503) and no API data. Both
/// executors keep retry eligibility: the job waits for a retry (`waiting_retry`), the
/// run is closed `interrupted`, the continuation keeps the checkpointed scope and the
/// run (its consumption), the job keeps its original deadline and reservations, and
/// nothing is sent. The same record already finished gives the same outcome.
#[test]
fn a_running_run_with_a_transient_token_failure_waits_for_retry_both_executors() {
    for enrich in [false, true] {
        for (outcome, status) in [("rate_limited", 429), ("failed", 503)] {
            for archived in ["running", "partial"] {
                let context = format!("enrichment={enrich} status={status} archive={archived}");
                let (server, _sandbox) = fixture();
                let s = service(&server);
                let revision = s.shared.current().config.airs.management_revision.clone();
                let run = uuid::Uuid::new_v4().to_string();
                let now = airs_runtime_core::time::now_ms();
                airs_history_store::run_summary::seed_run_for_tests(
                    &s.storage.history_root,
                    &cloud_of(&s),
                    &run,
                    (now - 86_400_000, now),
                    archived,
                    &[("oauth.token", outcome, status)],
                )
                .unwrap();
                let id = if enrich {
                    submit_enrichment(&s, "key-transient-running-01")
                } else {
                    submit(&s, "key-transient-running-01")
                };
                let scope = bound_scope(&revision);
                crash_job(&s, &id, COLLECTING, scope.clone(), Some(&run));
                let deadline = job(&s, &id).deadline_ms;
                let reserved = s.storage.control(|c| c.consumed_sends(&id).unwrap());
                let w = if enrich {
                    enrichment_worker(&s)
                } else {
                    worker(&s)
                };
                let (_, state) = w.step(&s.tenant, |_| true).unwrap().unwrap();
                assert_eq!(state, State::WaitingRetry, "{context}");
                let after = job(&s, &id);
                assert_eq!(
                    after.deadline_ms, deadline,
                    "the original deadline ({context})"
                );
                assert_eq!(
                    s.storage.control(|c| c.consumed_sends(&id).unwrap()),
                    reserved,
                    "no new reservation ({context})"
                );
                let (phase, state, _) = s
                    .storage
                    .control(|c| c.last_checkpoint(&id).unwrap().unwrap());
                assert_eq!(phase, "continue", "{context}");
                assert_eq!(state["target"], scope["target"], "the scope ({context})");
                assert_eq!(state["range"], scope["range"], "the scope ({context})");
                assert_eq!(
                    state["runs"],
                    json!([run]),
                    "the run's consumption ({context})"
                );
                assert_eq!(state["reason"], "retry", "{context}");
                let closed: String = rusqlite::Connection::open_with_flags(
                    archive_path(&s),
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                )
                .unwrap()
                .query_row(
                    "SELECT state FROM collection_runs WHERE run_id=?1",
                    [&run],
                    |r| r.get(0),
                )
                .unwrap();
                let expected = if archived == "running" {
                    "interrupted"
                } else {
                    archived
                };
                assert_eq!(closed, expected, "{context}");
                assert!(
                    !recorded(&s),
                    "a transient failure is no denial ({context})"
                );
                assert_eq!(server.requests().len(), 0, "{context}");
            }
        }
    }
}

/// The archive authority's classification (contract §6.1): a 403 denies at once, a
/// single unanswered 401 only once its run is finished (its replay may be pending), an
/// exhausted replay (two refusals) at once; a recovered 401 never; an unbound run or a
/// run of another revision never.
#[test]
fn the_journal_authority_classifies_denials_by_their_definitive_outcome() {
    let other = "00000000-0000-4000-8000-0000000000ff";
    let src = "airs.management.dashboard.sessions_overview";
    type Case<'a> = (&'a str, Vec<(&'a str, &'a str, i64)>, Option<&'a str>, bool);
    let cases: Vec<Case> = vec![
        (
            "running",
            vec![(src, "auth_denied", 403)],
            Some("this"),
            true,
        ),
        (
            "running",
            vec![(src, "auth_denied", 401)],
            Some("this"),
            false,
        ),
        (
            "partial",
            vec![(src, "auth_denied", 401)],
            Some("this"),
            true,
        ),
        (
            "running",
            vec![(src, "auth_denied", 401), (src, "auth_denied", 401)],
            Some("this"),
            true,
        ),
        (
            "complete",
            vec![(src, "auth_denied", 401), (src, "returned", 200)],
            Some("this"),
            false,
        ),
        (
            "partial",
            vec![("oauth.token", "auth_denied", 401)],
            Some("this"),
            true,
        ),
        (
            "partial",
            vec![("oauth.token", "failed", 503)],
            Some("this"),
            false,
        ),
        // M4 review 6 finding 1: a token refusal is definitive while the run is still
        // running (no replay of a token refusal is ever pending), even after API data.
        (
            "running",
            vec![("oauth.token", "auth_denied", 401)],
            Some("this"),
            true,
        ),
        (
            "running",
            vec![("oauth.token", "auth_denied", 403)],
            Some("this"),
            true,
        ),
        (
            "running",
            vec![
                ("oauth.token", "returned", 200),
                (src, "returned", 200),
                ("oauth.token", "auth_denied", 401),
            ],
            Some("this"),
            true,
        ),
        (
            "running",
            vec![("oauth.token", "rate_limited", 429)],
            Some("this"),
            false,
        ),
        (
            "partial",
            vec![(src, "auth_denied", 403)],
            Some(other),
            false,
        ),
        ("partial", vec![(src, "auth_denied", 403)], None, false),
    ];
    for (n, (state, reads, bound, expected)) in cases.into_iter().enumerate() {
        let (server, _sandbox) = fixture();
        let s = service(&server);
        let revision = s.shared.current().config.airs.management_revision.clone();
        let bound = bound.map(|b| if b == other { other } else { revision.as_str() });
        let now = airs_runtime_core::time::now_ms();
        let run = uuid::Uuid::new_v4().to_string();
        airs_history_store::run_summary::seed_bound_run_for_tests(
            &s.storage.history_root,
            &cloud_of(&s),
            &run,
            (now - 86_400_000, now),
            state,
            &reads,
            bound,
        )
        .unwrap();
        assert_eq!(
            blocked(&s),
            expected,
            "case {n}: {state} {reads:?} {bound:?}"
        );
        assert_eq!(
            recorded(&s),
            expected,
            "case {n}: the record follows the journal"
        );
    }
}

/// M4 review 5 finding 1, the live path: a run journals a 403 and a cancellation
/// commits before the run ends (observed at the next send). The job ends canceled; the
/// run's denial is
/// recorded before that (never lost to the cancellation), the run is bound to its
/// revision in the archive itself, and the next job of either executor makes no
/// attempt. Both executors.
#[test]
fn a_cancellation_during_a_denied_run_keeps_the_revision_blocked() {
    use airs_test_harness::{Planned, Rx};
    for enrich in [false, true] {
        let context = format!("enrichment={enrich}");
        let (server, _sandbox) = fixture();
        // A 403 the run journals while a concurrent peer read, admitted before it, is
        // still in flight; the cancellation commits during that read. (Since round 8 a
        // journaled 403 fences every later send of the run, so the in-flight peer is the
        // last transport.)
        server.set_default(
            Route::Profiles,
            Planned::status(403).with_delay(std::time::Duration::from_millis(50)),
        );
        server.set_default(
            Route::Runtime(Rx::CustomerApps),
            Planned::builtin().with_delay(std::time::Duration::from_millis(500)),
        );
        let s = service(&server);
        let revision = s.shared.current().config.airs.management_revision.clone();
        let id = if enrich {
            submit_enrichment(&s, "key-cancel-denied-01")
        } else {
            submit(&s, "key-cancel-denied-01")
        };
        let previous =
            airs_mcp::collection::WATCHER_POLL_MS.swap(60_000, std::sync::atomic::Ordering::SeqCst);
        let w = if enrich {
            enrichment_worker(&s)
        } else {
            worker(&s)
        };
        // The cancellation commits once the slow read is in flight.
        let state = std::thread::scope(|scope| {
            scope.spawn(|| {
                let started = std::time::Instant::now();
                while server
                    .requests_for(Route::Runtime(Rx::CustomerApps))
                    .is_empty()
                    || server.requests_for(Route::Profiles).is_empty()
                {
                    assert!(started.elapsed().as_secs() < 30, "the read was never sent");
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                s.storage.control(|c| {
                    c.cancel(&s.tenant, &id, airs_runtime_core::time::now_ms())
                        .unwrap()
                });
            });
            w.step(&s.tenant, |_| true).unwrap().unwrap().1
        });
        airs_mcp::collection::WATCHER_POLL_MS.store(previous, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(state, State::Canceled, "{context}");
        assert!(
            recorded(&s),
            "the denial outlives the cancellation ({context})"
        );
        let bound: Vec<String> = rusqlite::Connection::open_with_flags(
            archive_path(&s),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap()
        .prepare(
            "SELECT json_extract(provenance_json,'$.credential_revision') FROM collection_runs",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
        assert_eq!(
            bound,
            vec![revision],
            "the archive binds the run ({context})"
        );
        let sent = server.requests().len();
        next_job_makes_no_attempt(&s, &server, "key-cancel-denied-02", &context);
        assert_eq!(server.requests().len(), sent, "{context}");
        assert_eq!(job(&s, &id).state, State::Canceled, "{context}");
    }
}

/// M4 round 6 audit, the reload admission point: a rotation to a revision whose denial
/// only the archive journal holds resumes no `needs_credentials` job (and records the
/// denial); a rotation to an undenied revision resumes it with its deadline.
#[test]
fn a_rotation_to_a_journal_denied_revision_resumes_nothing() {
    let (server, _sandbox) = fixture();
    let s = service(&server);
    let (id, _) = crashed_denial(&s, false, "key-rotation-denied-01", false);
    let (_, state) = worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(state, State::NeedsCredentials);
    let deadline = job(&s, &id).deadline_ms;
    let denied_elsewhere = "00000000-0000-4000-8000-0000000000bb";
    let now = airs_runtime_core::time::now_ms();
    airs_history_store::run_summary::seed_bound_run_for_tests(
        &s.storage.history_root,
        &cloud_of(&s),
        &uuid::Uuid::new_v4().to_string(),
        (now - 86_400_000, now),
        "partial",
        &[("airs.management.profiles", "auth_denied", 403)],
        Some(denied_elsewhere),
    )
    .unwrap();
    let rotate = |revision: &str| {
        airs_mcp::collection::credentials_rotated(
            &s.storage,
            &s.tenant,
            &cloud_of(&s),
            revision,
            airs_runtime_core::time::now_ms(),
        )
        .unwrap()
    };
    assert_eq!(rotate(denied_elsewhere), 0);
    assert_eq!(job(&s, &id).state, State::NeedsCredentials);
    assert!(
        s.storage
            .control(|c| c.denied(&s.tenant, "management", denied_elsewhere).unwrap())
    );
    assert_eq!(rotate("00000000-0000-4000-8000-0000000000cc"), 1);
    let j = job(&s, &id);
    assert_eq!((j.state, j.deadline_ms), (State::Queued, deadline));
    assert_eq!(server.requests().len(), 0);
}

/// End a crashed job without its executor reconciling the run (the reviewer's terminal
/// paths): `cancel` (scheduler-pass cancellation), `expire_at_claim`, `expire_in_pass`.
fn end_unreconciled(s: &Service, w: &Worker, id: &str, how: &str, context: &str) -> State {
    let now = airs_runtime_core::time::now_ms();
    match how {
        "cancel" => {
            s.storage.control(|c| {
                c.cancel(&s.tenant, id, now).unwrap();
                c.scheduler_pass(&s.tenant, now).unwrap();
            });
            assert_eq!(job(s, id).state, State::Canceled, "{context}");
        }
        "expire_at_claim" => {
            control_db(s)
                .execute(
                    "UPDATE jobs SET deadline_ms=?1 WHERE job_id=?2",
                    rusqlite::params![now - 1, id],
                )
                .unwrap();
            assert!(w.step(&s.tenant, |_| true).unwrap().is_none(), "{context}");
            assert_eq!(job(s, id).state, State::Failed, "{context}");
        }
        _ => {
            control_db(s)
                .execute(
                    "UPDATE jobs SET state='waiting_retry', retry_after_ms=?1, deadline_ms=?1 WHERE job_id=?2",
                    rusqlite::params![now - 1, id],
                )
                .unwrap();
            s.storage
                .control(|c| c.scheduler_pass(&s.tenant, now).unwrap());
            assert_eq!(job(s, id).state, State::Failed, "{context}");
        }
    }
    job(s, id).state
}

fn worker_for(s: &Service, enrich: bool) -> Worker {
    if enrich {
        enrichment_worker(s)
    } else {
        worker(s)
    }
}

/// M4 review 6 finding 1 (the reviewer's probe): a run the dead process left `running`
/// holds a token-endpoint refusal (an OAuth 401 or 403) of the configured revision; its
/// job is canceled or expires (at its claim, or in the scheduler pass) before any
/// executor reconciles the run. The journal blocks the revision at once (no executor,
/// no pass), the tick pass records it, the terminal job keeps its state, and the next
/// job of either executor makes no attempt. Both executors, both kinds of binding.
#[test]
fn a_running_token_refusal_blocks_the_revision_after_cancel_or_expiry() {
    for status in [401, 403] {
        for enrich in [false, true] {
            for archive_bound in [false, true] {
                for how in ["cancel", "expire_at_claim", "expire_in_pass"] {
                    let context = format!(
                        "oauth {status} enrichment={enrich} archive_bound={archive_bound} {how}"
                    );
                    let (server, _sandbox) = fixture();
                    let s = service(&server);
                    let (id, _) = crashed_with(
                        &s,
                        enrich,
                        "key-token-refusal-01",
                        archive_bound,
                        &[("oauth.token", "auth_denied", status)],
                    );
                    let w = worker_for(&s, enrich);
                    let terminal = end_unreconciled(&s, &w, &id, how, &context);
                    assert!(!recorded(&s), "no executor reconciled the run ({context})");
                    assert!(blocked(&s), "the journal blocks the revision ({context})");
                    airs_mcp::run::tick_passes_for_tests(&w, &s.shared);
                    assert!(recorded(&s), "the pass records the denial ({context})");
                    next_job_makes_no_attempt(&s, &server, "key-token-refusal-02", &context);
                    assert_eq!(job(&s, &id).state, terminal, "{context}");
                    assert_eq!(server.requests().len(), 0, "{context}");
                }
            }
        }
    }
}

/// A later job of either executor waits for the management hold before any send:
/// `waiting_retry`, no reservation, no request.
fn next_job_waits_for_the_hold(s: &Service, server: &FixtureServer, key: &str, context: &str) {
    for enrich in [false, true] {
        let key = format!("{key}-{}", if enrich { "e" } else { "c" });
        let next = if enrich {
            submit_enrichment(s, &key)
        } else {
            submit(s, &key)
        };
        let before = server.requests().len();
        let (stepped, state) = worker_for(s, enrich)
            .step(&s.tenant, |_| true)
            .unwrap()
            .unwrap();
        assert_eq!(stepped, next, "{context}");
        assert_eq!(
            state,
            State::WaitingRetry,
            "{context} next enrichment={enrich}"
        );
        assert_eq!(
            s.storage.control(|c| c.consumed_sends(&next).unwrap()),
            0,
            "no reservation ({context} next enrichment={enrich})"
        );
        assert_eq!(server.requests().len(), before, "no request ({context})");
        let retry = job(s, &next).retry_after_ms.unwrap();
        assert!(
            retry >= airs_runtime_core::time::now_ms() + 12 * 60_000,
            "the job waits out the service hold ({context}): {retry}"
        );
    }
}

fn control_hold(s: &Service) -> Option<i64> {
    s.storage.control(|c| {
        c.cooldown_until(&s.tenant, "management", airs_runtime_core::time::now_ms())
            .unwrap()
    })
}

/// M4 review 6 finding 2 (the reviewer's probe): an API 429 two minutes ago, whose
/// companion archive cooldown honours a `Retry-After: 1`, in a run the dead process
/// left `running`. The job is canceled, expires (at its claim or in the scheduler
/// pass), or is recovered while every control-hold write fails. The service's
/// 15-minute hold still holds: admission derives it from the journal, so the next job
/// of either executor waits with no reservation and no request while 13 minutes
/// remain; the tick pass restores the control record (once a failing write clears).
#[test]
fn the_management_hold_survives_cancel_expiry_and_a_failed_hold_write() {
    for enrich in [false, true] {
        for archive_bound in [false, true] {
            for how in [
                "cancel",
                "expire_at_claim",
                "expire_in_pass",
                "failed_hold_write",
            ] {
                let context = format!("enrichment={enrich} archive_bound={archive_bound} {how}");
                let (server, _sandbox) = fixture();
                let s = service(&server);
                let revision = s.shared.current().config.airs.management_revision.clone();
                let now = airs_runtime_core::time::now_ms();
                let at = now - 2 * 60_000;
                let run = uuid::Uuid::new_v4().to_string();
                airs_history_store::run_summary::seed_rate_limited_run_for_tests(
                    &s.storage.history_root,
                    &cloud_of(&s),
                    &run,
                    (now - 86_400_000, now),
                    "running",
                    at,
                    Some(1),
                    archive_bound.then_some(revision.as_str()),
                )
                .unwrap();
                let (id, _) =
                    crash_seeded(&s, enrich, "key-management-hold-01", archive_bound, &run);
                let w = worker_for(&s, enrich);
                if how == "failed_hold_write" {
                    control_db(&s)
                        .execute_batch(
                            "CREATE TRIGGER fail_hold_i BEFORE INSERT ON cooldowns BEGIN SELECT RAISE(ABORT, 'injected failure'); END;
                             CREATE TRIGGER fail_hold_u BEFORE UPDATE ON cooldowns BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
                        )
                        .unwrap();
                    // The executor recovers the run; its hold write fails.
                    let (stepped, _) = w.step(&s.tenant, |_| true).unwrap().unwrap();
                    assert_eq!(stepped, id, "{context}");
                } else {
                    end_unreconciled(&s, &w, &id, how, &context);
                }
                assert_eq!(
                    control_hold(&s),
                    None,
                    "no control hold was recorded ({context})"
                );
                next_job_waits_for_the_hold(&s, &server, "key-management-hold-02", &context);
                airs_mcp::run::tick_passes_for_tests(&w, &s.shared);
                if how == "failed_hold_write" {
                    assert_eq!(control_hold(&s), None, "{context}");
                    next_job_waits_for_the_hold(&s, &server, "key-management-hold-03", &context);
                    control_db(&s)
                        .execute_batch("DROP TRIGGER fail_hold_i; DROP TRIGGER fail_hold_u;")
                        .unwrap();
                    airs_mcp::run::tick_passes_for_tests(&w, &s.shared);
                }
                let until = control_hold(&s).expect("the pass restores the hold");
                assert_eq!(until, at + 15 * 60_000, "from the refusal time ({context})");
                next_job_waits_for_the_hold(&s, &server, "key-management-hold-04", &context);
                assert_eq!(server.requests().len(), 0, "{context}");
            }
        }
    }
}

/// M4 review 6 finding 2: a provider cooldown longer than the service hold (a long
/// `Retry-After`) is kept; an expired service hold and an expired cooldown hold
/// nothing.
#[test]
fn the_management_hold_keeps_a_longer_provider_cooldown() {
    let (server, _sandbox) = fixture();
    let s = service(&server);
    let hold =
        || airs_mcp::collection::management_hold(&s.storage, &s.tenant, &cloud_of(&s)).unwrap();
    assert_eq!(hold(), None);
    let now = airs_runtime_core::time::now_ms();
    let seed = |at: i64, retry: Option<u64>| {
        airs_history_store::run_summary::seed_rate_limited_run_for_tests(
            &s.storage.history_root,
            &cloud_of(&s),
            &uuid::Uuid::new_v4().to_string(),
            (now - 86_400_000, now),
            "partial",
            at,
            retry,
            None,
        )
        .unwrap()
    };
    // A 429 sixteen minutes ago with a short Retry-After: nothing holds.
    seed(now - 16 * 60_000, Some(1));
    assert_eq!(hold(), None);
    // A 429 a minute ago with Retry-After: 3600 holds for the hour.
    seed(now - 60_000, Some(3600));
    assert_eq!(hold(), Some(now - 60_000 + 3_600_000));
    assert_eq!(control_hold(&s), Some(now - 60_000 + 3_600_000));
}

fn with_revision(s: &Service, revision: &str) {
    let current = s.shared.current();
    let mut config = (*current.config).clone();
    config.airs.management_revision = revision.into();
    s.shared.swap(airs_mcp::run::Live::new(config).unwrap());
}

/// M4 review 6 finding 3 (the reviewer's probe): jobs parked `needs_credentials` under
/// one revision, then a start with a new, undenied revision (no SIGHUP edge), or a
/// resumption whose write fails across several worker passes. The startup
/// reconciliation and every later pass resume the parked jobs, with their original
/// deadlines, scope and consumption; a pass under the parking revision itself, or under
/// a denied one, resumes nothing. Both executors.
#[test]
fn parked_jobs_resume_at_startup_and_after_a_failed_resumption() {
    for enrich in [false, true] {
        for how in ["startup", "failed_write"] {
            let context = format!("enrichment={enrich} {how}");
            let (server, _sandbox) = fixture();
            let s = service(&server);
            let (id, _) = crashed_denial(&s, enrich, "key-parked-credentials-01", false);
            let w = worker_for(&s, enrich);
            let (_, state) = w.step(&s.tenant, |_| true).unwrap().unwrap();
            assert_eq!(state, State::NeedsCredentials, "{context}");
            let before = job(&s, &id);
            let checkpoint = s.storage.control(|c| c.last_checkpoint(&id).unwrap());
            let used = s.storage.control(|c| c.consumed_sends(&id).unwrap());
            // Passes under the parking (denied) revision keep it parked.
            airs_mcp::run::startup_reconcile_for_tests(&w, &s.shared);
            airs_mcp::run::tick_passes_for_tests(&w, &s.shared);
            assert_eq!(job(&s, &id).state, State::NeedsCredentials, "{context}");
            // A revision the journal denies resumes nothing either.
            let denied = "00000000-0000-4000-8000-0000000000bb";
            let now = airs_runtime_core::time::now_ms();
            airs_history_store::run_summary::seed_bound_run_for_tests(
                &s.storage.history_root,
                &cloud_of(&s),
                &uuid::Uuid::new_v4().to_string(),
                (now - 86_400_000, now),
                "partial",
                &[("airs.management.profiles", "auth_denied", 403)],
                Some(denied),
            )
            .unwrap();
            with_revision(&s, denied);
            airs_mcp::run::startup_reconcile_for_tests(&w, &s.shared);
            assert_eq!(job(&s, &id).state, State::NeedsCredentials, "{context}");
            // A new, undenied revision, configured without any reload edge.
            with_revision(&s, "00000000-0000-4000-8000-0000000000cc");
            if how == "failed_write" {
                control_db(&s)
                    .execute_batch("CREATE TRIGGER fail_resume BEFORE UPDATE OF state ON jobs WHEN NEW.state='queued' BEGIN SELECT RAISE(ABORT, 'injected failure'); END;")
                    .unwrap();
                for _ in 0..3 {
                    airs_mcp::run::tick_passes_for_tests(&w, &s.shared);
                    assert_eq!(job(&s, &id).state, State::NeedsCredentials, "{context}");
                }
                control_db(&s)
                    .execute_batch("DROP TRIGGER fail_resume;")
                    .unwrap();
                airs_mcp::run::tick_passes_for_tests(&w, &s.shared);
            } else {
                airs_mcp::run::startup_reconcile_for_tests(&w, &s.shared);
            }
            let after = job(&s, &id);
            assert_eq!(after.state, State::Queued, "{context}");
            assert_eq!(after.deadline_ms, before.deadline_ms, "{context}");
            assert_eq!(
                s.storage.control(|c| c.last_checkpoint(&id).unwrap()),
                checkpoint,
                "the scope is kept ({context})"
            );
            assert_eq!(
                s.storage.control(|c| c.consumed_sends(&id).unwrap()),
                used,
                "the consumption is kept ({context})"
            );
            // Idempotent: another pass changes nothing.
            airs_mcp::run::tick_passes_for_tests(&w, &s.shared);
            assert_eq!(job(&s, &id).state, State::Queued, "{context}");
            assert_eq!(server.requests().len(), 0, "{context}");
        }
    }
}

/// M4 review 6 finding 1, the executor path: recovery of a run the dead process left
/// `running` whose token endpoint refused the credentials after the run had already
/// read API data. Both executors park `needs_credentials` with no further attempt and
/// record the denial.
#[test]
fn a_running_token_refusal_after_api_data_parks_both_executors() {
    for enrich in [false, true] {
        let (out, denied, _, used, archived) = recover_seeded(
            "running",
            &[
                (
                    "airs.management.dashboard.sessions_overview",
                    "returned",
                    200,
                ),
                ("oauth.token", "auth_denied", 401),
            ],
            enrich,
        );
        assert_eq!(out, State::NeedsCredentials, "enrichment={enrich}");
        assert!(denied, "enrichment={enrich}");
        assert_eq!(used, 0, "enrichment={enrich}");
        assert_eq!(archived, "interrupted", "enrichment={enrich}");
    }
}

/// M4 review 6 finding 3, idempotence: a job parked under the configured revision for
/// a reason the journal does not record as a denial (its secret file is unreadable) is
/// not requeued by the passes (no churn while nothing changed); a new revision resumes
/// it. Both executors.
#[test]
fn a_job_parked_under_the_configured_revision_stays_parked_until_a_rotation() {
    for enrich in [false, true] {
        let context = format!("enrichment={enrich}");
        let (server, _sandbox) = fixture();
        let s = service(&server);
        let secret = s
            .shared
            .current()
            .config
            .airs
            .management_client_secret_file
            .clone();
        std::fs::remove_file(&secret).unwrap();
        let id = if enrich {
            submit_enrichment(&s, "key-parked-unreadable-01")
        } else {
            submit(&s, "key-parked-unreadable-01")
        };
        let w = worker_for(&s, enrich);
        let (_, state) = w.step(&s.tenant, |_| true).unwrap().unwrap();
        assert_eq!(state, State::NeedsCredentials, "{context}");
        assert!(!blocked(&s), "no denial is journaled ({context})");
        let transitions = || -> i64 {
            control_db(&s)
                .query_row(
                    "SELECT count(*) FROM job_events WHERE job_id=?1",
                    [&id],
                    |r| r.get(0),
                )
                .unwrap()
        };
        let before = transitions();
        for _ in 0..3 {
            airs_mcp::run::tick_passes_for_tests(&w, &s.shared);
        }
        assert_eq!(job(&s, &id).state, State::NeedsCredentials, "{context}");
        assert_eq!(transitions(), before, "no requeue churn ({context})");
        with_revision(&s, "00000000-0000-4000-8000-0000000000dd");
        airs_mcp::run::tick_passes_for_tests(&w, &s.shared);
        assert_eq!(job(&s, &id).state, State::Queued, "{context}");
        assert_eq!(server.requests().len(), 0, "{context}");
    }
}

/// The archive's management cooldown end (the per-transport admission check reads it).
fn archive_cooldown(s: &Service) -> Option<i64> {
    let conn = rusqlite::Connection::open(archive_path(s)).unwrap();
    conn.query_row(
        "SELECT until_ms FROM service_cooldowns WHERE service='management'",
        [],
        |r| r.get(0),
    )
    .ok()
}

/// M4 review 7 finding 1 (the reviewer's active-run probe): the profiles walk is
/// refused with a 429 carrying `Retry-After: 1` after 100 ms, while the run's peer
/// configuration walks were already admitted and answer only after 1.5 s; the collector
/// then reaches sessions discovery. No transport is admitted after the 429 inside the
/// service's 15-minute hold, in either executor, whatever the provider's duration: the
/// 429 commits the service minimum as the archive cooldown in its own transaction, and
/// every later send's journal admission refuses. A longer provider duration is kept.
#[test]
fn an_active_run_admits_no_send_during_the_management_hold_both_executors() {
    use airs_test_harness::{Planned, Rx};
    use std::time::Duration;
    for retry in [1u64, 900, 3600] {
        for enrich in [false, true] {
            let context = format!("Retry-After: {retry} enrichment={enrich}");
            let (server, _sandbox) = fixture();
            if enrich {
                seed_examples(&server);
            }
            server.set_default(
                Route::Profiles,
                Planned::status(429)
                    .with_retry_after(retry)
                    .with_delay(Duration::from_millis(100)),
            );
            for peer in [Rx::CustomerApps, Rx::AppsOverview] {
                server.set_default(
                    Route::Runtime(peer),
                    Planned::builtin().with_delay(Duration::from_millis(1500)),
                );
            }
            let s = service(&server);
            if enrich {
                submit_enrichment(&s, "key-active-hold-00001");
            } else {
                submit(&s, "key-active-hold-00001");
            }
            let before = airs_runtime_core::time::now_ms();
            let (_, state) = worker_for(&s, enrich)
                .step(&s.tenant, |_| true)
                .unwrap()
                .unwrap();
            assert_ne!(state, State::Running, "{context}");
            let limited = server.requests_for(Route::Profiles).len();
            assert_eq!(limited, 1, "one profiles read, never retried ({context})");
            assert_eq!(
                server.requests_for(Route::Runtime(Rx::CustomerApps)).len(),
                1,
                "the peer read was admitted before the 429 and answered after it ({context})"
            );
            assert_eq!(
                server
                    .requests_for(Route::Runtime(Rx::SessionsOverview))
                    .len(),
                0,
                "no later management transport inside the hold ({context})"
            );
            let until = archive_cooldown(&s).expect("the 429 committed a cooldown");
            let floor = before + (retry.max(900) as i64) * 1000;
            assert!(
                until >= floor - 5_000 && until <= floor + 30_000,
                "the archive cooldown is the service minimum or the longer provider \
                 duration ({context}): {until} vs {floor}"
            );
            // The next job of either executor waits too, with no request.
            let sent = server.requests().len();
            next_job_waits_for_the_hold(&s, &server, "key-active-hold-00002", &context);
            assert_eq!(server.requests().len(), sent, "{context}");
        }
    }
}

/// M4 review 7 finding 2 (the reviewer's active-run probe): the token endpoint refuses
/// the credentials (401 or 403). Once that refusal is durable in the run's journal, no
/// further transport of the run is admitted: exactly one token request and no API
/// read, in either executor, and the job parks `needs_credentials` with the revision
/// blocked.
#[test]
fn an_active_run_sends_nothing_after_a_definitive_token_refusal_both_executors() {
    use airs_test_harness::Planned;
    for status in [401u16, 403] {
        for enrich in [false, true] {
            let context = format!("oauth {status} enrichment={enrich}");
            let (server, _sandbox) = fixture();
            if enrich {
                seed_examples(&server);
            }
            server.set_default(Route::Token, Planned::status(status));
            let s = service(&server);
            let id = if enrich {
                submit_enrichment(&s, "key-active-token-0001")
            } else {
                submit(&s, "key-active-token-0001")
            };
            let (_, state) = worker_for(&s, enrich)
                .step(&s.tenant, |_| true)
                .unwrap()
                .unwrap();
            assert_eq!(state, State::NeedsCredentials, "{context}");
            assert_eq!(job(&s, &id).state, State::NeedsCredentials, "{context}");
            assert_eq!(
                server.requests_for(Route::Token).len(),
                1,
                "one token request, never another after the refusal ({context})"
            );
            assert_eq!(
                server.requests().len(),
                1,
                "no API transport after the refusal ({context})"
            );
            assert!(blocked(&s), "{context}");
        }
    }
}

/// The fence keeps the one legitimate replay: an API 401 is followed by exactly one
/// accounted fresh token and one replay, in either executor, and the run completes; a
/// refused replay (two 401s of the same read) is definitive and fences the rest of the
/// run.
#[test]
fn the_fence_keeps_the_single_fresh_token_replay_both_executors() {
    use airs_test_harness::{Planned, Rx};
    let overview = Route::Runtime(Rx::SessionsOverview);
    for enrich in [false, true] {
        let (server, _sandbox) = fixture();
        if enrich {
            seed_examples(&server);
        }
        server.push_once(overview, Planned::status(401));
        let s = service(&server);
        if enrich {
            submit_enrichment(&s, "key-fence-replay-0001");
        } else {
            submit(&s, "key-fence-replay-0001");
        }
        let (_, state) = worker_for(&s, enrich)
            .step(&s.tenant, |_| true)
            .unwrap()
            .unwrap();
        assert!(
            matches!(state, State::Succeeded | State::Partial),
            "enrichment={enrich}: {state:?}"
        );
        assert_eq!(
            server.requests_for(overview).len(),
            2,
            "enrichment={enrich}"
        );
        assert_eq!(
            server.requests_for(Route::Token).len(),
            2,
            "enrichment={enrich}"
        );
        assert!(!blocked(&s), "enrichment={enrich}");
        // Both 401s: the replay is refused too; nothing after it is admitted.
        let (server, _sandbox) = fixture();
        if enrich {
            seed_examples(&server);
        }
        server.set_default(overview, Planned::status(401));
        let s = service(&server);
        if enrich {
            submit_enrichment(&s, "key-fence-replay-0002");
        } else {
            submit(&s, "key-fence-replay-0002");
        }
        let (_, state) = worker_for(&s, enrich)
            .step(&s.tenant, |_| true)
            .unwrap()
            .unwrap();
        assert_eq!(state, State::NeedsCredentials, "enrichment={enrich}");
        assert_eq!(
            server.requests_for(overview).len(),
            2,
            "enrichment={enrich}"
        );
        let after: Vec<_> = server
            .requests()
            .into_iter()
            .filter(|r| r.path.contains("/sessions/session") && !r.path.ends_with("overview"))
            .collect();
        assert!(
            after.is_empty(),
            "no session read after the refused replay (enrichment={enrich})"
        );
        assert!(blocked(&s), "enrichment={enrich}");
    }
}

/// Collection sessions for the replay-identity probes: `pan_session` holds two
/// transaction reads that differ only by `scan_id` (`"scan"`) or only by
/// `scan_sub_req_id` (`"sub"`); the other session has none.
fn two_transactions(server: &FixtureServer, variant: &str) {
    use airs_test_harness::fixtures;
    let mut session = fixtures::runtime::session("pan_session");
    let mut second = session["session_actions"][0].clone();
    match variant {
        "scan" => second["scan_id"] = json!("scan-2"),
        _ => second["scan_sub_req_id"] = json!(1),
    }
    session["session_actions"] = json!([session["session_actions"][0].clone(), second]);
    session["pagination"]["total_items"] = json!(2);
    let mut empty = fixtures::runtime::session("second");
    empty["session_actions"] = json!([]);
    empty["pagination"]["total_items"] = json!(0);
    server.runtime(|d| {
        let key = |s: &str| ("uuid-shared".to_owned(), "A & B".to_owned(), s.to_owned());
        d.session_records.insert(key("pan_session"), session);
        d.session_records.insert(key("second"), empty);
    });
}

/// M4 review 8 finding 1 (the reviewer's probe): two distinct transaction reads (two
/// scan IDs, or one scan's two sub-requests) each get a first 401 that their own
/// replay would answer. Each read keeps its single fresh-token replay (its own replay
/// chain): four transaction sends, the job is not parked and the revision stays
/// usable, in either executor. Each read refused twice (by its own replay) still
/// blocks the revision.
#[test]
fn distinct_transaction_reads_each_keep_their_own_fresh_token_replay_both_executors() {
    use airs_test_harness::{Planned, Rx};
    use std::time::Duration;
    let tx = Route::Runtime(Rx::SessionTransaction);
    for (enrich, variant, baseline) in [(false, "scan", 2), (false, "sub", 2), (true, "scan", 5)] {
        for refused_replay in [false, true] {
            let context = format!("enrichment={enrich} {variant} refused_replay={refused_replay}");
            let (server, _sandbox) = fixture();
            if enrich {
                seed_examples(&server);
            } else {
                two_transactions(&server, variant);
            }
            // Both first sends are in flight before either refusal answers, and the
            // fresh tokens take longer still: the reviewer's staggered timing.
            server.push_once(
                tx,
                Planned::status(401).with_delay(Duration::from_millis(10)),
            );
            server.push_once(
                tx,
                Planned::status(401).with_delay(Duration::from_millis(20)),
            );
            if refused_replay {
                // Every later transaction send is refused too: each read's own replay.
                server.set_default(tx, Planned::status(401));
            }
            server.set_default(
                Route::Token,
                Planned::builtin().with_delay(Duration::from_millis(40)),
            );
            let s = service(&server);
            let id = if enrich {
                submit_enrichment(&s, "key-replay-chain-0001")
            } else {
                submit(&s, "key-replay-chain-0001")
            };
            let (_, state) = worker_for(&s, enrich)
                .step(&s.tenant, |_| true)
                .unwrap()
                .unwrap();
            let sent = server.requests_for(tx).len();
            if refused_replay {
                assert_eq!(state, State::NeedsCredentials, "{context}");
                assert!(blocked(&s), "{context}");
                assert!(
                    sent <= 2 * baseline,
                    "at most one replay per read ({context}): {sent}"
                );
                continue;
            }
            assert_ne!(state, State::NeedsCredentials, "{context}");
            assert_eq!(job(&s, &id).state, state, "{context}");
            assert_eq!(
                sent,
                baseline + 2,
                "each refused read sent once more, as its own replay ({context})"
            );
            assert!(
                server.requests_for(Route::Token).len() <= 3,
                "at most one fresh token per refused read ({context})"
            );
            assert!(!blocked(&s), "the revision stays usable ({context})");
            assert!(!recorded(&s), "{context}");
        }
    }
}

/// The journal authority classifies by replay chain: two 401s are a refused replay only
/// within one chain (a read and its own replay); distinct reads never answer, or
/// refuse, each other.
#[test]
fn the_journal_authority_classifies_refusals_by_replay_chain() {
    let src = "airs.management.dashboard.session_transaction";
    let c = |n: u8| format!(r#"{{"operation":"{src}","query":[["replay_chain","{n}"]]}}"#);
    let (one, two) = (c(1), c(2));
    type Case<'a> = (&'a str, Vec<(&'a str, &'a str, i64, &'a str)>, bool);
    let cases: Vec<Case> = vec![
        // Two distinct reads, each awaiting its own replay: not definitive.
        (
            "running",
            vec![
                (src, "auth_denied", 401, &one),
                (src, "auth_denied", 401, &two),
            ],
            false,
        ),
        // One read and its refused replay: definitive at once.
        (
            "running",
            vec![
                (src, "auth_denied", 401, &one),
                (src, "auth_denied", 401, &one),
            ],
            true,
        ),
        // Each read answered by its own replay.
        (
            "partial",
            vec![
                (src, "auth_denied", 401, &one),
                (src, "returned", 200, &one),
                (src, "auth_denied", 401, &two),
                (src, "returned", 200, &two),
            ],
            false,
        ),
        // Another read's success never answers a refusal it did not replay.
        (
            "partial",
            vec![
                (src, "auth_denied", 401, &one),
                (src, "returned", 200, &two),
            ],
            true,
        ),
    ];
    for (n, (state, reads, expected)) in cases.into_iter().enumerate() {
        let (server, _sandbox) = fixture();
        let s = service(&server);
        let revision = s.shared.current().config.airs.management_revision.clone();
        let now = airs_runtime_core::time::now_ms();
        let run = uuid::Uuid::new_v4().to_string();
        airs_history_store::run_summary::seed_bound_run_with_descriptors_for_tests(
            &s.storage.history_root,
            &cloud_of(&s),
            &run,
            (now - 86_400_000, now),
            state,
            &reads,
            Some(&revision),
        )
        .unwrap();
        assert_eq!(blocked(&s), expected, "case {n}: {state} {reads:?}");
    }
}

/// M4 review 8 finding 2 (the reviewer's probe): a scanner 401 (Results or Reports,
/// authenticated by a static key) with the service's fresh-token replay enabled is
/// final: exactly one transport, no OAuth renewal, and the durable record, the job
/// result and the control reservations count that one transport. Both executors.
#[test]
fn a_scanner_401_is_one_accounted_transport_both_executors() {
    use airs_test_harness::Planned;
    for (enrich, route) in [
        (false, Route::ScanResults),
        (true, Route::ScanResults),
        (true, Route::ScanReports),
    ] {
        let context = format!("enrichment={enrich} {route:?}");
        let (server, _sandbox) = fixture();
        seed_examples(&server);
        server.push_once(route, Planned::status(401));
        let s = service_with(&server, "uuid-shared");
        let id = if enrich {
            submit_enrichment(&s, "key-scanner-401-0001")
        } else {
            submit(&s, "key-scanner-401-0001")
        };
        let (_, state) = worker_for(&s, enrich)
            .step(&s.tenant, |_| true)
            .unwrap()
            .unwrap();
        assert_ne!(state, State::Running, "{context}");
        assert_eq!(
            server.requests_for(route).len(),
            1,
            "a scanner refusal is never replayed ({context})"
        );
        assert_eq!(
            server.requests_for(Route::Token).len(),
            1,
            "no OAuth renewal for a scanner refusal ({context})"
        );
        let result = job(&s, &id).result.unwrap();
        let run = s
            .storage
            .control(|c| c.last_checkpoint(&id).unwrap().unwrap())
            .2
            .expect("the job's checkpoint names its archive run");
        {
            let summary = airs_history_store::run_summary::run_summary(
                &s.storage.history_root,
                &cloud_of(&s),
                &run,
                &scm::Session::new(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                summary.results_sends,
                server.requests_for(Route::ScanResults).len() as u64,
                "durable Results accounting is the actual transports ({context})"
            );
            assert_eq!(
                summary.reports_sends,
                server.requests_for(Route::ScanReports).len() as u64,
                "durable Reports accounting is the actual transports ({context})"
            );
        }
        if enrich {
            let reports = server.requests_for(Route::ScanReports).len() as u64;
            assert_eq!(result["sends"], reports, "{context}: {result}");
            let now = airs_runtime_core::time::now_ms();
            let spent = s
                .storage
                .control(|c| c.reports_spent(&s.tenant, now).unwrap());
            assert_eq!(
                spent.0, reports,
                "one reservation per Reports transport ({context})"
            );
        } else {
            assert_eq!(
                result["sends"]["results"],
                server.requests_for(Route::ScanResults).len() as u64,
                "{context}: {result}"
            );
        }
        assert!(
            !blocked(&s),
            "a scanner refusal never blocks the management revision ({context})"
        );
    }
}

/// Export an archive the service wrote and import it into a fresh root (M5 review 1
/// finding 2): every row the service's own collection, enrichment and receipt paths
/// write must pass the transfer's strict schemas, round-trip, and re-export identically.
fn round_trip(s: &Service) {
    let cloud = cloud_of(s);
    let dir = private_dir("round-trip");
    let exported = dir.0.join("export.jsonl");
    let session = scm::Session::new();
    let secrets = airs_runtime_core::error::SecretSet::default();
    let budget = std::time::Duration::from_secs(60);
    airs_history_store::transfer::export_file(
        &s.storage.history_root,
        &cloud,
        &exported,
        budget,
        &session,
        &secrets,
    )
    .expect("a service-written archive exports");
    airs_history_store::transfer::validate_file(&cloud, &exported, budget, &session, &secrets)
        .expect("its export validates");
    let destination = dir.0.join("destination");
    let imported = airs_history_store::transfer::import_file(
        &destination,
        &cloud,
        &exported,
        budget,
        &session,
        &secrets,
    )
    .expect("its export imports");
    assert_eq!(imported["outcome"], "committed");
    let again = dir.0.join("again.jsonl");
    airs_history_store::transfer::export_file(
        &destination,
        &cloud,
        &again,
        budget,
        &session,
        &secrets,
    )
    .unwrap();
    let rows = |p: &std::path::Path| -> Vec<String> {
        std::fs::read_to_string(p)
            .unwrap()
            .lines()
            .filter(|l| l.contains("\"kind\":\"row\""))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(rows(&exported), rows(&again), "the round trip is exact");
    // A repeated import changes nothing.
    let repeated = airs_history_store::transfer::import_file(
        &destination,
        &cloud,
        &exported,
        budget,
        &session,
        &secrets,
    )
    .unwrap();
    assert_eq!(repeated["inserted"], 0);
}

#[test]
fn service_collection_and_receipt_archives_round_trip_through_transfer() {
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    submit(&s, "key-roundtrip-0000001");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(ingest_receipt(&s, "receipt-roundtrip-01"), (1, 0));
    assert_eq!(ingest_receipt(&s, "receipt-roundtrip-02"), (1, 0));
    // The service's own provenance is in the export (credential binding, fact counts,
    // the receipt run's first-acceptance record).
    let db = rusqlite::Connection::open_with_flags(
        archive_path(&s),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let (bound, counted, recorded): (i64, i64, i64) = db
        .query_row(
            "SELECT (SELECT count(*) FROM collection_runs WHERE json_extract(provenance_json,'$.credential_revision') IS NOT NULL),
                    (SELECT count(*) FROM collection_runs WHERE json_type(provenance_json,'$.fact_counts')='object'),
                    (SELECT count(*) FROM collection_runs WHERE json_type(provenance_json,'$.first_accepted')='array')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert!(
        bound > 0 && counted > 0 && recorded > 0,
        "{bound} {counted} {recorded}"
    );
    drop(db);
    round_trip(&s);
}

#[test]
fn service_enrichment_archives_round_trip_through_transfer() {
    let (server, _sandbox) = fixture();
    seed_examples(&server);
    let s = service_with(&server, "uuid-shared");
    submit_enrichment(&s, "key-roundtrip-enrich1");
    enrichment_worker(&s)
        .step(&s.tenant, |_| true)
        .unwrap()
        .unwrap();
    round_trip(&s);
}

// ---- M6 reports ----

fn reporter(s: &Service) -> (Worker, airs_mcp::reports::Bundles) {
    let dir = storage::private_child(&s.storage.admitted, "reports").unwrap();
    let bundles = airs_mcp::reports::Bundles { root: dir.path };
    let mut executors: std::collections::BTreeMap<String, Arc<dyn Executor>> = Default::default();
    let r: Arc<dyn Executor> = Arc::new(airs_mcp::reports::Reporter {
        shared: s.shared.clone(),
        bundles: bundles.clone(),
    });
    for op in ["reports_create", "daily_report", "catch_up", "revision"] {
        executors.insert(op.into(), r.clone());
    }
    (
        Worker {
            storage: s.storage.clone(),
            owner: "owner-test".into(),
            executors,
        },
        bundles,
    )
}

/// Runs queued jobs until `id` leaves the queue, and returns its terminal state. A tick
/// may queue several due jobs with one submission time, which the worker orders by ID.
fn run_until_done(w: &Worker, s: &Service, id: &str) -> State {
    for _ in 0..8 {
        if job(s, id).state != State::Queued {
            break;
        }
        w.step(&s.tenant, |_| true).unwrap().unwrap();
    }
    job(s, id).state
}

/// The last whole hour and the 24 hours before it.
fn day_period() -> (i64, i64) {
    let until = (airs_runtime_core::time::now_ms() / 3_600_000 + 1) * 3_600_000;
    (until - 86_400_000, until)
}

fn submit_report(
    s: &Service,
    key: &str,
    report_id: &str,
    kind: &str,
    period: (i64, i64),
) -> String {
    let params = json!({"report_id": report_id, "kind": kind, "since_ms": period.0, "until_ms": period.1, "app_ids": [], "title": "test"});
    s.storage.control(|c| {
        c.submit(
            Submission {
                tenant: &s.tenant,
                operation: if kind == "revision" {
                    "revision"
                } else {
                    "reports_create"
                },
                actor: Actor::Principal {
                    sub: "PLACEHOLDER_CALLER_SUB".into(),
                    client_id: "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID".into(),
                },
                idempotency_key: key,
                expected_epoch: "0a1b2c3d",
                params: &params,
                config_revision: "example-1",
                credential_revision: None,
                deadline_ms: airs_runtime_core::time::now_ms() + 3_600_000,
                queue_tenant: 64,
                queue_principal: 8,
                join_equivalent: false,
                scheduled_slot: None,
            },
            airs_runtime_core::time::now_ms(),
        )
        .unwrap()
        .job_id
    })
}

/// M6.1/M6.3: a report renders from the archive alone (zero AIRS requests) into an
/// immutable private bundle whose manifest binds every file's size and SHA-256; the
/// same evidence again makes no new version; changed evidence makes a new immutable
/// version linked to its predecessor and moves the latest pointer.
#[test]
fn reports_render_offline_into_immutable_versions_and_deduplicate() {
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    submit(&s, "key-report-collect-01");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let before = server.requests().len();
    let (w, bundles) = reporter(&s);
    let report = airs_mcp::reports::new_id("r1");
    let period = day_period();
    let first = submit_report(&s, "key-report-00000001", &report, "manual", period);
    let (_, state) = w.step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(state, State::Succeeded, "{:?}", job(&s, &first).result);
    assert_eq!(
        server.requests().len(),
        before,
        "rendering makes no AIRS request"
    );
    let result = job(&s, &first).result.unwrap();
    let v1 = result["version_id"].as_str().unwrap().to_owned();
    let row = s
        .storage
        .control(|c| c.version(&s.tenant, &v1).unwrap().unwrap());
    assert_eq!(row.manifest["version"], 0);
    assert_eq!(row.manifest["predecessor"], Value::Null);
    for name in airs_mcp::reports::FILES {
        let bytes = bundles
            .read(&row, name)
            .expect("published files match their manifest");
        assert!(!bytes.is_empty(), "{name}");
        let mode = std::fs::metadata(bundles.dir(&report, &v1).join(name))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "{name}");
    }
    assert!(
        row.manifest.get("x_fence").is_none(),
        "the public manifest is the frozen shape"
    );
    let page = String::from_utf8(bundles.read(&row, "report.html").unwrap()).unwrap();
    assert!(
        page.contains("id=\"report-data\"") && page.contains("script-src &#39;sha256-"),
        "the published page is the historical page under its hashed CSP"
    );
    // Same evidence: no new bundle.
    submit_report(&s, "key-report-00000002", &report, "manual", period);
    w.step(&s.tenant, |_| true).unwrap().unwrap();
    let latest = s.storage.control(|c| {
        c.report(&s.tenant, &report)
            .unwrap()
            .unwrap()
            .latest_version
            .unwrap()
    });
    assert_eq!(latest, v1, "identical evidence produces no new version");
    // New evidence (a receipt promoted onto the event changes it) → a new version.
    assert_eq!(ingest_receipt(&s, "receipt-report-00001"), (1, 0));
    let third = submit_report(&s, "key-report-00000003", &report, "manual", period);
    w.step(&s.tenant, |_| true).unwrap().unwrap();
    let v2 = job(&s, &third).result.unwrap()["version_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(v2, v1, "changed evidence makes a new version");
    let row2 = s
        .storage
        .control(|c| c.version(&s.tenant, &v2).unwrap().unwrap());
    assert_eq!(row2.manifest["predecessor"], v1.as_str());
    assert_eq!(row2.manifest["version"], 1);
    assert!(
        bundles.read(&row, "review.md").is_ok(),
        "the earlier version is unchanged"
    );
}

/// C6/C7: a bundle placed without its control row (death between rename and record)
/// is reattached from its own manifest; one placed under an older fence is quarantined,
/// never reattached; a stage left by a dead renderer is discarded.
#[test]
fn report_recovery_reattaches_or_quarantines_by_fence() {
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    submit(&s, "key-report-collect-02");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let (w, bundles) = reporter(&s);
    let report = airs_mcp::reports::new_id("r1");
    let id = submit_report(&s, "key-report-00000011", &report, "manual", day_period());
    w.step(&s.tenant, |_| true).unwrap().unwrap();
    let v = job(&s, &id).result.unwrap()["version_id"]
        .as_str()
        .unwrap()
        .to_owned();
    // Simulate C6: drop the control rows, keep the bundle.
    s.storage.control(|c| {
        c.forget_version_for_tests(&v).unwrap();
    });
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(bundles.root.join(".stage-dead"))
        .unwrap();
    let (re, q, d) = s
        .storage
        .control(|c| bundles.recover(c, &s.tenant, 1).unwrap());
    assert_eq!((re, q, d), (1, 0, 1));
    assert!(
        s.storage
            .control(|c| c.version(&s.tenant, &v).unwrap())
            .is_some(),
        "reattached"
    );
    // Fenced: the same situation after a prune advanced the fence → quarantine.
    s.storage.control(|c| {
        c.forget_version_for_tests(&v).unwrap();
        c.advance_fence(&s.tenant).unwrap();
    });
    let (re, q, _) = s
        .storage
        .control(|c| bundles.recover(c, &s.tenant, 2).unwrap());
    assert_eq!((re, q), (0, 1));
    assert!(
        s.storage
            .control(|c| c.version(&s.tenant, &v).unwrap())
            .is_none(),
        "never reattached"
    );
    assert!(!bundles.dir(&report, &v).exists(), "moved to quarantine");
}

/// M6.1 `history_query`: the cohort counts every archived event of the range, and
/// non-overlapping groups (verdict) sum to it; an invalid grouping is refused.
#[test]
fn cohort_queries_count_the_whole_range() {
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    submit(&s, "key-cohort-collect-01");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let (since, until) = day_period();
    let root = s.storage.history_root.clone();
    let (cohort, rows) = airs_history_store::query::cohort(
        &root,
        &cloud_of(&s),
        (since, until),
        &[],
        &["verdict".into()],
        &scm::Session::new(),
    )
    .unwrap()
    .unwrap();
    assert!(cohort.events >= 1);
    assert_eq!(
        rows.iter().map(|r| r.events).sum::<u64>(),
        cohort.events,
        "verdict groups partition the cohort"
    );
    assert!(
        airs_history_store::query::cohort(
            &root,
            &cloud_of(&s),
            (since, until),
            &[],
            &["colour".into()],
            &scm::Session::new()
        )
        .is_err()
    );
}

/// M6.2 end to end: the daily schedule queues the previous Chicago day at midnight and
/// the report executor publishes it as a `daily` version with its scheduled time and
/// the day's half-open bounds, from the archive only.
#[test]
fn the_daily_schedule_publishes_a_daily_report() {
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    submit(&s, "key-daily-collect-001");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let before = server.requests().len();
    let now = airs_runtime_core::time::now_ms();
    let ctx = airs_mcp::schedule_store::TickContext {
        interval_minutes: 15,
        epoch: "0a1b2c3d",
        config_revision: "example-1",
        credential_revision: None,
        queue_tenant: 64,
    };
    let tick = s.storage.control(|c| {
        c.schedule_set_kind(&s.tenant, "daily_report", true, None, now - 2 * 86_400_000)
            .unwrap();
        c.daily_tick(&s.tenant, &ctx, now).unwrap()
    });
    assert!(
        !tick.reports.is_empty(),
        "today's (or a catch-up) report is queued"
    );
    let (w, bundles) = reporter(&s);
    let state = run_until_done(&w, &s, &tick.reports[0]);
    let job = job(&s, &tick.reports[0]);
    assert_eq!(state, State::Succeeded, "{:?}", job.result);
    let v = job.result.unwrap()["version_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let row = s
        .storage
        .control(|c| c.version(&s.tenant, &v).unwrap().unwrap());
    assert!(["daily", "catch_up"].contains(&row.manifest["kind"].as_str().unwrap()));
    assert!(row.manifest["scheduled_at"].is_string());
    assert_eq!(row.manifest["timezone"], "America/Chicago");
    assert!(bundles.read(&row, "report.html").is_ok());
    // M6 review 4: a scheduled revision of that report (the scheduler's own parameters,
    // the original's generated title) reaches the executor and publishes a revision.
    if row.manifest["kind"] == "daily" {
        let original = s
            .storage
            .control(|c| c.report(&s.tenant, &row.report_id).unwrap().unwrap());
        assert!(
            !original.title.is_empty(),
            "the original carries its generated title"
        );
        let scheduled = original.scheduled_ms.unwrap();
        let revisions = s.storage.control(|c| {
            c.daily_tick(&s.tenant, &ctx, scheduled + 5 * 3_600_000)
                .unwrap()
                .revisions
        });
        assert_eq!(revisions.len(), 1, "the +4 revision is queued");
        let state = run_until_done(&w, &s, &revisions[0]);
        let rev = crate::job(&s, &revisions[0]);
        assert_eq!(state, State::Succeeded, "{:?}", rev.result);
        let rv = rev.result.unwrap()["version_id"]
            .as_str()
            .unwrap()
            .to_owned();
        let revised = s
            .storage
            .control(|c| c.version(&s.tenant, &rv).unwrap().unwrap());
        assert_eq!(
            revised.report_id, row.report_id,
            "a revision of the same report"
        );
        assert_eq!(revised.manifest["kind"], "revision");
    }
    assert_eq!(
        server.requests().len(),
        before,
        "zero AIRS requests to render"
    );
}

#[test]
fn review_queued_revision_cannot_recreate_deleted_content() {
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    submit(&s, "key-review-collect-01");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let (w, bundles) = reporter(&s);
    let report = airs_mcp::reports::new_id("r1");
    let period = day_period();
    let first = submit_report(&s, "key-review-report-01", &report, "daily", period);
    w.step(&s.tenant, |_| true).unwrap().unwrap();
    let v1 = job(&s, &first).result.unwrap()["version_id"]
        .as_str()
        .unwrap()
        .to_owned();
    ingest_receipt(&s, "receipt-review-00001");
    let second = submit_report(&s, "key-review-report-02", &report, "revision", period);
    w.step(&s.tenant, |_| true).unwrap().unwrap();
    let v2 = job(&s, &second).result.unwrap()["version_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(v1, v2);
    // A revision is queued before deletion, but executed after it.
    let queued = submit_report(&s, "key-review-report-03", &report, "revision", period);
    s.storage.control(|c| {
        c.delete_versions(
            &s.tenant,
            std::slice::from_ref(&v2),
            // A real job (the tombstone references it).
            &second,
            airs_runtime_core::time::now_ms(),
        )
        .unwrap();
        bundles.purge_deleted(c, &s.tenant).unwrap();
    });
    assert!(!bundles.dir(&report, &v2).exists());
    w.step(&s.tenant, |_| true).unwrap().unwrap();
    let latest = s.storage.control(|c| {
        c.report(&s.tenant, &report)
            .unwrap()
            .unwrap()
            .latest_version
    });
    assert_eq!(
        latest,
        Some(v1),
        "queued revision must respect the report tombstone; job result: {:?}",
        job(&s, &queued).result
    );
}

/// M6 review 4 finding 1, deterministic through the real report executor: a revision job
/// in the scheduler's review-4 shape (empty title, no applications, unlike its original's
/// "test" title) runs to success against the stored report; it is never treated as a
/// conflicting report (which retried every 60 s and never rendered).
#[test]
fn a_revision_with_other_metadata_revises_the_stored_report() {
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    submit(&s, "key-rev-meta-collect1");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let (w, _bundles) = reporter(&s);
    let report = airs_mcp::reports::new_id("r1");
    let period = day_period();
    let first = submit_report(&s, "key-rev-meta-report1", &report, "daily", period);
    let (_, state) = w.step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(state, State::Succeeded, "{:?}", job(&s, &first).result);
    let params = json!({"report_id": report, "kind": "revision", "since_ms": period.0,
        "until_ms": period.1, "app_ids": [], "title": "", "scheduled_ms": period.1,
        "cutoff_ms": period.1 + 4 * 3_600_000, "final_revision": false});
    let revision = s.storage.control(|c| {
        c.submit(
            Submission {
                tenant: &s.tenant,
                operation: "revision",
                actor: Actor::Service("daily-scheduler".into()),
                idempotency_key: "key-rev-meta-revise01",
                expected_epoch: "0a1b2c3d",
                params: &params,
                config_revision: "example-1",
                credential_revision: None,
                deadline_ms: airs_runtime_core::time::now_ms() + 3_600_000,
                queue_tenant: 64,
                queue_principal: 8,
                join_equivalent: false,
                scheduled_slot: None,
            },
            airs_runtime_core::time::now_ms(),
        )
        .unwrap()
        .job_id
    });
    let (_, state) = w.step(&s.tenant, |_| true).unwrap().unwrap();
    let done = job(&s, &revision);
    assert_eq!(state, State::Succeeded, "{:?}", done.result);
    let v = done.result.unwrap()["version_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let row = s
        .storage
        .control(|c| c.version(&s.tenant, &v).unwrap().unwrap());
    // The revision ran against the stored report (identical evidence deduplicates to
    // the existing version rather than publishing another).
    assert_eq!(row.report_id, report, "a revision of the stored report");
    let stored = s
        .storage
        .control(|c| c.report(&s.tenant, &report).unwrap().unwrap());
    assert_eq!(stored.title, "test", "the original's metadata is kept");
}

/// Archive history config for `s`, as the service derives it.
fn history_cloud(s: &Service) -> airs_history_store::identity::Cloud {
    let config = s.shared.current().config.clone();
    airs_history_store::identity::Cloud::from_parts(
        &config.airs.tsg,
        [
            config.airs.management_endpoint.clone(),
            config.airs.dashboard_endpoint.clone(),
            config.airs.token_endpoint.clone(),
        ],
    )
    .unwrap()
}

/// One scan event for the report period, observed by the archive at `observed_ms`.
fn seed_observed(s: &Service, scan: u64, period: (i64, i64), observed_ms: i64) {
    use airs_runtime_core::signal::evidence::{Event, Identity};
    let event = Event {
        identity: Identity {
            app_id: "app-a".into(),
            app_name: "Application A".into(),
            session_id: "session-a".into(),
            scan_id: format!("00000000-0000-4000-8000-{scan:012x}"),
            sub_request: Some(0),
        },
        timestamp_ms: Some(period.0 + 3_600_000),
        profile: None,
        verdict: "benign".into(),
        action: "allow".into(),
        report_ids: vec![],
        detections: vec![],
        detector_hints: vec![],
        sources: vec!["scan-results".into()],
        notes: vec![],
    };
    airs_history_store::repository::seed_observed_event_for_tests(
        &s.storage.history_root,
        &history_cloud(s),
        event,
        &format!("seeded-run-{scan}"),
        observed_ms,
    )
    .unwrap();
}

/// M6 reviews 4 to 6: the scheduler's own daily report for a titled period, then its
/// +4 and final +24 revisions, run through the real report executor. Evidence is
/// seeded with controlled observation times: one fact before the period end, one at
/// +2 hours (inside the +4 cut-off), one at +22 hours (inside the final cut-off) and
/// one at +25 hours (after it). Each version counts exactly the facts its cut-off
/// admits, chains to its predecessor, keeps the original's title, and the original
/// bundle's bytes never change. Each worker step admits only the job under test.
#[test]
fn scheduled_revisions_count_evidence_by_observation_time() {
    const HOUR: i64 = 3_600_000;
    let (server, _sandbox) = fixture();
    let s = service_with(&server, "uuid-shared");
    let (w, bundles) = reporter(&s);
    let tz = airs_runtime_core::schedule::report_tz().unwrap();
    let today = jiff::Timestamp::from_millisecond(airs_runtime_core::time::now_ms())
        .unwrap()
        .to_zoned(tz.clone())
        .date();
    let bounds = airs_runtime_core::schedule::report_period_for_midnight(today, &tz).unwrap();
    let period = (bounds.start.as_millisecond(), bounds.end.as_millisecond());
    let ctx = airs_mcp::schedule_store::TickContext {
        interval_minutes: 15,
        epoch: "0a1b2c3d",
        config_revision: "example-1",
        credential_revision: None,
        queue_tenant: 64,
    };
    s.storage.control(|c| {
        c.schedule_set_kind(&s.tenant, "daily_report", true, None, period.1 - HOUR)
            .unwrap();
    });
    let tick = |at: i64| {
        s.storage
            .control(|c| c.daily_tick(&s.tenant, &ctx, at).unwrap())
    };
    let run = |id: &str| {
        // The final tick also queues the next day's daily report.
        let state = run_until_done(&w, &s, id);
        assert_eq!(state, State::Succeeded, "{:?}", job(&s, id).result);
        let v = job(&s, id).result.unwrap()["version_id"]
            .as_str()
            .unwrap()
            .to_owned();
        s.storage
            .control(|c| c.version(&s.tenant, &v).unwrap().unwrap())
    };
    let html = |row: &airs_mcp::reports::VersionRow| {
        String::from_utf8(bundles.read(row, "report.html").unwrap()).unwrap()
    };

    seed_observed(&s, 1, period, period.1 - 1);
    let t = tick(period.1 + 1);
    assert_eq!(t.reports.len(), 1, "the daily report is queued");
    let params = s.storage.control(|c| c.params(&t.reports[0]).unwrap());
    assert_eq!(params["since_ms"], period.0);
    assert_eq!(params["until_ms"], period.1);
    let title = params["title"].as_str().unwrap().to_owned();
    assert!(!title.is_empty());
    let report = params["report_id"].as_str().unwrap().to_owned();
    let first = run(&t.reports[0]);
    assert!(first.summary.starts_with("1 events:"), "{}", first.summary);
    let first_bytes = bundles.read(&first, "report.html").unwrap();

    seed_observed(&s, 2, period, period.1 + 2 * HOUR);
    let t = tick(period.1 + 5 * HOUR);
    assert_eq!(t.revisions.len(), 1, "the +4 revision is queued");
    let p4 = s.storage.control(|c| c.params(&t.revisions[0]).unwrap());
    assert_eq!(
        p4["title"],
        title.as_str(),
        "it carries the original's title"
    );
    let second = run(&t.revisions[0]);
    assert_eq!(second.report_id, report);
    assert!(
        second.summary.starts_with("2 events:"),
        "{}",
        second.summary
    );
    assert_eq!(second.manifest["kind"], "revision");
    assert_eq!(second.manifest["predecessor"], first.version_id.as_str());

    seed_observed(&s, 3, period, period.1 + 22 * HOUR);
    seed_observed(&s, 4, period, period.1 + 25 * HOUR);
    let t = tick(period.1 + 26 * HOUR);
    assert_eq!(t.revisions.len(), 1, "the final +24 revision is queued");
    assert_eq!(t.reports.len(), 1, "and the next day's daily report");
    let pf = s.storage.control(|c| c.params(&t.revisions[0]).unwrap());
    assert_eq!(pf["final_revision"], true);
    let third = run(&t.revisions[0]);
    assert!(
        third.summary.starts_with("3 events:"),
        "the +25 fact is excluded: {}",
        third.summary
    );
    assert_eq!(third.manifest["predecessor"], second.version_id.as_str());
    assert_eq!(
        third.manifest["as_of"],
        airs_runtime_core::time::iso(period.1 + 24 * HOUR)
    );
    let stored = s
        .storage
        .control(|c| c.report(&s.tenant, &report).unwrap().unwrap());
    assert_eq!(
        stored.latest_version.as_deref(),
        Some(third.version_id.as_str())
    );

    // The original version is immutable, and every version renders the stored title.
    assert_eq!(bundles.read(&first, "report.html").unwrap(), first_bytes);
    let again = s
        .storage
        .control(|c| c.version(&s.tenant, &first.version_id).unwrap().unwrap());
    assert_eq!(again.digest, first.digest);
    assert_eq!(again.manifest, first.manifest);
    for row in [&first, &second, &third] {
        assert!(html(row).contains(&title), "{}", row.version_id);
    }
}

/// A revision job in the scheduler's shape for `report` (cutoff +`hours`).
fn submit_revision(
    s: &Service,
    key: &str,
    report: &str,
    period: (i64, i64),
    title: &str,
    hours: i64,
    final_revision: bool,
) -> String {
    let params = json!({"report_id": report, "kind": "revision", "since_ms": period.0,
        "until_ms": period.1, "app_ids": [], "title": title, "scheduled_ms": period.1,
        "cutoff_ms": period.1 + hours * 3_600_000, "final_revision": final_revision});
    s.storage.control(|c| {
        c.submit(
            Submission {
                tenant: &s.tenant,
                operation: "revision",
                actor: Actor::Service("daily-scheduler".into()),
                idempotency_key: key,
                expected_epoch: "0a1b2c3d",
                params: &params,
                config_revision: "example-1",
                credential_revision: None,
                deadline_ms: airs_runtime_core::time::now_ms() + 3_600_000,
                queue_tenant: 64,
                queue_principal: 8,
                join_equivalent: false,
                scheduled_slot: None,
            },
            airs_runtime_core::time::now_ms(),
        )
        .unwrap()
        .job_id
    })
}

/// M6 review 5: a revision reuses a published version (unchanged evidence, or an
/// exhausted automatic-revision allowance) only after verifying its bundle. An intact
/// bundle succeeds; a missing or corrupted file fails `storage_unavailable` and the
/// version is never reported as succeeded.
#[test]
fn a_reused_version_is_verified_before_success() {
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    submit(&s, "key-reuse-collect-001");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let (w, bundles) = reporter(&s);
    let period = day_period();
    let state_of = |id: &str| job(&s, id);
    // Each damage is applied to the published bundle before the reuse is attempted.
    let damages = [
        "intact",
        "html-missing",
        "html-corrupted",
        "manifest-missing",
        "manifest-malformed",
        "manifest-mismatched",
        "manifest-digest",
        "fence-missing",
        "fence-changed",
        "summary-missing",
        "summary-changed",
        // Positive control: an unrelated tenant fence advance after publication leaves
        // an intact version reusable (it is checked against its own publication fence).
        "fence-advanced",
    ];
    let damage_bundle = |dir: &std::path::Path, damage: &str| {
        let html = dir.join("report.html");
        let manifest = dir.join("manifest.json");
        let edit = |key: &str, value: Value| {
            let mut m: Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
            if value.is_null() {
                m.as_object_mut().unwrap().remove(key);
            } else {
                m[key] = value;
            }
            std::fs::write(&manifest, serde_json::to_vec(&m).unwrap()).unwrap();
        };
        match damage {
            "html-missing" => std::fs::remove_file(&html).unwrap(),
            "html-corrupted" => std::fs::write(&html, b"<html>tampered</html>").unwrap(),
            "manifest-missing" => std::fs::remove_file(&manifest).unwrap(),
            "manifest-malformed" => std::fs::write(&manifest, b"{\"kind\": ").unwrap(),
            "manifest-mismatched" => edit("kind", json!("tampered")),
            "manifest-digest" => edit("x_evidence_digest", json!("0".repeat(64))),
            "fence-missing" => edit("x_fence", Value::Null),
            "fence-changed" => edit("x_fence", json!(1_000_000)),
            "summary-missing" => edit("x_summary", Value::Null),
            "summary-changed" => edit("x_summary", json!("0 events: tampered")),
            _ => {}
        }
    };
    // Both reuse paths: an unchanged-evidence dedup, and an exhausted revision allowance.
    for exhausted in [false, true] {
        for (n, damage) in damages.into_iter().enumerate() {
            let tag = format!("{}{n:02}", if exhausted { "x" } else { "d" });
            let report = airs_mcp::reports::new_id("r1");
            let first = submit_report(
                &s,
                &format!("key-reuse-report-{tag}"),
                &report,
                "manual",
                period,
            );
            w.step(&s.tenant, |_| true).unwrap().unwrap();
            let v1 = state_of(&first).result.unwrap()["version_id"]
                .as_str()
                .unwrap()
                .to_owned();
            if exhausted {
                s.storage.control(|c| {
                    c.exhaust_revisions_for_tests(&s.tenant, &report);
                });
                // Late evidence, so only the exhausted allowance leads to reuse.
                ingest_claim(
                    &s,
                    &format!("rk-reuse-late-{tag}-0001"),
                    "malicious",
                    "block",
                );
            }
            damage_bundle(&bundles.dir(&report, &v1), damage);
            if damage == "fence-advanced" {
                s.storage.control(|c| c.advance_fence(&s.tenant)).unwrap();
            }
            let rev = submit_revision(
                &s,
                &format!("key-reuse-revise-{tag}"),
                &report,
                period,
                "test",
                4,
                false,
            );
            let (_, state) = w.step(&s.tenant, |_| true).unwrap().unwrap();
            let done = state_of(&rev);
            let case = format!("exhausted={exhausted} {damage}");
            if damage == "intact" || damage == "fence-advanced" {
                assert_eq!(state, State::Succeeded, "{case}: {:?}", done.result);
                assert_eq!(done.result.unwrap()["version_id"], v1.as_str(), "{case}");
            } else {
                assert_eq!(state, State::Failed, "{case}: {:?}", done.result);
                assert_eq!(
                    done.result.unwrap()["error"]["code"],
                    "storage_unavailable",
                    "{case}"
                );
            }
        }
    }
}

/// M6 review 5 minor: a revision queued by the earlier scheduler (empty title) whose
/// evidence changed renders with the stored report's title, period and scope.
#[test]
fn a_legacy_revision_with_changed_evidence_renders_the_stored_title() {
    let (server, _sandbox) = fixture();
    seed_scan(&server);
    let s = service_with(&server, "uuid-shared");
    submit(&s, "key-legacy-collect-01");
    worker(&s).step(&s.tenant, |_| true).unwrap().unwrap();
    let (w, bundles) = reporter(&s);
    let report = airs_mcp::reports::new_id("r1");
    let period = day_period();
    let first = submit_report(&s, "key-legacy-report-001", &report, "daily", period);
    w.step(&s.tenant, |_| true).unwrap().unwrap();
    assert_eq!(job(&s, &first).state, State::Succeeded);
    ingest_claim(&s, "rk-legacy-late-00001", "malicious", "block");
    let rev = submit_revision(&s, "key-legacy-revise-01", &report, period, "", 4, false);
    let (_, state) = w.step(&s.tenant, |_| true).unwrap().unwrap();
    let done = job(&s, &rev);
    assert_eq!(state, State::Succeeded, "{:?}", done.result);
    let v = done.result.unwrap()["version_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let row = s
        .storage
        .control(|c| c.version(&s.tenant, &v).unwrap().unwrap());
    assert_eq!(
        row.manifest["kind"], "revision",
        "changed evidence renders a new version"
    );
    let html = bundles.read(&row, "report.html").unwrap();
    let text = String::from_utf8_lossy(&html);
    assert!(
        text.contains("<title>test"),
        "the stored title, not the default"
    );
    assert!(!text.contains("AIRS runtime history report"));
}
