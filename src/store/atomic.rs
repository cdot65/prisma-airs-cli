//! Write transactions on the store (DESIGN §3.9, D78): directory creation, the lock,
//! stale temporary-file cleanup, the atomic write (temp + fsync + rename + directory
//! fsync) and the rollback of multi-file changes.

use super::lock::{self, Lock, Waiter};
use super::{
    DirError, DirState, FileError, Store, check_dir, dir_metadata_ok, euid, open_checked, shown,
    unsafe_dir_error, unsafe_file_error, valid_tenant_name,
};
use crate::error::CliError;
use rustix::fs::{Mode, OFlags};
use std::fs::File;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The steps of [`write_atomic`] a unit test can make fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    TempCreate,
    Write,
    Fsync,
    Rename,
    DirSync,
}

#[cfg(test)]
thread_local! {
    static FAIL_AT: std::cell::Cell<Option<Step>> = const { std::cell::Cell::new(None) };
    /// Which atomic writes (1-based, counted since the hook was set) fail; empty: all.
    static FAIL_WRITES: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Atomic writes started since the hook was set.
    static WRITES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Make `step` fail in this thread's next atomic write(s) (tests only).
#[cfg(test)]
pub fn fail_at(step: Option<Step>) {
    fail_writes(step, &[]);
}

/// Make `step` fail only in the listed atomic writes of this thread (1-based, counted
/// from this call; an empty list: every write). Tests only.
#[cfg(test)]
pub fn fail_writes(step: Option<Step>, writes: &[u32]) {
    FAIL_AT.with(|cell| cell.set(step));
    FAIL_WRITES.with(|cell| *cell.borrow_mut() = writes.to_vec());
    WRITES.with(|cell| cell.set(0));
}

#[cfg(test)]
fn count_write() {
    WRITES.with(|cell| cell.set(cell.get() + 1));
}

#[cfg(not(test))]
#[inline]
fn count_write() {}

#[cfg(test)]
fn injected(step: Step) -> std::io::Result<()> {
    let current = WRITES.with(std::cell::Cell::get);
    let selected =
        FAIL_WRITES.with(|cell| cell.borrow().is_empty() || cell.borrow().contains(&current));
    if FAIL_AT.with(std::cell::Cell::get) == Some(step) && selected {
        Err(std::io::Error::other("injected failure"))
    } else {
        Ok(())
    }
}

#[cfg(not(test))]
#[inline]
fn injected(_: Step) -> std::io::Result<()> {
    Ok(())
}

/// How an atomic write ended when it did not fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Saved {
    /// Renamed into place and the directory synced.
    Durable,
    /// Renamed into place, but the directory fsync failed: print [`dir_sync_warning`].
    DirectoryNotSynced,
}

/// `⚠ <path> was saved, but its directory could not be synced; …` (without the indent).
pub fn dir_sync_warning(path: &Path) -> String {
    format!(
        "⚠ {} was saved, but its directory could not be synced; the change may not survive a system crash",
        shown(path)
    )
}

fn not_changed(path: &Path) -> CliError {
    CliError::operational(format!("Cannot save {}; it was not changed", shown(path)))
}

fn same_file(path: &Path, identity: (u64, u64)) -> bool {
    std::fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && (metadata.dev(), metadata.ino()) == identity)
}

/// `<path> is read-only; restore write permission (chmod 600) to change it`.
pub fn read_only_error(path: &Path) -> CliError {
    CliError::operational(format!(
        "{} is read-only; restore write permission (chmod 600) to change it",
        shown(path)
    ))
}

/// `Cannot save <path>: it would exceed 1 MiB` when `bytes` are too large.
pub fn check_size(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    if bytes.len() as u64 > super::MAX_FILE_BYTES {
        return Err(CliError::operational(format!(
            "Cannot save {}: it would exceed 1 MiB",
            shown(path)
        )));
    }
    Ok(())
}

/// Check the file a writer would replace: a missing file is fine; an existing one must
/// pass §3.8 and have owner write.
pub fn check_replaceable(path: &Path) -> Result<(), CliError> {
    match open_checked(path, OFlags::RDONLY) {
        Ok(file) => {
            let mode = file
                .metadata()
                .map_err(|_| not_changed(path))?
                .permissions()
                .mode();
            if mode & 0o200 == 0 {
                return Err(read_only_error(path));
            }
            Ok(())
        }
        Err(FileError::NotFound) => Ok(()),
        Err(FileError::Unsafe) => Err(unsafe_file_error(path)),
        Err(FileError::TooLarge | FileError::Io) => Err(not_changed(path)),
    }
}

/// 16 lowercase hex digits from 8 random bytes (the v4 UUID generator's source).
fn random_hex() -> String {
    uuid::Uuid::new_v4().as_bytes()[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `.<file name>.<pid>.<16 hex>.tmp` in the same directory.
pub fn temp_name(file_name: &str) -> String {
    format!(".{file_name}.{}.{}.tmp", std::process::id(), random_hex())
}

/// A [`temp_name`] as long as any process's can be (the pid zero-padded to the 10 digits
/// of `u32::MAX`), for checking that a directory accepts the names of later rewrites.
pub fn longest_temp_name(file_name: &str) -> String {
    format!(
        ".{file_name}.{:010}.{}.tmp",
        std::process::id(),
        random_hex()
    )
}

/// `open(dir, O_RDONLY)` + `fsync`; whether it worked.
pub fn sync_dir(dir: &Path) -> bool {
    let synced = (|| -> std::io::Result<()> {
        injected(Step::DirSync)?;
        File::open(dir)?.sync_all()
    })();
    synced.is_ok()
}

/// Atomically replace `path` (a file in `dir`) with `bytes` (§3.9 "Atomic write").
pub fn write_atomic(dir: &Path, path: &Path, bytes: &[u8]) -> Result<Saved, CliError> {
    count_write();
    check_replaceable(path)?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| not_changed(path))?;
    let temp = dir.join(temp_name(&file_name));
    let written = (|| -> std::io::Result<()> {
        injected(Step::TempCreate)?;
        let fd = rustix::fs::open(
            &temp,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?;
        let mut file = File::from(fd);
        rustix::fs::fchmod(&file, Mode::from_raw_mode(0o600))?;
        injected(Step::Write)?;
        file.write_all(bytes)?;
        injected(Step::Fsync)?;
        file.sync_all()?;
        drop(file);
        injected(Step::Rename)?;
        std::fs::rename(&temp, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
        return Err(not_changed(path));
    }
    Ok(if sync_dir(dir) {
        Saved::Durable
    } else {
        Saved::DirectoryNotSynced
    })
}

/// Create `dir` (missing parents included) with mode 0700, or check an existing one
/// (§3.8 "Creation"). Returns whether it was created. An existing directory is never
/// chmod'ed.
pub fn ensure_dir(dir: &Path) -> Result<bool, CliError> {
    let cannot = || {
        CliError::operational(format!(
            "Cannot create {}; check the permissions of its parent directory",
            shown(dir)
        ))
    };
    match check_dir(dir) {
        Ok(DirState::Present) => return Ok(false),
        Ok(DirState::Missing) => {}
        Err(DirError::Unsafe) => return Err(unsafe_dir_error(dir)),
        Err(DirError::Io) => return Err(cannot()),
    }
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|_| cannot())?;
    match check_dir(dir) {
        Ok(DirState::Present) => Ok(true),
        Err(DirError::Unsafe) => Err(unsafe_dir_error(dir)),
        _ => Err(cannot()),
    }
}

/// `^\.config\.toml\.[0-9]{1,10}\.[0-9a-f]{16}\.tmp$`
pub fn is_index_temp(name: &str) -> bool {
    name.strip_prefix(".config.toml.").is_some_and(temp_suffix)
}

/// `^\.<tenant name>\.toml\.[0-9]{1,10}\.[0-9a-f]{16}\.tmp$`
pub fn is_tenant_temp(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('.') else {
        return false;
    };
    let Some((tenant, suffix)) = rest.split_once(".toml.") else {
        return false;
    };
    valid_tenant_name(tenant) && temp_suffix(suffix)
}

/// `[0-9]{1,10}\.[0-9a-f]{16}\.tmp`
fn temp_suffix(rest: &str) -> bool {
    let Some((pid, rest)) = rest.split_once('.') else {
        return false;
    };
    let Some(hex) = rest.strip_suffix(".tmp") else {
        return false;
    };
    (1..=10).contains(&pid.len())
        && pid.bytes().all(|b| b.is_ascii_digit())
        && hex.len() == 16
        && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Open an existing store directory for `*at` calls: `O_DIRECTORY | O_NOFOLLOW`, then
/// the §3.8 directory rule on the descriptor (`fstat`), so the checked directory is the
/// one later used. `Ok(None)`: it does not exist.
pub fn open_dir_checked(dir: &Path) -> Result<Option<File>, DirError> {
    let opened = rustix::fs::open(
        dir,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    );
    let fd = match opened {
        Ok(fd) => fd,
        Err(errno) if errno == rustix::io::Errno::NOENT => return Ok(None),
        // A symlink (`O_NOFOLLOW`) or a non-directory (`O_DIRECTORY`).
        Err(errno) if errno == rustix::io::Errno::LOOP || errno == rustix::io::Errno::NOTDIR => {
            return Err(DirError::Unsafe);
        }
        Err(_) => return Err(DirError::Io),
    };
    let file = File::from(fd);
    let metadata = file.metadata().map_err(|_| DirError::Io)?;
    if dir_metadata_ok(
        metadata.file_type().is_dir(),
        metadata.uid(),
        metadata.permissions().mode(),
        euid(),
    ) {
        Ok(Some(file))
    } else {
        Err(DirError::Unsafe)
    }
}

/// Remove leftovers of crashed writers (§3.9 step 4): matching names that are regular
/// files owned by the effective uid. Each directory is opened with
/// [`open_dir_checked`] and every lookup and unlink is relative to that descriptor, so
/// a symlinked or unsafe directory is never swept. Errors are ignored; nothing else is
/// touched.
pub fn remove_stale_temps(store: &Store) {
    let sweep = |dir: &Path, matches: fn(&str) -> bool| {
        let Ok(Some(handle)) = open_dir_checked(dir) else {
            return;
        };
        let Ok(entries) = rustix::fs::Dir::read_from(&handle) else {
            return;
        };
        let names: Vec<std::ffi::CString> = entries
            .flatten()
            .map(|entry| entry.file_name().to_owned())
            .filter(|name| name.to_str().is_ok_and(matches))
            .collect();
        for name in names {
            let regular_and_mine = rustix::fs::statat(
                &handle,
                name.as_c_str(),
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .is_ok_and(|stat| {
                rustix::fs::FileType::from_raw_mode(stat.st_mode)
                    == rustix::fs::FileType::RegularFile
                    && stat.st_uid == euid()
            });
            if regular_and_mine {
                let _ =
                    rustix::fs::unlinkat(&handle, name.as_c_str(), rustix::fs::AtFlags::empty());
            }
        }
    };
    sweep(store.dir(), is_index_temp);
    sweep(&store.tenants_dir(), is_tenant_temp);
}

/// The §3.8 check of an existing `D/tenants` for a transaction that will not create it:
/// a missing directory is fine, an unsafe one is refused before anything is changed.
fn check_tenants_dir(dir: &Path) -> Result<(), CliError> {
    match check_dir(dir) {
        Ok(DirState::Missing | DirState::Present) => Ok(()),
        Err(DirError::Unsafe) => Err(unsafe_dir_error(dir)),
        Err(DirError::Io) => Err(CliError::operational(format!(
            "Cannot create {}; check the permissions of its parent directory",
            shown(dir)
        ))),
    }
}

/// The previous state of one written file, for rollback.
#[derive(Debug)]
struct Entry {
    path: PathBuf,
    previous: Option<Vec<u8>>,
    owned_identity: Option<(u64, u64)>,
}

/// An open write transaction (§3.9 "Transaction" steps 2–7): the directories exist,
/// the lock is held and stale temporary files are gone. Files written through it can
/// be rolled back, newest first, until [`Transaction::commit`].
#[derive(Debug)]
pub struct Transaction {
    store: Store,
    lock: Option<Lock>,
    created_tenants_dir: bool,
    journal: Vec<Entry>,
    warnings: Vec<String>,
}

impl Transaction {
    /// Steps 2–4: create `D` (and `D/tenants` when `tenant_files`; otherwise an
    /// existing `D/tenants` is still checked, §3.8), take the lock and remove stale
    /// temporary files.
    pub fn begin(
        store: &Store,
        tenant_files: bool,
        waiter: &mut dyn Waiter,
    ) -> Result<Self, CliError> {
        ensure_dir(store.dir())?;
        let created_tenants_dir = if tenant_files {
            ensure_dir(&store.tenants_dir())?
        } else {
            check_tenants_dir(&store.tenants_dir())?;
            false
        };
        let lock = lock::acquire(store, waiter)?;
        remove_stale_temps(store);
        Ok(Self {
            store: store.clone(),
            lock: Some(lock),
            created_tenants_dir,
            journal: Vec::new(),
            warnings: Vec::new(),
        })
    }

    #[cfg(test)]
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Atomically write `bytes` to `path` (in `D` or `D/tenants`), remembering what it
    /// replaced. `previous` is the file's current content (`None`: it did not exist),
    /// which the caller read under the lock.
    pub fn write(
        &mut self,
        path: &Path,
        bytes: &[u8],
        previous: Option<Vec<u8>>,
    ) -> Result<(), CliError> {
        check_size(path, bytes)?;
        let dir = path.parent().unwrap_or(self.store.dir()).to_path_buf();
        let saved = write_atomic(&dir, path, bytes)?;
        self.journal.push(Entry {
            path: path.to_path_buf(),
            previous,
            owned_identity: None,
        });
        let mut synced = saved == Saved::Durable;
        if self.created_tenants_dir && dir == self.store.tenants_dir() {
            synced &= sync_dir(self.store.dir());
            self.created_tenants_dir = false;
        }
        if !synced {
            self.warnings.push(dir_sync_warning(path));
        }
        Ok(())
    }

    /// Durable, private, no-clobber format rollback copy under the config lock.
    /// Failed callers roll back the transaction. Raced replacements are never removed.
    pub fn scanner_snapshot(
        &mut self,
        tenant: &str,
        kind: &str,
        bytes: &[u8],
    ) -> Result<PathBuf, CliError> {
        let failed = || {
            CliError::operational(
                "Cannot safely save scanner rollback snapshot; tenant configuration was not changed",
            )
        };
        if !valid_tenant_name(tenant) || !["pre-scanners", "pre-restore", "pre-mcp"].contains(&kind)
        {
            return Err(failed());
        }
        let dir = self.store.tenants_dir();
        let name = format!(".{tenant}.{kind}.{}.toml", uuid::Uuid::new_v4());
        let path = dir.join(&name);
        check_size(&path, bytes)?;
        let parent = open_dir_checked(&dir)
            .map_err(|_| failed())?
            .ok_or_else(failed)?;
        let temp = temp_name(&format!("{tenant}.toml"));
        let temp_path = dir.join(&temp);
        count_write();
        injected(Step::TempCreate).map_err(|_| failed())?;
        let fd = rustix::fs::openat(
            &parent,
            &temp,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|_| failed())?;
        let mut file = File::from(fd);
        let metadata = file.metadata().map_err(|_| failed())?;
        let identity = (metadata.dev(), metadata.ino());
        let saved = (|| -> std::io::Result<()> {
            rustix::fs::fchmod(&file, Mode::from_raw_mode(0o600))?;
            injected(Step::Write)?;
            file.write_all(bytes)?;
            injected(Step::Fsync)?;
            file.sync_all()?;
            injected(Step::Rename)?;
            rustix::fs::renameat_with(
                &parent,
                &temp,
                &parent,
                &name,
                rustix::fs::RenameFlags::NOREPLACE,
            )?;
            Ok(())
        })();
        if saved.is_err() {
            if same_file(&temp_path, identity) {
                let _ = rustix::fs::unlinkat(&parent, &temp, rustix::fs::AtFlags::empty());
            }
            return Err(failed());
        }
        self.journal.push(Entry {
            path: path.clone(),
            previous: None,
            owned_identity: Some(identity),
        });
        injected(Step::DirSync)
            .and_then(|()| parent.sync_all())
            .map_err(|_| failed())?;
        if !same_file(&path, identity) {
            return Err(failed());
        }
        Ok(path)
    }

    /// Remove `path` under the lock (no rollback entry). The unlink is relative to its
    /// directory opened with [`open_dir_checked`], so a symlinked or unsafe directory is
    /// refused (an `Other` error) and never followed; a symlink in place of the file is
    /// removed, not its target.
    pub fn remove(&mut self, path: &Path) -> std::io::Result<()> {
        let unsafe_dir = || std::io::Error::other("unsafe directory");
        let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
            return Err(unsafe_dir());
        };
        let handle = match open_dir_checked(dir) {
            Ok(Some(handle)) => handle,
            Ok(None) => return Err(std::io::ErrorKind::NotFound.into()),
            Err(_) => return Err(unsafe_dir()),
        };
        rustix::fs::unlinkat(&handle, name, rustix::fs::AtFlags::empty())?;
        let synced = injected(Step::DirSync).is_ok() && handle.sync_all().is_ok();
        if !synced {
            self.warnings.push(dir_sync_warning(path));
        }
        Ok(())
    }

    /// Undo every write of this transaction, newest first (best effort): a new file is
    /// unlinked, a replaced one gets its previous bytes back. Whether every file was
    /// restored.
    pub fn rollback(&mut self) -> bool {
        let mut restored = true;
        while let Some(entry) = self.journal.pop() {
            let undone = match entry.previous {
                None if entry
                    .owned_identity
                    .is_some_and(|identity| !same_file(&entry.path, identity)) =>
                {
                    false
                }
                None => match std::fs::remove_file(&entry.path) {
                    Ok(()) => true,
                    Err(error) => error.kind() == std::io::ErrorKind::NotFound,
                },
                Some(bytes) => {
                    let dir = entry
                        .path
                        .parent()
                        .unwrap_or(self.store.dir())
                        .to_path_buf();
                    write_atomic(&dir, &entry.path, &bytes).is_ok()
                }
            };
            restored &= undone;
        }
        restored
    }

    /// Release the lock and return the directory-sync warnings to print.
    pub fn commit(mut self) -> Vec<String> {
        self.journal.clear();
        if let Some(lock) = self.lock.take() {
            lock.release();
        }
        std::mem::take(&mut self.warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::super::testdir::TestDir;
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn scanner_snapshot_is_private_durable_and_rollback_removes_only_owned_copy() {
        let dir = TestDir::new("scanner-snapshot");
        let store = Store::at(dir.path().join("D"));
        let mut tx = Transaction::begin(&store, true, &mut lock::Silent).unwrap();
        let bytes = b"mgmtTsgId = \"100\"\n";
        let first = tx.scanner_snapshot("prod", "pre-scanners", bytes).unwrap();
        assert_eq!(std::fs::read(&first).unwrap(), bytes);
        assert_eq!(mode(&first), 0o600);
        let second = tx.scanner_snapshot("prod", "pre-scanners", bytes).unwrap();
        assert_ne!(first, second);
        let held = File::open(&second).unwrap();
        std::fs::remove_file(&second).unwrap();
        std::fs::write(&second, b"raced replacement").unwrap();
        assert!(!tx.rollback(), "raced object must not be unlinked");
        assert!(!first.exists());
        assert_eq!(std::fs::read(&second).unwrap(), b"raced replacement");
        drop(held);
        tx.commit();
    }

    #[test]
    fn scanner_snapshot_failures_leave_live_bytes_and_no_owned_temps() {
        for step in [
            Step::TempCreate,
            Step::Write,
            Step::Fsync,
            Step::Rename,
            Step::DirSync,
        ] {
            let dir = TestDir::new("scanner-snapshot-failure");
            let store = Store::at(dir.path().join("D"));
            let mut tx = Transaction::begin(&store, true, &mut lock::Silent).unwrap();
            let live = store.tenant_path("prod");
            write_atomic(&store.tenants_dir(), &live, b"original").unwrap();
            fail_at(Some(step));
            let result = tx.scanner_snapshot("prod", "pre-scanners", b"original");
            fail_at(None);
            assert!(result.is_err(), "{step:?}");
            assert!(tx.rollback());
            tx.commit();
            assert_eq!(std::fs::read(&live).unwrap(), b"original");
            assert_eq!(names(&store.tenants_dir()), ["prod.toml"]);
        }
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn writes_new_and_replaced_files_0600() {
        let dir = TestDir::new("atomic-basic");
        let path = dir.path().join("config.toml");
        assert_eq!(
            write_atomic(dir.path(), &path, b"one\n").unwrap(),
            Saved::Durable
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"one\n");
        assert_eq!(mode(&path), 0o600);
        write_atomic(dir.path(), &path, b"two\n").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two\n");
        assert_eq!(names(dir.path()), ["config.toml"], "no temporary file left");
    }

    #[test]
    fn the_umask_cannot_widen_the_mode() {
        let dir = TestDir::new("atomic-mode");
        let path = dir.path().join("t.toml");
        write_atomic(dir.path(), &path, b"x").unwrap();
        assert_eq!(mode(&path), 0o600);
    }

    #[test]
    fn unsafe_and_read_only_targets_are_refused_unchanged() {
        let dir = TestDir::new("atomic-refuse");
        let ro = dir.write("ro.toml", b"old", 0o400);
        assert_eq!(
            write_atomic(dir.path(), &ro, b"new").unwrap_err().message(),
            format!(
                "{} is read-only; restore write permission (chmod 600) to change it",
                ro.display()
            )
        );
        assert_eq!(std::fs::read(&ro).unwrap(), b"old");
        let wide = dir.write("wide.toml", b"old", 0o644);
        assert!(
            write_atomic(dir.path(), &wide, b"new")
                .unwrap_err()
                .message()
                .starts_with("Refusing to use")
        );
        let link = dir.path().join("link.toml");
        let target = dir.write("target", b"old", 0o600);
        symlink(&target, &link).unwrap();
        assert!(
            write_atomic(dir.path(), &link, b"new")
                .unwrap_err()
                .message()
                .starts_with("Refusing to use")
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"old", "never followed");
        assert_eq!(
            names(dir.path()),
            ["link.toml", "ro.toml", "target", "wide.toml"]
        );
    }

    #[test]
    fn a_failure_before_the_rename_keeps_the_old_bytes_and_removes_the_temp() {
        for step in [Step::TempCreate, Step::Write, Step::Fsync, Step::Rename] {
            let dir = TestDir::new("atomic-fail");
            let path = dir.write("config.toml", b"old\n", 0o600);
            fail_at(Some(step));
            let result = write_atomic(dir.path(), &path, b"new\n");
            fail_at(None);
            assert_eq!(
                result.unwrap_err().message(),
                format!("Cannot save {}; it was not changed", path.display()),
                "{step:?}"
            );
            assert_eq!(std::fs::read(&path).unwrap(), b"old\n", "{step:?}");
            assert_eq!(names(dir.path()), ["config.toml"], "{step:?}: temp removed");
            // A new file that fails is not created at all.
            let fresh = dir.path().join("fresh.toml");
            fail_at(Some(step));
            assert!(write_atomic(dir.path(), &fresh, b"x").is_err());
            fail_at(None);
            assert!(!fresh.exists(), "{step:?}");
        }
    }

    #[test]
    fn a_directory_sync_failure_after_the_rename_is_only_a_warning() {
        let dir = TestDir::new("atomic-dirsync");
        let path = dir.write("config.toml", b"old\n", 0o600);
        fail_at(Some(Step::DirSync));
        let saved = write_atomic(dir.path(), &path, b"new\n");
        fail_at(None);
        assert_eq!(saved.unwrap(), Saved::DirectoryNotSynced);
        assert_eq!(std::fs::read(&path).unwrap(), b"new\n");
        assert_eq!(
            dir_sync_warning(&path),
            format!(
                "⚠ {} was saved, but its directory could not be synced; the change may not survive a system crash",
                path.display()
            )
        );
    }

    #[test]
    fn temp_names_and_patterns() {
        let name = temp_name("config.toml");
        assert!(is_index_temp(&name), "{name}");
        let tenant = temp_name("dev-1.toml");
        assert!(is_tenant_temp(&tenant), "{tenant}");
        assert!(!is_index_temp(&tenant));
        for name in [
            ".config.toml.1.0123456789abcdef.tmp",
            ".config.toml.1234567890.ffffffffffffffff.tmp",
        ] {
            assert!(is_index_temp(name), "{name}");
        }
        for name in [
            "config.toml.1.0123456789abcdef.tmp",
            ".config.toml..0123456789abcdef.tmp",
            ".config.toml.12345678901.0123456789abcdef.tmp",
            ".config.toml.1.0123456789ABCDEF.tmp",
            ".config.toml.1.0123456789abcde.tmp",
            ".config.toml.1.0123456789abcdef.tmp.x",
            ".config.toml.x.0123456789abcdef.tmp",
        ] {
            assert!(!is_index_temp(name), "{name}");
        }
        assert!(is_tenant_temp(".dev.toml.7.0123456789abcdef.tmp"));
        for name in [
            ".-dev.toml.7.0123456789abcdef.tmp",
            ".dev.json.7.0123456789abcdef.tmp",
            "dev.toml.7.0123456789abcdef.tmp",
            ".dev.toml.7.0123456789abcdef",
        ] {
            assert!(!is_tenant_temp(name), "{name}");
        }
    }

    #[test]
    fn stale_temps_are_removed_and_nothing_else() {
        let dir = TestDir::new("atomic-stale");
        let store = Store::at(dir.path());
        for name in [
            ".config.toml.12.0123456789abcdef.tmp",
            "config.toml",
            "notes.tmp",
            ".config.toml.12.0123456789abcdef.tmp.keep",
        ] {
            dir.write(name, b"x", 0o600);
        }
        for name in [
            "tenants/.dev.toml.3.fedcba9876543210.tmp",
            "tenants/dev.toml",
            "tenants/.dev.json.3.fedcba9876543210.tmp",
        ] {
            dir.write(name, b"x", 0o600);
        }
        std::fs::create_dir(dir.path().join(".config.toml.1.aaaaaaaaaaaaaaaa.tmp")).unwrap();
        let target = dir.write("target", b"x", 0o600);
        symlink(
            &target,
            dir.path().join(".config.toml.2.bbbbbbbbbbbbbbbb.tmp"),
        )
        .unwrap();
        remove_stale_temps(&store);
        assert_eq!(
            names(dir.path()),
            [
                ".config.toml.1.aaaaaaaaaaaaaaaa.tmp",
                ".config.toml.12.0123456789abcdef.tmp.keep",
                ".config.toml.2.bbbbbbbbbbbbbbbb.tmp",
                "config.toml",
                "notes.tmp",
                "target",
                "tenants"
            ]
        );
        assert_eq!(
            names(&store.tenants_dir()),
            [".dev.json.3.fedcba9876543210.tmp", "dev.toml"]
        );
    }

    #[test]
    fn directories_are_created_0700_and_existing_ones_checked() {
        let dir = TestDir::new("atomic-dirs");
        let store_dir = dir.path().join("a/b/airs-cli");
        assert!(ensure_dir(&store_dir).unwrap());
        assert_eq!(mode(&store_dir), 0o700);
        assert_eq!(mode(&dir.path().join("a")), 0o700, "missing parents too");
        assert!(!ensure_dir(&store_dir).unwrap(), "exists");
        std::fs::set_permissions(&store_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!ensure_dir(&store_dir).unwrap());
        assert_eq!(
            mode(&store_dir),
            0o755,
            "an existing directory is never chmod'ed"
        );
        std::fs::set_permissions(&store_dir, std::fs::Permissions::from_mode(0o775)).unwrap();
        assert!(
            ensure_dir(&store_dir)
                .unwrap_err()
                .message()
                .starts_with("Refusing to use")
        );
        let blocker = dir.write("file", b"", 0o600);
        assert_eq!(
            ensure_dir(&blocker.join("sub")).unwrap_err().message(),
            format!(
                "Cannot create {}; check the permissions of its parent directory",
                blocker.join("sub").display()
            )
        );
    }

    #[test]
    fn transactions_write_and_roll_back_newest_first() {
        let dir = TestDir::new("atomic-tx");
        let store = Store::at(dir.path().join("D"));
        let mut tx = Transaction::begin(&store, true, &mut lock::Silent).unwrap();
        assert_eq!(mode(store.dir()), 0o700);
        assert_eq!(mode(&store.tenants_dir()), 0o700);
        let tenant = store.tenant_path("dev");
        tx.write(&tenant, b"mgmtTsgId = \"1\"\n", None).unwrap();
        tx.write(&store.index_path(), b"version = 1\n", None)
            .unwrap();
        assert!(tx.rollback());
        assert!(!tenant.exists() && !store.index_path().exists());
        let warnings = tx.commit();
        assert!(warnings.is_empty());

        std::fs::write(store.index_path(), b"old\n").unwrap();
        std::fs::set_permissions(store.index_path(), std::fs::Permissions::from_mode(0o600))
            .unwrap();
        let mut tx = Transaction::begin(&store, true, &mut lock::Silent).unwrap();
        tx.write(&store.index_path(), b"new\n", Some(b"old\n".to_vec()))
            .unwrap();
        tx.write(&tenant, b"t\n", None).unwrap();
        // The next write fails: the caller rolls back.
        fail_at(Some(Step::Rename));
        let failed = tx.write(&store.tenant_path("other"), b"x", None);
        fail_at(None);
        assert!(failed.is_err());
        assert!(tx.rollback());
        assert_eq!(std::fs::read(store.index_path()).unwrap(), b"old\n");
        assert!(!tenant.exists());
        let doomed = store.tenant_path("doomed");
        tx.write(&doomed, b"x\n", None).unwrap();
        tx.remove(&doomed).unwrap();
        assert!(!doomed.exists());
        assert_eq!(tx.store(), &store);
        drop(tx.commit());
        assert_eq!(std::fs::read(store.lock_path()).unwrap(), b"", "released");
    }

    #[test]
    fn a_transaction_holds_the_lock_and_sweeps_stale_temps() {
        let dir = TestDir::new("atomic-tx-lock");
        let store = Store::at(dir.path());
        dir.write(".config.toml.9.0123456789abcdef.tmp", b"x", 0o600);
        let tx = Transaction::begin(&store, false, &mut lock::Silent).unwrap();
        assert!(
            !dir.path()
                .join(".config.toml.9.0123456789abcdef.tmp")
                .exists()
        );
        assert!(!store.tenants_dir().exists(), "tenants/ only when needed");
        let busy = lock::acquire_with(&store, &mut lock::Silent, std::time::Duration::ZERO);
        assert!(
            busy.unwrap_err()
                .message()
                .starts_with("Another change is in progress")
        );
        drop(tx.commit());
        drop(lock::acquire(&store, &mut lock::Silent).unwrap());
    }

    #[test]
    fn an_unsafe_tenants_dir_is_refused_by_every_transaction_and_never_swept() {
        let dir = TestDir::new("atomic-tenants-dir");
        let store = Store::at(dir.path().join("D"));
        let stale = ".dev.toml.7.0123456789abcdef.tmp";
        let kept = dir.write(&format!("elsewhere/{stale}"), b"x", 0o600);
        let target = dir.write("elsewhere/dev.toml", b"precious", 0o600);
        dir.write("D/config.toml", b"version = 1\n", 0o600);
        symlink(dir.path().join("elsewhere"), store.tenants_dir()).unwrap();
        for tenant_files in [false, true] {
            assert_eq!(
                Transaction::begin(&store, tenant_files, &mut lock::Silent)
                    .unwrap_err()
                    .message(),
                unsafe_dir_error(&store.tenants_dir()).message(),
                "tenant_files: {tenant_files}"
            );
        }
        remove_stale_temps(&store);
        assert!(kept.exists(), "a symlinked directory is never swept");
        assert_eq!(std::fs::read(&target).unwrap(), b"precious");
        assert!(!store.lock_path().exists(), "refused before the lock");
        // A group- or other-writable directory is refused too, and not swept.
        std::fs::remove_file(store.tenants_dir()).unwrap();
        let stale_inside = dir.write(&format!("D/tenants/{stale}"), b"x", 0o600);
        std::fs::set_permissions(store.tenants_dir(), std::fs::Permissions::from_mode(0o777))
            .unwrap();
        assert!(
            Transaction::begin(&store, false, &mut lock::Silent)
                .unwrap_err()
                .message()
                .starts_with("Refusing to use")
        );
        remove_stale_temps(&store);
        assert!(stale_inside.exists());
        assert_eq!(
            open_dir_checked(&store.tenants_dir()).err(),
            Some(DirError::Unsafe)
        );
        assert!(matches!(
            open_dir_checked(&dir.path().join("missing")),
            Ok(None)
        ));
        std::fs::set_permissions(store.tenants_dir(), std::fs::Permissions::from_mode(0o700))
            .unwrap();
        drop(
            Transaction::begin(&store, false, &mut lock::Silent)
                .unwrap()
                .commit(),
        );
        assert!(!stale_inside.exists(), "a safe directory is swept");
    }

    #[test]
    fn remove_never_follows_a_directory_swapped_for_a_symlink() {
        let dir = TestDir::new("atomic-remove");
        let store = Store::at(dir.path().join("D"));
        let mut tx = Transaction::begin(&store, true, &mut lock::Silent).unwrap();
        let target = dir.write("elsewhere/dev.toml", b"precious", 0o600);
        std::fs::remove_dir(store.tenants_dir()).unwrap();
        symlink(dir.path().join("elsewhere"), store.tenants_dir()).unwrap();
        let error = tx.remove(&store.tenant_path("dev")).unwrap_err();
        assert_ne!(error.kind(), std::io::ErrorKind::NotFound);
        assert_eq!(std::fs::read(&target).unwrap(), b"precious");
        // A missing directory or file is NotFound.
        std::fs::remove_file(store.tenants_dir()).unwrap();
        assert_eq!(
            tx.remove(&store.tenant_path("dev")).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        std::fs::create_dir(store.tenants_dir()).unwrap();
        assert_eq!(
            tx.remove(&store.tenant_path("dev")).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        drop(tx.commit());
    }

    #[test]
    fn oversized_bytes_are_refused_before_writing() {
        let dir = TestDir::new("atomic-size");
        let store = Store::at(dir.path());
        let mut tx = Transaction::begin(&store, false, &mut lock::Silent).unwrap();
        let big = vec![b'x'; super::super::MAX_FILE_BYTES as usize + 1];
        assert_eq!(
            tx.write(&store.index_path(), &big, None)
                .unwrap_err()
                .message(),
            format!(
                "Cannot save {}: it would exceed 1 MiB",
                store.index_path().display()
            )
        );
        assert!(!store.index_path().exists());
        drop(tx.commit());
    }
}
