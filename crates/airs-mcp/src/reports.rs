//! Historical reports (plan M6, contract §5, §8, §9.2 C5–C7): a report is a half-open
//! UTC period; each version is an immutable private bundle (`report.html`,
//! `evidence.json`, `review.md`, `manifest.json`) rendered from the archive alone by
//! the CLI's own renderer. Rendering, revision and retrieval make zero network calls.
//!
//! Publication (crash points C5–C7):
//! 1. render into a private staging directory and fsync every file (C5: an unpublished
//!    stage is discarded on recovery);
//! 2. recheck the `reports` fence: a prune or deletion that advanced it since rendering
//!    began discards the stage (C7: stale renderers stop);
//! 3. rename the stage into `reports/<report_id>/<version_id>/` (immutable);
//! 4. one control transaction rechecks the fence, records the version and its
//!    publication and moves the latest pointer.
//!
//! A crash between 3 and 4 leaves a bundle with no control row (C6): recovery
//! reattaches it from its own manifest when the report exists and the fence still
//! matches, and otherwise quarantines it; a fenced bundle is never reattached.

use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use airs_runtime_core::dto::ErrorCode;

use crate::control::{Control, Refused};
use crate::run::Shared;
use crate::worker::{Executor, JobContext, Outcome};

pub const FILES: [&str; 3] = ["report.html", "evidence.json", "review.md"];
pub const MANIFEST: &str = "manifest.json";
/// Contract §6 bundle bounds.
pub const MAX_HTML: usize = 32 * 1024 * 1024;
pub const MAX_BUNDLE: usize = 64 * 1024 * 1024;
/// Default automatic revisions: every 4 elapsed hours, allowance 6 (contract §6, §8).
pub const REVISION_INTERVAL_HOURS: u32 = 4;
pub const REVISION_ALLOWANCE: u32 = 6;
pub const SUMMARY_MAX: usize = 8192;
const FENCE: &str = "reports";

fn refused(code: ErrorCode, reason: &'static str) -> Refused {
    Refused {
        code,
        reason,
        retry_after_seconds: None,
    }
}

fn storage(_: rusqlite::Error) -> Refused {
    refused(ErrorCode::StorageUnavailable, "control store unavailable")
}

fn io(_: std::io::Error) -> Refused {
    refused(ErrorCode::StorageUnavailable, "report storage I/O failed")
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `r1_`/`v1_` identifiers (frozen patterns: lowercase UUIDv4).
pub fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4())
}

/// A report's period and scope.
#[derive(Clone, Debug, PartialEq)]
pub struct ReportRow {
    pub report_id: String,
    pub kind: String,
    pub since_ms: i64,
    pub until_ms: i64,
    pub app_ids: Vec<String>,
    pub title: String,
    pub scheduled_ms: Option<i64>,
    pub latest_version: Option<String>,
    pub automatic_revisions: i64,
}

/// One published version.
#[derive(Clone, Debug, PartialEq)]
pub struct VersionRow {
    pub version_id: String,
    pub report_id: String,
    pub version: i64,
    pub digest: String,
    pub manifest: Value,
    pub summary: String,
}

impl Control {
    /// Create the report row if it does not exist (idempotent by report ID).
    #[allow(clippy::too_many_arguments)]
    pub fn ensure_report(
        &mut self,
        tenant: &str,
        report_id: &str,
        kind: &str,
        (since_ms, until_ms): (i64, i64),
        app_ids: &[String],
        title: &str,
        scheduled_ms: Option<i64>,
        now_ms: i64,
    ) -> Result<(), Refused> {
        let apps = serde_json::to_string(app_ids).unwrap_or_else(|_| "[]".into());
        self.conn
            .execute(
                "INSERT INTO reports VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL,0,?9) ON CONFLICT DO NOTHING",
                params![
                    report_id,
                    tenant,
                    kind,
                    since_ms,
                    until_ms,
                    apps,
                    title,
                    scheduled_ms,
                    now_ms
                ],
            )
            .map_err(storage)?;
        // An existing row must describe the same report: never silently keep another
        // report's period, scope or title under this ID (M6 review 3).
        let same: bool = self
            .conn
            .query_row(
                "SELECT tenant=?2 AND kind=?3 AND since_ms=?4 AND until_ms=?5 AND app_ids_json=?6 AND title=?7 FROM reports WHERE report_id=?1",
                params![report_id, tenant, kind, since_ms, until_ms, apps, title],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if !same {
            return Err(refused(
                ErrorCode::Conflict,
                "the report ID already names a different report",
            ));
        }
        Ok(())
    }

    /// Test support: spend a report's automatic-revision allowance.
    #[doc(hidden)]
    pub fn exhaust_revisions_for_tests(&mut self, tenant: &str, report_id: &str) {
        let _ = self.conn.execute(
            "UPDATE reports SET automatic_revisions=1000 WHERE tenant=?1 AND report_id=?2",
            params![tenant, report_id],
        );
    }

    pub fn report(&self, tenant: &str, report_id: &str) -> Result<Option<ReportRow>, Refused> {
        self.conn
            .query_row(
                "SELECT report_id,kind,since_ms,until_ms,app_ids_json,title,scheduled_ms,latest_version,automatic_revisions FROM reports WHERE tenant=?1 AND report_id=?2",
                params![tenant, report_id],
                row_report,
            )
            .optional()
            .map_err(storage)
    }

    /// The daily (or catch-up) report of a period, if one exists.
    pub fn daily_report(&self, tenant: &str, since_ms: i64) -> Result<Option<ReportRow>, Refused> {
        self.conn
            .query_row(
                "SELECT report_id,kind,since_ms,until_ms,app_ids_json,title,scheduled_ms,latest_version,automatic_revisions FROM reports WHERE tenant=?1 AND since_ms=?2 AND kind IN ('daily','catch_up')",
                params![tenant, since_ms],
                row_report,
            )
            .optional()
            .map_err(storage)
    }

    pub fn version(&self, tenant: &str, version_id: &str) -> Result<Option<VersionRow>, Refused> {
        self.conn
            .query_row(
                "SELECT v.version_id,v.report_id,v.version,v.digest,v.manifest_json,v.summary FROM report_versions v JOIN reports r USING(report_id) WHERE r.tenant=?1 AND v.version_id=?2",
                params![tenant, version_id],
                row_version,
            )
            .optional()
            .map_err(storage)
    }

    /// Versions newest first, optionally filtered by kind and by period overlap.
    pub fn versions(
        &self,
        tenant: &str,
        kind: Option<&str>,
        range: Option<(i64, i64)>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<VersionRow>, Refused> {
        let (a, b) = range.unwrap_or((i64::MIN, i64::MAX));
        let mut stmt = self
            .conn
            .prepare(
                "SELECT v.version_id,v.report_id,v.version,v.digest,v.manifest_json,v.summary FROM report_versions v JOIN reports r USING(report_id)
                 WHERE r.tenant=?1 AND (?2 IS NULL OR v.kind=?2) AND r.since_ms<?4 AND r.until_ms>?3
                 ORDER BY v.published_ms DESC, v.version_id DESC LIMIT ?5 OFFSET ?6",
            )
            .map_err(storage)?;
        stmt.query_map(
            params![tenant, kind, a, b, limit as i64, offset as i64],
            row_version,
        )
        .map_err(storage)?
        .collect::<Result<_, _>>()
        .map_err(storage)
    }

    /// The current generation of the reports fence (advanced by prune and deletion).
    pub fn fence(&self, tenant: &str) -> Result<i64, Refused> {
        self.conn
            .query_row(
                "SELECT coalesce((SELECT generation FROM fences WHERE tenant=?1 AND scope=?2),0)",
                params![tenant, FENCE],
                |r| r.get(0),
            )
            .map_err(storage)
    }

    /// The reports-fence generation a version was published under (its `publications`
    /// row), or `None` when this tenant has no publication of that version.
    pub fn publication_fence(
        &self,
        tenant: &str,
        version_id: &str,
    ) -> Result<Option<i64>, Refused> {
        self.conn
            .query_row(
                "SELECT fence_generation FROM publications WHERE tenant=?1 AND report_version=?2",
                params![tenant, version_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)
    }

    /// Advance the reports fence (contract C7: done before any prune or deletion).
    pub fn advance_fence(&mut self, tenant: &str) -> Result<i64, Refused> {
        self.conn
            .execute(
                "INSERT INTO fences VALUES (?1,?2,1) ON CONFLICT(tenant,scope) DO UPDATE SET generation=generation+1",
                params![tenant, FENCE],
            )
            .map_err(storage)?;
        self.fence(tenant)
    }

    /// Whether `job_id` is a job of `tenant` whose canonical report parameters name
    /// `report_id` and `kind`: a recovered bundle is reattached only to the job that
    /// rendered it (M6 review 2 finding 3).
    pub fn report_job_binds(
        &self,
        tenant: &str,
        job_id: &str,
        report_id: &str,
        kind: &str,
    ) -> Result<bool, Refused> {
        let ours: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM jobs WHERE job_id=?1 AND tenant=?2)",
                params![job_id, tenant],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if !ours {
            return Ok(false);
        }
        let Ok(params) = self.params(job_id) else {
            return Ok(false);
        };
        Ok(serde_json::from_value::<ReportParams>(params)
            .is_ok_and(|p| p.report_id == report_id && p.kind == kind))
    }

    /// Record a version and move the latest pointer, in one transaction, only if the
    /// fence still has the generation rendering began under (C7).
    #[allow(clippy::too_many_arguments)]
    pub fn record_version(
        &mut self,
        tenant: &str,
        v: &VersionRow,
        kind: &str,
        job_id: &str,
        fence: i64,
        automatic: bool,
        now_ms: i64,
    ) -> Result<(), Refused> {
        let tx = self.conn.transaction().map_err(storage)?;
        let current: i64 = tx
            .query_row(
                "SELECT coalesce((SELECT generation FROM fences WHERE tenant=?1 AND scope=?2),0)",
                params![tenant, FENCE],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if current != fence {
            return Err(refused(
                ErrorCode::Conflict,
                "the reports fence advanced during rendering",
            ));
        }
        // A report with deleted content is never published into again, whatever fence
        // a queued job acquired after the deletion (M6 review 1 finding 2).
        let deleted: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tombstones WHERE tenant=?1 AND kind=?2 AND object_id=?3)",
                params![tenant, TOMBSTONE_REPORT, v.report_id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if deleted {
            return Err(refused(
                ErrorCode::Conflict,
                "the report has deleted content and is not published into again",
            ));
        }
        // A version is published only by a job of this tenant: never without its
        // completion event (M6 review 2 finding 3).
        let bound: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM jobs WHERE job_id=?1 AND tenant=?2)",
                params![job_id, tenant],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if !bound {
            return Err(refused(
                ErrorCode::Conflict,
                "the version names no job of this tenant",
            ));
        }
        let manifest_text = v.manifest.to_string();
        tx.execute(
            "INSERT INTO report_versions VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                v.version_id,
                v.report_id,
                v.version,
                kind,
                v.digest,
                manifest_text,
                v.summary,
                job_id,
                now_ms
            ],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO publications VALUES (?1,?2,?3,?4,?5)",
            params![
                v.version_id,
                tenant,
                hex(&Sha256::digest(manifest_text.as_bytes())),
                fence,
                now_ms
            ],
        )
        .map_err(storage)?;
        tx.execute(
            "UPDATE reports SET latest_version=?2, automatic_revisions=automatic_revisions+?3 WHERE report_id=?1",
            params![v.report_id, v.version_id, i64::from(automatic)],
        )
        .map_err(storage)?;
        // The completion event commits with the publication (M6 review 1 finding 5).
        tx.execute(
            "INSERT INTO completion_events VALUES (?1,?2,'report_published',?3,0) ON CONFLICT DO NOTHING",
            params![format!("{job_id}:report_published"), job_id, now_ms],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }

    /// Record an attributed assessment beside a version (never written into it).
    #[allow(clippy::too_many_arguments)]
    pub fn add_assessment(
        &mut self,
        tenant: &str,
        version_id: &str,
        (sub, client, label): (&str, &str, &str),
        review_sha256: &str,
        markdown: &str,
        now_ms: i64,
    ) -> Result<String, Refused> {
        let id = new_id("a1");
        self.conn
            .execute(
                "INSERT INTO assessments VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    id,
                    tenant,
                    version_id,
                    sub,
                    client,
                    label,
                    review_sha256,
                    markdown,
                    now_ms
                ],
            )
            .map_err(storage)?;
        Ok(id)
    }

    /// Whether a report version was deleted (`artifact_delete`): its tombstone.
    pub fn version_deleted(&self, tenant: &str, version_id: &str) -> Result<bool, Refused> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tombstones WHERE tenant=?1 AND kind=?2 AND object_id=?3)",
                params![tenant, TOMBSTONE_VERSION, version_id],
                |r| r.get(0),
            )
            .map_err(storage)
    }

    /// Whether a report has deleted content (`artifact_delete` left its `report`
    /// tombstone): no further version of it is ever published (M6 review 1 finding 2).
    pub fn report_deleted(&self, tenant: &str, report_id: &str) -> Result<bool, Refused> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tombstones WHERE tenant=?1 AND kind=?2 AND object_id=?3)",
                params![tenant, TOMBSTONE_REPORT, report_id],
                |r| r.get(0),
            )
            .map_err(storage)
    }

    /// The job that published a version.
    pub fn version_job(&self, tenant: &str, version_id: &str) -> Result<Option<String>, Refused> {
        self.conn
            .query_row(
                "SELECT v.job_id FROM report_versions v JOIN reports r ON r.report_id=v.report_id
                 WHERE r.tenant=?1 AND v.version_id=?2",
                params![tenant, version_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)
    }

    /// Insert the `report_published` completion event of every published version that
    /// lacks one (a store written before events were atomic with publication); idempotent.
    pub fn repair_publication_events(&mut self, tenant: &str) -> Result<u64, Refused> {
        self.conn
            .execute(
                "INSERT INTO completion_events
                 SELECT v.job_id || ':report_published', v.job_id, 'report_published', v.published_ms, 0
                 FROM report_versions v JOIN reports r ON r.report_id=v.report_id
                 WHERE r.tenant=?1 AND v.job_id <> '' AND EXISTS (SELECT 1 FROM jobs j WHERE j.job_id=v.job_id)
                 ON CONFLICT DO NOTHING",
                [tenant],
            )
            .map(|n| n as u64)
            .map_err(storage)
    }

    /// The deletion a preview describes (`artifact_delete`): every named version must be
    /// a live version of this tenant (a deleted or unknown one is `not_found`). Returns
    /// the versions and, per affected report, the latest pointer the deletion leaves.
    pub fn deletion_plan(
        &self,
        tenant: &str,
        version_ids: &[String],
    ) -> Result<DeletionPlan, Refused> {
        let mut versions = Vec::new();
        for id in version_ids {
            let v = self
                .version(tenant, id)?
                .ok_or(refused(ErrorCode::NotFound, "no such report version"))?;
            versions.push(v);
        }
        let deleting: std::collections::BTreeSet<&str> =
            versions.iter().map(|v| v.version_id.as_str()).collect();
        let mut latest = Vec::new();
        let reports: std::collections::BTreeSet<&str> =
            versions.iter().map(|v| v.report_id.as_str()).collect();
        for report in reports {
            let current = self.report(tenant, report)?.and_then(|r| r.latest_version);
            let moves = current.as_deref().is_some_and(|c| deleting.contains(c));
            let after: Option<String> = if moves {
                let mut stmt = self
                    .conn
                    .prepare("SELECT version_id FROM report_versions WHERE report_id=?1 ORDER BY version DESC, published_ms DESC, version_id DESC")
                    .map_err(storage)?;
                let ids: Vec<String> = stmt
                    .query_map([report], |r| r.get(0))
                    .map_err(storage)?
                    .collect::<Result<_, _>>()
                    .map_err(storage)?;
                ids.into_iter().find(|id| !deleting.contains(id.as_str()))
            } else {
                current.clone()
            };
            latest.push((report.to_owned(), current, after));
        }
        let assessments: i64 = {
            let ids = serde_json::to_string(version_ids).unwrap_or_else(|_| "[]".into());
            self.conn
                .query_row(
                    "SELECT count(*) FROM assessments WHERE tenant=?1 AND version_id IN (SELECT value FROM json_each(?2))",
                    params![tenant, ids],
                    |r| r.get(0),
                )
                .map_err(storage)?
        };
        Ok(DeletionPlan {
            versions,
            latest,
            assessments: assessments.max(0) as u64,
        })
    }

    /// Delete report versions (`artifact_delete`, contract §5.1/§6.2), in one control
    /// transaction: the reports fence advances first (C7: a render in flight can no
    /// longer publish, and recovery never reattaches an unrecorded bundle); each live
    /// version's assessments, publication and version rows go; a `report_version`
    /// tombstone (with the job) and a `report` tombstone (no automatic revision of that
    /// report again) are recorded; every affected report's latest pointer moves to its
    /// newest remaining version, or is cleared. A version already tombstoned (a retry of
    /// the same job) is skipped; one neither live nor tombstoned refuses the whole
    /// deletion (`conflict`). The bundle files are removed afterwards by
    /// [`Bundles::purge_deleted`]. Returns `(deleted now, already deleted, assessments)`.
    pub fn delete_versions(
        &mut self,
        tenant: &str,
        version_ids: &[String],
        job_id: &str,
        now_ms: i64,
    ) -> Result<(u64, u64, u64), Refused> {
        let tx = self.conn.transaction().map_err(storage)?;
        tx.execute(
            "INSERT INTO fences VALUES (?1,?2,1) ON CONFLICT(tenant,scope) DO UPDATE SET generation=generation+1",
            params![tenant, FENCE],
        )
        .map_err(storage)?;
        let (mut deleted, mut already, mut assessments) = (0u64, 0u64, 0u64);
        let mut reports = std::collections::BTreeSet::new();
        for id in version_ids {
            let tombstoned: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM tombstones WHERE tenant=?1 AND kind=?2 AND object_id=?3)",
                    params![tenant, TOMBSTONE_VERSION, id],
                    |r| r.get(0),
                )
                .map_err(storage)?;
            let report: Option<String> = tx
                .query_row(
                    "SELECT v.report_id FROM report_versions v JOIN reports r USING(report_id) WHERE r.tenant=?1 AND v.version_id=?2",
                    params![tenant, id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage)?;
            let Some(report) = report else {
                if tombstoned {
                    already += 1;
                    continue;
                }
                return Err(refused(
                    ErrorCode::Conflict,
                    "a report version to delete no longer exists",
                ));
            };
            assessments += tx
                .execute(
                    "DELETE FROM assessments WHERE tenant=?1 AND version_id=?2",
                    params![tenant, id],
                )
                .map_err(storage)? as u64;
            tx.execute(
                "DELETE FROM publications WHERE report_version=?1",
                params![id],
            )
            .map_err(storage)?;
            tx.execute(
                "DELETE FROM report_versions WHERE version_id=?1",
                params![id],
            )
            .map_err(storage)?;
            for (kind, object) in [
                (TOMBSTONE_VERSION, id.as_str()),
                (TOMBSTONE_REPORT, report.as_str()),
            ] {
                tx.execute(
                    "INSERT INTO tombstones VALUES (?1,?2,?3,?4,?5) ON CONFLICT DO NOTHING",
                    params![tenant, kind, object, now_ms, job_id],
                )
                .map_err(storage)?;
            }
            reports.insert(report);
            deleted += 1;
        }
        for report in reports {
            tx.execute(
                "UPDATE reports SET latest_version=(SELECT version_id FROM report_versions WHERE report_id=?1 ORDER BY version DESC, published_ms DESC, version_id DESC LIMIT 1)
                 WHERE report_id=?1 AND (latest_version IS NULL OR latest_version NOT IN (SELECT version_id FROM report_versions WHERE report_id=?1))",
                params![report],
            )
            .map_err(storage)?;
        }
        tx.commit().map_err(storage)?;
        Ok((deleted, already, assessments))
    }

    /// Test support: forget a version's control rows, as a crash between placing its
    /// bundle and recording it would leave them (crash point C6).
    #[doc(hidden)]
    pub fn forget_version_for_tests(&mut self, version_id: &str) -> Result<(), Refused> {
        self.conn
            .execute_batch(&format!(
                "DELETE FROM publications WHERE report_version='{version_id}'; UPDATE reports SET latest_version=NULL; DELETE FROM report_versions WHERE version_id='{version_id}';"
            ))
            .map_err(storage)
    }

    /// Whether an idempotency key is already bound (a replay is never re-charged).
    pub fn key_known(
        &self,
        tenant: &str,
        actor: &crate::control::Actor,
        operation: &str,
        epoch: &str,
        key: &str,
    ) -> Result<bool, Refused> {
        let (sub, client) = match actor {
            crate::control::Actor::Principal { sub, client_id } => {
                (sub.as_str(), client_id.as_str())
            }
            crate::control::Actor::Service(n) => ("", n.as_str()),
        };
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM idempotency WHERE tenant=?1 AND sub=?2 AND client_id=?3 AND operation=?4 AND epoch=?5 AND key=?6)",
                params![tenant, sub, client, operation, epoch, key],
                |r| r.get(0),
            )
            .map_err(storage)
    }

    /// Manual report quotas (contract §6), charged at admission and never refunded:
    /// at most `per_hour` in any rolling hour for the principal and `per_day` per
    /// America/Chicago day for the tenant.
    pub fn manual_report_quota(
        &self,
        tenant: &str,
        (sub, client): (&str, &str),
        (per_hour, per_day): (u64, u64),
        now_ms: i64,
    ) -> Result<(), Refused> {
        let hour: (i64, Option<i64>) = self
            .conn
            .query_row(
                "SELECT count(*), min(submitted_ms) FROM jobs WHERE tenant=?1 AND operation='reports_create' AND submitter_sub=?2 AND submitter_client_id=?3 AND submitted_ms>?4",
                params![tenant, sub, client, now_ms - 3_600_000],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(storage)?;
        if hour.0 as u64 >= per_hour {
            let wait = hour
                .1
                .map_or(3_600_000, |oldest| oldest + 3_600_000 - now_ms)
                .max(1_000);
            return Err(Refused {
                code: ErrorCode::QuotaExhausted,
                reason: "the hourly manual report quota is used up",
                retry_after_seconds: Some((wait as u64).div_ceil(1000)),
            });
        }
        let tz = airs_runtime_core::schedule::report_tz().map_err(|_| {
            refused(
                ErrorCode::StorageUnavailable,
                "embedded timezone unavailable",
            )
        })?;
        let day_start = jiff::Timestamp::from_millisecond(now_ms)
            .ok()
            .and_then(|t| t.to_zoned(tz.clone()).date().to_zoned(tz.clone()).ok())
            .map(|z| z.timestamp().as_millisecond())
            .unwrap_or(now_ms - 86_400_000);
        let day: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM jobs WHERE tenant=?1 AND operation='reports_create' AND submitted_ms>=?2",
                params![tenant, day_start],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if day as u64 >= per_day {
            return Err(Refused {
                code: ErrorCode::QuotaExhausted,
                reason: "the daily manual report quota is used up",
                retry_after_seconds: None,
            });
        }
        Ok(())
    }

    /// A completion event for a finished job (contract §4.1 completion events).
    pub fn completion_event(
        &mut self,
        job_id: &str,
        kind: &str,
        now_ms: i64,
    ) -> Result<(), Refused> {
        self.conn
            .execute(
                "INSERT INTO completion_events VALUES (?1,?2,?3,?4,0) ON CONFLICT DO NOTHING",
                params![format!("{job_id}:{kind}"), job_id, kind, now_ms],
            )
            .map_err(storage)?;
        Ok(())
    }
}

fn row_report(r: &rusqlite::Row<'_>) -> rusqlite::Result<ReportRow> {
    Ok(ReportRow {
        report_id: r.get(0)?,
        kind: r.get(1)?,
        since_ms: r.get(2)?,
        until_ms: r.get(3)?,
        app_ids: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
        title: r.get(5)?,
        scheduled_ms: r.get(6)?,
        latest_version: r.get(7)?,
        automatic_revisions: r.get(8)?,
    })
}

fn row_version(r: &rusqlite::Row<'_>) -> rusqlite::Result<VersionRow> {
    Ok(VersionRow {
        version_id: r.get(0)?,
        report_id: r.get(1)?,
        version: r.get(2)?,
        digest: r.get(3)?,
        manifest: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
        summary: r.get(5)?,
    })
}

/// The private bundle directory tree under the admitted data subtree.
/// Tombstone kinds of report deletion (`artifact_delete`): a deleted version, and a
/// report that lost a version (no automatic revision of it again).
pub const TOMBSTONE_VERSION: &str = "report_version";
pub const TOMBSTONE_REPORT: &str = "report";

/// What an `artifact_delete` would remove, for its preview.
pub struct DeletionPlan {
    pub versions: Vec<VersionRow>,
    /// Per affected report: its latest pointer now, and after the deletion.
    pub latest: Vec<(String, Option<String>, Option<String>)>,
    pub assessments: u64,
}

#[derive(Clone, Debug)]
pub struct Bundles {
    pub root: PathBuf,
}

impl Bundles {
    pub fn dir(&self, report_id: &str, version_id: &str) -> PathBuf {
        self.root.join(report_id).join(version_id)
    }

    /// Stage files privately (0700 dir, 0600 files, fsynced); returns the stage path.
    pub fn stage(&self, files: &[(&str, &[u8])]) -> Result<PathBuf, Refused> {
        let stage = self.root.join(format!(".stage-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&stage)
            .map_err(io)?;
        for (name, bytes) in files {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(stage.join(name))
                .map_err(io)?;
            f.write_all(bytes).and_then(|_| f.sync_all()).map_err(io)?;
        }
        sync(&stage)?;
        Ok(stage)
    }

    /// Move a stage to its immutable place (never over an existing version).
    pub fn place(&self, stage: &Path, report_id: &str, version_id: &str) -> Result<(), Refused> {
        let parent = self.root.join(report_id);
        match std::fs::DirBuilder::new().mode(0o700).create(&parent) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io(e)),
        }
        let target = parent.join(version_id);
        if target.exists() {
            return Err(refused(
                ErrorCode::Conflict,
                "a bundle version is never replaced",
            ));
        }
        std::fs::rename(stage, &target).map_err(io)?;
        sync(&parent)?;
        sync(&self.root)
    }

    /// Read a published file, checked against its manifest entry.
    /// Whether a bundle on disk is exactly what its manifest declares (M6 review 1
    /// finding 4): the manifest lists every required file once, and each listed file
    /// exists as a regular file with the declared size and SHA-256; the on-disk
    /// manifest is present, bounded, parseable and matches the stored version, including
    /// its recovery fields: the evidence digest, the publication fence `fence` and the
    /// agent-facing summary. No other file is present.
    fn verify_bundle(&self, v: &VersionRow, dir: &Path, fence: i64) -> bool {
        let Some(listed) = v.manifest["files"].as_array() else {
            return false;
        };
        let names: std::collections::BTreeSet<&str> =
            listed.iter().filter_map(|e| e["name"].as_str()).collect();
        if names.len() != listed.len() || FILES.iter().any(|f| !names.contains(f)) {
            return false;
        }
        for entry in listed {
            let (Some(name), Some(bytes), Some(sha)) = (
                entry["name"].as_str(),
                entry["bytes"].as_u64(),
                entry["sha256"].as_str(),
            ) else {
                return false;
            };
            if name.contains('/') || name == MANIFEST {
                return false;
            }
            let path = dir.join(name);
            match std::fs::symlink_metadata(&path) {
                Ok(m) if m.file_type().is_file() && m.len() == bytes => {}
                _ => return false,
            }
            match std::fs::read(&path) {
                Ok(b) if hex(&Sha256::digest(&b)) == sha => {}
                _ => return false,
            }
        }
        // The on-disk manifest is required too (M6 review 6): a regular, bounded,
        // parseable file whose public view is exactly the stored version's manifest and
        // whose recovery binding names the stored evidence digest.
        let manifest_path = dir.join(MANIFEST);
        match std::fs::symlink_metadata(&manifest_path) {
            Ok(m) if m.file_type().is_file() && m.len() <= 1024 * 1024 => {}
            _ => return false,
        }
        let Some(on_disk) = std::fs::read(&manifest_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        else {
            return false;
        };
        // The recovery fields too (M6 review 7): recovery reattaches from them, so a
        // damaged fence or summary would quarantine a good bundle or restore a wrong
        // summary.
        if public_manifest(&on_disk) != v.manifest
            || on_disk.get("x_evidence_digest").and_then(Value::as_str) != Some(v.digest.as_str())
            || on_disk.get("x_fence").and_then(Value::as_i64) != Some(fence)
            || on_disk.get("x_summary").and_then(Value::as_str) != Some(v.summary.as_str())
        {
            return false;
        }
        match std::fs::read_dir(dir) {
            Ok(entries) => entries.flatten().all(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n == MANIFEST || names.contains(n.as_str())
            }),
            Err(_) => false,
        }
    }

    pub fn read(&self, v: &VersionRow, name: &str) -> Result<Vec<u8>, Refused> {
        let entry = v.manifest["files"]
            .as_array()
            .and_then(|f| f.iter().find(|e| e["name"] == name))
            .cloned()
            .ok_or(refused(ErrorCode::NotFound, "no such artifact"))?;
        let bytes = std::fs::read(self.dir(&v.report_id, &v.version_id).join(name)).map_err(io)?;
        if entry["sha256"].as_str() != Some(hex(&Sha256::digest(&bytes)).as_str()) {
            return Err(refused(
                ErrorCode::StorageUnavailable,
                "the artifact no longer matches its manifest",
            ));
        }
        Ok(bytes)
    }

    /// Files and bytes a version's bundle holds on disk (for a deletion preview).
    pub fn size(&self, report_id: &str, version_id: &str) -> (u64, u64) {
        let Ok(entries) = std::fs::read_dir(self.dir(report_id, version_id)) else {
            return (0, 0);
        };
        entries
            .flatten()
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .fold((0, 0), |(n, b), m| (n + 1, b + m.len()))
    }

    /// Remove the bundle of every deleted (tombstoned) version, crash-safely: the
    /// version directory is first renamed out of the published tree (`.deleted-*`, never
    /// readable as a version), the root synced, then removed; leftover `.deleted-*`
    /// directories of an interrupted removal are removed too. Idempotent. Returns the
    /// versions removed now.
    pub fn purge_deleted(&self, c: &Control, tenant: &str) -> Result<u64, Refused> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Ok(0);
        };
        let mut removed = 0;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(".deleted-") {
                std::fs::remove_dir_all(entry.path()).map_err(io)?;
                continue;
            }
            if !name.starts_with("r1_") {
                continue;
            }
            let Ok(versions) = std::fs::read_dir(entry.path()) else {
                continue;
            };
            for v in versions.flatten() {
                let vid = v.file_name().to_string_lossy().into_owned();
                if !c.version_deleted(tenant, &vid)? {
                    continue;
                }
                let doomed = self.root.join(format!(".deleted-{vid}"));
                std::fs::rename(v.path(), &doomed).map_err(io)?;
                sync(&entry.path())?;
                sync(&self.root)?;
                std::fs::remove_dir_all(&doomed).map_err(io)?;
                removed += 1;
            }
        }
        sync(&self.root)?;
        Ok(removed)
    }

    /// Recovery (C5/C6): discard unpublished stages; reattach a placed bundle that has
    /// no control row from its own manifest when its report exists and the fence still
    /// matches; quarantine anything else. Returns `(reattached, quarantined, discarded)`.
    pub fn recover(
        &self,
        c: &mut Control,
        tenant: &str,
        now_ms: i64,
    ) -> Result<(u64, u64, u64), Refused> {
        let (mut reattached, mut quarantined, mut discarded) = (0, 0, 0);
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Ok((0, 0, 0));
        };
        // A deletion interrupted after its control commit completes first: no deleted
        // version's bundle is ever reattached or presented.
        discarded += self.purge_deleted(c, tenant)?;
        // Every published version has its completion event (finding 5).
        c.repair_publication_events(tenant)?;
        let fence = c.fence(tenant)?;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(".deleted-") {
                continue;
            }
            if name.starts_with(".stage-") {
                let _ = std::fs::remove_dir_all(entry.path());
                discarded += 1;
                continue;
            }
            if !name.starts_with("r1_") {
                continue;
            }
            let Ok(versions) = std::fs::read_dir(entry.path()) else {
                continue;
            };
            for v in versions.flatten() {
                let vid = v.file_name().to_string_lossy().into_owned();
                if c.version(tenant, &vid)?.is_some() {
                    continue;
                }
                let manifest: Option<Value> = std::fs::read(v.path().join(MANIFEST))
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok());
                let row = manifest.as_ref().and_then(|m| {
                    Some(VersionRow {
                        version_id: vid.clone(),
                        report_id: name.clone(),
                        version: m["version"].as_i64()?,
                        digest: m.get("x_evidence_digest")?.as_str()?.to_owned(),
                        manifest: public_manifest(m),
                        summary: truncate_utf8(
                            m["x_summary"].as_str().unwrap_or_default().to_owned(),
                            SUMMARY_MAX,
                        ),
                    })
                });
                let fenced_ok =
                    manifest.as_ref().and_then(|m| m["x_fence"].as_i64()) == Some(fence);
                let ok = fenced_ok
                    && row.as_ref().is_some_and(|r| {
                        r.manifest["report_id"] == name.as_str()
                            && r.manifest["version_id"] == vid.as_str()
                            && r.manifest["job_id"].as_str().is_some_and(|j| {
                                // The manifest's job must be this tenant's report job for
                                // this report and kind (M6 review 2 finding 3).
                                c.report_job_binds(
                                    tenant,
                                    j,
                                    &name,
                                    r.manifest["kind"].as_str().unwrap_or_default(),
                                )
                                .unwrap_or(false)
                            })
                            && c.report(tenant, &name).ok().flatten().is_some()
                            && self.verify_bundle(r, &v.path(), fence)
                    });
                if let (true, Some(r)) = (ok, row) {
                    let kind = r.manifest["kind"].as_str().unwrap_or("manual").to_owned();
                    let job = r.manifest["job_id"].as_str().unwrap_or_default().to_owned();
                    c.record_version(tenant, &r, &kind, &job, fence, kind == "revision", now_ms)?;
                    reattached += 1;
                } else {
                    let q = self.root.join(".quarantine");
                    let _ = std::fs::DirBuilder::new().mode(0o700).create(&q);
                    let _ = std::fs::rename(v.path(), q.join(format!("{name}-{vid}")));
                    quarantined += 1;
                }
            }
        }
        Ok((reattached, quarantined, discarded))
    }
}

fn sync(dir: &Path) -> Result<(), Refused> {
    std::fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(io)
}

/// A report job's canonical parameters.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct ReportParams {
    pub report_id: String,
    /// `daily`, `manual`, `catch_up` or `revision`.
    pub kind: String,
    pub since_ms: i64,
    pub until_ms: i64,
    #[serde(default)]
    pub app_ids: Vec<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub scheduled_ms: Option<i64>,
    /// A revision's evidence cutoff (scheduled midnight + its opportunity).
    #[serde(default)]
    pub cutoff_ms: Option<i64>,
    /// Whether this revision is the window's final (+24 h) snapshot.
    #[serde(default)]
    pub final_revision: bool,
}

/// A short plain-text summary of a version (≤ 8 KiB) for `reports_get`.
/// The agent-facing summary: the same full-cohort counts as the page (every event of
/// the period, not the capped detail; M6 review 1 minor), then the gaps.
pub fn summary(input: &airs_history_store::report::ReportInput) -> String {
    let summary = &input.summary;
    let count = |dimension: &str, values: &[&str]| -> u64 {
        summary
            .dimensions
            .iter()
            .position(|d| *d == dimension)
            .map_or(0, |i| {
                summary
                    .cells
                    .iter()
                    .filter(|c| values.contains(&c.key[i].as_str()))
                    .map(|c| c.events)
                    .sum()
            })
    };
    let mut s = format!(
        "{} events: {} malicious, {} benign, {} unknown; {} blocked, {} allowed.",
        summary.events,
        count("verdict", &["malicious"]),
        count("verdict", &["benign"]),
        count("verdict", &["unknown", "error"]),
        count("action", &["block"]),
        count("action", &["allow"]),
    );
    for g in &input.gaps {
        s.push_str("\nGap: ");
        s.push_str(g);
    }
    truncate_utf8(s, SUMMARY_MAX)
}

/// `s` cut to at most `max` bytes at a character boundary (the agent summary's ceiling
/// is in bytes; M6 review 3 minor).
pub(crate) fn truncate_utf8(mut s: String, max: usize) -> String {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

/// The report executor: `reports_create`, `daily_report`, `catch_up` and `revision`.
pub struct Reporter {
    pub shared: Arc<Shared>,
    pub bundles: Bundles,
}

impl Reporter {
    /// Whether a published version may be returned as this job's result: its bundle
    /// verifies against the stored row and the fence it was published under. Never
    /// `succeeded` for an unverified bundle (M6 reviews 5 to 7). A later, unrelated
    /// fence advance does not invalidate an intact version.
    fn reusable(&self, ctx: &JobContext<'_>, tenant: &str, v: &VersionRow) -> bool {
        let Ok(Some(fence)) = ctx.with_control(|c| c.publication_fence(tenant, &v.version_id))
        else {
            return false;
        };
        self.bundles
            .verify_bundle(v, &self.bundles.dir(&v.report_id, &v.version_id), fence)
    }
}

fn failure(operation: &str, code: ErrorCode, reason: &str) -> Value {
    json!({"operation": operation, "committed_facts": 0, "uncertain_sends": 0,
        "error": {"schema_version": 1, "code": code, "reason": reason}})
}

impl Executor for Reporter {
    fn run(&self, ctx: &JobContext<'_>) -> Outcome {
        let live = self.shared.current();
        let config = &live.config;
        let operation = ctx.job.operation.clone();
        let Ok(p) = ctx
            .params()
            .map_err(|_| ())
            .and_then(|v| serde_json::from_value::<ReportParams>(v).map_err(|_| ()))
        else {
            return Outcome::Failed(failure(
                &operation,
                ErrorCode::InvalidRequest,
                "stored parameters are invalid",
            ));
        };
        let tenant = ctx.job.tenant.clone();
        let now = airs_runtime_core::time::now_ms();
        let report_kind = if p.kind == "revision" {
            "daily"
        } else {
            p.kind.as_str()
        };
        // A revision always concerns an existing report: it uses that report's stored row
        // and never re-initialises it from its own parameters (a revision queued with
        // other metadata still revises the right report; M6 review 4). Other jobs
        // initialise their report; a row naming a different report is a permanent
        // conflict, not a retry.
        let prepared = ctx.with_control(|c| {
            if p.kind != "revision" {
                c.ensure_report(
                    &tenant,
                    &p.report_id,
                    report_kind,
                    (p.since_ms, p.until_ms),
                    &p.app_ids,
                    &p.title,
                    p.scheduled_ms,
                    now,
                )?;
            }
            Ok((c.report(&tenant, &p.report_id)?, c.fence(&tenant)?))
        });
        let (report, fence) = match prepared {
            Ok((Some(report), fence)) => (report, fence),
            Ok((None, _)) if p.kind == "revision" => {
                return Outcome::Failed(failure(
                    &operation,
                    ErrorCode::NotFound,
                    "the revised report does not exist",
                ));
            }
            Err(e) if e.code == ErrorCode::Conflict => {
                return Outcome::Failed(failure(&operation, ErrorCode::Conflict, e.reason));
            }
            _ => return Outcome::RetryLater { after_ms: 60_000 },
        };
        // A revision (or any job) of a report with deleted content publishes nothing, even
        // when it was queued before the deletion (M6 review 1 finding 2).
        if ctx
            .with_control(|c| c.report_deleted(&tenant, &p.report_id))
            .unwrap_or(true)
        {
            return Outcome::Failed(failure(
                &operation,
                ErrorCode::Conflict,
                "the report has deleted content and is not published into again",
            ));
        }
        let latest = report
            .latest_version
            .as_ref()
            .and_then(|v| ctx.with_control(|c| c.version(&tenant, v)).ok().flatten());
        let result_of = |v: &VersionRow, complete: bool| {
            json!({
                "operation": operation,
                "report_id": v.report_id,
                "version_id": v.version_id,
                "completeness": if complete { "complete" } else { "partial" },
                "manifest_sha256": hex(&Sha256::digest(v.manifest.to_string().as_bytes())),
            })
        };
        // Revisions within the allowance; the final allowance is kept for +24 h.
        if p.kind == "revision" {
            let Some(_) = latest.as_ref() else {
                return Outcome::Failed(failure(
                    &operation,
                    ErrorCode::NotFound,
                    "no original to revise",
                ));
            };
            let used = report.automatic_revisions.max(0) as u32;
            if !airs_runtime_core::schedule::may_revise(p.final_revision, used, REVISION_ALLOWANCE)
                .unwrap_or(false)
            {
                let v = latest.expect("checked");
                if !self.reusable(ctx, &tenant, &v) {
                    return Outcome::Failed(failure(
                        &operation,
                        ErrorCode::StorageUnavailable,
                        "the published bundle failed verification",
                    ));
                }
                let complete = v.manifest["completeness"] == "complete";
                return Outcome::Succeeded(result_of(&v, complete));
            }
        }
        let Ok(cloud) = crate::tools::cloud(config) else {
            return Outcome::Failed(failure(
                &operation,
                ErrorCode::StorageUnavailable,
                "archive identity unavailable",
            ));
        };
        let cutoff = p.cutoff_ms.unwrap_or(now);
        let input = airs_history_store::report::report_evidence(
            ctx.history_root(),
            &cloud,
            &config.service.tenant,
            (report.since_ms, report.until_ms),
            &report.app_ids,
            cutoff,
            airs_runtime_core::VERSION,
            &scm::Session::new(),
        );
        let input = match input {
            Ok(Some(i)) => i,
            Ok(None) => {
                return Outcome::Failed(failure(
                    &operation,
                    ErrorCode::NotFound,
                    "no archived history for this service yet",
                ));
            }
            Err(_) => {
                return Outcome::Failed(failure(
                    &operation,
                    ErrorCode::InvalidRequest,
                    "the archived evidence cannot form a report for this period",
                ));
            }
        };
        // Identical report-relevant evidence produces no new bundle.
        if let Some(v) = latest.as_ref().filter(|v| v.digest == input.digest) {
            // Reuse only a bundle that is exactly what its manifest declares: never
            // `succeeded` for a missing or corrupted bundle (M6 review 5).
            if !self.reusable(ctx, &tenant, v) {
                return Outcome::Failed(failure(
                    &operation,
                    ErrorCode::StorageUnavailable,
                    "the published bundle failed verification",
                ));
            }
            // A retry of the job that published it re-asserts its completion event
            // (idempotent), so no path returns success without it (finding 5).
            let job = ctx.job.job_id.clone();
            let published_by_this_job = ctx
                .with_control(|c| c.version_job(&tenant, &v.version_id))
                .ok()
                .flatten()
                .is_some_and(|j| j == job);
            if published_by_this_job
                && ctx
                    .with_control(|c| c.completion_event(&job, "report_published", now))
                    .is_err()
            {
                return Outcome::RetryLater { after_ms: 60_000 };
            }
            return Outcome::Succeeded(result_of(v, input.complete));
        }
        let title = if report.title.is_empty() {
            "AIRS runtime history report".to_owned()
        } else {
            report.title.clone()
        };
        let version = latest.as_ref().map_or(0, |v| v.version + 1);
        // The historical page (filters, full-cohort summary, matrix, timeline); its
        // 32 MiB fallback drops the detail table, and a page still over it is refused.
        let page = crate::report_html::Page {
            title: &title,
            kind: &p.kind,
            version: version.max(0) as u32,
            period: (report.since_ms, report.until_ms),
            cutoff_ms: cutoff,
        };
        let html = match crate::report_html::render(
            &page,
            &input,
            crate::report_html::Limits::default(),
        ) {
            Ok(html) => html,
            Err(crate::report_html::Refusal::TooLarge) => {
                return Outcome::Failed(failure(
                    &operation,
                    ErrorCode::ResultTooLarge,
                    "the report exceeds the bundle bounds",
                ));
            }
            Err(crate::report_html::Refusal::Evidence) => {
                return Outcome::Failed(failure(
                    &operation,
                    ErrorCode::Internal,
                    "the report could not be rendered",
                ));
            }
        };
        let rendered = (|| {
            let json = serde_json::to_string_pretty(&input.evidence).ok()?;
            let review =
                airs_runtime_core::signal::review::packet_with_review(&input.evidence, None)
                    .ok()?;
            Some((html, json, review))
        })();
        let Some((html, json_text, review)) = rendered else {
            return Outcome::Failed(failure(
                &operation,
                ErrorCode::Internal,
                "the report could not be rendered",
            ));
        };
        if html.len() > MAX_HTML || html.len() + json_text.len() + review.len() > MAX_BUNDLE {
            return Outcome::Failed(failure(
                &operation,
                ErrorCode::ResultTooLarge,
                "the report exceeds the bundle bounds",
            ));
        }
        let version_id = new_id("v1");
        if version > 24 {
            return Outcome::Failed(failure(
                &operation,
                ErrorCode::QuotaExhausted,
                "the report has no versions left",
            ));
        }
        let files: [(&str, &[u8]); 3] = [
            ("report.html", html.as_bytes()),
            ("evidence.json", json_text.as_bytes()),
            ("review.md", review.as_bytes()),
        ];
        let manifest = json!({
            "report_id": p.report_id,
            "version_id": version_id,
            "version": version,
            "kind": p.kind,
            "job_id": ctx.job.job_id,
            "period": {"since": airs_runtime_core::time::iso(report.since_ms), "until": airs_runtime_core::time::iso(report.until_ms)},
            "scheduled_at": report.scheduled_ms.or(p.scheduled_ms).map(airs_runtime_core::time::iso),
            "as_of": airs_runtime_core::time::iso(cutoff),
            "timezone": airs_runtime_core::schedule::REPORT_TZ,
            "tzdb_version": "jiff-bundled",
            "analysis_version": 1,
            "redaction_version": input.evidence.redaction_version,
            "completeness": if input.complete { "complete" } else { "partial" },
            "gaps": input.gaps.iter().take(100).map(|g| g.chars().take(256).collect::<String>()).collect::<Vec<_>>(),
            "predecessor": latest.as_ref().map(|v| v.version_id.clone()),
            "files": files.iter().map(|(n, b)| json!({"name": n, "bytes": b.len(), "sha256": hex(&Sha256::digest(b))})).collect::<Vec<_>>(),
            "x_evidence_digest": input.digest,
            "x_fence": fence,
            // The agent-facing summary, restored if recovery reattaches this bundle
            // (M6 review 2 minor).
            "x_summary": summary(&input),
        });
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap_or_default();
        let mut all: Vec<(&str, &[u8])> = files.to_vec();
        all.push((MANIFEST, &manifest_bytes));
        let Ok(stage) = self.bundles.stage(&all) else {
            return Outcome::RetryLater { after_ms: 60_000 };
        };
        // C7: a fence advanced since rendering began discards the stage.
        if ctx.with_control(|c| c.fence(&tenant)).ok() != Some(fence) {
            let _ = std::fs::remove_dir_all(&stage);
            return Outcome::Failed(failure(
                &operation,
                ErrorCode::Conflict,
                "a prune or deletion ran during rendering",
            ));
        }
        if self
            .bundles
            .place(&stage, &p.report_id, &version_id)
            .is_err()
        {
            let _ = std::fs::remove_dir_all(&stage);
            return Outcome::RetryLater { after_ms: 60_000 };
        }
        let row = VersionRow {
            version_id: version_id.clone(),
            report_id: p.report_id.clone(),
            version,
            digest: input.digest.clone(),
            manifest: public_manifest(&manifest),
            summary: summary(&input),
        };
        let kind = p.kind.clone();
        let job = ctx.job.job_id.clone();
        match ctx.with_control(|c| {
            c.record_version(&tenant, &row, &kind, &job, fence, kind == "revision", now)
        }) {
            Ok(()) => Outcome::Succeeded(result_of(&row, input.complete)),
            Err(_) => Outcome::RetryLater { after_ms: 60_000 },
        }
    }
}

/// The frozen `manifest` shape (the on-disk manifest also carries the private
/// `x_evidence_digest`, `x_fence` and `x_summary` recovery fields).
pub fn public_manifest(m: &Value) -> Value {
    let mut m = m.clone();
    if let Some(o) = m.as_object_mut() {
        o.remove("x_evidence_digest");
        o.remove("x_fence");
        o.remove("x_summary");
    }
    m
}
