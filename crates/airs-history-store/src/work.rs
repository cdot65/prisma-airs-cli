//! Durable scanner-work admission. Callers commit reservations before network I/O.
//! All SQL runs inside the caller's bounded transaction; this module sends nothing.
use super::{database::sql_error, failure};
use airs_runtime_core::{
    error::CliError, signal::evidence::Identity, tenant::scanners::ScannerReference,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Results,
    Reports,
}
impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Self::Results => "results",
            Self::Reports => "reports",
        }
    }
}

/// Public, already privacy-projected request identity; never SDK extensions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub identity: Identity,
    pub report_id: Option<String>,
}
impl Descriptor {
    pub fn id(&self, source: Source) -> Result<String, CliError> {
        let i = &self.identity;
        if [&i.app_id, &i.app_name, &i.session_id].iter().any(|s| {
            s.is_empty()
                || s.chars().count() >= 4096
                || s.len() > 4096
                || s.chars().any(char::is_control)
                || [
                    airs_runtime_core::signal::privacy::WITHHELD,
                    airs_runtime_core::error::WITHHELD,
                    "[REDACTED]",
                ]
                .contains(&s.as_str())
        }) || !airs_runtime_core::scan_logs::is_scan_id(&i.scan_id)
            || uuid::Uuid::parse_str(&i.scan_id).is_ok_and(|id| id.to_string() != i.scan_id)
            || i.sub_request.is_none_or(|n| i64::try_from(n).is_err())
            || (source == Source::Reports
                && !self.report_id.as_deref().is_some_and(|s| {
                    airs_runtime_core::scan_logs::is_report_id(s)
                        && ![
                            airs_runtime_core::signal::privacy::WITHHELD,
                            airs_runtime_core::error::WITHHELD,
                            "[REDACTED]",
                        ]
                        .contains(&s)
                }))
            || (source == Source::Results && self.report_id.is_some())
        {
            return Err(failure("invalid normalized scanner work identity"));
        }
        Ok(format!(
            "{:x}",
            Sha256::digest(encode(&(source, self))?.as_bytes())
        ))
    }
}

fn encode<T: Serialize>(v: &T) -> Result<String, CliError> {
    serde_json::to_string(v).map_err(|_| failure("cannot encode work provenance"))
}
fn fresh() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Clone, Debug)]
pub struct Attempt {
    pub id: String,
    pub work: String,
    pub transport: String,
    pub revision: String,
    pub epoch: i64,
    pub ordinal: i64,
    pub bytes: usize,
}
#[derive(Debug)]
pub enum Admission {
    Due(Attempt),
    Skip(&'static str),
}

fn transition(
    conn: &Connection,
    work: &str,
    now: i64,
    kind: &str,
    epoch: i64,
    revisions: (Option<&str>, Option<&str>),
    reason: &str,
) -> Result<(), CliError> {
    conn.execute(
        "INSERT INTO work_transitions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'{}')",
        params![
            fresh(),
            work,
            now,
            kind,
            epoch,
            revisions.0,
            revisions.1,
            reason
        ],
    )
    .map_err(sql_error)?;
    Ok(())
}

/// A cooldown is cloud/service wide, including every named credential. A backwards
/// wall-clock observation defers work; expiry does not delete its clock evidence.
pub fn cooling(conn: &Connection, source: Source, now: i64) -> Result<bool, CliError> {
    cooling_service(conn, source.name(), now)
}

pub fn cooling_service(conn: &Connection, service: &str, now: i64) -> Result<bool, CliError> {
    let prior: Option<(i64, i64)> = conn
        .query_row(
            "SELECT until_ms,observed_ms FROM service_cooldowns WHERE service=?1",
            [service],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql_error)?;
    Ok(prior.is_some_and(|(until, observed)| now < observed || now < until))
}

/// Only a parsed bounded Retry-After duration reaches this interface. Invalid or
/// absent values use fifteen minutes; all values are capped at twenty-four hours.
pub fn cooldown(
    conn: &Connection,
    source: Source,
    now: i64,
    retry_after_seconds: Option<u64>,
) -> Result<(), CliError> {
    cooldown_service(conn, source.name(), now, retry_after_seconds)
}

pub fn cooldown_service(
    conn: &Connection,
    service: &str,
    now: i64,
    retry_after_seconds: Option<u64>,
) -> Result<(), CliError> {
    let seconds = retry_after_seconds
        .filter(|s| *s > 0)
        .unwrap_or(900)
        .min(86400);
    let until = now.saturating_add((seconds * 1000) as i64);
    conn.execute(
        "INSERT INTO service_cooldowns VALUES(?1,?2,?3,'rate_limited') ON CONFLICT(service) DO UPDATE SET until_ms=max(until_ms,excluded.until_ms),observed_ms=max(observed_ms,excluded.observed_ms),reason=excluded.reason",
        params![service, until, now],
    ).map_err(sql_error)?;
    Ok(())
}

/// Must be called in the same committed transaction that durably records `started`.
/// The caller may send only after that commit succeeds. Unfinished reservations
/// retain their ordinal across restarts; only proven `not_sent` releases it.
pub fn reserve(
    conn: &Connection,
    descriptor: &Descriptor,
    source: Source,
    reference: Option<&ScannerReference>,
    run: &str,
    transport: &str,
    now: i64,
) -> Result<Admission, CliError> {
    let id = descriptor.id(source)?;
    if transport.is_empty() || transport.len() > 64 || run.is_empty() {
        return Err(failure("invalid transport reservation identity"));
    }
    if reference.is_some_and(|r| !r.valid() || r.application_id != descriptor.identity.app_id) {
        return Err(failure("scanner reference does not match work application"));
    }
    conn.execute(
        "INSERT OR IGNORE INTO sync_work VALUES(?1,?2,?3,'pending',NULL,0,0,NULL,NULL,NULL,'{}')",
        params![id, source.name(), encode(descriptor)?],
    )
    .map_err(sql_error)?;
    type StoredWork = (
        String,
        Option<String>,
        i64,
        i64,
        Option<i64>,
        Option<i64>,
        String,
        Option<String>,
    );
    let (state, revision, epoch, attempts, last, eligible, provenance, reason): StoredWork = conn.query_row(
        "SELECT state,binding_revision,retry_epoch,attempts,last_attempt_ms,eligible_after_ms,provenance_json,reason FROM sync_work WHERE work_id=?1",
        [&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?)),
    ).map_err(sql_error)?;
    if state == "complete" {
        return Ok(Admission::Skip("already complete"));
    }
    let Some(reference) = reference else {
        if state != "ineligible" || reason.as_deref() != Some("binding_unavailable") {
            transition(
                conn,
                &id,
                now,
                "ineligible",
                epoch,
                (revision.as_deref(), None),
                "application-bound credential unavailable",
            )?;
            conn.execute("UPDATE sync_work SET state='ineligible',reason='binding_unavailable' WHERE work_id=?1",[&id]).map_err(sql_error)?;
        }
        return Ok(Admission::Skip("application-bound credential unavailable"));
    };
    let old: Option<ScannerReference> = serde_json::from_str::<serde_json::Value>(&provenance)
        .ok()
        .and_then(|v| v.get("scanner_reference").cloned())
        .and_then(|v| serde_json::from_value(v).ok());
    // Changed application/endpoint is a rebind, never evidence of historical access.
    if old.as_ref().is_some_and(|old| {
        old.application_id != reference.application_id || old.endpoint != reference.endpoint
    }) {
        if state != "ineligible" {
            transition(
                conn,
                &id,
                now,
                "ineligible",
                epoch,
                (revision.as_deref(), Some(&reference.revision)),
                "scanner application or endpoint rebound",
            )?;
            conn.execute(
                "UPDATE sync_work SET state='ineligible',reason='binding_rebound' WHERE work_id=?1",
                [&id],
            )
            .map_err(sql_error)?;
        }
        return Ok(Admission::Skip("historical binding was rebound"));
    }
    let changed = revision.as_deref() != Some(reference.revision.as_str());
    let attempts = if changed { 0 } else { attempts };
    if !changed && attempts >= 3 && state == "pending" {
        transition(
            conn,
            &id,
            now,
            "deferred",
            epoch,
            (revision.as_deref(), revision.as_deref()),
            "unfinished reservations exhausted eligible attempts",
        )?;
        conn.execute(
            "UPDATE sync_work SET state='deferred',reason='attempt_limit' WHERE work_id=?1",
            [&id],
        )
        .map_err(sql_error)?;
    }
    if !changed && (state == "ineligible" || state == "deferred" || attempts >= 3) {
        return Ok(Admission::Skip(
            "credential revision ineligible or attempts exhausted",
        ));
    }
    // A 401/403 on any item denies this app/revision for both scanner services.
    let denied: bool=conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM work_attempts a JOIN sync_work w ON w.work_id=a.work_id WHERE a.binding_revision=?1 AND a.outcome='auth_denied' AND json_extract(w.descriptor_json,'$.identity.app_id')=?2)",
        params![reference.revision,reference.application_id],|r|r.get(0),
    ).map_err(sql_error)?;
    if denied {
        if state != "ineligible" || changed {
            transition(
                conn,
                &id,
                now,
                "ineligible",
                epoch,
                (revision.as_deref(), Some(&reference.revision)),
                "application revision authorization denied",
            )?;
            conn.execute("UPDATE sync_work SET state='ineligible',binding_revision=?2,reason='authorization_denied',provenance_json=json_set(provenance_json,'$.scanner_reference',json(?3),'$.historical_access','unqualified') WHERE work_id=?1",params![id,reference.revision,encode(reference)?]).map_err(sql_error)?;
        }
        return Ok(Admission::Skip("application revision authorization denied"));
    }
    if cooling(conn, source, now)? {
        return Ok(Admission::Skip("service cooldown or uncertain wall clock"));
    }
    if last.is_some_and(|last| now < last) {
        return Ok(Admission::Skip("wall clock moved backwards"));
    }
    if !changed && eligible.is_some_and(|at| now < at) {
        return Ok(Admission::Skip("retry spacing not elapsed"));
    }
    if changed {
        transition(
            conn,
            &id,
            now,
            "binding_changed",
            epoch,
            (revision.as_deref(), Some(&reference.revision)),
            "configured revision changed; historical access remains unqualified",
        )?;
    }
    let ordinal = attempts + 1;
    let provenance = encode(
        &serde_json::json!({"scanner_reference":reference,"historical_access":"unqualified","reservation":"conservative_before_transport"}),
    )?;
    let attempt = Attempt {
        id: fresh(),
        work: id,
        transport: transport.into(),
        revision: reference.revision.clone(),
        epoch,
        ordinal,
        bytes: encode(reference)?
            .len()
            .saturating_mul(2)
            .saturating_add(encode(descriptor)?.len())
            .saturating_add(2048),
    };
    conn.execute(
        "INSERT INTO work_attempts VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,NULL,NULL,'started',?10)",
        params![
            attempt.id,
            attempt.work,
            source.name(),
            run,
            transport,
            reference.revision,
            epoch,
            ordinal,
            now,
            provenance
        ],
    )
    .map_err(sql_error)?;
    // Started/uncertain attempts enforce the same spacing as completed failures.
    let after = now.saturating_add(if ordinal == 1 { 60_000 } else { 900_000 });
    conn.execute("UPDATE sync_work SET state='pending',binding_revision=?2,attempts=?3,last_attempt_ms=?4,eligible_after_ms=?5,reason='reserved',provenance_json=?6 WHERE work_id=?1",params![attempt.work,reference.revision,ordinal,now,after,provenance]).map_err(sql_error)?;
    Ok(Admission::Due(attempt))
}

#[derive(Clone, Copy, Debug)]
pub enum Outcome {
    NotSent,
    Returned,
    Missing,
    Pending,
    Rejected,
    AuthDenied,
    RateLimited,
    Failed,
    Interrupted,
}
impl Outcome {
    fn name(self) -> &'static str {
        match self {
            Self::NotSent => "not_sent",
            Self::Returned => "returned",
            Self::Missing => "missing",
            Self::Pending => "pending",
            Self::Rejected => "rejected",
            Self::AuthDenied => "auth_denied",
            Self::RateLimited => "rate_limited",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }
}

/// Finish a reservation once. `Returned` means attributed usable data, not merely
/// HTTP200. Final data/facts and this completion must commit in one transaction.
pub fn finish(
    conn: &Connection,
    attempt: &Attempt,
    outcome: Outcome,
    status: Option<u16>,
    now: i64,
) -> Result<(), CliError> {
    if status.is_some_and(|v| !(100..=599).contains(&v)) {
        return Err(failure("invalid safe work status"));
    }
    let changed=conn.execute("UPDATE work_attempts SET ended_ms=?2,status=?3,outcome=?4 WHERE attempt_id=?1 AND outcome='started' AND work_id=?5 AND transport_id=?6 AND binding_revision=?7 AND retry_epoch=?8 AND ordinal=?9",params![attempt.id,now,status,outcome.name(),attempt.work,attempt.transport,attempt.revision,attempt.epoch,attempt.ordinal]).map_err(sql_error)?;
    if changed != 1 {
        return Err(failure(
            "attempt was already finished or reservation changed",
        ));
    }
    let active: bool = conn
        .query_row(
            "SELECT binding_revision=?2 AND retry_epoch=?3 FROM sync_work WHERE work_id=?1",
            params![attempt.work, attempt.revision, attempt.epoch],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if !active {
        return Ok(());
    }
    let (state, kind, reason) = match outcome {
        Outcome::NotSent => {
            conn.execute("UPDATE sync_work SET attempts=attempts-1,last_attempt_ms=(SELECT max(started_ms) FROM work_attempts WHERE work_id=?1 AND binding_revision=?2 AND retry_epoch=?3 AND outcome!='not_sent'),eligible_after_ms=(SELECT max(started_ms+CASE WHEN ordinal=1 THEN 60000 ELSE 900000 END) FROM work_attempts WHERE work_id=?1 AND binding_revision=?2 AND retry_epoch=?3 AND outcome!='not_sent'),reason='not_sent' WHERE work_id=?1",params![attempt.work,attempt.revision,attempt.epoch]).map_err(sql_error)?;
            return Ok(());
        }
        Outcome::Returned => (
            "complete",
            Some("completed"),
            "usable attributed data retained",
        ),
        Outcome::AuthDenied => (
            "ineligible",
            Some("ineligible"),
            "application revision authorization denied",
        ),
        _ if attempt.ordinal >= 3 => (
            "deferred",
            Some("deferred"),
            "eligible attempt limit reached",
        ),
        _ => ("pending", None, "retry spacing applies"),
    };
    conn.execute(
        "UPDATE sync_work SET state=?2,reason=?3 WHERE work_id=?1",
        params![attempt.work, state, reason],
    )
    .map_err(sql_error)?;
    if let Some(kind) = kind {
        transition(
            conn,
            &attempt.work,
            now,
            kind,
            attempt.epoch,
            (Some(&attempt.revision), Some(&attempt.revision)),
            reason,
        )?;
    }
    Ok(())
}

/// Explicit local reset preserves all attempt/authorization records and cooldowns.
/// Authorization-ineligible and completed work are never re-enabled by retry.
pub fn reset(conn: &Connection, source: Source, now: i64) -> Result<u64, CliError> {
    let mut query=conn.prepare("SELECT work_id,retry_epoch,binding_revision FROM sync_work WHERE kind=?1 AND state IN ('pending','deferred') AND (attempts>0 OR state='deferred') ORDER BY work_id LIMIT 200").map_err(sql_error)?;
    let rows = query
        .query_map([source.name()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    for (id, epoch, rev) in &rows {
        let next = epoch
            .checked_add(1)
            .ok_or_else(|| failure("retry epoch exhausted"))?;
        transition(
            conn,
            id,
            now,
            "retry_reset",
            next,
            (rev.as_deref(), rev.as_deref()),
            "explicit operator retry reset",
        )?;
        conn.execute("UPDATE sync_work SET retry_epoch=?2,attempts=0,state='pending',last_attempt_ms=NULL,eligible_after_ms=NULL,reason='explicit_retry' WHERE work_id=?1",params![id,next]).map_err(sql_error)?;
    }
    Ok(rows.len() as u64)
}

pub fn complete_cached(conn: &Connection, id: &str, now: i64) -> Result<(), CliError> {
    let (state, epoch, revision): (String, i64, Option<String>) = conn
        .query_row(
            "SELECT state,retry_epoch,binding_revision FROM sync_work WHERE work_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(sql_error)?;
    if state != "complete" {
        transition(
            conn,
            id,
            now,
            "completed",
            epoch,
            (revision.as_deref(), revision.as_deref()),
            "usable previously retained facts; no remote request",
        )?;
        conn.execute(
            "UPDATE sync_work SET state='complete',reason='retained_usable_facts' WHERE work_id=?1",
            [id],
        )
        .map_err(sql_error)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub enum LocalDeferral {
    BoundedView,
    ReportOwnership,
}

pub fn configuration_ineligible(
    conn: &Connection,
    id: &str,
    now: i64,
    reference: Option<&ScannerReference>,
) -> Result<(), CliError> {
    let (epoch, revision): (i64, Option<String>) = conn
        .query_row(
            "SELECT retry_epoch,binding_revision FROM sync_work WHERE work_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(sql_error)?;
    let reason = if reference.is_some() {
        "binding_rebound"
    } else {
        "binding_unavailable"
    };
    let public =
        serde_json::to_value(reference).map_err(|_| failure("invalid public binding context"))?;
    conn.execute(
        "INSERT INTO work_transitions VALUES(?1,?2,?3,'ineligible',?4,?5,?6,?7,?8)",
        params![
            fresh(),
            id,
            now,
            epoch,
            revision,
            reference.map(|r| &r.revision),
            reason,
            encode(&serde_json::json!({"observed_configuration":public,"no_remote_request":true}))?
        ],
    )
    .map_err(sql_error)?;
    conn.execute("UPDATE sync_work SET state='ineligible',reason=?2,provenance_json=json_set(provenance_json,'$.ineligible_configuration',json(?3)) WHERE work_id=?1",params![id,reason,encode(&public)?]).map_err(sql_error)?;
    Ok(())
}
pub fn defer_local(
    conn: &Connection,
    id: &str,
    now: i64,
    reference: &ScannerReference,
    deferral: LocalDeferral,
) -> Result<(), CliError> {
    let (epoch, revision): (i64, Option<String>) = conn
        .query_row(
            "SELECT retry_epoch,binding_revision FROM sync_work WHERE work_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(sql_error)?;
    let reason = match deferral {
        LocalDeferral::BoundedView => "bounded_revisit_view",
        LocalDeferral::ReportOwnership => "report_ownership_unavailable_or_ambiguous",
    };
    transition(
        conn,
        id,
        now,
        "deferred",
        epoch,
        (revision.as_deref(), Some(&reference.revision)),
        reason,
    )?;
    conn.execute("UPDATE sync_work SET state='deferred',reason=?2,binding_revision=?3,provenance_json=json_set(provenance_json,'$.scanner_reference',json(?4),'$.eligibility_checked_without_send',1) WHERE work_id=?1",params![id,reason,reference.revision,encode(reference)?]).map_err(sql_error)?;
    Ok(())
}

#[cfg(test)]
#[path = "work_tests.rs"]
mod tests;
