//! M4.3 quarter-hour collection schedule (contract §6 "Collection interval", §10).
//!
//! Rules implemented:
//! - Slots are UTC multiples of the interval (default 15 min, bound 5–60 min, and the
//!   interval must divide 60 so slots stay on quarter/whole-hour boundaries).
//! - A slot is due at its start; nothing is due before the schedule's `effective_ms`.
//! - Missed ticks coalesce: only the newest due slot is submitted; older unsubmitted
//!   slots since the last completed one are recorded `skipped` (never fetched later).
//! - At most one scheduled collection is live; while one is queued/running/retrying, a
//!   due slot is recorded `pending` and coalesces into the next submission.
//! - The job is submitted as the service actor with a slot-derived idempotency key,
//!   so a restart between recording the slot and submitting cannot double-submit.
//! - Disabling records its effective time and never deletes slots or history.

use rusqlite::{OptionalExtension, params};
use serde_json::json;

use crate::control::{Actor, Control, Refused, Submission};

pub const DEFAULT_INTERVAL_MIN: i64 = 15;
/// At most this many missed slots are recorded per catch-up (a week of quarter hours).
pub const MAX_MISSED_SLOTS: i64 = 672;
pub const SCHEDULE: &str = "collection";
pub const SERVICE_ACTOR: &str = "scheduler";

/// The newest slot start `<= now` for `interval_min`, in epoch ms.
pub fn slot_at(now_ms: i64, interval_min: i64) -> i64 {
    let step = interval_min * 60_000;
    now_ms.div_euclid(step) * step
}

pub fn valid_interval(interval_min: i64) -> bool {
    (5..=60).contains(&interval_min) && 60 % interval_min == 0
}

/// Slot names are their UTC instant, so ordering is lexical and stable.
pub fn slot_name(slot_ms: i64) -> String {
    airs_runtime_core::time::iso(slot_ms)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Tick {
    /// Nothing due (disabled, before effective time, or slot already handled).
    Idle,
    /// A scheduled collection is live; the slot was recorded pending.
    Coalesced { slot: String },
    /// The slot's job was submitted (or joined after a restart).
    Submitted {
        slot: String,
        job_id: String,
        skipped: u64,
    },
}

impl Control {
    /// One schedule tick, in one transaction, called by the worker before claiming.
    pub fn schedule_tick(
        &mut self,
        tenant: &str,
        ctx: &TickContext<'_>,
        now_ms: i64,
    ) -> Result<Tick, Refused> {
        if self.quarantine()?.is_some() {
            return Ok(Tick::Idle);
        }
        // Slots whose job has finished are completed, committed on their own so no later
        // early return (disabled, not due) can roll the bookkeeping back.
        self.conn
            .execute(
                "UPDATE schedule_slots SET state='completed' WHERE tenant=?1 AND name=?2 AND state='submitted' AND job_id IN (SELECT job_id FROM jobs WHERE state IN ('canceled','succeeded','partial','failed'))",
                params![tenant, SCHEDULE],
            )
            .map_err(storage)?;
        // A scheduler job without its slot association (a store written before the
        // association joined the submission transaction) is re-associated from its
        // slot-derived idempotency key, even after its slot has passed.
        self.repair_slot_associations(tenant)?;
        let tx = self.conn.transaction().map_err(storage)?;
        let row: Option<(i64, String, i64)> = tx
            .query_row(
                "SELECT enabled, spec_json, effective_ms FROM schedules WHERE tenant=?1 AND name=?2",
                params![tenant, SCHEDULE],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(storage)?;
        let Some((1, spec, effective_ms)) = row else {
            return Ok(Tick::Idle);
        };
        let spec: serde_json::Value = serde_json::from_str(&spec).unwrap_or_default();
        let fallback = if valid_interval(ctx.interval_minutes) {
            ctx.interval_minutes
        } else {
            DEFAULT_INTERVAL_MIN
        };
        // The version in effect now gives the interval; the version in effect at the
        // due slot decides whether it runs (a future change never suppresses ticks
        // before it takes effect).
        let Some(now_version) = version_at(&spec, effective_ms, now_ms) else {
            return Ok(Tick::Idle);
        };
        let interval = now_version
            .interval
            .filter(|i| valid_interval(*i))
            .unwrap_or(fallback);
        let slot_ms = slot_at(now_ms, interval);
        if !version_at(&spec, effective_ms, slot_ms).is_some_and(|v| v.enabled) {
            return Ok(Tick::Idle);
        }
        let slot = slot_name(slot_ms);
        let state: Option<String> = tx
            .query_row(
                "SELECT state FROM schedule_slots WHERE tenant=?1 AND name=?2 AND slot=?3",
                params![tenant, SCHEDULE, slot],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        if matches!(
            state.as_deref(),
            Some("submitted" | "completed" | "skipped")
        ) {
            return Ok(Tick::Idle);
        }
        // One live scheduled collection at a time: record and coalesce.
        let live: Option<String> = tx
            .query_row(
                "SELECT job_id FROM jobs WHERE tenant=?1 AND operation='collection_start' AND service_actor=?2
                 AND state IN ('queued','running','waiting_retry','needs_credentials','blocked_policy','cancel_requested') LIMIT 1",
                params![tenant, SERVICE_ACTOR],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        if live.is_some() {
            tx.execute(
                "INSERT INTO schedule_slots VALUES (?1,?2,?3,NULL,'pending') ON CONFLICT DO NOTHING",
                params![tenant, SCHEDULE, slot],
            )
            .map_err(storage)?;
            tx.commit().map_err(storage)?;
            return Ok(Tick::Coalesced { slot });
        }
        // Slots missed while nothing ran (downtime) are recorded as skipped history, never
        // fetched: from the last recorded slot to this one, bounded, and only where the
        // version in effect at that slot had the schedule enabled.
        let last: Option<String> = tx
            .query_row(
                "SELECT max(slot) FROM schedule_slots WHERE tenant=?1 AND name=?2",
                params![tenant, SCHEDULE],
                |r| r.get(0),
            )
            .map_err(storage)?;
        let mut missed = 0u64;
        if let Some(last_ms) = last.as_deref().and_then(airs_runtime_core::dto::utc_ms) {
            // Each missed slot follows the grid of the version in effect at that time:
            // the interval of the version at the previous slot, re-aligned to a new
            // version's own grid from its effective time.
            let step_at = |at: i64| -> i64 {
                version_at(&spec, effective_ms, at)
                    .and_then(|v| v.interval)
                    .filter(|i| valid_interval(*i))
                    .unwrap_or(fallback)
                    * 60_000
            };
            let next_after = |at: i64| -> i64 {
                // The next slot after `at` on the grid of the version in effect then; a
                // version taking effect before that slot starts its own grid from its
                // effective time, and so on chronologically: every intervening version
                // is reconsidered before a slot is accepted.
                let mut from = at;
                let mut candidate = at + step_at(at);
                loop {
                    let next = versions(&spec, effective_ms)
                        .into_iter()
                        .filter(|v| v.effective_ms > from && v.effective_ms <= candidate)
                        .min_by_key(|v| v.effective_ms);
                    match next {
                        Some(v) => {
                            let step = v
                                .interval
                                .filter(|i| valid_interval(*i))
                                .unwrap_or(fallback)
                                * 60_000;
                            from = v.effective_ms;
                            candidate = (v.effective_ms + step - 1).div_euclid(step) * step;
                        }
                        None => return candidate,
                    }
                }
            };
            let mut at = next_after(last_ms);
            let mut seen = 0;
            while at < slot_ms && seen < MAX_MISSED_SLOTS {
                if at >= effective_ms
                    && version_at(&spec, effective_ms, at).is_some_and(|v| v.enabled)
                {
                    missed += tx
                        .execute(
                            "INSERT INTO schedule_slots VALUES (?1,?2,?3,NULL,'skipped') ON CONFLICT DO NOTHING",
                            params![tenant, SCHEDULE, slot_name(at)],
                        )
                        .map_err(storage)? as u64;
                }
                at = next_after(at);
                seen += 1;
            }
        }
        // Older pending slots coalesce into this one.
        let skipped = missed + tx
            .execute(
                "UPDATE schedule_slots SET state='skipped' WHERE tenant=?1 AND name=?2 AND slot<?3 AND state='pending'",
                params![tenant, SCHEDULE, slot],
            )
            .map_err(storage)? as u64;
        // The due slot is recorded before submission, so a submission that fails (and
        // rolls back whole) still leaves the slot in history: a later tick records it
        // skipped; a successful submission takes the row over.
        tx.execute(
            "INSERT INTO schedule_slots VALUES (?1,?2,?3,NULL,'pending') ON CONFLICT DO NOTHING",
            params![tenant, SCHEDULE, slot],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        // Submission is its own transaction (C1), carrying the slot association: the
        // slot-derived key makes a retry after a crash here join (and re-associate) the
        // same job.
        let key = format!("sched-{}-{}", SCHEDULE, slot_ms);
        let submitted = self.submit(
            Submission {
                tenant,
                operation: "collection_start",
                actor: Actor::Service(SERVICE_ACTOR.into()),
                idempotency_key: &key,
                expected_epoch: ctx.epoch,
                params: &json!({"mode": "recent", "range": null, "app_ids": []}),
                config_revision: ctx.config_revision,
                credential_revision: ctx.credential_revision,
                // A tick that cannot finish before the next slot is superseded by it.
                deadline_ms: slot_ms + interval * 60_000 * 4,
                queue_tenant: ctx.queue_tenant,
                queue_principal: ctx.queue_tenant,
                join_equivalent: false,
                // The job, its idempotency record, `scheduled_slot` and the slot's
                // association commit together (no separate write can be lost).
                scheduled_slot: Some((SCHEDULE, &slot)),
            },
            now_ms,
        )?;
        Ok(Tick::Submitted {
            slot,
            job_id: submitted.job_id,
            skipped,
        })
    }

    /// Re-associate every scheduler collection job whose `scheduled_slot` is missing,
    /// from its `sched-collection-<slot ms>` idempotency key, in one transaction.
    pub fn repair_slot_associations(&mut self, tenant: &str) -> Result<u64, Refused> {
        let tx = self.conn.transaction().map_err(storage)?;
        let stranded: Vec<(String, String)> = {
            let mut stmt = tx
                .prepare(
                    "SELECT j.job_id, i.key FROM jobs j JOIN idempotency i ON i.job_id=j.job_id
                     WHERE j.tenant=?1 AND j.operation='collection_start' AND j.service_actor=?2
                     AND j.scheduled_slot IS NULL AND i.key LIKE 'sched-collection-%'
                     ORDER BY j.submitted_ms",
                )
                .map_err(storage)?;
            stmt.query_map(params![tenant, SERVICE_ACTOR], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?
        };
        let mut repaired = 0;
        for (job_id, key) in stranded {
            let Some(slot_ms) = key
                .strip_prefix("sched-collection-")
                .and_then(|ms| ms.parse::<i64>().ok())
            else {
                continue;
            };
            Self::associate_slot(&tx, tenant, SCHEDULE, &slot_name(slot_ms), &job_id)?;
            repaired += 1;
        }
        tx.commit().map_err(storage)?;
        Ok(repaired)
    }

    /// Mark a scheduled job's slot completed when the job reaches a terminal state.
    pub fn complete_slot(&mut self, job_id: &str) -> Result<(), Refused> {
        self.conn
            .execute(
                "UPDATE schedule_slots SET state='completed' WHERE job_id=?1 AND state='submitted'",
                params![job_id],
            )
            .map_err(storage)?;
        Ok(())
    }
}

/// One effective-dated version of the schedule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub effective_ms: i64,
    pub enabled: bool,
    pub interval: Option<i64>,
}

/// The schedule's versions in effect order (equal times: the later change wins). A spec
/// written before versions existed is read as one version.
fn versions(spec: &serde_json::Value, effective_ms: i64) -> Vec<Version> {
    let mut out: Vec<Version> = match spec["versions"].as_array() {
        Some(list) => list
            .iter()
            .filter_map(|v| {
                Some(Version {
                    effective_ms: v["effective_ms"].as_i64()?,
                    enabled: v["enabled"].as_bool()?,
                    interval: v["interval_minutes"].as_i64(),
                })
            })
            .collect(),
        None => {
            let mut legacy = vec![Version {
                effective_ms,
                enabled: true,
                interval: spec["interval_minutes"].as_i64(),
            }];
            if let Some(off) = spec["disabled_from_ms"].as_i64() {
                legacy.push(Version {
                    effective_ms: off,
                    enabled: false,
                    interval: None,
                });
            }
            legacy
        }
    };
    out.sort_by_key(|v| v.effective_ms);
    out
}

/// The version in effect at `at_ms`, if any has taken effect.
pub fn version_at(spec: &serde_json::Value, effective_ms: i64, at_ms: i64) -> Option<Version> {
    versions(spec, effective_ms)
        .into_iter()
        .rev()
        .find(|v| v.effective_ms <= at_ms)
}

/// One schedule as `schedules_list` reports it (frozen result item): the version in
/// effect now (or, when none has taken effect yet, the first one, not enabled).
pub fn schedule_view(
    kind: &str,
    spec: &serde_json::Value,
    effective_ms: i64,
    now_ms: i64,
    backlog: i64,
    last: Option<String>,
) -> serde_json::Value {
    let all = versions(spec, effective_ms);
    let (enabled, effective, interval) = match version_at(spec, effective_ms, now_ms) {
        Some(v) => (v.enabled, v.effective_ms, v.interval),
        None => (
            false,
            all.first().map_or(effective_ms, |v| v.effective_ms),
            all.first().and_then(|v| v.interval),
        ),
    };
    json!({
        "schedule_id": spec["schedule_id"],
        "kind": kind,
        "enabled": enabled,
        "effective_at": airs_runtime_core::time::iso(effective),
        "interval_minutes": interval,
        "revision_allowance": serde_json::Value::Null,
        "revision_interval_hours": serde_json::Value::Null,
        "backlog_slots": backlog,
        "last_completed_slot": last,
    })
}

impl Control {
    /// The stored spec (versions) of schedule `kind`.
    fn schedule_spec_of(
        &self,
        tenant: &str,
        kind: &str,
    ) -> Result<Option<(serde_json::Value, i64)>, Refused> {
        let row: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT spec_json, effective_ms FROM schedules WHERE tenant=?1 AND name=?2",
                params![tenant, kind],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        Ok(row.map(|(t, e)| (serde_json::from_str(&t).unwrap_or_default(), e)))
    }

    /// Append one effective-dated version and bump the revision; nothing earlier is
    /// overwritten, so ticks before `effective_ms` keep the previous version.
    fn push_version(
        &mut self,
        tenant: &str,
        kind: &str,
        id: Option<&str>,
        version: Version,
    ) -> Result<(String, i64), Refused> {
        let prior = self.schedule_spec_of(tenant, kind)?;
        let (mut spec, effective) = prior.clone().unwrap_or((json!({}), version.effective_ms));
        if let Some(id) = id
            && spec["schedule_id"].as_str() != Some(id)
        {
            return Err(Refused {
                code: airs_runtime_core::dto::ErrorCode::NotFound,
                reason: "no such schedule",
                retry_after_seconds: None,
            });
        }
        let mut list: Vec<serde_json::Value> = versions(&spec, effective)
            .into_iter()
            .map(|v| json!({"effective_ms": v.effective_ms, "enabled": v.enabled, "interval_minutes": v.interval}))
            .collect();
        if prior.is_none() {
            list.clear();
        }
        list.push(json!({"effective_ms": version.effective_ms, "enabled": version.enabled, "interval_minutes": version.interval}));
        let schedule_id = spec["schedule_id"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("s1_{}", uuid::Uuid::new_v4()));
        let revision = spec["revision"].as_i64().unwrap_or(0) + 1;
        spec = json!({"schedule_id": schedule_id, "revision": revision, "versions": list});
        let earliest = versions(&spec, version.effective_ms)
            .first()
            .map_or(version.effective_ms, |v| v.effective_ms);
        let tx = self.conn.savepoint().map_err(storage)?;
        tx.execute(
            "INSERT INTO schedules VALUES (?1,?2,1,?3,?4) ON CONFLICT(tenant,name) DO UPDATE SET enabled=1, spec_json=excluded.spec_json, effective_ms=excluded.effective_ms",
            params![tenant, kind, spec.to_string(), earliest],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok((schedule_id, revision))
    }

    /// `schedules_set` for the collection schedule (contract §6, §4.1): a new version
    /// from `effective_ms` (interval 5–60, dividing an hour). Earlier versions, slots and
    /// history are kept.
    pub fn schedule_set(
        &mut self,
        tenant: &str,
        enabled: bool,
        interval: Option<i64>,
        effective_ms: i64,
    ) -> Result<(String, i64), Refused> {
        self.schedule_set_kind(tenant, SCHEDULE, enabled, interval, effective_ms)
    }

    /// `schedules_set` for any kind (`collection`, `daily_report`, `daily_enrichment`).
    /// The interval applies to the collection schedule only.
    pub fn schedule_set_kind(
        &mut self,
        tenant: &str,
        kind: &str,
        enabled: bool,
        interval: Option<i64>,
        effective_ms: i64,
    ) -> Result<(String, i64), Refused> {
        if interval.is_some_and(|i| !valid_interval(i)) {
            return Err(Refused {
                code: airs_runtime_core::dto::ErrorCode::InvalidRequest,
                reason: "invalid collection interval",
                retry_after_seconds: None,
            });
        }
        self.push_version(
            tenant,
            kind,
            None,
            Version {
                effective_ms,
                enabled,
                interval,
            },
        )
    }

    /// `schedules_disable`: a disabled version from `effective_ms` (slots before it still
    /// run; nothing is erased).
    pub fn schedule_disable(
        &mut self,
        tenant: &str,
        schedule_id: &str,
        effective_ms: i64,
    ) -> Result<(String, i64), Refused> {
        // The schedule this ID names, of whichever kind.
        let kind: Option<String> = self
            .conn
            .query_row(
                "SELECT name FROM schedules WHERE tenant=?1 AND json_extract(spec_json,'$.schedule_id')=?2",
                params![tenant, schedule_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        let Some(kind) = kind else {
            return Err(Refused {
                code: airs_runtime_core::dto::ErrorCode::NotFound,
                reason: "no such schedule",
                retry_after_seconds: None,
            });
        };
        let interval = self
            .schedule_spec_of(tenant, &kind)?
            .and_then(|(spec, e)| version_at(&spec, e, effective_ms).and_then(|v| v.interval));
        self.push_version(
            tenant,
            &kind,
            Some(schedule_id),
            Version {
                effective_ms,
                enabled: false,
                interval,
            },
        )
    }

    /// `schedules_list`: every schedule with the version in effect now, its backlog and
    /// last completed slot.
    pub fn schedules_list(
        &self,
        tenant: &str,
        now_ms: i64,
    ) -> Result<Vec<serde_json::Value>, Refused> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT name, spec_json, effective_ms FROM schedules WHERE tenant=?1 ORDER BY name",
            )
            .map_err(storage)?;
        let rows: Vec<(String, String, i64)> = stmt
            .query_map([tenant], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(storage)?
            .collect::<Result<_, _>>()
            .map_err(storage)?;
        let mut out = Vec::new();
        for (name, spec, effective) in rows {
            let spec: serde_json::Value = serde_json::from_str(&spec).unwrap_or_default();
            if spec["schedule_id"].is_null() {
                continue;
            }
            let (backlog, last): (i64, Option<String>) = self
                .conn
                .query_row(
                    "SELECT coalesce(sum(state='pending'),0), max(CASE WHEN state='completed' THEN slot END) FROM schedule_slots WHERE tenant=?1 AND name=?2",
                    params![tenant, name],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(storage)?;
            out.push(schedule_view(
                &name, &spec, effective, now_ms, backlog, last,
            ));
        }
        Ok(out)
    }
}

pub struct TickContext<'a> {
    /// The configured collection interval (`limits.collection_interval_minutes`),
    /// used unless the schedule's own spec names one.
    pub interval_minutes: i64,
    pub epoch: &'a str,
    pub config_revision: &'a str,
    pub credential_revision: Option<&'a str>,
    pub queue_tenant: u64,
}

fn storage(_: rusqlite::Error) -> Refused {
    Refused {
        code: airs_runtime_core::dto::ErrorCode::StorageUnavailable,
        reason: "control store unavailable",
        retry_after_seconds: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{State, Trigger};
    use std::os::unix::fs::DirBuilderExt;

    const TENANT: &str = "svc-test";
    const EPOCH: &str = "0a1b2c3d";
    const Q: i64 = 15 * 60_000;
    /// A quarter-hour boundary.
    const T0: i64 = 1_793_000_000_000 / Q * Q;

    struct Dir(std::path::PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn open() -> (Dir, Control) {
        let d = std::env::temp_dir().join(format!("airs-mcp-sched-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new().mode(0o700).create(&d).unwrap();
        let c = Control::open(&d, TENANT, EPOCH, &[1u8; 32], 1_000).unwrap();
        (Dir(d), c)
    }

    fn ctx() -> TickContext<'static> {
        TickContext {
            interval_minutes: 15,
            epoch: EPOCH,
            config_revision: "test-1",
            credential_revision: None,
            queue_tenant: 64,
        }
    }

    fn enable(c: &Control, effective: i64) {
        c.conn
            .execute(
                "INSERT INTO schedules VALUES (?1,'collection',1,'{}',?2) ON CONFLICT(tenant,name) DO UPDATE SET enabled=1, effective_ms=excluded.effective_ms",
                params![TENANT, effective],
            )
            .unwrap();
    }

    fn jobs(c: &Control) -> i64 {
        c.conn
            .query_row("SELECT count(*) FROM jobs", [], |r| r.get(0))
            .unwrap()
    }

    fn finish(c: &mut Control, job: &str, now: i64) {
        c.claim_next(TENANT, "w", now, |_| true).unwrap().unwrap();
        c.finish(TENANT, job, Trigger::Succeed, &json!({}), now)
            .unwrap();
    }

    #[test]
    fn intervals_stay_on_hour_divisors_within_bounds() {
        for ok in [5, 6, 10, 12, 15, 20, 30, 60] {
            assert!(valid_interval(ok), "{ok}");
        }
        for bad in [0, 4, 7, 45, 61, 90] {
            assert!(!valid_interval(bad), "{bad}");
        }
        assert_eq!(slot_at(T0 + Q - 1, 15), T0);
        assert_eq!(slot_at(T0 + Q, 15), T0 + Q);
    }

    /// One job per slot, never two; nothing before the effective time or while
    /// disabled; the slot-derived key makes a repeated submission join the same job.
    #[test]
    fn one_job_per_slot_with_a_slot_derived_key() {
        let (_d, mut c) = open();
        assert_eq!(
            c.schedule_tick(TENANT, &ctx(), T0).unwrap(),
            Tick::Idle,
            "no schedule"
        );
        enable(&c, T0 + Q);
        assert_eq!(
            c.schedule_tick(TENANT, &ctx(), T0 + 1).unwrap(),
            Tick::Idle,
            "before effective"
        );
        let Tick::Submitted {
            job_id, skipped, ..
        } = c.schedule_tick(TENANT, &ctx(), T0 + Q + 5).unwrap()
        else {
            panic!("the first due slot submits");
        };
        assert_eq!(skipped, 0);
        assert_eq!(
            c.schedule_tick(TENANT, &ctx(), T0 + Q + 60_000).unwrap(),
            Tick::Idle,
            "same slot"
        );
        assert_eq!(jobs(&c), 1);
        let job = c.job(TENANT, &job_id).unwrap().unwrap();
        assert_eq!(job.actor, Actor::Service(SERVICE_ACTOR.into()));
        // A crash between recording and submitting repeats the slot key: same job.
        let again = c
            .submit(
                Submission {
                    tenant: TENANT,
                    operation: "collection_start",
                    actor: Actor::Service(SERVICE_ACTOR.into()),
                    idempotency_key: &format!("sched-collection-{}", T0 + Q),
                    expected_epoch: EPOCH,
                    params: &json!({"mode": "recent", "range": null, "app_ids": []}),
                    config_revision: "test-1",
                    credential_revision: None,
                    deadline_ms: T0 + Q * 5,
                    queue_tenant: 64,
                    queue_principal: 64,
                    join_equivalent: false,
                    scheduled_slot: None,
                },
                T0 + Q + 10,
            )
            .unwrap();
        assert!(again.existing && again.job_id == job_id);
        c.conn
            .execute("UPDATE schedules SET enabled=0", [])
            .unwrap();
        assert_eq!(
            c.schedule_tick(TENANT, &ctx(), T0 + 3 * Q).unwrap(),
            Tick::Idle,
            "disabled"
        );
    }

    /// Missed ticks coalesce: while a scheduled collection is live, every due slot is
    /// recorded pending; when it finishes, one catch-up job for the newest slot is
    /// submitted and the older pending slots become skipped. A whole day with a job
    /// that never finishes makes one submission, not 96.
    /// M4 review 2 minor: slots missed during downtime (nothing ran, nothing pending)
    /// are recorded as skipped history; catch-up still makes one job.
    #[test]
    fn slots_missed_during_downtime_are_recorded_as_skipped() {
        let (_d, mut c) = open();
        enable(&c, T0);
        let Tick::Submitted { job_id, .. } = c.schedule_tick(TENANT, &ctx(), T0).unwrap() else {
            panic!("first slot");
        };
        finish(&mut c, &job_id, T0 + 1);
        // The service was down for three slots; the next tick is four intervals later.
        let Tick::Submitted { skipped, .. } = c.schedule_tick(TENANT, &ctx(), T0 + 4 * Q).unwrap()
        else {
            panic!("catch-up");
        };
        assert_eq!(
            skipped, 3,
            "the three missed slots are history, not fetched"
        );
        let recorded: i64 = c
            .conn
            .query_row(
                "SELECT count(*) FROM schedule_slots WHERE state='skipped'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(recorded, 3);
        assert_eq!(jobs(&c), 2, "one catch-up job");
    }

    #[test]
    fn missed_ticks_coalesce_into_one_catch_up_job() {
        let (_d, mut c) = open();
        enable(&c, T0);
        let Tick::Submitted { job_id: first, .. } = c.schedule_tick(TENANT, &ctx(), T0).unwrap()
        else {
            panic!();
        };
        for n in 1..=3 {
            assert!(matches!(
                c.schedule_tick(TENANT, &ctx(), T0 + n * Q).unwrap(),
                Tick::Coalesced { .. }
            ));
        }
        // It finishes within its deadline (four intervals after its slot).
        finish(&mut c, &first, T0 + 3 * Q + 1);
        let Tick::Submitted { skipped, slot, .. } =
            c.schedule_tick(TENANT, &ctx(), T0 + 4 * Q).unwrap()
        else {
            panic!("the catch-up job");
        };
        assert_eq!(slot, slot_name(T0 + 4 * Q), "the newest due slot");
        assert_eq!(skipped, 3, "older pending slots are skipped, never fetched");
        assert_eq!(jobs(&c), 2);
        // A day with a live job that never finishes: no more submissions.
        for n in 5..(5 + 96) {
            c.schedule_tick(TENANT, &ctx(), T0 + n * Q).unwrap();
        }
        assert_eq!(jobs(&c), 2, "never 96 unconditional traversals");
        let completed: i64 = c
            .conn
            .query_row(
                "SELECT count(*) FROM schedule_slots WHERE state='completed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(completed, 1, "the finished slot is completed");
        let state = c.job(TENANT, &first).unwrap().unwrap().state;
        assert_eq!(state, State::Succeeded);
    }

    /// A quarantined store schedules nothing (M3: restore and loss disable schedules
    /// and quarantine the store).
    #[test]
    fn a_quarantined_store_schedules_nothing() {
        let (_d, mut c) = open();
        enable(&c, T0);
        c.conn
            .execute("INSERT INTO meta VALUES ('quarantine','restored')", [])
            .unwrap();
        assert_eq!(c.schedule_tick(TENANT, &ctx(), T0).unwrap(), Tick::Idle);
        assert_eq!(jobs(&c), 0);
    }

    /// M4 review 1 finding 10: changes are effective-dated. A future re-time or disable
    /// never suppresses the ticks before it; the list shows the version in effect now;
    /// completion bookkeeping is kept even when the tick then has nothing to do.
    #[test]
    fn schedule_changes_take_effect_only_at_their_time() {
        let (_d, mut c) = open();
        let (id, _) = c.schedule_set(TENANT, true, Some(15), T0).unwrap();
        // A future disable (in two intervals): the next slot still runs.
        c.schedule_disable(TENANT, &id, T0 + 2 * Q).unwrap();
        let Tick::Submitted { job_id, .. } = c.schedule_tick(TENANT, &ctx(), T0 + 1).unwrap()
        else {
            panic!("the slot before the disable runs");
        };
        let listed = c.schedules_list(TENANT, T0 + 1).unwrap();
        assert_eq!(
            listed[0]["enabled"], true,
            "the disable is not in effect yet"
        );
        let listed = c.schedules_list(TENANT, T0 + 2 * Q).unwrap();
        assert_eq!(listed[0]["enabled"], false, "and is once its time comes");
        // A future re-enable at another time does not suppress anything before it either.
        c.schedule_set(TENANT, true, Some(15), T0 + 4 * Q).unwrap();
        // The job finishes; the next tick falls in the disabled window: nothing is due,
        // but the completed slot is recorded all the same.
        finish(&mut c, &job_id, T0 + Q);
        assert_eq!(
            c.schedule_tick(TENANT, &ctx(), T0 + 2 * Q + 1).unwrap(),
            Tick::Idle
        );
        let completed: i64 = c
            .conn
            .query_row(
                "SELECT count(*) FROM schedule_slots WHERE state='completed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            completed, 1,
            "completion is committed although the tick did nothing"
        );
        assert!(
            matches!(
                c.schedule_tick(TENANT, &ctx(), T0 + 4 * Q).unwrap(),
                Tick::Submitted { .. }
            ),
            "re-enabled at its time"
        );
    }

    /// M4 review 3 minor (the reviewer's probe): downtime history follows each version's
    /// own interval. One slot completes on the 15-minute version, then a 30-minute
    /// version takes effect four quarter-hours later: the three 15-minute slots missed in
    /// between are recorded as skipped.
    #[test]
    fn downtime_history_follows_each_versions_interval() {
        let (_d, mut c) = open();
        enable(&c, T0);
        let Tick::Submitted { job_id, .. } = c.schedule_tick(TENANT, &ctx(), T0).unwrap() else {
            panic!("the first slot");
        };
        finish(&mut c, &job_id, T0 + 1);
        c.schedule_set(TENANT, true, Some(30), T0 + 4 * Q).unwrap();
        let Tick::Submitted { skipped, .. } = c.schedule_tick(TENANT, &ctx(), T0 + 4 * Q).unwrap()
        else {
            panic!("the catch-up slot");
        };
        assert_eq!(skipped, 3, "the 15-minute version's three missed slots");
    }

    /// M4 review 4 minor (the reviewer's probe): several interval changes during
    /// downtime. An hour-aligned 15-minute schedule, a 60-minute version from 00:01 and
    /// a 15-minute version from 00:10; the catch-up at 01:00 records 00:15, 00:30 and
    /// 00:45 as skipped (each version's grid, walked chronologically).
    #[test]
    fn downtime_history_walks_every_version_chronologically() {
        let h = 3_600_000;
        let t = (T0 / h + 1) * h;
        let (_d, mut c) = open();
        enable(&c, t);
        let Tick::Submitted { job_id, .. } = c.schedule_tick(TENANT, &ctx(), t).unwrap() else {
            panic!("the first slot");
        };
        finish(&mut c, &job_id, t + 1);
        c.schedule_set(TENANT, true, Some(60), t + 60_000).unwrap();
        c.schedule_set(TENANT, true, Some(15), t + 10 * 60_000)
            .unwrap();
        let Tick::Submitted { skipped, .. } = c.schedule_tick(TENANT, &ctx(), t + h).unwrap()
        else {
            panic!("the catch-up slot");
        };
        assert_eq!(skipped, 3, "00:15, 00:30 and 00:45");
        let slots: Vec<String> = c
            .conn
            .prepare("SELECT slot FROM schedule_slots WHERE state='skipped' ORDER BY slot")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let want: Vec<String> = [15, 30, 45]
            .iter()
            .map(|m| slot_name(t + m * 60_000))
            .collect();
        assert_eq!(slots, want);
    }

    fn slot_row(c: &Control, slot_ms: i64) -> Option<(Option<String>, String)> {
        c.conn
            .query_row(
                "SELECT job_id, state FROM schedule_slots WHERE tenant=?1 AND name=?2 AND slot=?3",
                params![TENANT, SCHEDULE, slot_name(slot_ms)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .unwrap()
    }

    fn scheduled_slot(c: &Control, job: &str) -> Option<String> {
        c.conn
            .query_row(
                "SELECT scheduled_slot FROM jobs WHERE job_id=?1",
                [job],
                |r| r.get(0),
            )
            .unwrap()
    }

    /// Every scheduler job names its slot, and that slot's row names the job.
    fn assert_attribution(c: &Control) {
        let rows: Vec<(String, Option<String>)> = {
            let mut st = c
                .conn
                .prepare("SELECT job_id, scheduled_slot FROM jobs WHERE service_actor=?1")
                .unwrap();
            st.query_map([SERVICE_ACTOR], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        for (job, slot) in rows {
            let slot = slot.unwrap_or_else(|| panic!("job {job} lost its slot"));
            let named: String = c
                .conn
                .query_row(
                    "SELECT job_id FROM schedule_slots WHERE tenant=?1 AND name=?2 AND slot=?3",
                    params![TENANT, SCHEDULE, slot],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(named, job, "slot {slot} names its job");
        }
    }

    /// Fail the slot association of a submission (both the insert and the upsert
    /// update of an existing pending row), on this connection only.
    fn fail_slot_association(c: &Control) {
        c.conn
            .execute_batch(
                "CREATE TEMP TRIGGER fail_slot_insert BEFORE INSERT ON schedule_slots
                   WHEN NEW.state IN ('submitted','completed') BEGIN SELECT RAISE(ABORT,'injected'); END;
                 CREATE TEMP TRIGGER fail_slot_update BEFORE UPDATE ON schedule_slots
                   WHEN NEW.state IN ('submitted','completed') BEGIN SELECT RAISE(ABORT,'injected'); END;
                 CREATE TEMP TRIGGER fail_job_slot BEFORE UPDATE OF scheduled_slot ON jobs
                   BEGIN SELECT RAISE(ABORT,'injected'); END;",
            )
            .unwrap();
    }

    fn reopen(d: &Dir, c: Control) -> Control {
        drop(c);
        Control::open(&d.0, TENANT, EPOCH, &[1u8; 32], 1_000).unwrap()
    }

    #[test]
    fn a_failed_slot_association_commits_no_job_and_keeps_the_slot_in_history() {
        // The reviewer's probe: the association fails after the job would have been
        // committed; restart in a later slot, finish, tick again.
        let (d, mut c) = open();
        enable(&c, T0);
        fail_slot_association(&c);
        assert!(c.schedule_tick(TENANT, &ctx(), T0 + Q + 5).is_err());
        assert_eq!(jobs(&c), 0, "the submission rolled back whole");
        let keys: i64 = c
            .conn
            .query_row("SELECT count(*) FROM idempotency", [], |r| r.get(0))
            .unwrap();
        assert_eq!(keys, 0, "no orphaned idempotency record");
        assert_eq!(slot_row(&c, T0 + Q), Some((None, "pending".into())));
        let mut c = reopen(&d, c);
        let Tick::Submitted {
            job_id, skipped, ..
        } = c.schedule_tick(TENANT, &ctx(), T0 + 2 * Q + 5).unwrap()
        else {
            panic!("the next slot submits");
        };
        assert_eq!(skipped, 1, "the failed slot is history, never fetched");
        assert_eq!(slot_row(&c, T0 + Q), Some((None, "skipped".into())));
        assert_eq!(scheduled_slot(&c, &job_id), Some(slot_name(T0 + 2 * Q)));
        finish(&mut c, &job_id, T0 + 2 * Q + 60_000);
        assert_eq!(
            c.schedule_tick(TENANT, &ctx(), T0 + 2 * Q + 120_000)
                .unwrap(),
            Tick::Idle
        );
        assert_eq!(
            slot_row(&c, T0 + 2 * Q),
            Some((Some(job_id.clone()), "completed".into()))
        );
        assert_eq!(jobs(&c), 1);
        assert_attribution(&c);
    }

    #[test]
    fn a_retry_in_the_same_slot_after_a_failed_association_submits_and_associates() {
        let (d, c) = open();
        enable(&c, T0);
        fail_slot_association(&c);
        let mut c = c;
        assert!(c.schedule_tick(TENANT, &ctx(), T0 + Q + 5).is_err());
        let mut c = reopen(&d, c);
        let Tick::Submitted { job_id, .. } =
            c.schedule_tick(TENANT, &ctx(), T0 + Q + 60_000).unwrap()
        else {
            panic!("the same slot submits once the fault clears");
        };
        assert_eq!(
            slot_row(&c, T0 + Q),
            Some((Some(job_id.clone()), "submitted".into()))
        );
        assert_eq!(scheduled_slot(&c, &job_id), Some(slot_name(T0 + Q)));
        assert_eq!(jobs(&c), 1);
        assert_attribution(&c);
    }

    #[test]
    fn a_job_stranded_without_its_slot_is_reassociated_after_the_slot_has_passed() {
        // A store written by the earlier two-step submission: the scheduler job
        // committed with its slot key but neither its slot row nor `scheduled_slot`.
        let (d, mut c) = open();
        enable(&c, T0);
        let stranded = c
            .submit(
                Submission {
                    tenant: TENANT,
                    operation: "collection_start",
                    actor: Actor::Service(SERVICE_ACTOR.into()),
                    idempotency_key: &format!("sched-collection-{}", T0 + Q),
                    expected_epoch: EPOCH,
                    params: &json!({"mode": "recent", "range": null, "app_ids": []}),
                    config_revision: "test-1",
                    credential_revision: None,
                    deadline_ms: T0 + Q * 5,
                    queue_tenant: 64,
                    queue_principal: 64,
                    join_equivalent: false,
                    scheduled_slot: None,
                },
                T0 + Q + 5,
            )
            .unwrap()
            .job_id;
        assert_eq!(slot_row(&c, T0 + Q), None);
        let mut c = reopen(&d, c);
        // The next slot: the stranded job is still live, so the new slot coalesces,
        // and the original slot is re-associated first.
        assert!(matches!(
            c.schedule_tick(TENANT, &ctx(), T0 + 2 * Q + 5).unwrap(),
            Tick::Coalesced { .. }
        ));
        assert_eq!(
            slot_row(&c, T0 + Q),
            Some((Some(stranded.clone()), "submitted".into()))
        );
        assert_eq!(scheduled_slot(&c, &stranded), Some(slot_name(T0 + Q)));
        finish(&mut c, &stranded, T0 + 2 * Q + 60_000);
        let Tick::Submitted { job_id, .. } = c
            .schedule_tick(TENANT, &ctx(), T0 + 2 * Q + 120_000)
            .unwrap()
        else {
            panic!("the coalesced slot submits once the stranded job finishes");
        };
        assert_eq!(
            slot_row(&c, T0 + Q),
            Some((Some(stranded.clone()), "completed".into()))
        );
        assert_ne!(job_id, stranded);
        assert_attribution(&c);
        // A job already terminal when repaired is recorded completed directly.
        let (_d2, mut c2) = open();
        enable(&c2, T0);
        let done = c2
            .submit(
                Submission {
                    tenant: TENANT,
                    operation: "collection_start",
                    actor: Actor::Service(SERVICE_ACTOR.into()),
                    idempotency_key: &format!("sched-collection-{}", T0 + Q),
                    expected_epoch: EPOCH,
                    params: &json!({"mode": "recent", "range": null, "app_ids": []}),
                    config_revision: "test-1",
                    credential_revision: None,
                    deadline_ms: T0 + Q * 5,
                    queue_tenant: 64,
                    queue_principal: 64,
                    join_equivalent: false,
                    scheduled_slot: None,
                },
                T0 + Q + 5,
            )
            .unwrap()
            .job_id;
        finish(&mut c2, &done, T0 + Q + 60_000);
        assert_eq!(c2.repair_slot_associations(TENANT).unwrap(), 1);
        assert_eq!(
            slot_row(&c2, T0 + Q),
            Some((Some(done.clone()), "completed".into()))
        );
        assert_eq!(
            c2.repair_slot_associations(TENANT).unwrap(),
            0,
            "idempotent"
        );
    }
}
