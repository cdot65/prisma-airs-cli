//! Historical report input (plan M6.1): the archive's committed evidence for one
//! half-open UTC period, as the Signal Brief `Evidence` the CLI renderer already
//! renders. Read-only, through the bounded reader; no network.
//!
//! - Events: every event whose observed time is in `[since, until)`, first seen no
//!   later than the cutoff (a revision's evidence cutoff), optionally limited to
//!   application IDs, rebuilt from immutable facts (`cached_event`). Above the
//!   10,000-event evidence bound the input is truncated deterministically and the gap
//!   is stated; it is never extrapolated.
//! - Events without an observed time cannot be placed in the period: they are counted
//!   and stated as a gap, never silently included or dropped.
//! - Profiles and applications: the latest observation at or before the period end,
//!   by observation link (`profile_runs`/`application_runs`), so a configuration seen
//!   again after a different one (A, B, A) is current again; configuration after the
//!   period is never used as historical fact.
//! - Summary: full-cohort counts by Chicago report date, application, verdict and
//!   action, and detector-positive event counts by detector, independent of the
//!   10,000-event detail bound, within one budget of summary cells. A filter dimension
//!   that does not fit is disabled and stated, never computed from capped detail.
//! - Profile timeline: each profile's observations in the period (and the one current
//!   at its start), collapsed to changes; differences between observations, not exact
//!   effective change times.
//! - Sources: one per coverage category, `complete` only when every coverage group of
//!   that category is complete for the period.

use std::collections::BTreeMap;
use std::path::Path;

use airs_runtime_core::error::CliError;
use airs_runtime_core::signal::evidence::{
    Application, Budgets, Event, Evidence, Identity, Profile, Sampling, Source, Status, Window,
};
use rusqlite::params;

use crate::database::{Reader, sql_error};
use crate::identity::Cloud;

/// The evidence bound (Signal Brief evidence version 1).
pub const MAX_EVENTS: usize = 10_000;
/// The interactive summary budget (plan: at most 50,000 summary cells). One cell is
/// one row of a summary table: a distinct key with its count.
pub const MAX_SUMMARY_CELLS: usize = 50_000;
/// The profile timeline display bound.
pub const MAX_TIMELINE: usize = 1_000;
/// One coherent read for a report's summary, detail and posture.
pub const REPORT_READ_BUDGET: std::time::Duration = std::time::Duration::from_secs(60);

/// Filter dimensions of the event summary, in key order.
pub const DIMENSIONS: [&str; 4] = ["day", "application", "verdict", "action"];

/// One summary row: its key (aligned with `Summary::dimensions`, the detector last
/// for detector rows) and its count of distinct events.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Cell {
    pub key: Vec<String>,
    pub events: u64,
}

/// Full-cohort summary of a report period.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Summary {
    /// Active dimensions of `cells` (a subset of `DIMENSIONS`, in order).
    pub dimensions: Vec<&'static str>,
    /// Every event of the cohort is counted in exactly one cell.
    pub cells: Vec<Cell>,
    /// Detector-positive events per detector (key: dimensions, then detector). Rows
    /// of different detectors overlap and are never summed. `None` when disabled.
    pub detector_cells: Option<Vec<Cell>>,
    /// Filters disabled because their dimension exceeded the cell budget.
    pub disabled_filters: Vec<&'static str>,
    /// The full cohort: events observed in the period, first seen by the cutoff.
    pub events: u64,
    pub cell_budget: usize,
}

/// One span of a profile's observed configuration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TimelineEntry {
    pub profile: String,
    pub revision: Option<u64>,
    /// The first and last observation of this configuration in the span.
    pub first_observed_ms: i64,
    pub last_observed_ms: i64,
    /// A short stable identifier of the observed configuration.
    pub observation: String,
    /// Whether this span began before the period (the configuration at its start).
    pub at_start: bool,
}

/// A period's report input.
pub struct ReportInput {
    pub evidence: Evidence,
    /// Report-relevant digest: the evidence without run identifiers or observation
    /// times, so identical evidence yields an identical digest.
    pub digest: String,
    pub gaps: Vec<String>,
    pub complete: bool,
    pub summary: Summary,
    pub timeline: Vec<TimelineEntry>,
    pub timeline_truncated: bool,
}

fn failure(message: &'static str) -> CliError {
    CliError::operational(message)
}

#[allow(clippy::too_many_arguments)]
pub fn report_evidence(
    root: &Path,
    cloud: &Cloud,
    tenant: &str,
    period: (i64, i64),
    apps: &[String],
    cutoff_ms: i64,
    version: &str,
    session: &scm::Session,
) -> Result<Option<ReportInput>, CliError> {
    report_evidence_within(
        root,
        cloud,
        tenant,
        period,
        apps,
        cutoff_ms,
        version,
        MAX_SUMMARY_CELLS,
        session,
    )
}

/// `report_evidence` with an explicit summary cell budget (tests pin the fallback).
#[allow(clippy::too_many_arguments)]
pub fn report_evidence_within(
    root: &Path,
    cloud: &Cloud,
    tenant: &str,
    (since, until): (i64, i64),
    apps: &[String],
    cutoff_ms: i64,
    version: &str,
    cell_budget: usize,
    session: &scm::Session,
) -> Result<Option<ReportInput>, CliError> {
    let hours = (until - since) / 3_600_000;
    if since >= until || (until - since) % 3_600_000 != 0 || hours < 1 {
        return Err(failure("a report period must be a whole number of hours"));
    }
    let Some(read) = Reader::within(root, cloud, REPORT_READ_BUDGET, session)? else {
        return Ok(None);
    };
    // Everything below, coverage included, is read in this one read transaction and as of
    // the cutoff (M6 review 1 finding 1).
    let groups =
        crate::inspect::coverage_groups_conn(&read.conn, since, until, apps, Some(cutoff_ms))?;
    let (late, receipts) = late_as_of(&read.conn, cutoff_ms)?;
    let late_ids = serde_json::to_string(&late.keys().collect::<Vec<_>>())
        .map_err(|_| failure("invalid late-event set"))?;
    let app_json =
        serde_json::to_string(apps).map_err(|_| failure("invalid application filter"))?;
    let mut gaps = Vec::new();
    let identities: Vec<Identity> = {
        let mut stmt = read
            .conn
            .prepare(
                "SELECT event_ms,event_id,app_id,app_name,session_id,scan_id,sub_request FROM events
                 WHERE event_ms>=?1 AND event_ms<?2 AND first_seen_ms<=?3
                 AND (json_array_length(?4)=0 OR app_id IN (SELECT value FROM json_each(?4)))
                 AND event_id NOT IN (SELECT value FROM json_each(?6))
                 ORDER BY event_ms, event_id LIMIT ?5",
            )
            .map_err(sql_error)?;
        let mut ordered: Vec<(i64, String, Identity)> = stmt
            .query_map(
                params![
                    since,
                    until,
                    cutoff_ms,
                    app_json,
                    (MAX_EVENTS + 1) as i64,
                    late_ids
                ],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        Identity {
                            app_id: r.get(2)?,
                            app_name: r.get(3)?,
                            session_id: r.get(4)?,
                            scan_id: r.get(5)?,
                            sub_request: r.get::<_, Option<i64>>(6)?.map(|n| n.max(0) as u64),
                        },
                    ))
                },
            )
            .map_err(sql_error)?
            .collect::<Result<_, _>>()
            .map_err(sql_error)?;
        for (id, l) in &late {
            if late_in_scope(l, (since, until), apps, cutoff_ms) {
                ordered.push((
                    l.projection.event_ms.unwrap_or_default(),
                    id.clone(),
                    l.identity.clone(),
                ));
            }
        }
        ordered.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        ordered.truncate(MAX_EVENTS + 1);
        ordered
            .into_iter()
            .map(|(_, _, identity)| identity)
            .collect()
    };
    let truncated = identities.len() > MAX_EVENTS;
    if truncated {
        gaps.push(format!(
            "more than {MAX_EVENTS} events in the period; the first {MAX_EVENTS} by time are shown and nothing is extrapolated"
        ));
    }
    let mut events: Vec<Event> = Vec::new();
    for identity in identities.into_iter().take(MAX_EVENTS) {
        let event = match late.values().find(|l| l.identity == identity) {
            Some(l) => crate::repository::cached_event_as_of(
                &read.conn,
                &identity,
                cutoff_ms,
                &l.projection,
                &receipts,
            )?,
            None => crate::repository::cached_event(&read.conn, &identity)?,
        };
        if let Some(event) = event {
            events.push(event);
        } else {
            gaps.push("an event too large to rebuild was omitted".into());
        }
    }
    let unknown_time: i64 = read
        .conn
        .query_row(
            "SELECT count(*) FROM events WHERE event_ms IS NULL AND first_seen_ms>=?1 AND first_seen_ms<?2
             AND first_seen_ms<=?4 AND (json_array_length(?3)=0 OR app_id IN (SELECT value FROM json_each(?3)))
             AND event_id NOT IN (SELECT value FROM json_each(?5))",
            params![since, until, app_json, cutoff_ms, late_ids],
            |r| r.get::<_, i64>(0),
        )
        .map_err(sql_error)?
        + late
            .values()
            .filter(|l| {
                l.projection.event_ms.is_none()
                    && l.first_seen_ms >= since
                    && l.first_seen_ms < until
                    && (apps.is_empty() || apps.contains(&l.identity.app_id))
            })
            .count() as i64;
    if unknown_time > 0 {
        gaps.push(format!(
            "{unknown_time} events first seen in the period have no observed time and are not placed in it"
        ));
    }
    let profiles: Vec<Profile> = latest(
        &read.conn,
        "SELECT o.fields_json FROM profile_observations o JOIN profile_runs r ON r.observation_id=o.observation_id
         WHERE r.observed_ms<=?1 AND r.observed_ms=(SELECT max(r2.observed_ms) FROM profile_runs r2
           JOIN profile_observations o2 ON o2.observation_id=r2.observation_id
           WHERE o2.profile_id=o.profile_id AND r2.observed_ms<=?1)
         GROUP BY o.observation_id ORDER BY o.profile_id, o.observation_id",
        until,
    )?
    .into_iter()
    .filter_map(|v| serde_json::from_value(v["profile"].clone()).ok())
    .collect();
    let applications: Vec<Application> = latest(
        &read.conn,
        "SELECT o.fields_json FROM application_observations o JOIN application_runs r ON r.observation_id=o.observation_id
         WHERE r.observed_ms<=?1 AND r.observed_ms=(SELECT max(r2.observed_ms) FROM application_runs r2
           JOIN application_observations o2 ON o2.observation_id=r2.observation_id
           WHERE o2.application_key=o.application_key AND r2.observed_ms<=?1)
         GROUP BY o.observation_id ORDER BY o.application_key, o.observation_id",
        until,
    )?
    .into_iter()
    .filter_map(|v| serde_json::from_value(v["application"].clone()).ok())
    .collect();
    let summary = summarize(
        &read.conn,
        (since, until),
        &app_json,
        cutoff_ms,
        cell_budget,
    )?;
    let (timeline, timeline_truncated) = profile_timeline(&read.conn, (since, until))?;
    if !summary.disabled_filters.is_empty() {
        gaps.push(format!(
            "the {} filter(s) exceed the {} summary-cell budget and are disabled; report a narrower range or application scope to filter by them",
            summary.disabled_filters.join(", "),
            summary.cell_budget
        ));
    }
    drop(read);
    // Coverage per category, conservatively.
    let mut categories: BTreeMap<String, (bool, Vec<String>)> = BTreeMap::new();
    for g in groups {
        let entry = categories
            .entry(g.source.to_string())
            .or_insert((true, Vec::new()));
        if g.state != "complete" {
            entry.0 = false;
            entry.1.push(format!(
                "{} coverage is {} for {}",
                g.source,
                g.state,
                g.application_id.as_deref().unwrap_or("the tenant")
            ));
        }
    }
    let mut sources: Vec<Source> = categories
        .iter()
        .map(|(name, (complete, notes))| Source {
            name: name.clone(),
            status: if *complete {
                Status::Complete
            } else {
                Status::Partial
            },
            required: matches!(name.as_str(), "sessions" | "results"),
            records: 0,
            pages: 0,
            unit: "archive coverage groups".into(),
            population: "retained history".into(),
            started_ms: since,
            ended_ms: until,
            notes: notes.clone(),
        })
        .collect();
    if sources.is_empty() {
        sources.push(Source {
            name: "sessions".into(),
            status: Status::Unavailable,
            required: true,
            records: 0,
            pages: 0,
            unit: "archive coverage groups".into(),
            population: "retained history".into(),
            started_ms: since,
            ended_ms: until,
            notes: vec!["no collection covered this period".into()],
        });
        gaps.push("no collection covered this period".into());
    }
    for (complete, notes) in categories.values() {
        if !complete {
            gaps.extend(notes.iter().cloned());
        }
    }
    let complete = gaps.is_empty() && sources.iter().all(|s| s.status == Status::Complete);
    // Every source an event cites is listed, with the status of its coverage category.
    let cited: std::collections::BTreeSet<String> = events
        .iter()
        .flat_map(|e| e.sources.iter().cloned())
        .collect();
    for name in cited {
        if sources.iter().any(|x| x.name == name) {
            continue;
        }
        let category = crate::inspect::source_category(&name).unwrap_or("unknown");
        let complete = categories.get(category).is_some_and(|c| c.0);
        sources.push(Source {
            name: name.clone(),
            status: if complete {
                Status::Complete
            } else {
                Status::Partial
            },
            required: false,
            records: 0,
            pages: 0,
            unit: "archived observations".into(),
            population: "retained history".into(),
            started_ms: since,
            ended_ms: until,
            notes: vec![format!("coverage category {category}")],
        });
    }
    for event in &mut events {
        if event.sources.is_empty() {
            event.sources.push(sources[0].name.clone());
        }
    }
    let mut evidence = Evidence {
        schema_version: 1,
        rules_version: 2,
        redaction_version: 1,
        collection_id: uuid::Uuid::new_v4().to_string(),
        digest: String::new(),
        cli_version: version.into(),
        sdk_version: "0.2.4".into(),
        tenant: tenant.into(),
        app_ids: apps.to_vec(),
        window: Window {
            start_ms: since,
            end_ms: until,
            interval: hours as u64,
            unit: "hours".into(),
        },
        collected_ms: cutoff_ms.max(until),
        sources,
        events,
        profiles,
        applications,
        metrics: vec![],
        sampling: Sampling {
            algorithm: "coverage-greedy-v2".into(),
            candidates: 0,
            selected_report_ids: vec![],
            uncovered_groups: vec![],
            limit: 0,
        },
        budgets: Budgets {
            seconds: 120,
            concurrency: 1,
            requests: 1,
            max_pages: 1,
            max_events: MAX_EVENTS as u64,
            max_reports: 0,
            attempts: 0,
        },
    };
    evidence
        .seal_native()
        .map_err(|_| failure("archived evidence cannot form a valid report"))?;
    let mut stable = serde_json::to_value(&evidence).map_err(|_| failure("invalid evidence"))?;
    if let Some(o) = stable.as_object_mut() {
        o.remove("collection_id");
        o.remove("collected_ms");
        o.remove("digest");
    }
    let digest = {
        use sha2::Digest;
        let bytes = serde_json::to_vec(&(stable, &gaps, &summary, &timeline))
            .map_err(|_| failure("invalid evidence"))?;
        hex(&sha2::Sha256::digest(bytes))
    };
    Ok(Some(ReportInput {
        evidence,
        digest,
        gaps,
        complete,
        summary,
        timeline,
        timeline_truncated,
    }))
}

/// The Chicago report date of `e.event_ms`, as SQL over compiled integer offsets.
fn day_expression(since: i64, until: i64) -> Result<String, CliError> {
    let segments = airs_runtime_core::schedule::report_offset_segments(since, until)
        .map_err(|_| failure("the report timezone is unavailable"))?;
    let mut case = String::from("CASE");
    for (end, offset) in &segments {
        case.push_str(&format!(" WHEN e.event_ms<{end} THEN {offset}"));
    }
    case.push_str(&format!(" ELSE {} END", segments.last().map_or(0, |s| s.1)));
    Ok(format!("date((e.event_ms+({case}))/1000,'unixepoch')"))
}

/// An event with a fact first observed after a report's evidence cutoff, with its
/// projection as of that cutoff (M6 review 1 finding 1).
struct Late {
    identity: Identity,
    first_seen_ms: i64,
    projection: crate::repository::Projection,
}

/// Every event first seen by `cutoff_ms` that has a fact (or a promoted receipt fact)
/// first observed after it, with its projection rebuilt from the facts observed by the
/// cutoff. Every other event's current projection already equals its as-of projection,
/// so only these (normally few) are rebuilt, read-only, in the report's transaction.
fn late_as_of(
    conn: &rusqlite::Connection,
    cutoff_ms: i64,
) -> Result<(BTreeMap<String, Late>, crate::repository::ReceiptsAsOf), CliError> {
    // Receipt ownership as of the cutoff: an event whose receipts then differ from its
    // receipts now (a later promotion, demotion or reassignment) is rebuilt too, even
    // with no later fact of its own (M6 review 2 finding 1).
    let receipts = crate::repository::receipts_as_of(conn, cutoff_ms)?;
    let reassigned = serde_json::to_string(&receipts.affected_events())
        .map_err(|_| failure("invalid receipt reassignment set"))?;
    let rows: Vec<(String, Identity, i64)> = {
        let mut stmt = conn
            .prepare(
                "SELECT event_id,app_id,app_name,session_id,scan_id,sub_request,first_seen_ms FROM events
                 WHERE first_seen_ms<=?1 AND event_id IN (
                   SELECT event_id FROM event_facts WHERE observed_ms>?1
                   UNION SELECT r.promoted_event_id FROM receipt_facts f JOIN receipts r ON r.receipt_id=f.receipt_id
                     WHERE f.observed_ms>?1 AND r.promoted_event_id IS NOT NULL
                   UNION SELECT value FROM json_each(?2))
                 ORDER BY event_id",
            )
            .map_err(sql_error)?;
        stmt.query_map(rusqlite::params![cutoff_ms, reassigned], |r| {
            Ok((
                r.get(0)?,
                Identity {
                    app_id: r.get(1)?,
                    app_name: r.get(2)?,
                    session_id: r.get(3)?,
                    scan_id: r.get(4)?,
                    sub_request: r.get::<_, Option<i64>>(5)?.map(|n| n.max(0) as u64),
                },
                r.get(6)?,
            ))
        })
        .map_err(sql_error)?
        .collect::<Result<_, _>>()
        .map_err(sql_error)?
    };
    let mut out = BTreeMap::new();
    for (id, identity, first_seen_ms) in rows {
        let projection = crate::repository::projection_with(conn, &id, Some(cutoff_ms), &receipts)?;
        out.insert(
            id,
            Late {
                identity,
                first_seen_ms,
                projection,
            },
        );
    }
    Ok((out, receipts))
}

/// Whether a late event, as of the cutoff, belongs to the report's cohort.
fn late_in_scope(late: &Late, (since, until): (i64, i64), apps: &[String], cutoff_ms: i64) -> bool {
    late.first_seen_ms <= cutoff_ms
        && late
            .projection
            .event_ms
            .is_some_and(|at| at >= since && at < until)
        && (apps.is_empty() || apps.contains(&late.identity.app_id))
}

/// The full-cohort summary within `budget` cells. Plans are tried in order; the first
/// whose event and detector tables together fit is used. The detector filter is
/// disabled first, then the date filter, then the application filter; the verdict and
/// action dimensions (at most 16 cells) always fit.
fn summarize(
    conn: &rusqlite::Connection,
    (since, until): (i64, i64),
    app_json: &str,
    cutoff_ms: i64,
    budget: usize,
) -> Result<Summary, CliError> {
    // As of the cutoff: the SQL counts every event except the late ones, whose as-of
    // projections are added in Rust (M6 review 1 finding 1).
    let (late, _) = late_as_of(conn, cutoff_ms)?;
    let late_ids = serde_json::to_string(&late.keys().collect::<Vec<_>>())
        .map_err(|_| failure("invalid late-event set"))?;
    let apps: Vec<String> =
        serde_json::from_str(app_json).map_err(|_| failure("invalid application filter"))?;
    let late_in: Vec<&Late> = late
        .values()
        .filter(|l| late_in_scope(l, (since, until), &apps, cutoff_ms))
        .collect();
    let late_key = |l: &Late, d: &str| -> Result<String, CliError> {
        Ok(match d {
            "day" => {
                airs_runtime_core::schedule::report_date(l.projection.event_ms.unwrap_or_default())
                    .map_err(|_| failure("the report timezone is unavailable"))?
            }
            "application" => l.identity.app_id.clone(),
            "verdict" => l.projection.verdict.clone(),
            _ => l.projection.action.clone(),
        })
    };
    let merge = |cells: &mut Vec<Cell>, key: Vec<String>| match cells
        .binary_search_by(|c| c.key.cmp(&key))
    {
        Ok(i) => cells[i].events += 1,
        Err(i) => cells.insert(i, Cell { key, events: 1 }),
    };
    let day = day_expression(since, until)?;
    let column = |d: &str| match d {
        "day" => day.clone(),
        "application" => "e.app_id".to_owned(),
        "verdict" => "e.verdict".to_owned(),
        _ => "e.action".to_owned(),
    };
    let scope = "e.event_ms>=?1 AND e.event_ms<?2 AND e.first_seen_ms<=?3
        AND (json_array_length(?4)=0 OR e.app_id IN (SELECT value FROM json_each(?4)))
        AND e.event_id NOT IN (SELECT value FROM json_each(?6))";
    let positive =
        "(d.verdict='malicious' OR json_extract(d.provenance_json,'$.positive_metadata')=1
        OR json_extract(d.provenance_json,'$.comparison_subsumed')=1)";
    let rows = |sql: &str, width: usize, limit: usize| -> Result<Vec<Cell>, CliError> {
        let mut stmt = conn.prepare(sql).map_err(sql_error)?;
        stmt.query_map(
            params![
                since,
                until,
                cutoff_ms,
                app_json,
                (limit + 1) as i64,
                late_ids
            ],
            |r| {
                let mut key = Vec::with_capacity(width);
                for i in 0..width {
                    key.push(r.get::<_, Option<String>>(i)?.unwrap_or_default());
                }
                Ok(Cell {
                    key,
                    events: r.get::<_, i64>(width)?.max(0) as u64,
                })
            },
        )
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)
    };
    let plans: [(&[&'static str], bool, &[&'static str]); 4] = [
        (&DIMENSIONS, true, &[]),
        (&DIMENSIONS, false, &["detector"]),
        (
            &["application", "verdict", "action"],
            false,
            &["detector", "date"],
        ),
        (
            &["verdict", "action"],
            false,
            &["detector", "date", "application"],
        ),
    ];
    for (dims, detectors, disabled) in plans {
        let columns: Vec<String> = dims.iter().map(|d| column(d)).collect();
        let order = (1..=dims.len())
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let mut cells = rows(
            &format!(
                "SELECT {}, count(*) FROM events e WHERE {scope} GROUP BY {order} ORDER BY {order} LIMIT ?5",
                columns.join(",")
            ),
            dims.len(),
            budget,
        )?;
        if cells.len() > budget {
            continue;
        }
        for l in &late_in {
            let key = dims
                .iter()
                .map(|d| late_key(l, d))
                .collect::<Result<Vec<_>, _>>()?;
            merge(&mut cells, key);
        }
        if cells.len() > budget {
            continue;
        }
        let detector_cells = if detectors {
            let order = (1..=dims.len() + 1)
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let mut d = rows(
                &format!(
                    "SELECT {}, d.detector, count(DISTINCT e.event_id) FROM events e
                     JOIN detector_observations d ON d.event_id=e.event_id
                     WHERE {scope} AND {positive} GROUP BY {order} ORDER BY {order} LIMIT ?5",
                    columns.join(",")
                ),
                dims.len() + 1,
                budget - cells.len(),
            )?;
            for l in &late_in {
                for detector in l.projection.positive_detectors() {
                    let mut key = dims
                        .iter()
                        .map(|dim| late_key(l, dim))
                        .collect::<Result<Vec<_>, _>>()?;
                    key.push(detector);
                    merge(&mut d, key);
                }
            }
            if cells.len() + d.len() > budget {
                continue;
            }
            Some(d)
        } else {
            None
        };
        let events = cells.iter().map(|c| c.events).sum();
        return Ok(Summary {
            dimensions: dims.to_vec(),
            cells,
            detector_cells,
            disabled_filters: disabled.to_vec(),
            events,
            cell_budget: budget,
        });
    }
    Err(failure("the report summary does not fit its cell budget"))
}

/// Each profile's observed configurations over the period: the one current at its
/// start, then every change, by observation link time.
fn profile_timeline(
    conn: &rusqlite::Connection,
    (since, until): (i64, i64),
) -> Result<(Vec<TimelineEntry>, bool), CliError> {
    let mut stmt = conn
        .prepare(
            "SELECT o.profile_id, o.profile_name, o.revision, o.observation_id, r.observed_ms
             FROM profile_runs r JOIN profile_observations o ON o.observation_id=r.observation_id
             WHERE r.observed_ms<=?2 AND (r.observed_ms>?1 OR r.observed_ms=(
               SELECT max(r2.observed_ms) FROM profile_runs r2 JOIN profile_observations o2
               ON o2.observation_id=r2.observation_id WHERE o2.profile_id=o.profile_id AND r2.observed_ms<=?1))
             ORDER BY o.profile_id, r.observed_ms, o.observation_id",
        )
        .map_err(sql_error)?;
    let rows = stmt
        .query_map(params![since, until], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    let mut out: Vec<(String, String, TimelineEntry)> = Vec::new();
    for (profile_id, name, revision, observation, at) in rows {
        if let Some((id, obs, last)) = out.last_mut()
            && *id == profile_id
            && *obs == observation
        {
            last.last_observed_ms = at;
            continue;
        }
        out.push((
            profile_id,
            observation.clone(),
            TimelineEntry {
                profile: name,
                revision: revision.map(|n| n.max(0) as u64),
                first_observed_ms: at,
                last_observed_ms: at,
                observation: observation.chars().take(12).collect(),
                at_start: at <= since,
            },
        ));
    }
    let truncated = out.len() > MAX_TIMELINE;
    Ok((
        out.into_iter()
            .take(MAX_TIMELINE)
            .map(|(_, _, e)| e)
            .collect(),
        truncated,
    ))
}

fn latest(
    conn: &rusqlite::Connection,
    sql: &str,
    until: i64,
) -> Result<Vec<serde_json::Value>, CliError> {
    let mut stmt = conn.prepare(sql).map_err(sql_error)?;
    stmt.query_map([until], |r| r.get::<_, String>(0))
        .map_err(sql_error)?
        .map(|r| {
            r.map_err(sql_error)
                .map(|t| serde_json::from_str(&t).unwrap_or_default())
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
