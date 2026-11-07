//! Versioned logical history transfer; never accepts SQL or SQLite pages.
mod codec;
mod io;
#[cfg(test)]
mod review_tests;
mod rows;
mod safety;
mod semantic;

use crate::{
    database::{self, Reader, sql_error},
    failure,
    identity::Cloud,
};
use airs_runtime_core::error::{CliError, SecretSet};
use std::{
    io::{BufWriter, Write},
    path::Path,
    time::{Duration, Instant},
};

fn active(session: &scm::Session, until: Instant) -> Result<(), CliError> {
    if session.is_cancelled() {
        return Err(CliError::Interrupted(
            airs_runtime_core::error::InterruptContext::NoDataWritten,
        ));
    }
    if Instant::now() >= until {
        return Err(failure(
            "logical transfer deadline exceeded; any committed import batches remain explicitly incomplete",
        ));
    }
    Ok(())
}

/// Consistent full-archive snapshot under C1's explicit maintenance protocol.
/// Only parameter values are exported; SQL and SQLite pages are never output.
pub fn export_file(
    root: &Path,
    cloud: &Cloud,
    path: &Path,
    budget: Duration,
    session: &scm::Session,
    secrets: &SecretSet,
) -> Result<serde_json::Value, CliError> {
    let until = Instant::now() + budget;
    cloud.check_public(secrets)?;
    active(session, until)?;
    io::preflight(path)?;
    let read = Reader::maintenance(root, cloud, budget, session)?
        .ok_or_else(|| failure("archive is missing; export never initializes it"))?;
    let mut output = io::Output::new(path)?;
    let (footer, bytes) = {
        let writer = BufWriter::new(&mut output.file);
        let mut encoder =
            codec::Encoder::new(writer, codec::Header::new(cloud, database::now_ms()?))?;
        for table in codec::Table::ALL {
            let spec = rows::Spec::load(&read.conn, table)?;
            let mut statement = read.conn.prepare(&spec.select()).map_err(sql_error)?;
            let mut cursor = statement.query([]).map_err(sql_error)?;
            while let Some(row) = cursor.next().map_err(sql_error)? {
                active(session, until)?;
                let row = spec.read(row)?;
                safety::validate(&spec, &row, secrets)?;
                encoder.row(row)?;
            }
        }
        let (mut writer, footer, bytes) = encoder.finish()?;
        writer
            .flush()
            .map_err(|_| failure("cannot flush logical transfer; no final artifact published"))?;
        (footer, bytes)
    };
    active(session, until)?;
    read.finish()?;
    output.check()?;
    let durability_uncertain = output.publish()?;
    Ok(
        serde_json::json!({"kind":"runtime_history_export","format_version":1,"cloud_id":cloud.cloud_id,"sha256":footer.sha256,"records":footer.counts,"bytes":bytes,"published":true,"durable":!durability_uncertain,"network_requests":0}),
    )
}

/// Where an interrupted import stopped: the transfer it applies (its footer SHA-256),
/// how many bounded batches committed, their rows and inserts, and the archive
/// generation right after the last committed batch. Persisted by the caller after
/// every batch (a durable resume checkpoint).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImportCheckpoint {
    pub transfer_sha256: String,
    pub batches: u64,
    pub rows: u64,
    pub inserted: u64,
    pub generation: i64,
    /// Whether the final projection rebuild also committed.
    pub rebuilt: bool,
}

/// What an import that did not finish left in the destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportOutcome {
    /// Refused before any destination change (validation, binding or the first batch).
    RefusedBeforeChange,
    /// These batches committed; the rest was refused or interrupted. Resume from the
    /// checkpoint (each committed batch is idempotent).
    PartiallyCommitted(ImportCheckpoint),
    /// A commit's outcome is unknown (the commit itself failed): the batch after the
    /// checkpoint may or may not have committed; a resume re-applies it idempotently.
    Uncertain(ImportCheckpoint),
}

/// An import that did not finish: why, and what it left (boxed: the error path is rare
/// and the checkpoint large).
#[derive(Debug)]
pub struct ImportError {
    pub error: CliError,
    pub outcome: Box<ImportOutcome>,
}

/// How the destination phase of an import runs.
pub struct ImportControl<'a> {
    /// The mutation generation the import's preview saw (`None`: unbound, the driver's
    /// explicit operator import). Checked inside the first batch's write transaction;
    /// each later batch must find exactly the generation the previous batch left.
    pub expected_generation: Option<i64>,
    /// Rows per bounded, separately committed batch.
    pub batch_rows: usize,
    /// The import's identity (the service job ID): its archive-side journal row, which
    /// each batch updates in its own transaction. `None`: a fresh identity.
    pub operation_id: Option<&'a str>,
    /// Resume after this checkpoint (an interrupted import of the same transfer).
    pub resume: Option<ImportCheckpoint>,
    /// Called after every committed batch (and the final rebuild) to persist progress.
    /// An error stops the import after that durable batch.
    pub on_batch: &'a mut dyn FnMut(&ImportCheckpoint) -> Result<(), CliError>,
}

/// The default bounded batch.
pub const IMPORT_BATCH_ROWS: usize = 1000;

/// The explicit import of one logical transfer (contract M5.1/M5.2) into the archive of
/// `cloud`. Input is data only (never SQL, schema or pages). Every content check runs on
/// a private staging copy before the destination is touched:
///
/// 1. framing, per-table counts, the exact-byte SHA-256 footer and no trailing data;
/// 2. the header's version, schema digest and explicit cloud binding;
/// 3. every row: its column cardinality and types, the safety allowlist, and insertion
///    into a staging store of this schema (duplicate primary keys refuse);
/// 4. relations: no orphan (referential integrity of the whole staged set);
/// 5. semantics ([`semantic::validate`]): event identities, example ownership, strict
///    work descriptors and every projection exactly as the facts derive it.
///
/// The destination is then changed in bounded batches, each in its own write
/// transaction under the archive's exclusive writer: the generation binding, held keys
/// (a key the archive holds must carry identical content) and erasure records (no
/// pruned excerpt in any fact copy, no pruned example) are checked inside the batch's
/// own transaction, so no concurrent write can slip between check and apply.
pub fn import_file(
    root: &Path,
    cloud: &Cloud,
    path: &Path,
    budget: Duration,
    session: &scm::Session,
    secrets: &SecretSet,
) -> Result<serde_json::Value, CliError> {
    let mut ignore = |_: &ImportCheckpoint| Ok(());
    import_file_controlled(
        root,
        cloud,
        path,
        budget,
        session,
        secrets,
        ImportControl {
            expected_generation: None,
            batch_rows: IMPORT_BATCH_ROWS,
            resume: None,
            operation_id: None,
            on_batch: &mut ignore,
        },
    )
    .map_err(|e| e.error)
}

/// Every content check of [`import_file`] (framing, header and cloud binding, rows,
/// relations and semantics) without any destination: the record count of a valid
/// transfer, or the refusal. Used when a staged upload is finalized.
pub fn validate_file(
    cloud: &Cloud,
    path: &Path,
    budget: Duration,
    session: &scm::Session,
    secrets: &SecretSet,
) -> Result<u64, CliError> {
    let staged = stage_and_validate(cloud, path, budget, session, secrets)?;
    Ok(staged.rows)
}

struct Staged {
    stage: rusqlite::Connection,
    specs: Vec<rows::Spec>,
    rows: u64,
    sha256: String,
    until: Instant,
    _cleanup: Cleanup,
}

struct Cleanup(std::path::PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn stage_and_validate(
    cloud: &Cloud,
    path: &Path,
    budget: Duration,
    session: &scm::Session,
    secrets: &SecretSet,
) -> Result<Staged, CliError> {
    use rusqlite::{Connection, params_from_iter};
    use std::io::BufReader;
    let until = Instant::now() + budget;
    cloud.check_public(secrets)?;
    active(session, until)?;
    let input = io::Input::open(path)?;
    // Private staging outside the destination, removed whatever happens.
    let staging_dir = std::env::temp_dir().join(format!("airs-import-{}", uuid::Uuid::new_v4()));
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&staging_dir)
            .map_err(|_| failure("cannot create private import staging"))?;
    }
    let cleanup = Cleanup(staging_dir.clone());
    let mut stage = Connection::open(staging_dir.join("stage.sqlite3")).map_err(sql_error)?;
    // The staging store is disk-backed with a bounded page cache and temporary storage
    // on disk (sorts, `EXCEPT`), under the import's own deadline and cancellation, so
    // validating a large transfer neither grows memory with it nor outlives its budget.
    database::deadline(&mut stage, until, session)?;
    stage
        .execute_batch(
            "PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA foreign_keys=OFF;
             PRAGMA cache_size=-8192; PRAGMA temp_store=FILE; PRAGMA mmap_size=0;",
        )
        .map_err(sql_error)?;
    stage.execute_batch(database::SCHEMA).map_err(sql_error)?;
    let specs: Vec<rows::Spec> = codec::Table::ALL
        .into_iter()
        .map(|t| rows::Spec::load(&stage, t))
        .collect::<Result<_, _>>()?;
    let mut decoder = codec::Decoder::new(BufReader::new(&input.file));
    let mut rows_read = 0u64;
    let mut sha256 = String::new();
    stage.execute_batch("BEGIN").map_err(sql_error)?;
    loop {
        active(session, until)?;
        match decoder.next()? {
            None => break,
            Some(codec::Line::Header(header)) => header.validate(cloud, secrets)?,
            Some(codec::Line::Row(row)) => {
                let spec = specs
                    .iter()
                    .find(|s| s.table == row.table)
                    .expect("every table");
                safety::validate(spec, &row, secrets)?;
                let (values, _) = spec.values(&row)?;
                stage
                    .execute(&spec.insert(), params_from_iter(values))
                    .map_err(|_| {
                        failure("logical transfer repeats a record key or violates its record kind")
                    })?;
                rows_read += 1;
            }
            Some(codec::Line::Footer(footer)) => sha256 = footer.sha256,
        }
    }
    input.check()?;
    stage.execute_batch("COMMIT").map_err(sql_error)?;
    // Relations of the whole staged set.
    let orphan: bool = stage
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
            [],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if orphan {
        return Err(failure(
            "logical transfer references records it does not contain",
        ));
    }
    active(session, until)?;
    semantic::validate(&stage, &specs)?;
    active(session, until)?;
    Ok(Staged {
        stage,
        specs,
        rows: rows_read,
        sha256,
        until,
        _cleanup: cleanup,
    })
}

/// [`import_file`] with an explicit generation binding, bounded batch size, resume point
/// and per-batch progress, reporting what an unfinished import left behind.
///
/// - **Known progress first.** The archive-side journal row of `operation_id` (written by
///   every committed batch in that batch's own transaction) is read before anything
///   else: it, not the possibly stale control checkpoint, is the import's progress, so
///   a crash between a batch commit and its checkpoint is reconciled (C4), and a refusal
///   after earlier committed batches is never reported as `refused_before_change`.
/// - **Whole-transfer prevalidation.** Before the first batch, every staged row's held
///   key and erasure records are checked against one destination snapshot (inside a
///   write transaction under the writer gate, rolled back). Its generation then binds
///   the first batch, and every later batch must find exactly the generation the
///   previous one left; a late conflict (beyond any batch) therefore refuses before any
///   destination change. An archive that did not exist must still be empty when the
///   first batch creates it (no concurrent-creation race).
pub fn import_file_controlled(
    root: &Path,
    cloud: &Cloud,
    path: &Path,
    budget: Duration,
    session: &scm::Session,
    secrets: &SecretSet,
    control: ImportControl<'_>,
) -> Result<serde_json::Value, ImportError> {
    let operation = control
        .operation_id
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    // What the archive already committed for this operation.
    let journaled = journal_of(root, cloud, session, &operation).map_err(|error| ImportError {
        error,
        outcome: Box::new(known(control.resume.as_ref())),
    })?;
    let progress = journaled
        .as_ref()
        .map(|(c, _)| c.clone())
        .or(control.resume.clone());
    let before = |error: CliError| ImportError {
        error,
        outcome: Box::new(known(progress.as_ref())),
    };
    let staged = stage_and_validate(cloud, path, budget, session, secrets).map_err(before)?;
    let batch_rows = control.batch_rows.max(1);
    let mut checkpoint = match (&journaled, &control.resume) {
        (Some((journal, started)), _) => {
            if journal.transfer_sha256 != staged.sha256 {
                return Err(before(failure(
                    "the archive's journal for this import names another transfer; refusing",
                )));
            }
            // The generation the archive holds if nothing else wrote after the last
            // journaled commit (each write transaction advances the counter by one).
            ImportCheckpoint {
                generation: started + 1,
                ..journal.clone()
            }
        }
        (None, Some(resume)) if resume.batches > 0 || resume.rebuilt => {
            return Err(before(failure(
                "the archive holds no journal of this import's committed batches; refusing",
            )));
        }
        (None, resume) => {
            if let Some(resume) = resume
                && resume.transfer_sha256 != staged.sha256
            {
                return Err(before(failure(
                    "the resume checkpoint names another transfer; refusing",
                )));
            }
            ImportCheckpoint {
                transfer_sha256: staged.sha256.clone(),
                batches: 0,
                rows: 0,
                inserted: 0,
                generation: 0,
                rebuilt: false,
            }
        }
    };
    let stop = |error: CliError, checkpoint: &ImportCheckpoint, uncertain: bool| ImportError {
        error,
        outcome: Box::new(if uncertain {
            ImportOutcome::Uncertain(checkpoint.clone())
        } else {
            known(Some(checkpoint))
        }),
    };
    // A fresh import: the whole staged set against one destination snapshot.
    let mut creating = false;
    if checkpoint.batches == 0 && !checkpoint.rebuilt {
        match prevalidate(root, cloud, &staged, session, control.expected_generation) {
            Ok(Some(generation)) => checkpoint.generation = generation,
            Ok(None) => creating = true,
            Err(error) => return Err(stop(error, &checkpoint, false)),
        }
    }
    let counts = staged
        .specs
        .iter()
        .map(|spec| {
            staged
                .stage
                .query_row(
                    &format!("SELECT count(*) FROM {}", spec.table.name()),
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .map(|n| n.max(0) as u64)
                .map_err(sql_error)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| stop(e, &checkpoint, false))?;
    let total_batches = staged.rows.div_ceil(batch_rows as u64);
    for batch in checkpoint.batches..total_batches {
        let remaining = staged.until.saturating_duration_since(Instant::now());
        if remaining.is_zero() || session.is_cancelled() {
            return Err(stop(
                failure(
                    "logical transfer deadline exceeded; committed import batches remain explicitly incomplete",
                ),
                &checkpoint,
                false,
            ));
        }
        let applied = apply_batch(
            root,
            cloud,
            &staged,
            &counts,
            batch * batch_rows as u64,
            batch_rows as u64,
            remaining,
            session,
            (creating && checkpoint.batches == 0, checkpoint.generation),
            (&operation, &checkpoint),
        );
        match applied {
            Ok((inserted, rows, generation)) => {
                checkpoint.batches = batch + 1;
                checkpoint.rows += rows;
                checkpoint.inserted += inserted;
                checkpoint.generation = generation;
                (control.on_batch)(&checkpoint).map_err(|e| stop(e, &checkpoint, false))?;
            }
            Err((error, uncertain)) => return Err(stop(error, &checkpoint, uncertain)),
        }
    }
    if !checkpoint.rebuilt {
        match rebuild_all(
            root,
            cloud,
            &staged,
            session,
            checkpoint.generation,
            (&operation, &checkpoint),
        ) {
            Ok(generation) => {
                checkpoint.rebuilt = true;
                checkpoint.generation = generation;
                (control.on_batch)(&checkpoint).map_err(|e| stop(e, &checkpoint, false))?;
            }
            Err((error, uncertain)) => return Err(stop(error, &checkpoint, uncertain)),
        }
    }
    Ok(serde_json::json!({
        "kind": "runtime_history_import",
        "format_version": 1,
        "cloud_id": cloud.cloud_id,
        "records": staged.rows,
        "inserted": checkpoint.inserted,
        "already_present": staged.rows - checkpoint.inserted,
        "batches": checkpoint.batches,
        "transfer_sha256": staged.sha256,
        "outcome": "committed",
        "network_requests": 0,
    }))
}

/// The outcome a stop at known progress `checkpoint` reports.
fn known(checkpoint: Option<&ImportCheckpoint>) -> ImportOutcome {
    match checkpoint {
        Some(c) if c.batches > 0 || c.rebuilt => ImportOutcome::PartiallyCommitted(c.clone()),
        _ => ImportOutcome::RefusedBeforeChange,
    }
}

/// The archive-side journal row of `operation`: its progress and the generation its
/// last committed transaction started from (`None`: no archive, or no such import).
fn journal_of(
    root: &Path,
    cloud: &Cloud,
    session: &scm::Session,
    operation: &str,
) -> Result<Option<(ImportCheckpoint, i64)>, CliError> {
    let Some(read) = Reader::open(root, cloud, session)? else {
        return Ok(None);
    };
    let has_journal: bool = read
        .conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='import_operations')",
            [],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if !has_journal {
        return Ok(None);
    }
    read.conn
        .query_row(
            "SELECT transfer_sha256, batches, rows, inserted, rebuilt, started_generation FROM import_operations WHERE operation_id=?1",
            [operation],
            |r| {
                Ok((
                    ImportCheckpoint {
                        transfer_sha256: r.get(0)?,
                        batches: r.get::<_, i64>(1)?.max(0) as u64,
                        rows: r.get::<_, i64>(2)?.max(0) as u64,
                        inserted: r.get::<_, i64>(3)?.max(0) as u64,
                        generation: 0,
                        rebuilt: r.get::<_, i64>(4)? == 1,
                    },
                    r.get::<_, i64>(5)?,
                ))
            },
        )
        .optional_row()
}

/// Record a committed step of `operation` in the archive, in the caller's write
/// transaction (`started` is that transaction's starting generation).
fn journal_step(
    conn: &rusqlite::Connection,
    operation: &str,
    progress: &ImportCheckpoint,
    started: i64,
) -> Result<(), CliError> {
    conn.execute(
        "INSERT INTO import_operations VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(operation_id) DO UPDATE SET batches=excluded.batches, rows=excluded.rows,
           inserted=excluded.inserted, rebuilt=excluded.rebuilt,
           started_generation=excluded.started_generation, updated_ms=excluded.updated_ms",
        rusqlite::params![
            operation,
            progress.transfer_sha256,
            progress.batches as i64,
            progress.rows as i64,
            progress.inserted as i64,
            i64::from(progress.rebuilt),
            started,
            database::now_ms()?,
        ],
    )
    .map_err(sql_error)?;
    // A published import of a transfer supersedes every earlier unfinished import of the
    // same transfer (the identical contents are now present in full): their journal rows
    // are kept for audit and marked finished, so a canceled or failed import retried as a
    // new job no longer blocks coverage (M5 review 4 finding 1). Unfinished imports of
    // other transfers stay unfinished.
    if progress.rebuilt {
        conn.execute(
            "UPDATE import_operations SET rebuilt=1, updated_ms=?3 WHERE transfer_sha256=?1 AND operation_id<>?2 AND rebuilt=0",
            rusqlite::params![progress.transfer_sha256, operation, database::now_ms()?],
        )
        .map_err(sql_error)?;
    }
    Ok(())
}

/// The whole staged set checked against one destination snapshot: the binding, every
/// held key and every erasure record, inside a write transaction under the writer gate
/// (rolled back: nothing changes). Returns the snapshot's generation, or `None` when the
/// archive does not exist (an empty destination: nothing to conflict with).
fn prevalidate(
    root: &Path,
    cloud: &Cloud,
    staged: &Staged,
    session: &scm::Session,
    expected: Option<i64>,
) -> Result<Option<i64>, CliError> {
    if Reader::open(root, cloud, session)?.is_none() {
        // A bound preview of a missing archive saw generation 0.
        if expected.is_some_and(|g| g != 0) {
            return Err(failure(crate::repository::GENERATION_CHANGED));
        }
        return Ok(None);
    }
    let budget = staged.until.saturating_duration_since(Instant::now());
    // Never upgrades: a refused import leaves an accepted-M4 archive byte for byte
    // unchanged (M5 review 3 finding 3); the first batch's writer upgrades it.
    let w = database::Writer::existing_unupgraded(root, cloud, budget, session)?;
    w.conn.execute_batch("BEGIN IMMEDIATE").map_err(sql_error)?;
    let result = (|| {
        let generation = w.generation()?;
        if let Some(expected) = expected
            && generation != expected
        {
            return Err(failure(crate::repository::GENERATION_CHANGED));
        }
        for spec in &staged.specs {
            let mut statement = staged.stage.prepare(&spec.select()).map_err(sql_error)?;
            let mut cursor = statement.query([]).map_err(sql_error)?;
            while let Some(row) = cursor.next().map_err(sql_error)? {
                w.check()?;
                let incoming = spec.read(row)?;
                check_row(&w.conn, staged, spec, &incoming)?;
            }
        }
        Ok(generation)
    })();
    let _ = w.conn.execute_batch("ROLLBACK");
    result.map(Some)
}

/// One staged row against the destination: a held immutable record must carry
/// identical content, and no erasure record may be undone.
fn check_row(
    destination: &rusqlite::Connection,
    staged: &Staged,
    spec: &rows::Spec,
    incoming: &codec::Row,
) -> Result<(), CliError> {
    use rusqlite::params_from_iter;
    let key_sql = spec
        .columns
        .iter()
        .filter(|c| c.primary > 0)
        .map(|c| format!("{}=?", c.name))
        .collect::<Vec<_>>()
        .join(" AND ");
    let base = spec.select();
    let base = base.split(" ORDER BY ").next().unwrap_or_default();
    let key_values: Vec<rusqlite::types::Value> = spec
        .columns
        .iter()
        .zip(&incoming.values)
        .filter(|(c, _)| c.primary > 0)
        .map(|(c, v)| {
            if c.integer {
                rusqlite::types::Value::Integer(v.as_i64().unwrap_or_default())
            } else {
                rusqlite::types::Value::Text(v.as_str().unwrap_or_default().to_owned())
            }
        })
        .collect();
    let held = destination
        .query_row(
            &format!("{base} WHERE {key_sql}"),
            params_from_iter(key_values),
            |r| Ok(spec.read(r)),
        )
        .optional_row()?;
    // Immutable records (facts, runs, observations, examples, journal rows) a destination
    // already holds must be identical. Projection rows (events, their reports, detector
    // observations, receipts) are derived, not immutable: the destination's row is kept
    // and the final rebuild recomputes it from every fact the archive then holds.
    let projection = matches!(
        spec.table,
        codec::Table::Events
            | codec::Table::EventReports
            | codec::Table::DetectorObservations
            | codec::Table::Receipts
    );
    // An immutable record's content is its identity; when, and in which run, the
    // destination first observed it is the destination's own metadata and is kept:
    // `observed_ms`, and the first observing `run_id` of an observation (every observing
    // run is in its link table). An overlapping archive saw the same content at another
    // time, in another run.
    let observation = matches!(
        spec.table,
        codec::Table::ProfileObservations
            | codec::Table::ApplicationObservations
            | codec::Table::AggregateObservations
    );
    let content = |values: &[serde_json::Value]| -> Vec<serde_json::Value> {
        spec.columns
            .iter()
            .zip(values)
            .filter(|(c, _)| c.name != "observed_ms" && !(observation && c.name == "run_id"))
            .map(|(_, v)| v.clone())
            .collect()
    };
    if let Some(held) = held
        && !projection
        && content(&held?.values) != content(&incoming.values)
    {
        return Err(failure(
            "logical transfer would change a record the archive already holds",
        ));
    }
    // A version-1 (accepted-M4) destination predates erasure records: it holds none.
    if database::archive_schema_version(destination)? < 2 {
        return Ok(());
    }
    erasure_check(destination, &staged.stage, spec.table, &incoming.values)
}

/// The writer's generation read in a short read transaction while it still holds the
/// archive's exclusive writer gate (no other writer can commit in between).
fn generation_after_commit(w: &database::Writer) -> Result<i64, CliError> {
    w.conn.execute_batch("BEGIN DEFERRED").map_err(sql_error)?;
    let _: i64 = w
        .conn
        .query_row("SELECT 1 FROM archive_meta", [], |r| r.get(0))
        .map_err(sql_error)?;
    let generation = w.generation();
    w.conn.execute_batch("COMMIT").map_err(sql_error)?;
    generation
}

/// Whether a destination holds nothing an import could conflict with (a just-created
/// archive): every transferred table, its erasure records and its import journal empty.
fn pristine(conn: &rusqlite::Connection, specs: &[rows::Spec]) -> Result<bool, CliError> {
    for name in specs
        .iter()
        .map(|s| s.table.name())
        .chain(["pruned_examples", "import_operations"])
    {
        let any: bool = conn
            .query_row(&format!("SELECT EXISTS(SELECT 1 FROM {name})"), [], |r| {
                r.get(0)
            })
            .map_err(sql_error)?;
        if any {
            return Ok(false);
        }
    }
    Ok(true)
}

/// One bounded batch (global rows `first..first+count` in table and key order), checked
/// and applied inside one write transaction together with its journal row. Returns
/// `(inserted, rows, generation after commit)`, or the error and whether the commit's
/// outcome is unknown.
#[allow(clippy::too_many_arguments)]
fn apply_batch(
    root: &Path,
    cloud: &Cloud,
    staged: &Staged,
    counts: &[u64],
    first: u64,
    count: u64,
    budget: Duration,
    session: &scm::Session,
    (creating, expected): (bool, i64),
    (operation, progress): (&str, &ImportCheckpoint),
) -> Result<(u64, u64, i64), (CliError, bool)> {
    use rusqlite::params_from_iter;
    let refuse = |e: CliError| (e, false);
    let w = database::Writer::open(root, cloud, budget, creating, session).map_err(refuse)?;
    w.conn
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| refuse(sql_error(e)))?;
    let rollback = |e: CliError| {
        let _ = w.conn.execute_batch("ROLLBACK");
        (e, false)
    };
    let started = w.generation().map_err(rollback)?;
    let unchanged = if creating {
        pristine(&w.conn, &staged.specs).map_err(rollback)?
    } else {
        started == expected || w.bound_to(expected).map_err(rollback)?
    };
    if !unchanged {
        return Err(rollback(failure(crate::repository::GENERATION_CHANGED)));
    }
    let (mut inserted, mut rows) = (0u64, 0u64);
    // The events and receipts this batch added facts to: their projections are rebuilt
    // in this same transaction, so every committed batch leaves projections consistent
    // with every fact the archive holds (M5 review 3 finding 5). Bounded by the batch.
    let mut touched_events: std::collections::BTreeSet<String> = Default::default();
    let mut touched_receipts: std::collections::BTreeSet<String> = Default::default();
    let mut skip = first;
    let mut left = count;
    for (spec, total) in staged.specs.iter().zip(counts) {
        if left == 0 {
            break;
        }
        if skip >= *total {
            skip -= total;
            continue;
        }
        let take = left.min(total - skip);
        let insert = spec
            .insert()
            .replacen("INSERT INTO", "INSERT OR IGNORE INTO", 1);
        let mut statement = staged
            .stage
            .prepare(&format!("{} LIMIT ?1 OFFSET ?2", spec.select()))
            .map_err(|e| rollback(sql_error(e)))?;
        let mut cursor = statement
            .query(rusqlite::params![take as i64, skip as i64])
            .map_err(|e| rollback(sql_error(e)))?;
        while let Some(row) = cursor.next().map_err(|e| rollback(sql_error(e)))? {
            w.check().map_err(rollback)?;
            let incoming = spec.read(row).map_err(rollback)?;
            check_row(&w.conn, staged, spec, &incoming).map_err(rollback)?;
            let (values, _) = spec.values(&incoming).map_err(rollback)?;
            let added = w
                .conn
                .execute(&insert, params_from_iter(values))
                .map_err(|e| rollback(sql_error(e)))? as u64;
            if added > 0 {
                let owner = incoming
                    .values
                    .get(1)
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                match (spec.table, owner) {
                    (codec::Table::EventFacts, Some(event)) => {
                        touched_events.insert(event);
                    }
                    (codec::Table::ReceiptFacts, Some(receipt)) => {
                        touched_receipts.insert(receipt);
                    }
                    _ => {}
                }
            }
            inserted += added;
            rows += 1;
        }
        left -= take;
        skip = 0;
    }
    for receipt in &touched_receipts {
        w.check().map_err(rollback)?;
        crate::repository::refresh_receipt_projection(&w.conn, receipt).map_err(rollback)?;
    }
    for event in &touched_events {
        w.check().map_err(rollback)?;
        let exists: bool = w
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM events WHERE event_id=?1)",
                [event],
                |r| r.get(0),
            )
            .map_err(|e| rollback(sql_error(e)))?;
        if exists {
            crate::repository::rebuild(&w.conn, event).map_err(rollback)?;
        }
    }
    let step = ImportCheckpoint {
        batches: progress.batches + 1,
        rows: progress.rows + rows,
        inserted: progress.inserted + inserted,
        ..progress.clone()
    };
    journal_step(&w.conn, operation, &step, started).map_err(rollback)?;
    w.check().map_err(rollback)?;
    if let Err(e) = w.conn.execute_batch("COMMIT") {
        // Whether this commit reached the file is unknown.
        let _ = w.conn.execute_batch("ROLLBACK");
        return Err((sql_error(e), true));
    }
    let generation = generation_after_commit(&w).map_err(|e| (e, true))?;
    Ok((inserted, rows, generation))
}

/// The archive's erasure records apply to imported rows: no pruned example, and no fact
/// copy (event fact, or a receipt fact promoted to an event) carrying a pruned excerpt.
/// Only an explicit, separately authorized import of that data may restore it.
fn erasure_check(
    destination: &rusqlite::Connection,
    stage: &rusqlite::Connection,
    table: codec::Table,
    values: &[serde_json::Value],
) -> Result<(), CliError> {
    let restore = || {
        failure(
            "logical transfer would restore excerpts the archive pruned; import them only by an explicit separate authorization",
        )
    };
    let text = |i: usize| values.get(i).and_then(|v| v.as_str()).unwrap_or_default();
    match table {
        codec::Table::Examples => {
            let gone: bool = destination
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM pruned_examples WHERE example_id=?1)",
                    [text(0)],
                    |r| r.get(0),
                )
                .map_err(sql_error)?;
            if gone {
                return Err(restore());
            }
        }
        codec::Table::EventFacts => {
            // (fact_id, event_id, source, observed_ms, redaction_version, fields_json)
            if crate::repository::fact_carries_pruned(destination, text(1), text(5))? {
                return Err(restore());
            }
        }
        codec::Table::ReceiptFacts => {
            // (fact_id, receipt_id, observed_ms, fields_json): the event it joins.
            let event: Option<String> = stage
                .query_row(
                    "SELECT promoted_event_id FROM receipts WHERE receipt_id=?1",
                    [text(1)],
                    |r| r.get(0),
                )
                .optional_row()?
                .flatten();
            if let Some(event) = event
                && crate::repository::fact_carries_pruned(destination, &event, text(3))?
            {
                return Err(restore());
            }
        }
        _ => {}
    }
    Ok(())
}

/// The final projection rebuild of every staged event and receipt against everything
/// the archive now holds, in one write transaction. Returns the generation after it.
fn rebuild_all(
    root: &Path,
    cloud: &Cloud,
    staged: &Staged,
    session: &scm::Session,
    expected: i64,
    (operation, progress): (&str, &ImportCheckpoint),
) -> Result<i64, (CliError, bool)> {
    let refuse = |e: CliError| (e, false);
    let budget = staged.until.saturating_duration_since(Instant::now());
    let w = database::Writer::open(root, cloud, budget, true, session).map_err(refuse)?;
    w.conn
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| refuse(sql_error(e)))?;
    let rollback = |e: CliError| {
        let _ = w.conn.execute_batch("ROLLBACK");
        (e, false)
    };
    let started = w.generation().map_err(rollback)?;
    if !w.bound_to(expected).map_err(rollback)? {
        return Err(rollback(failure(crate::repository::GENERATION_CHANGED)));
    }
    // The staged IDs a page at a time (bounded memory whatever the transfer size; M5
    // review 4 minor), keyset-paged from a NULL start so no key, the empty one included,
    // is skipped.
    let paged = |table: &str,
                 column: &str,
                 each: &mut dyn FnMut(&str) -> Result<(), CliError>|
     -> Result<(), CliError> {
        let sql = format!(
            "SELECT {column} FROM {table} WHERE ?1 IS NULL OR {column} > ?1 ORDER BY {column} LIMIT 1000"
        );
        let mut after: Option<String> = None;
        loop {
            let page: Vec<String> = staged
                .stage
                .prepare(&sql)
                .map_err(sql_error)?
                .query_map([&after], |r| r.get(0))
                .map_err(sql_error)?
                .collect::<rusqlite::Result<_>>()
                .map_err(sql_error)?;
            let Some(last) = page.last().cloned() else {
                return Ok(());
            };
            for id in &page {
                each(id)?;
            }
            after = Some(last);
        }
    };
    paged("receipts", "receipt_id", &mut |receipt| {
        w.check()?;
        crate::repository::refresh_receipt_projection(&w.conn, receipt)
    })
    .map_err(rollback)?;
    paged("events", "event_id", &mut |event| {
        w.check()?;
        crate::repository::rebuild(&w.conn, event)
    })
    .map_err(rollback)?;
    let step = ImportCheckpoint {
        rebuilt: true,
        ..progress.clone()
    };
    journal_step(&w.conn, operation, &step, started).map_err(rollback)?;
    w.check().map_err(rollback)?;
    if let Err(e) = w.conn.execute_batch("COMMIT") {
        let _ = w.conn.execute_batch("ROLLBACK");
        return Err((sql_error(e), true));
    }
    generation_after_commit(&w).map_err(|e| (e, true))
}

/// `query_row` returning `Option` (no row) while propagating real errors.
trait OptionalRow<T> {
    fn optional_row(self) -> Result<Option<T>, CliError>;
}
impl<T> OptionalRow<T> for rusqlite::Result<T> {
    fn optional_row(self) -> Result<Option<T>, CliError> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(sql_error(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logical_transfer_export_is_coherent_allowlisted_private_and_has_no_missing_initialization() {
        let _isolation = crate::test_isolation();
        let temp = crate::test_support::TestDir::new("transfer-export");
        let cloud = Cloud::from_tenant(&airs_runtime_core::tenant::TenantContext {
            selected: airs_runtime_core::tenant::Selected {
                name: "fixture".into(),
                tsg_id: "100".into(),
                by_flag: false,
            },
            config: airs_runtime_core::tenant::config::TenantConfig::default(),
        })
        .unwrap();
        let root = temp.path().join("history");
        let file = temp.path().join("archive.jsonl");
        let session = scm::Session::new();
        assert!(
            export_file(
                &root,
                &cloud,
                &file,
                Duration::from_secs(10),
                &session,
                &SecretSet::default()
            )
            .is_err()
        );
        assert!(!root.exists());
        assert!(!file.exists());
        let e = airs_runtime_core::signal::evidence::Evidence::parse(include_str!(
            "../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        crate::repository::import_evidence(
            &root,
            &cloud,
            &e,
            &SecretSet::default(),
            &airs_runtime_core::signal::privacy::PayloadSpans::default(),
            &session,
        )
        .unwrap();
        let before = std::fs::read(root.join(&cloud.cloud_id).join("archive.sqlite3")).unwrap();
        let summary = export_file(
            &root,
            &cloud,
            &file,
            Duration::from_secs(10),
            &session,
            &SecretSet::default(),
        )
        .unwrap();
        assert_eq!(summary["network_requests"], 0);
        assert_eq!(summary["durable"], true);
        assert_eq!(summary["records"]["events"], 1);
        assert_eq!(
            std::fs::read(root.join(&cloud.cloud_id).join("archive.sqlite3")).unwrap(),
            before
        );
        let mut decoder =
            codec::Decoder::new(std::io::BufReader::new(std::fs::File::open(&file).unwrap()));
        let mut footers = 0;
        while let Some(line) = decoder.next().unwrap() {
            if matches!(line, codec::Line::Footer(_)) {
                footers += 1;
            }
        }
        assert_eq!(footers, 1);
        assert!(
            export_file(
                &root,
                &cloud,
                &file,
                Duration::from_secs(10),
                &session,
                &SecretSet::default()
            )
            .is_err()
        );
    }
}
