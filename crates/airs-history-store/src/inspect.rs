//! Coherent bounded inspection. Rendering happens after the reader/gate is released.
use super::{
    database::{self, Reader, sql_error},
    failure,
    identity::Cloud,
};
use airs_runtime_core::error::CliError;
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

pub fn status(
    root: &Path,
    cloud: &Cloud,
    full: Option<Duration>,
    session: &scm::Session,
) -> Result<Value, CliError> {
    let read = if let Some(budget) = full {
        Reader::maintenance(root, cloud, budget, session)?
    } else {
        Reader::open(root, cloud, session)?
    };
    let Some(read) = read else {
        if full.is_some() {
            return Err(failure(
                "archive is missing; run explicit sync or import before full statistics",
            ));
        }
        return Ok(
            json!({"kind":"runtime_history_status","archive_present":false,"cloud_id":cloud.cloud_id,"environment_id":cloud.environment_id,"tsg_id":cloud.tsg,"events":null,"distinct_scan_ids":null,"partial_receipts":null,"database_bytes":null,"oldest_event_ms":null,"latest_event_ms":null}),
        );
    };
    if full.is_some() {
        let check: String = read
            .conn
            .query_row("PRAGMA integrity_check(1)", [], |r| r.get(0))
            .map_err(sql_error)?;
        if check != "ok"
            || read
                .conn
                .prepare("PRAGMA foreign_key_check")
                .map_err(sql_error)?
                .exists([])
                .map_err(sql_error)?
        {
            return Err(failure(
                "archive integrity check failed; no automatic recovery was attempted",
            ));
        }
    }
    let count = |table: &str| -> Result<i64, CliError> {
        // Only fixed internal table identifiers, never user-supplied SQL.
        let sql = match table {
            "events" => "SELECT count(*) FROM events",
            "receipts" => "SELECT count(*) FROM receipts WHERE state!='promoted'",
            "facts" => "SELECT count(*) FROM event_facts",
            "profiles" => "SELECT count(*) FROM profile_observations",
            "applications" => "SELECT count(*) FROM application_observations",
            "incomplete_runs" => "SELECT count(*) FROM collection_runs WHERE state!='complete'",
            _ => return Err(failure("invalid internal history counter")),
        };
        read.conn
            .query_row(sql, [], |r| r.get(0))
            .map_err(sql_error)
    };
    let events = count("events")?;
    let partial = count("receipts")?;
    let facts = count("facts")?;
    let profiles = count("profiles")?;
    let applications = count("applications")?;
    let incomplete_runs = count("incomplete_runs")?;
    let since: Option<i64> = read
        .conn
        .query_row("SELECT min(event_ms) FROM events", [], |r| r.get(0))
        .map_err(sql_error)?;
    let until: Option<i64> = read
        .conn
        .query_row("SELECT max(event_ms) FROM events", [], |r| r.get(0))
        .map_err(sql_error)?;
    let distinct = if full.is_some() {
        Some(
            read.conn
                .query_row("SELECT count(DISTINCT scan_id) FROM events", [], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(sql_error)?,
        )
    } else {
        None
    };
    let unfinished = unfinished_in(&read.conn)?;
    let database_bytes = std::fs::metadata(read.dir.path.join(super::paths::DATABASE))
        .map_err(|_| failure("cannot inspect archive size"))?
        .len();
    let mut document = json!({"kind":if full.is_some(){"runtime_history_stats"}else{"runtime_history_status"},"archive_present":true,"schema_version":database::VERSION,"cloud_id":cloud.cloud_id,"environment_id":cloud.environment_id,"tsg_id":cloud.tsg,"events":events,"distinct_scan_ids":distinct,"distinct_scan_ids_note":if full.is_none(){Some("full distinct count requires explicit history stats maintenance")}else{None},"partial_receipts":partial,"facts":facts,"profile_observations":profiles,"application_observations":applications,"incomplete_runs":incomplete_runs,"integrity_verified":full.is_some(),"database_bytes":database_bytes,"oldest_event_ms":since,"latest_event_ms":until,"coverage_note":"observed data does not prove complete provider history","unfinished_imports":unfinished});
    if full.is_some() {
        let mut statement=read.conn.prepare("SELECT source,state,count(*) FROM coverage_segments GROUP BY source,state ORDER BY source,state LIMIT 1001").map_err(sql_error)?;
        let mut cursor = statement.query([]).map_err(sql_error)?;
        let mut sources = Vec::new();
        let mut bytes = 0usize;
        while let Some(row) = cursor.next().map_err(sql_error)? {
            let item = json!({"source":row.get::<_,String>(0).map_err(sql_error)?,"state":row.get::<_,String>(1).map_err(sql_error)?,"segments":row.get::<_,i64>(2).map_err(sql_error)?});
            bytes = bytes
                .checked_add(item.to_string().len())
                .ok_or_else(|| failure("statistics exceed the supported bound"))?;
            if sources.len() == 1000 || bytes > 4 * 1024 * 1024 {
                return Err(failure(
                    "statistics source groups exceed the supported bound",
                ));
            }
            sources.push(item);
        }
        drop(cursor);
        drop(statement);
        document["coverage_groups"] = json!(sources);
    }
    read.finish()?;
    Ok(document)
}

pub fn coverage(
    root: &Path,
    cloud: &Cloud,
    since: i64,
    until: i64,
    apps: &[String],
    session: &scm::Session,
) -> Result<Value, CliError> {
    let read = Reader::open(root, cloud, session)?.ok_or_else(|| {
        failure("archive is missing; run explicit sync or import before coverage inspection")
    })?;
    // Bound records/bytes independently of the requested interval. A display cap
    // never changes recorded segment states into a claim of complete coverage.
    let app_json =
        serde_json::to_string(apps).map_err(|_| failure("invalid application filter"))?;
    let predicate = "until_ms>?1 AND since_ms<?2 AND (json_array_length(?3)=0 OR app_id IN (SELECT value FROM json_each(?3)) OR (json_extract(fields_json,'$.scope_identity_supported')=1 AND (json_extract(fields_json,'$.scope_kind')='cloud' OR EXISTS (SELECT 1 FROM json_each(fields_json,'$.selected_app_ids') selection WHERE selection.value IN (SELECT value FROM json_each(?3))))))";
    let total: i64 = read
        .conn
        .query_row(
            &format!("SELECT count(*) FROM coverage_segments WHERE {predicate}"),
            rusqlite::params![since, until, app_json],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let mut rows=read.conn.prepare(&format!("SELECT segment_id,app_id,app_name,source,since_ms,until_ms,observed_ms,state,fields_json FROM coverage_segments WHERE {predicate} ORDER BY source,app_id,since_ms,segment_id LIMIT 1000")).map_err(sql_error)?;
    let mut cursor = rows
        .query(rusqlite::params![since, until, app_json])
        .map_err(sql_error)?;
    let mut items = Vec::new();
    let mut bytes = 0usize;
    while let Some(r) = cursor.next().map_err(sql_error)? {
        let context: String = r.get(8).map_err(sql_error)?;
        let context: serde_json::Value = serde_json::from_str(&context)
            .map_err(|_| failure("invalid retained coverage metadata"))?;
        let scope = context
            .get("scope_kind")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_selection");
        let supported = context
            .get("scope_identity_supported")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let selected: Vec<String> = serde_json::from_value(
            context
                .get("selected_app_ids")
                .cloned()
                .unwrap_or(json!([])),
        )
        .map_err(|_| failure("invalid retained coverage selection"))?;
        if !["tenant", "application", "selection", "unknown_selection"].contains(&scope)
            || selected.len() > 1000
            || selected
                .iter()
                .any(|s| !airs_runtime_core::tenant::scanners::valid_application(s))
            || (supported && scope == "unknown_selection")
        {
            return Err(failure("invalid retained coverage scope"));
        }
        let item = json!({"segment_id":r.get::<_,String>(0).map_err(sql_error)?,"app_id":r.get::<_,Option<String>>(1).map_err(sql_error)?,"app_name":r.get::<_,Option<String>>(2).map_err(sql_error)?,"source":r.get::<_,String>(3).map_err(sql_error)?,"since_ms":r.get::<_,i64>(4).map_err(sql_error)?,"until_ms":r.get::<_,i64>(5).map_err(sql_error)?,"observed_ms":r.get::<_,i64>(6).map_err(sql_error)?,"state":r.get::<_,String>(7).map_err(sql_error)?,"scope_kind":scope,"scope_identity_supported":supported,"selected_app_ids":selected});
        bytes = bytes
            .checked_add(item.to_string().len())
            .ok_or_else(|| failure("coverage output exceeds the supported bound"))?;
        if bytes > 4 * 1024 * 1024 {
            return Err(failure("coverage output exceeds 4 MiB; narrow the scope"));
        }
        items.push(item);
    }
    drop(cursor);
    drop(rows);
    let document = json!({"kind":"runtime_history_coverage","cloud_id":cloud.cloud_id,"since_ms":since,"until_ms":until,"app_ids":apps,"segments":items,"matching_segments":total,"display_truncated":total>1000,"coverage_note":"traversed-as-of observations retain boundary and late-arrival uncertainty; absent segments are unknown"});
    read.finish()?;
    Ok(document)
}
/// One `history_coverage` group (contract §4.3 `$defs/coverage_source`): a provider
/// source category for one application, or tenant-wide (`application_id` = `None`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageGroup {
    pub source: &'static str,
    pub application_id: Option<String>,
    /// `complete`, `partial`, `falling_behind` or `unavailable`. A group with no
    /// segment is absent, which means unknown.
    pub state: &'static str,
    /// The earliest traversal instant among the segments that cover part of the range.
    pub traversed_as_of_ms: Option<i64>,
    /// Milliseconds of the requested range that no covering segment spans.
    pub gap_ms: i64,
}

/// The most archive segments one coverage request may aggregate.
pub const MAX_COVERAGE_SEGMENTS: usize = 100_000;
const MAX_COVERAGE_GROUPS: usize = 10_000;
const CATEGORIES: [&str; 5] = ["inventory", "profiles", "reports", "results", "sessions"];

/// The service's source category for a recorded evidence source name (contract §4.3
/// coverage mapping). Local evidence-handling sources (`event-window`,
/// `evidence-storage`, `archive-range`) and unrecognized legacy names describe no
/// provider traversal and are not reported. Neither is `application-telemetry`: it is
/// an aggregate token-consumption read the dashboard serves only over 7, 30 or 60 days,
/// so for any other window the collector records it unavailable ("no exact
/// representation"); counted under `inventory`, it made the category unavailable on
/// every 24-hour tick although the inventory reads themselves succeeded (M8 live
/// evidence, 2026-11-07). The Signal Brief still reports it as a source.
pub fn source_category(name: &str) -> Option<&'static str> {
    match name {
        "sessions" | "session-details" => Some("sessions"),
        "scan-results" => Some("results"),
        "scan-reports" => Some("reports"),
        "applications" | "registered-apps" | "transaction-metadata" => Some("inventory"),
        "profiles" => Some("profiles"),
        _ => None,
    }
}

/// Severity order: a group reports its worst segment state.
fn severity(state: &str) -> u8 {
    match state {
        "complete" => 0,
        "partial" => 1,
        "falling_behind" => 2,
        _ => 3,
    }
}

/// Aggregate the recorded coverage segments overlapping `[since, until)` (with the same
/// application filter as [`coverage`]) into one group per source category and scope,
/// conservatively (contract §4.3 coverage note):
///
/// - **Scope is kept.** A segment counts for exactly the scope it recorded: a
///   tenant-wide traversal (`scope_kind` `cloud`, supported) for the tenant-wide group
///   (`application_id` `None`); an application or selection traversal for each named
///   application only; an unsupported or unknown scope is never complete and covers
///   nothing anywhere.
/// - **Sources are not merged.** Completeness is decided per underlying source within
///   a scope (`traversed_as_of` complete; legacy `observed` and `partial` partial;
///   `unavailable`/`falling_behind` cover nothing; a gap makes it at least partial).
///   A category is complete only if every underlying source that recorded anything for
///   that scope is complete and gapless; its gap is the largest underlying gap. Disjoint
///   halves from two sources never add up to completeness.
/// - An `offline-gap` marker applies to every category of the scope it records.
///
/// `None` when the archive is missing; never creates or migrates anything.
pub fn coverage_groups(
    root: &Path,
    cloud: &Cloud,
    since: i64,
    until: i64,
    apps: &[String],
    session: &scm::Session,
) -> Result<Option<Vec<CoverageGroup>>, CliError> {
    let Some(read) = Reader::open(root, cloud, session)? else {
        return Ok(None);
    };
    coverage_groups_in(read, since, until, apps).map(Some)
}

/// Coverage as the service presents it: absent, refused while an import is unfinished,
/// or the groups, with the unfinished check and the coverage read in one read
/// transaction of one reader, so no import can commit between them (M5 review 4
/// finding 2).
pub enum PublishedCoverage {
    Absent,
    Unfinished,
    Groups(Vec<CoverageGroup>),
}

pub fn coverage_if_published(
    root: &Path,
    cloud: &Cloud,
    since: i64,
    until: i64,
    apps: &[String],
    session: &scm::Session,
) -> Result<PublishedCoverage, CliError> {
    let Some(read) = Reader::open(root, cloud, session)? else {
        return Ok(PublishedCoverage::Absent);
    };
    // The reader's own read transaction (`BEGIN DEFERRED`, committed by `finish`) spans
    // the check and the whole coverage read.
    if unfinished_in(&read.conn)? > 0 {
        return Ok(PublishedCoverage::Unfinished);
    }
    coverage_groups_in(read, since, until, apps).map(PublishedCoverage::Groups)
}

fn coverage_groups_in(
    read: Reader,
    since: i64,
    until: i64,
    apps: &[String],
) -> Result<Vec<CoverageGroup>, CliError> {
    let out = coverage_groups_conn(&read.conn, since, until, apps, None)?;
    read.finish()?;
    Ok(out)
}

/// Coverage groups through an open connection, optionally only from segments observed
/// by `cutoff_ms` (a report version's evidence cutoff, read in the report's own
/// transaction: M6 review 1 finding 1).
pub(crate) fn coverage_groups_conn(
    conn: &rusqlite::Connection,
    since: i64,
    until: i64,
    apps: &[String],
    cutoff_ms: Option<i64>,
) -> Result<Vec<CoverageGroup>, CliError> {
    let app_json =
        serde_json::to_string(apps).map_err(|_| failure("invalid application filter"))?;
    let predicate = "until_ms>?1 AND since_ms<?2 AND (json_array_length(?3)=0 OR app_id IN (SELECT value FROM json_each(?3)) OR (json_extract(fields_json,'$.scope_identity_supported')=1 AND (json_extract(fields_json,'$.scope_kind')='cloud' OR EXISTS (SELECT 1 FROM json_each(fields_json,'$.selected_app_ids') selection WHERE selection.value IN (SELECT value FROM json_each(?3))))))";
    let mut rows = conn
        .prepare(&format!("SELECT source,app_id,since_ms,until_ms,observed_ms,state,fields_json FROM coverage_segments WHERE {predicate} AND (?4 IS NULL OR observed_ms<=?4) ORDER BY source,app_id,since_ms,segment_id LIMIT {}", MAX_COVERAGE_SEGMENTS + 1))
        .map_err(sql_error)?;
    let mut cursor = rows
        .query(rusqlite::params![since, until, app_json, cutoff_ms])
        .map_err(sql_error)?;
    // (category, scope app) → underlying source → (worst severity, covering intervals,
    // earliest covering traversal)
    type Source = (u8, Vec<(i64, i64)>, Option<i64>);
    type Groups = std::collections::BTreeMap<
        (&'static str, Option<String>),
        std::collections::BTreeMap<String, Source>,
    >;
    let mut groups: Groups = std::collections::BTreeMap::new();
    let mut seen = 0usize;
    while let Some(r) = cursor.next().map_err(sql_error)? {
        seen += 1;
        if seen > MAX_COVERAGE_SEGMENTS {
            return Err(failure(
                "coverage spans too many segments; narrow the range",
            ));
        }
        let name: String = r.get(0).map_err(sql_error)?;
        let app: Option<String> = r.get(1).map_err(sql_error)?;
        let interval = (
            r.get::<_, i64>(2).map_err(sql_error)?.max(since),
            r.get::<_, i64>(3).map_err(sql_error)?.min(until),
        );
        let observed: i64 = r.get(4).map_err(sql_error)?;
        let recorded = r.get::<_, String>(5).map_err(sql_error)?;
        let fields: Value = serde_json::from_str(&r.get::<_, String>(6).map_err(sql_error)?)
            .map_err(|_| failure("invalid retained coverage metadata"))?;
        let supported = fields["scope_identity_supported"].as_bool() == Some(true);
        let selected: Vec<String> = fields["selected_app_ids"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        // The scopes this segment speaks for, and whether it may cover anything there.
        let (scopes, may_cover): (Vec<Option<String>>, bool) =
            match (fields["scope_kind"].as_str(), supported) {
                (Some("cloud"), true) => (vec![None], true),
                (Some("application" | "selection"), true) => {
                    let mut named = selected.clone();
                    if let Some(a) = &app {
                        named.push(a.clone());
                    }
                    named.sort();
                    named.dedup();
                    (named.into_iter().map(Some).collect(), true)
                }
                // Unknown or unsupported scope: recorded, never complete, covers nothing.
                _ => (vec![app.clone()], false),
            };
        let state = match recorded.as_str() {
            "traversed_as_of" if may_cover => "complete",
            "traversed_as_of" | "observed" | "partial" => "partial",
            "falling_behind" => "falling_behind",
            _ => "unavailable",
        };
        let covers = may_cover
            && matches!(
                recorded.as_str(),
                "traversed_as_of" | "observed" | "partial"
            );
        let categories: Vec<&'static str> = if name == "offline-gap" {
            CATEGORIES.to_vec()
        } else {
            source_category(&name).into_iter().collect()
        };
        if apps.is_empty()
            || scopes.iter().any(|s| s.is_none())
            || scopes.iter().flatten().any(|a| apps.contains(a))
        {
            for scope in scopes
                .iter()
                .filter(|s| apps.is_empty() || s.as_ref().is_none_or(|a| apps.contains(a)))
            {
                for category in &categories {
                    let source = groups
                        .entry((category, scope.clone()))
                        .or_default()
                        .entry(name.clone())
                        .or_insert((0, Vec::new(), None));
                    source.0 = source.0.max(severity(state));
                    if covers && interval.0 < interval.1 {
                        source.1.push(interval);
                        source.2 = Some(source.2.map_or(observed, |t: i64| t.min(observed)));
                    }
                }
            }
        }
        if groups.len() > MAX_COVERAGE_GROUPS {
            return Err(failure("coverage spans too many groups; narrow the scope"));
        }
    }
    drop(cursor);
    drop(rows);
    let out = groups
        .into_iter()
        .map(|((source, application_id), sources)| {
            let mut worst = 0u8;
            let mut gap_ms: Option<i64> = None;
            let mut traversed: Option<i64> = None;
            for (name, (severity_seen, mut intervals, earliest)) in sources {
                if name == "offline-gap" {
                    // The marker sets the state; it is not a source with a gap of its own.
                    worst = worst.max(severity_seen);
                    continue;
                }
                intervals.sort_unstable();
                let (mut covered, mut end) = (0i64, since);
                for (a, b) in intervals {
                    let a = a.max(end);
                    if b > a {
                        covered += b - a;
                        end = b;
                    }
                }
                let gap = (until - since) - covered;
                let s = if gap > 0 {
                    severity_seen.max(1)
                } else {
                    severity_seen
                };
                worst = worst.max(s);
                gap_ms = Some(gap_ms.map_or(gap, |g: i64| g.max(gap)));
                traversed = match (traversed, earliest) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
            }
            CoverageGroup {
                source,
                application_id,
                state: ["complete", "partial", "falling_behind", "unavailable"][usize::from(worst)],
                traversed_as_of_ms: traversed,
                // Only a marker recorded: nothing covers any of the range.
                gap_ms: gap_ms.unwrap_or(until - since),
            }
        })
        .collect();
    Ok(out)
}

/// The traversals the rolling selector governs: the session inventory listing, the
/// only time-windowed traversal. Snapshot sources (inventory, profiles) and per-item work
/// (session details, scan results, revisits) are not windowed: their unfinished items
/// are retained work, revisited explicitly, and never move the selector.
pub const WINDOWED_SOURCES: [&str; 1] = ["sessions"];

/// The lower bound of an ordinary polling tick (contract §6 "Tick lower bound"): the
/// oldest last valid traversal among the polled sources (Reports excluded: polling never
/// reads them) minus the 15-minute overlap, kept within [now − 24 h, now − 1 h] (the
/// narrowest supported selector is one hour) and never before the archive's bootstrap.
/// `None` (a source never traversed, or no archive) means the first collection's 24 h.
pub fn polling_lower_bound(
    root: &Path,
    cloud: &Cloud,
    apps: &[String],
    now_ms: i64,
    session: &scm::Session,
) -> Result<Option<i64>, CliError> {
    const OVERLAP_MS: i64 = 15 * 60_000;
    let bootstrap: Option<i64> = match Reader::open(root, cloud, session)? {
        None => return Ok(None),
        Some(read) => read
            .conn
            .query_row(
                "SELECT bootstrap_ms FROM archive_meta WHERE singleton=1",
                [],
                |r| r.get(0),
            )
            .map_err(sql_error)?,
    };
    let Some(bootstrap) = bootstrap else {
        return Ok(None);
    };
    // Progress is per underlying source and per requested scope. A complete traversal is
    // a supported-scope `traversed_as_of` segment; partial, observed, unavailable or
    // falling-behind segments never advance progress. Scope is preserved:
    // - a tenant-wide request is advanced only by tenant-wide (`cloud`) traversals;
    // - a request for applications is advanced, per requested application, by a
    //   tenant-wide traversal or by an application/selection traversal naming that
    //   application; every requested application needs its own progress, and the tick
    //   starts at the oldest of those.
    // A selection never stands for another application or for the whole tenant.
    let Some(read) = Reader::open(root, cloud, session)? else {
        return Ok(None);
    };
    let mut stmt = read
        .conn
        .prepare(
            // A traversal's progress point is the end of the range it actually covered,
            // capped by when it was observed: a historical or delayed traversal of
            // [now-20h, now-12h] observed now has progress now-12h, never now.
            "SELECT source, app_id, min(until_ms, observed_ms), fields_json FROM coverage_segments
             WHERE until_ms>?1 AND since_ms<?2 AND state='traversed_as_of'
               AND json_extract(fields_json,'$.scope_identity_supported')=1",
        )
        .map_err(sql_error)?;
    let rows = stmt
        .query_map(rusqlite::params![now_ms - 86_400_000, now_ms], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(sql_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sql_error)?;
    // `None` covers every application (tenant-wide); `Some(set)` names its applications.
    let complete: Vec<(String, Option<Vec<String>>, i64)> = rows
        .into_iter()
        .filter(|(source, ..)| WINDOWED_SOURCES.contains(&source.as_str()))
        .filter_map(|(source, app, observed, fields)| {
            let fields: Value = serde_json::from_str(&fields).ok()?;
            let named: Vec<String> = fields["selected_app_ids"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let covers = match fields["scope_kind"].as_str()? {
                "cloud" if app.is_none() && named.is_empty() => None,
                "application" | "selection" => {
                    let mut set = named;
                    set.extend(app);
                    Some(set)
                }
                _ => return None,
            };
            Some((source, covers, observed))
        })
        .collect();
    let latest = |source: &str, app: Option<&str>| -> Option<i64> {
        complete
            .iter()
            .filter(|(s, covers, _)| {
                s == source
                    && match (covers, app) {
                        (None, _) => true,
                        (Some(set), Some(a)) => set.iter().any(|x| x == a),
                        (Some(_), None) => false,
                    }
            })
            .map(|(.., observed)| *observed)
            .max()
    };
    let mut polled: Vec<Option<i64>> = Vec::new();
    for source in WINDOWED_SOURCES {
        if apps.is_empty() {
            polled.push(latest(source, None));
        } else {
            for a in apps {
                polled.push(latest(source, Some(a)));
            }
        }
    }
    if polled.is_empty() || polled.iter().any(Option::is_none) {
        return Ok(None);
    }
    let oldest = polled.into_iter().flatten().min().unwrap_or(now_ms);
    // The upper clamp sits a minute inside the hour: the collector derives its selector
    // from its own (slightly later) clock, and one hour must still cover the bound.
    let since = (oldest - OVERLAP_MS).clamp(now_ms - 86_400_000, now_ms - 3_540_000);
    Ok(Some(since.max(bootstrap)))
}

/// The archive's mutation generation: SQLite's file change counter, advanced by every
/// committed archive write (contract §3: a lifecycle preview is bound to it, and the
/// binding is checked again inside the executing write transaction). 0 when the archive
/// does not exist.
pub fn data_generation(
    root: &Path,
    cloud: &Cloud,
    session: &scm::Session,
) -> Result<i64, CliError> {
    let Some(read) = Reader::open(root, cloud, session)? else {
        return Ok(0);
    };
    read.generation()
}

/// Imports the archive's own journal records as unfinished (batches committed, the
/// final publication not yet): such an archive holds a partially imported transfer
/// (M5 review 3 finding 5). A version-1 archive has no imports.
fn unfinished_in(conn: &rusqlite::Connection) -> Result<u64, CliError> {
    if database::archive_schema_version(conn)? < 2 {
        return Ok(0);
    }
    conn.query_row(
        "SELECT count(*) FROM import_operations WHERE rebuilt=0",
        [],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n.max(0) as u64)
    .map_err(sql_error)
}

/// How many imports into the archive are unfinished (0 when there is no archive).
pub fn unfinished_imports(
    root: &Path,
    cloud: &Cloud,
    session: &scm::Session,
) -> Result<u64, CliError> {
    match Reader::open(root, cloud, session)? {
        Some(read) => unfinished_in(&read.conn),
        None => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Writer;
    use crate::test_support::TestDir;
    use rusqlite::params;

    const H: i64 = 3_600_000;
    /// `(source, app, since, until, observed, state)`.
    type Segment<'a> = (&'a str, Option<&'a str>, i64, i64, i64, &'a str);

    /// An archive holding exactly these `(source, app, since, until, observed, state,
    /// scope)` segments; `scope` is the recorded `fields_json` scope.
    fn archive(segments: &[(Segment<'_>, &str)]) -> (TestDir, Cloud) {
        let d = TestDir::new("coverage-groups");
        let c = Cloud::from_parts(
            "123",
            [
                "https://api.example.test/aisec".into(),
                "https://api.example.test/aisec".into(),
                "https://auth.example.test/oauth2/access_token".into(),
            ],
        )
        .unwrap();
        let w = Writer::open(
            d.path(),
            &c,
            Duration::from_secs(10),
            true,
            &scm::Session::new(),
        )
        .unwrap();
        w.conn
            .execute(
                "INSERT INTO collection_runs VALUES ('run-1','sync',0,1,0,1,'complete','{}')",
                [],
            )
            .unwrap();
        for (i, ((source, app, since, until, observed, state), scope)) in
            segments.iter().enumerate()
        {
            w.conn
                .execute(
                    "INSERT INTO coverage_segments VALUES (?1,'run-1',?2,NULL,?3,?4,?5,?6,?7,?8)",
                    params![
                        format!("seg-{i}"),
                        app,
                        source,
                        since,
                        until,
                        observed,
                        state,
                        scope
                    ],
                )
                .unwrap();
        }
        drop(w);
        (d, c)
    }

    const TENANT_WIDE: &str =
        r#"{"scope_kind":"cloud","scope_identity_supported":true,"selected_app_ids":[]}"#;
    const SELECTION_AB: &str = r#"{"scope_kind":"selection","scope_identity_supported":true,"selected_app_ids":["app-a","app-b"]}"#;
    const UNKNOWN: &str = r#"{"scope_kind":"unknown_selection","scope_identity_supported":false,"selected_app_ids":[]}"#;

    /// M4 review 2 finding 1: polling progress is the latest complete traversal per
    /// underlying source; twelve successful quarter-hour traversals give a one-hour
    /// selector (not one reaching back to the first of them), and a partial-only
    /// traversal never advances progress.
    #[test]
    fn polling_progress_is_the_latest_complete_traversal_and_partials_never_advance() {
        let now = 100 * H;
        let q = H / 4;
        let segments: Vec<(Segment<'_>, &str)> = (0..12)
            .map(|k| {
                let at = now - (12 - k) * q;
                (
                    ("sessions", None, at - q, at, at, "traversed_as_of"),
                    TENANT_WIDE,
                )
            })
            .collect();
        let (d, c) = archive(&segments);
        bootstrap(&d, &c, 0);
        let since = polling_lower_bound(d.path(), &c, &[], now, &scm::Session::new())
            .unwrap()
            .expect("progress");
        assert!(
            now - since <= H,
            "one-hour selector after quarter-hour traversals: {}",
            now - since
        );
        let (d, c) = archive(&[(
            ("sessions", None, now - 2 * H, now - H, now - H, "partial"),
            TENANT_WIDE,
        )]);
        bootstrap(&d, &c, 0);
        assert_eq!(
            polling_lower_bound(d.path(), &c, &[], now, &scm::Session::new()).unwrap(),
            None,
            "a partial traversal is no progress"
        );
    }

    fn bootstrap(d: &TestDir, c: &Cloud, at: i64) {
        let w = Writer::open(
            d.path(),
            c,
            Duration::from_secs(10),
            true,
            &scm::Session::new(),
        )
        .unwrap();
        w.conn
            .execute("UPDATE archive_meta SET bootstrap_ms=?1", [at])
            .unwrap();
    }

    fn groups(d: &TestDir, c: &Cloud, since: i64, until: i64) -> Vec<CoverageGroup> {
        coverage_groups(d.path(), c, since, until, &[], &scm::Session::new())
            .unwrap()
            .unwrap()
    }

    fn find(g: &[CoverageGroup], source: &str, app: Option<&str>) -> CoverageGroup {
        g.iter()
            .find(|x| x.source == source && x.application_id.as_deref() == app)
            .cloned()
            .unwrap_or_else(|| panic!("{source} {app:?}: {g:?}"))
    }

    #[test]
    fn coverage_is_decided_per_source_and_never_merged_into_completeness() {
        let _isolation = crate::test_isolation();
        let (d, c) = archive(&[
            // One source, complete over the whole range.
            (
                ("sessions", None, 0, 4 * H, 10, "traversed_as_of"),
                TENANT_WIDE,
            ),
            // Two inventory sources each covering half: neither is complete, so the
            // category is partial with a 2 h gap (review M2-1 finding 1).
            (
                ("applications", None, 0, 2 * H, 20, "traversed_as_of"),
                TENANT_WIDE,
            ),
            (
                ("registered-apps", None, 2 * H, 4 * H, 30, "traversed_as_of"),
                TENANT_WIDE,
            ),
            // A legacy observation is never complete.
            (
                ("scan-reports", None, 0, 4 * H, 40, "observed"),
                TENANT_WIDE,
            ),
            // An unavailable source dominates.
            (("profiles", None, 0, 4 * H, 50, "unavailable"), TENANT_WIDE),
            // Local evidence handling is not a provider source.
            (
                ("evidence-storage", None, 0, 4 * H, 70, "traversed_as_of"),
                TENANT_WIDE,
            ),
        ]);
        let g = groups(&d, &c, 0, 4 * H);
        assert_eq!(
            find(&g, "sessions", None),
            CoverageGroup {
                source: "sessions",
                application_id: None,
                state: "complete",
                traversed_as_of_ms: Some(10),
                gap_ms: 0
            }
        );
        let inventory = find(&g, "inventory", None);
        assert_eq!((inventory.state, inventory.gap_ms), ("partial", 2 * H));
        assert_eq!(find(&g, "reports", None).state, "partial");
        assert_eq!(find(&g, "profiles", None).state, "unavailable");
        assert_eq!(g.len(), 4, "no results group, no local source: {g:?}");
        // Clipping: a narrower range sees only its own overlap.
        let narrow = groups(&d, &c, 0, H);
        assert_eq!(find(&narrow, "inventory", None).state, "complete");
    }

    #[test]
    fn a_scoped_traversal_never_becomes_tenant_wide_and_unknown_scope_is_never_complete() {
        let _isolation = crate::test_isolation();
        let (d, c) = archive(&[
            (
                ("sessions", None, 0, 4 * H, 10, "traversed_as_of"),
                SELECTION_AB,
            ),
            (
                (
                    "scan-results",
                    Some("app-c"),
                    0,
                    4 * H,
                    20,
                    "traversed_as_of",
                ),
                UNKNOWN,
            ),
        ]);
        let g = groups(&d, &c, 0, 4 * H);
        assert_eq!(find(&g, "sessions", Some("app-a")).state, "complete");
        assert_eq!(find(&g, "sessions", Some("app-b")).state, "complete");
        assert!(
            !g.iter()
                .any(|x| x.source == "sessions" && x.application_id.is_none()),
            "a selection traversal says nothing tenant-wide: {g:?}"
        );
        let unknown = find(&g, "results", Some("app-c"));
        assert_eq!((unknown.state, unknown.gap_ms), ("partial", 4 * H));
        // The application filter keeps only the named application's groups.
        let only_a = coverage_groups(
            d.path(),
            &c,
            0,
            4 * H,
            &["app-a".into()],
            &scm::Session::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(only_a.len(), 1, "{only_a:?}");
        assert_eq!(only_a[0].application_id.as_deref(), Some("app-a"));
    }

    #[test]
    fn an_offline_gap_marks_its_scope_falling_behind_without_inventing_a_gap() {
        let _isolation = crate::test_isolation();
        let (d, c) = archive(&[
            (
                ("sessions", None, 0, 4 * H, 10, "traversed_as_of"),
                TENANT_WIDE,
            ),
            (
                ("offline-gap", None, H, 2 * H, 20, "falling_behind"),
                TENANT_WIDE,
            ),
        ]);
        let g = groups(&d, &c, 0, 4 * H);
        assert_eq!(g.len(), CATEGORIES.len(), "{g:?}");
        for group in &g {
            assert_eq!(group.state, "falling_behind", "{group:?}");
            assert_eq!(group.application_id, None);
        }
        assert_eq!(
            find(&g, "sessions", None).gap_ms,
            0,
            "the marker is not a source"
        );
        let reports = find(&g, "reports", None);
        assert_eq!((reports.gap_ms, reports.traversed_as_of_ms), (4 * H, None));
    }

    #[test]
    fn a_missing_archive_is_none_and_nothing_is_created() {
        let d = TestDir::new("coverage-missing");
        let c = Cloud::from_parts(
            "123",
            [
                "https://api.example.test/aisec".into(),
                "https://api.example.test/aisec".into(),
                "https://auth.example.test/oauth2/access_token".into(),
            ],
        )
        .unwrap();
        assert_eq!(
            coverage_groups(d.path(), &c, 0, H, &[], &scm::Session::new()).unwrap(),
            None
        );
        assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 0);
    }

    /// M4 review 3 finding 1: progress keeps its scope. A completed A/B selection never
    /// advances application C or tenant-wide polling; an A/B request needs complete
    /// progress for both A and B; a tenant-wide traversal covers any application.
    #[test]
    fn polling_progress_keeps_its_application_scope() {
        let now = 100 * H;
        let none = |d: &TestDir, c: &Cloud, apps: &[&str], why: &str| {
            let apps: Vec<String> = apps.iter().map(|a| (*a).into()).collect();
            assert_eq!(
                polling_lower_bound(d.path(), c, &apps, now, &scm::Session::new()).unwrap(),
                None,
                "{why}"
            );
        };
        let (d, c) = archive(&[(
            ("sessions", None, now - H, now, now, "traversed_as_of"),
            SELECTION_AB,
        )]);
        bootstrap(&d, &c, 0);
        none(&d, &c, &["app-c"], "an A/B selection never advances C");
        none(&d, &c, &[], "a selection is not tenant-wide progress");
        assert!(
            polling_lower_bound(
                d.path(),
                &c,
                &["app-a".into(), "app-b".into()],
                now,
                &scm::Session::new()
            )
            .unwrap()
            .is_some(),
            "the selection does advance its own applications"
        );
        let app_a = r#"{"scope_kind":"application","scope_identity_supported":true,"selected_app_ids":["app-a"]}"#;
        let (d, c) = archive(&[(
            (
                "sessions",
                Some("app-a"),
                now - H,
                now,
                now,
                "traversed_as_of",
            ),
            app_a,
        )]);
        bootstrap(&d, &c, 0);
        none(&d, &c, &["app-a", "app-b"], "app-b has never traversed");
        let (d, c) = archive(&[(
            ("sessions", None, now - H, now, now, "traversed_as_of"),
            TENANT_WIDE,
        )]);
        bootstrap(&d, &c, 0);
        assert!(
            polling_lower_bound(d.path(), &c, &["app-z".into()], now, &scm::Session::new())
                .unwrap()
                .is_some()
        );
        assert!(
            polling_lower_bound(d.path(), &c, &[], now, &scm::Session::new())
                .unwrap()
                .is_some()
        );
    }

    /// M4 review 4 finding 1 (the reviewer's probe): a complete traversal's progress is
    /// the end of its target, capped by observation time. A historical backfill or a
    /// delayed recovery that traversed [now-20h, now-12h] and was observed now leaves
    /// the next polling tick reading back from now-12h (minus the overlap), never from
    /// now-59min; a later recent traversal then advances progress normally.
    #[test]
    fn historical_and_delayed_traversals_never_skip_unobserved_hours() {
        let now = 100 * H;
        let (d, c) = archive(&[(
            (
                "sessions",
                None,
                now - 20 * H,
                now - 12 * H,
                now,
                "traversed_as_of",
            ),
            TENANT_WIDE,
        )]);
        bootstrap(&d, &c, 0);
        let since = polling_lower_bound(d.path(), &c, &[], now, &scm::Session::new())
            .unwrap()
            .unwrap();
        assert_eq!(
            since,
            now - 12 * H - 900_000,
            "progress stops at the target end"
        );
        // Recent polling afterwards: a traversal reaching now advances progress.
        let (d, c) = archive(&[
            (
                (
                    "sessions",
                    None,
                    now - 20 * H,
                    now - 12 * H,
                    now,
                    "traversed_as_of",
                ),
                TENANT_WIDE,
            ),
            (
                (
                    "sessions",
                    None,
                    now - 12 * H - 900_000,
                    now,
                    now,
                    "traversed_as_of",
                ),
                TENANT_WIDE,
            ),
        ]);
        bootstrap(&d, &c, 0);
        let since = polling_lower_bound(d.path(), &c, &[], now, &scm::Session::new())
            .unwrap()
            .unwrap();
        assert_eq!(
            since,
            now - 3_540_000,
            "a traversal reaching now narrows to one hour"
        );
    }

    /// M5 review 5 minor (coordinated writer): while the coverage reader is open, which
    /// runs the unfinished check and the coverage read in its one read transaction, a
    /// writer cannot commit a journal row. The interleaving that review 4 found is
    /// therefore impossible. Once the reader finishes, the same write commits.
    #[test]
    fn no_writer_commits_between_the_unfinished_check_and_the_coverage_read() {
        let _isolation = crate::test_isolation();
        let d = TestDir::new("coverage-interleave");
        let c = Cloud::from_parts(
            "123",
            [
                "https://api.example.test/aisec".into(),
                "https://api.example.test/aisec".into(),
                "https://auth.example.test/oauth2/access_token".into(),
            ],
        )
        .unwrap();
        {
            let w = Writer::open(
                d.path(),
                &c,
                std::time::Duration::from_secs(5),
                true,
                &scm::Session::new(),
            )
            .unwrap();
            drop(w);
        }
        let file = d.path().join(&c.cloud_id).join("archive.sqlite3");
        let read = Reader::open(d.path(), &c, &scm::Session::new())
            .unwrap()
            .unwrap();
        assert_eq!(unfinished_in(&read.conn).unwrap(), 0);
        // A coordinated writer, between the check and the coverage query.
        let writer = rusqlite::Connection::open(&file).unwrap();
        writer
            .busy_timeout(std::time::Duration::from_millis(200))
            .unwrap();
        let attempt = writer.execute_batch(&format!(
            "BEGIN IMMEDIATE; INSERT INTO import_operations VALUES ('job-race','{}',1,1,1,0,0,1); COMMIT;",
            "b".repeat(64)
        ));
        assert!(
            attempt.is_err(),
            "no import can commit inside the coverage read"
        );
        let _ = writer.execute_batch("ROLLBACK");
        coverage_groups_in(read, 0, i64::MAX, &[]).unwrap();
        writer
            .execute_batch(&format!(
                "BEGIN IMMEDIATE; INSERT INTO import_operations VALUES ('job-race','{}',1,1,1,0,0,1); COMMIT;",
                "b".repeat(64)
            ))
            .unwrap();
        let after = Reader::open(d.path(), &c, &scm::Session::new())
            .unwrap()
            .unwrap();
        assert_eq!(
            unfinished_in(&after.conn).unwrap(),
            1,
            "it commits once the read ends"
        );
    }
}

#[cfg(test)]
mod category_tests {
    use super::source_category;

    /// M8 live evidence (2026-11-07): `application-telemetry` is unavailable by design for
    /// every window but 7, 30 or 60 days; it must not drag the `inventory` category down.
    #[test]
    fn application_telemetry_is_not_inventory_coverage() {
        assert_eq!(source_category("application-telemetry"), None);
        for s in ["applications", "registered-apps", "transaction-metadata"] {
            assert_eq!(source_category(s), Some("inventory"), "{s}");
        }
        assert_eq!(source_category("sessions"), Some("sessions"));
        assert_eq!(source_category("event-window"), None);
    }
}
