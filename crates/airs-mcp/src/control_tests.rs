use super::*;
use crate::jobs::{State, Trigger};
use std::os::unix::fs::DirBuilderExt;

const TENANT: &str = "svc-test";
const EPOCH: &str = "0a1b2c3d";
const KEY: [u8; 32] = [9u8; 32];

struct Dir(std::path::PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn dir() -> Dir {
    let d = std::env::temp_dir().join(format!("airs-mcp-control-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new().mode(0o700).create(&d).unwrap();
    Dir(d)
}

fn open(d: &Dir) -> Control {
    Control::open(&d.0, TENANT, EPOCH, &KEY, 1_000).unwrap()
}

fn principal(client: &str) -> Actor {
    Actor::Principal {
        sub: "human-a".into(),
        client_id: client.into(),
    }
}

fn submission<'a>(actor: Actor, key: &'a str, params: &'a Value) -> Submission<'a> {
    Submission {
        tenant: TENANT,
        operation: "collection_start",
        actor,
        idempotency_key: key,
        expected_epoch: EPOCH,
        params,
        config_revision: "test-1",
        credential_revision: Some("00000000-0000-4000-8000-0000000000e1"),
        deadline_ms: 1_000 + 86_400_000,
        queue_tenant: DEFAULT_QUEUE_TENANT,
        queue_principal: DEFAULT_QUEUE_PRINCIPAL,
        join_equivalent: false,
        scheduled_slot: None,
    }
}

/// Crash point C1: a lost response is recovered by repeating the submission. The same
/// key and parameters return the same job (also after reopening); changed parameters
/// are `idempotency_conflict`; a key from another epoch is `control_restored`; nothing
/// in the refused cases creates a job.
#[test]
fn c1_submission_is_idempotent_across_reopen_and_refuses_changed_or_stale_keys() {
    let d = dir();
    let params = json!({"range": {"since": "a", "until": "b"}, "apps": ["x"]});
    let reordered = json!({"apps": ["x"], "range": {"until": "b", "since": "a"}});
    let first = {
        let mut c = open(&d);
        let s = c
            .submit(
                submission(principal("cli"), "key-0123456789abcdef", &params),
                2_000,
            )
            .unwrap();
        assert!(!s.existing);
        assert!(s.job_id.starts_with(&format!("j1_{EPOCH}_")));
        assert!(airs_runtime_core::dto::is_job_id(&s.job_id));
        s
    };
    let mut c = open(&d);
    let again = c
        .submit(
            submission(principal("cli"), "key-0123456789abcdef", &reordered),
            3_000,
        )
        .unwrap();
    assert_eq!(
        (again.job_id.as_str(), again.existing),
        (first.job_id.as_str(), true)
    );
    let changed = json!({"range": {"since": "a", "until": "c"}, "apps": ["x"]});
    let err = c
        .submit(
            submission(principal("cli"), "key-0123456789abcdef", &changed),
            3_000,
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::IdempotencyConflict);
    let mut stale = submission(principal("cli"), "key-fedcba9876543210", &params);
    stale.expected_epoch = "ffffffff";
    assert_eq!(
        c.submit(stale, 3_000).unwrap_err().code,
        ErrorCode::ControlRestored
    );
    // The same key under another principal is a different scope.
    let other = c
        .submit(
            submission(principal("agent"), "key-0123456789abcdef", &params),
            3_000,
        )
        .unwrap();
    assert_ne!(other.job_id, first.job_id);
    let jobs: i64 = c
        .conn
        .query_row("SELECT count(*) FROM jobs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(jobs, 2, "refused submissions created nothing");
    let job = c.job(TENANT, &first.job_id).unwrap().unwrap();
    assert_eq!(job.state, State::Queued);
    assert_eq!(job.params_digest, canonical(&params).1);
    assert_eq!(
        c.job("svc-other", &first.job_id).unwrap(),
        None,
        "tenant-scoped"
    );
}

/// Contract §6: 8 live jobs per principal and 64 per tenant; beyond is `overloaded`
/// with a retry hint, and terminal jobs do not count.
#[test]
fn queue_bounds_refuse_with_overloaded_and_terminal_jobs_free_capacity() {
    let d = dir();
    let mut c = open(&d);
    let params = json!({});
    let mut ids = Vec::new();
    for i in 0..DEFAULT_QUEUE_PRINCIPAL {
        let key = format!("key-principal-{i:04}");
        ids.push(
            c.submit(submission(principal("cli"), &key, &params), 2_000)
                .unwrap()
                .job_id,
        );
    }
    let err = c
        .submit(
            submission(principal("cli"), "key-principal-over", &params),
            2_000,
        )
        .unwrap_err();
    assert_eq!(
        (err.code, err.retry_after_seconds),
        (ErrorCode::Overloaded, Some(OVERLOADED_RETRY_SECONDS))
    );
    c.cancel(TENANT, &ids[0], 2_100).unwrap();
    c.apply(
        TENANT,
        &ids[0],
        Trigger::CancelCheckpoint,
        "checkpoint",
        2_200,
    )
    .unwrap();
    assert!(
        c.submit(
            submission(principal("cli"), "key-principal-over", &params),
            2_300
        )
        .is_ok()
    );
    let mut tenant_cap = submission(principal("other"), "key-tenant-capped", &params);
    tenant_cap.queue_tenant = 8; // 8 live jobs: the tenant is full for any principal
    assert_eq!(
        c.submit(tenant_cap, 2_400).unwrap_err().code,
        ErrorCode::Overloaded
    );
}

/// Crash point C2: a reservation is committed before the send; after a restart every
/// `reserved` row is `uncertain` and stays consumed. Settled rows never change again,
/// and only a proven no-send releases its allowance.
#[test]
fn c2_reservations_survive_restart_as_consumed_uncertain_sends() {
    let d = dir();
    let job = {
        let mut c = open(&d);
        let job = c
            .submit(
                submission(principal("cli"), "key-reservations-01", &json!({})),
                2_000,
            )
            .unwrap()
            .job_id;
        c.claim_next(TENANT, "owner-1", 2_100, |_| true)
            .unwrap()
            .unwrap();
        let sent = c
            .reserve_send(&job, "sessions", "management", None, 2_200)
            .unwrap();
        c.settle_send(&sent, SendOutcome::Sent, 2_300).unwrap();
        let refused = c
            .reserve_send(&job, "sessions", "management", None, 2_400)
            .unwrap();
        c.settle_send(&refused, SendOutcome::NotSent, 2_500)
            .unwrap();
        // The process dies here, with one reservation still `reserved`.
        c.reserve_send(&job, "results", "scanner", None, 2_600)
            .unwrap();
        assert_eq!(
            c.settle_send(&sent, SendOutcome::NotSent, 2_700)
                .unwrap_err()
                .code,
            ErrorCode::Conflict,
            "a sent request can never be released"
        );
        job
    };
    let mut c = open(&d);
    let recovered = c.recover(3_000).unwrap();
    assert_eq!(
        recovered,
        Recovery {
            uncertain: 1,
            requeued: 1
        }
    );
    assert_eq!(
        c.consumed_sends(&job).unwrap(),
        2,
        "sent + uncertain; not_sent released"
    );
    let states: Vec<String> = {
        let mut s = c
            .conn
            .prepare("SELECT state FROM send_reservations ORDER BY reserved_ms")
            .unwrap();
        s.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(states, ["sent", "not_sent", "uncertain"]);
    assert_eq!(
        c.recover(4_000).unwrap(),
        Recovery::default(),
        "recovery is idempotent"
    );
    // Contract §10 has no recovery edge: the job stays running, unclaimed.
    assert_eq!(c.job(TENANT, &job).unwrap().unwrap().state, State::Running);
    let claimed: Option<String> = c
        .conn
        .query_row("SELECT claimed_by FROM jobs WHERE job_id=?1", [&job], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(claimed, None);
}

/// A restart never loses a running job: its claim is released without a state change
/// (contract §10 has no recovery edge), the next owner resumes it from its last
/// checkpoint, including the archive operation ID crash point C4 looks up, and every
/// recorded event is a §10 edge.
#[test]
fn running_jobs_resume_from_their_checkpoint_after_restart() {
    let d = dir();
    let job = {
        let mut c = open(&d);
        let job = c
            .submit(
                submission(principal("cli"), "key-checkpoint-0001", &json!({})),
                2_000,
            )
            .unwrap()
            .job_id;
        c.claim_next(TENANT, "owner-1", 2_100, |_| true)
            .unwrap()
            .unwrap();
        c.checkpoint(&job, "sessions", &json!({"page": 3}), Some("op-1"), 2_200)
            .unwrap();
        job
    };
    let mut c = open(&d);
    assert_eq!(c.recover(3_000).unwrap().requeued, 1);
    let claimed = c
        .claim_next(TENANT, "owner-2", 3_100, |_| true)
        .unwrap()
        .unwrap();
    assert_eq!(claimed.job_id, job);
    let (phase, state, op) = c.last_checkpoint(&job).unwrap().unwrap();
    assert_eq!(
        (phase.as_str(), state, op.as_deref()),
        ("sessions", json!({"page": 3}), Some("op-1"))
    );
    let events: Vec<(Option<String>, String)> = {
        let mut s = c
            .conn
            .prepare("SELECT from_state,to_state FROM job_events WHERE job_id=?1 ORDER BY sequence")
            .unwrap();
        s.query_map([&job], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(
        events,
        [
            (None, "queued".to_owned()),
            (Some("queued".to_owned()), "running".to_owned())
        ],
        "no recovery edge is written"
    );
    assert_events_are_contract_edges(&c);
}

/// Cancellation is explicit and completes only at a checkpoint; repeating it is
/// harmless; a terminal job is unchanged; illegal edges are `conflict`.
#[test]
fn cancellation_is_explicit_idempotent_and_checkpointed() {
    let d = dir();
    let mut c = open(&d);
    let job = c
        .submit(
            submission(principal("cli"), "key-cancel-000001", &json!({})),
            2_000,
        )
        .unwrap()
        .job_id;
    c.claim_next(TENANT, "owner-1", 2_100, |_| true)
        .unwrap()
        .unwrap();
    assert_eq!(
        c.cancel(TENANT, &job, 2_200).unwrap(),
        State::CancelRequested
    );
    assert_eq!(
        c.cancel(TENANT, &job, 2_300).unwrap(),
        State::CancelRequested
    );
    assert_eq!(
        c.apply(TENANT, &job, Trigger::Succeed, "late success", 2_400)
            .unwrap_err()
            .code,
        ErrorCode::Conflict,
        "a cancel-requested job cannot finish successfully"
    );
    assert_eq!(
        c.apply(TENANT, &job, Trigger::CancelCheckpoint, "checkpoint", 2_500)
            .unwrap(),
        State::Canceled
    );
    assert_eq!(c.cancel(TENANT, &job, 2_600).unwrap(), State::Canceled);
    assert_eq!(
        c.cancel(
            TENANT,
            "j1_0a1b2c3d_00000000-0000-4000-8000-000000000000",
            2_700
        )
        .unwrap_err()
        .code,
        ErrorCode::NotFound
    );
}

/// Fair dispatch: a principal with a recent claim waits behind one that has none, so a
/// busy submitter cannot starve another. A submitter whose grant no longer admits the
/// job is blocked by policy, and a job past its deadline fails instead of running.
#[test]
fn claims_are_fair_recheck_grants_and_respect_deadlines() {
    let d = dir();
    let mut c = open(&d);
    let p = json!({});
    let a1 = c
        .submit(
            submission(principal("busy"), "key-busy-000000001", &p),
            2_000,
        )
        .unwrap()
        .job_id;
    let a2 = c
        .submit(
            submission(principal("busy"), "key-busy-000000002", &p),
            2_001,
        )
        .unwrap()
        .job_id;
    let b1 = c
        .submit(
            submission(principal("quiet"), "key-quiet-00000001", &p),
            2_002,
        )
        .unwrap()
        .job_id;
    assert_eq!(
        c.claim_next(TENANT, "o", 3_000, |_| true)
            .unwrap()
            .unwrap()
            .job_id,
        a1
    );
    assert_eq!(
        c.claim_next(TENANT, "o", 3_001, |_| true)
            .unwrap()
            .unwrap()
            .job_id,
        b1,
        "the quiet principal goes before the busy one's second job"
    );
    assert_eq!(
        c.claim_next(TENANT, "o", 3_002, |_| true)
            .unwrap()
            .unwrap()
            .job_id,
        a2
    );
    let revoked = c
        .submit(
            submission(principal("revoked"), "key-revoked-000001", &p),
            4_000,
        )
        .unwrap()
        .job_id;
    assert_eq!(c.claim_next(TENANT, "o", 4_100, |_| false).unwrap(), None);
    assert_eq!(
        c.job(TENANT, &revoked).unwrap().unwrap().state,
        State::BlockedPolicy
    );
    let mut late = submission(principal("late"), "key-late-000000001", &p);
    late.deadline_ms = 5_000;
    let late = c.submit(late, 4_500).unwrap().job_id;
    assert_eq!(c.claim_next(TENANT, "o", 6_000, |_| true).unwrap(), None);
    assert_eq!(c.job(TENANT, &late).unwrap().unwrap().state, State::Failed);
    assert_eq!(
        c.claim_next(TENANT, "o", 6_100, |_| true).unwrap(),
        None,
        "queue drained"
    );
}

/// The store is never recreated or repaired to make startup succeed: another tenant's
/// store or a damaged file is refused and left byte-for-byte unchanged (C12).
#[test]
fn foreign_or_damaged_control_stores_are_refused_and_left_intact() {
    let d = dir();
    drop(open(&d));
    let path = d.0.join(FILE);
    let before = std::fs::read(&path).unwrap();
    let err = Control::open(&d.0, "svc-other", EPOCH, &KEY, 2_000)
        .err()
        .unwrap();
    assert_eq!(err.code, ErrorCode::MaintenanceRequired);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let mut damaged = before.clone();
    for byte in damaged.iter_mut().skip(4096).take(4096) {
        *byte ^= 0xa5;
    }
    std::fs::write(&path, &damaged).unwrap();
    assert!(Control::open(&d.0, TENANT, EPOCH, &KEY, 2_000).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), damaged, "never rewritten");
}

/// Contract §6, §8: two sends / ten IDs per Chicago day by default, charged durably,
/// never refunded or reset by reopening the store; a clock rollback cannot reopen an
/// earlier day; the refusal names the wait until the next admission day.
#[test]
fn reports_allowances_are_durable_and_cannot_be_reopened_by_rollback() {
    let d = dir();
    // 2026-11-06T12:00:00Z is 06:00 in Chicago (CST), on 2026-11-06.
    let noon = 1_793_966_400_000;
    {
        let mut c = open(&d);
        assert_eq!(
            c.charge_reports(TENANT, noon, (1, 5), (2, 10)).unwrap(),
            "2026-11-06"
        );
        assert_eq!(
            c.charge_reports(TENANT, noon + 1_000, (1, 5), (2, 10))
                .unwrap(),
            "2026-11-06"
        );
    }
    let mut c = open(&d);
    let err = c
        .charge_reports(TENANT, noon + 2_000, (1, 1), (2, 10))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::QuotaExhausted);
    // Next Chicago midnight is 2026-11-07T06:00:00Z, 18 h after noon UTC.
    assert_eq!(err.retry_after_seconds, Some(18 * 3600 - 2));
    // A clock rolled back a day still lands on the high-water day, which is spent.
    let rolled = c
        .charge_reports(TENANT, noon - 86_400_000, (1, 1), (2, 10))
        .unwrap_err();
    assert_eq!(rolled.code, ErrorCode::QuotaExhausted);
    // The next Chicago day opens a fresh allowance.
    assert_eq!(
        c.charge_reports(TENANT, noon + 18 * 3_600_000, (1, 1), (2, 10))
            .unwrap(),
        "2026-11-07"
    );
    // ID caps bind independently of sends.
    assert_eq!(
        c.charge_reports(TENANT, noon + 18 * 3_600_000 + 1, (0, 10), (2, 10))
            .unwrap_err()
            .code,
        ErrorCode::QuotaExhausted
    );
}

const CRASH_DIR: &str = "AIRS_MCP_CONTROL_CRASH_DIR";

/// Child half of the crash tests: never runs unless the parent sets `CRASH_DIR`.
#[test]
fn crash_child() {
    let Some(dir) = std::env::var_os(CRASH_DIR) else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let mut c = Control::open(&dir, TENANT, EPOCH, &KEY, 1_000).unwrap();
    match std::env::var("AIRS_MCP_CONTROL_CRASH_POINT").as_deref() {
        // C1: the process dies inside the submission transaction, after its writes.
        Ok("c1") => {
            let tx = c
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            tx.execute(
                "INSERT INTO jobs (job_id,epoch,tenant,operation,submitter_sub,submitter_client_id,service_actor,state,submitted_ms,deadline_ms,retry_after_ms,config_revision,credential_revision,scheduled_slot,claimed_by,result_json,updated_ms) VALUES ('j1_0a1b2c3d_00000000-0000-4000-8000-0000000000c1',?1,?2,'collection_start','human-a','cli',NULL,'queued',2000,9000,NULL,'test-1',NULL,NULL,NULL,NULL,2000)",
                params![EPOCH, TENANT],
            )
            .unwrap();
            tx.execute_batch("PRAGMA cache_size=1; CREATE TABLE crash_padding(x); INSERT INTO crash_padding SELECT randomblob(4096) FROM (WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<64) SELECT i FROM n);")
                .unwrap();
            std::process::exit(86);
        }
        // C1 through the real submission: the crash point inside `submit()` ends the
        // process after every write of the transaction and before its commit. A tiny
        // page cache forces the writes to spill, so a hot journal is left behind.
        Ok("submit") => {
            c.conn.execute_batch("PRAGMA cache_size=1;").unwrap();
            let _ = c.submit(
                submission(principal("cli"), "key-crash-submit-01", &json!({"n": 1})),
                2_000,
            );
            unreachable!("the crash point ends the process");
        }
        // The same inside the join path: the first submission commits, the joining one
        // dies before its commit.
        Ok("submit-join") => {
            let p = json!({"mode": "recent"});
            let mut first = submission(principal("cli"), "key-crash-join-0001", &p);
            first.join_equivalent = true;
            c.submit(first, 2_000).unwrap();
            c.conn.execute_batch("PRAGMA cache_size=1;").unwrap();
            let mut second = submission(principal("cli"), "key-crash-join-0002", &p);
            second.join_equivalent = true;
            let _ = c.submit(second, 2_100);
            unreachable!("the crash point ends the process");
        }
        // C2: the reservation commits, then the process dies before settling it.
        Ok("c2") => {
            let job = c
                .submit(
                    submission(principal("cli"), "key-crash-c2-000001", &json!({})),
                    2_000,
                )
                .unwrap()
                .job_id;
            c.claim_next(TENANT, "owner-crashed", 2_100, |_| true)
                .unwrap()
                .unwrap();
            c.reserve_send(&job, "sessions", "management", None, 2_200)
                .unwrap();
            std::process::exit(87);
        }
        _ => {}
    }
}

fn crash(d: &Dir, point: &str) -> i32 {
    std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "control::tests::crash_child", "--nocapture"])
        .env(CRASH_DIR, &d.0)
        .env("AIRS_MCP_CONTROL_CRASH_POINT", point)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .code()
        .unwrap_or(-1)
}

/// Crash point C1 with a real process death: an uncommitted submission leaves a hot
/// journal; the next open rolls it back, so no partial job, parameter or key exists.
#[test]
fn c1_a_process_dying_inside_a_submission_leaves_nothing() {
    let d = dir();
    drop(open(&d));
    assert_eq!(crash(&d, "c1"), 86);
    assert!(
        d.0.join("control.sqlite3-journal").exists(),
        "the dead process left a hot journal for the next open to roll back"
    );
    let c = open(&d);
    for table in ["jobs", "job_params", "idempotency", "job_events"] {
        let n: i64 = c
            .conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "{table}");
    }
    let padding: Option<String> = c
        .conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE name='crash_padding'",
            [],
            |r| r.get(0),
        )
        .optional()
        .unwrap();
    assert_eq!(
        padding, None,
        "the uncommitted schema change was rolled back"
    );
}

/// Crash point C2 with a real process death: the committed reservation of a process
/// that died before sending (or after; it cannot tell) is consumed as `uncertain`, and
/// the running job is requeued.
#[test]
fn c2_a_process_dying_after_a_reservation_leaves_it_consumed() {
    let d = dir();
    drop(open(&d));
    assert_eq!(crash(&d, "c2"), 87);
    let mut c = open(&d);
    assert_eq!(
        c.recover(3_000).unwrap(),
        Recovery {
            uncertain: 1,
            requeued: 1
        }
    );
    let job: String = c
        .conn
        .query_row("SELECT job_id FROM jobs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(c.consumed_sends(&job).unwrap(), 1);
}

/// Contract §6: an equivalent live collection joins the recorded job, but only the same
/// submitter's (so the joiner can see and cancel it) and only one that will still run;
/// another principal, a cancelling job and a finished job each get a job of their own.
#[test]
fn equivalent_live_collections_join_only_the_submitters_executable_job() {
    let d = dir();
    let mut c = open(&d);
    let p = json!({"mode": "recent"});
    let join = |key: &'static str, who: &'static str| {
        let mut s = submission(principal(who), key, &p);
        s.join_equivalent = true;
        s
    };
    let first = c.submit(join("key-join-0000000001", "cli"), 2_000).unwrap();
    let joined = c.submit(join("key-join-0000000002", "cli"), 2_100).unwrap();
    assert_eq!(
        (joined.job_id.as_str(), joined.joined),
        (first.job_id.as_str(), true)
    );
    let repeat = c.submit(join("key-join-0000000002", "cli"), 2_200).unwrap();
    assert_eq!(
        (repeat.job_id.as_str(), repeat.existing),
        (first.job_id.as_str(), true)
    );
    // Another principal never receives the first principal's job.
    let other = c
        .submit(join("key-join-0000000003", "agent"), 2_250)
        .unwrap();
    assert!(
        !other.joined && other.job_id != first.job_id,
        "no cross-principal join"
    );
    // A job already cancelling is never joined.
    c.cancel(TENANT, &first.job_id, 2_300).unwrap();
    let during = c.submit(join("key-join-0000000004", "cli"), 2_350).unwrap();
    assert!(
        !during.joined && during.job_id != first.job_id,
        "no join of a cancelling job"
    );
    c.apply(
        TENANT,
        &first.job_id,
        Trigger::CancelCheckpoint,
        "checkpoint",
        2_400,
    )
    .unwrap();
    // And a terminal job is never joined.
    let later = c
        .submit(join("key-join-0000000005", "agent"), 2_500)
        .unwrap();
    assert_eq!(
        later.job_id, other.job_id,
        "the agent joins its own live job"
    );
}

/// Crash point C9: a backup set is consistent and complete only once its manifest is
/// published; a set without a manifest is never restorable, and a tampered copy is
/// refused against its manifest.
#[test]
fn c9_backup_sets_are_complete_only_with_a_matching_manifest() {
    let d = dir();
    let mut c = open(&d);
    c.submit(
        submission(principal("cli"), "key-backup-0000001", &json!({})),
        2_000,
    )
    .unwrap();
    let set = d.0.join("backup-1");
    let manifest = c.backup_into(&set, 3_000).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(set.join(FILE))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o600,
            "the backup copy is owner-only whatever the umask"
        );
    }
    assert_eq!(
        (manifest.tenant.as_str(), manifest.epoch.as_str()),
        (TENANT, EPOCH)
    );
    assert_eq!(
        manifest.sha256,
        hex(&Sha256::digest(std::fs::read(set.join(FILE)).unwrap()))
    );
    // An incomplete set (no manifest) is refused.
    let partial = d.0.join("backup-partial");
    std::fs::create_dir(&partial).unwrap();
    std::fs::copy(set.join(FILE), partial.join(FILE)).unwrap();
    let target = dir();
    assert_eq!(
        restore(&partial, &target.0, TENANT, 4_000)
            .unwrap_err()
            .reason,
        "the backup set is incomplete"
    );
    // A tampered copy no longer matches its manifest.
    let tampered = d.0.join("backup-tampered");
    std::fs::create_dir(&tampered).unwrap();
    std::fs::copy(set.join(MANIFEST), tampered.join(MANIFEST)).unwrap();
    let mut bytes = std::fs::read(set.join(FILE)).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    std::fs::write(tampered.join(FILE), bytes).unwrap();
    assert!(restore(&tampered, &target.0, TENANT, 4_000).is_err());
    // Another service's set is refused.
    assert!(restore(&set, &target.0, "svc-other", 4_000).is_err());
    assert!(
        !target.0.join(FILE).exists(),
        "refused restores place nothing"
    );
}

/// Crash points C10/C11: a restored control store opens with no active epoch, so every
/// submission is `control_restored` until an Administrator activates the new epoch;
/// old keys stay `control_restored` afterwards; restored jobs stay inactive; open
/// reservations stay consumed; cooldowns hold at least 24 hours; an existing store is
/// never overwritten.
#[test]
fn c11_a_restored_store_waits_for_activation_and_keeps_old_work_inactive() {
    let d = dir();
    let set = d.0.join("backup-1");
    let (queued, running) = {
        let mut c = open(&d);
        let queued = c
            .submit(
                submission(principal("cli"), "key-restore-000001", &json!({"n": 1})),
                2_000,
            )
            .unwrap()
            .job_id;
        let running = c
            .submit(
                submission(principal("cli"), "key-restore-000002", &json!({"n": 2})),
                2_001,
            )
            .unwrap()
            .job_id;
        // Claim the oldest (queued), so claim again to get the second one running.
        let first = c
            .claim_next(TENANT, "owner-1", 2_100, |_| true)
            .unwrap()
            .unwrap();
        assert_eq!(first.job_id, queued);
        c.reserve_send(&queued, "sessions", "management", None, 2_200)
            .unwrap();
        c.conn
            .execute(
                "INSERT INTO cooldowns VALUES (?1,'reports','',?2,'429')",
                params![TENANT, 2_500],
            )
            .unwrap();
        c.backup_into(&set, 3_000).unwrap();
        (queued, running)
    };
    let target = dir();
    let now = 10_000;
    let epoch = restore(&set, &target.0, TENANT, now).unwrap();
    assert_ne!(epoch, EPOCH);
    assert_eq!(
        restore(&set, &target.0, TENANT, now).unwrap_err().code,
        ErrorCode::Conflict,
        "an existing control store is never overwritten"
    );
    let mut c = Control::open(&target.0, TENANT, "ffffffff", &[0u8; 32], now).unwrap();
    assert_eq!(
        c.active_epoch().unwrap_err().code,
        ErrorCode::ControlRestored
    );
    assert_eq!(c.pending_epoch().unwrap(), Some(epoch.clone()));
    let refused = c
        .submit(
            submission(principal("cli"), "key-restore-000003", &json!({})),
            now,
        )
        .unwrap_err();
    assert_eq!(
        refused.code,
        ErrorCode::ControlRestored,
        "nothing runs before activation"
    );
    // Job states are kept (no restore edge); the quarantine keeps them from running.
    assert_eq!(
        c.job(TENANT, &queued).unwrap().unwrap().state,
        State::Running
    );
    assert_eq!(
        c.job(TENANT, &running).unwrap().unwrap().state,
        State::Queued
    );
    assert_eq!(c.quarantine().unwrap().as_deref(), Some("restored"));
    assert_events_are_contract_edges(&c);
    // The backed-up reservation stays consumed, and every restored non-terminal job is
    // marked exhausted: what it may have sent after the backup is unknown.
    let exhausted: i64 = c
        .conn
        .query_row("SELECT count(*) FROM exhausted_jobs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(exhausted, 2);
    assert_eq!(
        c.consumed_sends(&queued).unwrap(),
        1,
        "reservations stay consumed"
    );
    let (planes, until): (i64, i64) = c
        .conn
        .query_row(
            "SELECT count(DISTINCT plane), min(until_ms) FROM cooldowns",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        planes, 4,
        "every plane holds, not only those the backup recorded"
    );
    assert!(until >= now + 86_400_000, "cooldowns hold at least 24 h");
    assert_eq!(
        c.claim_next(TENANT, "owner-2", now, |_| true).unwrap(),
        None
    );
    // Explicit activation: new keys work; keys minted under the old epoch do not.
    c.activate_epoch(&epoch, now + 1).unwrap();
    assert_eq!(c.active_epoch().unwrap().0, epoch);
    let one = json!({"n": 1});
    let mut old = submission(principal("cli"), "key-restore-000001", &one);
    old.expected_epoch = EPOCH;
    assert_eq!(
        c.submit(old, now + 2).unwrap_err().code,
        ErrorCode::ControlRestored
    );
    let empty = json!({});
    let mut fresh = submission(principal("cli"), "key-restore-000004", &empty);
    fresh.expected_epoch = &epoch;
    assert!(fresh.idempotency_key.len() >= 16);
    let fresh = c.submit(fresh, now + 3).unwrap();
    assert!(fresh.job_id.starts_with(&format!("j1_{epoch}_")));
    assert_eq!(
        c.activate_epoch(&epoch, now + 4).unwrap_err().code,
        ErrorCode::Conflict
    );
    // Activation does not end the quarantine: nothing is claimed until reconciled.
    assert_eq!(
        c.claim_next(TENANT, "owner-2", now + 5, |_| true).unwrap(),
        None
    );
    assert!(c.reconcile().unwrap());
    // Reconciled, the restored jobs still never run: each fails at its next claim with
    // the recovery gap, and only work submitted under the new epoch runs.
    let nine = json!({"n": 9});
    let mut new = submission(principal("cli"), "key-restore-000009", &nine);
    new.expected_epoch = &epoch;
    let new = c.submit(new, now + 6).unwrap().job_id;
    let claimed = claim_some(&mut c, now + 7).expect("new work runs");
    assert!(
        claimed.starts_with(&format!("j1_{epoch}_")),
        "only work under the new epoch runs"
    );
    let _ = new;
    for job in [&queued, &running] {
        let j = c.job(TENANT, job).unwrap().unwrap();
        assert_eq!(j.state, State::Failed, "{job}");
        assert_eq!(
            j.gaps,
            ["recovery left this job's consumption unknown; its budget is spent"]
        );
    }
    assert_events_are_contract_edges(&c);
}

/// Claim until a job is returned (claims that fail an exhausted or expired job return
/// none), at most a few times.
fn claim_some(c: &mut Control, now: i64) -> Option<String> {
    (0..5).find_map(|n| {
        c.claim_next(TENANT, "owner-2", now + n, |_| true)
            .unwrap()
            .map(|j| j.job_id)
    })
}

/// Every recorded job event is an edge of the frozen §10 table (or the initial
/// `queued`), whatever path wrote it.
fn assert_events_are_contract_edges(c: &Control) {
    use crate::jobs::{Trigger as T, transition};
    let triggers = [
        T::Claim,
        T::TransientFailure,
        T::CredentialDenied,
        T::PolicyBlock,
        T::CancelRequest,
        T::CancelCheckpoint,
        T::Succeed,
        T::PartialResult,
        T::Fail,
        T::CredentialsRestored,
        T::RetryDue,
        T::PolicyCleared,
        T::DeadlinePassed,
    ];
    let mut stmt = c
        .conn
        .prepare("SELECT from_state, to_state FROM job_events")
        .unwrap();
    let rows: Vec<(Option<String>, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for (from, to) in rows {
        match from {
            None => assert_eq!(to, "queued"),
            Some(from) => {
                let (f, t) = (State::parse(&from).unwrap(), State::parse(&to).unwrap());
                assert!(
                    triggers.iter().any(|tr| transition(f, *tr) == Some(t)),
                    "{from} -> {to} is not a contract edge"
                );
            }
        }
    }
}

fn sha(path: &std::path::Path) -> String {
    hex(&Sha256::digest(std::fs::read(path).unwrap()))
}

/// M3 review 1 finding 1 (C11): losing the database of a provisioned directory is a
/// loss, never a first start. The seed's epoch is not reused: the new store waits for
/// explicit activation, quarantined, and a key minted before the loss is
/// `control_restored` and creates nothing.
#[test]
fn a_lost_database_never_reuses_the_seed_epoch() {
    let d = dir();
    {
        let mut c = open(&d);
        c.submit(
            submission(principal("cli"), "key-lost-0000000001", &json!({})),
            2_000,
        )
        .unwrap();
    }
    std::fs::remove_file(d.0.join(FILE)).unwrap();
    let mut c = open(&d);
    assert_eq!(
        c.active_epoch().unwrap_err().code,
        ErrorCode::ControlRestored
    );
    let pending = c.pending_epoch().unwrap().expect("a fresh pending epoch");
    assert_ne!(pending, EPOCH, "the seed epoch is never reused after loss");
    assert_eq!(c.quarantine().unwrap().as_deref(), Some("lost"));
    let retry = c.submit(
        submission(principal("cli"), "key-lost-0000000001", &json!({})),
        3_000,
    );
    assert_eq!(retry.unwrap_err().code, ErrorCode::ControlRestored);
    c.activate_epoch(&pending, 3_100).unwrap();
    let stale = c.submit(
        submission(principal("cli"), "key-lost-0000000001", &json!({})),
        3_200,
    );
    assert_eq!(
        stale.unwrap_err().code,
        ErrorCode::ControlRestored,
        "old keys stay refused"
    );
    let jobs: i64 = c
        .conn
        .query_row("SELECT count(*) FROM jobs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(jobs, 0, "nothing was created under the old epoch");
    // Unknown consumption is spent: the day's Reports allowance is exhausted.
    let charged = c.charge_reports(TENANT, 3_300, (1, 1), (2, 10));
    assert_eq!(charged.unwrap_err().code, ErrorCode::QuotaExhausted);
}

/// M3 review 1 finding 2 (the reviewer's probe): back up, spend the day's allowance,
/// restore and activate. The restored store must not grant the allowance again, must
/// hold every plane at least 24 h, disable schedules, and run nothing until reconciled.
#[test]
fn a_restore_never_resets_spent_allowances_or_reopens_schedules() {
    let d = dir();
    let set = d.0.join("backup-before-spend");
    let now = 1_793_000_000_000; // a fixed instant
    {
        let mut c = open(&d);
        c.conn
            .execute(
                "INSERT INTO schedules VALUES (?1,'collection',1,'{}',0)",
                [TENANT],
            )
            .unwrap();
        c.backup_into(&set, now).unwrap();
        c.charge_reports(TENANT, now + 1, (2, 10), (2, 10)).unwrap();
    }
    let target = dir();
    let epoch = restore(&set, &target.0, TENANT, now + 2).unwrap();
    let mut c = Control::open(&target.0, TENANT, "ffffffff", &[0u8; 32], now + 2).unwrap();
    c.activate_epoch(&epoch, now + 3).unwrap();
    assert_eq!(
        c.charge_reports(TENANT, now + 4, (1, 1), (2, 10))
            .unwrap_err()
            .code,
        ErrorCode::QuotaExhausted,
        "the restored store cannot spend the allowance again"
    );
    let enabled: i64 = c
        .conn
        .query_row("SELECT count(*) FROM schedules WHERE enabled=1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        enabled, 0,
        "restored schedules stay disabled until changed explicitly"
    );
    let rows: i64 = c
        .conn
        .query_row("SELECT count(*) FROM schedules", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1, "schedules are kept");
    assert_eq!(c.quarantine().unwrap().as_deref(), Some("restored"));
}

/// M3 review 1 finding 3 (C12): a refused database is never modified: a foreign WAL
/// database keeps every byte of its file and sidecars.
#[test]
fn a_refused_database_and_its_sidecars_are_never_modified() {
    let d = dir();
    let path = d.0.join(FILE);
    let foreign = Connection::open(&path).unwrap();
    foreign
        .execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE t(x); INSERT INTO t VALUES (1);")
        .unwrap();
    // Leave the WAL un-checkpointed, as a crashed foreign writer would.
    std::mem::forget(foreign);
    let wal = d.0.join(format!("{FILE}-wal"));
    assert!(wal.exists());
    let before = (sha(&path), sha(&wal));
    let refused = Control::open(&d.0, TENANT, EPOCH, &KEY, 2_000)
        .map(|_| ())
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::MaintenanceRequired);
    assert_eq!(
        (sha(&path), sha(&wal)),
        before,
        "the original bytes are untouched"
    );
    let leftovers: Vec<_> = std::fs::read_dir(&d.0)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.starts_with(".verify-"))
        .collect();
    assert!(leftovers.is_empty(), "the private copy is removed");
}

/// C12: a valid store with an uncheckpointed WAL is recovered on a private copy; the
/// original and its sidecars are preserved byte for byte, and the verified copy, with
/// the WAL's committed rows, is published in its place.
#[test]
fn a_recovered_store_preserves_the_original_and_publishes_a_verified_copy() {
    let d = dir();
    drop(open(&d));
    let path = d.0.join(FILE);
    let writer = Connection::open(&path).unwrap();
    writer.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
    writer
        .execute("UPDATE meta SET value='42' WHERE key='created_ms'", [])
        .unwrap();
    std::mem::forget(writer);
    let wal = d.0.join(format!("{FILE}-wal"));
    assert!(wal.exists());
    let before = (sha(&path), sha(&wal));
    let c = Control::open(&d.0, TENANT, EPOCH, &KEY, 5_000).unwrap();
    let created: String = c
        .conn
        .query_row("SELECT value FROM meta WHERE key='created_ms'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        created, "42",
        "the WAL's committed change is in the published copy"
    );
    let mode: String = c
        .conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "delete");
    let preserved = std::fs::read_dir(&d.0)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("preserved-")
        })
        .expect("the original is preserved");
    assert_eq!(
        (
            sha(&preserved.join(FILE)),
            sha(&preserved.join(format!("{FILE}-wal")))
        ),
        before,
        "preserved byte for byte"
    );
}

/// M3 review 1 finding 4: a set whose manifest matches its bytes but whose database
/// belongs to another tenant, or records an unsupported schema, is refused before
/// anything is placed.
#[test]
fn a_restore_validates_the_copy_itself_not_only_its_manifest() {
    let d = dir();
    let foreign = d.0.join("foreign");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&foreign)
        .unwrap();
    let set = d.0.join("set-foreign");
    {
        let mut other = Control::open(&foreign, "svc-other", EPOCH, &KEY, 1_000).unwrap();
        other.backup_into(&set, 2_000).unwrap();
    }
    // Forge the manifest's tenant; its size and hash still match the bytes.
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(set.join(MANIFEST)).unwrap()).unwrap();
    manifest["tenant"] = json!(TENANT);
    std::fs::write(set.join(MANIFEST), manifest.to_string()).unwrap();
    let target = dir();
    let refused = restore(&set, &target.0, TENANT, 3_000).unwrap_err();
    assert_eq!(
        refused.reason,
        "the backup copy does not match this service"
    );
    // An unsupported internal schema (neither the current version nor the accepted-M4
    // version 1 that restore migrates), with a manifest recomputed to match.
    let set2 = d.0.join("set-schema");
    {
        let mut c = open(&d);
        c.backup_into(&set2, 2_000).unwrap();
    }
    let db = Connection::open(set2.join(FILE)).unwrap();
    db.execute_batch(
        "PRAGMA ignore_check_constraints=ON; UPDATE meta SET value='99' WHERE key='schema';",
    )
    .unwrap();
    drop(db);
    let bytes = std::fs::read(set2.join(FILE)).unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(set2.join(MANIFEST)).unwrap()).unwrap();
    manifest["bytes"] = json!(bytes.len());
    manifest["sha256"] = json!(hex(&Sha256::digest(&bytes)));
    std::fs::write(set2.join(MANIFEST), manifest.to_string()).unwrap();
    assert!(restore(&set2, &target.0, TENANT, 3_000).is_err());
    let placed: Vec<_> = std::fs::read_dir(&target.0).unwrap().collect();
    assert!(placed.is_empty(), "refused restores place nothing");
}

/// M3 review 1 minor: a deadline failure records its fixed failure result and gap in
/// the same transaction as the edge, from the scheduler and from a claim.
#[test]
fn deadline_failures_record_their_result_and_gap() {
    let d = dir();
    let mut c = open(&d);
    let empty = json!({});
    let mut s = submission(principal("cli"), "key-deadline-00001", &empty);
    s.deadline_ms = 5_000;
    let job = c.submit(s, 2_000).unwrap().job_id;
    assert_eq!(
        c.claim_next(TENANT, "owner-1", 6_000, |_| true).unwrap(),
        None
    );
    let failed = c.job(TENANT, &job).unwrap().unwrap();
    assert_eq!(failed.state, State::Failed);
    assert_eq!(failed.gaps, ["deadline passed before the job could run"]);
    let result = failed.result.unwrap();
    assert_eq!(result["operation"], "collection_start");
    assert_eq!(result["error"]["schema_version"], 1);
    assert_events_are_contract_edges(&c);
}

/// M3 review 1 minor (C1): a process dying inside the real `submit()` transaction, on
/// the new-job path and on the join path, leaves nothing of that submission, and a retry
/// with the same key then succeeds exactly once. The new-job path is shown to leave a
/// hot journal, recovered on a verified copy (C12). The join transaction writes too
/// little to spill, so its death leaves an uncommitted transaction rather than a hot
/// journal; this test claims only that nothing of it survives.
#[test]
fn c1_dying_inside_submit_leaves_nothing_on_both_paths() {
    let count = |c: &Control, table: &str| -> i64 {
        c.conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    };
    let d = dir();
    drop(open(&d));
    assert_eq!(crash(&d, "submit"), 88);
    // The new-job path spills its writes (a one-page cache), so the dead process left a
    // hot journal: the next open recovers it on a verified copy (C12).
    // A hot journal: the file exists and begins with SQLite's journal header magic
    // (a zeroed header would mean the transaction never reached its first sync).
    let journal = std::fs::read(d.0.join(format!("{FILE}-journal"))).expect("a journal was left");
    assert!(
        journal.starts_with(&[0xd9, 0xd5, 0x05, 0xf9, 0x20, 0xa1, 0x63, 0xd7]),
        "the journal has a hot header"
    );
    let mut c = open(&d);
    for table in ["jobs", "job_params", "idempotency", "job_events"] {
        assert_eq!(count(&c, table), 0, "{table}");
    }
    let retry = c
        .submit(
            submission(principal("cli"), "key-crash-submit-01", &json!({"n": 1})),
            3_000,
        )
        .unwrap();
    assert!(!retry.existing, "the lost submission never committed");
    assert_eq!(count(&c, "jobs"), 1);

    let d = dir();
    drop(open(&d));
    assert_eq!(crash(&d, "submit-join"), 88);
    let mut c = open(&d);
    assert_eq!(count(&c, "jobs"), 1, "only the committed first job");
    assert_eq!(
        count(&c, "idempotency"),
        1,
        "the joining key never committed"
    );
    let p = json!({"mode": "recent"});
    let mut again = submission(principal("cli"), "key-crash-join-0002", &p);
    again.join_equivalent = true;
    let joined = c.submit(again, 3_000).unwrap();
    assert!(joined.joined && !joined.existing);
    assert_events_are_contract_edges(&c);
}

fn restored_with_job(now: i64) -> (Dir, Dir, Control, String, String) {
    let d = dir();
    let set = d.0.join("backup");
    let job = {
        let mut c = open(&d);
        let job = c
            .submit(
                submission(principal("cli"), "key-recover-0000001", &json!({})),
                2_000,
            )
            .unwrap()
            .job_id;
        c.backup_into(&set, 3_000).unwrap();
        job
    };
    let target = dir();
    let epoch = restore(&set, &target.0, TENANT, now).unwrap();
    let c = Control::open(&target.0, TENANT, "ffffffff", &[0u8; 32], now).unwrap();
    (d, target, c, job, epoch)
}

/// M3 review 2 finding 1: nothing is claimed until the store is both reconciled and
/// has an active epoch, in either order of the two maintenance steps.
#[test]
fn claims_need_both_activation_and_reconciliation_in_either_order() {
    let now = 10_000;
    let fresh = |c: &mut Control, epoch: &str, key: &'static str| {
        let empty = json!({});
        let mut s = submission(principal("cli"), key, &empty);
        s.expected_epoch = epoch;
        c.submit(s, now).unwrap().job_id
    };
    // Reconcile first, then activate.
    let (_a, _b, mut c, _restored, epoch) = restored_with_job(now);
    assert!(c.reconcile().unwrap());
    assert_eq!(
        c.claim_next(TENANT, "o", now + 1, |_| true).unwrap(),
        None,
        "no active epoch yet"
    );
    c.activate_epoch(&epoch, now + 2).unwrap();
    let job = fresh(&mut c, &epoch, "key-order-00000001");
    assert_eq!(claim_some(&mut c, now + 3), Some(job));
    // Activate first, then reconcile.
    let (_a, _b, mut c, _restored, epoch) = restored_with_job(now);
    c.activate_epoch(&epoch, now + 1).unwrap();
    let job = fresh(&mut c, &epoch, "key-order-00000002");
    assert_eq!(
        c.claim_next(TENANT, "o", now + 2, |_| true).unwrap(),
        None,
        "still quarantined"
    );
    assert!(c.reconcile().unwrap());
    assert_eq!(claim_some(&mut c, now + 3), Some(job));
}

/// M3 review 2 finding 2 and review 3 finding 1 (the reviewers' probes): a running
/// collection is backed up, then spends its whole tick budget (200 sends) before the
/// restore. Restored, activated and reconciled, its consumption is unknown, so its
/// budget is spent: during the recovery hold, after it (a clock rolled forward past it),
/// and after reconciliation, it can reserve nothing, and its next claim fails it with
/// the recovery gap and `quota_exhausted`.
#[test]
fn restored_jobs_never_regain_their_budget() {
    let d = dir();
    let set = d.0.join("backup");
    let job = {
        let mut c = open(&d);
        let job = c
            .submit(
                submission(principal("cli"), "key-unknown-0000001", &json!({})),
                2_000,
            )
            .unwrap()
            .job_id;
        c.claim_next(TENANT, "owner-1", 2_100, |_| true)
            .unwrap()
            .unwrap();
        c.backup_into(&set, 3_000).unwrap();
        for _ in 0..200 {
            let t = c
                .reserve_send(&job, "sessions", "management", None, 3_100)
                .unwrap();
            c.settle_send(&t, SendOutcome::Sent, 3_200).unwrap();
        }
        job
    };
    let target = dir();
    let now = 10_000;
    let epoch = restore(&set, &target.0, TENANT, now).unwrap();
    let mut c = Control::open(&target.0, TENANT, "ffffffff", &[0u8; 32], now).unwrap();
    c.activate_epoch(&epoch, now + 1).unwrap();
    assert!(c.reconcile().unwrap());
    for at in [now + 2, now + 86_400_001, now + 3 * 86_400_000] {
        let refused = c
            .reserve_send(&job, "sessions", "management", None, at)
            .unwrap_err();
        assert_eq!(refused.code, ErrorCode::QuotaExhausted, "at {at}");
    }
    assert_eq!(
        c.claim_next(TENANT, "o", now + 86_400_001, |_| true)
            .unwrap(),
        None
    );
    let j = c.job(TENANT, &job).unwrap().unwrap();
    assert_eq!(j.state, State::Failed);
    let result = j.result.unwrap();
    assert_eq!(result["error"]["code"], "quota_exhausted");
    assert_eq!(
        result["error"]["reason"],
        "recovery left this job's consumption unknown; its budget is spent",
        "the reason names the cause, not a deadline"
    );
    assert_events_are_contract_edges(&c);
}

/// M3 review 2 finding 5 (the reviewer's probe): a manifest-consistent backup whose
/// database lacks a required column is refused before anything is placed, and an
/// existing store of that shape is refused on open without being modified.
#[test]
fn stores_with_the_right_tables_but_wrong_structure_are_refused() {
    let d = dir();
    let set = d.0.join("backup");
    {
        let mut c = open(&d);
        c.backup_into(&set, 2_000).unwrap();
    }
    let db = Connection::open(set.join(FILE)).unwrap();
    db.execute_batch("ALTER TABLE jobs DROP COLUMN gaps_json;")
        .unwrap();
    drop(db);
    let bytes = std::fs::read(set.join(FILE)).unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(set.join(MANIFEST)).unwrap()).unwrap();
    manifest["bytes"] = json!(bytes.len());
    manifest["sha256"] = json!(hex(&Sha256::digest(&bytes)));
    std::fs::write(set.join(MANIFEST), manifest.to_string()).unwrap();
    let target = dir();
    assert_eq!(
        restore(&set, &target.0, TENANT, 3_000).unwrap_err().reason,
        "the backup copy does not match this service"
    );
    assert!(
        std::fs::read_dir(&target.0).unwrap().next().is_none(),
        "nothing placed"
    );
    // The same database as a live store: refused, bytes untouched.
    let live = dir();
    std::fs::copy(set.join(FILE), live.0.join(FILE)).unwrap();
    let before = sha(&live.0.join(FILE));
    let refused = Control::open(&live.0, TENANT, EPOCH, &KEY, 3_000)
        .map(|_| ())
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::MaintenanceRequired);
    assert_eq!(sha(&live.0.join(FILE)), before);
}

/// M3 review 2 finding 6 (the reviewer's probe): a job blocked because its grant was
/// revoked resumes once the grant is restored, through the §10 edge with its deadline
/// kept; while the grant stays revoked nothing changes and no event is written.
#[test]
fn a_restored_grant_requeues_a_policy_blocked_job() {
    let d = dir();
    let mut c = open(&d);
    let job = c
        .submit(
            submission(principal("cli"), "key-blocked-0000001", &json!({})),
            2_000,
        )
        .unwrap()
        .job_id;
    assert_eq!(c.claim_next(TENANT, "o", 2_100, |_| false).unwrap(), None);
    assert_eq!(
        c.job(TENANT, &job).unwrap().unwrap().state,
        State::BlockedPolicy
    );
    let events = |c: &Control| -> i64 {
        c.conn
            .query_row(
                "SELECT count(*) FROM job_events WHERE job_id=?1",
                [&job],
                |r| r.get(0),
            )
            .unwrap()
    };
    let before = events(&c);
    assert_eq!(c.reconsider_blocked(TENANT, 2_200, &|_| false).unwrap(), 0);
    assert_eq!(events(&c), before, "still blocked: no churn");
    let deadline = c.job(TENANT, &job).unwrap().unwrap().deadline_ms;
    assert_eq!(c.reconsider_blocked(TENANT, 2_300, &|_| true).unwrap(), 1);
    let j = c.job(TENANT, &job).unwrap().unwrap();
    assert_eq!((j.state, j.deadline_ms), (State::Queued, deadline));
    assert_eq!(
        c.claim_next(TENANT, "o", 2_400, |_| true)
            .unwrap()
            .unwrap()
            .job_id,
        job
    );
    assert_events_are_contract_edges(&c);
}

/// Turn a control-store file into the exact accepted-M4 (schema version 1) structure:
/// the only structural difference is the `upload_sessions.state` constraint.
fn downgrade_to_v1(path: &std::path::Path) {
    let v1 = Connection::open_in_memory().unwrap();
    v1.execute_batch(SCHEMA_V1).unwrap();
    let sql: String = v1
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='upload_sessions'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let c = Connection::open(path).unwrap();
    // Objects later versions added (M6's report tables and indexes, still empty here) do
    // not exist in version 1.
    let v1_names: std::collections::BTreeSet<String> = fingerprint(&v1)
        .unwrap()
        .into_iter()
        .map(|(_, name, _)| name)
        .collect();
    for (kind, name, _) in fingerprint(&c).unwrap() {
        if !v1_names.contains(&name) {
            c.execute_batch(&format!("DROP {} IF EXISTS {name}", kind.to_uppercase()))
                .unwrap();
        }
    }
    let cookie: i64 = c
        .query_row("PRAGMA schema_version", [], |r| r.get(0))
        .unwrap();
    c.execute_batch("PRAGMA writable_schema=ON").unwrap();
    c.execute(
        "UPDATE sqlite_master SET sql=?1 WHERE type='table' AND name='upload_sessions'",
        [&sql],
    )
    .unwrap();
    c.execute_batch(&format!(
        "PRAGMA schema_version={}; PRAGMA writable_schema=OFF",
        cookie + 1
    ))
    .unwrap();
    c.execute("UPDATE meta SET value='1' WHERE key='schema'", [])
        .unwrap();
    drop(c);
    let check = Connection::open(path).unwrap();
    assert_eq!(
        fingerprint(&check).unwrap(),
        schema_fingerprint(SCHEMA_V1).unwrap()
    );
}

/// M5 review 3 finding 2: an accepted-M4 control store (schema version 1, exactly the
/// frozen `control_v1.sql` structure) opens after a verified migration on a private copy:
/// its jobs, idempotency, epochs and results are unchanged, the original is preserved,
/// and the store is now version 2. A version-1 store whose structure differs is refused
/// with its bytes untouched. A version-1 backup set restores too.
#[test]
fn an_accepted_m4_control_store_migrates_on_open_and_restore() {
    let d = dir();
    let params = json!({"range": {"since": "a", "until": "b"}});
    let job = {
        let mut c = open(&d);
        c.submit(
            submission(principal("cli"), "key-m4-control-0000001", &params),
            2_000,
        )
        .unwrap()
        .job_id
    };
    let path = d.0.join(FILE);
    downgrade_to_v1(&path);
    let before = std::fs::read(&path).unwrap();
    let mut c = open(&d);
    let schema: String = c
        .conn
        .query_row("SELECT value FROM meta WHERE key='schema'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(schema, SCHEMA_VERSION.to_string());
    assert_eq!(
        fingerprint(&c.conn).unwrap(),
        expected_fingerprint().unwrap()
    );
    assert!(
        c.job(TENANT, &job).unwrap().is_some(),
        "the M4 job survives"
    );
    let again = c
        .submit(
            submission(principal("cli"), "key-m4-control-0000001", &params),
            3_000,
        )
        .unwrap();
    assert!(
        again.existing && again.job_id == job,
        "its idempotency survives"
    );
    // The original version-1 file is preserved beside the migrated store.
    let preserved: Vec<_> = std::fs::read_dir(&d.0)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("preserved-"))
        .collect();
    assert_eq!(preserved.len(), 1);
    assert_eq!(
        std::fs::read(preserved[0].path().join(FILE)).unwrap(),
        before,
        "the original is kept byte for byte"
    );
    // A version-1 backup set restores (migrated on the staged copy).
    let set = d.0.join("set");
    c.backup_into(&set, 4_000).unwrap();
    drop(c);
    downgrade_to_v1(&set.join(FILE));
    let bytes = std::fs::read(set.join(FILE)).unwrap();
    let mut manifest: Value =
        serde_json::from_slice(&std::fs::read(set.join(MANIFEST)).unwrap()).unwrap();
    manifest["schema"] = json!(1);
    manifest["bytes"] = json!(bytes.len());
    manifest["sha256"] = json!(hex(&Sha256::digest(&bytes)));
    std::fs::write(set.join(MANIFEST), serde_json::to_vec(&manifest).unwrap()).unwrap();
    let target = dir();
    restore(&set, &target.0, TENANT, 5_000).unwrap();
    let restored = Connection::open(target.0.join(FILE)).unwrap();
    assert_eq!(
        fingerprint(&restored).unwrap(),
        expected_fingerprint().unwrap()
    );
    // A version-1 store with any other structure is refused, untouched.
    let other = dir();
    drop(open(&other));
    let p = other.0.join(FILE);
    downgrade_to_v1(&p);
    Connection::open(&p)
        .unwrap()
        .execute_batch("CREATE TABLE unexpected (x INTEGER)")
        .unwrap();
    let bytes = std::fs::read(&p).unwrap();
    assert_eq!(
        Control::open(&other.0, TENANT, EPOCH, &KEY, 1_000)
            .err()
            .unwrap()
            .code,
        ErrorCode::MaintenanceRequired
    );
    assert_eq!(std::fs::read(&p).unwrap(), bytes);
}

/// M6 adds report tables (schema version 3): a version-2 (accepted M4 + M5) control store,
/// built from the frozen version-2 schema with a job in it, migrates on open with its job
/// intact and fingerprints exactly as a fresh store.
#[test]
fn a_version_2_control_store_migrates_to_the_m6_schema() {
    let d = dir();
    let path = d.0.join(FILE);
    {
        let c = Connection::open(&path).unwrap();
        c.execute_batch(include_str!("control_v2.sql")).unwrap();
        c.execute_batch(&format!(
            "INSERT INTO meta VALUES ('schema','2'),('created_ms','1'),('service_tenant','{TENANT}');
             INSERT INTO epochs VALUES ('{EPOCH}',1,1,'initial',zeroblob(32),'active');
             INSERT INTO jobs (job_id,epoch,tenant,operation,submitter_sub,submitter_client_id,service_actor,state,submitted_ms,deadline_ms,retry_after_ms,config_revision,credential_revision,scheduled_slot,claimed_by,result_json,updated_ms)
               VALUES ('j-v2','{EPOCH}','{TENANT}','status','s','c',NULL,'succeeded',1,2,NULL,'r',NULL,NULL,NULL,NULL,1);"
        ))
        .unwrap();
    }
    assert!(migrate_file(&path).unwrap(), "a version-2 store migrates");
    let c = Connection::open(&path).unwrap();
    assert_eq!(fingerprint(&c).unwrap(), expected_fingerprint().unwrap());
    let job: String = c
        .query_row("SELECT state FROM jobs WHERE job_id='j-v2'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(job, "succeeded");
    assert!(
        !migrate_file(&path).unwrap(),
        "a current store is left alone"
    );
}

fn review_c6_fixture() -> (
    Dir,
    Control,
    crate::reports::Bundles,
    crate::reports::VersionRow,
) {
    use sha2::{Digest, Sha256};
    let d = dir();
    let mut c = open(&d);
    let report = crate::reports::new_id("r1");
    let version = crate::reports::new_id("v1");
    // The bundle's job: a report job of this tenant for this report and kind.
    let args = json!({"report_id": report, "kind": "daily", "since_ms": 0, "until_ms": 3600000});
    let mut s = submission(principal("cli"), "key-review-c6-fixture", &args);
    s.operation = "reports_create";
    let job_id = c.submit(s, 1000).unwrap().job_id;
    let root = d.0.join("bundles");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let bundles = crate::reports::Bundles { root };
    c.ensure_report(
        TENANT,
        &report,
        "daily",
        (0, 3600000),
        &[],
        "test",
        Some(3600000),
        1,
    )
    .unwrap();
    let files = [
        ("report.html", "<p>test</p>"),
        ("review.md", "# Test"),
        ("evidence.json", "{}"),
    ];
    let manifest = json!({"report_id": report, "version_id": version, "version": 0, "kind": "daily", "job_id": job_id, "x_fence": 0, "x_evidence_digest": "ab".repeat(32), "x_summary": "1 event in the period", "period": {"since":"2026-07-04T05:00:00Z","until":"2026-07-05T05:00:00Z"}, "scheduled_at":"2026-07-05T05:00:00Z", "as_of":"2026-07-05T05:00:00Z", "timezone":"America/Chicago", "tzdb_version":"jiff-bundled", "analysis_version":1, "redaction_version":1, "completeness":"partial", "gaps":[], "predecessor":null, "files": files.iter().map(|(n,b)| json!({"name": n, "bytes": b.len(), "sha256": crate::reports::hex(&Sha256::digest(b.as_bytes()))})).collect::<Vec<_>>()});
    let manifest_bytes = manifest.to_string();
    let mut all: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_bytes())).collect();
    all.push(("manifest.json", manifest_bytes.as_bytes()));
    let stage = bundles.stage(&all).unwrap();
    bundles.place(&stage, &report, &version).unwrap();
    let row = crate::reports::VersionRow {
        report_id: report,
        version_id: version,
        version: 0,
        digest: "ab".repeat(32),
        manifest,
        summary: "test".into(),
    };
    (d, c, bundles, row)
}

#[test]
fn review_c6_must_validate_all_bundle_files() {
    let (_d, mut c, bundles, row) = review_c6_fixture();
    std::fs::write(
        bundles
            .dir(&row.report_id, &row.version_id)
            .join("evidence.json"),
        b"corrupt",
    )
    .unwrap();
    let recovered = bundles.recover(&mut c, TENANT, 2).unwrap();
    assert_eq!(
        recovered.0, 0,
        "C6 recovery must not publish a bundle with corrupt evidence.json"
    );
}

#[test]
fn review_c6_must_record_publication_completion() {
    let (_d, mut c, bundles, row) = review_c6_fixture();
    assert_eq!(bundles.recover(&mut c, TENANT, 2).unwrap().0, 1);
    let events: i64 = c
        .conn
        .query_row(
            "SELECT count(*) FROM completion_events WHERE job_id=?1 AND kind='report_published'",
            [row.manifest["job_id"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        events, 1,
        "a recovered publication must have its completion event"
    );
}

#[test]
fn review_queued_publisher_must_obey_report_tombstone() {
    let (_d, mut c, _bundles, mut row) = review_c6_fixture();
    c.record_version(
        TENANT,
        &row,
        "daily",
        row.manifest["job_id"].as_str().unwrap(),
        0,
        false,
        1,
    )
    .unwrap();
    let first = row.version_id.clone();
    row.version_id = crate::reports::new_id("v1");
    row.version = 1;
    row.digest = "cd".repeat(32);
    row.manifest["version_id"] = json!(row.version_id);
    row.manifest["version"] = json!(1);
    row.manifest["kind"] = json!("revision");
    row.manifest["predecessor"] = json!(first);
    c.record_version(
        TENANT,
        &row,
        "revision",
        row.manifest["job_id"].as_str().unwrap(),
        0,
        true,
        2,
    )
    .unwrap();
    c.delete_versions(
        TENANT,
        &[row.version_id.clone()],
        row.manifest["job_id"].as_str().unwrap(),
        3,
    )
    .unwrap();
    assert_eq!(
        c.report(TENANT, &row.report_id)
            .unwrap()
            .unwrap()
            .latest_version,
        Some(first)
    );
    // Reporter acquires its fence at execution, after deletion, then calls record_version.
    row.version_id = crate::reports::new_id("v1");
    row.manifest["version_id"] = json!(row.version_id);
    let fence = c.fence(TENANT).unwrap();
    assert!(
        c.record_version(
            TENANT,
            &row,
            "revision",
            row.manifest["job_id"].as_str().unwrap(),
            fence,
            true,
            4
        )
        .is_err(),
        "a queued revision must not publish into a tombstoned report even with the current fence"
    );
}

/// M6 review 1 finding 5: a published version whose completion event is missing (a store
/// written before events were atomic with publication) gets it back from recovery,
/// idempotently.
#[test]
fn recovery_repairs_a_missing_publication_event() {
    let (_d, mut c, bundles, row) = review_c6_fixture();
    let job = row.manifest["job_id"].as_str().unwrap().to_owned();
    c.record_version(TENANT, &row, "daily", &job, 0, false, 1)
        .unwrap();
    c.conn
        .execute("DELETE FROM completion_events WHERE job_id=?1", [&job])
        .unwrap();
    let count = |c: &Control| -> i64 {
        c.conn
            .query_row(
                "SELECT count(*) FROM completion_events WHERE job_id=?1 AND kind='report_published'",
                [&job],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(count(&c), 0);
    bundles.recover(&mut c, TENANT, 2).unwrap();
    assert_eq!(count(&c), 1, "recovery repairs the missing event");
    bundles.recover(&mut c, TENANT, 3).unwrap();
    assert_eq!(count(&c), 1, "idempotent");
}

#[test]
fn review2_recovery_restores_the_agent_summary() {
    let (_d, mut c, bundles, row) = review_c6_fixture();
    assert_eq!(bundles.recover(&mut c, TENANT, 2).unwrap().0, 1);
    let recovered = c.version(TENANT, &row.version_id).unwrap().unwrap();
    assert!(
        !recovered.summary.is_empty(),
        "reattachment loses the agent-facing summary"
    );
}

#[test]
fn review2_recovery_refuses_a_manifest_naming_an_unknown_job() {
    let (_d, mut c, bundles, row) = review_c6_fixture();
    let path = bundles
        .dir(&row.report_id, &row.version_id)
        .join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let prefix = manifest["job_id"]
        .as_str()
        .unwrap()
        .rsplit_once('_')
        .unwrap()
        .0;
    let unknown_job = format!("{prefix}_00000000-0000-4000-8000-000000000000");
    assert!(airs_runtime_core::dto::is_job_id(&unknown_job));
    manifest["job_id"] = json!(unknown_job);
    std::fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(
        bundles.recover(&mut c, TENANT, 2).unwrap().0,
        0,
        "C6 must not activate a bundle with an unknown job"
    );
}

/// M6 review 2 finding 3, a job of another report: a well-formed, existing job that
/// rendered a different report never has this bundle reattached to it.
#[test]
fn recovery_refuses_a_manifest_naming_another_reports_job() {
    let (_d, mut c, bundles, row) = review_c6_fixture();
    let args =
        json!({"report_id": "r1_other", "kind": "daily", "since_ms": 0, "until_ms": 3600000});
    let mut s = submission(principal("cli"), "key-review-c6-another", &args);
    s.operation = "reports_create";
    let other = c.submit(s, 1000).unwrap().job_id;
    let path = bundles
        .dir(&row.report_id, &row.version_id)
        .join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["job_id"] = json!(other);
    std::fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let (reattached, quarantined, _) = bundles.recover(&mut c, TENANT, 2).unwrap();
    assert_eq!((reattached, quarantined), (0, 1));
}

/// M6 review 2 finding 3, publication: a version naming no job of this tenant is
/// refused, never published without its completion event.
#[test]
fn publication_refuses_a_version_without_a_job_of_this_tenant() {
    let (_d, mut c, _bundles, row) = review_c6_fixture();
    let fence = c.fence(TENANT).unwrap();
    let refused = c
        .record_version(
            TENANT,
            &row,
            "daily",
            "j1_0a1b2c3d_00000000-0000-4000-8000-000000000000",
            fence,
            false,
            2,
        )
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::Conflict);
    assert!(c.version(TENANT, &row.version_id).unwrap().is_none());
}
