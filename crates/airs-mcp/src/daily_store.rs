//! Daily reports, revisions and daily enrichment (contract §8, plan M6.2).
//!
//! - At `00:00 America/Chicago` the `daily_report` schedule queues the previous local
//!   day's report with half-open UTC bounds (23, 24 or 25 hours long).
//! - After downtime, one catch-up report per missed day, serially (one submission per
//!   tick, oldest first, at most 31 days back), from retained history only; catch-up
//!   reports get no revision window.
//! - Revisions: opportunities at +4..+24 elapsed hours after scheduled midnight
//!   (`revision_opportunities(4)`); delayed opportunities coalesce to the latest due
//!   one, whose evidence cutoff stays at its own offset (the final cutoff stays at +24
//!   even when processed later). The executor applies the allowance and
//!   identical-digest suppression.
//! - `daily_enrichment` (disabled until explicitly enabled) queues one enrichment job
//!   for the previous day once the day's report is queued and 15 minutes have passed
//!   (after the first post-midnight collection tick).
//!
//! Every submission is the service actor's, under a slot-derived idempotency key, so
//! a restart rejoins the same job; slots are recorded in `schedule_slots`.

use rusqlite::{OptionalExtension, params};
use serde_json::json;

use crate::control::{Actor, Control, Refused, Submission};
use crate::schedule_store::TickContext;

pub const DAILY: &str = "daily_report";
pub const ENRICH: &str = "daily_enrichment";
pub const ACTOR: &str = "scheduler";
const CATCH_UP_DAYS: i64 = 31;
const ENRICH_DELAY_MS: i64 = 15 * 60_000;

fn storage(_: rusqlite::Error) -> Refused {
    Refused {
        code: airs_runtime_core::dto::ErrorCode::StorageUnavailable,
        reason: "control store unavailable",
        retry_after_seconds: None,
    }
}

/// A deterministic `r1_` report ID for a tenant's daily report of a local date.
pub fn daily_report_id(tenant: &str, date: &str) -> String {
    use sha2::Digest;
    let h = sha2::Sha256::digest(format!("daily\u{1f}{tenant}\u{1f}{date}").as_bytes());
    let mut b = [0u8; 16];
    b.copy_from_slice(&h[..16]);
    format!("r1_{}", uuid::Builder::from_random_bytes(b).into_uuid())
}

/// What one daily tick submitted.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct DailyTick {
    pub reports: Vec<String>,
    pub revisions: Vec<String>,
    pub enrichments: Vec<String>,
}

/// Whether the version in effect at `at_ms` is enabled.
fn on_at(spec: &serde_json::Value, base: i64, at_ms: i64) -> bool {
    crate::schedule_store::version_at(spec, base, at_ms).is_some_and(|v| v.enabled)
}

impl Control {
    /// The schedule `name`'s effective-dated versions (M4 round 2): `Some(spec, base)`
    /// when the schedule exists; whether it is on at an instant is the version in effect
    /// then ([`on_at`]).
    fn schedule_enabled(
        &self,
        tenant: &str,
        name: &str,
    ) -> Result<Option<(serde_json::Value, i64)>, Refused> {
        let row: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT spec_json, effective_ms FROM schedules WHERE tenant=?1 AND name=?2",
                params![tenant, name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        Ok(row.map(|(spec, eff)| (serde_json::from_str(&spec).unwrap_or_default(), eff)))
    }

    fn slot_taken(&self, tenant: &str, name: &str, slot: &str) -> Result<bool, Refused> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schedule_slots WHERE tenant=?1 AND name=?2 AND slot=?3)",
                params![tenant, name, slot],
                |r| r.get(0),
            )
            .map_err(storage)
    }

    #[allow(clippy::too_many_arguments)]
    fn submit_slot(
        &mut self,
        tenant: &str,
        ctx: &TickContext<'_>,
        name: &str,
        slot: &str,
        operation: &str,
        params: &serde_json::Value,
        now_ms: i64,
    ) -> Result<String, Refused> {
        // A bounded key (16–64 characters) derived from the schedule and slot.
        let key = {
            use sha2::Digest;
            let h = sha2::Sha256::digest(format!("{name}\u{1f}{slot}").as_bytes());
            format!("sched-{}", crate::reports::hex(&h[..20]))
        };
        let submitted = self.submit(
            Submission {
                tenant,
                operation,
                actor: Actor::Service(ACTOR.into()),
                idempotency_key: &key,
                expected_epoch: ctx.epoch,
                params,
                config_revision: ctx.config_revision,
                credential_revision: ctx.credential_revision,
                // Reports default to a 72 h deadline (contract §10).
                deadline_ms: now_ms + 72 * 3_600_000,
                queue_tenant: ctx.queue_tenant,
                queue_principal: ctx.queue_tenant,
                join_equivalent: false,
                // The slot association commits with the submission (M4 review 9).
                scheduled_slot: Some((name, slot)),
            },
            now_ms,
        )?;
        Ok(submitted.job_id)
    }

    /// One daily tick (worker thread). Nothing runs while the store is quarantined.
    pub fn daily_tick(
        &mut self,
        tenant: &str,
        ctx: &TickContext<'_>,
        now_ms: i64,
    ) -> Result<DailyTick, Refused> {
        let mut out = DailyTick::default();
        if self.quarantine()?.is_some() {
            return Ok(out);
        }
        let Some((daily_spec, daily_base)) = self.schedule_enabled(tenant, DAILY)? else {
            return Ok(out);
        };
        let tz = airs_runtime_core::schedule::report_tz()
            .map_err(|_| storage(rusqlite::Error::InvalidQuery))?;
        let Ok(now) = jiff::Timestamp::from_millisecond(now_ms) else {
            return Ok(out);
        };
        let today = now.to_zoned(tz.clone()).date();
        let midnight = |d: jiff::civil::Date| -> Option<i64> {
            d.to_zoned(tz.clone())
                .ok()
                .map(|z| z.timestamp().as_millisecond())
        };
        // Reports: today's midnight, plus missed days, oldest first, one per tick.
        let mut submitted_report = false;
        for back in (0..CATCH_UP_DAYS).rev() {
            let Ok(day) = today.checked_sub(jiff::Span::new().days(back)) else {
                continue;
            };
            let Some(scheduled) = midnight(day) else {
                continue;
            };
            if !on_at(&daily_spec, daily_base, scheduled) {
                continue;
            }
            let slot = day.to_string();
            if self.slot_taken(tenant, DAILY, &slot)? {
                continue;
            }
            let Ok(period) = airs_runtime_core::schedule::report_period_for_midnight(day, &tz)
            else {
                continue;
            };
            // A missed day is identified by its America/Chicago calendar date, not by 24
            // elapsed hours: a spring-forward day is 23 hours long (M6 review 1
            // finding 3). Revision opportunities keep their elapsed-hour arithmetic.
            let catch_up = day < today;
            let kind = if catch_up { "catch_up" } else { "daily" };
            let params = json!({
                "report_id": daily_report_id(tenant, &slot),
                "kind": kind,
                "since_ms": period.start.as_millisecond(),
                "until_ms": period.end.as_millisecond(),
                "app_ids": [],
                "title": format!("AIRS runtime daily report {}", day.checked_sub(jiff::Span::new().days(1)).map(|d| d.to_string()).unwrap_or_default()),
                "scheduled_ms": scheduled,
            });
            let op = if catch_up { "catch_up" } else { "daily_report" };
            out.reports
                .push(self.submit_slot(tenant, ctx, DAILY, &slot, op, &params, now_ms)?);
            submitted_report = true;
            break;
        }
        // Revisions of daily (not catch-up) reports in or just past their window.
        let opportunities = airs_runtime_core::schedule::revision_opportunities(
            crate::reports::REVISION_INTERVAL_HOURS,
        )
        .unwrap_or_default();
        let open: Vec<(String, i64, i64, i64, String, String)> = {
            let mut stmt = self
                .conn
                .prepare("SELECT report_id, scheduled_ms, since_ms, until_ms, app_ids_json, title FROM reports WHERE tenant=?1 AND kind='daily' AND scheduled_ms IS NOT NULL AND scheduled_ms>?2 AND latest_version IS NOT NULL AND NOT EXISTS (SELECT 1 FROM tombstones t WHERE t.tenant=?1 AND t.kind='report' AND t.object_id=reports.report_id) ORDER BY scheduled_ms")
                .map_err(storage)?;
            stmt.query_map(params![tenant, now_ms - 48 * 3_600_000], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })
            .map_err(storage)?
            .collect::<Result<_, _>>()
            .map_err(storage)?
        };
        for (report, scheduled, since, until, apps_json, title) in open {
            // A revision carries its original report's own metadata (M6 review 4).
            let apps: serde_json::Value =
                serde_json::from_str(&apps_json).unwrap_or_else(|_| serde_json::json!([]));
            let elapsed = ((now_ms - scheduled) / 3_600_000).max(0) as u32;
            let Some(opp) = airs_runtime_core::schedule::due_opportunity(&opportunities, elapsed)
            else {
                continue;
            };
            let slot = format!("rev:{report}:{opp}");
            if self.slot_taken(tenant, DAILY, &slot)? {
                continue;
            }
            let params = json!({
                "report_id": report,
                "kind": "revision",
                "since_ms": since,
                "until_ms": until,
                "app_ids": apps,
                "title": title,
                "scheduled_ms": scheduled,
                "cutoff_ms": scheduled + i64::from(opp) * 3_600_000,
                "final_revision": opp == airs_runtime_core::schedule::WINDOW_HOURS,
            });
            out.revisions
                .push(self.submit_slot(tenant, ctx, DAILY, &slot, "revision", &params, now_ms)?);
        }
        // Daily enrichment of the previous day, when explicitly enabled.
        if let Some((enrich_spec, enrich_base)) = self.schedule_enabled(tenant, ENRICH)? {
            let slot = today.to_string();
            if let Some(scheduled) = midnight(today)
                && on_at(&enrich_spec, enrich_base, scheduled)
                && now_ms >= scheduled + ENRICH_DELAY_MS
                && self.slot_taken(tenant, DAILY, &slot)?
                && !self.slot_taken(tenant, ENRICH, &slot)?
                && let Ok(period) =
                    airs_runtime_core::schedule::report_period_for_midnight(today, &tz)
            {
                let params = json!({"since_ms": period.start.as_millisecond(), "until_ms": period.end.as_millisecond(), "max_reports": 5});
                out.enrichments.push(self.submit_slot(
                    tenant,
                    ctx,
                    ENRICH,
                    &slot,
                    "enrichment_start",
                    &params,
                    now_ms,
                )?);
            }
        }
        let _ = submitted_report;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::DirBuilderExt;

    const TENANT: &str = "svc-test";
    const EPOCH: &str = "0a1b2c3d";

    struct Dir(std::path::PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn open() -> (Dir, Control) {
        let d = std::env::temp_dir().join(format!("airs-mcp-daily-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new().mode(0o700).create(&d).unwrap();
        let c = Control::open(&d, TENANT, EPOCH, &[1u8; 32], 1_000).unwrap();
        (Dir(d), c)
    }

    fn ctx() -> TickContext<'static> {
        TickContext {
            interval_minutes: 15,
            epoch: EPOCH,
            config_revision: "t",
            credential_revision: None,
            queue_tenant: 64,
        }
    }

    fn chicago_ms(y: i16, m: i8, d: i8, h: i8) -> i64 {
        let tz = airs_runtime_core::schedule::report_tz().unwrap();
        jiff::civil::date(y, m, d)
            .at(h, 0, 0, 0)
            .to_zoned(tz)
            .unwrap()
            .timestamp()
            .as_millisecond()
    }

    fn params_of(c: &Control, job: &str) -> serde_json::Value {
        c.params(job).unwrap()
    }

    /// Midnight America/Chicago queues the previous local day with half-open UTC
    /// bounds; the days around both 2026 DST changes are 23 and 25 hours long; one job
    /// per day under a slot key (a second tick submits nothing new).
    #[test]
    fn daily_reports_cover_the_previous_local_day_across_dst() {
        for (day, hours) in [((2026, 3, 9), 23), ((2026, 11, 2), 25), ((2026, 6, 2), 24)] {
            let (_d, mut c) = open();
            let at = chicago_ms(day.0, day.1, day.2, 0) + 60_000;
            c.schedule_set_kind(TENANT, DAILY, true, None, at - 3_600_000)
                .unwrap();
            let t = c.daily_tick(TENANT, &ctx(), at).unwrap();
            assert_eq!(t.reports.len(), 1, "{day:?}");
            let p = params_of(&c, &t.reports[0]);
            assert_eq!(p["kind"], "daily");
            let (since, until) = (
                p["since_ms"].as_i64().unwrap(),
                p["until_ms"].as_i64().unwrap(),
            );
            assert_eq!((until - since) / 3_600_000, hours, "{day:?}");
            assert_eq!(
                until,
                chicago_ms(day.0, day.1, day.2, 0),
                "the period ends at local midnight"
            );
            assert_eq!(p["scheduled_ms"].as_i64().unwrap(), until);
            assert!(
                c.daily_tick(TENANT, &ctx(), at + 60_000)
                    .unwrap()
                    .reports
                    .is_empty(),
                "one per day"
            );
        }
    }

    /// A daily submission and its slot association commit together (the review-9
    /// pattern): a failed association commits no job, and the next tick submits the
    /// slot with `scheduled_slot` set and the slot row naming the job.
    #[test]
    fn a_daily_slot_association_commits_with_its_job() {
        let (_d, mut c) = open();
        let at = chicago_ms(2026, 6, 2, 0) + 60_000;
        c.schedule_set_kind(TENANT, DAILY, true, None, at - 3_600_000)
            .unwrap();
        c.conn
            .execute_batch(
                "CREATE TEMP TRIGGER fail_slot BEFORE INSERT ON schedule_slots
                   BEGIN SELECT RAISE(ABORT,'injected'); END;",
            )
            .unwrap();
        assert!(c.daily_tick(TENANT, &ctx(), at).is_err());
        let jobs: i64 = c
            .conn
            .query_row("SELECT count(*) FROM jobs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(jobs, 0, "no job without its slot");
        c.conn.execute_batch("DROP TRIGGER fail_slot").unwrap();
        let t = c.daily_tick(TENANT, &ctx(), at + 60_000).unwrap();
        assert_eq!(t.reports.len(), 1);
        let job = &t.reports[0];
        let (slot, named): (Option<String>, String) = c
            .conn
            .query_row(
                "SELECT j.scheduled_slot, s.job_id FROM jobs j JOIN schedule_slots s ON s.slot=j.scheduled_slot AND s.name=?2 WHERE j.job_id=?1",
                params![job, DAILY],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(slot.is_some());
        assert_eq!(&named, job);
    }

    /// A report that lost a version to `artifact_delete` carries a `report` tombstone and
    /// is never revised automatically again (no deleted content is re-published).
    #[test]
    fn a_report_with_a_deleted_version_is_never_revised_again() {
        let (_d, mut c) = open();
        let midnight = chicago_ms(2026, 6, 2, 0);
        c.schedule_set_kind(TENANT, DAILY, true, None, midnight - 1)
            .unwrap();
        let report = c.daily_tick(TENANT, &ctx(), midnight + 1).unwrap().reports[0].clone();
        let p = params_of(&c, &report);
        let rid = p["report_id"].as_str().unwrap().to_owned();
        c.ensure_report(
            TENANT,
            &rid,
            "daily",
            (
                p["since_ms"].as_i64().unwrap(),
                p["until_ms"].as_i64().unwrap(),
            ),
            &[],
            "",
            Some(midnight),
            midnight,
        )
        .unwrap();
        c.conn.execute("UPDATE reports SET latest_version='v1_00000000-0000-4000-8000-000000000001' WHERE report_id=?1", [&rid]).unwrap();
        c.conn
            .execute(
                "INSERT INTO tombstones VALUES (?1,'report',?2,1,?3)",
                params![TENANT, rid, report],
            )
            .unwrap();
        assert!(
            c.daily_tick(TENANT, &ctx(), midnight + 9 * 3_600_000)
                .unwrap()
                .revisions
                .is_empty(),
            "a tombstoned report is not revised"
        );
    }

    /// After downtime: one catch-up report per missed day, oldest first, one per tick;
    /// catch-up reports are never revised.
    #[test]
    fn missed_days_catch_up_serially() {
        let (_d, mut c) = open();
        let start = chicago_ms(2026, 6, 1, 0);
        c.schedule_set_kind(TENANT, DAILY, true, None, start)
            .unwrap();
        let now = chicago_ms(2026, 6, 4, 2);
        let mut kinds = vec![];
        for _ in 0..5 {
            for job in c.daily_tick(TENANT, &ctx(), now).unwrap().reports {
                kinds.push(params_of(&c, &job)["kind"].as_str().unwrap().to_owned());
            }
        }
        assert_eq!(
            kinds,
            ["catch_up", "catch_up", "catch_up", "daily"],
            "June 1–3 catch up, June 4 is today's"
        );
    }

    /// Revision opportunities coalesce: a report whose original is published gets one
    /// revision for the latest due opportunity, with that opportunity's cutoff; the +24
    /// final keeps its +24 cutoff when processed late; each opportunity runs once.
    #[test]
    fn revisions_coalesce_to_the_latest_due_opportunity() {
        let (_d, mut c) = open();
        let midnight = chicago_ms(2026, 6, 2, 0);
        c.schedule_set_kind(TENANT, DAILY, true, None, midnight - 1)
            .unwrap();
        let report = c.daily_tick(TENANT, &ctx(), midnight + 1).unwrap().reports[0].clone();
        let p = params_of(&c, &report);
        let rid = p["report_id"].as_str().unwrap().to_owned();
        c.ensure_report(
            TENANT,
            &rid,
            "daily",
            (
                p["since_ms"].as_i64().unwrap(),
                p["until_ms"].as_i64().unwrap(),
            ),
            // The original as its executor initialises it: the daily job's own generated
            // title and applications (an empty title masked M6 review 4's regression).
            &serde_json::from_value::<Vec<String>>(p["app_ids"].clone()).unwrap_or_default(),
            p["title"].as_str().unwrap(),
            Some(midnight),
            midnight,
        )
        .unwrap();
        assert!(
            !p["title"].as_str().unwrap().is_empty(),
            "daily reports carry a title"
        );
        c.conn.execute("UPDATE reports SET latest_version='v1_00000000-0000-4000-8000-000000000001' WHERE report_id=?1", [&rid]).unwrap();
        assert!(
            c.daily_tick(TENANT, &ctx(), midnight + 3 * 3_600_000)
                .unwrap()
                .revisions
                .is_empty(),
            "before +4"
        );
        let r = c
            .daily_tick(TENANT, &ctx(), midnight + 9 * 3_600_000)
            .unwrap()
            .revisions;
        assert_eq!(r.len(), 1, "+4 and +8 coalesce to +8");
        // Each revision carries its original report's metadata, so the executor's report
        // row is the original's, never a conflict.
        let rev = params_of(&c, &r[0]);
        assert_eq!(rev["title"], p["title"]);
        assert_eq!(rev["app_ids"], p["app_ids"]);
        assert_eq!(
            params_of(&c, &r[0])["cutoff_ms"].as_i64().unwrap(),
            midnight + 8 * 3_600_000
        );
        assert!(
            c.daily_tick(TENANT, &ctx(), midnight + 9 * 3_600_000 + 1)
                .unwrap()
                .revisions
                .is_empty(),
            "once"
        );
        let late = c
            .daily_tick(TENANT, &ctx(), midnight + 30 * 3_600_000)
            .unwrap()
            .revisions;
        assert_eq!(late.len(), 1);
        let fin = params_of(&c, &late[0]);
        assert_eq!(
            fin["cutoff_ms"].as_i64().unwrap(),
            midnight + 24 * 3_600_000,
            "the final cutoff stays at +24"
        );
        assert_eq!(fin["final_revision"], true);
    }

    /// Daily enrichment is off unless enabled; enabled, it follows the day's report.
    #[test]
    fn daily_enrichment_runs_only_when_enabled_after_the_report() {
        let (_d, mut c) = open();
        let midnight = chicago_ms(2026, 6, 2, 0);
        c.schedule_set_kind(TENANT, DAILY, true, None, midnight - 1)
            .unwrap();
        let t = c
            .daily_tick(TENANT, &ctx(), midnight + 20 * 60_000)
            .unwrap();
        assert!(t.enrichments.is_empty(), "disabled by default");
        c.schedule_set_kind(TENANT, ENRICH, true, None, midnight - 1)
            .unwrap();
        let t = c
            .daily_tick(TENANT, &ctx(), midnight + 21 * 60_000)
            .unwrap();
        assert_eq!(t.enrichments.len(), 1);
        let p = params_of(&c, &t.enrichments[0]);
        assert_eq!(p["until_ms"].as_i64().unwrap(), midnight);
        assert!(
            c.daily_tick(TENANT, &ctx(), midnight + 22 * 60_000)
                .unwrap()
                .enrichments
                .is_empty()
        );
    }

    #[test]
    fn review_missed_spring_forward_day_is_catch_up() {
        let (_d, mut c) = open();
        let yesterday = chicago_ms(2026, 3, 8, 0);
        let today = chicago_ms(2026, 3, 9, 0);
        assert_eq!(today - yesterday, 23 * 3_600_000);
        c.schedule_set_kind(TENANT, DAILY, true, None, yesterday - 1)
            .unwrap();
        let tick = c.daily_tick(TENANT, &ctx(), today).unwrap();
        let p = params_of(&c, &tick.reports[0]);
        assert_eq!(p["scheduled_ms"], yesterday);
        assert_eq!(
            p["kind"], "catch_up",
            "yesterday's missed daily slot must have no new revision window"
        );
    }

    /// The fall-back counterpart: a missed November 1, 2026 slot is 25 hours old at
    /// November 2 midnight and is a catch-up; that day's own slot is the daily report.
    #[test]
    fn missed_fall_back_day_is_catch_up_and_today_is_daily() {
        let (_d, mut c) = open();
        let yesterday = chicago_ms(2026, 11, 1, 0);
        let today = chicago_ms(2026, 11, 2, 0);
        assert_eq!(today - yesterday, 25 * 3_600_000);
        c.schedule_set_kind(TENANT, DAILY, true, None, yesterday - 1)
            .unwrap();
        let tick = c.daily_tick(TENANT, &ctx(), today).unwrap();
        let p = params_of(&c, &tick.reports[0]);
        assert_eq!(
            (p["scheduled_ms"].as_i64(), p["kind"].as_str()),
            (Some(yesterday), Some("catch_up"))
        );
        let tick = c.daily_tick(TENANT, &ctx(), today + 60_000).unwrap();
        let p = params_of(&c, &tick.reports[0]);
        assert_eq!(
            (p["scheduled_ms"].as_i64(), p["kind"].as_str()),
            (Some(today), Some("daily"))
        );
    }
}
