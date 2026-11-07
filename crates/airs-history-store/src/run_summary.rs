//! One collection run's counts for the service's `collection_start` result (contract
//! §4.3 `result_collection`): facts first seen by this run versus facts it re-observed,
//! and the management reads it actually sent. Read-only, through the bounded reader.

use std::path::Path;

use airs_runtime_core::error::CliError;
use rusqlite::{OptionalExtension, params};

use crate::database::{self, Reader, Writer, sql_error};
use crate::identity::Cloud;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunSummary {
    /// Facts this run linked whose earliest run is this one.
    pub new_facts: u64,
    /// Facts this run linked that an earlier run had already recorded.
    pub duplicate_facts: u64,
    /// Management (API) reads this run recorded as actually sent: OAuth token requests
    /// excluded, from the durable journal.
    pub management_sends: u64,
    /// Journal rows still `started` (sent or not unknown: never proof of no send).
    pub unconfirmed_reads: u64,
    /// OAuth token requests refused by authentication (bad credentials), and refused
    /// transiently (429, 5xx, transport): a token outcome, never an API outcome.
    pub token_denied: u64,
    pub token_transient: u64,
    /// OAuth token refusals (401/403) not followed by a token the same run obtained:
    /// definitive whatever the run's state, since the collector never replays a token
    /// refusal (contract §6.1).
    pub token_terminal_denied: u64,
    /// Required sources the run could not complete (its durable completeness).
    pub unfinished_sources: u64,
    /// Results reads this run attempted (anything but `not_sent`).
    pub results_sends: u64,
    /// Management (API) reads refused by authentication (401/403), and API reads that
    /// returned data (OAuth successes excluded): a run with the first and none of the
    /// second was denied outright.
    pub auth_denied: u64,
    pub returned: u64,
    /// API reads whose authentication refusal is final: a 403, or a 401 not followed by
    /// a successful read of the same operation (its single replay exhausted). A first
    /// 401 recovered by the replay is not terminal; an unrelated successful read never
    /// erases a terminal one.
    pub terminal_denied: u64,
    /// Management (API) reads refused with 429.
    pub rate_limited: u64,
    /// When the latest API 429 of the run was recorded (its durable refusal time, for
    /// a recovered cooldown); `None` when the run saw none.
    pub last_rate_limited_ms: Option<i64>,
    /// Strategic examples: report IDs selected, served from the archive, and Reports
    /// transports actually attempted (each carries up to five IDs), and whether a 429
    /// stopped them.
    pub selected_report_ids: u64,
    pub cached_examples: u64,
    pub reports_sends: u64,
    pub reports_rate_limited: bool,
    /// OAuth token requests the run actually sent (`oauth.token` journal rows).
    pub oauth_sends: u64,
    /// The run's final state (`complete`, `partial`, `interrupted`, `failed`).
    pub state: String,
}

pub fn run_summary(
    root: &Path,
    cloud: &Cloud,
    run_id: &str,
    session: &scm::Session,
) -> Result<Option<RunSummary>, CliError> {
    let Some(read) = Reader::open(root, cloud, session)? else {
        return Ok(None);
    };
    let Some((state, unfinished, selected, cached)) = read
        .conn
        .query_row(
            "SELECT state, coalesce(json_extract(provenance_json,'$.progress.incomplete_required_sources'),0), coalesce(json_extract(provenance_json,'$.progress.selected_report_ids'),0), coalesce(json_extract(provenance_json,'$.progress.cached_examples'),0) FROM collection_runs WHERE run_id=?1",
            params![run_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)),
        )
        .optional()
        .map_err(sql_error)?
    else {
        return Ok(None);
    };
    // A service run counts its facts as it writes them; only a run without that record
    // falls back to the start-time comparison.
    let recorded: Option<(i64, i64)> = read
        .conn
        .query_row(
            "SELECT json_extract(provenance_json,'$.fact_counts.new'), json_extract(provenance_json,'$.fact_counts.duplicate') FROM collection_runs WHERE run_id=?1 AND json_type(provenance_json,'$.fact_counts')='object'",
            params![run_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql_error)?;
    // A fact is new to this run when no earlier-started run links it.
    let (new_facts, duplicate_facts): (i64, i64) = if let Some(counts) = recorded {
        counts
    } else {
        read.conn
            .query_row(
                "WITH mine AS (SELECT fact_id FROM fact_runs WHERE run_id=?1),
                  this AS (SELECT started_ms FROM collection_runs WHERE run_id=?1)
             SELECT
               COALESCE(SUM(NOT EXISTS (
                 SELECT 1 FROM fact_runs o JOIN collection_runs c ON c.run_id=o.run_id
                 WHERE o.fact_id=mine.fact_id AND o.run_id<>?1
                   AND c.started_ms <= (SELECT started_ms FROM this))),0),
               COALESCE(SUM(EXISTS (
                 SELECT 1 FROM fact_runs o JOIN collection_runs c ON c.run_id=o.run_id
                 WHERE o.fact_id=mine.fact_id AND o.run_id<>?1
                   AND c.started_ms <= (SELECT started_ms FROM this))),0)
             FROM mine",
                params![run_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(sql_error)?
    };
    // Classified from the durable journal: `oauth.token` rows are token outcomes, every
    // other row an API (management) outcome.
    let api = |sql: &str| -> Result<i64, CliError> {
        read.conn
            .query_row(&format!("SELECT coalesce(sum({sql}),0) FROM management_reads WHERE run_id=?1 AND source!='oauth.token'"), params![run_id], |r| r.get(0))
            .map_err(sql_error)
    };
    let token = |sql: &str| -> Result<i64, CliError> {
        read.conn
            .query_row(&format!("SELECT coalesce(sum({sql}),0) FROM management_reads WHERE run_id=?1 AND source='oauth.token'"), params![run_id], |r| r.get(0))
            .map_err(sql_error)
    };
    let management_sends = api("actual_sends=1")?;
    let (auth_denied, returned, rate_limited) = (
        api("outcome='auth_denied'")?,
        api("outcome='returned'")?,
        api("outcome='rate_limited'")?,
    );
    let last_rate_limited_ms: Option<i64> = read
        .conn
        .query_row(
            "SELECT max(coalesce(ended_ms, started_ms)) FROM management_reads WHERE run_id=?1 AND source!='oauth.token' AND outcome='rate_limited'",
            params![run_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let terminal_denied: i64 = read
        .conn
        .query_row(
            "SELECT count(*) FROM management_reads d WHERE d.run_id=?1 AND d.source!='oauth.token' AND d.outcome='auth_denied'
               AND (d.status=403 OR NOT EXISTS (
                 SELECT 1 FROM management_reads r WHERE r.run_id=?1 AND r.source=d.source AND r.descriptor_json=d.descriptor_json
                   AND r.read_id<>d.read_id AND r.started_ms>=d.started_ms AND r.outcome='returned'))",
            params![run_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let unconfirmed = api("outcome='started'")? + token("outcome='started'")?;
    let tokens = token("actual_sends=1")?;
    let token_denied = token("outcome='auth_denied'")?;
    let token_transient = token("outcome IN ('rate_limited','failed','interrupted')")?;
    let token_terminal_denied: i64 = read
        .conn
        .query_row(
            &format!(
                "SELECT count(*) FROM management_reads d WHERE d.run_id=?1 AND {TOKEN_TERMINAL}"
            ),
            params![run_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let (reports_sends, limited): (i64, i64) = read
        .conn
        .query_row(
            "SELECT count(DISTINCT transport_id), coalesce(sum(outcome='rate_limited'),0) FROM work_attempts WHERE run_id=?1 AND source='reports' AND outcome!='not_sent'",
            params![run_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(sql_error)?;
    let results_sends: i64 = read
        .conn
        .query_row(
            // Actual Results transports: one reservation batch (at most five IDs) is one
            // transport (`transport_id`); a scanner read is never replayed.
            "SELECT count(DISTINCT transport_id) FROM work_attempts WHERE run_id=?1 AND source='results' AND outcome!='not_sent'",
            params![run_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    Ok(Some(RunSummary {
        new_facts: new_facts.max(0) as u64,
        duplicate_facts: duplicate_facts.max(0) as u64,
        management_sends: management_sends.max(0) as u64,
        results_sends: results_sends.max(0) as u64,
        oauth_sends: tokens.max(0) as u64,
        selected_report_ids: selected.max(0) as u64,
        cached_examples: cached.max(0) as u64,
        reports_sends: reports_sends.max(0) as u64,
        reports_rate_limited: limited > 0,
        auth_denied: auth_denied.max(0) as u64,
        returned: returned.max(0) as u64,
        terminal_denied: terminal_denied.max(0) as u64,
        rate_limited: rate_limited.max(0) as u64,
        last_rate_limited_ms,
        unconfirmed_reads: unconfirmed.max(0) as u64,
        token_denied: token_denied.max(0) as u64,
        token_transient: token_transient.max(0) as u64,
        token_terminal_denied: token_terminal_denied.max(0) as u64,
        unfinished_sources: unfinished.max(0) as u64,
        state,
    }))
}

/// A journal row `d` that is a definitive OAuth credential refusal: the token endpoint
/// answered 401 or 403 and its run obtained no token afterwards (a token started in the
/// same millisecond does not count: ambiguity keeps the refusal).
pub(crate) const TOKEN_TERMINAL: &str = "d.source='oauth.token' AND d.outcome='auth_denied' AND d.status IN (401,403)
    AND NOT EXISTS (SELECT 1 FROM management_reads t WHERE t.run_id=d.run_id AND t.source='oauth.token'
      AND t.read_id<>d.read_id AND t.started_ms>d.started_ms AND t.outcome='returned')";

/// The service's management hold (contract §6, §10), derived from the archive itself
/// so that no finalization path (a cancellation, an expiry, a failed control write)
/// can shorten it: the latest API 429 the journal recorded plus `hold_ms`, or the
/// archive's own management cooldown (a provider `Retry-After`, an OAuth 429), whichever
/// ends later. `None` when nothing holds at `now`. Bounded: only journal rows started
/// within `hold_ms` plus the longest read window are read (an index range).
pub fn management_hold_until(
    root: &Path,
    cloud: &Cloud,
    hold_ms: i64,
    now: i64,
    session: &scm::Session,
) -> Result<Option<i64>, CliError> {
    let Some(read) = Reader::open(root, cloud, session)? else {
        return Ok(None);
    };
    let last_429: Option<i64> = read
        .conn
        .query_row(
            "SELECT max(coalesce(ended_ms, started_ms)) FROM management_reads
              WHERE started_ms>=?1 AND source!='oauth.token' AND outcome='rate_limited'",
            params![now.saturating_sub(hold_ms).saturating_sub(READ_WINDOW_MS)],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let archive: Option<i64> = read
        .conn
        .query_row(
            "SELECT until_ms FROM service_cooldowns WHERE service='management'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    let until = [last_429.map(|at| at.saturating_add(hold_ms)), archive]
        .into_iter()
        .flatten()
        .max();
    Ok(until.filter(|u| *u > now))
}

/// The longest a management read can stay in flight (a tick's 120 s, with margin):
/// bounds the journal scan of [`management_hold_until`].
const READ_WINDOW_MS: i64 = 60 * 60_000;

/// How many of a credential revision's most recent runs [`revision_denied`] reads.
///
/// The service refuses every new run of a revision once any of its runs shows a
/// definitive denial, so a denying run is always among the revision's latest runs; the
/// bound keeps the admission check's cost independent of the archive's age.
pub const RECENT_REVISION_RUNS: i64 = 16;

/// Whether the archive journal records a definitive credential denial of `revision`
/// (contract §6.1): the authority for a revision's denial, whatever the control store
/// holds. A run is bound to `revision` by its provenance (`credential_revision`,
/// recorded before any send) or, for a run recorded before that binding, by `also`
/// (run IDs the caller's own records bind to it). A run denies its revision when:
/// - an API read was refused with a 403;
/// - an API read was refused with a 401 not followed by a successful read of the same
///   operation, once the run is finished, or at once when its single replay was also
///   refused (two refusals of the same operation);
/// - the run is finished, no API read returned data and a read (OAuth included) was
///   refused by authentication.
///
/// - the token endpoint refused the credentials (an OAuth 401 or 403) and the run
///   obtained no token after it, whatever the run's state: the collector never replays a
///   token refusal, so nothing pending can answer it.
///
/// A run still `running` with one unanswered API 401 is not a denial: its replay may
/// still succeed.
pub fn revision_denied(
    root: &Path,
    cloud: &Cloud,
    revision: &str,
    also: &[String],
    session: &scm::Session,
) -> Result<bool, CliError> {
    let Some(read) = Reader::open(root, cloud, session)? else {
        return Ok(false);
    };
    let also = serde_json::json!(also).to_string();
    let denied: i64 = read
        .conn
        .query_row(
            format!("WITH bound AS (
               SELECT run_id, state FROM (
                 SELECT run_id, state FROM collection_runs
                  WHERE json_extract(provenance_json,'$.credential_revision')=?1
                  ORDER BY started_ms DESC LIMIT ?3)
               UNION
               SELECT run_id, state FROM collection_runs WHERE run_id IN (SELECT value FROM json_each(?2)))
             SELECT count(*) FROM bound c WHERE
               EXISTS (SELECT 1 FROM management_reads d WHERE d.run_id=c.run_id AND {TOKEN_TERMINAL})
               OR EXISTS (SELECT 1 FROM management_reads d WHERE d.run_id=c.run_id AND d.source!='oauth.token' AND d.outcome='auth_denied'
                 AND (d.status=403 OR (
                   NOT EXISTS (SELECT 1 FROM management_reads r WHERE r.run_id=c.run_id AND r.source=d.source AND r.descriptor_json=d.descriptor_json
                     AND r.read_id<>d.read_id AND r.started_ms>=d.started_ms AND r.outcome='returned')
                   AND (c.state!='running' OR EXISTS (SELECT 1 FROM management_reads x WHERE x.run_id=c.run_id AND x.source=d.source
                     AND x.descriptor_json=d.descriptor_json AND x.read_id<>d.read_id AND x.outcome='auth_denied')))))
               OR (c.state!='running'
                 AND NOT EXISTS (SELECT 1 FROM management_reads r WHERE r.run_id=c.run_id AND r.source!='oauth.token' AND r.outcome='returned')
                 AND EXISTS (SELECT 1 FROM management_reads d WHERE d.run_id=c.run_id AND d.outcome='auth_denied'))").as_str(),
            params![revision, also, RECENT_REVISION_RUNS],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    Ok(denied > 0)
}

/// Crash point C4 resume: a run left `running` by a process that died is closed as
/// `interrupted` (its started management reads stay unconfirmed, never proof of no
/// send). Returns whether the run was still running.
pub fn interrupt_run(
    root: &Path,
    cloud: &Cloud,
    run_id: &str,
    session: &scm::Session,
) -> Result<bool, CliError> {
    let w = Writer::existing_batch(root, cloud, database::RETAIN_BUDGET, session)?;
    w.conn.execute_batch("BEGIN IMMEDIATE").map_err(sql_error)?;
    let changed = w
        .conn
        .execute(
            "UPDATE collection_runs SET ended_ms=?2, state='interrupted' WHERE run_id=?1 AND state='running'",
            params![run_id, database::now_ms()?],
        )
        .map_err(sql_error)?;
    w.check()?;
    w.conn.execute_batch("COMMIT").map_err(sql_error)?;
    w.check()?;
    Ok(changed == 1)
}

/// Whether an archive run exists (`false` when the archive does not exist yet).
pub fn run_exists(
    root: &Path,
    cloud: &Cloud,
    run_id: &str,
    session: &scm::Session,
) -> Result<bool, CliError> {
    let Some(read) = Reader::open(root, cloud, session)? else {
        return Ok(false);
    };
    read.conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM collection_runs WHERE run_id=?1)",
            params![run_id],
            |r| r.get(0),
        )
        .map_err(sql_error)
}

/// The receipts one `receipts_ingest` run linked, from the run's own `first_accepted`
/// record (written in the transaction that inserted each receipt): `accepted` are the
/// receipts this run inserted first, `duplicates` the rest. No timestamp or run-ID
/// ordering is consulted, so a replay of the same run reports the same counts. A run
/// written before the record existed counts only when it holds nothing; otherwise this
/// errs (the service refuses such a batch `conflict` for explicit reconciliation).
pub fn receipt_run_counts(
    root: &Path,
    cloud: &Cloud,
    run_id: &str,
    session: &scm::Session,
) -> Result<(u64, u64), CliError> {
    let Some(read) = Reader::open(root, cloud, session)? else {
        return Ok((0, 0));
    };
    // A run that recorded its first acceptances (service receipt runs) is counted from
    // that record alone: no clock ordering decides ownership.
    let recorded: Option<(i64, i64)> = read
        .conn
        .query_row(
            "WITH mine AS (SELECT DISTINCT f.receipt_id FROM receipt_runs r JOIN receipt_facts f USING(fact_id) WHERE r.run_id=?1),
                  first AS (SELECT value AS receipt_id FROM collection_runs c, json_each(c.provenance_json,'$.first_accepted') WHERE c.run_id=?1)
             SELECT coalesce(sum(receipt_id IN (SELECT receipt_id FROM first)),0), coalesce(sum(receipt_id NOT IN (SELECT receipt_id FROM first)),0) FROM mine
             WHERE EXISTS (SELECT 1 FROM collection_runs WHERE run_id=?1 AND json_type(provenance_json,'$.first_accepted')='array')",
            params![run_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql_error)?;
    let has_record: bool = read
        .conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM collection_runs WHERE run_id=?1 AND json_type(provenance_json,'$.first_accepted')='array')",
            params![run_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if has_record {
        let (a, d) = recorded.unwrap_or((0, 0));
        return Ok((a.max(0) as u64, d.max(0) as u64));
    }
    // A run without the record is counted only when it holds nothing: ownership of an
    // older run's receipts is never inferred from timestamps (M4 review 13).
    match receipt_run_record(root, cloud, run_id, session)? {
        ReceiptRunRecord::Absent | ReceiptRunRecord::Unrecorded { members: 0 } => Ok((0, 0)),
        _ => Err(crate::failure(
            "the receipt run predates its acceptance record; its counts cannot be proven",
        )),
    }
}

/// What a receipt run records about its first acceptances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiptRunRecord {
    /// No such run (nothing was written under it).
    Absent,
    /// The run carries its `first_accepted` record: its counts are proven.
    Recorded,
    /// A run written before the record existed, with this many receipt facts linked.
    Unrecorded { members: u64 },
}

/// Whether a receipt run exists and carries its first-acceptance record.
pub fn receipt_run_record(
    root: &Path,
    cloud: &Cloud,
    run_id: &str,
    session: &scm::Session,
) -> Result<ReceiptRunRecord, CliError> {
    let Some(read) = Reader::open(root, cloud, session)? else {
        return Ok(ReceiptRunRecord::Absent);
    };
    let row: Option<(bool, i64)> = read
        .conn
        .query_row(
            "SELECT json_type(provenance_json,'$.first_accepted')='array', (SELECT count(*) FROM receipt_runs WHERE run_id=?1) FROM collection_runs WHERE run_id=?1",
            params![run_id],
            |r| Ok((r.get::<_, Option<bool>>(0)?.unwrap_or(false), r.get(1)?)),
        )
        .optional()
        .map_err(sql_error)?;
    Ok(match row {
        None => ReceiptRunRecord::Absent,
        Some((true, _)) => ReceiptRunRecord::Recorded,
        Some((false, n)) => ReceiptRunRecord::Unrecorded {
            members: n.max(0) as u64,
        },
    })
}

/// Test support: an archive run left by a process that died after recording it, with
/// the given `(source, outcome, status)` management reads (crash-point probes).
#[doc(hidden)]
pub fn seed_run_for_tests(
    root: &Path,
    cloud: &Cloud,
    run_id: &str,
    target: (i64, i64),
    state: &str,
    reads: &[(&str, &str, i64)],
) -> Result<(), CliError> {
    seed_bound_run_for_tests(root, cloud, run_id, target, state, reads, None)
}

/// Test support: [`seed_run_for_tests`] with the run bound to a credential revision in
/// its provenance, as the service records it.
/// Every journal row shares one descriptor (a read and its replays).
#[doc(hidden)]
pub fn seed_bound_run_for_tests(
    root: &Path,
    cloud: &Cloud,
    run_id: &str,
    target: (i64, i64),
    state: &str,
    reads: &[(&str, &str, i64)],
    revision: Option<&str>,
) -> Result<(), CliError> {
    let reads: Vec<_> = reads.iter().map(|(s, o, c)| (*s, *o, *c, "{}")).collect();
    seed_bound_run_with_descriptors_for_tests(root, cloud, run_id, target, state, &reads, revision)
}

/// Test support: [`seed_bound_run_for_tests`] with an explicit descriptor per journal
/// row (the transport's logical identity and `replay_chain`).
#[doc(hidden)]
pub fn seed_bound_run_with_descriptors_for_tests(
    root: &Path,
    cloud: &Cloud,
    run_id: &str,
    target: (i64, i64),
    state: &str,
    reads: &[(&str, &str, i64, &str)],
    revision: Option<&str>,
) -> Result<(), CliError> {
    let session = scm::Session::new();
    let w = Writer::open(
        root,
        cloud,
        std::time::Duration::from_secs(10),
        true,
        &session,
    )?;
    w.conn.execute_batch("BEGIN IMMEDIATE").map_err(sql_error)?;
    let now = database::now_ms()?;
    w.conn
        .execute(
            "INSERT INTO collection_runs VALUES (?1,'sync',?2,?2,?3,?4,?5,?6)",
            params![
                run_id,
                now,
                target.0,
                target.1,
                state,
                revision
                    .map_or(
                        serde_json::json!({}),
                        |r| serde_json::json!({"credential_revision": r})
                    )
                    .to_string()
            ],
        )
        .map_err(sql_error)?;
    for (n, (source, outcome, status, descriptor)) in reads.iter().enumerate() {
        w.conn
            .execute(
                "INSERT INTO management_reads VALUES (?1,?2,?3,NULL,NULL,?4,?4,?5,1,NULL,?6,?7)",
                params![
                    format!("{run_id}-read-{n}"),
                    run_id,
                    source,
                    now,
                    status,
                    outcome,
                    descriptor
                ],
            )
            .map_err(sql_error)?;
    }
    w.conn.execute_batch("COMMIT").map_err(sql_error)?;
    Ok(())
}

/// Test support: an archive run, left in `state`, whose management read was refused
/// with a 429 at `at_ms`, with the companion archive cooldown the collector commits
/// atomically with that refusal (`Retry-After: retry_after_seconds`).
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn seed_rate_limited_run_for_tests(
    root: &Path,
    cloud: &Cloud,
    run_id: &str,
    target: (i64, i64),
    state: &str,
    at_ms: i64,
    retry_after_seconds: Option<u64>,
    revision: Option<&str>,
) -> Result<(), CliError> {
    seed_bound_run_for_tests(root, cloud, run_id, target, state, &[], revision)?;
    let session = scm::Session::new();
    let w = Writer::existing_batch(root, cloud, std::time::Duration::from_secs(10), &session)?;
    w.conn.execute_batch("BEGIN IMMEDIATE").map_err(sql_error)?;
    w.conn
        .execute(
            "INSERT INTO management_reads VALUES (?1,?2,'airs.management.dashboard.sessions_overview',NULL,NULL,?3,?3,429,1,NULL,'rate_limited','{}')",
            params![format!("{run_id}-read-429"), run_id, at_ms],
        )
        .map_err(sql_error)?;
    crate::work::cooldown_service(&w.conn, "management", at_ms, retry_after_seconds)?;
    w.conn.execute_batch("COMMIT").map_err(sql_error)?;
    Ok(())
}
