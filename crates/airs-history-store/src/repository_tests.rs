use super::*;
use crate::test_support::TestDir;
use airs_runtime_core::{
    signal::evidence::Identity,
    tenant::{Selected, TenantContext, config::TenantConfig, scanners::ScannerReference},
};
const SCAN: &str = "00000000-0000-4000-8000-0000000000a1";
const A: &str = "https://example.invalid/scan-a";
const B: &str = "https://example.invalid/scan-b";
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
fn event() -> Event {
    Event {
        identity: Identity {
            app_id: "app-1".into(),
            app_name: "Exact application".into(),
            session_id: "session-1".into(),
            scan_id: SCAN.into(),
            sub_request: Some(0),
        },
        timestamp_ms: Some(1),
        profile: None,
        verdict: "malicious".into(),
        action: "allow".into(),
        report_ids: vec!["report-1".into()],
        detections: vec![],
        detector_hints: vec!["prompt/injection".into()],
        sources: vec!["synthetic discovery".into()],
        notes: vec![],
    }
}
fn reference(endpoint: &str) -> ScannerReference {
    ScannerReference {
        name: "archive".into(),
        application_id: "app-1".into(),
        endpoint: endpoint.into(),
        revision: "11111111-1111-4111-8111-111111111111".into(),
    }
}
fn full(root: &Path, c: &Cloud, endpoint: Option<&str>) {
    let w = Writer::open(root, c, Duration::from_secs(2), true, &scm::Session::new()).unwrap();
    w.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    w.conn.execute("INSERT OR IGNORE INTO collection_runs VALUES ('full','sync',1,1,NULL,NULL,'complete','{}')",[]).unwrap();
    let fact = EventFact {
        identity_withheld: false,
        event: event(),
        origin: "synthetic full identity".into(),
        scanner_endpoint: endpoint.map(str::to_owned),
        scanner_reference: endpoint.map(reference),
        batch_report_id: None,
    };
    store(&w.conn, &Record::Event(fact), "full", 1).unwrap();
    w.conn.execute_batch("COMMIT").unwrap();
    w.check().unwrap();
}
fn receipt(key: &str, endpoint: &str) -> Received {
    let mut e = event();
    e.identity.app_name.clear();
    e.timestamp_ms = None;
    Received {
        receipt_key: key.into(),
        batch_report_id: None,
        event: e,
        source: "cli_results",
        endpoint: endpoint.into(),
        reference: Some(reference(endpoint)),
    }
}
fn retain(root: &Path, c: &Cloud, input: Received) {
    retain_received(
        root,
        c,
        "22222222-2222-4222-8222-222222222222",
        "scan",
        &[input],
        &SecretSet::default(),
        &PayloadSpans::default(),
    )
    .unwrap();
}
fn states(root: &Path, c: &Cloud) -> Vec<String> {
    let r = database::Reader::open(root, c, &scm::Session::new())
        .unwrap()
        .unwrap();
    let v = r
        .conn
        .prepare("SELECT state FROM receipts ORDER BY state")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<String>>>()
        .unwrap();
    r.finish().unwrap();
    v
}
#[test]
fn named_receipts_require_known_endpoint_support_in_both_orders_and_invalidate_on_conflict() {
    let _isolation = crate::test_isolation();
    for first in [true, false] {
        for known in [true, false] {
            let d = TestDir::new("received-endpoints");
            let c = cloud();
            if !first {
                full(d.path(), &c, known.then_some(A));
            }
            retain(d.path(), &c, receipt("job/one/item/0", A));
            retain(d.path(), &c, receipt("job/two/item/0", B));
            if first {
                full(d.path(), &c, known.then_some(A));
            }
            assert_eq!(
                states(d.path(), &c),
                if known {
                    vec!["partial", "promoted"]
                } else {
                    vec!["partial", "partial"]
                }
            );
            let r = database::Reader::open(d.path(), &c, &scm::Session::new())
                .unwrap()
                .unwrap();
            assert_eq!(
                r.conn
                    .query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                1
            );
            r.finish().unwrap();
            if known {
                full(d.path(), &c, Some(B));
                assert_eq!(states(d.path(), &c), vec!["partial", "partial"]);
            }
        }
    }
}
#[test]
fn rotation_keeps_revisions_and_batch_report_context_is_not_item_ownership() {
    let _isolation = crate::test_isolation();
    let d = TestDir::new("received-reference");
    let c = cloud();
    let mut submitted = receipt("job/one/item/0", A);
    submitted.source = "cli_submit";
    submitted.batch_report_id = Some("batch-report-context".into());
    submitted.event.report_ids.clear();
    submitted.event.verdict = "unknown".into();
    submitted.event.action = "unknown".into();
    retain(d.path(), &c, submitted);
    let mut completed = receipt("job/one/item/0", A);
    completed.reference.as_mut().unwrap().revision = "33333333-3333-4333-8333-333333333333".into();
    retain(d.path(), &c, completed);
    full(d.path(), &c, Some(A));
    assert_eq!(states(d.path(), &c), vec!["promoted"]);
    let r = database::Reader::open(d.path(), &c, &scm::Session::new())
        .unwrap()
        .unwrap();
    let(row,facts,refs,batch):(String,i64,i64,String)=r.conn.query_row("SELECT report_id,(SELECT count(*) FROM receipt_facts),json_array_length(binding_json,'$.scanner_references'),(SELECT json_extract(fields_json,'$.batch_report_id') FROM receipt_facts WHERE json_extract(fields_json,'$.batch_report_id') IS NOT NULL) FROM receipts",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(
        (row, facts, refs, batch),
        ("report-1".into(), 2, 2, "batch-report-context".into())
    );
    r.finish().unwrap();
}

#[test]
fn admitted_root_ip_endpoint_preserves_intact_named_binding() {
    let _isolation = crate::test_isolation();
    let d = TestDir::new("received-ip-binding");
    let c = cloud();
    let endpoint = "http://127.0.0.1:1";
    retain(d.path(), &c, receipt("job/one/item/0", endpoint));
    let r = database::Reader::open(d.path(), &c, &scm::Session::new())
        .unwrap()
        .unwrap();
    let(app,endpoint,name):(String,String,String)=r.conn.query_row("SELECT app_id,json_extract(binding_json,'$.scanner_endpoint'),json_extract(binding_json,'$.scanner_references[0].name') FROM receipts",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(
        (app, endpoint, name),
        (
            "app-1".into(),
            "http://127.0.0.1:1".into(),
            "archive".into()
        )
    );
    r.finish().unwrap();
}

#[test]
fn deferred_batch_reconciliation_preserves_prior_supported_identity_when_new_fields_are_unknown() {
    let _isolation = crate::test_isolation();
    let d = TestDir::new("received-known-then-unknown");
    let c = cloud();
    full(d.path(), &c, Some(A));
    retain(d.path(), &c, receipt("job/one/item/0", A));
    assert_eq!(states(d.path(), &c), vec!["promoted"]);
    let mut unknown = receipt("job/one/item/0", A);
    unknown.event.identity.scan_id.clear();
    unknown.event.report_ids.clear();
    unknown.event.verdict = "unknown".into();
    unknown.event.action = "unknown".into();
    retain(d.path(), &c, unknown);
    assert_eq!(states(d.path(), &c), vec!["promoted"]);
}

#[test]
fn history_cached_detector_variants_remain_conflicting_and_preserve_raw_facts() {
    let _isolation = crate::test_isolation();
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(database::SCHEMA).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON; INSERT INTO collection_runs VALUES('run','sync',1,1,NULL,NULL,'complete','{}')").unwrap();
    let e = Evidence::parse(include_str!("../../../tests/fixtures/signal/evidence.json")).unwrap();
    for (n, action) in ["allow", "block"].into_iter().enumerate() {
        let mut event = e.events[0].clone();
        event.action = action.into();
        event.detections[0].action = action.into();
        let fact = EventFact {
            identity_withheld: false,
            event,
            origin: format!("synthetic-variant-{n}"),
            scanner_endpoint: Some(A.into()),
            scanner_reference: Some(reference(A)),
            batch_report_id: None,
        };
        store(&conn, &Record::Event(fact), "run", n as i64 + 1).unwrap();
    }
    let view = cached_event(&conn, &e.events[0].identity).unwrap().unwrap();
    assert_eq!(view.action, "unknown");
    assert_eq!(view.detections.len(), 1);
    assert_eq!(view.detections[0].action, "unknown");
    assert!(
        view.notes
            .iter()
            .any(|s| s.contains("archive preserves all variants"))
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM event_facts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    let mut snapshot = e.clone();
    snapshot.events = vec![view];
    snapshot.seal_native().unwrap();
}

#[test]
fn history_cached_view_removes_promoted_receipt_facts_after_global_identity_ambiguity() {
    let _isolation = crate::test_isolation();
    let dir = TestDir::new("promoted-cache-ownership");
    let c = cloud();
    full(dir.path(), &c, Some(A));
    let base = event();
    let descriptor = super::super::work::Descriptor {
        identity: base.identity.clone(),
        report_id: Some("report-1".into()),
    };
    let mut received = receipt("promoted-extra-hint", A);
    received.event.detector_hints.push("response/dlp".into());
    retain(dir.path(), &c, received);
    assert_eq!(states(dir.path(), &c), vec!["promoted"]);
    let reader = database::Reader::open(dir.path(), &c, &scm::Session::new())
        .unwrap()
        .unwrap();
    assert!(
        cached_event(&reader.conn, &base.identity)
            .unwrap()
            .unwrap()
            .detector_hints
            .contains(&"response/dlp".into())
    );
    assert!(report_ownership(&reader.conn, &descriptor).unwrap());
    reader.finish().unwrap();
    let mut competing = base.clone();
    competing.identity.app_name = "Competing exact application".into();
    let w = Writer::open(
        dir.path(),
        &c,
        Duration::from_secs(5),
        true,
        &scm::Session::new(),
    )
    .unwrap();
    w.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    store(
        &w.conn,
        &Record::Event(EventFact {
            identity_withheld: false,
            event: competing,
            origin: "synthetic-competing-context".into(),
            scanner_endpoint: Some(A.into()),
            scanner_reference: Some(reference(A)),
            batch_report_id: None,
        }),
        "full",
        2,
    )
    .unwrap();
    w.conn.execute_batch("COMMIT").unwrap();
    drop(w);
    assert_eq!(states(dir.path(), &c), vec!["ambiguous"]);
    let reader = database::Reader::open(dir.path(), &c, &scm::Session::new())
        .unwrap()
        .unwrap();
    assert!(
        !cached_event(&reader.conn, &base.identity)
            .unwrap()
            .unwrap()
            .detector_hints
            .contains(&"response/dlp".into())
    );
    assert!(!report_ownership(&reader.conn, &descriptor).unwrap());
    reader.finish().unwrap();
}

#[test]
fn history_private_supplied_identity_constraints_never_promote_to_an_existing_public_bucket() {
    let _isolation = crate::test_isolation();
    for native in [false, true] {
        for field in ["app_name", "report_id"] {
            for label in [
                "private@example.invalid",
                airs_runtime_core::signal::privacy::WITHHELD,
            ] {
                let conn = Connection::open_in_memory().unwrap();
                conn.execute_batch(database::SCHEMA).unwrap();
                conn.execute_batch("PRAGMA foreign_keys=ON; INSERT INTO collection_runs VALUES('run','sync',1,1,NULL,NULL,'complete','{}')").unwrap();
                let mut e =
                    Evidence::parse(include_str!("../../../tests/fixtures/signal/evidence.json"))
                        .unwrap();
                let public = e.events[0].clone();
                let fact = EventFact {
                    event: public.clone(),
                    identity_withheld: false,
                    origin: "public-full-identity".into(),
                    scanner_endpoint: None,
                    scanner_reference: None,
                    batch_report_id: None,
                };
                store(&conn, &Record::Event(fact), "run", 1).unwrap();
                if field == "app_name" {
                    e.events[0].identity.app_name = label.into();
                } else {
                    e.events[0].identity.app_name.clear();
                    e.events[0].report_ids = vec![label.into()];
                    e.events[0].detections.clear();
                    e.sampling.selected_report_ids.clear();
                }
                e.seal().unwrap();
                let prepared =
                    prepare_evidence(&e, &SecretSet::default(), &PayloadSpans::default(), native)
                        .unwrap();
                for record in prepared
                    .records
                    .iter()
                    .filter(|r| matches!(r, Record::Event(_) | Record::Receipt { .. }))
                {
                    store(&conn, record, "run", 2).unwrap();
                }
                // Repeated discovery must not resurrect the unsupported erased constraint.
                reconcile_scan(&conn, &public.identity.scan_id).unwrap();
                let receipt:(String,Option<String>,bool)=conn.query_row("SELECT state,promoted_event_id,json_extract(binding_json,'$.identity_withheld') FROM receipts",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
                assert_eq!(receipt, ("partial".into(), None, true));
                assert_eq!(
                    conn.query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
                        .unwrap(),
                    1
                );
                assert_eq!(
                    conn.query_row("SELECT count(*) FROM event_facts", [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    1
                );
                let contents: String = conn
                    .query_row("SELECT fields_json FROM receipt_facts", [], |r| r.get(0))
                    .unwrap();
                assert!(!contents.contains("private@example.invalid"));
            }
        }
    }
}

#[test]
fn history_private_report_constraint_invalidates_a_prior_missing_report_promotion() {
    let _isolation = crate::test_isolation();
    for label in [
        "private@example.invalid",
        airs_runtime_core::signal::privacy::WITHHELD,
    ] {
        let dir = TestDir::new("private-receipt-report");
        let c = cloud();
        full(dir.path(), &c, Some(A));
        let mut received = receipt("stable/receipt/item", A);
        received.event.report_ids.clear();
        retain(dir.path(), &c, received);
        assert_eq!(states(dir.path(), &c), vec!["promoted"]);
        let mut received = receipt("stable/receipt/item", A);
        received.event.report_ids = vec![label.into()];
        retain(dir.path(), &c, received);
        assert_eq!(states(dir.path(), &c), vec!["partial"]);
        full(dir.path(), &c, Some(A));
        assert_eq!(states(dir.path(), &c), vec!["partial"]);
        let reader = database::Reader::open(dir.path(), &c, &scm::Session::new())
            .unwrap()
            .unwrap();
        let flag: bool = reader
            .conn
            .query_row(
                "SELECT json_extract(binding_json,'$.identity_withheld') FROM receipts",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(flag);
        reader.finish().unwrap();
        let bytes = std::fs::read(
            dir.path()
                .join(&c.cloud_id)
                .join(super::super::paths::DATABASE),
        )
        .unwrap();
        assert!(
            !bytes
                .windows(b"private@example.invalid".len())
                .any(|s| s == b"private@example.invalid")
        );
    }
}

/// M4 review 13 minor: a service collection run's new/duplicate fact counts come from the
/// record its writes keep, so equal or rolled-back start times change nothing.
#[test]
fn service_fact_counts_do_not_depend_on_run_timestamps() {
    let d = TestDir::new("fact-counts");
    let c = cloud();
    let w = Writer::open(
        d.path(),
        &c,
        Duration::from_secs(2),
        true,
        &scm::Session::new(),
    )
    .unwrap();
    w.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    for run in ["run-a", "run-b"] {
        w.conn
            .execute(
                "INSERT INTO collection_runs VALUES (?1,'sync',5,6,NULL,NULL,'complete','{\"fact_counts\":{\"new\":0,\"duplicate\":0}}')",
                [run],
            )
            .unwrap();
    }
    let fact = EventFact {
        identity_withheld: false,
        event: event(),
        origin: "synthetic counted fact".into(),
        scanner_endpoint: None,
        scanner_reference: None,
        batch_report_id: None,
    };
    store(&w.conn, &Record::Event(fact.clone()), "run-a", 5).unwrap();
    store(&w.conn, &Record::Event(fact.clone()), "run-b", 5).unwrap();
    // A repeated write within a run links nothing new and counts nothing.
    store(&w.conn, &Record::Event(fact), "run-b", 5).unwrap();
    w.conn.execute_batch("COMMIT").unwrap();
    w.check().unwrap();
    drop(w);
    let counts = |run: &str| {
        let s = crate::run_summary::run_summary(d.path(), &c, run, &scm::Session::new())
            .unwrap()
            .unwrap();
        (s.new_facts, s.duplicate_facts)
    };
    assert_eq!(
        counts("run-a"),
        (1, 0),
        "equal start times: the creating run owns it"
    );
    assert_eq!(counts("run-b"), (0, 1));
    // Roll the creating run's start time back after the other: unchanged.
    let w = Writer::open(
        d.path(),
        &c,
        Duration::from_secs(2),
        false,
        &scm::Session::new(),
    )
    .unwrap();
    w.conn
        .execute(
            "UPDATE collection_runs SET started_ms=1 WHERE run_id='run-b'",
            [],
        )
        .unwrap();
    drop(w);
    assert_eq!(counts("run-a"), (1, 0));
    assert_eq!(counts("run-b"), (0, 1));
}

/// An event carrying one present excerpt (the erasure tests' fixture).
fn excerpted() -> EventFact {
    use airs_runtime_core::signal::evidence::{Detection, Snippet, SnippetState};
    let mut e = event();
    e.detections = vec![Detection {
        detector: "prompt_injection".into(),
        direction: "prompt".into(),
        verdict: "malicious".into(),
        action: "allow".into(),
        report_id: "report-1".into(),
        row: 0,
        snippets: vec![Snippet {
            family: "pi".into(),
            state: SnippetState::Present,
            excerpts: vec!["Synthetic erased excerpt".into()],
            variants: 1,
            truncated: false,
        }],
    }];
    EventFact {
        event: e,
        identity_withheld: false,
        origin: "synthetic".into(),
        scanner_endpoint: None,
        scanner_reference: None,
        batch_report_id: None,
    }
}

/// The erasure tests' in-memory archive, after a real examples-only prune of `fact`.
fn pruned_archive(fact: &EventFact) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(database::SCHEMA).unwrap();
    c.execute(
        "INSERT INTO collection_runs VALUES ('run','import',1,2,NULL,NULL,'complete','{}')",
        [],
    )
    .unwrap();
    store(&c, &Record::Event(fact.clone()), "run", 1).unwrap();
    // The prune's own effects: tombstones, examples removed, every fact copy stripped.
    c.execute(
        "INSERT INTO pruned_examples SELECT example_id,2 FROM examples",
        [],
    )
    .unwrap();
    c.execute("DELETE FROM example_runs", []).unwrap();
    c.execute("DELETE FROM examples", []).unwrap();
    c.execute("UPDATE event_facts SET fields_json=json_set(fields_json,'$.event.detections[0].snippets[0].excerpts',json('[]'),'$.event.detections[0].snippets[0].state','withheld')", []).unwrap();
    c
}

fn holds_erased(c: &Connection) -> bool {
    let n: i64 = c
        .query_row(
            "SELECT (SELECT count(*) FROM examples JOIN pruned_examples USING(example_id))
                  + (SELECT count(*) FROM examples WHERE instr(excerpts_json,'Synthetic erased excerpt')>0)
                  + (SELECT count(*) FROM event_facts WHERE instr(fields_json,'Synthetic erased excerpt')>0)
                  + (SELECT count(*) FROM receipt_facts WHERE instr(fields_json,'Synthetic erased excerpt')>0)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    n > 0
}

/// M5 review 1 finding 3 (the reviewer's probe): ordinary retention of the repeated
/// fact, and of a new variant carrying the same erased excerpt, never recreates a
/// pruned example or stores the excerpt in any fact copy.
#[test]
fn ordinary_retention_never_resurrects_a_pruned_excerpt() {
    let fact = excerpted();
    let c = pruned_archive(&fact);
    assert!(!holds_erased(&c));
    // The repeated fact.
    store(&c, &Record::Event(fact.clone()), "run", 3).unwrap();
    assert!(!holds_erased(&c), "a repeated fact brings nothing back");
    // A new variant of it (another origin) carrying the same erased excerpt.
    let mut variant = fact.clone();
    variant.origin = "synthetic variant".into();
    store(&c, &Record::Event(variant), "run", 4).unwrap();
    assert!(!holds_erased(&c), "a new variant brings nothing back");
    let facts: i64 = c
        .query_row("SELECT count(*) FROM event_facts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(facts, 2, "the variant is kept, without the erased excerpt");
}

/// Finding 3, receipt promotion: a receipt carrying the erased excerpt, promoted to the
/// pruned event, has it erased from its facts.
#[test]
fn a_promoted_receipt_never_brings_back_a_pruned_excerpt() {
    let fact = excerpted();
    let c = pruned_archive(&fact);
    store(
        &c,
        &Record::Receipt {
            origin: "synthetic-receipt-origin".into(),
            fact: fact.clone(),
        },
        "run",
        5,
    )
    .unwrap();
    let promoted: Option<String> = c
        .query_row("SELECT promoted_event_id FROM receipts", [], |r| r.get(0))
        .unwrap();
    assert!(promoted.is_some(), "the receipt is promoted to the event");
    assert!(!holds_erased(&c), "the promoted receipt's copy is erased");
}

/// M5 review 1 finding 4: the mutation generation advances with every committed write,
/// including a change inside an existing run that adds no run or tombstone; a prune
/// bound to a stale generation is refused before any erasure, and one bound to the
/// current generation runs.
#[test]
fn the_mutation_generation_binds_every_write_and_a_stale_prune_is_refused() {
    let _isolation = crate::test_isolation();
    let d = TestDir::new("generation");
    let root = d.path().join("history");
    let c = cloud();
    let session = scm::Session::new();
    let w = Writer::open(&root, &c, Duration::from_secs(2), true, &session).unwrap();
    w.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    w.conn
        .execute(
            "INSERT INTO collection_runs VALUES ('run','import',1,2,NULL,NULL,'complete','{}')",
            [],
        )
        .unwrap();
    store(&w.conn, &Record::Event(excerpted()), "run", 1).unwrap();
    w.conn.execute_batch("COMMIT").unwrap();
    drop(w);
    let first = crate::inspect::data_generation(&root, &c, &session).unwrap();
    // A write inside the existing run (no new run, no tombstone).
    let w = Writer::open(&root, &c, Duration::from_secs(2), false, &session).unwrap();
    w.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    let mut variant = excerpted();
    variant.origin = "another origin, same run".into();
    store(&w.conn, &Record::Event(variant), "run", 2).unwrap();
    w.conn.execute_batch("COMMIT").unwrap();
    drop(w);
    let second = crate::inspect::data_generation(&root, &c, &session).unwrap();
    assert_ne!(first, second, "a write inside an existing run advances it");
    let path = root.join(&c.cloud_id).join("archive.sqlite3");
    let before = std::fs::read(&path).unwrap();
    let stale = prune_examples_bound(
        &root,
        &c,
        i64::MAX,
        &[],
        first,
        Duration::from_secs(2),
        &session,
    );
    assert!(stale.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before, "nothing erased");
    prune_examples_bound(
        &root,
        &c,
        i64::MAX,
        &[],
        second,
        Duration::from_secs(2),
        &session,
    )
    .unwrap();
    assert_ne!(
        crate::inspect::data_generation(&root, &c, &session).unwrap(),
        second
    );
}

/// M5 review 2 finding 2: a receipt-only excerpt (a promoted receipt fact whose snippet
/// no examples row holds) is tombstoned by the prune's own erasure, so neither the same
/// receipt, a new receipt variant nor a repeated event fact brings it back.
#[test]
fn a_receipt_only_excerpt_is_tombstoned_and_never_resurrects() {
    let mut full = excerpted();
    full.event.detections.clear();
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(database::SCHEMA).unwrap();
    c.execute(
        "INSERT INTO collection_runs VALUES ('run','import',1,2,NULL,NULL,'complete','{}')",
        [],
    )
    .unwrap();
    store(&c, &Record::Event(full), "run", 1).unwrap();
    let receipt = excerpted();
    store(
        &c,
        &Record::Receipt {
            origin: "receipt-only".into(),
            fact: receipt.clone(),
        },
        "run",
        2,
    )
    .unwrap();
    let promoted: String = c
        .query_row("SELECT promoted_event_id FROM receipts", [], |r| r.get(0))
        .unwrap();
    assert!(holds_erased(&c));
    erase_events(&c, &[promoted], 3, &|| Ok(())).unwrap();
    assert!(!holds_erased(&c), "every copy stripped");
    let tombstones: i64 = c
        .query_row("SELECT count(*) FROM pruned_examples", [], |r| r.get(0))
        .unwrap();
    assert!(tombstones > 0, "the receipt-only snippet has a tombstone");
    let mut variant = receipt.clone();
    variant.origin = "new receipt variant".into();
    store(
        &c,
        &Record::Receipt {
            origin: "receipt-only".into(),
            fact: variant,
        },
        "run",
        4,
    )
    .unwrap();
    store(
        &c,
        &Record::Receipt {
            origin: "another receipt".into(),
            fact: receipt.clone(),
        },
        "run",
        5,
    )
    .unwrap();
    store(&c, &Record::Event(receipt), "run", 6).unwrap();
    assert!(
        !holds_erased(&c),
        "no retention, variant or promotion resurrects it"
    );
}

/// M5 review 3 finding 6: a prune of an ordinary large selection (20,000 events, more
/// than SQLite's 1 MiB value limit as one ID list) previews and executes: the selection
/// is SQL, never one bound list, and its rows are processed a page at a time. A
/// receipt-only excerpt (a promoted receipt fact carrying a snippet no example row holds)
/// is tombstoned and stripped, and the reported tombstones are the recorded ones.
#[test]
fn a_prune_of_twenty_thousand_events_stays_within_sqlite_limits() {
    let _isolation = crate::test_isolation();
    let d = TestDir::new("large-prune");
    let c = cloud();
    let n = 20_000usize;
    {
        // Give the 20,000-row fixture setup headroom during concurrent release builds.
        // The prune operations below retain their separate operation budgets.
        let w = Writer::open(
            d.path(),
            &c,
            Duration::from_secs(300),
            true,
            &scm::Session::new(),
        )
        .unwrap();
        w.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        w.conn
            .execute("INSERT OR IGNORE INTO collection_runs VALUES ('large','sync',1,1,NULL,NULL,'complete','{}')", [])
            .unwrap();
        for i in 0..n {
            let mut fact = excerpted();
            fact.event.identity.scan_id = format!("00000000-0000-4000-8000-{i:012x}");
            fact.event.identity.session_id = format!("session-{i}");
            fact.origin = format!("synthetic large {i}");
            store(&w.conn, &Record::Event(fact), "large", 1).unwrap();
        }
        w.conn.execute_batch("COMMIT").unwrap();
        w.check().unwrap();
    }
    let preview = prune_examples(
        d.path(),
        &c,
        i64::MAX,
        &[],
        false,
        Duration::from_secs(120),
        &scm::Session::new(),
    )
    .unwrap();
    assert_eq!(preview["events"].as_u64().unwrap() as usize, n, "{preview}");
    assert_eq!(
        preview["examples"].as_u64().unwrap() as usize,
        n,
        "{preview}"
    );
    let generation = preview["generation"].as_i64().unwrap();
    let done = prune_examples_bound(
        d.path(),
        &c,
        i64::MAX,
        &[],
        generation,
        Duration::from_secs(600),
        &scm::Session::new(),
    )
    .unwrap();
    assert_eq!(done["events"].as_u64().unwrap() as usize, n, "{done}");
    assert!(done["tombstones"].as_u64().unwrap() >= n as u64, "{done}");
    let r = Reader::open(d.path(), &c, &scm::Session::new())
        .unwrap()
        .unwrap();
    let (examples, carrying): (i64, i64) = r
        .conn
        .query_row(
            "SELECT (SELECT count(*) FROM examples), (SELECT count(*) FROM event_facts WHERE instr(fields_json,'\"excerpts\":[\"')>0)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((examples, carrying), (0, 0));
}
