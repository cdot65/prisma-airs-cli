//! Storage feasibility (contract §9): private-subtree admission on the real mount
//! shape, filesystem classification, single-owner lock and a synthetic SQLite
//! backup/restore drill. No customer history ever enters this binary.

use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use rustix::fs::{FlockOperation, flock};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageError {
    NotAbsolute,
    Missing,
    NotPrivateDirectory,
    UnsafeAncestor,
    NetworkFilesystem,
    UnknownFilesystem,
    OwnedByAnotherProcess,
    Io,
    Integrity,
    ManifestMismatch,
    ContentMismatch,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotAbsolute => "data directory must be absolute",
            Self::Missing => "data directory does not exist (the init step creates it)",
            Self::NotPrivateDirectory => {
                "data directory must be a real 0700 directory owned by this UID"
            }
            Self::UnsafeAncestor => {
                "a data directory ancestor is group/other writable or foreign-owned"
            }
            Self::NetworkFilesystem => "data directory is on a network filesystem",
            Self::UnknownFilesystem => "cannot classify the data directory filesystem",
            Self::OwnedByAnotherProcess => "another process owns this data directory",
            Self::Io => "storage I/O failed",
            Self::Integrity => "database integrity check failed",
            Self::ManifestMismatch => "backup manifest does not match its files",
            Self::ContentMismatch => "restored rows do not carry the seeded marker data",
        })
    }
}

/// Admitted private data directory, with its filesystem classification.
#[derive(Debug, Clone, Serialize)]
pub struct Admitted {
    #[serde(skip)]
    pub path: PathBuf,
    pub filesystem: &'static str,
    pub uid: u32,
}

fn euid() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// Mirrors the CLI's history admission (src/history/paths.rs) without creating
/// anything: the constrained init step owns creation (contract §9.3).
pub fn admit(path: &Path) -> Result<Admitted, StorageError> {
    if !path.is_absolute() {
        return Err(StorageError::NotAbsolute);
    }
    let m = fs::symlink_metadata(path).map_err(|_| StorageError::Missing)?;
    if !m.is_dir() || m.uid() != euid() || m.mode() & 0o077 != 0 {
        return Err(StorageError::NotPrivateDirectory);
    }
    let canonical = path.canonicalize().map_err(|_| StorageError::Io)?;
    if canonical != path {
        return Err(StorageError::NotPrivateDirectory);
    }
    for ancestor in canonical.ancestors().skip(1) {
        let a = fs::symlink_metadata(ancestor).map_err(|_| StorageError::UnsafeAncestor)?;
        let sticky_root = a.uid() == 0 && a.mode() & 0o1000 != 0;
        if !a.is_dir()
            || (a.uid() != 0 && a.uid() != euid())
            || (a.mode() & 0o022 != 0 && !sticky_root)
        {
            return Err(StorageError::UnsafeAncestor);
        }
    }
    let dir = File::open(path).map_err(|_| StorageError::Io)?;
    let stat = rustix::fs::fstatfs(&dir).map_err(|_| StorageError::UnknownFilesystem)?;
    let filesystem = classify(stat.f_type as i64)?;
    Ok(Admitted {
        path: canonical,
        filesystem,
        uid: euid(),
    })
}

fn classify(magic: i64) -> Result<&'static str, StorageError> {
    match magic {
        0xEF53 => Ok("ext4"),
        0x5846_5342 => Ok("xfs"),
        0x9123_683E => Ok("btrfs"),
        0x0102_1994 => Ok("tmpfs"), // tests and dry runs only; never production
        0x794C_7630 => Ok("overlayfs"),
        0x6969 | 0xFF53_4D42 | 0xFE53_4D42 | 0x517B | 0x6573_5546 => {
            Err(StorageError::NetworkFilesystem)
        }
        _ => Err(StorageError::UnknownFilesystem),
    }
}

/// The exclusive owner lock plus a published owner record. Dropping it releases
/// the lock. A local `flock` is NOT distributed fencing: it proves exclusion
/// only between processes sharing this kernel. Cross-node exclusion comes from
/// ReadWriteOncePod attachment, demonstrated separately (contract §9.2).
pub struct Owner {
    _lock: File,
    pub record: OwnerRecord,
}

#[derive(Debug, Clone, Serialize)]
pub struct OwnerRecord {
    pub owner_id: String,
    pub pod: String,
    pub node: String,
    pub pid: u32,
    pub acquired_at_unix: u64,
}

pub fn acquire(admitted: &Admitted) -> Result<Owner, StorageError> {
    let lock_path = admitted.path.join("owner.lock");
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(&lock_path)
        .map_err(|_| StorageError::Io)?;
    let m = lock.metadata().map_err(|_| StorageError::Io)?;
    if !m.is_file() || m.uid() != euid() || m.mode() & 0o077 != 0 || m.nlink() != 1 {
        return Err(StorageError::NotPrivateDirectory);
    }
    flock(&lock, FlockOperation::NonBlockingLockExclusive)
        .map_err(|_| StorageError::OwnedByAnotherProcess)?;
    // Pod/node names are non-secret downward-API metadata for the receipt.
    let record = OwnerRecord {
        owner_id: uuid::Uuid::new_v4().to_string(),
        pod: std::env::var("POD_NAME").unwrap_or_else(|_| "unknown".into()),
        node: std::env::var("NODE_NAME").unwrap_or_else(|_| "unknown".into()),
        pid: std::process::id(),
        acquired_at_unix: now(),
    };
    let bytes = serde_json::to_vec_pretty(&record).map_err(|_| StorageError::Io)?;
    publish_private(&admitted.path, "owner.json", &bytes, true)?;
    Ok(Owner {
        _lock: lock,
        record,
    })
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Private temp + fsync + rename (+ replace when allowed) + directory fsync.
pub fn publish_private(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    replace: bool,
) -> Result<(), StorageError> {
    let tmp = dir.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4()));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|_| StorageError::Io)?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|_| StorageError::Io)?;
    drop(f);
    let target = dir.join(name);
    let result = if replace {
        fs::rename(&tmp, &target).map_err(|_| StorageError::Io)
    } else {
        // No-clobber: hard-link then unlink the temp name.
        fs::hard_link(&tmp, &target)
            .map_err(|_| StorageError::Io)
            .and_then(|_| fs::remove_file(&tmp).map_err(|_| StorageError::Io))
    };
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
        return result;
    }
    File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|_| StorageError::Io)
}

/// Create (if absent) and admit a private 0700 child of an admitted directory.
/// Used for `backups/` so the drill runs in a shell-less distroless image.
pub fn private_child(parent: &Admitted, name: &str) -> Result<Admitted, StorageError> {
    if name.contains('/') || name.starts_with('.') || name.is_empty() {
        return Err(StorageError::Io);
    }
    let path = parent.path.join(name);
    match fs::DirBuilder::new().mode(0o700).create(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(StorageError::Io),
    }
    admit(&path)
}

// ---------------------------------------------------------------------------
// Synthetic backup/restore drill (contract §9.5). The marker lets the runbook
// check that the off-volume backup target holds no plaintext copy.

pub const DRILL_DB: &str = "drill.sqlite3";

#[derive(Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: String,
    pub created_at_unix: u64,
    pub sqlite_version: String,
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
    pub rows: i64,
    pub marker_sha256: String,
}

fn connect(path: &Path, flags: rusqlite::OpenFlags) -> Result<rusqlite::Connection, StorageError> {
    let c = rusqlite::Connection::open_with_flags(path, flags).map_err(|_| StorageError::Io)?;
    c.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA secure_delete=ON; PRAGMA trusted_schema=OFF;")
        .map_err(|_| StorageError::Io)?;
    Ok(c)
}

pub fn drill_seed(admitted: &Admitted, rows: u32, marker: &str) -> Result<(), StorageError> {
    let path = admitted.path.join(DRILL_DB);
    if path.exists() {
        return Err(StorageError::Io);
    }
    File::options()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|_| StorageError::Io)?;
    let mut c = connect(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let tx = c.transaction().map_err(|_| StorageError::Io)?;
    tx.execute_batch("CREATE TABLE drill(id INTEGER PRIMARY KEY, body TEXT NOT NULL) STRICT;")
        .map_err(|_| StorageError::Io)?;
    for i in 0..rows {
        tx.execute(
            "INSERT INTO drill(id, body) VALUES (?1, ?2)",
            rusqlite::params![i, format!("{marker}:{i}")],
        )
        .map_err(|_| StorageError::Io)?;
    }
    tx.commit().map_err(|_| StorageError::Io)
}

fn sha256_file(path: &Path) -> Result<(u64, String), StorageError> {
    let bytes = fs::read(path).map_err(|_| StorageError::Io)?;
    Ok((bytes.len() as u64, hex(&Sha256::digest(&bytes))))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Consistent application backup: `VACUUM INTO` a new private file in a new
/// private directory under `out`, then a checksummed manifest.
pub fn drill_backup(
    admitted: &Admitted,
    out: &Admitted,
    marker: &str,
) -> Result<PathBuf, StorageError> {
    let set = out.path.join(format!("backup-{}", uuid::Uuid::new_v4()));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&set)
        .map_err(|_| StorageError::Io)?;
    let dest = set.join(DRILL_DB);
    let source = connect(
        &admitted.path.join(DRILL_DB),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    )?;
    source
        .execute("VACUUM INTO ?1", [dest.to_str().ok_or(StorageError::Io)?])
        .map_err(|_| StorageError::Io)?;
    fs::set_permissions(&dest, std::os::unix::fs::PermissionsExt::from_mode(0o600))
        .map_err(|_| StorageError::Io)?;
    File::open(&dest)
        .and_then(|f| f.sync_all())
        .map_err(|_| StorageError::Io)?;
    let rows = verify_db(&dest, marker)?;
    let (bytes, sha256) = sha256_file(&dest)?;
    let manifest = Manifest {
        format: "airs-mcp-m0-drill-backup/1".into(),
        created_at_unix: now(),
        sqlite_version: rusqlite::version().into(),
        file: DRILL_DB.into(),
        bytes,
        sha256,
        rows,
        marker_sha256: hex(&Sha256::digest(marker.as_bytes())),
    };
    let json = serde_json::to_vec_pretty(&manifest).map_err(|_| StorageError::Io)?;
    publish_private(&set, "manifest.json", &json, false)?;
    Ok(set)
}

/// Integrity check plus content verification: every row must be exactly
/// `<marker>:<id>` with ids 0..n contiguous, so a restore is proven to carry the
/// seeded data, not merely a file whose hash matches its own manifest.
fn verify_db(path: &Path, marker: &str) -> Result<i64, StorageError> {
    let c = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| StorageError::Io)?;
    let ok: String = c
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|_| StorageError::Integrity)?;
    if ok != "ok" {
        return Err(StorageError::Integrity);
    }
    let mut stmt = c
        .prepare("SELECT id, body FROM drill ORDER BY id")
        .map_err(|_| StorageError::Integrity)?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map_err(|_| StorageError::Integrity)?;
    let mut n = 0i64;
    for row in rows {
        let (id, body) = row.map_err(|_| StorageError::Integrity)?;
        if id != n || body != format!("{marker}:{id}") {
            return Err(StorageError::ContentMismatch);
        }
        n += 1;
    }
    Ok(n)
}

/// Verify a restored backup set: manifest hash/length, SQLite integrity, rows,
/// and that it holds the expected synthetic marker.
pub fn drill_verify(set: &Path, marker: &str) -> Result<Manifest, StorageError> {
    let raw = fs::read(set.join("manifest.json")).map_err(|_| StorageError::Io)?;
    let manifest: Manifest =
        serde_json::from_slice(&raw).map_err(|_| StorageError::ManifestMismatch)?;
    if manifest.file != DRILL_DB
        || manifest.marker_sha256 != hex(&Sha256::digest(marker.as_bytes()))
    {
        return Err(StorageError::ManifestMismatch);
    }
    let (bytes, sha256) = sha256_file(&set.join(DRILL_DB))?;
    if bytes != manifest.bytes || sha256 != manifest.sha256 {
        return Err(StorageError::ManifestMismatch);
    }
    if verify_db(&set.join(DRILL_DB), marker)? != manifest.rows {
        return Err(StorageError::ManifestMismatch);
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private_dir() -> (PathBuf, Admitted) {
        let base = std::env::temp_dir().join(format!("airs-mcp-spike-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        let canonical = base.canonicalize().unwrap();
        let admitted = admit(&canonical).unwrap();
        (canonical, admitted)
    }

    #[test]
    fn admission_refuses_loose_modes_and_symlinks() {
        let (base, _) = private_dir();
        let loose = base.join("loose");
        fs::DirBuilder::new().mode(0o750).create(&loose).unwrap();
        assert_eq!(
            admit(&loose).unwrap_err(),
            StorageError::NotPrivateDirectory
        );
        let link = base.join("link");
        std::os::unix::fs::symlink(&base, &link).unwrap();
        assert_eq!(admit(&link).unwrap_err(), StorageError::NotPrivateDirectory);
        assert_eq!(
            admit(Path::new("relative")).unwrap_err(),
            StorageError::NotAbsolute
        );
        // A group-writable ancestor (the fsGroup shape) is refused for its children.
        let group = base.join("group");
        fs::DirBuilder::new().mode(0o700).create(&group).unwrap();
        fs::set_permissions(&group, std::os::unix::fs::PermissionsExt::from_mode(0o2770)).unwrap();
        let child = group.join("data");
        fs::DirBuilder::new().mode(0o700).create(&child).unwrap();
        assert_eq!(admit(&child).unwrap_err(), StorageError::UnsafeAncestor);
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn second_owner_is_refused_while_the_first_holds_the_lock() {
        let (base, admitted) = private_dir();
        let first = acquire(&admitted).unwrap();
        assert!(matches!(
            acquire(&admitted),
            Err(StorageError::OwnedByAnotherProcess)
        ));
        drop(first);
        assert!(acquire(&admitted).is_ok());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn backup_drill_round_trips_and_detects_tampering() {
        let (base, admitted) = private_dir();
        let out = private_child(&admitted, "backups").unwrap();
        assert!(private_child(&admitted, "../escape").is_err());
        drill_seed(&admitted, 50, "M0-MARKER").unwrap();
        let set = drill_backup(&admitted, &out, "M0-MARKER").unwrap();
        let m = drill_verify(&set, "M0-MARKER").unwrap();
        assert_eq!(m.rows, 50);
        assert_eq!(
            drill_verify(&set, "OTHER").unwrap_err(),
            StorageError::ManifestMismatch
        );
        // Content check: change one row and rewrite the manifest hash so only the
        // row verification can catch it.
        let copy = base.join("tampered");
        fs::DirBuilder::new().mode(0o700).create(&copy).unwrap();
        fs::copy(set.join(DRILL_DB), copy.join(DRILL_DB)).unwrap();
        let c = rusqlite::Connection::open(copy.join(DRILL_DB)).unwrap();
        c.execute("UPDATE drill SET body = 'forged:7' WHERE id = 7", [])
            .unwrap();
        drop(c);
        let mut m: Manifest =
            serde_json::from_slice(&fs::read(set.join("manifest.json")).unwrap()).unwrap();
        let (bytes, sha) = sha256_file(&copy.join(DRILL_DB)).unwrap();
        m.bytes = bytes;
        m.sha256 = sha;
        fs::write(copy.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
        assert_eq!(
            drill_verify(&copy, "M0-MARKER").unwrap_err(),
            StorageError::ContentMismatch
        );
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(set.join(DRILL_DB))
            .unwrap();
        f.write_all(b"x").unwrap();
        assert_eq!(
            drill_verify(&set, "M0-MARKER").unwrap_err(),
            StorageError::ManifestMismatch
        );
        fs::remove_dir_all(&base).unwrap();
    }
}
