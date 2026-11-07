use super::*;
use crate::test_support::TestDir;
use airs_runtime_core::tenant::{Selected, config::TenantConfig};
use serde_json::{Value, json};
const SCAN: &str = "00000000-0000-4000-8000-0000000000a1";
fn fixture() -> (TestDir, Receipts, scm::Session) {
    let d = TestDir::new("history-received");
    let tenant = airs_runtime_core::tenant::TenantContext {
        selected: Selected {
            name: "fixture".into(),
            tsg_id: "100".into(),
            by_flag: false,
        },
        config: TenantConfig::default(),
    };
    let root = crate::root(Some(&d.path().join("history")), None, None).unwrap();
    let cloud = Cloud::from_tenant(&tenant).unwrap();
    let r = Receipts::prepare(
        true,
        Some((root, cloud)),
        None,
        "https://example.invalid/scan",
        "bulk",
        (&SecretSet::default(), &PayloadSpans::default()),
    );
    (d, r, scm::Session::new())
}
fn queued(r: &Receipts) -> Vec<Value> {
    r.0.pending
        .lock()
        .unwrap()
        .0
        .iter()
        .map(|v| serde_json::to_value(&v.event).unwrap())
        .collect()
}
#[test]
fn result_status_aliases_pending_and_nested_empty_scan_ids_preserve_supported_outcomes() {
    let _isolation = crate::test_isolation();
    for (status, expected) in [
        ("COMPLETE", ("malicious", "allow")),
        ("completed", ("malicious", "allow")),
        ("PENDING", ("unknown", "unknown")),
        ("failed", ("error", "failed")),
    ] {
        let (d, r, _) = fixture();
        let mut response = airs_test_harness::fixtures::scan_block("Monitor");
        response["scan_id"] = json!("");
        response["action"] = json!("allow");
        let rows: Vec<ScanIdResult> = serde_json::from_value(
            json!([{"scan_id":SCAN,"req_id":0,"status":status,"result":response}]),
        )
        .unwrap();
        r.results(SCAN, &rows);
        let v = queued(&r);
        assert_eq!(
            (
                v[0]["verdict"].as_str().unwrap(),
                v[0]["action"].as_str().unwrap()
            ),
            expected
        );
        assert_eq!(v[0]["identity"]["scan_id"], SCAN);
        assert!(
            !d.path().join("history").exists(),
            "capture must not perform storage before checkpoint"
        );
    }
}
#[test]
fn report_metadata_retains_monitoring_and_failure_without_snippets_or_invented_allow() {
    let _isolation = crate::test_isolation();
    for (detections, expected) in [
        (
            json!([{"detection_service":"PI","data_type":"PROMPT","verdict":"UNSAFE","action":"ALLOW","result_detail":{"pi_snippets":["SYNTHETIC_REPORT_SECRET_SNIPPET"]}}]),
            ("malicious", "allow"),
        ),
        (
            json!([{"detection_service":"mc","verdict":"detected","action":"block"}]),
            ("malicious", "block"),
        ),
        (
            json!([{"detection_service":"topic_guardrails","verdict":"violation"}]),
            ("malicious", "unknown"),
        ),
        (
            json!([{"detection_service":"url_filtering","verdict":"error","action":"block"}]),
            ("error", "failed"),
        ),
        (
            json!([{"detection_service":"pi","verdict":"malicious","action":"allow"},{"detection_service":"dlp","verdict":"malicious","action":"block"}]),
            ("malicious", "unknown"),
        ),
        (
            json!([{"detection_service":"pi","verdict":"benign","action":"block"}]),
            ("benign", "block"),
        ),
        (
            json!([{"detection_service":"pi","action":"block"}]),
            ("unknown", "block"),
        ),
        (json!([]), ("unknown", "unknown")),
        (
            json!([{"verdict":"malicious","action":"allow"}]),
            ("malicious", "unknown"),
        ),
    ] {
        let (_, r, _) = fixture();
        let reports: Vec<ThreatScanReport>=serde_json::from_value(json!([{"report_id":"report-1","scan_id":SCAN,"req_id":0,"detection_results":detections}])).unwrap();
        r.reports(&["report-1".into()], &reports);
        let v = queued(&r);
        assert_eq!(
            (
                v[0]["verdict"].as_str().unwrap(),
                v[0]["action"].as_str().unwrap()
            ),
            expected
        );
        assert!(
            !serde_json::to_string(&v)
                .unwrap()
                .contains("SYNTHETIC_REPORT_SECRET_SNIPPET")
        );
        assert!(
            v[0]["detections"]
                .as_array()
                .unwrap()
                .iter()
                .all(|v| v["snippets"].as_array().unwrap().is_empty())
        );
    }
    let (_, r, _) = fixture();
    let mut detections = vec![
        json!({"detection_service":"prompt-injection","verdict":"benign","action":"allow"});
        1000
    ];
    detections.push(
        json!({"detection_service":"prompt-injection","verdict":"malicious","action":"block"}),
    );
    let reports: Vec<ThreatScanReport> = serde_json::from_value(
        json!([{"report_id":"report-1","scan_id":SCAN,"req_id":0,"detection_results":detections}]),
    )
    .unwrap();
    r.reports(&["report-1".into()], &reports);
    let v = queued(&r);
    assert_eq!(v[0]["verdict"], "unknown");
    assert_eq!(v[0]["action"], "unknown");
}
#[test]
fn unknown_item_receipts_do_not_coalesce_across_flushes_and_capture_is_bounded() {
    let _isolation = crate::test_isolation();
    let (d, r, session) = fixture();
    for _ in 0..2 {
        r.capture((0..3).map(|_| r.event(SCAN, None, None, None, "cli_results")));
        assert_eq!(queued(&r).len(), 3);
        r.flush();
    }
    assert_eq!(r.finish(&session, false).unwrap().state, "complete");
    let (root, cloud) = r.0.context.as_ref().unwrap();
    let read = super::super::database::Reader::open(root, cloud, &scm::Session::new())
        .unwrap()
        .unwrap();
    assert_eq!(
        read.conn
            .query_row("SELECT count(*) FROM receipts", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        6
    );
    read.finish().unwrap();
    assert!(d.path().join("history").exists());
    let (_, r, session) = fixture();
    let mut oversized = r.event(SCAN, None, None, None, "cli_results");
    oversized.event.notes = vec!["x".repeat(256 * 1024)];
    r.capture([oversized]);
    assert!(queued(&r).is_empty());
    assert_eq!(r.counts().unconfirmed, 1);
    let finished = r.finish(&session, false).unwrap();
    assert_eq!((finished.state, finished.unconfirmed), ("partial", 1));

    // Overflow must protect later submitted text that could not be registered.
    let (_owner, r, session) = fixture();
    r.payload(&"x".repeat(4 * 1024 * 1024 + 1));
    r.payload("SYNTHETIC_LATE_PAYLOAD_ECHO");
    let mut response = airs_test_harness::fixtures::scan_allow("Monitor");
    response["profile_name"] = json!("SYNTHETIC_LATE_PAYLOAD_ECHO");
    r.sync_response(&serde_json::from_value(response).unwrap(), "Monitor");
    assert_eq!(r.counts().unconfirmed, 1);
    assert!(!r.0.context.as_ref().unwrap().0.exists());
    let warning = r.finish(&session, false).unwrap().warning().unwrap();
    assert!(warning.starts_with("Runtime history:"));
    assert!(!warning.contains("SYNTHETIC_LATE_PAYLOAD_ECHO"));
    assert_eq!(r.finish(&session, false), None, "one outcome per run");
}

#[test]
fn completion_contention_preserves_received_data_and_warns_once_without_zero_loss_claim() {
    let _isolation = crate::test_isolation();
    let (_owner, r, session) = fixture();
    r.capture([r.event(SCAN, None, None, None, "cli_results")]);
    r.flush();
    let (root, cloud) = r.0.context.as_ref().unwrap();
    let read = super::super::database::Reader::open(root, cloud, &scm::Session::new())
        .unwrap()
        .unwrap();
    let warning = r.finish(&session, false).unwrap().warning().unwrap();
    assert_eq!(r.finish(&session, false), None, "warns once");
    assert!(warning.contains("retained observations remain available"));
    assert!(!warning.contains("for 0"));
    assert_eq!(
        read.conn
            .query_row("SELECT count(*) FROM receipts", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        read.conn
            .query_row("SELECT state FROM collection_runs", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "running"
    );
    read.finish().unwrap();
}

#[test]
fn flag_derived_verdict_keeps_the_contradictory_api_category_explicit() {
    let _isolation = crate::test_isolation();
    let (_owner, r, _) = fixture();
    let mut response = airs_test_harness::fixtures::scan_allow("Monitor");
    response["prompt_detected"]["injection"] = json!(true);
    let response: ScanResponse = serde_json::from_value(response).unwrap();
    r.results(
        SCAN,
        &[serde_json::from_value(
            json!({"scan_id":SCAN,"req_id":0,"status":"complete","result":response}),
        )
        .unwrap()],
    );
    let v = queued(&r);
    assert_eq!(v[0]["verdict"], "malicious");
    let notes = v[0]["notes"].as_array().unwrap();
    assert!(notes.iter().any(|s| s == "API scan category: benign"));
    assert!(notes.iter().any(|s| {
        s.as_str()
            .is_some_and(|s| s.contains("derived from detector flags"))
    }));
}

/// Migrated from the former executable `interrupted_bulk_retains_durable_receipts_and_resume
/// _deduplicates_without_resubmission` (plan M1.3): the domain rule without the removed CLI
/// adapter. Receipts scoped to one bulk job keep one durable receipt per request index; a
/// resumed run's results for the same job and scan add facts to those receipts instead of
/// new ones, a repeated resume changes nothing, and no prompt text is ever stored.
#[test]
fn job_scoped_receipts_deduplicate_across_resume_and_never_store_payload() {
    let _isolation = crate::test_isolation();
    let (d, first, session) = fixture();
    let items = || [(0, Some(SCAN.to_owned())), (1, Some(SCAN.to_owned()))];
    first.payload("SYNTHETIC_BULK_PAYLOAD_FIRST");
    first.payload("SYNTHETIC_BULK_PAYLOAD_SECOND");
    first.job("job-1", items());
    let objects: Vec<AsyncScanObject> = (0..2)
        .map(|i| {
            serde_json::from_value(json!({"req_id":i,"scan_req":{"ai_profile":{"profile_name":"Monitor"},"contents":[{"prompt":"SYNTHETIC_BULK_PAYLOAD_FIRST"}]}}))
                .unwrap()
        })
        .collect();
    let response: AsyncScanResponse = serde_json::from_value(
        json!({"received":"2026-01-01T00:00:00Z","scan_id":SCAN,"report_id":"R-1"}),
    )
    .unwrap();
    first.submission(&objects, &response);
    first.flush();
    assert_eq!(
        first.finish(&scm::Session::new(), true).unwrap().state,
        "partial"
    );
    let (root, cloud) = first.0.context.clone().unwrap();
    let count = |sql: &str| {
        let read = super::super::database::Reader::open(&root, &cloud, &scm::Session::new())
            .unwrap()
            .unwrap();
        let value = read
            .conn
            .query_row(sql, [], |r| r.get::<_, i64>(0))
            .unwrap();
        read.finish().unwrap();
        value
    };
    assert_eq!(count("SELECT count(*) FROM receipts"), 2);
    let rows: Vec<ScanIdResult> = (0..2)
        .map(|i| {
            let mut result = airs_test_harness::fixtures::scan_allow("Monitor");
            result["scan_id"] = json!(SCAN);
            serde_json::from_value(
                json!({"scan_id":SCAN,"req_id":i,"status":"COMPLETED","result":result}),
            )
            .unwrap()
        })
        .collect();
    let resume = |_: ()| {
        let r = Receipts::prepare(
            true,
            Some((root.clone(), cloud.clone())),
            None,
            "https://example.invalid/scan",
            "resume",
            (&SecretSet::default(), &PayloadSpans::default()),
        );
        r.job("job-1", items());
        r.results(SCAN, &rows);
        r.flush();
        r.finish(&session, false).unwrap()
    };
    assert_eq!(resume(()).state, "complete");
    let after = (
        count("SELECT count(*) FROM receipts"),
        count("SELECT count(*) FROM receipt_facts"),
        count("SELECT count(*) FROM events"),
    );
    assert_eq!(after, (2, 4, 0));
    resume(());
    assert_eq!(
        (
            count("SELECT count(*) FROM receipts"),
            count("SELECT count(*) FROM receipt_facts"),
            count("SELECT count(*) FROM events"),
        ),
        after,
        "a repeated resume adds nothing"
    );
    for entry in std::fs::read_dir(d.path().join("history").join(&cloud.cloud_id)).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains("SYNTHETIC_BULK_PAYLOAD_FIRST"));
        assert!(!text.contains("SYNTHETIC_BULK_PAYLOAD_SECOND"));
    }
}

fn submit(r: &Receipts, scan: &str, requests: std::ops::Range<u64>) {
    let objects: Vec<AsyncScanObject> = requests
        .map(|i| {
            serde_json::from_value(json!({"req_id":i,"scan_req":{"ai_profile":{"profile_name":"Monitor"},"contents":[{"prompt":"SYNTHETIC_CHUNK_PAYLOAD"}]}}))
                .unwrap()
        })
        .collect();
    let response: AsyncScanResponse = serde_json::from_value(
        json!({"received":"2026-01-01T00:00:00Z","scan_id":scan,"report_id":"R-1"}),
    )
    .unwrap();
    r.submission(&objects, &response);
}

fn archive_count(r: &Receipts, sql: &str) -> i64 {
    let (root, cloud) = r.0.context.as_ref().unwrap();
    let read = super::super::database::Reader::open(root, cloud, &scm::Session::new())
        .unwrap()
        .unwrap();
    let value = read
        .conn
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .unwrap();
    read.finish().unwrap();
    value
}

/// Review M1-1 finding 2 (from the retired `failed_c3_checkpoint_never_creates_secondary
/// _history`): receipts captured but never flushed, because the caller's own checkpoint
/// did not complete, are abandoned at finish. No archive is created, every one counts as
/// unconfirmed, and the run reports one partial outcome.
#[test]
fn unflushed_receipts_are_abandoned_without_creating_an_archive() {
    let _isolation = crate::test_isolation();
    let (d, r, session) = fixture();
    submit(&r, SCAN, 0..2);
    let finished = r.finish(&session, true).unwrap();
    assert_eq!(
        finished,
        Finished {
            state: "partial",
            unconfirmed: 2,
            finalization_failed: false,
        }
    );
    assert!(
        finished
            .warning()
            .unwrap()
            .contains("for 2 received observation(s)")
    );
    assert!(!d.path().join("history").exists(), "no secondary archive");
    assert_eq!(r.finish(&session, true), None, "one outcome per run");
}

/// Review M1-1 finding 2 (from the retired signal tests): a cancelled session finishes
/// the run as `interrupted`, after the flushed receipts are durable.
#[test]
fn a_cancelled_run_finishes_interrupted_with_its_flushed_receipts() {
    let _isolation = crate::test_isolation();
    let (_d, r, session) = fixture();
    submit(&r, SCAN, 0..2);
    r.flush();
    session.cancel();
    let finished = r.finish(&session, false).unwrap();
    assert_eq!((finished.state, finished.unconfirmed), ("interrupted", 0));
    assert_eq!(finished.warning(), None);
    assert_eq!(archive_count(&r, "SELECT count(*) FROM receipts"), 2);
    assert_eq!(
        archive_count(
            &r,
            "SELECT count(*) FROM collection_runs WHERE state='interrupted'"
        ),
        1
    );
}

/// Review M1-1 finding 2 (from the retired `large_logical_bulk_batches_keep_all_twenty
/// _item_sdk_chunks`): one logical batch submitted as SDK chunks of 20, 20 and 5 under
/// distinct scan IDs keeps all 45 receipts; a resume's results attach facts to them and
/// add no receipt.
#[test]
fn chunked_submissions_keep_every_receipt_through_resume() {
    let _isolation = crate::test_isolation();
    let (_d, r, session) = fixture();
    let scans = [
        "00000000-0000-4000-8000-0000000000c1",
        "00000000-0000-4000-8000-0000000000c2",
        "00000000-0000-4000-8000-0000000000c3",
    ];
    let chunks = [0..20, 20..40, 40..45];
    let items = || {
        chunks
            .iter()
            .zip(scans)
            .flat_map(|(range, scan)| range.clone().map(move |i| (i, Some(scan.to_owned()))))
    };
    r.job("job-45", items());
    for (range, scan) in chunks.iter().zip(scans) {
        submit(&r, scan, range.clone());
    }
    r.flush();
    assert_eq!(r.finish(&session, false).unwrap().state, "complete");
    assert_eq!(archive_count(&r, "SELECT count(*) FROM receipts"), 45);
    let (root, cloud) = r.0.context.clone().unwrap();
    let resumed = Receipts::prepare(
        true,
        Some((root, cloud)),
        None,
        "https://example.invalid/scan",
        "resume",
        (&SecretSet::default(), &PayloadSpans::default()),
    );
    resumed.job("job-45", items());
    for (range, scan) in chunks.iter().zip(scans) {
        let rows: Vec<ScanIdResult> = range
            .clone()
            .map(|i| {
                let mut result = airs_test_harness::fixtures::scan_allow("Monitor");
                result["scan_id"] = json!(scan);
                serde_json::from_value(
                    json!({"scan_id":scan,"req_id":i,"status":"COMPLETED","result":result}),
                )
                .unwrap()
            })
            .collect();
        resumed.results(scan, &rows);
    }
    resumed.flush();
    assert_eq!(resumed.finish(&session, false).unwrap().state, "complete");
    assert_eq!(archive_count(&resumed, "SELECT count(*) FROM receipts"), 45);
    assert_eq!(
        archive_count(&resumed, "SELECT count(*) FROM receipt_facts"),
        90
    );
}

/// Review M1-1 finding 2 (from the retired `busy_archive_skips_bounded_batches_with_one
/// _warning_and_no_remote_retries`): a batch that cannot be persisted while another
/// holder keeps the archive busy is counted as unconfirmed, the earlier retained batch
/// stays, and the run reports exactly one partial outcome with the accurate count.
#[test]
fn a_failed_batch_under_contention_counts_exactly_its_receipts() {
    let _isolation = crate::test_isolation();
    let (_d, r, session) = fixture();
    submit(&r, SCAN, 0..2);
    r.flush();
    assert_eq!(archive_count(&r, "SELECT count(*) FROM receipts"), 2);
    let (root, cloud) = r.0.context.clone().unwrap();
    let busy = super::super::database::Reader::open(&root, &cloud, &scm::Session::new())
        .unwrap()
        .unwrap();
    submit(&r, "00000000-0000-4000-8000-0000000000d4", 2..5);
    r.flush();
    assert_eq!(r.counts().unconfirmed, 3);
    busy.finish().unwrap();
    let finished = r.finish(&session, false).unwrap();
    assert_eq!((finished.state, finished.unconfirmed), ("partial", 3));
    assert!(
        finished
            .warning()
            .unwrap()
            .contains("for 3 received observation(s)")
    );
    assert_eq!(r.finish(&session, false), None, "one outcome per run");
    assert_eq!(archive_count(&r, "SELECT count(*) FROM receipts"), 2);
}
