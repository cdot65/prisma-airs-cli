use super::*;
use crate::test_support::TestDir;
use airs_runtime_core::{
    signal::evidence::{Identity, Source as EvidenceSource, Status},
    tenant::{Selected, config::TenantConfig, scanners::ScannerCredential},
};
use airs_test_harness::FixtureServer;

fn fixture() -> (TestDir, FixtureServer, TenantContext, Evidence) {
    let dir = TestDir::new("sync-backlog");
    let server = FixtureServer::start();
    let mut config = TenantConfig::parse(
        format!(
            "mgmtEndpoint={}\nmgmtDashboardEndpoint={}\nmgmtTokenEndpoint={}\n",
            json!(server.url("/mgmt")),
            json!(server.url("/mgmt")),
            json!(server.url("/oauth"))
        )
        .as_bytes(),
    )
    .unwrap();
    for (app, name) in [("app-a", "scanner-a"), ("app-z", "scanner-z")] {
        let scanner = ScannerCredential::input(
            format!(
                "applicationId={}\nendpoint={}\napiKey='FAKE-LOCAL-ARCHIVE-KEY'\n",
                json!(app),
                json!(server.url("/scan"))
            )
            .as_bytes(),
        )
        .unwrap();
        config.set_scanner(name, scanner, false).unwrap();
    }
    let tenant = TenantContext {
        selected: Selected {
            name: "fixture".into(),
            tsg_id: "100".into(),
            by_flag: false,
        },
        config,
    };
    let mut e =
        Evidence::parse(include_str!("../../../tests/fixtures/signal/evidence.json")).unwrap();
    e.rules_version = 2;
    e.events.clear();
    e.metrics.clear();
    e.profiles.clear();
    e.applications.clear();
    e.app_ids.clear();
    e.budgets.max_reports = 0;
    e.sampling.limit = 0;
    e.sampling.selected_report_ids.clear();
    e.sources = vec![EvidenceSource {
        name: "session-details".into(),
        status: Status::Complete,
        required: true,
        records: 0,
        pages: 0,
        unit: "synthetic local scheduling fixtures".into(),
        population: "local test rows; not provider coverage".into(),
        started_ms: 1,
        ended_ms: 1,
        notes: vec![],
    }];
    e.seal_native().unwrap();
    (dir, server, tenant, e)
}
fn start(dir: &TestDir, tenant: &TenantContext, e: &Evidence, apps: Vec<String>) -> Sync {
    let session = scm::Session::new();
    let secrets = SecretSet::default();
    secrets.register_config(&tenant.config);
    let budget = airs_runtime_core::read_budget::ReadBudget::new(120, 1000, 4, session.clone());
    budget.stage(2);
    Sync::start(
        &dir.path().join("history"),
        tenant,
        (e.window.start_ms, e.window.end_ms),
        (Duration::from_secs(120), true, apps, None, None),
        (&secrets, &PayloadSpans::default()),
        &session,
        budget,
    )
    .unwrap()
}
fn event(e: &Evidence, app: &str, n: u64) -> Event {
    Event {
        identity: Identity {
            app_id: app.into(),
            app_name: format!("Exact {app}"),
            session_id: format!("session-{n}"),
            scan_id: format!("00000000-0000-4000-8000-{n:012x}"),
            sub_request: Some(0),
        },
        timestamp_ms: Some(e.window.start_ms + 1),
        profile: None,
        verdict: "unknown".into(),
        action: "unknown".into(),
        report_ids: vec![],
        detections: vec![],
        detector_hints: vec![],
        sources: vec!["session-details".into()],
        notes: vec![],
    }
}
fn seed(sync: &Sync, e: &mut Evidence) {
    e.seal_native().unwrap();
    sync.checkpoint(e).unwrap();
    for chunk in e.events.chunks(25) {
        sync.batch(|conn| {
            for (index, event) in chunk.iter().enumerate() {
                let d = Descriptor {
                    identity: event.identity.clone(),
                    report_id: None,
                };
                let Admission::Due(a) = work::reserve(
                    conn,
                    &d,
                    Source::Results,
                    sync.references.get(&event.identity.app_id),
                    &sync.run,
                    &format!("fixture-transport-{}", index / 5),
                    1,
                )?
                else {
                    panic!("fixture reservation")
                };
                work::finish(conn, &a, Outcome::NotSent, None, 2)?;
            }
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn history_backlog_due_filter_and_app_rotation_progress_beyond_the_first_two_hundred() {
    let _isolation = crate::test_isolation();
    let (dir, server, tenant, mut e) = fixture();
    for n in 1..=500 {
        e.events.push(event(&e, "app-a", n));
    }
    e.events.push(event(&e, "app-z", 501));
    let sync = start(&dir, &tenant, &e, vec![]);
    seed(&sync, &mut e);
    let first = sync.prepare_backlog(10000, 0).unwrap();
    assert!(!first.is_empty());
    assert!(first.len() <= 200);
    assert!(
        first
            .iter()
            .any(|i| i.descriptor.identity.app_id == "app-z")
    );
    let mut visited = std::collections::BTreeSet::new();
    let mut batch = first;
    while visited.len() <= 200 {
        assert!(!batch.is_empty());
        for item in batch {
            assert!(visited.insert(item.descriptor.id(Source::Results).unwrap()));
            let (a, _) = sync
                .reserve(
                    std::slice::from_ref(&item.descriptor),
                    Source::Results,
                    &item.descriptor.identity.app_id,
                )
                .unwrap();
            sync.outcomes(
                &a.into_iter()
                    .map(|(_, a)| (a, Outcome::Missing))
                    .collect::<Vec<_>>(),
                Some(200),
                Source::Results,
                None,
            )
            .unwrap();
        }
        batch = sync.prepare_backlog(10000, 0).unwrap();
    }
    assert!(visited.len() > 200);
    let mut extra = e.clone();
    extra.events = vec![event(&e, "app-z", 9999)];
    seed(&sync, &mut extra);
    drop(sync);
    let scoped = start(&dir, &tenant, &e, vec!["app-z".into()]);
    let selected = scoped.prepare_backlog(10000, 0).unwrap();
    assert!(!selected.is_empty());
    assert!(
        selected
            .iter()
            .all(|i| i.descriptor.identity.app_id == "app-z")
    );
    assert_eq!(server.request_count(), 0);
}

#[test]
fn history_oversized_first_cached_view_defers_at_rotated_revision_without_stranding_small_work() {
    let _isolation = crate::test_isolation();
    let (dir, server, mut tenant, mut e) = fixture();
    let one = event(&e, "app-a", 1);
    let two = event(&e, "app-a", 2);
    let d1 = Descriptor {
        identity: one.identity.clone(),
        report_id: None,
    };
    let d2 = Descriptor {
        identity: two.identity.clone(),
        report_id: None,
    };
    let (mut big, small) = if d1.id(Source::Results).unwrap() < d2.id(Source::Results).unwrap() {
        (one, two)
    } else {
        (two, one)
    };
    big.notes = (0..50)
        .map(|n| format!("variant-A-{n:03}-{}", "x".repeat(3980)))
        .collect();
    e.events = vec![big.clone(), small.clone()];
    let sync = start(&dir, &tenant, &e, vec![]);
    seed(&sync, &mut e);
    big.notes = (0..50)
        .map(|n| format!("variant-B-{n:03}-{}", "x".repeat(3980)))
        .collect();
    let mut variant = e.clone();
    variant.events = vec![big.clone()];
    variant.seal_native().unwrap();
    sync.checkpoint(&variant).unwrap();
    drop(sync);
    let binding = ScannerCredential::input(
        format!(
            "applicationId='app-a'\nendpoint={}\napiKey='FAKE-ROTATED-LOCAL-KEY'\n",
            json!(server.url("/scan"))
        )
        .as_bytes(),
    )
    .unwrap();
    tenant
        .config
        .set_scanner("scanner-a", binding, true)
        .unwrap();
    let sync = start(&dir, &tenant, &e, vec![]);
    let first = sync.prepare_backlog(10000, 0).unwrap();
    assert!(
        first
            .iter()
            .any(|i| i.descriptor.identity == small.identity)
    );
    assert!(!first.iter().any(|i| i.descriptor.identity == big.identity));
    let again = sync.prepare_backlog(10000, 0).unwrap();
    assert!(
        again
            .iter()
            .any(|i| i.descriptor.identity == small.identity)
    );
    let d = Descriptor {
        identity: big.identity.clone(),
        report_id: None,
    };
    let (state, revision, attempts): (String, String, i64) = sync
        .batch(|conn| {
            conn.query_row(
                "SELECT state,binding_revision,attempts FROM sync_work WHERE work_id=?1",
                [d.id(Source::Results)?],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(sql_error)
        })
        .unwrap();
    assert_eq!((state.as_str(), attempts), ("deferred", 0));
    assert_eq!(revision, sync.references["app-a"].revision);
    assert_eq!(server.request_count(), 0);
}

#[test]
fn history_recent_session_head_cannot_complete_an_older_unfinished_tail() {
    let _isolation = crate::test_isolation();
    let (dir, server, tenant, e) = fixture();
    let sync = start(&dir, &tenant, &e, vec![]);
    let key = ("app-a".into(), "Exact app-a".into(), "session-one".into());
    sync.sessions_discovered(std::slice::from_ref(&key), &e.window)
        .unwrap();
    let mut recent = e.window.clone();
    recent.start_ms += 86_400_000;
    recent.end_ms += 86_400_000;
    sync.sessions_discovered(std::slice::from_ref(&key), &recent)
        .unwrap();
    sync.session_finished((&key.0, &key.1, &key.2), &recent, true)
        .unwrap();
    let(state,attempts,epoch,rev):(String,i64,i64,Option<String>)=sync.batch(|conn|conn.query_row("SELECT state,attempts,retry_epoch,binding_revision FROM sync_work WHERE kind='session'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(sql_error)).unwrap();
    assert_eq!(
        (state.as_str(), attempts, epoch, rev),
        ("falling_behind", 0, 0, None)
    );
    assert_eq!(server.request_count(), 0);
}

#[test]
fn history_removed_and_rebound_bindings_become_durably_ineligible_without_probes() {
    let _isolation = crate::test_isolation();
    let (dir, server, mut tenant, mut e) = fixture();
    e.events = vec![event(&e, "app-a", 1), event(&e, "app-z", 2)];
    let sync = start(&dir, &tenant, &e, vec![]);
    seed(&sync, &mut e);
    drop(sync);
    assert!(tenant.config.remove_scanner("scanner-a"));
    let scanner = ScannerCredential::input(
        format!(
            "applicationId='app-z'\nendpoint={}\napiKey='FAKE-REBOUND-KEY'\n",
            json!(server.url("/different-scan"))
        )
        .as_bytes(),
    )
    .unwrap();
    tenant
        .config
        .set_scanner("scanner-z", scanner, true)
        .unwrap();
    let sync = start(&dir, &tenant, &e, vec![]);
    assert!(sync.prepare_backlog(10000, 0).unwrap().is_empty());
    let count=sync.batch(|conn|conn.query_row("SELECT count(*) FROM sync_work WHERE kind='results' AND state='ineligible' AND reason IN ('binding_unavailable','binding_rebound')",[],|r|r.get::<_,i64>(0)).map_err(sql_error)).unwrap();
    assert_eq!(count, 2);
    let before = sync
        .batch(|conn| {
            conn.query_row(
                "SELECT count(*) FROM work_transitions WHERE kind='ineligible'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map_err(sql_error)
        })
        .unwrap();
    assert!(sync.prepare_backlog(10000, 0).unwrap().is_empty());
    for event in &e.events {
        let (rows, _) = sync
            .reserve(
                &[Descriptor {
                    identity: event.identity.clone(),
                    report_id: None,
                }],
                Source::Results,
                &event.identity.app_id,
            )
            .unwrap();
        assert!(rows.is_empty());
    }
    let after = sync
        .batch(|conn| {
            conn.query_row(
                "SELECT count(*) FROM work_transitions WHERE kind='ineligible'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map_err(sql_error)
        })
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(server.request_count(), 0);
}

#[test]
fn history_report_policy_exclusions_precede_due_limit_and_do_not_starve_eligible_examples() {
    let _isolation = crate::test_isolation();
    let (dir, server, tenant, mut e) = fixture();
    for n in 1..=250 {
        let mut item = event(&e, "app-a", n);
        item.report_ids.push(format!("report-{n}"));
        if n % 2 == 0 {
            item.verdict = "benign".into();
        } else {
            item.verdict = "malicious".into();
            item.timestamp_ms = None;
        }
        e.events.push(item);
    }
    let mut eligible = event(&e, "app-a", 251);
    eligible.verdict = "malicious".into();
    eligible.report_ids.push("eligible-example".into());
    e.events.push(eligible);
    let sync = start(&dir, &tenant, &e, vec![]);
    e.seal_native().unwrap();
    sync.checkpoint(&e).unwrap();
    for (n, item) in e.events.iter().enumerate() {
        let descriptor = Descriptor {
            identity: item.identity.clone(),
            report_id: item.report_ids.first().cloned(),
        };
        sync.batch(|conn| {
            let Admission::Due(attempt) = work::reserve(
                conn,
                &descriptor,
                Source::Reports,
                sync.references.get("app-a"),
                &sync.run,
                "synthetic-no-remote",
                n as i64 + 1,
            )?
            else {
                panic!("fixture reservation")
            };
            work::finish(conn, &attempt, Outcome::Missing, Some(200), n as i64 + 2)
        })
        .unwrap();
    }
    let selected = sync.prepare_backlog(10000, 5).unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0].descriptor.report_id.as_deref(),
        Some("eligible-example")
    );
    assert_eq!(server.request_count(), 0);
}

#[test]
fn history_cancelled_after_reservation_releases_proven_unsent_ordinal_before_snapshot() {
    let _isolation = crate::test_isolation();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (dir, server, tenant, mut e) = fixture();
        e.events.push(event(&e, "app-a", 1));
        let sync = start(&dir, &tenant, &e, vec![]);
        e.seal_native().unwrap();
        sync.checkpoint(&e).unwrap();
        let descriptor = Descriptor {
            identity: e.events[0].identity.clone(),
            report_id: None,
        };
        let (admitted, _) = sync
            .reserve(&[descriptor], Source::Results, "app-a")
            .unwrap();
        assert_eq!(admitted.len(), 1);
        let (_, binding) = tenant.config.scanner_for_application("app-a").unwrap();
        let (scanner, status) = airs_runtime_core::clients::build_scanner_bounded(
            &binding.overlay(&tenant.config),
            &sync.session,
            None,
            Some(&sync.budget),
        )
        .unwrap();
        let before = status.attempts("airs.scan.results");
        sync.session.cancel();
        let mut events = e.events.clone();
        collect::enrich_in_context(
            &scanner,
            &status,
            &sync.budget,
            &mut events,
            &sync.secrets,
            None,
        )
        .await;
        let sent = status.attempts("airs.scan.results") > before;
        assert!(!sent);
        assert!(
            sync.finish_transport_only(
                &admitted,
                sent,
                status.response("airs.scan.results"),
                Source::Results
            )
            .unwrap()
        );
        // The ordinary checkpoint still refuses cancellation, while the no-send proof
        // has already been committed without an HTTP request.
        assert!(sync.checkpoint(&e).is_err());
        sync.final_batch(|conn| {
            let state: (String, i64) = conn
                .query_row(
                    "SELECT state,attempts FROM sync_work WHERE kind='results'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(sql_error)?;
            assert_eq!(state, ("pending".into(), 0));
            assert_eq!(
                conn.query_row("SELECT outcome FROM work_attempts", [], |r| r
                    .get::<_, String>(0))
                    .map_err(sql_error)?,
                "not_sent"
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(server.request_count(), 0);
    });
}

#[test]
fn history_management_clock_admission_denial_retains_a_safe_explicit_diagnostic() {
    let _isolation = crate::test_isolation();
    let (dir, server, tenant, e) = fixture();
    let sync = start(&dir, &tenant, &e, vec![]);
    sync.batch(|conn|{
        conn.execute("INSERT INTO management_reads VALUES('future-clock',?1,'synthetic',NULL,NULL,?2,NULL,NULL,0,NULL,'started','{}')",params![sync.run,database::now_ms()?+60_000]).map_err(sql_error)?;
        Ok(())
    }).unwrap();
    assert!(
        airs_runtime_core::read_budget::ReadJournal::begin(
            &sync,
            "airs.management.dashboard.sessions_overview",
            &[]
        )
        .is_err()
    );
    let progress = sync.finish(&e).unwrap();
    assert!(
        progress["management_admission_notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().contains("Wall clock moved backwards"))
    );
    sync.final_batch(|conn| {
        assert_eq!(
            conn.query_row("SELECT count(*) FROM management_reads", [], |r| r
                .get::<_, i64>(0))
                .map_err(sql_error)?,
            1
        );
        let saved: String = conn
            .query_row(
                "SELECT provenance_json FROM collection_runs WHERE run_id=?1",
                [&sync.run],
                |r| r.get(0),
            )
            .map_err(sql_error)?;
        assert!(saved.contains("Wall clock moved backwards"));
        Ok(())
    })
    .unwrap();
    assert_eq!(server.request_count(), 0);
}
