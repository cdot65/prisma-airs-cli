//! Full-cohort aggregate queries (plan M6.1, tool `history_query`): counts over every
//! archived event of a half-open UTC range, grouped by up to three of day (UTC date),
//! application, detector, verdict, action and profile. Read-only, one bounded read; no
//! payloads, no network.
//!
//! The cohort counts every event in the range, independent of any display cap. Rows
//! are capped for display (`display_truncated`) but never extrapolated. Grouping by
//! detector counts an event once per detector it was observed with, so detector rows
//! overlap and are not summed into the cohort.

use std::path::Path;

use airs_runtime_core::error::CliError;
use rusqlite::params;
use serde_json::{Map, Value};

use crate::database::{Reader, sql_error};
use crate::identity::Cloud;

pub const GROUPS: [&str; 6] = [
    "day",
    "application",
    "detector",
    "verdict",
    "action",
    "profile",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cohort {
    pub events: u64,
    pub unknown_time: u64,
    pub unattributed_receipts: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub key: Map<String, Value>,
    pub events: u64,
    pub malicious: u64,
    pub unknown: u64,
    pub allowed: u64,
    pub blocked: u64,
}

fn column(group: &str) -> &'static str {
    match group {
        "day" => "date(e.event_ms/1000,'unixepoch')",
        "application" => "e.app_id",
        "detector" => "d.detector",
        "verdict" => "e.verdict",
        "action" => "e.action",
        _ => "e.profile",
    }
}

/// The cohort and its grouped rows (all of them; the caller pages and caps).
pub fn cohort(
    root: &Path,
    cloud: &Cloud,
    (since, until): (i64, i64),
    apps: &[String],
    group_by: &[String],
    session: &scm::Session,
) -> Result<Option<(Cohort, Vec<Row>)>, CliError> {
    if group_by.len() > 3
        || group_by.iter().any(|g| !GROUPS.contains(&g.as_str()))
        || group_by
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != group_by.len()
    {
        return Err(CliError::operational("invalid grouping"));
    }
    let Some(read) = Reader::open(root, cloud, session)? else {
        return Ok(None);
    };
    let app_json = serde_json::to_string(apps)
        .map_err(|_| CliError::operational("invalid application filter"))?;
    let scope = "(json_array_length(?3)=0 OR e.app_id IN (SELECT value FROM json_each(?3)))";
    let events: i64 = read
        .conn
        .query_row(
            &format!(
                "SELECT count(*) FROM events e WHERE e.event_ms>=?1 AND e.event_ms<?2 AND {scope}"
            ),
            params![since, until, app_json],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let unknown_time: i64 = read
        .conn
        .query_row(
            &format!("SELECT count(*) FROM events e WHERE e.event_ms IS NULL AND e.first_seen_ms>=?1 AND e.first_seen_ms<?2 AND {scope}"),
            params![since, until, app_json],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let unattributed: i64 = read
        .conn
        .query_row(
            "SELECT count(*) FROM receipts WHERE state!='promoted' AND last_seen_ms>=?1 AND first_seen_ms<?2",
            params![since, until],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let detector = group_by.iter().any(|g| g == "detector");
    let keys: Vec<String> = group_by.iter().map(|g| column(g).to_owned()).collect();
    let select = if keys.is_empty() {
        "'all'".to_owned()
    } else {
        keys.join(", ")
    };
    let order = if keys.is_empty() {
        "1".to_owned()
    } else {
        (1..=keys.len())
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let join = if detector {
        "JOIN detector_observations d ON d.event_id=e.event_id"
    } else {
        ""
    };
    let sql = format!(
        "SELECT {select}, count(DISTINCT e.event_id), count(DISTINCT CASE WHEN e.verdict='malicious' THEN e.event_id END),
                count(DISTINCT CASE WHEN e.verdict IN ('unknown','error') THEN e.event_id END),
                count(DISTINCT CASE WHEN e.action='allow' THEN e.event_id END),
                count(DISTINCT CASE WHEN e.action='block' THEN e.event_id END)
         FROM events e {join} WHERE e.event_ms>=?1 AND e.event_ms<?2 AND {scope}
         GROUP BY {order} ORDER BY {order}"
    );
    let width = keys.len().max(1);
    let mut stmt = read.conn.prepare(&sql).map_err(sql_error)?;
    let rows = stmt
        .query_map(params![since, until, app_json], |r| {
            let mut key = Map::new();
            for (i, g) in group_by.iter().enumerate() {
                key.insert(
                    g.clone(),
                    r.get::<_, Option<String>>(i)?
                        .map_or(Value::Null, Value::String),
                );
            }
            let n = |i: usize| -> rusqlite::Result<u64> {
                Ok(r.get::<_, i64>(width + i)?.max(0) as u64)
            };
            Ok(Row {
                key,
                events: n(0)?,
                malicious: n(1)?,
                unknown: n(2)?,
                allowed: n(3)?,
                blocked: n(4)?,
            })
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(Some((
        Cohort {
            events: events.max(0) as u64,
            unknown_time: unknown_time.max(0) as u64,
            unattributed_receipts: unattributed.max(0) as u64,
        },
        rows,
    )))
}
