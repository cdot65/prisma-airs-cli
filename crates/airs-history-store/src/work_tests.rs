use super::*;

fn fixture() -> (Connection, Descriptor, ScannerReference) {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(super::super::database::SCHEMA).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON; INSERT INTO collection_runs VALUES('run','sync',0,NULL,0,1,'running','{}')").unwrap();
    let descriptor = Descriptor {
        identity: Identity {
            app_id: "app-one".into(),
            app_name: "App One".into(),
            session_id: "session-one".into(),
            scan_id: "11111111-1111-4111-8111-111111111111".into(),
            sub_request: Some(0),
        },
        report_id: None,
    };
    let reference = ScannerReference {
        name: "scanner-one".into(),
        revision: "22222222-2222-4222-8222-222222222222".into(),
        application_id: "app-one".into(),
        endpoint: "https://service.example.invalid".into(),
    };
    assert!(reference.valid());
    (conn, descriptor, reference)
}
fn due(conn: &Connection, d: &Descriptor, r: &ScannerReference, now: i64) -> Attempt {
    let Admission::Due(a) = reserve(
        conn,
        d,
        if d.report_id.is_some() {
            Source::Reports
        } else {
            Source::Results
        },
        Some(r),
        "run",
        "transport",
        now,
    )
    .unwrap() else {
        panic!("expected reservation")
    };
    a
}
fn skipped(conn: &Connection, d: &Descriptor, r: &ScannerReference, now: i64) -> bool {
    matches!(
        reserve(
            conn,
            d,
            if d.report_id.is_some() {
                Source::Reports
            } else {
                Source::Results
            },
            Some(r),
            "run",
            "transport",
            now
        )
        .unwrap(),
        Admission::Skip(_)
    )
}

#[test]
fn conservative_reservations_spacing_exhaustion_and_explicit_epochs() {
    let _isolation = crate::test_isolation();
    let (mut conn, d, r) = fixture();
    // Started but never finished survives transaction commit and consumes attempt1.
    let tx = conn.transaction().unwrap();
    let a = due(&tx, &d, &r, 1_000);
    tx.commit().unwrap();
    assert_eq!(a.ordinal, 1);
    assert!(skipped(&conn, &d, &r, 60_999));
    let a = due(&conn, &d, &r, 61_000);
    assert_eq!(a.ordinal, 2);
    finish(&conn, &a, Outcome::Missing, Some(200), 61_001).unwrap();
    assert!(skipped(&conn, &d, &r, 960_999));
    let a = due(&conn, &d, &r, 961_000);
    assert_eq!(a.ordinal, 3);
    finish(&conn, &a, Outcome::Pending, Some(200), 961_001).unwrap();
    assert!(skipped(&conn, &d, &r, 10_000_000));
    assert_eq!(reset(&conn, Source::Results, 10_000_001).unwrap(), 1);
    assert_eq!(reset(&conn, Source::Results, 10_000_002).unwrap(), 0);
    let a = due(&conn, &d, &r, 10_000_003);
    assert_eq!((a.epoch, a.ordinal), (1, 1));
    assert_eq!(
        conn.query_row("SELECT count(*) FROM work_attempts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM work_transitions WHERE kind='retry_reset'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert!(skipped(&conn, &d, &r, 0));
}

#[test]
fn proven_not_sent_releases_only_its_ordinal_and_finish_is_once() {
    let _isolation = crate::test_isolation();
    let (conn, d, r) = fixture();
    let a = due(&conn, &d, &r, 100);
    finish(&conn, &a, Outcome::NotSent, None, 101).unwrap();
    assert!(finish(&conn, &a, Outcome::Returned, Some(200), 102).is_err());
    let b = due(&conn, &d, &r, 102);
    assert_eq!(b.ordinal, 1);
    finish(&conn, &b, Outcome::Missing, Some(200), 103).unwrap();
    let c = due(&conn, &d, &r, 60_102);
    assert_eq!(c.ordinal, 2);
    finish(&conn, &c, Outcome::NotSent, None, 60_103).unwrap();
    let d = due(&conn, &d, &r, 60_104);
    assert_eq!(d.ordinal, 2);
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM work_attempts WHERE outcome='not_sent'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
}

#[test]
fn authorization_is_app_revision_wide_rotation_is_scoped_and_reset_cannot_override() {
    let _isolation = crate::test_isolation();
    let (conn, d, mut r) = fixture();
    let a = due(&conn, &d, &r, 100);
    finish(&conn, &a, Outcome::AuthDenied, Some(403), 101).unwrap();
    let mut report = d.clone();
    report.report_id = Some("report-one".into());
    assert!(skipped(&conn, &report, &r, 1_000_000));
    assert_eq!(reset(&conn, Source::Results, 1_000_001).unwrap(), 0);
    assert!(skipped(&conn, &d, &r, 1_000_002));
    r.revision = "33333333-3333-4333-8333-333333333333".into();
    let a = due(&conn, &d, &r, 1_000_003);
    assert_eq!(a.ordinal, 1);
    finish(&conn, &a, Outcome::Missing, Some(200), 1_000_004).unwrap();
    r.endpoint = "https://rebound.example.invalid".into();
    r.revision = "44444444-4444-4444-8444-444444444444".into();
    assert!(skipped(&conn, &d, &r, 2_000_000));
    assert!(skipped(&conn, &report, &r, 2_000_000));
    assert_eq!(reset(&conn, Source::Results, 2_000_001).unwrap(), 0);
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM work_attempts WHERE outcome='auth_denied'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn service_cooldowns_cross_credentials_bound_duration_and_preserve_clock_uncertainty() {
    let _isolation = crate::test_isolation();
    let (conn, d, mut r) = fixture();
    cooldown(&conn, Source::Reports, 100, Some(u64::MAX)).unwrap();
    assert!(!cooling(&conn, Source::Results, 101).unwrap());
    assert!(cooling(&conn, Source::Reports, 0).unwrap());
    assert!(cooling(&conn, Source::Reports, 86_400_099).unwrap());
    assert!(!cooling(&conn, Source::Reports, 86_400_100).unwrap());
    let mut report = d.clone();
    report.report_id = Some("report-one".into());
    assert!(skipped(&conn, &report, &r, 101));
    r.revision = "33333333-3333-4333-8333-333333333333".into();
    assert!(skipped(&conn, &report, &r, 101));
    cooldown(&conn, Source::Results, 200, None).unwrap();
    assert!(skipped(&conn, &d, &r, 900_199));
    assert!(!skipped(&conn, &d, &r, 900_200));
    // Missing/rejected results alone do not create authorization evidence.
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM work_attempts WHERE outcome='auth_denied'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn missing_credentials_leave_work_durable_and_session_attempts_are_forbidden() {
    let _isolation = crate::test_isolation();
    let (conn, d, r) = fixture();
    assert!(matches!(
        reserve(&conn, &d, Source::Results, None, "run", "transport", 100).unwrap(),
        Admission::Skip(_)
    ));
    assert_eq!(
        conn.query_row("SELECT count(*) FROM work_attempts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let a = due(&conn, &d, &r, 101);
    finish(&conn, &a, Outcome::Returned, Some(200), 102).unwrap();
    assert!(skipped(&conn, &d, &r, 10_000_000));
    conn.execute("INSERT INTO sync_work VALUES('session','session','{}','pending',NULL,0,0,NULL,NULL,NULL,'{}')",[]).unwrap();
    assert!(conn.execute("INSERT INTO work_attempts VALUES('bad','session','results','run','transport','revision',0,1,0,NULL,NULL,'started','{}')",[]).is_err());
    assert!(
        conn.execute(
            "UPDATE sync_work SET attempts=1 WHERE work_id='session'",
            []
        )
        .is_err()
    );
}

#[test]
fn unsent_local_deferral_is_scoped_to_evaluated_revision_and_explicitly_resettable() {
    let _isolation = crate::test_isolation();
    for initial in [false, true] {
        let (conn, d, mut r) = fixture();
        let id = d.id(Source::Results).unwrap();
        if initial {
            let a = due(&conn, &d, &r, 1);
            finish(&conn, &a, Outcome::NotSent, None, 2).unwrap();
        } else {
            conn.execute("INSERT INTO sync_work VALUES(?1,'results',?2,'pending',NULL,0,0,NULL,NULL,NULL,'{}')",params![id,encode(&d).unwrap()]).unwrap();
        }
        r.revision = "33333333-3333-4333-8333-333333333333".into();
        defer_local(&conn, &id, 3, &r, LocalDeferral::BoundedView).unwrap();
        assert!(skipped(&conn, &d, &r, 4));
        let revision: String = conn
            .query_row(
                "SELECT binding_revision FROM sync_work WHERE work_id=?1",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(revision, r.revision);
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM work_attempts WHERE outcome!='not_sent'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(reset(&conn, Source::Results, 5).unwrap(), 1);
        let a = due(&conn, &d, &r, 6);
        assert_eq!((a.epoch, a.ordinal), (1, 1));
    }
}
