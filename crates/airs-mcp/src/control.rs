//! The service control store, `control/control.sqlite3` (RUNTIME_MCP_CONTRACT §9.1,
//! §9.2, §10). It holds jobs, idempotency, send reservations, allowances and audit;
//! never credentials. Every state change is one transaction; no network, rendering or
//! user interaction happens inside one. Lock order is maintenance gate → control →
//! archive: this module never opens the archive.

use std::path::{Path, PathBuf};

use airs_runtime_core::dto::ErrorCode;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::jobs::{State, Trigger, transition};

pub const FILE: &str = "control.sqlite3";
/// A backup set's manifest, published last (crash point C9).
pub const MANIFEST: &str = "manifest.json";
/// Version 2 (M5): `upload_sessions.state` gains `removed`. Version 1 is the accepted-M4
/// schema, kept frozen in `control_v1.sql`; a version-1 store is migrated (verified, on a
/// private copy) when it is opened or restored ([`migrate_file`]).
pub const SCHEMA_VERSION: i64 = 3;
const SCHEMA: &str = include_str!("control.sql");
/// The accepted-M4 control schema (version 1), frozen.
const SCHEMA_V1: &str = include_str!("control_v1.sql");
/// Schema version 2 (accepted M4 + M5), frozen: version 3 adds M6's report tables.
const SCHEMA_V2: &str = include_str!("control_v2.sql");
/// Queue bounds (contract §6): queued jobs per tenant and per principal.
pub const DEFAULT_QUEUE_TENANT: u64 = 64;
pub const DEFAULT_QUEUE_PRINCIPAL: u64 = 8;
/// What an `overloaded` refusal asks the caller to wait.
pub const OVERLOADED_RETRY_SECONDS: u64 = 30;
/// An allowance count beyond every ceiling: what recovery records for a day whose
/// consumption it cannot know.
const EXHAUSTED: i64 = 1_000_000;
/// Marks that this `control/` directory has held a provisioned store, so a later
/// missing database is a loss, not a first start (C11).
pub const PROVISIONED: &str = "provisioned";

/// The tables a control store of this schema must have.
fn required_tables() -> Vec<&'static str> {
    SCHEMA
        .lines()
        .filter_map(|l| l.strip_prefix("CREATE TABLE "))
        .filter_map(|l| l.split([' ', '(']).next())
        .collect()
}

/// The store's schema objects as `(type, name, sql)`, internal objects excluded.
fn fingerprint(conn: &Connection) -> Result<Vec<(String, String, String)>, &'static str> {
    let mut stmt = conn
        .prepare("SELECT type, name, coalesce(sql,'') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name")
        .map_err(|_| "control store cannot be read")?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|_| "control store cannot be read")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "control store cannot be read")
}

/// The fingerprint of a store freshly created from this version's schema.
fn expected_fingerprint() -> Result<Vec<(String, String, String)>, &'static str> {
    schema_fingerprint(SCHEMA)
}

fn schema_fingerprint(schema: &str) -> Result<Vec<(String, String, String)>, &'static str> {
    let fresh = Connection::open_in_memory().map_err(|_| "control schema unavailable")?;
    fresh
        .execute_batch(schema)
        .map_err(|_| "control schema unavailable")?;
    fingerprint(&fresh)
}

/// Migrate a control-store file in place from schema version 1 (the accepted-M4
/// structure, exactly) to this version. Call it only on a private copy: the caller
/// validates the result in full and publishes it, keeping the original. Returns whether
/// a migration happened; a store already at this version is left alone; any other
/// version or structure is refused.
///
/// Version 1 → 2 widens one `CHECK` (`upload_sessions.state` gains `removed`). Every
/// existing row already satisfies the wider constraint, so the table definition is
/// rewritten to this version's exact text (SQLite's documented `writable_schema`
/// procedure for constraint changes), the schema cookie advanced, and the result checked:
/// integrity, foreign keys, and a fingerprint identical to a fresh version-2 store. Jobs,
/// checkpoints, epochs, authority records, uploads and quotas are untouched rows.
///
/// Version 2 → 3 (M6) only adds the report tables and indexes, new and empty, created
/// from this version's own schema text; a version-1 store takes both steps in the same
/// exclusive transaction. The result must fingerprint exactly as a fresh version-3 store.
pub fn migrate_file(path: &Path) -> Result<bool, &'static str> {
    let conn = Connection::open(path).map_err(|_| "control store cannot be opened")?;
    let version: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key='schema'", [], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|_| "control store cannot be read")?;
    match version.as_deref() {
        Some(v) if v == SCHEMA_VERSION.to_string() => return Ok(false),
        Some("1") | Some("2") => {}
        _ => return Err("control store schema is not supported"),
    }
    let from_v1 = version.as_deref() == Some("1");
    let source = if from_v1 { SCHEMA_V1 } else { SCHEMA_V2 };
    if fingerprint(&conn)? != schema_fingerprint(source)? {
        return Err("control store structure does not match its recorded schema version");
    }
    let fresh = Connection::open_in_memory().map_err(|_| "control schema unavailable")?;
    fresh
        .execute_batch(SCHEMA)
        .map_err(|_| "control schema unavailable")?;
    let target: String = fresh
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='upload_sessions'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "control schema unavailable")?;
    let cookie: i64 = conn
        .query_row("PRAGMA schema_version", [], |r| r.get(0))
        .map_err(|_| "control store cannot be read")?;
    conn.execute_batch("BEGIN EXCLUSIVE; PRAGMA writable_schema=ON;")
        .map_err(|_| "control store cannot be migrated")?;
    // Objects version 3 adds (M6's report tables and indexes): new and empty, created
    // from this version's own schema text, in creation order.
    let added: Vec<String> = {
        let have: std::collections::BTreeSet<String> = fingerprint(&conn)?
            .into_iter()
            .map(|(_, name, _)| name)
            .collect();
        let mut stmt = fresh
            .prepare("SELECT name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND sql IS NOT NULL ORDER BY rowid")
            .map_err(|_| "control schema unavailable")?;
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|_| "control schema unavailable")?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "control schema unavailable")?
            .into_iter()
            .filter(|(name, _)| !have.contains(name))
            .map(|(_, sql)| sql)
            .collect()
    };
    let migrated = (|| {
        if from_v1 {
            conn.execute(
                "UPDATE sqlite_master SET sql=?1 WHERE type='table' AND name='upload_sessions'",
                [&target],
            )
            .map_err(|_| "control store cannot be migrated")?;
            conn.execute_batch(&format!("PRAGMA schema_version={}", cookie + 1))
                .map_err(|_| "control store cannot be migrated")?;
        }
        conn.execute_batch("PRAGMA writable_schema=OFF;")
            .map_err(|_| "control store cannot be migrated")?;
        for sql in &added {
            conn.execute_batch(sql)
                .map_err(|_| "control store cannot be migrated")?;
        }
        conn.execute(
            "UPDATE meta SET value=?1 WHERE key='schema'",
            [SCHEMA_VERSION.to_string()],
        )
        .map_err(|_| "control store cannot be migrated")?;
        Ok::<_, &'static str>(())
    })();
    match migrated {
        Ok(()) => conn
            .execute_batch("COMMIT")
            .map_err(|_| "control store cannot be migrated")?,
        Err(e) => {
            let _ = conn.execute_batch("PRAGMA writable_schema=OFF; ROLLBACK");
            return Err(e);
        }
    }
    drop(conn);
    // Re-read from a fresh connection: the migrated schema must be exactly this version's.
    let check = Connection::open(path).map_err(|_| "control store cannot be opened")?;
    let ok: String = check
        .query_row("PRAGMA integrity_check(1)", [], |r| r.get(0))
        .map_err(|_| "control store cannot be read")?;
    if ok != "ok" || fingerprint(&check)? != expected_fingerprint()? {
        return Err("control store migration did not verify");
    }
    Ok(true)
}

/// Validate an opened copy's identity and structure without trusting any manifest:
/// integrity, schema, service tenant, every required table, referential integrity,
/// and an epoch that is either active or awaiting activation. Read-only.
fn validate(conn: &Connection, tenant: &str) -> Result<(), &'static str> {
    let one = |sql: &str| -> Result<Option<String>, &'static str> {
        conn.query_row(sql, [], |r| r.get(0))
            .optional()
            .map_err(|_| "control store cannot be read")
    };
    if one("PRAGMA integrity_check(1)")?.as_deref() != Some("ok") {
        return Err("control store integrity check failed");
    }
    if one("SELECT value FROM meta WHERE key='schema'")? != Some(SCHEMA_VERSION.to_string()) {
        return Err("control store schema is not supported");
    }
    let owner: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key='service_tenant'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| "control store cannot be read")?;
    if owner.as_deref() != Some(tenant) {
        return Err("control store belongs to another service tenant");
    }
    // The structure itself, not only table names: every table, column, constraint and
    // index must be exactly what this version's schema creates.
    if fingerprint(conn)? != expected_fingerprint()? {
        return Err("control store structure does not match this version's schema");
    }
    for table in required_tables() {
        let present: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |r| r.get(0),
            )
            .map_err(|_| "control store cannot be read")?;
        if !present {
            return Err("control store lacks a required table");
        }
    }
    let dangling: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "control store cannot be read")?;
    if dangling {
        return Err("control store fails its referential integrity check");
    }
    let epochs: i64 = conn
        .query_row(
            "SELECT count(*) FROM epochs WHERE state IN ('active','pending_activation')",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "control store cannot be read")?;
    if epochs != 1 {
        return Err("control store has no single active or pending epoch");
    }
    Ok(())
}

/// A fixed control-store failure; never carries SQLite text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refused {
    pub code: ErrorCode,
    pub reason: &'static str,
    pub retry_after_seconds: Option<u64>,
}

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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Canonical JSON (sorted keys, no whitespace) and its SHA-256: the immutable parameter
/// record and digest of contract §10.
pub fn canonical(value: &Value) -> (String, String) {
    fn sort(v: &Value) -> Value {
        match v {
            Value::Object(m) => {
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                Value::Object(keys.into_iter().map(|k| (k.clone(), sort(&m[k]))).collect())
            }
            Value::Array(a) => Value::Array(a.iter().map(sort).collect()),
            other => other.clone(),
        }
    }
    let text = sort(value).to_string();
    let digest = hex(&Sha256::digest(text.as_bytes()));
    (text, digest)
}

/// Who submitted a job: a principal, or the service actor (schedules).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Actor {
    Principal { sub: String, client_id: String },
    Service(String),
}

/// One durable submission (contract §10: persisted before a job ID is returned).
pub struct Submission<'a> {
    pub tenant: &'a str,
    pub operation: &'a str,
    pub actor: Actor,
    pub idempotency_key: &'a str,
    pub expected_epoch: &'a str,
    pub params: &'a Value,
    pub config_revision: &'a str,
    pub credential_revision: Option<&'a str>,
    pub deadline_ms: i64,
    pub queue_tenant: u64,
    pub queue_principal: u64,
    /// Join an equivalent live job of the same operation and parameters instead of
    /// queueing another (contract §6: equivalent queued collections join).
    pub join_equivalent: bool,
    /// A scheduled submission's `(schedule, slot)`: the job's `scheduled_slot` and the
    /// slot's `submitted` association are written in the submission's own transaction
    /// (contract §10: the slot persists with the submission), and a same-key retry
    /// repairs them.
    pub scheduled_slot: Option<(&'a str, &'a str)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submitted {
    pub job_id: String,
    /// `true` when an earlier submission with the same key and digest already made it.
    pub existing: bool,
    /// `true` when this key was attached to an equivalent live job.
    pub joined: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    pub job_id: String,
    pub epoch: String,
    pub tenant: String,
    pub operation: String,
    pub actor: Actor,
    pub state: State,
    pub submitted_ms: i64,
    pub deadline_ms: i64,
    pub retry_after_ms: Option<i64>,
    pub updated_ms: i64,
    pub params_digest: String,
    pub result: Option<Value>,
    /// Coverage or execution gaps recorded with the job (contract §10: a deadline
    /// failure lists its gap).
    pub gaps: Vec<String>,
}

/// The outcome of a send reservation (crash point C2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendOutcome {
    /// The request left the process.
    Sent,
    /// It may have left (timeout, cancellation after write): stays consumed.
    Uncertain,
    /// Proven never to have left (refused before transport): releases the allowance.
    NotSent,
}

/// A complete backup set's manifest (crash point C9).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupManifest {
    pub schema: i64,
    pub tenant: String,
    pub epoch: String,
    pub created_ms: i64,
    pub bytes: u64,
    pub sha256: String,
}

/// Restore a control store from a complete backup set (crash points C10, C11) into
/// `control_dir`, which must not hold a control store (a damaged one is moved aside by
/// the operator, never deleted here). The set is validated before anything is placed:
/// complete manifest, size and SHA-256, then the staged copy's own integrity, schema,
/// tenant, tables, referential integrity and epoch against the manifest. The staged
/// copy is then marked restored in one transaction before it is published:
///
/// - every active epoch is retired and a new epoch waits for explicit Administrator
///   activation, so the service accepts no submission until then and every older
///   idempotency key is `control_restored`;
/// - job states are kept, claims released, open reservations `uncertain` (consumed);
/// - the store enters recovery quarantine ([`Control::enter_quarantine`]): nothing
///   runs until reconciled, the admission day is exhausted, every plane holds a
///   cooldown of at least 24 hours, and schedules are disabled.
pub fn restore(
    set: &Path,
    control_dir: &Path,
    tenant: &str,
    now_ms: i64,
) -> Result<String, Refused> {
    let bad = |reason: &'static str| refused(ErrorCode::InvalidRequest, reason);
    let manifest: BackupManifest = serde_json::from_slice(
        &std::fs::read(set.join(MANIFEST)).map_err(|_| bad("the backup set is incomplete"))?,
    )
    .map_err(|_| bad("the backup manifest is invalid"))?;
    let bytes = std::fs::read(set.join(FILE)).map_err(|_| bad("the backup copy is missing"))?;
    if !(manifest.schema == SCHEMA_VERSION || manifest.schema == 1 || manifest.schema == 2)
        || manifest.tenant != tenant
        || bytes.len() as u64 != manifest.bytes
        || hex(&Sha256::digest(&bytes)) != manifest.sha256
    {
        return Err(bad(
            "the backup set does not match its manifest or this service",
        ));
    }
    let target = control_dir.join(FILE);
    if target.exists() {
        return Err(refused(
            ErrorCode::Conflict,
            "a control store exists; move it aside explicitly before restoring",
        ));
    }
    let staged = control_dir.join(format!(".restore-{}.sqlite3", uuid_v4()));
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged)
            .map_err(|_| refused(ErrorCode::StorageUnavailable, "cannot stage the restore"))?;
        f.write_all(&bytes)
            .and_then(|_| f.sync_all())
            .map_err(|_| refused(ErrorCode::StorageUnavailable, "cannot stage the restore"))?;
    }
    let result = (|| {
        // A version-1 (accepted-M4) backup is migrated on the staged copy, then validated
        // in full like any other.
        migrate_file(&staged).map_err(|_| bad("the backup copy does not match this service"))?;
        let mut conn = Connection::open(&staged).map_err(storage)?;
        conn.execute_batch(
            "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;",
        )
        .map_err(storage)?;
        // The copy's own contents must match the manifest and this service before
        // anything in it is changed or published (the manifest alone is not trusted).
        validate(&conn, tenant).map_err(|_| bad("the backup copy does not match this service"))?;
        let recorded: Option<String> = conn
            .query_row("SELECT epoch FROM epochs WHERE state='active'", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(storage)?;
        if recorded.as_deref() != Some(manifest.epoch.as_str()) {
            return Err(bad("the backup copy's epoch does not match its manifest"));
        }
        let epoch = format!(
            "{:08x}",
            u32::from_be_bytes(
                uuid::Uuid::new_v4().as_bytes()[..4]
                    .try_into()
                    .expect("4 bytes")
            )
        );
        let key = *uuid::Uuid::new_v4().as_bytes();
        let key = [key, *uuid::Uuid::new_v4().as_bytes()].concat();
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(storage)?;
        tx.execute(
            "UPDATE epochs SET state='retired' WHERE state!='retired'",
            [],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO epochs VALUES (?1, ?2, NULL, 'restore', ?3, 'pending_activation')",
            params![epoch, now_ms, key],
        )
        .map_err(storage)?;
        // Job states are kept as the backup recorded them (contract §10 has no restore
        // edge); the quarantine keeps every one of them from running. Claims of the
        // backed-up process are released and its open reservations stay consumed.
        tx.execute(
            "UPDATE jobs SET claimed_by=NULL WHERE claimed_by IS NOT NULL",
            [],
        )
        .map_err(storage)?;
        tx.execute(
            "UPDATE send_reservations SET state='uncertain', updated_ms=?1 WHERE state='reserved'",
            [now_ms],
        )
        .map_err(storage)?;
        Control::enter_quarantine(&tx, tenant, "restored", now_ms)?;
        tx.commit().map_err(storage)?;
        drop(conn);
        Ok(epoch)
    })();
    let epoch = match result {
        Ok(epoch) => epoch,
        Err(e) => {
            let _ = std::fs::remove_file(&staged);
            return Err(e);
        }
    };
    // No-clobber publication: the hard link fails if a store appeared meanwhile.
    std::fs::hard_link(&staged, &target).map_err(|_| {
        refused(
            ErrorCode::Conflict,
            "a control store appeared during the restore",
        )
    })?;
    let _ = std::fs::remove_file(&staged);
    std::fs::File::open(control_dir)
        .and_then(|d| d.sync_all())
        .map_err(|_| {
            refused(
                ErrorCode::StorageUnavailable,
                "cannot sync the control directory",
            )
        })?;
    Ok(epoch)
}

/// What startup recovery changed (crash points C2, C12).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recovery {
    /// `reserved` reservations of a previous process, now `uncertain` (consumed).
    pub uncertain: u64,
    /// `running` jobs of a previous owner, released (still `running`) so the next
    /// claim resumes them from their checkpoint.
    pub requeued: u64,
}

pub struct Control {
    pub(crate) conn: Connection,
    path: PathBuf,
}

fn storage_io(_: std::io::Error) -> Refused {
    refused(ErrorCode::StorageUnavailable, "control storage I/O failed")
}

fn sync_dir(dir: &Path) -> Result<(), Refused> {
    std::fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(storage_io)
}

/// Test-only crash point: the crash tests' child process dies here, inside the real
/// transaction, when its parent names this point. Compiled out of every other build.
#[cfg(test)]
fn crash_point(name: &str) {
    if std::env::var("AIRS_MCP_CONTROL_CRASH_POINT").as_deref() == Ok(name)
        && std::env::var_os("AIRS_MCP_CONTROL_CRASH_DIR").is_some()
    {
        std::process::exit(88);
    }
}

#[cfg(not(test))]
fn crash_point(_: &str) {}

fn uuid_v4() -> String {
    uuid::Uuid::new_v4().to_string()
}

impl Control {
    /// Open the control store under the admitted, exclusively owned `control/`
    /// directory.
    ///
    /// - **First start** (no database, never provisioned): a new store adopts the
    ///   seed's epoch and cursor key as its active epoch, is built in a staged file and
    ///   published without clobbering, and the directory is marked provisioned.
    /// - **Loss** (no database, but the directory was provisioned): the seed's epoch is
    ///   never reused. A new store gets a fresh epoch awaiting explicit Administrator
    ///   activation and enters recovery quarantine (C11), exactly as a restore does.
    /// - **Existing store** (C12): the database and any sidecars are copied into a
    ///   private directory, and only that copy is opened read-write (hot-journal
    ///   rollback happens there) and validated: integrity, schema, tenant, tables,
    ///   referential integrity, epoch. A refused store's original bytes and sidecars are
    ///   never touched. When the original had a hot journal or WAL, it is preserved
    ///   under `preserved-<ms>-<uuid>/` and the verified, recovered copy is published
    ///   in its place.
    pub fn open(
        dir: &Path,
        tenant: &str,
        seed_epoch: &str,
        seed_key: &[u8; 32],
        now_ms: i64,
    ) -> Result<Self, Refused> {
        let path = dir.join(FILE);
        let unavailable = |reason| refused(ErrorCode::StorageUnavailable, reason);
        if !path.exists() {
            let lost = dir.join(PROVISIONED).exists();
            let staged = dir.join(format!(".create-{}.sqlite3", uuid_v4()));
            let built = (|| {
                let conn = Connection::open(&staged).map_err(storage)?;
                Self::pragmas(&conn)?;
                let mut fresh = Self {
                    conn,
                    path: staged.clone(),
                };
                if lost {
                    fresh.initialize_after_loss(tenant, now_ms)?;
                } else {
                    fresh.initialize(tenant, seed_epoch, seed_key, now_ms)?;
                }
                Ok::<_, Refused>(())
            })();
            if let Err(e) = built {
                let _ = std::fs::remove_file(&staged);
                return Err(e);
            }
            let published = std::fs::hard_link(&staged, &path);
            let _ = std::fs::remove_file(&staged);
            published.map_err(|_| {
                refused(
                    ErrorCode::Conflict,
                    "a control store appeared during creation",
                )
            })?;
            sync_dir(dir)?;
        } else {
            Self::verify_copy(dir, &path, tenant, now_ms)?;
        }
        Self::mark_provisioned(dir)?;
        let conn =
            Connection::open(&path).map_err(|_| unavailable("cannot open the control store"))?;
        Self::pragmas(&conn)?;
        let control = Self { conn, path };
        control.verify(tenant)?;
        Ok(control)
    }

    fn pragmas(conn: &Connection) -> Result<(), Refused> {
        conn.execute_batch(
            "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA secure_delete=ON; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=2000;",
        )
        .map_err(storage)
    }

    fn mark_provisioned(dir: &Path) -> Result<(), Refused> {
        use std::os::unix::fs::OpenOptionsExt;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(dir.join(PROVISIONED))
        {
            Ok(f) => {
                f.sync_all().map_err(storage_io)?;
                sync_dir(dir)
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(_) => Err(refused(
                ErrorCode::StorageUnavailable,
                "cannot mark the control directory",
            )),
        }
    }

    /// C12: validate an existing store on a private copy before the original is opened.
    fn verify_copy(dir: &Path, path: &Path, tenant: &str, now_ms: i64) -> Result<(), Refused> {
        use std::os::unix::fs::DirBuilderExt;
        let refuse = |reason: &'static str| refused(ErrorCode::MaintenanceRequired, reason);
        let work = dir.join(format!(".verify-{}", uuid_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&work)
            .map_err(storage_io)?;
        let cleanup = |work: &Path| {
            let _ = std::fs::remove_dir_all(work);
        };
        let sidecars: Vec<String> = ["-journal", "-wal", "-shm"]
            .iter()
            .map(|suffix| format!("{FILE}{suffix}"))
            .filter(|name| dir.join(name).exists())
            .collect();
        let copied = std::iter::once(FILE.to_owned())
            .chain(sidecars.iter().cloned())
            .try_for_each(|name| std::fs::copy(dir.join(&name), work.join(&name)).map(|_| ()));
        if copied.is_err() {
            cleanup(&work);
            return Err(refused(
                ErrorCode::StorageUnavailable,
                "cannot copy the control store for verification",
            ));
        }
        let checked = (|| {
            {
                // Roll back any hot journal on the copy first (opening it does), then
                // migrate a version-1 (accepted-M4) store on the copy.
                let conn = Connection::open(work.join(FILE))
                    .map_err(|_| refuse("control store cannot be opened"))?;
                conn.execute_batch("PRAGMA journal_mode=DELETE;")
                    .map_err(|_| refuse("control store cannot be recovered"))?;
            }
            let migrated = migrate_file(&work.join(FILE)).map_err(refuse)?;
            let conn = Connection::open(work.join(FILE))
                .map_err(|_| refuse("control store cannot be opened"))?;
            validate(&conn, tenant).map_err(refuse)?;
            // Fold any recovered journal or WAL into the copy's main file.
            conn.execute_batch("PRAGMA journal_mode=DELETE;")
                .map_err(|_| refuse("control store cannot be recovered"))?;
            validate(&conn, tenant).map_err(refuse)?;
            Ok(migrated)
        })();
        let migrated = match checked {
            Ok(m) => m,
            Err(e) => {
                cleanup(&work);
                return Err(e);
            }
        };
        if !sidecars.is_empty() || migrated {
            // Preserve the original and its sidecars, then publish the verified copy.
            let preserved = dir.join(format!("preserved-{now_ms}-{}", uuid_v4()));
            let moved = std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&preserved)
                .and_then(|_| {
                    std::iter::once(FILE.to_owned())
                        .chain(sidecars.iter().cloned())
                        .try_for_each(|name| {
                            std::fs::rename(dir.join(&name), preserved.join(&name))
                        })
                })
                .and_then(|_| std::fs::rename(work.join(FILE), path));
            cleanup(&work);
            moved.map_err(|_| refuse("cannot preserve the original control store"))?;
            sync_dir(&preserved)?;
            return sync_dir(dir);
        }
        cleanup(&work);
        Ok(())
    }

    fn initialize(
        &mut self,
        tenant: &str,
        epoch: &str,
        key: &[u8; 32],
        now_ms: i64,
    ) -> Result<(), Refused> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(storage)?;
        tx.execute_batch(SCHEMA).map_err(storage)?;
        tx.execute(
            "INSERT INTO meta VALUES ('schema', ?1), ('created_ms', ?2), ('service_tenant', ?3)",
            params![SCHEMA_VERSION.to_string(), now_ms.to_string(), tenant],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO epochs VALUES (?1, ?2, ?2, 'initial', ?3, 'active')",
            params![epoch, now_ms, key.as_slice()],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }

    /// A store recreated after loss: schema and identity, a fresh epoch awaiting
    /// activation (never the seed's), and recovery quarantine.
    fn initialize_after_loss(&mut self, tenant: &str, now_ms: i64) -> Result<(), Refused> {
        let epoch = format!(
            "{:08x}",
            u32::from_be_bytes(
                uuid::Uuid::new_v4().as_bytes()[..4]
                    .try_into()
                    .expect("4 bytes")
            )
        );
        let key = [
            *uuid::Uuid::new_v4().as_bytes(),
            *uuid::Uuid::new_v4().as_bytes(),
        ]
        .concat();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(storage)?;
        tx.execute_batch(SCHEMA).map_err(storage)?;
        tx.execute(
            "INSERT INTO meta VALUES ('schema', ?1), ('created_ms', ?2), ('service_tenant', ?3)",
            params![SCHEMA_VERSION.to_string(), now_ms.to_string(), tenant],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO epochs VALUES (?1, ?2, NULL, 'replacement', ?3, 'pending_activation')",
            params![epoch, now_ms, key],
        )
        .map_err(storage)?;
        Self::enter_quarantine(&tx, tenant, "lost", now_ms)?;
        tx.commit().map_err(storage)
    }

    fn verify(&self, tenant: &str) -> Result<(), Refused> {
        let meta = |key: &str| -> Result<Option<String>, Refused> {
            self.conn
                .query_row("SELECT value FROM meta WHERE key=?1", [key], |r| r.get(0))
                .optional()
                .map_err(storage)
        };
        if meta("schema")? != Some(SCHEMA_VERSION.to_string()) {
            return Err(refused(
                ErrorCode::MaintenanceRequired,
                "control store schema is not supported",
            ));
        }
        if meta("service_tenant")?.as_deref() != Some(tenant) {
            return Err(refused(
                ErrorCode::MaintenanceRequired,
                "control store belongs to another service tenant",
            ));
        }
        let check: String = self
            .conn
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .map_err(storage)?;
        if check != "ok" {
            return Err(refused(
                ErrorCode::MaintenanceRequired,
                "control store integrity check failed; it was not modified",
            ));
        }
        // A restored store has no active epoch until an Administrator activates the
        // pending one; it opens, and every submission is `control_restored` meanwhile.
        if self.active_epoch().is_err() && self.pending_epoch()?.is_none() {
            return Err(refused(
                ErrorCode::MaintenanceRequired,
                "control store has neither an active nor a pending epoch",
            ));
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The active recovery epoch and its cursor key.
    pub fn active_epoch(&self) -> Result<(String, [u8; 32]), Refused> {
        let row: Option<(String, Vec<u8>)> = self
            .conn
            .query_row(
                "SELECT epoch, cursor_key FROM epochs WHERE state='active'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        match row {
            Some((epoch, key)) if key.len() == 32 => {
                let mut k = [0u8; 32];
                k.copy_from_slice(&key);
                Ok((epoch, k))
            }
            _ => Err(refused(
                ErrorCode::ControlRestored,
                "no active recovery epoch; an administrator must activate one",
            )),
        }
    }

    /// Crash point C1: idempotency row, job, parameters and first event in one
    /// transaction. The same key with the same digest returns the same job; a different
    /// digest is `idempotency_conflict`; a key minted under another epoch is
    /// `control_restored` and creates nothing.
    pub fn submit(&mut self, s: Submission<'_>, now_ms: i64) -> Result<Submitted, Refused> {
        let (canonical_json, digest) = canonical(s.params);
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let epoch: String = tx
            .query_row("SELECT epoch FROM epochs WHERE state='active'", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(storage)?
            .ok_or_else(|| refused(ErrorCode::ControlRestored, "no active recovery epoch"))?;
        if s.expected_epoch != epoch {
            return Err(refused(
                ErrorCode::ControlRestored,
                "the idempotency key was minted under another recovery epoch",
            ));
        }
        let (sub, client) = match &s.actor {
            Actor::Principal { sub, client_id } => (sub.as_str(), client_id.as_str()),
            Actor::Service(name) => ("", name.as_str()),
        };
        let prior: Option<(String, String)> = tx
            .query_row(
                "SELECT job_id, digest FROM idempotency WHERE tenant=?1 AND sub=?2 AND client_id=?3 AND operation=?4 AND epoch=?5 AND key=?6",
                params![s.tenant, sub, client, s.operation, epoch, s.idempotency_key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        if let Some((job_id, prior_digest)) = prior {
            return if prior_digest == digest {
                if let Some((name, slot)) = s.scheduled_slot {
                    Self::associate_slot(&tx, s.tenant, name, slot, &job_id)?;
                    tx.commit().map_err(storage)?;
                }
                Ok(Submitted {
                    job_id,
                    existing: true,
                    joined: false,
                })
            } else {
                Err(refused(
                    ErrorCode::IdempotencyConflict,
                    "the idempotency key was used with different parameters",
                ))
            };
        }
        if s.join_equivalent {
            // Join only the same submitter's job (so the joiner can see and cancel what
            // it was given) and only one that will still execute: never a job already
            // cancelling, blocked or awaiting credentials.
            let equivalent: Option<String> = tx
                .query_row(
                    "SELECT j.job_id FROM jobs j JOIN job_params p USING(job_id) WHERE j.tenant=?1 AND j.operation=?2 AND p.digest=?3 AND j.epoch=?4 AND j.state IN ('queued','running','waiting_retry') AND coalesce(j.submitter_sub,'')=?5 AND coalesce(j.submitter_client_id, j.service_actor)=?6 ORDER BY j.submitted_ms LIMIT 1",
                    params![s.tenant, s.operation, digest, epoch, sub, client],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage)?;
            if let Some(job_id) = equivalent {
                tx.execute(
                    "INSERT INTO idempotency VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        s.tenant,
                        sub,
                        client,
                        s.operation,
                        epoch,
                        s.idempotency_key,
                        job_id,
                        digest
                    ],
                )
                .map_err(storage)?;
                crash_point("submit-join");
                tx.commit().map_err(storage)?;
                return Ok(Submitted {
                    job_id,
                    existing: false,
                    joined: true,
                });
            }
        }
        Self::admit_queue(&tx, s.tenant, &s.actor, s.queue_tenant, s.queue_principal)?;
        let job_id = format!("j1_{epoch}_{}", uuid_v4());
        let (submitter_sub, submitter_client, service) = match &s.actor {
            Actor::Principal { sub, client_id } => {
                (Some(sub.as_str()), Some(client_id.as_str()), None)
            }
            Actor::Service(name) => (None, None, Some(name.as_str())),
        };
        tx.execute(
            "INSERT INTO jobs (job_id,epoch,tenant,operation,submitter_sub,submitter_client_id,service_actor,state,submitted_ms,deadline_ms,retry_after_ms,config_revision,credential_revision,scheduled_slot,claimed_by,result_json,updated_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,'queued',?8,?9,NULL,?10,?11,?12,NULL,NULL,?8)",
            params![job_id, epoch, s.tenant, s.operation, submitter_sub, submitter_client, service, now_ms, s.deadline_ms, s.config_revision, s.credential_revision, s.scheduled_slot.map(|(_, slot)| slot)],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO job_params VALUES (?1, ?2, ?3)",
            params![job_id, canonical_json, digest],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO idempotency VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                s.tenant,
                sub,
                client,
                s.operation,
                epoch,
                s.idempotency_key,
                job_id,
                digest
            ],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO job_events VALUES (?1, 0, NULL, 'queued', 'submitted', ?2)",
            params![job_id, now_ms],
        )
        .map_err(storage)?;
        if let Some((name, slot)) = s.scheduled_slot {
            Self::associate_slot(&tx, s.tenant, name, slot, &job_id)?;
        }
        crash_point("submit");
        tx.commit().map_err(storage)?;
        Ok(Submitted {
            job_id,
            existing: false,
            joined: false,
        })
    }

    /// Associate a scheduled job with its slot inside the caller's transaction: the
    /// job's `scheduled_slot` and the slot row (`submitted`, or `completed` once the job
    /// is terminal). An existing row for the slot (pending, skipped, or recorded without
    /// a job) takes this job; a row already naming another job is left alone.
    pub(crate) fn associate_slot(
        tx: &Connection,
        tenant: &str,
        name: &str,
        slot: &str,
        job_id: &str,
    ) -> Result<(), Refused> {
        tx.execute(
            "UPDATE jobs SET scheduled_slot=?2 WHERE job_id=?1 AND scheduled_slot IS NULL",
            params![job_id, slot],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO schedule_slots VALUES (?1,?2,?3,?4,
                CASE WHEN (SELECT state FROM jobs WHERE job_id=?4) IN ('canceled','succeeded','partial','failed') THEN 'completed' ELSE 'submitted' END)
             ON CONFLICT(tenant,name,slot) DO UPDATE SET job_id=excluded.job_id, state=excluded.state
             WHERE schedule_slots.job_id IS NULL OR schedule_slots.job_id=excluded.job_id",
            params![tenant, name, slot, job_id],
        )
        .map_err(storage)?;
        Ok(())
    }

    /// Queue admission (contract §6), shared by every job-creating transaction
    /// (ordinary submissions and authority executions): at most `queue_tenant` live
    /// jobs per tenant and `queue_principal` per principal, else `overloaded` with a
    /// retry hint. Service-actor jobs count toward the tenant only.
    pub(crate) fn admit_queue(
        tx: &Connection,
        tenant: &str,
        actor: &Actor,
        queue_tenant: u64,
        queue_principal: u64,
    ) -> Result<(), Refused> {
        let live = "state IN ('queued','running','waiting_retry','needs_credentials','blocked_policy','cancel_requested')";
        let tenant_load: i64 = tx
            .query_row(
                &format!("SELECT count(*) FROM jobs WHERE tenant=?1 AND {live}"),
                [tenant],
                |r| r.get(0),
            )
            .map_err(storage)?;
        let principal_load: i64 = match actor {
            Actor::Principal { sub, client_id } => tx
                .query_row(
                    &format!("SELECT count(*) FROM jobs WHERE tenant=?1 AND submitter_sub=?2 AND submitter_client_id=?3 AND {live}"),
                    params![tenant, sub, client_id],
                    |r| r.get(0),
                )
                .map_err(storage)?,
            Actor::Service(_) => 0,
        };
        if tenant_load as u64 >= queue_tenant || principal_load as u64 >= queue_principal {
            return Err(Refused {
                code: ErrorCode::Overloaded,
                reason: "the job queue is full; retry later",
                retry_after_seconds: Some(OVERLOADED_RETRY_SECONDS),
            });
        }
        Ok(())
    }

    fn read_job(conn: &Connection, tenant: &str, job_id: &str) -> Result<Option<Job>, Refused> {
        conn.query_row(
            "SELECT j.job_id,j.epoch,j.tenant,j.operation,j.submitter_sub,j.submitter_client_id,j.service_actor,j.state,j.submitted_ms,j.deadline_ms,j.retry_after_ms,p.digest,j.result_json,j.updated_ms,j.gaps_json FROM jobs j JOIN job_params p USING(job_id) WHERE j.tenant=?1 AND j.job_id=?2",
            params![tenant, job_id],
            |r| {
                let actor = match (r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?, r.get::<_, Option<String>>(6)?) {
                    (Some(sub), Some(client_id), None) => Actor::Principal { sub, client_id },
                    (_, _, Some(service)) => Actor::Service(service),
                    _ => Actor::Service(String::new()),
                };
                Ok(Job {
                    job_id: r.get(0)?,
                    epoch: r.get(1)?,
                    tenant: r.get(2)?,
                    operation: r.get(3)?,
                    actor,
                    state: State::parse(&r.get::<_, String>(7)?).unwrap_or(State::Failed),
                    submitted_ms: r.get(8)?,
                    deadline_ms: r.get(9)?,
                    retry_after_ms: r.get(10)?,
                    updated_ms: r.get(13)?,
                    params_digest: r.get(11)?,
                    result: r
                        .get::<_, Option<String>>(12)?
                        .and_then(|t| serde_json::from_str(&t).ok()),
                    gaps: serde_json::from_str(&r.get::<_, String>(14)?).unwrap_or_default(),
                })
            },
        )
        .optional()
        .map_err(storage)
    }

    /// Record that `revision` was denied on `plane` (a 401 after the SDK's single
    /// fresh-token replay, or a 403). Contract §10: such a revision is never retried.
    pub fn record_denial(
        &mut self,
        tenant: &str,
        plane: &str,
        revision: &str,
        now_ms: i64,
    ) -> Result<(), Refused> {
        self.conn
            .execute(
                "INSERT INTO credential_denials VALUES (?1,?2,'',?3,?4) ON CONFLICT DO NOTHING",
                params![tenant, plane, revision, now_ms],
            )
            .map_err(storage)?;
        Ok(())
    }

    /// Whether `revision` is already denied on `plane`: callers stop before any send.
    pub fn denied(&self, tenant: &str, plane: &str, revision: &str) -> Result<bool, Refused> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM credential_denials WHERE tenant=?1 AND plane=?2 AND application='' AND credential_revision=?3)",
                params![tenant, plane, revision],
                |r| r.get(0),
            )
            .map_err(storage)
    }

    /// The archive runs the tenant's checkpoints bind to `revision` (the run ID of each
    /// `collecting` checkpoint recorded with that revision), most recent first, at most
    /// `limit`: the archive's denial authority reads them for runs recorded before the
    /// archive held the binding itself (contract §6.1).
    pub fn bound_runs(
        &self,
        tenant: &str,
        revision: &str,
        limit: i64,
    ) -> Result<Vec<String>, Refused> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT run FROM (
                   SELECT coalesce(c.archive_operation_id, json_extract(c.state_json,'$.run_id')) AS run, max(c.created_ms) AS at
                     FROM job_checkpoints c JOIN jobs j USING(job_id)
                    WHERE j.tenant=?1 AND c.phase='collecting'
                      AND json_extract(c.state_json,'$.credential_revision')=?2
                    GROUP BY run)
                 WHERE run IS NOT NULL ORDER BY at DESC LIMIT ?3",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![tenant, revision, limit], |r| r.get(0))
            .map_err(storage)?
            .collect::<Result<Vec<String>, _>>()
            .map_err(storage)?;
        Ok(rows)
    }

    /// The active credential revision for `plane` (contract §10 "validated same-scope
    /// credential revision"): every `needs_credentials` job of the tenant parked under
    /// another revision (or under none recorded) is requeued through the §10 edge,
    /// keeping its original deadline and consumed sends; a job parked under `revision`
    /// itself stays parked, so repeating the call is idempotent. Each job resumes in one
    /// transaction (its edge and its revision together): a failure leaves it parked for
    /// the next call. A revision that is itself denied resumes nothing. Returns how many
    /// resumed.
    pub fn credentials_restored(
        &mut self,
        tenant: &str,
        plane: &str,
        revision: &str,
        now_ms: i64,
    ) -> Result<u64, Refused> {
        if self.denied(tenant, plane, revision)? {
            return Ok(0);
        }
        let ids: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT job_id FROM jobs WHERE tenant=?1 AND state='needs_credentials' AND credential_revision IS NOT ?2 ORDER BY job_id")
                .map_err(storage)?;
            stmt.query_map(params![tenant, revision], |r| r.get(0))
                .map_err(storage)?
                .collect::<Result<Vec<String>, _>>()
                .map_err(storage)?
        };
        let mut resumed = 0;
        for id in &ids {
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(storage)?;
            let Some(job) = Self::read_job(&tx, tenant, id)? else {
                continue;
            };
            if job.state != State::NeedsCredentials {
                continue;
            }
            Self::record(
                &tx,
                id,
                job.state,
                Trigger::CredentialsRestored,
                "credential revision changed",
                now_ms,
            )?;
            tx.execute(
                "UPDATE jobs SET credential_revision=?2 WHERE job_id=?1",
                params![id, revision],
            )
            .map_err(storage)?;
            tx.commit().map_err(storage)?;
            resumed += 1;
        }
        Ok(resumed)
    }

    /// Hold `plane` until `until_ms` (a 429): an existing longer hold is kept.
    pub fn hold(
        &mut self,
        tenant: &str,
        plane: &str,
        until_ms: i64,
        reason: &str,
    ) -> Result<(), Refused> {
        self.conn
            .execute(
                "INSERT INTO cooldowns VALUES (?1,?2,'',?3,?4) ON CONFLICT(tenant, plane, application) DO UPDATE SET until_ms=max(until_ms, excluded.until_ms), reason=excluded.reason",
                params![tenant, plane, until_ms, reason],
            )
            .map_err(storage)?;
        Ok(())
    }

    /// The latest cooldown on `plane` for the tenant (any application), if it still holds.
    pub fn cooldown_until(
        &self,
        tenant: &str,
        plane: &str,
        now_ms: i64,
    ) -> Result<Option<i64>, Refused> {
        self.conn
            .query_row(
                "SELECT max(until_ms) FROM cooldowns WHERE tenant=?1 AND plane=?2 AND until_ms>?3",
                params![tenant, plane, now_ms],
                |r| r.get(0),
            )
            .map_err(storage)
    }

    /// A job's canonical parameters, exactly as submitted (`job_params`).
    pub fn params(&self, job_id: &str) -> Result<Value, Refused> {
        let text: String = self
            .conn
            .query_row(
                "SELECT canonical_json FROM job_params WHERE job_id=?1",
                [job_id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        serde_json::from_str(&text).map_err(|_| {
            refused(
                ErrorCode::StorageUnavailable,
                "stored parameters are invalid",
            )
        })
    }

    /// One job, tenant-scoped.
    pub fn job(&self, tenant: &str, job_id: &str) -> Result<Option<Job>, Refused> {
        Self::read_job(&self.conn, tenant, job_id)
    }

    /// Apply one contract §10 edge in its own transaction, with a job event. A refused
    /// edge changes nothing and is `conflict`.
    pub fn apply(
        &mut self,
        tenant: &str,
        job_id: &str,
        trigger: Trigger,
        reason: &str,
        now_ms: i64,
    ) -> Result<State, Refused> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let job = Self::read_job(&tx, tenant, job_id)?
            .ok_or_else(|| refused(ErrorCode::NotFound, "no such job"))?;
        let to = Self::record(&tx, job_id, job.state, trigger, reason, now_ms)?;
        tx.commit().map_err(storage)?;
        Ok(to)
    }

    /// Record one state change, validated by the frozen §10 table: the edge is
    /// `transition(from, trigger)` or the call is refused. Every writer of `jobs.state`
    /// goes through here (submission writes the initial `queued` row).
    fn record(
        tx: &Connection,
        job_id: &str,
        from: State,
        trigger: Trigger,
        reason: &str,
        now_ms: i64,
    ) -> Result<State, Refused> {
        let to = transition(from, trigger)
            .ok_or_else(|| refused(ErrorCode::Conflict, "the job's state does not allow this"))?;
        let sequence: i64 = tx
            .query_row(
                "SELECT coalesce(max(sequence),-1)+1 FROM job_events WHERE job_id=?1",
                [job_id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        let changed = tx
            .execute(
                "UPDATE jobs SET state=?1, updated_ms=?2 WHERE job_id=?3 AND state=?4",
                params![to.name(), now_ms, job_id, from.name()],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(refused(ErrorCode::Conflict, "the job changed concurrently"));
        }
        tx.execute(
            "INSERT INTO job_events VALUES (?1,?2,?3,?4,?5,?6)",
            params![job_id, sequence, from.name(), to.name(), reason, now_ms],
        )
        .map_err(storage)?;
        Ok(to)
    }

    /// `jobs_cancel`: explicit, idempotent by job. Only the submitter (or the service
    /// actor's jobs by an Operator via the caller's policy) reaches here; a terminal job
    /// is returned unchanged.
    pub fn cancel(&mut self, tenant: &str, job_id: &str, now_ms: i64) -> Result<State, Refused> {
        let job = self
            .job(tenant, job_id)?
            .ok_or_else(|| refused(ErrorCode::NotFound, "no such job"))?;
        if job.state.terminal() || job.state == State::CancelRequested {
            return Ok(job.state);
        }
        self.apply(
            tenant,
            job_id,
            Trigger::CancelRequest,
            "jobs_cancel",
            now_ms,
        )
    }

    /// Crash point C2: commit a `reserved` row before any AIRS send. Returns the
    /// transport ID the send must carry.
    pub fn reserve_send(
        &mut self,
        job_id: &str,
        source: &str,
        plane: &str,
        credential_revision: Option<&str>,
        now_ms: i64,
    ) -> Result<String, Refused> {
        // A job whose consumption recovery could not know has spent its budget (C11).
        let exhausted: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM exhausted_jobs WHERE job_id=?1)",
                [job_id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if exhausted {
            return Err(Refused {
                code: ErrorCode::QuotaExhausted,
                reason: "this job's consumption is unknown after recovery; its budget is spent",
                retry_after_seconds: None,
            });
        }
        // A cooldown on the plane (a 429, or a recovery hold) refuses the reservation
        // before any send, with the wait (contract §6, C11).
        let until: Option<i64> = self
            .conn
            .query_row(
                "SELECT max(c.until_ms) FROM cooldowns c JOIN jobs j ON j.tenant=c.tenant WHERE j.job_id=?1 AND c.plane=?2 AND c.until_ms>?3",
                params![job_id, plane, now_ms],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if let Some(until) = until {
            return Err(Refused {
                code: ErrorCode::UpstreamUnavailable,
                reason: "the plane is cooling down; retry later",
                retry_after_seconds: Some(((until - now_ms).max(0) as u64).div_ceil(1000)),
            });
        }
        let transport = uuid_v4();
        self.conn
            .execute(
                "INSERT INTO send_reservations VALUES (?1,?2,?3,?4,?5,'reserved',?6,?6)",
                params![
                    transport,
                    job_id,
                    source,
                    plane,
                    credential_revision,
                    now_ms
                ],
            )
            .map_err(storage)?;
        Ok(transport)
    }

    /// Record a reservation's outcome. Only `reserved` rows change; `sent` and
    /// `uncertain` never become `not_sent`.
    pub fn settle_send(
        &mut self,
        transport_id: &str,
        outcome: SendOutcome,
        now_ms: i64,
    ) -> Result<(), Refused> {
        let state = match outcome {
            SendOutcome::Sent => "sent",
            SendOutcome::Uncertain => "uncertain",
            SendOutcome::NotSent => "not_sent",
        };
        let changed = self
            .conn
            .execute(
                "UPDATE send_reservations SET state=?1, updated_ms=?2 WHERE transport_id=?3 AND state='reserved'",
                params![state, now_ms, transport_id],
            )
            .map_err(storage)?;
        if changed == 1 {
            Ok(())
        } else {
            Err(refused(
                ErrorCode::Conflict,
                "the reservation is already settled",
            ))
        }
    }

    /// Consumed sends (everything but `not_sent`) for `job_id`.
    /// The most recently submitted job of `operation` in the tenant.
    pub fn latest_job(&self, tenant: &str, operation: &str) -> Result<Option<Job>, Refused> {
        let id: Option<String> = self
            .conn
            .query_row(
                "SELECT job_id FROM jobs WHERE tenant=?1 AND operation=?2 ORDER BY submitted_ms DESC, job_id DESC LIMIT 1",
                params![tenant, operation],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        match id {
            Some(id) => self.job(tenant, &id),
            None => Ok(None),
        }
    }

    /// A job's consumed sends on one plane (anything but `not_sent`).
    pub fn plane_sends(&self, job_id: &str, plane: &str) -> Result<u64, Refused> {
        self.conn
            .query_row(
                "SELECT count(*) FROM send_reservations WHERE job_id=?1 AND plane=?2 AND state!='not_sent'",
                params![job_id, plane],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n.max(0) as u64)
            .map_err(storage)
    }

    pub fn consumed_sends(&self, job_id: &str) -> Result<u64, Refused> {
        self.conn
            .query_row(
                "SELECT count(*) FROM send_reservations WHERE job_id=?1 AND state!='not_sent'",
                [job_id],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n as u64)
            .map_err(storage)
    }

    /// Startup recovery under exclusive ownership (C2): every `reserved` row of the
    /// previous process becomes `uncertain` (consumed, never released), and every
    /// `running` job keeps its state with its claim released, to resume from its last
    /// checkpoint.
    pub fn recover(&mut self, now_ms: i64) -> Result<Recovery, Refused> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(storage)?;
        let uncertain = tx
            .execute(
                "UPDATE send_reservations SET state='uncertain', updated_ms=?1 WHERE state='reserved'",
                [now_ms],
            )
            .map_err(storage)? as u64;
        // Running jobs of the previous owner keep their state (contract §10 has no
        // recovery edge); their claim is released so the new owner resumes them from
        // their last checkpoint.
        let requeued = tx
            .execute(
                "UPDATE jobs SET claimed_by=NULL WHERE state='running' AND claimed_by IS NOT NULL",
                [],
            )
            .map_err(storage)? as u64;
        // A cancellation requested while the dead worker held its job completes at the
        // next scheduler pass: release the claim.
        tx.execute(
            "UPDATE jobs SET claimed_by=NULL WHERE state='cancel_requested'",
            [],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(Recovery {
            uncertain,
            requeued,
        })
    }

    /// Why the store is quarantined (`restored` or `lost`), if it is.
    fn quarantine_of(conn: &Connection) -> Result<Option<String>, Refused> {
        conn.query_row("SELECT value FROM meta WHERE key='quarantine'", [], |r| {
            r.get(0)
        })
        .optional()
        .map_err(storage)
    }

    /// The store's quarantine, if any (C10/C11: nothing is claimed until reconciled).
    pub fn quarantine(&self) -> Result<Option<String>, Refused> {
        Self::quarantine_of(&self.conn)
    }

    /// Enter recovery quarantine in `tx` (C10/C11), for a restored store or one
    /// recreated after loss. What the store cannot know is treated as spent:
    ///
    /// - the quarantine is persisted and survives epoch activation; the worker claims
    ///   nothing until an Administrator reconciles the store;
    /// - the Reports allowance of the current admission day is exhausted, and the
    ///   high-water mark moves to `now`, so no older day reopens;
    /// - every plane holds a cooldown of at least 24 hours, including planes the backup
    ///   recorded none for;
    /// - every schedule is disabled with `now` as its effective time (rows and history
    ///   are kept).
    fn enter_quarantine(
        tx: &Connection,
        tenant: &str,
        reason: &str,
        now_ms: i64,
    ) -> Result<(), Refused> {
        tx.execute(
            "INSERT INTO meta VALUES ('quarantine', ?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [reason],
        )
        .map_err(storage)?;
        let tz = airs_runtime_core::schedule::report_tz().map_err(|_| {
            refused(
                ErrorCode::StorageUnavailable,
                "embedded timezone unavailable",
            )
        })?;
        let high: Option<i64> = tx
            .query_row(
                "SELECT high_water_ms FROM allowance_high_water WHERE tenant=?1",
                [tenant],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        let ts = |ms: i64| {
            jiff::Timestamp::from_millisecond(ms)
                .map_err(|_| refused(ErrorCode::InvalidRequest, "invalid time"))
        };
        let (day, effective) =
            airs_runtime_core::schedule::admission_day(ts(now_ms)?, high.map(ts).transpose()?, &tz);
        tx.execute(
            "INSERT INTO allowance_days VALUES (?1,?2,?3,?3) ON CONFLICT(tenant, chicago_date) DO UPDATE SET sends=max(sends, excluded.sends), ids=max(ids, excluded.ids)",
            params![tenant, day.to_string(), EXHAUSTED],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO allowance_high_water VALUES (?1,?2) ON CONFLICT(tenant) DO UPDATE SET high_water_ms=max(high_water_ms, excluded.high_water_ms)",
            params![tenant, effective.as_millisecond()],
        )
        .map_err(storage)?;
        let hold = now_ms + 86_400_000;
        tx.execute("UPDATE cooldowns SET until_ms=max(until_ms, ?1)", [hold])
            .map_err(storage)?;
        for plane in ["management", "scanner", "reports", "token"] {
            tx.execute(
                "INSERT INTO cooldowns VALUES (?1,?2,'',?3,'recovery') ON CONFLICT(tenant, plane, application) DO UPDATE SET until_ms=max(until_ms, excluded.until_ms)",
                params![tenant, plane, hold],
            )
            .map_err(storage)?;
        }
        tx.execute(
            "UPDATE schedules SET enabled=0, effective_ms=?1 WHERE enabled=1",
            [now_ms],
        )
        .map_err(storage)?;
        // Sends a job made after the backup are unknown, so its whole budget is treated
        // as spent: every non-terminal job is marked exhausted. The mark is enforced at
        // reservation and at claim, and nothing (reconciliation, cooldown expiry, a clock
        // rollback) removes it.
        tx.execute(
            "INSERT OR IGNORE INTO exhausted_jobs SELECT job_id, 'recovery', ?1 FROM jobs WHERE state NOT IN ('canceled','succeeded','partial','failed')",
            [now_ms],
        )
        .map_err(storage)?;
        Ok(())
    }

    /// Administrator reconciliation (offline maintenance): end the quarantine. Holds
    /// already placed (exhausted day, cooldowns, disabled schedules) stay in force
    /// until they expire or are changed explicitly.
    pub fn reconcile(&mut self) -> Result<bool, Refused> {
        let changed = self
            .conn
            .execute("DELETE FROM meta WHERE key='quarantine'", [])
            .map_err(storage)?;
        Ok(changed == 1)
    }

    /// Claim the next queued job fairly (contract §6: round-robin per principal): the
    /// principal whose most recent claim is oldest goes first, then its oldest job.
    /// `admit` rechecks the submitter's current grant; a refused job is blocked by
    /// policy instead of running.
    pub fn claim_next(
        &mut self,
        tenant: &str,
        owner: &str,
        now_ms: i64,
        admit: impl Fn(&Job) -> bool,
    ) -> Result<Option<Job>, Refused> {
        self.claim_next_of(tenant, owner, now_ms, None, &admit)
    }

    /// [`Control::claim_next`] restricted to `operations` (those this version can
    /// execute) inside the same fair query: a job of another operation stays queued and
    /// is never claimed or blocked.
    pub fn claim_next_of(
        &mut self,
        tenant: &str,
        owner: &str,
        now_ms: i64,
        operations: Option<&[String]>,
        admit: &dyn Fn(&Job) -> bool,
    ) -> Result<Option<Job>, Refused> {
        let ops = operations.map(|o| serde_json::to_string(o).expect("strings serialize"));
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        // A restored or recreated store runs nothing until it is reconciled (C10/C11),
        // whether or not its new epoch has been activated.
        if Self::quarantine_of(&tx)?.is_some() {
            return Ok(None);
        }
        // Nor before an Administrator activates its epoch, whichever of activation and
        // reconciliation happens first.
        let active: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM epochs WHERE state='active')",
                [],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if !active {
            return Ok(None);
        }
        // Queued jobs, and running jobs whose previous owner died (recovery released
        // their claim without changing state; they resume from their checkpoint).
        let candidate: Option<String> = tx
            .query_row(
                "WITH last AS (SELECT coalesce(submitter_sub,'') AS s, coalesce(submitter_client_id, service_actor) AS c, max(e.at_ms) AS at FROM jobs j JOIN job_events e USING(job_id) WHERE j.tenant=?1 AND e.to_state='running' GROUP BY s, c)
                 SELECT j.job_id FROM jobs j LEFT JOIN last l ON l.s=coalesce(j.submitter_sub,'') AND l.c=coalesce(j.submitter_client_id, j.service_actor)
                 WHERE j.tenant=?1 AND (j.state='queued' OR (j.state='running' AND j.claimed_by IS NULL)) AND (?2 IS NULL OR j.operation IN (SELECT value FROM json_each(?2)))
                 ORDER BY j.state='queued', coalesce(l.at,-1), j.submitted_ms, j.job_id LIMIT 1",
                params![tenant, ops],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        let Some(job_id) = candidate else {
            return Ok(None);
        };
        let job = Self::read_job(&tx, tenant, &job_id)?
            .ok_or_else(|| refused(ErrorCode::Conflict, "the job changed concurrently"))?;
        // Queued jobs take the §10 claim edge; an orphaned running job is already running.
        let mut state = job.state;
        if state == State::Queued {
            state = Self::record(&tx, &job_id, state, Trigger::Claim, "claimed", now_ms)?;
        }
        let exhausted: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM exhausted_jobs WHERE job_id=?1)",
                [&job_id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if exhausted {
            // Recovery left its consumption unknown: it never runs again (C11).
            Self::fail_with_gap(
                &tx,
                &job,
                state,
                "recovery left this job's consumption unknown; its budget is spent",
                Some("quota_exhausted"),
                now_ms,
            )?;
            tx.commit().map_err(storage)?;
            return Ok(None);
        }
        if job.deadline_ms <= now_ms {
            // A job past its deadline never runs further; it fails with its gap recorded.
            Self::fail_with_gap(
                &tx,
                &job,
                state,
                "deadline passed before the job could run",
                None,
                now_ms,
            )?;
            tx.commit().map_err(storage)?;
            return Ok(None);
        }
        if !admit(&job) {
            Self::record(
                &tx,
                &job_id,
                state,
                Trigger::PolicyBlock,
                "submitter grant no longer admits the job",
                now_ms,
            )?;
            tx.execute("UPDATE jobs SET claimed_by=NULL WHERE job_id=?1", [&job_id])
                .map_err(storage)?;
            tx.commit().map_err(storage)?;
            return Ok(None);
        }
        tx.execute(
            "UPDATE jobs SET claimed_by=?1, retry_after_ms=NULL, updated_ms=?3 WHERE job_id=?2",
            params![owner, job_id, now_ms],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(Some(Job {
            state: State::Running,
            updated_ms: now_ms,
            ..job
        }))
    }

    /// Fail `job` (currently `from`) for its deadline, atomically recording the fixed
    /// `result_failure` (with its consumed-but-uncertain sends) and the gap.
    fn fail_with_gap(
        tx: &Connection,
        job: &Job,
        from: State,
        gap: &str,
        code: Option<&'static str>,
        now_ms: i64,
    ) -> Result<(), Refused> {
        let trigger = if from == State::Running {
            Trigger::Fail
        } else {
            Trigger::DeadlinePassed
        };
        // The code names what kept the job from finishing (in time).
        let code = code.unwrap_or(match from {
            State::NeedsCredentials => "credential_blocked",
            State::WaitingRetry => "upstream_unavailable",
            _ => "overloaded",
        });
        Self::record(tx, &job.job_id, from, trigger, gap, now_ms)?;
        let uncertain: i64 = tx
            .query_row(
                "SELECT count(*) FROM send_reservations WHERE job_id=?1 AND state IN ('uncertain','reserved')",
                [&job.job_id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        let result = serde_json::json!({
            "operation": job.operation,
            "committed_facts": 0,
            "uncertain_sends": uncertain,
            // The reason names the cause (the gap's own text: a deadline, or recovery).
            "error": {"schema_version": 1, "code": code, "reason": gap},
        });
        tx.execute(
            "UPDATE jobs SET result_json=?2, gaps_json=?3, claimed_by=NULL WHERE job_id=?1",
            params![
                job.job_id,
                result.to_string(),
                serde_json::json!([gap]).to_string()
            ],
        )
        .map_err(storage)?;
        Ok(())
    }

    /// Record a terminal result with its edge, in one transaction; the claim ends.
    pub fn finish(
        &mut self,
        tenant: &str,
        job_id: &str,
        trigger: Trigger,
        result: &Value,
        now_ms: i64,
    ) -> Result<State, Refused> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let job = Self::read_job(&tx, tenant, job_id)?
            .ok_or_else(|| refused(ErrorCode::NotFound, "no such job"))?;
        if !transition(job.state, trigger).is_some_and(|s| s.terminal()) {
            return Err(refused(
                ErrorCode::Conflict,
                "the job's state does not allow this",
            ));
        }
        let to = Self::record(&tx, job_id, job.state, trigger, "finished", now_ms)?;
        tx.execute(
            "UPDATE jobs SET result_json=?1, claimed_by=NULL WHERE job_id=?2",
            params![result.to_string(), job_id],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(to)
    }

    /// Apply a non-terminal or cancellation edge that also ends the worker's claim.
    /// Park a job in `needs_credentials` (the §10 `credential_denied` edge), recording
    /// the credential `revision` it was denied under, in one transaction: only another
    /// revision resumes it ([`Control::credentials_restored`]).
    pub fn park_credentials(
        &mut self,
        tenant: &str,
        job_id: &str,
        revision: &str,
        now_ms: i64,
    ) -> Result<State, Refused> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let job = Self::read_job(&tx, tenant, job_id)?
            .ok_or_else(|| refused(ErrorCode::NotFound, "no such job"))?;
        let to = Self::record(
            &tx,
            job_id,
            job.state,
            Trigger::CredentialDenied,
            "credentials denied",
            now_ms,
        )?;
        tx.execute(
            "UPDATE jobs SET claimed_by=NULL, retry_after_ms=NULL, credential_revision=?2 WHERE job_id=?1",
            params![job_id, revision],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(to)
    }

    pub fn release(
        &mut self,
        tenant: &str,
        job_id: &str,
        trigger: Trigger,
        reason: &str,
        retry_at_ms: Option<i64>,
        now_ms: i64,
    ) -> Result<State, Refused> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let job = Self::read_job(&tx, tenant, job_id)?
            .ok_or_else(|| refused(ErrorCode::NotFound, "no such job"))?;
        let to = Self::record(&tx, job_id, job.state, trigger, reason, now_ms)?;
        tx.execute(
            "UPDATE jobs SET claimed_by=NULL, retry_after_ms=?1 WHERE job_id=?2",
            params![retry_at_ms, job_id],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(to)
    }

    /// The scheduler pass (contract §10): due retries requeue; waiting, blocked and
    /// credential-blocked jobs past their original deadline fail; a cancel-requested job
    /// no worker holds (it was never running, or its worker died) completes its
    /// cancellation, since it can make no further sends. Returns how many jobs changed.
    pub fn scheduler_pass(&mut self, tenant: &str, now_ms: i64) -> Result<u64, Refused> {
        let due: Vec<(String, String, Option<i64>, i64, bool)> = {
            let mut stmt = self
                .conn
                .prepare("SELECT job_id, state, retry_after_ms, deadline_ms, claimed_by IS NOT NULL FROM jobs WHERE tenant=?1 AND state IN ('waiting_retry','needs_credentials','blocked_policy','cancel_requested') ORDER BY job_id")
                .map_err(storage)?;
            stmt.query_map([tenant], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .map_err(storage)?
            .collect::<Result<_, _>>()
            .map_err(storage)?
        };
        let mut changed = 0;
        for (job_id, state, retry_at, deadline, held) in due {
            let trigger = match state.as_str() {
                "cancel_requested" if held => continue,
                "cancel_requested" => Trigger::CancelCheckpoint,
                _ if deadline <= now_ms => Trigger::DeadlinePassed,
                "waiting_retry" if retry_at.is_some_and(|t| t <= now_ms) => Trigger::RetryDue,
                _ => continue,
            };
            if trigger == Trigger::DeadlinePassed {
                let tx = self
                    .conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(storage)?;
                if let Some(job) = Self::read_job(&tx, tenant, &job_id)? {
                    Self::fail_with_gap(
                        &tx,
                        &job,
                        job.state,
                        "deadline passed while the job waited",
                        None,
                        now_ms,
                    )?;
                }
                tx.commit().map_err(storage)?;
            } else {
                self.release(tenant, &job_id, trigger, "scheduler pass", None, now_ms)?;
            }
            changed += 1;
        }
        Ok(changed)
    }

    /// Reconsider policy-blocked jobs (contract §10 `blocked_policy → queued`): a job
    /// within its original deadline whose blocking condition has cleared, as judged by
    /// `admit` under the current configuration, is requeued through the validated
    /// `PolicyCleared` edge, keeping its deadline and consumed sends. A job still
    /// blocked is left as it is, with no event. Nothing is reconsidered while the store
    /// is quarantined or has no active epoch. Returns how many were requeued.
    pub fn reconsider_blocked(
        &mut self,
        tenant: &str,
        now_ms: i64,
        admit: &dyn Fn(&Job) -> bool,
    ) -> Result<u64, Refused> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let active: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM epochs WHERE state='active')",
                [],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if Self::quarantine_of(&tx)?.is_some() || !active {
            return Ok(0);
        }
        let ids: Vec<String> = {
            let mut stmt = tx
                .prepare("SELECT job_id FROM jobs WHERE tenant=?1 AND state='blocked_policy' AND claimed_by IS NULL AND deadline_ms>?2 ORDER BY job_id")
                .map_err(storage)?;
            stmt.query_map(params![tenant, now_ms], |r| r.get(0))
                .map_err(storage)?
                .collect::<Result<Vec<String>, _>>()
                .map_err(storage)?
        };
        let mut requeued = 0;
        for id in ids {
            let Some(job) = Self::read_job(&tx, tenant, &id)? else {
                continue;
            };
            if admit(&job) {
                Self::record(
                    &tx,
                    &id,
                    State::BlockedPolicy,
                    Trigger::PolicyCleared,
                    "blocking condition cleared",
                    now_ms,
                )?;
                requeued += 1;
            }
        }
        tx.commit().map_err(storage)?;
        Ok(requeued)
    }

    /// Persist a checkpoint (and, for crash point C4, the archive `operation_id` it
    /// covers) before the next phase.
    pub fn checkpoint(
        &mut self,
        job_id: &str,
        phase: &str,
        state: &Value,
        archive_operation_id: Option<&str>,
        now_ms: i64,
    ) -> Result<i64, Refused> {
        let tx = self.conn.transaction().map_err(storage)?;
        let sequence: i64 = tx
            .query_row(
                "SELECT coalesce(max(sequence),-1)+1 FROM job_checkpoints WHERE job_id=?1",
                [job_id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        tx.execute(
            "INSERT INTO job_checkpoints VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                job_id,
                sequence,
                phase,
                state.to_string(),
                archive_operation_id,
                now_ms
            ],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(sequence)
    }

    /// The latest checkpoint of a job, if any.
    pub fn last_checkpoint(
        &self,
        job_id: &str,
    ) -> Result<Option<(String, Value, Option<String>)>, Refused> {
        self.conn
            .query_row(
                "SELECT phase,state_json,archive_operation_id FROM job_checkpoints WHERE job_id=?1 ORDER BY sequence DESC LIMIT 1",
                [job_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)),
            )
            .optional()
            .map_err(storage)
            .map(|row| row.map(|(p, s, o)| (p, serde_json::from_str(&s).unwrap_or(json!(null)), o)))
    }

    /// What the tenant's current Reports admission day has already spent (sends, IDs),
    /// read-only, by the same admission-day rule as [`Control::charge_reports`].
    pub fn reports_spent(&self, tenant: &str, now_ms: i64) -> Result<(u64, u64), Refused> {
        let tz = airs_runtime_core::schedule::report_tz().map_err(|_| {
            refused(
                ErrorCode::StorageUnavailable,
                "embedded timezone unavailable",
            )
        })?;
        let high: Option<i64> = self
            .conn
            .query_row(
                "SELECT high_water_ms FROM allowance_high_water WHERE tenant=?1",
                [tenant],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        let ts = |ms: i64| {
            jiff::Timestamp::from_millisecond(ms)
                .map_err(|_| refused(ErrorCode::InvalidRequest, "invalid time"))
        };
        let (day, _) =
            airs_runtime_core::schedule::admission_day(ts(now_ms)?, high.map(ts).transpose()?, &tz);
        let used: (i64, i64) = self
            .conn
            .query_row(
                "SELECT sends, ids FROM allowance_days WHERE tenant=?1 AND chicago_date=?2",
                params![tenant, day.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?
            .unwrap_or((0, 0));
        Ok((used.0.max(0) as u64, used.1.max(0) as u64))
    }

    /// Charge a Reports allowance (contract §6, §8) in one transaction: the admission day
    /// is the America/Chicago date of `max(now, stored high-water)`, so a clock rollback
    /// or a policy change cannot reopen an earlier day. A charge that would exceed the
    /// day's sends or IDs is `quota_exhausted` with the seconds until the next admission
    /// day; charges are never refunded (an uncertain send stays consumed).
    pub fn charge_reports(
        &mut self,
        tenant: &str,
        now_ms: i64,
        (sends, ids): (u64, u64),
        (cap_sends, cap_ids): (u64, u64),
    ) -> Result<String, Refused> {
        let tz = airs_runtime_core::schedule::report_tz().map_err(|_| {
            refused(
                ErrorCode::StorageUnavailable,
                "embedded timezone unavailable",
            )
        })?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let high: Option<i64> = tx
            .query_row(
                "SELECT high_water_ms FROM allowance_high_water WHERE tenant=?1",
                [tenant],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        let ts = |ms: i64| {
            jiff::Timestamp::from_millisecond(ms)
                .map_err(|_| refused(ErrorCode::InvalidRequest, "invalid time"))
        };
        let (day, effective) =
            airs_runtime_core::schedule::admission_day(ts(now_ms)?, high.map(ts).transpose()?, &tz);
        let date = day.to_string();
        let used: (i64, i64) = tx
            .query_row(
                "SELECT sends, ids FROM allowance_days WHERE tenant=?1 AND chicago_date=?2",
                params![tenant, date],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?
            .unwrap_or((0, 0));
        if used.0 as u64 + sends > cap_sends || used.1 as u64 + ids > cap_ids {
            let next = day
                .tomorrow()
                .ok()
                .and_then(|d| d.to_zoned(tz.clone()).ok())
                .map(|z| z.timestamp().as_millisecond())
                .unwrap_or(effective.as_millisecond() + 86_400_000);
            return Err(Refused {
                code: ErrorCode::QuotaExhausted,
                reason: "the daily Reports allowance is used up",
                retry_after_seconds: Some(((next - now_ms).max(0) as u64).div_ceil(1000)),
            });
        }
        tx.execute(
            "INSERT INTO allowance_days VALUES (?1,?2,?3,?4) ON CONFLICT(tenant, chicago_date) DO UPDATE SET sends=sends+excluded.sends, ids=ids+excluded.ids",
            params![tenant, date, sends as i64, ids as i64],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO allowance_high_water VALUES (?1,?2) ON CONFLICT(tenant) DO UPDATE SET high_water_ms=max(high_water_ms, excluded.high_water_ms)",
            params![tenant, effective.as_millisecond()],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(date)
    }

    /// Crash point C9: a consistent copy of the control store into `dir` (a private,
    /// not-yet-existing backup set directory), written by `VACUUM INTO` while this
    /// connection is held. The manifest is published last, so a set without
    /// `manifest.json` is incomplete and never listed as a backup.
    pub fn backup_into(&mut self, dir: &Path, now_ms: i64) -> Result<BackupManifest, Refused> {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(dir)
            .map_err(|_| {
                refused(
                    ErrorCode::StorageUnavailable,
                    "cannot create the backup set",
                )
            })?;
        let copy = dir.join(FILE);
        let target = copy
            .to_str()
            .ok_or_else(|| refused(ErrorCode::StorageUnavailable, "backup path is not UTF-8"))?;
        self.conn
            .execute("VACUUM INTO ?1", [target])
            .map_err(storage)?;
        // `VACUUM INTO` creates the copy under the process umask: make it owner-only
        // (§9.4) before anything is read back or the manifest is published.
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o600)).map_err(
                |_| {
                    refused(
                        ErrorCode::StorageUnavailable,
                        "cannot protect the backup copy",
                    )
                },
            )?;
        }
        let bytes = std::fs::read(&copy)
            .map_err(|_| refused(ErrorCode::StorageUnavailable, "cannot read the backup copy"))?;
        let (epoch, _) = self.active_epoch()?;
        let tenant: String = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key='service_tenant'",
                [],
                |r| r.get(0),
            )
            .map_err(storage)?;
        let manifest = BackupManifest {
            schema: SCHEMA_VERSION,
            tenant,
            epoch,
            created_ms: now_ms,
            bytes: bytes.len() as u64,
            sha256: hex(&Sha256::digest(&bytes)),
        };
        let text = serde_json::to_vec(&manifest).expect("manifest serializes");
        let tmp = dir.join(".manifest.json.tmp");
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp)
                .map_err(|_| refused(ErrorCode::StorageUnavailable, "cannot write the manifest"))?;
            f.write_all(&text)
                .and_then(|_| f.sync_all())
                .map_err(|_| refused(ErrorCode::StorageUnavailable, "cannot write the manifest"))?;
        }
        std::fs::File::open(&copy)
            .and_then(|f| f.sync_all())
            .map_err(|_| refused(ErrorCode::StorageUnavailable, "cannot sync the backup copy"))?;
        std::fs::rename(&tmp, dir.join(MANIFEST))
            .map_err(|_| refused(ErrorCode::StorageUnavailable, "cannot publish the manifest"))?;
        std::fs::File::open(dir)
            .and_then(|d| d.sync_all())
            .map_err(|_| refused(ErrorCode::StorageUnavailable, "cannot sync the backup set"))?;
        Ok(manifest)
    }

    /// Activate a recovery epoch that a restore left pending (an explicit
    /// Administrator action; contract C11). Idempotency keys of every other epoch stay
    /// `control_restored`.
    pub fn activate_epoch(&mut self, epoch: &str, now_ms: i64) -> Result<(), Refused> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(storage)?;
        let pending: bool = tx
            .query_row(
                "SELECT count(*) FROM epochs WHERE epoch=?1 AND state='pending_activation'",
                [epoch],
                |r| r.get::<_, i64>(0),
            )
            .map_err(storage)?
            == 1;
        if !pending {
            return Err(refused(ErrorCode::Conflict, "no such pending epoch"));
        }
        tx.execute("UPDATE epochs SET state='retired' WHERE state='active'", [])
            .map_err(storage)?;
        tx.execute(
            "UPDATE epochs SET state='active', activated_ms=?1 WHERE epoch=?2",
            params![now_ms, epoch],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }

    /// Jobs per state across the store (operational metrics; no tenant or principal
    /// data leaves this count).
    pub fn job_state_counts(&self) -> Result<Vec<(crate::jobs::State, u64)>, Refused> {
        let mut stmt = self
            .conn
            .prepare("SELECT state, count(*) FROM jobs GROUP BY state")
            .map_err(storage)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        Ok(rows
            .into_iter()
            .filter_map(|(s, n)| crate::jobs::State::parse(&s).map(|st| (st, n.max(0) as u64)))
            .collect())
    }

    /// The pending epoch a restore created, if any.
    pub fn pending_epoch(&self) -> Result<Option<String>, Refused> {
        self.conn
            .query_row(
                "SELECT epoch FROM epochs WHERE state='pending_activation' ORDER BY created_ms DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)
    }

    /// A tenant's jobs visible to one principal, newest first, after `before` (paging).
    /// A digest of the caller's whole filtered job set (identity, state and update
    /// time of every job), read in the same lock as the page, so a cursor can detect
    /// that the set changed between pages.
    pub fn list_digest(
        &self,
        tenant: &str,
        actor: &Actor,
        states: &[State],
    ) -> Result<String, Refused> {
        let states = serde_json::to_string(&states.iter().map(|s| s.name()).collect::<Vec<_>>())
            .map_err(|_| refused(ErrorCode::InvalidRequest, "invalid state filter"))?;
        let (sub, client) = match actor {
            Actor::Principal { sub, client_id } => (sub.clone(), client_id.clone()),
            Actor::Service(name) => (String::new(), name.clone()),
        };
        let mut stmt = self
            .conn
            .prepare("SELECT job_id, state, updated_ms FROM jobs WHERE tenant=?1 AND coalesce(submitter_sub,'')=?2 AND coalesce(submitter_client_id, service_actor)=?3 AND (json_array_length(?4)=0 OR state IN (SELECT value FROM json_each(?4))) ORDER BY submitted_ms DESC, job_id DESC")
            .map_err(storage)?;
        let mut hasher = Sha256::new();
        let rows = stmt
            .query_map(params![tenant, sub, client, states], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(storage)?;
        for row in rows {
            let (id, state, updated) = row.map_err(storage)?;
            hasher.update(format!("{id}\u{1f}{state}\u{1f}{updated}\u{1e}").as_bytes());
        }
        Ok(hex(&hasher.finalize()))
    }

    pub fn list(
        &self,
        tenant: &str,
        actor: &Actor,
        states: &[State],
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Job>, Refused> {
        let states = serde_json::to_string(&states.iter().map(|s| s.name()).collect::<Vec<_>>())
            .map_err(|_| refused(ErrorCode::InvalidRequest, "invalid state filter"))?;
        let ids: Vec<String> = {
            let (sub, client) = match actor {
                Actor::Principal { sub, client_id } => (sub.clone(), client_id.clone()),
                Actor::Service(name) => (String::new(), name.clone()),
            };
            let mut stmt = self
                .conn
                .prepare("SELECT job_id FROM jobs WHERE tenant=?1 AND coalesce(submitter_sub,'')=?2 AND coalesce(submitter_client_id, service_actor)=?3 AND (json_array_length(?6)=0 OR state IN (SELECT value FROM json_each(?6))) ORDER BY submitted_ms DESC, job_id DESC LIMIT ?4 OFFSET ?5")
                .map_err(storage)?;
            stmt.query_map(
                params![tenant, sub, client, limit as i64, offset as i64, states],
                |r| r.get(0),
            )
            .map_err(storage)?
            .collect::<Result<_, _>>()
            .map_err(storage)?
        };
        ids.iter()
            .filter_map(|id| self.job(tenant, id).transpose())
            .collect()
    }
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
