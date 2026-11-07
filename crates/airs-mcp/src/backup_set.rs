//! The coordinated backup set (contract §9.4 "Content"; M8 review 1, fork C gap).
//!
//! Taken offline, under the data subtree's exclusive owner lock (the service is
//! stopped), so control, archive and artifacts are mutually consistent:
//!
//! - `VACUUM INTO` copies of the control store and of every archive store;
//! - the artifact manifest list: every published report version's files, from the
//!   control store's version manifests;
//! - `manifest.json`, written last, after every copy is fsynced:
//!   `{format: "airs-mcp-backup/1", created_at, epoch, stores: [{name, schema,
//!   sqlite_version, bytes, sha256, rows_digest}], artifacts: [{id, sha256, bytes}],
//!   complete: true}`.
//!
//! The set is a new directory `<dir>/<id>/` (0700, never reused); every file is 0600.
//!
//! `rows_digest` is the SHA-256 of a deterministic serialization of the copy's rows:
//! for every table in `sqlite_master` (type `table`, not `sqlite_*`), in name order,
//! the table name and then each row, ordered by all of its columns, as one tagged value
//! per column (`n` null, `i` integer, `r` real (IEEE bits), `t` text, `b` blob, each
//! length-prefixed). It identifies logical content independently of page layout. Rows
//! are read through a SQLite cursor, one at a time, never collected.
//!
//! `schema` is the store's application schema: the control store's `meta` row
//! `schema` (its `user_version` is unused), and an archive's `PRAGMA user_version`.
//!
//! Nothing is suppressed (M8 review 2 finding 5). An archive that cannot be
//! enumerated, inspected or copied, or a report version whose manifest is not a
//! well-formed file list, fails the set, so `complete: true` is never written over a
//! missing store or artifact. Files are hashed through a fixed buffer, never read
//! whole (finding 6), so a set of multi-GiB stores fits the pod's memory limit.

use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use rusqlite::types::ValueRef;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const FORMAT: &str = "airs-mcp-backup/1";
pub const MANIFEST: &str = "manifest.json";

/// The fixed hashing buffer: files are streamed through it, never read whole.
pub const HASH_BUFFER: usize = 1024 * 1024;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn fail(reason: &'static str) -> crate::run::ServeError {
    crate::run::ServeError(reason)
}

fn sync_dir(dir: &Path) -> Result<(), crate::run::ServeError> {
    std::fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|_| fail("cannot sync the backup set"))
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The length and SHA-256 of everything `r` yields, read through `buf`. Its size bounds
/// the memory used, whatever the input's length.
pub fn hash_reader(mut r: impl Read, buf: &mut [u8]) -> std::io::Result<(u64, String)> {
    let mut h = Sha256::new();
    let mut n = 0u64;
    loop {
        match r.read(buf) {
            Ok(0) => return Ok((n, hex(&h.finalize()))),
            Ok(k) => {
                h.update(&buf[..k]);
                n += k as u64;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}

/// The length and SHA-256 of the file at `path`, streamed through [`HASH_BUFFER`].
fn hash_file(path: &Path) -> Result<(u64, String), crate::run::ServeError> {
    let f = std::fs::File::open(path).map_err(|_| fail("cannot read a backup copy"))?;
    let mut buf = vec![0u8; HASH_BUFFER];
    hash_reader(f, &mut buf).map_err(|_| fail("cannot read a backup copy"))
}

/// A store's application schema version (see the module documentation).
fn app_schema(conn: &rusqlite::Connection, name: &str) -> Result<i64, crate::run::ServeError> {
    if name == "control" {
        let v: String = conn
            .query_row("SELECT value FROM meta WHERE key='schema'", [], |r| {
                r.get(0)
            })
            .map_err(|_| fail("the control copy records no schema"))?;
        v.parse()
            .map_err(|_| fail("the control copy's schema is not a number"))
    } else {
        conn.query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|_| fail("cannot read a backup copy's schema"))
    }
}

/// The active epoch recorded in a control copy.
fn copy_epoch(control_copy: &Path) -> Result<String, crate::run::ServeError> {
    let conn = rusqlite::Connection::open_with_flags(
        control_copy,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|_| fail("cannot open the control copy"))?;
    conn.query_row("SELECT epoch FROM epochs WHERE state='active'", [], |r| {
        r.get(0)
    })
    .map_err(|_| fail("the control store has no active epoch"))
}

/// The artifacts one report version lists. Its manifest must be a JSON object whose
/// `files` is a list of `{name, sha256 (64 lowercase hex), bytes}`. Anything else is
/// refused, never skipped.
fn version_artifacts(version: &str, manifest: &str) -> Result<Vec<Value>, crate::run::ServeError> {
    let m: Value = serde_json::from_str(manifest)
        .map_err(|_| fail("a report version's manifest is not JSON"))?;
    let files = m["files"]
        .as_array()
        .ok_or_else(|| fail("a report version's manifest lists no files"))?;
    let mut out = Vec::with_capacity(files.len());
    for f in files {
        let (Some(name), Some(sha), Some(bytes)) = (
            f["name"].as_str(),
            f["sha256"].as_str(),
            f["bytes"].as_u64(),
        ) else {
            return Err(fail(
                "a report version's manifest has a malformed file entry",
            ));
        };
        if name.is_empty() || name.contains('/') || !is_hex64(sha) {
            return Err(fail(
                "a report version's manifest has a malformed file entry",
            ));
        }
        out.push(json!({"id": format!("{version}/{name}"), "sha256": sha, "bytes": bytes}));
    }
    Ok(out)
}

/// Every artifact the report versions of a control store (or copy) list, in version
/// order: the inventory a set must record. Creation writes it into the manifest, and
/// verification rebuilds it from the verified control copy and requires the manifest's
/// list to equal it exactly (M8 review 3).
fn control_artifacts(control: &Path) -> Result<Vec<Value>, crate::run::ServeError> {
    let conn =
        rusqlite::Connection::open_with_flags(control, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| fail("cannot open the control copy"))?;
    let mut artifacts = Vec::new();
    let mut stmt = conn
        .prepare("SELECT version_id, manifest_json FROM report_versions ORDER BY version_id")
        .map_err(|_| fail("cannot read the report versions"))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|_| fail("cannot read the report versions"))?;
    for row in rows {
        let (version, manifest) = row.map_err(|_| fail("cannot read the report versions"))?;
        artifacts.extend(version_artifacts(&version, &manifest)?);
    }
    Ok(artifacts)
}

/// `(id, sha256, bytes)` of each entry, sorted: a multiset comparison key.
fn artifact_keys(list: &[Value]) -> Vec<(String, String, u64)> {
    let mut keys: Vec<_> = list
        .iter()
        .map(|a| {
            (
                a["id"].as_str().unwrap_or_default().to_owned(),
                a["sha256"].as_str().unwrap_or_default().to_owned(),
                a["bytes"].as_u64().unwrap_or(u64::MAX),
            )
        })
        .collect();
    keys.sort();
    keys
}

/// The deterministic content digest of a SQLite file (see the module documentation).
pub fn rows_digest(path: &Path) -> Result<String, crate::run::ServeError> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| fail("cannot open a backup copy"))?;
    let mut tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .and_then(|mut s| {
            s.query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|_| fail("cannot read a backup copy's tables"))?;
    tables.sort();
    let mut h = Sha256::new();
    let field = |h: &mut Sha256, tag: u8, bytes: &[u8]| {
        h.update([tag]);
        h.update((bytes.len() as u64).to_be_bytes());
        h.update(bytes);
    };
    for t in tables {
        field(&mut h, b'T', t.as_bytes());
        let quoted = format!("\"{}\"", t.replace('"', "\"\""));
        let cols: usize = conn
            .prepare(&format!("SELECT * FROM {quoted} LIMIT 0"))
            .map(|s| s.column_count())
            .map_err(|_| fail("cannot read a backup copy's table"))?;
        let order: Vec<String> = (1..=cols).map(|i| i.to_string()).collect();
        let sql = if cols == 0 {
            format!("SELECT * FROM {quoted}")
        } else {
            format!("SELECT * FROM {quoted} ORDER BY {}", order.join(","))
        };
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|_| fail("cannot read a backup copy's rows"))?;
        let mut rows = stmt
            .query([])
            .map_err(|_| fail("cannot read a backup copy's rows"))?;
        while let Some(row) = rows
            .next()
            .map_err(|_| fail("cannot read a backup copy's rows"))?
        {
            h.update(b"R");
            for i in 0..cols {
                match row
                    .get_ref(i)
                    .map_err(|_| fail("cannot read a backup copy's rows"))?
                {
                    ValueRef::Null => field(&mut h, b'n', &[]),
                    ValueRef::Integer(v) => field(&mut h, b'i', &v.to_be_bytes()),
                    ValueRef::Real(v) => field(&mut h, b'r', &v.to_bits().to_be_bytes()),
                    ValueRef::Text(v) => field(&mut h, b't', v),
                    ValueRef::Blob(v) => field(&mut h, b'b', v),
                }
            }
        }
    }
    Ok(hex(&h.finalize()))
}

/// One store copy's manifest entry.
fn store_entry(name: &str, copy: &Path) -> Result<Value, crate::run::ServeError> {
    let (bytes, sha256) = hash_file(copy)?;
    let conn =
        rusqlite::Connection::open_with_flags(copy, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| fail("cannot open a backup copy"))?;
    let schema = app_schema(&conn, name)?;
    let sqlite_version: String = conn
        .query_row("SELECT sqlite_version()", [], |r| r.get(0))
        .map_err(|_| fail("cannot read the SQLite version"))?;
    drop(conn);
    Ok(json!({
        "name": name,
        "schema": schema,
        "sqlite_version": sqlite_version,
        "bytes": bytes,
        "sha256": sha256,
        "rows_digest": rows_digest(copy)?,
    }))
}

/// `VACUUM INTO` `source` as `<set>/<name>.sqlite3` (0600, fsynced).
fn copy_store(source: &Path, set: &Path, name: &str) -> Result<PathBuf, crate::run::ServeError> {
    let target = set.join(format!("{name}.sqlite3"));
    let conn = rusqlite::Connection::open_with_flags(
        source,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| fail("cannot open a store to back up"))?;
    let t = target
        .to_str()
        .ok_or_else(|| fail("backup path is not UTF-8"))?;
    conn.execute("VACUUM INTO ?1", [t])
        .map_err(|_| fail("cannot copy a store"))?;
    drop(conn);
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| fail("cannot protect a backup copy"))?;
    std::fs::File::open(&target)
        .and_then(|f| f.sync_all())
        .map_err(|_| fail("cannot sync a backup copy"))?;
    Ok(target)
}

/// The archive stores under `history`: `history/<cloud>/archive.sqlite3`, in name order.
/// Only an absent `history` (no archive yet), or a cloud directory without an archive,
/// is empty. Any error enumerating or inspecting them fails the set.
fn archives(history: &Path) -> Result<Vec<(String, PathBuf)>, crate::run::ServeError> {
    let entries = match std::fs::read_dir(history) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(fail("cannot enumerate the archive stores")),
    };
    let mut out = Vec::new();
    for e in entries {
        let e = e.map_err(|_| fail("cannot enumerate the archive stores"))?;
        let name = e
            .file_name()
            .into_string()
            .map_err(|_| fail("an archive directory name is not UTF-8"))?;
        if name.starts_with('.') {
            continue;
        }
        let kind = e
            .file_type()
            .map_err(|_| fail("cannot inspect an archive directory"))?;
        if !kind.is_dir() {
            continue;
        }
        let db = e.path().join("archive.sqlite3");
        match std::fs::symlink_metadata(&db) {
            Ok(m) if m.is_file() => out.push((name, db)),
            Ok(_) => return Err(fail("an archive store is not a regular file")),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(fail("cannot inspect an archive store")),
        }
    }
    out.sort();
    Ok(out)
}

/// Take the coordinated set into a new `<dir>/<id>/`. The caller holds the data
/// subtree's exclusive owner lock.
pub fn create(
    control_db: &Path,
    history: &Path,
    dir: &Path,
    now_ms: i64,
) -> Result<Value, crate::run::ServeError> {
    // Discover the archives before anything is written: an unreadable archive fails the
    // set rather than producing a set without it.
    let found = archives(history)?;
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && dir.is_dir() => {}
        Err(_) => return Err(fail("cannot create the backup directory")),
    }
    let id = format!(
        "b1-{}-{}",
        airs_runtime_core::time::iso(now_ms).replace([':', '-', '.'], ""),
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    );
    let set = dir.join(&id);
    // A new directory, never reused (no-clobber).
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&set)
        .map_err(|_| fail("the backup set directory already exists or cannot be created"))?;
    let mut stores = Vec::new();
    let control_copy = copy_store(control_db, &set, "control")?;
    stores.push(store_entry("control", &control_copy)?);
    for (cloud, db) in found {
        let name = format!("archive-{cloud}");
        let copy = copy_store(&db, &set, &name)?;
        stores.push(store_entry(&name, &copy)?);
    }
    // The epoch and the artifact list, from the control copy (consistent with it).
    let epoch = copy_epoch(&control_copy)?;
    let artifacts = control_artifacts(&control_copy)?;
    let manifest = json!({
        "format": FORMAT,
        "created_at": airs_runtime_core::time::iso(now_ms),
        "epoch": epoch,
        "stores": stores,
        "artifacts": artifacts,
        "complete": true,
    });
    // The manifest is written last, under a temporary name, then renamed into place.
    let tmp = set.join(".manifest.json.tmp");
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|_| fail("cannot write the manifest"))?;
        f.write_all(&serde_json::to_vec_pretty(&manifest).expect("manifest serializes"))
            .and_then(|_| f.sync_all())
            .map_err(|_| fail("cannot write the manifest"))?;
    }
    sync_dir(&set)?;
    std::fs::rename(&tmp, set.join(MANIFEST)).map_err(|_| fail("cannot publish the manifest"))?;
    sync_dir(&set)?;
    sync_dir(dir)?;
    Ok(
        json!({"event": "backup_set", "set": id, "epoch": manifest["epoch"],
        "stores": manifest["stores"].as_array().map_or(0, Vec::len),
        "artifacts": manifest["artifacts"].as_array().map_or(0, Vec::len)}),
    )
}

/// Whether `name` is exactly a set directory name [`create`] makes:
/// `b1-<YYYYMMDDTHHMMSSmmmZ>-<8 lowercase hex>` (the compact UTC creation time, then the
/// first 8 hex digits of a v4 UUID). Nothing else under a backups directory is ever read
/// as a set or removed (M8 review 3).
fn is_set_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("b1-") else {
        return false;
    };
    let b = rest.as_bytes();
    if b.len() != 19 + 1 + 8 {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| r.into_iter().all(|i| b[i].is_ascii_digit());
    digits(0..8)
        && b[8] == b'T'
        && digits(9..18)
        && b[18] == b'Z'
        && b[19] == b'-'
        && b[20..]
            .iter()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
}

/// What a set directory's manifest says. Creation writes the manifest last and only ever
/// with `complete: true`, so there are exactly two states retention may act on: a
/// complete manifest, and no manifest at all (an interrupted run). Any manifest that is
/// present but not exactly a complete one of this format, whatever the reason (a damaged
/// published manifest, a foreign file, a wrong type), is uncertain, and retention never
/// decides about it (M8 reviews 3 and 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SetState {
    /// A manifest of this format with the full field set and `complete` exactly `true`.
    Complete,
    /// No manifest file at all: an interrupted run (the manifest is written last).
    Missing,
    /// A manifest that exists but is not a complete manifest of this format: unreadable,
    /// not JSON, another format, a missing or wrongly typed field, or `complete` that is
    /// missing, `null`, `false`, a string or anything but the boolean `true`.
    Uncertain,
}

/// Whether `m` is a complete manifest of this format with the full field set (the
/// structure [`verify`] establishes completion from).
fn is_complete_manifest(m: &Value) -> bool {
    m["format"] == FORMAT
        && m["complete"].as_bool() == Some(true)
        && m["created_at"].is_string()
        && m["epoch"].is_string()
        && m["stores"].is_array()
        && m["artifacts"].is_array()
}

fn set_state(set: &Path) -> SetState {
    let path = set.join(MANIFEST);
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return SetState::Missing,
        Err(_) => return SetState::Uncertain,
        Ok(m) if !m.is_file() => return SetState::Uncertain,
        Ok(_) => {}
    }
    let Some(m) = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
    else {
        return SetState::Uncertain;
    };
    if is_complete_manifest(&m) {
        SetState::Complete
    } else {
        SetState::Uncertain
    }
}

/// The set directories directly under `dir` (real directories with an exact set name,
/// never followed symlinks), sorted oldest first (the names sort by creation time),
/// each with its manifest state.
fn sets_in(dir: &Path) -> Result<Vec<(PathBuf, SetState)>, crate::run::ServeError> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|_| fail("cannot list the backups directory"))? {
        let entry = entry.map_err(|_| fail("cannot list the backups directory"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str().filter(|n| is_set_name(n)) else {
            continue;
        };
        let meta = std::fs::symlink_metadata(entry.path())
            .map_err(|_| fail("cannot inspect a backup set"))?;
        if !meta.is_dir() {
            continue;
        }
        let path = dir.join(name);
        let state = set_state(&path);
        out.push((path, state));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// The newest complete set under `dir`, for a verifier that cannot be told the new set's
/// ID (the backup Job's distroless image has no shell). An absent complete set refuses,
/// and so does an uncertain set newer than it (it could be the newest real set, damaged).
pub fn latest_complete(dir: &Path) -> Result<PathBuf, crate::run::ServeError> {
    for (path, state) in sets_in(dir)?.into_iter().rev() {
        match state {
            SetState::Complete => return Ok(path),
            SetState::Uncertain => {
                return Err(fail(
                    "a newer backup set has an unreadable, damaged or foreign manifest",
                ));
            }
            SetState::Missing => {}
        }
    }
    Err(fail("no complete backup set in the directory"))
}

/// On-volume retention (runbook §9a). After `newest` has just been verified, keep the
/// `keep` newest complete sets (always including `newest`). Remove older complete sets,
/// and interrupted runs (no manifest at all) older than `newest`. Only exact set names
/// that are real directories directly under `dir` are touched, and nothing newer than
/// `newest`. If any set has a manifest that is not exactly a complete one of this format
/// (unreadable, corrupt, foreign, a missing field, or `complete` missing, null, false or
/// not a boolean), nothing is removed and retention refuses: a damaged published manifest
/// is not an interrupted run (M8 reviews 3 and 4).
/// Off-volume copies are never touched here. Returns the removed set names.
pub fn prune(
    dir: &Path,
    keep: usize,
    newest: &Path,
) -> Result<Vec<String>, crate::run::ServeError> {
    if keep == 0 {
        return Err(fail("--keep must be at least 1"));
    }
    let sets = sets_in(dir)?;
    if !sets
        .iter()
        .any(|(p, state)| p == newest && *state == SetState::Complete)
    {
        return Err(fail(
            "the newest set is not a complete set in this directory",
        ));
    }
    if sets.iter().any(|(_, state)| *state == SetState::Uncertain) {
        return Err(fail(
            "a backup set has an unreadable, damaged or foreign manifest; nothing was removed",
        ));
    }
    let mut kept = 0;
    let mut doomed = Vec::new();
    for (path, state) in sets.iter().rev() {
        if path.as_path() > newest {
            continue;
        }
        let remove = match state {
            SetState::Complete => {
                kept += 1;
                kept > keep
            }
            SetState::Missing => path != newest,
            SetState::Uncertain => false,
        };
        if remove {
            doomed.push(path.clone());
        }
    }
    let mut removed = Vec::new();
    for path in doomed {
        std::fs::remove_dir_all(&path).map_err(|_| fail("cannot remove an old backup set"))?;
        removed.push(
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
    }
    sync_dir(dir)?;
    removed.sort();
    Ok(removed)
}

/// Whether `v` is a well-formed artifact list: `[{id: "<version>/<file>", sha256: 64 hex,
/// bytes}]`, with no other keys.
fn artifacts_ok(v: &Value) -> bool {
    v.as_array().is_some_and(|a| {
        a.iter().all(|e| {
            e.as_object().is_some_and(|o| o.len() == 3)
                && e["id"].as_str().is_some_and(|id| {
                    id.split_once('/')
                        .is_some_and(|(v, f)| !v.is_empty() && !f.is_empty() && !f.contains('/'))
                })
                && e["sha256"].as_str().is_some_and(is_hex64)
                && e["bytes"].as_u64().is_some()
        })
    })
}

/// Whether `v` looks like a SQLite version (`3.45.1`).
fn sqlite_version_ok(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() >= 2
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Verify a set:
/// - a complete manifest of the right format;
/// - every store copy present (0600), with its recorded size, SHA-256, rows digest and
///   application schema;
/// - an epoch that is the control copy's active epoch;
/// - a well-formed artifact list;
/// - no other file present.
pub fn verify(set: &Path) -> Result<Value, crate::run::ServeError> {
    let text = std::fs::read(set.join(MANIFEST)).map_err(|_| fail("the set has no manifest"))?;
    let m: Value = serde_json::from_slice(&text).map_err(|_| fail("the manifest is not JSON"))?;
    if m["format"] != FORMAT {
        return Err(fail("the manifest has another format"));
    }
    if m["complete"] != true {
        return Err(fail("the set is not complete"));
    }
    if m["created_at"]
        .as_str()
        .and_then(airs_runtime_core::dto::utc_ms)
        .is_none()
    {
        return Err(fail("the manifest has no valid creation time"));
    }
    if !artifacts_ok(&m["artifacts"]) {
        return Err(fail("the manifest's artifact list is malformed"));
    }
    let stores = m["stores"]
        .as_array()
        .ok_or_else(|| fail("the manifest lists no stores"))?;
    if stores.iter().filter(|s| s["name"] == "control").count() != 1 {
        return Err(fail("the set has no control store"));
    }
    let mut expected = vec![MANIFEST.to_owned()];
    for s in stores {
        let name = s["name"]
            .as_str()
            .ok_or_else(|| fail("a store has no name"))?;
        let valid_name = name == "control"
            || name.strip_prefix("archive-").is_some_and(|c| {
                !c.is_empty()
                    && c.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            });
        if !valid_name {
            return Err(fail("a store name is not a file name"));
        }
        let file = format!("{name}.sqlite3");
        if expected.contains(&file) {
            return Err(fail("a store is listed twice"));
        }
        let path = set.join(&file);
        let meta = std::fs::symlink_metadata(&path).map_err(|_| fail("a store copy is missing"))?;
        if !meta.is_file() || meta.permissions().mode() & 0o777 != 0o600 {
            return Err(fail("a store copy is not a private regular file"));
        }
        let (bytes, sha256) = hash_file(&path)?;
        if Some(bytes) != s["bytes"].as_u64() || s["sha256"].as_str() != Some(sha256.as_str()) {
            return Err(fail("a store copy does not match its manifest"));
        }
        if s["rows_digest"].as_str() != Some(rows_digest(&path)?.as_str()) {
            return Err(fail("a store copy's rows do not match its manifest"));
        }
        let conn = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|_| fail("cannot open a store copy"))?;
        let schema = app_schema(&conn, name)?;
        drop(conn);
        if s["schema"].as_i64() != Some(schema) {
            return Err(fail("a store copy's schema does not match its manifest"));
        }
        if !s["sqlite_version"].as_str().is_some_and(sqlite_version_ok) {
            return Err(fail("a store's SQLite version is malformed"));
        }
        if name == "control" && m["epoch"].as_str() != Some(copy_epoch(&path)?.as_str()) {
            return Err(fail("the manifest's epoch is not the control copy's epoch"));
        }
        // The artifact inventory must be exactly what the verified control copy lists:
        // the same IDs, hashes, lengths and multiplicity, in any order.
        if name == "control" {
            let expected_artifacts = control_artifacts(&path)?;
            let listed = m["artifacts"].as_array().cloned().unwrap_or_default();
            if artifact_keys(&listed) != artifact_keys(&expected_artifacts) {
                return Err(fail(
                    "the manifest's artifact list is not the control copy's inventory",
                ));
            }
        }
        expected.push(file);
    }
    for e in std::fs::read_dir(set).map_err(|_| fail("cannot list the set"))? {
        let e = e.map_err(|_| fail("cannot list the set"))?;
        let n = e.file_name().to_string_lossy().into_owned();
        if !expected.contains(&n) {
            return Err(fail("the set holds an unlisted file"));
        }
    }
    Ok(json!({"event": "backup_set_verified", "epoch": m["epoch"],
        "stores": stores.len(), "artifacts": m["artifacts"].as_array().map_or(0, Vec::len)}))
}

#[cfg(test)]
mod tests {
    /// A set directory with a full-field manifest. `complete: false` is never written by
    /// creation, so a set made with `false` is an *uncertain* one, not an interrupted run.
    fn set_dir(root: &Path, name: &str, complete: bool) -> PathBuf {
        let d = root.join(name);
        std::fs::DirBuilder::new().mode(0o700).create(&d).unwrap();
        std::fs::write(
            d.join(MANIFEST),
            serde_json::to_vec(&full_manifest(json!(complete))).unwrap(),
        )
        .unwrap();
        d
    }

    /// An interrupted run: the directory exists and the manifest was never written.
    fn interrupted_dir(root: &Path, name: &str) -> PathBuf {
        let d = root.join(name);
        std::fs::DirBuilder::new().mode(0o700).create(&d).unwrap();
        d
    }

    fn full_manifest(complete: Value) -> Value {
        json!({"format": FORMAT, "created_at": "2026-11-07T03:00:00.000Z", "epoch": "0a1b2c3d",
            "stores": [], "artifacts": [], "complete": complete})
    }

    /// The newest complete set is found by name order (creation time); incomplete
    /// sets, non-set names and symlinks are never chosen, and an empty directory refuses.
    #[test]
    fn the_latest_complete_set_is_found_and_absence_refuses() {
        let root = std::env::temp_dir().join(format!("airs-bs-latest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        assert!(latest_complete(&root).is_err(), "no set");
        set_dir(&root, "b1-20261101T030000000Z-aaaaaaaa", true);
        let newest = set_dir(&root, "b1-20261102T030000000Z-bbbbbbbb", true);
        interrupted_dir(&root, "b1-20261103T030000000Z-cccccccc");
        set_dir(&root, "zz-not-a-set", true);
        std::os::unix::fs::symlink(&newest, root.join("b1-20261109T030000000Z-dddddddd")).unwrap();
        assert_eq!(latest_complete(&root).unwrap(), newest);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// M8 review 3: only exact generated names are sets, and a manifest that exists
    /// but is unreadable, corrupt or foreign blocks retention entirely. An unrelated
    /// `b1-…` directory is never touched, a verified set whose manifest became unreadable
    /// is never deleted, and an interrupted run (no manifest) is still removable.
    #[test]
    fn retention_fails_closed_on_uncertain_sets_and_ignores_foreign_names() {
        assert!(is_set_name("b1-20261107T030000000Z-0a1b2c3d"));
        for bad in [
            "b1-000-not-a-backup",
            "b1-20261107T030000000Z-0A1B2C3D",
            "b1-20261107T030000000-0a1b2c3d",
            "b1-20261107T03000000Z-0a1b2c3d",
            "b1-20261107T030000000Z-0a1b2c3",
            "b1-20261107X030000000Z-0a1b2c3d",
            "b2-20261107T030000000Z-0a1b2c3d",
        ] {
            assert!(!is_set_name(bad), "{bad}");
        }
        let root = std::env::temp_dir().join(format!("airs-bs-unsure-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let foreign = set_dir(&root, "b1-000-not-a-backup", true);
        let interrupted = interrupted_dir(&root, "b1-20261101T030000000Z-00000001");
        let damaged = set_dir(&root, "b1-20261102T030000000Z-00000002", true);
        std::fs::write(damaged.join(MANIFEST), b"{not json").unwrap();
        let newest = set_dir(&root, "b1-20261103T030000000Z-00000003", true);
        assert!(
            prune(&root, 3, &newest).is_err(),
            "a corrupt manifest blocks retention"
        );
        assert!(
            damaged.exists() && interrupted.exists() && foreign.exists(),
            "nothing removed"
        );
        // An unreadable manifest (a directory where the file should be) is uncertain too.
        std::fs::remove_file(damaged.join(MANIFEST)).unwrap();
        std::fs::create_dir(damaged.join(MANIFEST)).unwrap();
        assert!(prune(&root, 3, &newest).is_err());
        assert!(damaged.exists());
        // A foreign format is uncertain.
        std::fs::remove_dir(damaged.join(MANIFEST)).unwrap();
        std::fs::write(
            damaged.join(MANIFEST),
            br#"{"format":"other/1","complete":true}"#,
        )
        .unwrap();
        assert!(prune(&root, 3, &newest).is_err());
        // latest_complete refuses when a newer set is uncertain.
        let newer_damaged = set_dir(&root, "b1-20261104T030000000Z-00000004", true);
        std::fs::write(newer_damaged.join(MANIFEST), b"").unwrap();
        assert!(latest_complete(&root).is_err());
        std::fs::remove_dir_all(&newer_damaged).unwrap();
        assert_eq!(latest_complete(&root).unwrap(), newest);
        // M8 review 4 finding 2: a recognized-format manifest whose completion is not
        // exactly `true` is a damaged published manifest, never an interrupted run. Each
        // variant blocks retention and nothing is removed.
        for (what, m) in [
            ("complete removed", {
                let mut m = full_manifest(json!(true));
                m.as_object_mut().unwrap().remove("complete");
                m
            }),
            ("complete null", full_manifest(Value::Null)),
            ("complete a string", full_manifest(json!("true"))),
            ("complete false", full_manifest(json!(false))),
            ("complete 1", full_manifest(json!(1))),
            ("stores missing", {
                let mut m = full_manifest(json!(true));
                m.as_object_mut().unwrap().remove("stores");
                m
            }),
        ] {
            std::fs::write(damaged.join(MANIFEST), serde_json::to_vec(&m).unwrap()).unwrap();
            assert!(prune(&root, 3, &newest).is_err(), "{what} blocks retention");
            assert!(
                damaged.exists() && interrupted.exists() && foreign.exists(),
                "{what}: nothing removed"
            );
            assert!(
                latest_complete(&root).is_ok(),
                "{what}: an older uncertain set does not hide the newest"
            );
        }
        // With the damage repaired, the interrupted run goes and the foreign name stays.
        std::fs::remove_dir_all(&damaged).unwrap();
        let removed = prune(&root, 3, &newest).unwrap();
        assert_eq!(removed, ["b1-20261101T030000000Z-00000001"]);
        assert!(
            foreign.exists(),
            "an unrelated b1- directory is never touched"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// Retention keeps the newest N complete sets including the just-verified one,
    /// removes older complete sets and interrupted (incomplete) older runs, and never
    /// touches anything newer, any non-set name or a symlink's target.
    #[test]
    fn retention_keeps_the_newest_verified_sets_only() {
        let root = std::env::temp_dir().join(format!("airs-bs-prune-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let outside =
            std::env::temp_dir().join(format!("airs-bs-outside-{}", uuid::Uuid::new_v4()));
        let target = set_dir(
            outside.parent().unwrap(),
            outside.file_name().unwrap().to_str().unwrap(),
            true,
        );
        let names = [
            ("b1-20261101T030000000Z-00000001", true),
            ("b1-20261102T030000000Z-00000002", false),
            ("b1-20261103T030000000Z-00000003", true),
            ("b1-20261104T030000000Z-00000004", true),
            ("b1-20261105T030000000Z-00000005", true),
            ("b1-20261106T030000000Z-00000006", false),
        ];
        for (n, c) in names {
            if c {
                set_dir(&root, n, true);
            } else {
                interrupted_dir(&root, n);
            }
        }
        set_dir(&root, "keep-me", true);
        std::os::unix::fs::symlink(&target, root.join("b1-20261100T030000000Z-0000000f")).unwrap();
        let newest = root.join("b1-20261105T030000000Z-00000005");
        assert!(prune(&root, 0, &newest).is_err(), "keep 0 refuses");
        assert!(
            prune(&root, 3, &root.join("b1-20261106T030000000Z-00000006")).is_err(),
            "an incomplete newest refuses"
        );
        let removed = prune(&root, 3, &newest).unwrap();
        assert_eq!(
            removed,
            [
                "b1-20261101T030000000Z-00000001",
                "b1-20261102T030000000Z-00000002"
            ]
        );
        for kept in [
            "b1-20261103T030000000Z-00000003",
            "b1-20261104T030000000Z-00000004",
            "b1-20261105T030000000Z-00000005",
            "b1-20261106T030000000Z-00000006",
            "keep-me",
        ] {
            assert!(root.join(kept).exists(), "{kept} kept");
        }
        assert!(
            target.join(MANIFEST).exists(),
            "a symlink's target is never removed"
        );
        std::fs::remove_dir_all(&root).unwrap();
        std::fs::remove_dir_all(&target).unwrap();
    }

    use super::*;

    /// A reader that records the largest read it was asked for.
    struct Counting<R> {
        inner: R,
        largest: usize,
    }

    impl<R: Read> Read for Counting<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.largest = self.largest.max(buf.len());
            self.inner.read(buf)
        }
    }

    /// M8 review 2 finding 6: hashing streams through the fixed buffer. An input many
    /// times the buffer's size is hashed correctly, and no read asks for more than it.
    #[test]
    fn hashing_streams_through_a_bounded_buffer() {
        let data: Vec<u8> = (0..(5 * 64 * 1024 + 17)).map(|i| (i % 251) as u8).collect();
        let mut buf = vec![0u8; 64 * 1024];
        let mut r = Counting {
            inner: std::io::Cursor::new(&data),
            largest: 0,
        };
        let (n, sha) = hash_reader(&mut r, &mut buf).unwrap();
        assert_eq!(n, data.len() as u64);
        assert_eq!(sha, hex(&Sha256::digest(&data)));
        assert!(r.largest <= 64 * 1024, "{}", r.largest);
    }

    /// M8 review 2 finding 5: a report version's manifest is a strict file list.
    #[test]
    fn version_artifacts_are_strict() {
        let sha = "a".repeat(64);
        let ok = format!(r#"{{"files":[{{"name":"report.html","sha256":"{sha}","bytes":3}}]}}"#);
        assert_eq!(version_artifacts("v1_x", &ok).unwrap().len(), 1);
        for bad in [
            "not json".to_owned(),
            r#"{"nofiles":[]}"#.to_owned(),
            r#"{"files":[{"name":"report.html","sha256":"aaaa","bytes":3}]}"#.to_owned(),
            format!(r#"{{"files":[{{"name":"a/b","sha256":"{sha}","bytes":3}}]}}"#),
            format!(r#"{{"files":[{{"name":"r","sha256":"{sha}"}}]}}"#),
        ] {
            assert!(version_artifacts("v1_x", &bad).is_err(), "{bad}");
        }
    }

    /// An unreadable history directory fails discovery; an absent one is empty.
    #[test]
    fn archive_discovery_propagates_errors() {
        let base = std::env::temp_dir().join(format!("airs-set-arch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&base).unwrap();
        assert!(archives(&base.join("absent")).unwrap().is_empty());
        let h = base.join("history");
        std::fs::create_dir(&h).unwrap();
        std::fs::create_dir(h.join("cloud")).unwrap();
        std::fs::write(h.join("cloud").join("archive.sqlite3"), b"x").unwrap();
        assert_eq!(archives(&h).unwrap().len(), 1);
        // An archive path that is a directory, not a file, is refused.
        std::fs::create_dir(h.join("odd")).unwrap();
        std::fs::create_dir(h.join("odd").join("archive.sqlite3")).unwrap();
        assert!(archives(&h).is_err());
        std::fs::remove_dir(h.join("odd").join("archive.sqlite3")).unwrap();
        // An unreadable history directory is an error, not an empty set.
        std::fs::set_permissions(&h, std::fs::Permissions::from_mode(0o000)).unwrap();
        let unreadable = archives(&h);
        std::fs::set_permissions(&h, std::fs::Permissions::from_mode(0o700)).unwrap();
        // Root ignores permission bits; only an unprivileged run can observe this.
        if rustix::process::geteuid().as_raw() != 0 {
            assert!(unreadable.is_err());
        }
        std::fs::remove_dir_all(&base).unwrap();
    }
}
