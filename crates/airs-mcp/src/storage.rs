//! The service data subtree (contract §9.3): private-subtree admission on the real
//! mount shape, filesystem classification, the single-owner lock with its owner record,
//! and private publication of the activated configuration. Qualified in M0.6 (P7);
//! the history archive itself lives under this subtree (airs-history-store).

use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use rustix::fs::{FlockOperation, flock};
use serde::Serialize;

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

/// A private 0600 candidate written beside its target and not yet visible under the
/// target's name. Dropping it uncommitted removes it, so a refused activation leaves
/// the previous file untouched. Every fallible step that can refuse (opening the
/// directory, writing and syncing the candidate) happens before publication.
pub struct Staged {
    pub path: PathBuf,
    target: PathBuf,
    /// The directory, opened before the candidate is written, so publication needs no
    /// further fallible open.
    dir: File,
    committed: bool,
}

/// The outcome of a publication that happened: the target now holds the staged bytes.
/// `durable` is false when the directory sync after the rename failed; the file is in
/// place and active, but its directory entry may not survive a power loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Published {
    pub durable: bool,
}

#[cfg(test)]
thread_local! {
    /// Test-only fault: fail the directory sync after the rename.
    pub static FAIL_DIRECTORY_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl Staged {
    /// Atomically replace the target with the staged bytes. `Err` means nothing was
    /// published (the rename failed; the previous target is intact). `Ok` means the
    /// target holds the new bytes, whatever the directory sync then reports.
    pub fn commit(mut self) -> Result<Published, StorageError> {
        fs::rename(&self.path, &self.target).map_err(|_| StorageError::Io)?;
        self.committed = true;
        #[cfg(test)]
        if FAIL_DIRECTORY_SYNC.with(|f| f.get()) {
            return Ok(Published { durable: false });
        }
        Ok(Published {
            durable: self.dir.sync_all().is_ok(),
        })
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Write `bytes` as a durable private candidate for `dir/name` without replacing it.
pub fn stage_private(dir: &Path, name: &str, bytes: &[u8]) -> Result<Staged, StorageError> {
    let handle = File::open(dir).map_err(|_| StorageError::Io)?;
    let path = dir.join(format!(".{name}.{}.staged", uuid::Uuid::new_v4()));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|_| StorageError::Io)?;
    let staged = Staged {
        path,
        target: dir.join(name),
        dir: handle,
        committed: false,
    };
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|_| StorageError::Io)?;
    Ok(staged)
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

/// The recovery epoch and the cursor key (contract §4.2), kept in `control/seed.json`
/// (0600) until the M3 control store adopts them. Created once with an exclusive,
/// no-clobber publication; never regenerated over an existing file.
pub struct ControlSeed {
    pub epoch: String,
    pub cursor_key: [u8; 32],
}

const SEED: &str = "seed.json";

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    for chunk in out.chunks_mut(16) {
        chunk.copy_from_slice(&uuid::Uuid::new_v4().as_bytes()[..chunk.len()]);
    }
    out
}

fn hex_bytes<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != 2 * N
        || !text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return None;
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// Read the seed under an admitted `control/` directory, creating it on first start.
pub fn control_seed(control: &Admitted) -> Result<ControlSeed, StorageError> {
    let path = control.path.join(SEED);
    if fs::symlink_metadata(&path).is_err() {
        let epoch = random_bytes::<4>();
        let key = random_bytes::<32>();
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        let body = serde_json::json!({"epoch": hex(&epoch), "cursor_key": hex(&key)}).to_string();
        // A concurrent creator loses the hard-link race and reads the winner's seed.
        let _ = publish_private(&control.path, SEED, body.as_bytes(), false);
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| StorageError::Integrity)?;
    if !metadata.file_type().is_file()
        || metadata.uid() != euid()
        || metadata.mode() & 0o077 != 0
        || metadata.len() > 4096
    {
        return Err(StorageError::Integrity);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).map_err(|_| StorageError::Io)?)
            .map_err(|_| StorageError::Integrity)?;
    let epoch = value["epoch"]
        .as_str()
        .filter(|e| hex_bytes::<4>(e).is_some());
    let key = value["cursor_key"].as_str().and_then(hex_bytes::<32>);
    match (epoch, key, value.as_object().map(serde_json::Map::len)) {
        (Some(epoch), Some(cursor_key), Some(2)) => Ok(ControlSeed {
            epoch: epoch.to_owned(),
            cursor_key,
        }),
        _ => Err(StorageError::Integrity),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn private_dir() -> (PathBuf, Admitted) {
        let base = std::env::temp_dir().join(format!("airs-mcp-{}", uuid::Uuid::new_v4()));
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
    fn the_control_seed_is_created_once_and_never_regenerated() {
        let (_dir, admitted) = private_dir();
        let control = private_child(&admitted, "control").unwrap();
        let first = control_seed(&control).unwrap();
        assert_eq!(first.epoch.len(), 8);
        let again = control_seed(&control).unwrap();
        assert_eq!(
            (again.epoch, again.cursor_key),
            (first.epoch.clone(), first.cursor_key)
        );
        let path = control.path.join(SEED);
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        fs::write(&path, br#"{"epoch":"zz","cursor_key":"00"}"#).unwrap();
        assert!(matches!(
            control_seed(&control),
            Err(StorageError::Integrity)
        ));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            control_seed(&control),
            Err(StorageError::Integrity)
        ));
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
}
