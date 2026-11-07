//! Semantic validation of a staged logical transfer (M5 review 1 finding 1): every
//! derived row must be exactly what the transferred facts derive, before any
//! destination is touched. A transfer that is well framed and checksummed but carries a
//! forged identity, an example its facts do not own, a malformed work descriptor or any
//! forged projection is refused; nothing is silently rebuilt.
use super::{codec::Table, rows::Spec};
use crate::{database::sql_error, failure, repository};
use airs_runtime_core::error::CliError;
use rusqlite::Connection;

fn refused(what: &str) -> CliError {
    failure(&format!(
        "logical transfer carries {what} its facts do not support"
    ))
}

/// Visit every `key` of `table` in key order, a bounded page at a time.
fn for_each_page(
    conn: &Connection,
    table: &str,
    key: &str,
    visit: &mut dyn FnMut(&str) -> Result<(), CliError>,
) -> Result<(), CliError> {
    // The cursor starts before every key (NULL), so the first page includes every
    // possible key, the empty string included (M5 review 3 finding 1).
    let mut last: Option<String> = None;
    let sql = format!(
        "SELECT {key} FROM {table} WHERE ?1 IS NULL OR {key} > ?1 ORDER BY {key} LIMIT 1000"
    );
    loop {
        let page: Vec<String> = conn
            .prepare(&sql)
            .map_err(sql_error)?
            .query_map([&last], |r| r.get(0))
            .map_err(sql_error)?
            .collect::<rusqlite::Result<_>>()
            .map_err(sql_error)?;
        let Some(end) = page.last().cloned() else {
            return Ok(());
        };
        for id in &page {
            visit(id)?;
        }
        last = Some(end);
    }
}

fn exists(conn: &Connection, sql: &str) -> Result<bool, CliError> {
    conn.query_row(sql, [], |r| r.get(0)).map_err(sql_error)
}

/// 1. Event identity: every fact of an event names that event (its ID is the digest of
///    the fact's identity) and the event's identity columns are exactly that identity;
///    an event without a fact of its own is not derivable. The derived identities are
///    kept in a staging table (disk-backed), never in memory.
fn identities(stage: &Connection) -> Result<(), CliError> {
    stage
        .execute_batch(
            "CREATE TABLE semantic_identity (event_id TEXT PRIMARY KEY, app_id TEXT, app_name TEXT,
               session_id TEXT, scan_id TEXT, sub_request INTEGER)",
        )
        .map_err(sql_error)?;
    {
        let mut statement = stage
            .prepare("SELECT event_id, fields_json FROM event_facts ORDER BY fact_id")
            .map_err(sql_error)?;
        let mut insert = stage
            .prepare("INSERT OR IGNORE INTO semantic_identity VALUES (?1,?2,?3,?4,?5,?6)")
            .map_err(sql_error)?;
        let mut same = stage
            .prepare(
                "SELECT app_id IS ?2 AND app_name IS ?3 AND session_id IS ?4 AND scan_id IS ?5 AND sub_request IS ?6
                   FROM semantic_identity WHERE event_id=?1",
            )
            .map_err(sql_error)?;
        let mut cursor = statement.query([]).map_err(sql_error)?;
        while let Some(row) = cursor.next().map_err(sql_error)? {
            let event: String = row.get(0).map_err(sql_error)?;
            let fields: String = row.get(1).map_err(sql_error)?;
            let (id, app_id, app_name, session_id, scan_id, sub_request) =
                repository::fact_identity(&fields)?;
            if id != event {
                return Err(refused("an event identity"));
            }
            let values =
                rusqlite::params![event, app_id, app_name, session_id, scan_id, sub_request];
            insert.execute(values).map_err(sql_error)?;
            if !same
                .query_row(values, |r| r.get::<_, bool>(0))
                .map_err(sql_error)?
            {
                return Err(refused("an event identity"));
            }
        }
    }
    if exists(
        stage,
        "SELECT EXISTS(SELECT 1 FROM events e LEFT JOIN semantic_identity i USING(event_id)
           WHERE i.event_id IS NULL OR e.app_id IS NOT i.app_id OR e.app_name IS NOT i.app_name
              OR e.session_id IS NOT i.session_id OR e.scan_id IS NOT i.scan_id
              OR e.sub_request IS NOT i.sub_request)",
    )? {
        return Err(refused("an event identity"));
    }
    Ok(())
}

/// 2. Examples: every example is exactly a row its event's facts derive (content
///    addressed, so its report, row, detector, direction, family, state and excerpts are
///    owned by a fact of that event); an example of an unowned report refuses. The
///    derivable rows are kept in a staging table and compared in SQL.
fn examples(stage: &Connection) -> Result<(), CliError> {
    stage
        .execute_batch(
            "CREATE TABLE semantic_examples (event_id TEXT, example_id TEXT, report_id TEXT, report_row INTEGER,
               detector TEXT, direction TEXT, family TEXT, state TEXT, excerpts_json TEXT,
               PRIMARY KEY(event_id, example_id, report_id, report_row, detector, direction, family, state, excerpts_json))
             WITHOUT ROWID",
        )
        .map_err(sql_error)?;
    {
        let mut statement = stage
            .prepare("SELECT event_id, fields_json FROM event_facts")
            .map_err(sql_error)?;
        let mut insert = stage
            .prepare("INSERT OR IGNORE INTO semantic_examples VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)")
            .map_err(sql_error)?;
        let mut cursor = statement.query([]).map_err(sql_error)?;
        while let Some(row) = cursor.next().map_err(sql_error)? {
            let event: String = row.get(0).map_err(sql_error)?;
            let fields: String = row.get(1).map_err(sql_error)?;
            for (example, report, report_row, detector, direction, family, state, excerpts) in
                repository::derived_examples(&event, &fields)?
            {
                insert
                    .execute(rusqlite::params![
                        event, example, report, report_row, detector, direction, family, state,
                        excerpts
                    ])
                    .map_err(sql_error)?;
            }
        }
    }
    if exists(
        stage,
        "SELECT EXISTS(SELECT 1 FROM examples x WHERE NOT EXISTS (SELECT 1 FROM semantic_examples d
           WHERE d.event_id=x.event_id AND d.example_id=x.example_id AND d.report_id IS x.report_id
             AND d.report_row IS x.report_row AND d.detector IS x.detector AND d.direction IS x.direction
             AND d.family IS x.family AND d.state IS x.state AND d.excerpts_json IS x.excerpts_json))",
    )? {
        return Err(refused("an example"));
    }
    Ok(())
}

/// 3. Work descriptors decode into their strict types and name their work: a session
///    descriptor is exactly `{application_id, application_name, session_id}` (its work
///    ID the digest of `session/<descriptor>`); a Results or Reports descriptor is a
///    strict scanner work descriptor whose ID is that descriptor's own.
fn work(stage: &Connection) -> Result<(), CliError> {
    use sha2::{Digest, Sha256};
    let mut statement = stage
        .prepare("SELECT work_id, kind, descriptor_json FROM sync_work ORDER BY work_id")
        .map_err(sql_error)?;
    let mut cursor = statement.query([]).map_err(sql_error)?;
    while let Some(row) = cursor.next().map_err(sql_error)? {
        let id: String = row.get(0).map_err(sql_error)?;
        let kind: String = row.get(1).map_err(sql_error)?;
        let descriptor: String = row.get(2).map_err(sql_error)?;
        let ok = match kind.as_str() {
            "session" => {
                #[derive(serde::Deserialize, serde::Serialize)]
                #[serde(deny_unknown_fields)]
                struct Session {
                    application_id: String,
                    application_name: String,
                    session_id: String,
                }
                serde_json::from_str::<Session>(&descriptor).is_ok_and(|s| {
                    [&s.application_id, &s.application_name, &s.session_id]
                        .iter()
                        .all(|v| !v.is_empty())
                        && serde_json::to_string(&s).ok().as_deref() == Some(descriptor.as_str())
                }) && format!("{:x}", Sha256::digest(format!("session/{descriptor}"))) == id
            }
            "results" | "reports" => {
                let source = if kind == "results" {
                    crate::work::Source::Results
                } else {
                    crate::work::Source::Reports
                };
                serde_json::from_str::<crate::work::Descriptor>(&descriptor).is_ok_and(|d| {
                    serde_json::to_string(&d).ok().as_deref() == Some(descriptor.as_str())
                        && d.id(source).ok().as_deref() == Some(id.as_str())
                })
            }
            _ => false,
        };
        if !ok {
            return Err(refused("a work descriptor"));
        }
    }
    Ok(())
}

/// 4. Projections: the events, their reports, the detector observations and the
///    receipts must be exactly what re-running the archive's own projection over the
///    staged facts produces. A copy of each projection table is kept in the staging
///    store, the staged copy is rebuilt (one event or receipt at a time) and the two are
///    compared in SQL (`EXCEPT` both ways); any difference refuses (the transfer is never
///    silently corrected).
fn projections(stage: &Connection, specs: &[Spec]) -> Result<(), CliError> {
    let tables = [
        Table::Events,
        Table::EventReports,
        Table::DetectorObservations,
        Table::Receipts,
    ];
    for t in tables {
        let spec = specs.iter().find(|s| s.table == t).expect("every table");
        stage
            .execute_batch(&format!(
                "CREATE TABLE semantic_before_{0} AS SELECT * FROM {0}",
                spec.table.name()
            ))
            .map_err(sql_error)?;
    }
    // Receipts, then events, in bounded keyset pages (no whole-transfer ID list, and no
    // cursor open while a rebuild writes).
    for_each_page(stage, "receipts", "receipt_id", &mut |id| {
        repository::refresh_receipt_projection(stage, id)
    })?;
    for_each_page(stage, "events", "event_id", &mut |id| {
        repository::rebuild(stage, id)
    })?;
    for t in tables {
        let name = t.name();
        if exists(
            stage,
            &format!(
                "SELECT EXISTS(SELECT * FROM semantic_before_{name} EXCEPT SELECT * FROM {name})
                     OR EXISTS(SELECT * FROM {name} EXCEPT SELECT * FROM semantic_before_{name})"
            ),
        )? {
            return Err(refused("a projection"));
        }
    }
    Ok(())
}

/// 5. Receipt identity: a receipt's ID is the digest of its origin key (never empty),
///    and every receipt is supported by at least one fact of its own; a receipt with no
///    fact (a forged projection) refuses.
fn receipt_identities(stage: &Connection) -> Result<(), CliError> {
    if exists(
        stage,
        "SELECT EXISTS(SELECT 1 FROM receipts r WHERE r.receipt_id='' OR r.origin_key=''
           OR NOT EXISTS (SELECT 1 FROM receipt_facts f WHERE f.receipt_id=r.receipt_id))",
    )? {
        return Err(refused("a receipt"));
    }
    let mut statement = stage
        .prepare("SELECT receipt_id, origin_key FROM receipts ORDER BY receipt_id")
        .map_err(sql_error)?;
    let mut cursor = statement.query([]).map_err(sql_error)?;
    while let Some(row) = cursor.next().map_err(sql_error)? {
        let id: String = row.get(0).map_err(sql_error)?;
        let origin: String = row.get(1).map_err(sql_error)?;
        if repository::receipt_id_of(&origin)? != id {
            return Err(refused("a receipt identity"));
        }
    }
    Ok(())
}

/// Every semantic check of a staged transfer, on the staging copy only.
pub fn validate(stage: &Connection, specs: &[Spec]) -> Result<(), CliError> {
    receipt_identities(stage)?;
    identities(stage)?;
    examples(stage)?;
    work(stage)?;
    projections(stage, specs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M5 review 3 finding 1: keyset paging visits every key, the empty string and keys
    /// across page boundaries included.
    #[test]
    fn keyset_paging_visits_every_key_including_the_empty_one() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE t (k TEXT PRIMARY KEY)")
            .unwrap();
        c.execute("INSERT INTO t VALUES ('')", []).unwrap();
        for i in 0..2_500 {
            c.execute("INSERT INTO t VALUES (?1)", [format!("k{i:05}")])
                .unwrap();
        }
        let mut seen = Vec::new();
        for_each_page(&c, "t", "k", &mut |k| {
            seen.push(k.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(seen.len(), 2_501);
        assert_eq!(seen[0], "", "the empty key is visited");
    }
}
