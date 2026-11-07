//! The single job worker (contract §6 concurrency, §9.2, §10). One worker per service
//! instance claims jobs fairly from the control store and runs the executor registered
//! for each operation. Executors touch AIRS only through [`JobContext::reserve_send`]
//! (crash point C2), persist progress only through [`JobContext::checkpoint`] (C3/C4),
//! and must stop at a checkpoint when cancellation is requested. M4 registers the
//! collection executor; M3 provides and tests the framework.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;

use crate::control::{Job, Refused, SendOutcome};
use crate::jobs::{State, Trigger};
use crate::tools::Storage;

/// How an executor's run ended.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Succeeded(Value),
    Partial(Value),
    Failed(Value),
    /// Cancellation was observed at a checkpoint; no further sends were made.
    Canceled,
    /// Cancellation was observed at a checkpoint after durable effects (an import's
    /// committed batches): the job ends canceled and keeps these details as its result.
    CanceledWith(Value),
    /// A retryable transport/token failure: wait and requeue within the deadline.
    RetryLater {
        after_ms: i64,
    },
    /// A second 401 after the replay, or a permanent denial, of the credential
    /// `revision` the run used (only another revision resumes the job).
    NeedsCredentials {
        revision: String,
    },
    /// Quota, quarantine or revocation.
    Blocked,
    /// The data volume is within its reserve for this operation (contract §6, §9.5):
    /// nothing was written; the job is requeued once space is freed.
    LowSpace,
}

/// What an executor may do with its job. Every call is one control-store transaction.
pub struct JobContext<'a> {
    pub job: &'a Job,
    storage: &'a Arc<Storage>,
}

impl JobContext<'_> {
    fn now() -> i64 {
        airs_runtime_core::time::now_ms()
    }

    /// Crash point C2: commit a reservation before an AIRS send; the returned
    /// transport ID is settled with [`JobContext::settle`].
    pub fn reserve_send(
        &self,
        source: &str,
        plane: &str,
        credential_revision: Option<&str>,
    ) -> Result<String, Refused> {
        self.storage.control(|c| {
            c.reserve_send(
                &self.job.job_id,
                source,
                plane,
                credential_revision,
                Self::now(),
            )
        })
    }

    pub fn settle(&self, transport_id: &str, outcome: SendOutcome) -> Result<(), Refused> {
        self.storage
            .control(|c| c.settle_send(transport_id, outcome, Self::now()))
    }

    /// Persist progress (and the archive operation it covers, crash point C4), and report
    /// whether cancellation has been requested: the executor stops before any further
    /// send, publication or destructive step when it has.
    pub fn checkpoint(
        &self,
        phase: &str,
        state: &Value,
        archive_operation_id: Option<&str>,
    ) -> Result<bool, Refused> {
        self.storage.control(|c| {
            c.checkpoint(
                &self.job.job_id,
                phase,
                state,
                archive_operation_id,
                Self::now(),
            )?;
            Ok(c.job(&self.job.tenant, &self.job.job_id)?
                .is_some_and(|j| j.state == State::CancelRequested))
        })
    }

    /// Whether `revision` is already denied on `plane` (never retried, §10).
    pub fn denied(&self, plane: &str, revision: &str) -> Result<bool, Refused> {
        self.storage
            .control(|c| c.denied(&self.job.tenant, plane, revision))
    }

    /// Record a final denial of `revision` on `plane`.
    pub fn record_denial(&self, plane: &str, revision: &str) -> Result<(), Refused> {
        self.storage
            .control(|c| c.record_denial(&self.job.tenant, plane, revision, Self::now()))
    }

    /// Charge the tenant's Reports allowance for this job (contract §6, §8): durable,
    /// by America/Chicago admission day, never refunded.
    pub fn charge_reports(&self, amount: (u64, u64), caps: (u64, u64)) -> Result<String, Refused> {
        self.storage
            .control(|c| c.charge_reports(&self.job.tenant, Self::now(), amount, caps))
    }

    /// An owned handle on the control storage (for hooks that outlive one call, such as
    /// the Reports admission gate and the cancellation watcher).
    pub fn storage(&self) -> Arc<Storage> {
        self.storage.clone()
    }

    /// Whether cancellation of this job has been requested.
    pub fn cancel_requested(&self) -> bool {
        self.storage
            .control(|c| c.job(&self.job.tenant, &self.job.job_id))
            .ok()
            .flatten()
            .is_some_and(|j| j.state == State::CancelRequested)
    }

    /// The admitted data subtree (exports and upload staging live under it).
    pub fn data_root(&self) -> &std::path::Path {
        &self.storage.admitted.path
    }

    /// The private report bundle tree (M6), created on first use.
    pub fn report_bundles(&self) -> Result<crate::reports::Bundles, Refused> {
        crate::storage::private_child(&self.storage.admitted, "reports")
            .map(|dir| crate::reports::Bundles { root: dir.path })
            .map_err(|_| Refused {
                code: airs_runtime_core::dto::ErrorCode::StorageUnavailable,
                reason: "the report bundle tree is unavailable",
                retry_after_seconds: None,
            })
    }

    /// Run one short control transaction (report bookkeeping).
    pub fn with_control<T>(
        &self,
        f: impl FnOnce(&mut crate::control::Control) -> Result<T, Refused>,
    ) -> Result<T, Refused> {
        self.storage.control(f)
    }

    /// The archive root this service collects into.
    pub fn history_root(&self) -> &std::path::Path {
        &self.storage.history_root
    }

    /// Hold `plane` until an absolute time (a recovered 429: its durable refusal time
    /// plus the hold); a longer existing hold is kept.
    pub fn hold_until(&self, plane: &str, until_ms: i64, reason: &str) -> Result<(), Refused> {
        self.storage
            .control(|c| c.hold(&self.job.tenant, plane, until_ms, reason))
    }

    /// Hold `plane` for `ms` from now (a 429 the run observed).
    pub fn hold(&self, plane: &str, ms: i64, reason: &str) -> Result<(), Refused> {
        self.storage
            .control(|c| c.hold(&self.job.tenant, plane, Self::now() + ms, reason))
    }

    /// The latest cooldown still holding on `plane`.
    pub fn cooldown_until(&self, plane: &str) -> Result<Option<i64>, Refused> {
        self.storage
            .control(|c| c.cooldown_until(&self.job.tenant, plane, Self::now()))
    }

    /// The job's canonical parameters.
    pub fn params(&self) -> Result<Value, Refused> {
        self.storage.control(|c| c.params(&self.job.job_id))
    }

    /// The last persisted checkpoint (resume point after a restart).
    pub fn resume_point(&self) -> Result<Option<(String, Value, Option<String>)>, Refused> {
        self.storage
            .control(|c| c.last_checkpoint(&self.job.job_id))
    }
}

/// An operation's executor.
pub trait Executor: Send + Sync {
    fn run(&self, ctx: &JobContext<'_>) -> Outcome;
}

/// The worker: the executors by operation, the storage, and this process's owner ID.
pub struct Worker {
    pub storage: Arc<Storage>,
    pub owner: String,
    pub executors: BTreeMap<String, Arc<dyn Executor>>,
}

impl Worker {
    /// Claim and run at most one job. `admit` rechecks the submitter's current grant
    /// (contract §2.5) at claim time. Jobs whose operation has no executor in this
    /// version stay queued and are not claimed. Returns the job ID and its final state.
    pub fn step(
        &self,
        tenant: &str,
        admit: impl Fn(&Job) -> bool,
    ) -> Result<Option<(String, State)>, Refused> {
        let now = airs_runtime_core::time::now_ms();
        let operations: Vec<String> = self.executors.keys().cloned().collect();
        let claimed = self
            .storage
            .control(|c| c.claim_next_of(tenant, &self.owner, now, Some(&operations), &admit))?;
        let Some(job) = claimed else {
            return Ok(None);
        };
        let executor = self.executors[&job.operation].clone();
        let ctx = JobContext {
            job: &job,
            storage: &self.storage,
        };
        // The disk reserve is checked before the executor writes anything (contract
        // §9.5): publication pauses at reserve + 512 MiB, collection at the reserve.
        // A lifecycle job is classified by its action (M8 review 2 finding 7). Its
        // parameters, unreadable, classify as allocating (an empty action).
        let action = (job.operation == "lifecycle_execute").then(|| {
            self.storage
                .control(|c| c.params(&job.job_id))
                .ok()
                .and_then(|p| p["action"].as_str().map(str::to_owned))
                .unwrap_or_default()
        });
        let outcome = if self
            .storage
            .space(&job.tenant)
            .admits_job(&job.operation, action.as_deref())
        {
            executor.run(&ctx)
        } else {
            Outcome::LowSpace
        };
        let now = airs_runtime_core::time::now_ms();
        let tenant = job.tenant.clone();
        let id = job.job_id.clone();
        let state = self.storage.control(|c| {
            let current = c
                .job(&tenant, &id)?
                .map(|j| j.state)
                .unwrap_or(State::Failed);
            // A cancellation requested during the run wins over every outcome except an
            // already-terminal state: the run ends canceled at its last checkpoint.
            // Every result the executor returns describes effects it committed (a late
            // cancellation, after its last checkpoint, cannot undo them), so the job ends
            // canceled *with* that result, in this one transaction (M5 review 3
            // finding 4); only an outcome without a result is released bare.
            if current == State::CancelRequested {
                return match &outcome {
                    Outcome::CanceledWith(result)
                    | Outcome::Succeeded(result)
                    | Outcome::Partial(result)
                    | Outcome::Failed(result) => {
                        c.finish(&tenant, &id, Trigger::CancelCheckpoint, result, now)
                    }
                    _ => c.release(
                        &tenant,
                        &id,
                        Trigger::CancelCheckpoint,
                        "canceled at checkpoint",
                        None,
                        now,
                    ),
                };
            }
            match outcome {
                Outcome::Succeeded(result) => c.finish(&tenant, &id, Trigger::Succeed, &result, now),
                Outcome::Partial(result) => c.finish(&tenant, &id, Trigger::PartialResult, &result, now),
                Outcome::Failed(result) => c.finish(&tenant, &id, Trigger::Fail, &result, now),
                Outcome::Canceled | Outcome::CanceledWith(_) => {
                    // An executor may only report cancellation after observing the request.
                    c.finish(&tenant, &id, Trigger::Fail, &serde_json::json!({"kind": "failure", "reason": "unrequested cancellation"}), now)
                }
                Outcome::RetryLater { after_ms } => c.release(&tenant, &id, Trigger::TransientFailure, "retry later", Some(now + after_ms.max(0)), now),
                Outcome::NeedsCredentials { revision } => c.park_credentials(&tenant, &id, &revision, now),
                Outcome::Blocked => c.release(&tenant, &id, Trigger::PolicyBlock, "blocked by policy", None, now),
                Outcome::LowSpace => c.release(&tenant, &id, Trigger::PolicyBlock, "the data volume is within its reserve", None, now),
            }
        })?;
        Ok(Some((id, state)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{Actor, Control, Submission};
    use serde_json::json;
    use std::os::unix::fs::DirBuilderExt;
    use std::sync::Mutex;

    const TENANT: &str = "svc-test";

    struct Dir(std::path::PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn storage() -> (Dir, Arc<Storage>, String) {
        let base = std::env::temp_dir().join(format!("airs-mcp-worker-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .unwrap();
        let base = base.canonicalize().unwrap();
        let admitted = crate::storage::admit(&base).unwrap();
        let control_dir = crate::storage::private_child(&admitted, "control").unwrap();
        let control =
            Control::open(&control_dir.path, TENANT, "0a1b2c3d", &[1u8; 32], 1_000).unwrap();
        let owner = crate::storage::OwnerRecord {
            owner_id: "w".into(),
            pod: "p".into(),
            node: "n".into(),
            pid: 0,
            acquired_at_unix: 0,
        };
        let history = base.join("history");
        (
            Dir(base),
            Arc::new(Storage::new(admitted, owner, history, control)),
            control_dir.path.display().to_string(),
        )
    }

    fn submit(s: &Storage, operation: &str, key: &str, deadline_ms: i64) -> String {
        let params = json!({"key": key});
        s.control(|c| {
            c.submit(
                Submission {
                    tenant: TENANT,
                    operation,
                    actor: Actor::Principal {
                        sub: "human-a".into(),
                        client_id: "cli".into(),
                    },
                    idempotency_key: key,
                    expected_epoch: "0a1b2c3d",
                    params: &params,
                    config_revision: "test-1",
                    credential_revision: None,
                    deadline_ms,
                    queue_tenant: 64,
                    queue_principal: 8,
                    join_equivalent: false,
                    scheduled_slot: None,
                },
                airs_runtime_core::time::now_ms(),
            )
        })
        .unwrap()
        .job_id
    }

    fn state(s: &Storage, job: &str) -> State {
        s.control(|c| c.job(TENANT, job)).unwrap().unwrap().state
    }

    /// Reserves and settles one send, checkpoints, and returns `outcome`; records every
    /// transport it reserved.
    struct Scripted {
        outcome: Outcome,
        sent: Mutex<Vec<String>>,
        cancel_before_checkpoint: Option<Arc<Storage>>,
    }

    impl Executor for Scripted {
        fn run(&self, ctx: &JobContext<'_>) -> Outcome {
            let transport = ctx.reserve_send("sessions", "management", None).unwrap();
            ctx.settle(&transport, SendOutcome::Sent).unwrap();
            self.sent.lock().unwrap().push(transport);
            if let Some(s) = &self.cancel_before_checkpoint {
                s.control(|c| c.cancel(TENANT, &ctx.job.job_id, airs_runtime_core::time::now_ms()))
                    .unwrap();
            }
            if ctx
                .checkpoint("sessions", &json!({"page": 1}), Some("op-1"))
                .unwrap()
            {
                // Cancellation observed: no further sends.
                return Outcome::Canceled;
            }
            let transport = ctx.reserve_send("results", "scanner", None).unwrap();
            ctx.settle(&transport, SendOutcome::Sent).unwrap();
            self.sent.lock().unwrap().push(transport);
            self.outcome.clone()
        }
    }

    fn worker(s: &Arc<Storage>, operation: &str, executor: Arc<dyn Executor>) -> Worker {
        Worker {
            storage: s.clone(),
            owner: "owner-1".into(),
            executors: [(operation.to_owned(), executor)].into_iter().collect(),
        }
    }

    #[test]
    fn a_job_runs_once_with_reserved_sends_and_keeps_its_result() {
        let (_d, s, _) = storage();
        let job = submit(&s, "collection_start", "key-worker-000001", i64::MAX);
        let exec = Arc::new(Scripted {
            outcome: Outcome::Succeeded(json!({"kind": "collection", "observed": 3})),
            sent: Mutex::new(vec![]),
            cancel_before_checkpoint: None,
        });
        let w = worker(&s, "collection_start", exec.clone());
        assert_eq!(
            w.step(TENANT, |_| true).unwrap(),
            Some((job.clone(), State::Succeeded))
        );
        assert_eq!(
            w.step(TENANT, |_| true).unwrap(),
            None,
            "a finished job never reruns"
        );
        let j = s.control(|c| c.job(TENANT, &job)).unwrap().unwrap();
        assert_eq!(j.result, Some(json!({"kind": "collection", "observed": 3})));
        assert_eq!(s.control(|c| c.consumed_sends(&job)).unwrap(), 2);
        assert_eq!(exec.sent.lock().unwrap().len(), 2);
    }

    #[test]
    fn cancellation_is_observed_at_the_next_checkpoint_and_stops_sends() {
        let (_d, s, _) = storage();
        let job = submit(&s, "collection_start", "key-worker-000002", i64::MAX);
        let exec = Arc::new(Scripted {
            outcome: Outcome::Succeeded(json!({})),
            sent: Mutex::new(vec![]),
            cancel_before_checkpoint: Some(s.clone()),
        });
        let w = worker(&s, "collection_start", exec.clone());
        assert_eq!(
            w.step(TENANT, |_| true).unwrap(),
            Some((job.clone(), State::Canceled))
        );
        assert_eq!(
            exec.sent.lock().unwrap().len(),
            1,
            "no send after the request"
        );
        assert_eq!(
            s.control(|c| c.consumed_sends(&job)).unwrap(),
            1,
            "the sent one stays consumed"
        );
    }

    /// M5 review 2 finding 5: an executor that observed cancellation after durable
    /// effects (an import's committed batches) ends the job canceled with its partial
    /// details kept as the result.
    #[test]
    fn a_cancellation_after_durable_effects_keeps_its_details() {
        struct CanceledAfterBatches(Arc<Storage>);
        impl Executor for CanceledAfterBatches {
            fn run(&self, ctx: &JobContext<'_>) -> Outcome {
                self.0
                    .control(|c| {
                        c.cancel(TENANT, &ctx.job.job_id, airs_runtime_core::time::now_ms())
                    })
                    .unwrap();
                assert!(
                    ctx.checkpoint("importing", &json!({"batches": 2}), Some("op-1"))
                        .unwrap()
                );
                Outcome::CanceledWith(
                    json!({"outcome": "partially_committed", "committed_batches": 2}),
                )
            }
        }
        let (_d, s, _) = storage();
        let job = submit(&s, "lifecycle_execute", "key-worker-000009", i64::MAX);
        let w = worker(
            &s,
            "lifecycle_execute",
            Arc::new(CanceledAfterBatches(s.clone())),
        );
        assert_eq!(
            w.step(TENANT, |_| true).unwrap(),
            Some((job.clone(), State::Canceled))
        );
        let j = s.control(|c| c.job(TENANT, &job)).unwrap().unwrap();
        assert_eq!(
            j.result,
            Some(json!({"outcome": "partially_committed", "committed_batches": 2}))
        );
    }

    #[test]
    fn retries_wait_requeue_when_due_and_fail_at_the_deadline() {
        let (_d, s, _) = storage();
        let now = airs_runtime_core::time::now_ms();
        let job = submit(&s, "collection_start", "key-worker-000003", now + 60_000);
        let exec = Arc::new(Scripted {
            outcome: Outcome::RetryLater { after_ms: 1_000 },
            sent: Mutex::new(vec![]),
            cancel_before_checkpoint: None,
        });
        let w = worker(&s, "collection_start", exec);
        assert_eq!(
            w.step(TENANT, |_| true).unwrap(),
            Some((job.clone(), State::WaitingRetry))
        );
        assert_eq!(
            s.control(|c| c.scheduler_pass(TENANT, now)).unwrap(),
            0,
            "not due yet"
        );
        assert_eq!(
            s.control(|c| c.scheduler_pass(TENANT, now + 30_000))
                .unwrap(),
            1
        );
        assert_eq!(state(&s, &job), State::Queued);
        assert_eq!(
            w.step(TENANT, |_| true).unwrap(),
            Some((job.clone(), State::WaitingRetry))
        );
        assert_eq!(
            s.control(|c| c.scheduler_pass(TENANT, now + 120_000))
                .unwrap(),
            1
        );
        assert_eq!(
            state(&s, &job),
            State::Failed,
            "the original deadline binds"
        );
        assert_eq!(
            s.control(|c| c.consumed_sends(&job)).unwrap(),
            4,
            "no attempt is refunded"
        );
    }

    #[test]
    fn unexecutable_operations_stay_queued_and_revoked_grants_block() {
        let (_d, s, _) = storage();
        let report = submit(&s, "reports_create", "key-worker-000004", i64::MAX);
        let exec = Arc::new(Scripted {
            outcome: Outcome::Succeeded(json!({})),
            sent: Mutex::new(vec![]),
            cancel_before_checkpoint: None,
        });
        let w = worker(&s, "collection_start", exec);
        assert_eq!(w.step(TENANT, |_| true).unwrap(), None);
        assert_eq!(
            state(&s, &report),
            State::Queued,
            "no executor: never claimed or blocked"
        );
        let revoked = submit(&s, "collection_start", "key-worker-000005", i64::MAX);
        assert_eq!(w.step(TENANT, |_| false).unwrap(), None);
        assert_eq!(state(&s, &revoked), State::BlockedPolicy);
        // A queued job canceled before any claim completes at the next scheduler pass.
        s.control(|c| c.cancel(TENANT, &report, airs_runtime_core::time::now_ms()))
            .unwrap();
        assert_eq!(
            s.control(|c| c.scheduler_pass(TENANT, airs_runtime_core::time::now_ms()))
                .unwrap(),
            1
        );
        assert_eq!(state(&s, &report), State::Canceled);
    }

    /// A worker that dies mid-run leaves a running job and a `reserved` send. Recovery
    /// keeps the send consumed (`uncertain`), requeues the job, and the next run resumes
    /// from the last checkpoint.
    #[test]
    fn a_worker_death_mid_run_resumes_from_the_checkpoint_with_its_send_consumed() {
        let (_d, s, control_dir) = storage();
        let job = submit(&s, "collection_start", "key-worker-000006", i64::MAX);
        s.control(|c| {
            let claimed = c
                .claim_next(TENANT, "owner-dead", 2_000, |_| true)?
                .unwrap();
            c.checkpoint(
                &claimed.job_id,
                "sessions",
                &json!({"page": 7}),
                Some("op-7"),
                2_100,
            )?;
            c.reserve_send(&claimed.job_id, "results", "scanner", None, 2_200)
        })
        .unwrap();
        // Restart: a new process opens the same store and recovers before any claim.
        let mut reopened = Control::open(
            std::path::Path::new(&control_dir),
            TENANT,
            "0a1b2c3d",
            &[1u8; 32],
            3_000,
        )
        .unwrap();
        let recovery = reopened.recover(3_000).unwrap();
        assert_eq!((recovery.uncertain, recovery.requeued), (1, 1));
        drop(reopened);
        let resumed = Arc::new(Mutex::new(None));
        type ResumePoint = Option<(String, serde_json::Value, Option<String>)>;
        struct Resume(Arc<Mutex<ResumePoint>>);
        impl Executor for Resume {
            fn run(&self, ctx: &JobContext<'_>) -> Outcome {
                *self.0.lock().unwrap() = ctx.resume_point().unwrap();
                Outcome::Partial(json!({"kind": "collection", "resumed": true}))
            }
        }
        let w = worker(&s, "collection_start", Arc::new(Resume(resumed.clone())));
        assert_eq!(
            w.step(TENANT, |_| true).unwrap(),
            Some((job.clone(), State::Partial))
        );
        assert_eq!(
            resumed.lock().unwrap().clone(),
            Some(("sessions".into(), json!({"page": 7}), Some("op-7".into())))
        );
        assert_eq!(
            s.control(|c| c.consumed_sends(&job)).unwrap(),
            1,
            "the uncertain send stays consumed"
        );
    }

    /// M5 review 3 finding 4: a cancellation that arrives after the executor's last
    /// checkpoint (here: requested from inside the run, after its effects) never
    /// discards the result describing those effects: success, partial and failure
    /// results are all kept on the canceled job.
    #[test]
    fn a_late_cancellation_keeps_the_committed_result_for_every_outcome() {
        struct Late(fn(Value) -> Outcome);
        impl Executor for Late {
            fn run(&self, ctx: &JobContext<'_>) -> Outcome {
                ctx.storage()
                    .control(|c| {
                        c.cancel(
                            &ctx.job.tenant,
                            &ctx.job.job_id,
                            airs_runtime_core::time::now_ms(),
                        )
                    })
                    .unwrap();
                (self.0)(json!({"operation": "lifecycle_execute", "imported_records": 1000}))
            }
        }
        type Make = fn(Value) -> Outcome;
        let cases: [(&str, Make); 3] = [
            ("succeeded", Outcome::Succeeded),
            ("partial", Outcome::Partial),
            ("failed", Outcome::Failed),
        ];
        for (name, make) in cases {
            let (_d, s, _) = storage();
            let key = format!("late-cancel-key-{name}-000");
            let job = submit(&s, "lifecycle_execute", &key, i64::MAX);
            let w = worker(&s, "lifecycle_execute", Arc::new(Late(make)));
            w.step(TENANT, |_| true).unwrap();
            let j = s.control(|c| c.job(TENANT, &job)).unwrap().unwrap();
            assert_eq!(j.state, State::Canceled, "{name}");
            assert_eq!(
                j.result.as_ref().map(|r| r["imported_records"].clone()),
                Some(json!(1000)),
                "{name}: the committed result is kept"
            );
        }
    }
}
