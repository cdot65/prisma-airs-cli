//! Bounded input-file reads for the M4 commands (`--config`, `--prompts`; DESIGN §13.2,
//! D103).
//!
//! A path is used as typed (relative to the working directory) and followed through
//! symlinks. It must name a regular file within the size limit, read whole, in UTF-8.
//! Every failure has a fixed message that names the path as typed (terminal-safe when
//! printed) and echoes neither the OS error nor any file content.

use crate::error::CliError;
use std::ffi::OsStr;
use std::fs::File;
use std::path::Path;

pub use airs_runtime_core::files::{names_a_file, read_bytes, read_text, shown};

/// The `--config` limit (1 MiB).
pub const CONFIG_LIMIT: u64 = 1_048_576;
/// The `--prompts` limit (16 MiB).
pub const PROMPTS_LIMIT: u64 = 16_777_216;

/// The red team `--script-file` limit (4 MiB, DESIGN §15.3).
pub const SCRIPT_LIMIT: u64 = 4 * 1_048_576;
/// The `prompt-sets upload` limit (7 MiB, below the SDK's 8 MiB request bound).
pub const UPLOAD_LIMIT: u64 = 7 * 1_048_576;

/// Read a `--config` file: one JSON value that is an object (the D44 classes count as
/// invalid JSON).
pub fn read_config(path: &OsStr) -> Result<serde_json::Map<String, serde_json::Value>, CliError> {
    let text = read_text(path, CONFIG_LIMIT)?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|_| CliError::operational(format!("{} is not valid JSON", shown(path))))?;
    match value {
        serde_json::Value::Object(object) => Ok(object),
        _ => Err(CliError::operational(format!(
            "{} must contain a JSON object",
            shown(path)
        ))),
    }
}

/// Create `path` and write `bytes` to it whole (`topics sample --output-file`, §13.11,
/// D114): `O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC`, mode 0644 minus the
/// umask. An existing path (a symlink included) is never opened or changed; a write
/// that fails part-way removes the file it created.
pub fn write_new(path: &OsStr, bytes: &[u8]) -> Result<(), CliError> {
    use rustix::fs::{Mode, OFlags};
    use std::io::Write;
    let target = Path::new(path);
    let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let cannot_write = || {
        CliError::operational(format!(
            "Cannot write {}; check that its directory exists and is writable",
            shown(path)
        ))
    };
    let fd = match rustix::fs::open(target, flags, Mode::from_raw_mode(0o644)) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::EXIST) => {
            return Err(CliError::operational(format!(
                "{} already exists; choose another --output-file",
                shown(path)
            )));
        }
        Err(_) => return Err(cannot_write()),
    };
    let mut file = File::from(fd);
    if file.write_all(bytes).and_then(|()| file.flush()).is_err() {
        drop(file);
        let _ = std::fs::remove_file(target);
        return Err(cannot_write());
    }
    Ok(())
}

/// Whether a [`publish_new`] also synced its directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Published {
    Durable,
    /// Linked into place, but the directory fsync failed: print
    /// [`crate::store::atomic::dir_sync_warning`].
    DirectoryNotSynced,
    /// Linked into place whole, but the file's own `fsync` failed (only from
    /// [`Reservation::link_unsynced`]; [`publish_new`] never returns it).
    FileNotSynced,
}

/// `<path> already exists; choose another <flag>` (nothing is overwritten).
pub fn already_exists(path: &Path, flag: &str) -> CliError {
    CliError::operational(format!(
        "{} already exists; choose another {flag}",
        shown(path.as_os_str())
    ))
}

/// Refuse a destination where anything exists (`lstat`: a dangling symlink included),
/// before the tenant is read and any request is sent (§15.9, §15.18); a path whose last
/// component is not a file name ([`names_a_file`]) is [`cannot_write`] (D226).
pub fn refuse_existing(path: &Path, flag: &str) -> Result<(), CliError> {
    if !names_a_file(path) {
        return Err(cannot_write(path));
    }
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(already_exists(path, flag)),
    }
}

/// The checks of a new private destination before the tenant is read (§17.12, §18.13):
/// nothing may exist at `path` (`lstat`, [`refuse_existing`]), and its directory must
/// exist (`Cannot write <path>; check that its directory exists and is writable`).
pub fn check_new(path: &Path, flag: &str) -> Result<(), CliError> {
    refuse_existing(path, flag)?;
    if std::fs::metadata(parent_dir(path)).is_ok_and(|metadata| metadata.is_dir()) {
        Ok(())
    } else {
        Err(cannot_write(path))
    }
}

/// The directory a new file at `path` is created in (`.` for a bare name).
pub(crate) fn parent_dir(path: &Path) -> std::path::PathBuf {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    }
}

/// Whether text that is not JSON is a TypeScript `--file-format yaml` backup (js-yaml
/// output): a `resourceType:` line or a `version: 1` line. Shared by the DLP and runtime
/// profile restores (§16.10, §18.16).
pub fn typescript_yaml(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim_end();
        line.starts_with("resourceType:") || line == "version: 1"
    })
}

/// `Cannot write <path>; check that its directory exists and is writable`.
pub fn cannot_write(path: &Path) -> CliError {
    CliError::operational(format!(
        "Cannot write {}; check that its directory exists and is writable",
        shown(path.as_os_str())
    ))
}

/// A private temporary reserved next to a destination (the first half of
/// [`publish_new`], DESIGN §18.5): `.airs-<16 hex>.tmp` created with `O_WRONLY | O_CREAT
/// | O_EXCL | O_NOFOLLOW | O_CLOEXEC`, mode 0600. Dropping it removes the temporary
/// unless [`Reservation::keep`] took it.
#[derive(Debug)]
pub struct Reservation {
    dir: std::path::PathBuf,
    temp: std::path::PathBuf,
    file: Option<File>,
    armed: bool,
    /// The `fsync` of the temporary (`File::sync_all`; a test makes it fail, as a volume
    /// that refuses it would).
    sync: fn(&File) -> std::io::Result<()>,
}

/// Why [`Reservation::write_and_link`] did not publish.
#[derive(Debug)]
pub enum LinkError {
    /// Writing the temporary failed part-way; the temporary was removed.
    Write,
    /// The bytes were written whole, but the `fsync` of the temporary failed (a network
    /// or FUSE volume can refuse it); the temporary is complete and still reserved, so
    /// the caller decides ([`Reservation::link_unsynced`], or drop it).
    NotSynced(Reservation),
    /// Something exists at the destination now (a symlink included); the temporary is
    /// complete and still reserved.
    Exists(Reservation),
    /// Any other link failure; the temporary is complete and still reserved.
    Link(Reservation),
}

impl Reservation {
    /// Reserve a temporary in the directory of `path`; a failure is [`cannot_write`].
    pub fn reserve_new(path: &Path) -> Result<Self, CliError> {
        use rustix::fs::{Mode, OFlags};
        let dir = parent_dir(path);
        let temp = dir.join(format!(
            ".airs-{}.tmp",
            &uuid::Uuid::new_v4().simple().to_string()[..16]
        ));
        let flags =
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let fd = rustix::fs::open(&temp, flags, Mode::from_raw_mode(0o600))
            .map_err(|_| cannot_write(path))?;
        // The umask cannot widen 0600, but a narrower one must not hide the owner bits.
        if rustix::fs::fchmod(&fd, Mode::from_raw_mode(0o600)).is_err() {
            drop(fd);
            let _ = std::fs::remove_file(&temp);
            return Err(cannot_write(path));
        }
        Ok(Self {
            dir,
            temp,
            file: Some(File::from(fd)),
            armed: true,
            sync: File::sync_all,
        })
    }

    /// Test only: make the `fsync` of the temporary fail.
    #[cfg(test)]
    pub fn fail_sync_for_test(&mut self) {
        self.sync = |_| Err(std::io::Error::other("fsync refused (test)"));
    }

    /// The temporary's path (an MCP export re-screens what it wrote there).
    pub fn temp_path(&self) -> &Path {
        &self.temp
    }

    /// Write `bytes` to the temporary, `fsync` and close it, then `link` it to `path`
    /// (never overwriting), unlink the temporary and `fsync` the directory. A failed
    /// write is [`LinkError::Write`]; a failed `fsync` after a whole write is
    /// [`LinkError::NotSynced`] (the complete temporary kept for the caller).
    pub fn write_and_link(mut self, path: &Path, bytes: &[u8]) -> Result<Published, LinkError> {
        use std::io::Write;
        let Some(mut file) = self.file.take() else {
            return Err(LinkError::Write);
        };
        if file.write_all(bytes).and_then(|()| file.flush()).is_err() {
            return Err(LinkError::Write);
        }
        let synced = (self.sync)(&file).is_ok();
        drop(file);
        if !synced {
            return Err(LinkError::NotSynced(self));
        }
        self.link(path).map(Self::finish)
    }

    /// Append `bytes` to the temporary (a streamed transfer, written chunk by chunk).
    pub fn append(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        match self.file.as_mut() {
            Some(file) => file.write_all(bytes),
            None => Err(std::io::Error::other("the reservation is closed")),
        }
    }

    /// After [`Reservation::append`]: flush, `fsync` and close the temporary, then link
    /// it to `path` (never overwriting); a failed `fsync` is [`LinkError::NotSynced`].
    pub fn finish_written(mut self, path: &Path) -> Result<Published, LinkError> {
        use std::io::Write;
        let Some(mut file) = self.file.take() else {
            return Err(LinkError::Write);
        };
        if file.flush().is_err() {
            return Err(LinkError::Write);
        }
        let synced = (self.sync)(&file).is_ok();
        drop(file);
        if !synced {
            return Err(LinkError::NotSynced(self));
        }
        self.link(path).map(Self::finish)
    }

    /// Link a complete temporary whose `fsync` failed ([`LinkError::NotSynced`]) to
    /// `path` (never overwriting); `Ok` is [`Published::FileNotSynced`].
    pub fn link_unsynced(self, path: &Path) -> Result<Published, LinkError> {
        self.link(path)?.finish();
        Ok(Published::FileNotSynced)
    }

    /// `link` the temporary to `path` (never overwriting).
    fn link(self, path: &Path) -> Result<Self, LinkError> {
        match rustix::fs::link(&self.temp, path) {
            Ok(()) => Ok(self),
            Err(rustix::io::Errno::EXIST) => Err(LinkError::Exists(self)),
            Err(_) => Err(LinkError::Link(self)),
        }
    }

    /// After the link: the temporary unlinked and the directory `fsync`ed.
    fn finish(mut self) -> Published {
        let _ = std::fs::remove_file(&self.temp);
        self.armed = false;
        if crate::store::atomic::sync_dir(&self.dir) {
            Published::Durable
        } else {
            Published::DirectoryNotSynced
        }
    }

    /// Keep the temporary (it is not removed on drop); its path.
    pub fn keep(mut self) -> std::path::PathBuf {
        self.armed = false;
        std::mem::take(&mut self.temp)
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        drop(self.file.take());
        if self.armed {
            let _ = std::fs::remove_file(&self.temp);
        }
    }
}

/// Publish `bytes` at `path` without ever overwriting (DESIGN §15.18 "Publishing", the TS
/// `writeReportFile`): a [`Reservation`] in the destination directory, written, `fsync`ed
/// and closed, then `link`ed to `path` (an existing path, a symlink included, is
/// [`already_exists`]); the temporary unlinked; the directory `fsync`ed. Any other failure
/// is [`cannot_write`], the temporary removed. A reader never sees a partial file.
pub fn publish_new(path: &Path, bytes: &[u8], flag: &str) -> Result<Published, CliError> {
    match Reservation::reserve_new(path)?.write_and_link(path, bytes) {
        Ok(published) => Ok(published),
        Err(LinkError::Exists(_)) => Err(already_exists(path, flag)),
        Err(LinkError::Write | LinkError::NotSynced(_) | LinkError::Link(_)) => {
            Err(cannot_write(path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    use crate::test_support::TempDir;

    fn temp(name: &str, bytes: &[u8]) -> (TempDir, OsString) {
        let dir = TempDir::new("files-test");
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        (dir, path.into_os_string())
    }

    fn message(result: Result<impl std::fmt::Debug, CliError>) -> String {
        match result {
            Err(CliError::Operational(failure)) => failure.message,
            other => panic!("expected an operational error, got {other:?}"),
        }
    }

    #[test]
    fn config_files_are_bounded_utf8_json_objects() {
        let (_dir, path) = temp("ok.json", br#"{"topic_name": "t", "n": 1.0}"#);
        let object = read_config(&path).unwrap();
        assert_eq!(object["topic_name"], "t");
        let shown = path.to_string_lossy().into_owned();
        for (bytes, expected) in [
            (&b"[1]"[..], format!("{shown} must contain a JSON object")),
            (b"\"x\"", format!("{shown} must contain a JSON object")),
            (b"{", format!("{shown} is not valid JSON")),
            (b"{} {}", format!("{shown} is not valid JSON")),
            (br#"{"a": "\ud800"}"#, format!("{shown} is not valid JSON")),
            (br#"{"a": 1e999}"#, format!("{shown} is not valid JSON")),
            (b"{\"a\": \"\xff\"}", format!("{shown} is not valid UTF-8")),
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert_eq!(message(read_config(&path)), expected, "{bytes:?}");
        }
    }

    #[test]
    fn new_files_are_created_exclusively() {
        let dir = TempDir::new("files-test");
        let path = dir.path().join("out.csv");
        write_new(path.as_os_str(), b"a,b\n").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"a,b\n");
        let shown = path.to_string_lossy().into_owned();
        assert_eq!(
            message(write_new(path.as_os_str(), b"x")),
            format!("{shown} already exists; choose another --output-file")
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"a,b\n", "untouched");
        let link = dir.path().join("link.csv");
        std::os::unix::fs::symlink(dir.path().join("dangling"), &link).unwrap();
        assert!(
            message(write_new(link.as_os_str(), b"x"))
                .ends_with("already exists; choose another --output-file")
        );
        assert!(
            !dir.path().join("dangling").exists(),
            "the link is not followed"
        );
        let missing = dir.path().join("nodir").join("x.csv");
        assert_eq!(
            message(write_new(missing.as_os_str(), b"x")),
            format!(
                "Cannot write {}; check that its directory exists and is writable",
                missing.to_string_lossy()
            )
        );
    }

    #[test]
    fn published_files_are_private_exclusive_and_leave_no_temporary() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new("files-test");
        let path = dir.path().join("t.csv");
        assert_eq!(
            publish_new(&path, b"a,b\n", "--output-file").unwrap(),
            Published::Durable
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"a,b\n");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let shown = path.to_string_lossy().into_owned();
        assert_eq!(
            message(publish_new(&path, b"x", "--output-file")),
            format!("{shown} already exists; choose another --output-file")
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"a,b\n", "untouched");
        let link = dir.path().join("link.csv");
        std::os::unix::fs::symlink(dir.path().join("dangling"), &link).unwrap();
        assert!(
            message(refuse_existing(&link, "--output-dir"))
                .ends_with("choose another --output-dir")
        );
        assert!(message(publish_new(&link, b"x", "--output-file")).contains("already exists"));
        assert!(
            !dir.path().join("dangling").exists(),
            "the link is not followed"
        );
        assert!(refuse_existing(&dir.path().join("new.csv"), "--output-file").is_ok());
        let missing = dir.path().join("nodir").join("x.csv");
        assert!(message(publish_new(&missing, b"x", "--output-file")).starts_with("Cannot write "));
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(".airs-"))
            .collect();
        assert!(leftovers.is_empty(), "no temporary is left");
    }

    /// The key-file reservation (§18.5): 0600, removed on drop, kept after a lost race.
    #[test]
    fn reservations_are_private_and_never_overwrite() {
        use std::os::unix::fs::PermissionsExt;
        let temporaries = |dir: &Path| {
            std::fs::read_dir(dir)
                .unwrap()
                .flatten()
                .filter(|entry| entry.file_name().to_string_lossy().starts_with(".airs-"))
                .count()
        };
        let dir = TempDir::new("files-test");
        let path = dir.path().join("key.txt");
        let reservation = Reservation::reserve_new(&path).unwrap();
        let mode = std::fs::metadata(reservation.temp_path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        drop(reservation);
        assert_eq!(temporaries(dir.path()), 0, "dropped: removed");
        assert!(!path.exists());
        let reservation = Reservation::reserve_new(&path).unwrap();
        assert!(reservation.write_and_link(&path, b"k\n").is_ok());
        assert_eq!(std::fs::read(&path).unwrap(), b"k\n");
        assert_eq!(temporaries(dir.path()), 0);
        // The destination appeared meanwhile: the complete temporary is kept.
        let reservation = Reservation::reserve_new(&path).unwrap();
        let Err(LinkError::Exists(reservation)) = reservation.write_and_link(&path, b"new\n")
        else {
            panic!("expected an existing destination");
        };
        let kept = reservation.keep();
        assert_eq!(std::fs::read(&kept).unwrap(), b"new\n");
        assert_eq!(std::fs::read(&path).unwrap(), b"k\n", "never overwritten");
        let missing = dir.path().join("nodir").join("k.txt");
        assert_eq!(
            message(Reservation::reserve_new(&missing)),
            format!(
                "Cannot write {}; check that its directory exists and is writable",
                missing.to_string_lossy()
            )
        );
    }

    /// D226: a destination whose last component is not a file name is refused by the
    /// checks before the tenant (it would otherwise fail only at the final link).
    #[test]
    fn destinations_must_name_a_file() {
        for good in [
            "k.txt",
            "dir/k.txt",
            "./k",
            "/abs/k.json",
            ".hidden",
            "..k",
            "k.",
        ] {
            assert!(names_a_file(Path::new(good)), "{good}");
        }
        for bad in [
            "",
            "newdir/",
            "out.json/",
            "a/.",
            "a/..",
            ".",
            "..",
            "/",
            "a//",
        ] {
            assert!(!names_a_file(Path::new(bad)), "{bad}");
        }
        let dir = TempDir::new("files-test");
        for bad in ["newdir/", "out.json/", "sub/.", "sub/.."] {
            let path = dir.path().join(bad);
            let text = format!(
                "Cannot write {}; check that its directory exists and is writable",
                path.to_string_lossy()
            );
            assert_eq!(message(check_new(&path, "--key-file")), text, "{bad}");
            assert_eq!(message(refuse_existing(&path, "--output-file")), text);
        }
        assert!(check_new(&dir.path().join("fine.txt"), "--key-file").is_ok());
    }

    /// A failed `fsync` after a whole write keeps the complete temporary for the caller
    /// (the key file links it); a report or backup ([`publish_new`]) still fails.
    #[test]
    fn a_failed_file_sync_keeps_the_complete_temporary() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new("files-test");
        let path = dir.path().join("key.txt");
        let mut reservation = Reservation::reserve_new(&path).unwrap();
        reservation.fail_sync_for_test();
        let Err(LinkError::NotSynced(reservation)) = reservation.write_and_link(&path, b"k\n")
        else {
            panic!("expected an unsynced temporary");
        };
        assert_eq!(std::fs::read(reservation.temp_path()).unwrap(), b"k\n");
        assert!(!path.exists());
        assert_eq!(
            reservation.link_unsynced(&path).unwrap(),
            Published::FileNotSynced
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"k\n");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let leftovers = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(".airs-"))
            .count();
        assert_eq!(leftovers, 0, "the temporary is unlinked after the link");
        // Linking an unsynced temporary never overwrites either.
        let mut reservation = Reservation::reserve_new(&path).unwrap();
        reservation.fail_sync_for_test();
        let Err(LinkError::NotSynced(reservation)) = reservation.write_and_link(&path, b"new\n")
        else {
            panic!("expected an unsynced temporary");
        };
        let Err(LinkError::Exists(reservation)) = reservation.link_unsynced(&path) else {
            panic!("expected an existing destination");
        };
        drop(reservation);
        assert_eq!(std::fs::read(&path).unwrap(), b"k\n", "never overwritten");
    }

    #[test]
    fn missing_directories_fifos_and_large_files_are_unreadable() {
        let dir = TempDir::new("files-test");
        let missing = dir.path().join("missing.json").into_os_string();
        let expected = |path: &OsStr| {
            format!(
                "Cannot read {} (missing, not a regular file, unreadable or larger than 1 MiB)",
                path.to_string_lossy()
            )
        };
        assert_eq!(message(read_config(&missing)), expected(&missing));
        let directory = dir.path().as_os_str().to_owned();
        assert_eq!(message(read_config(&directory)), expected(&directory));
        let big = dir.path().join("big.json");
        std::fs::write(&big, vec![b' '; CONFIG_LIMIT as usize + 1]).unwrap();
        assert_eq!(
            message(read_config(big.as_os_str())),
            expected(big.as_os_str())
        );
        let exact = dir.path().join("exact.json");
        let mut bytes = vec![b' '; CONFIG_LIMIT as usize - 2];
        bytes.splice(0..0, *b"{}");
        std::fs::write(&exact, &bytes).unwrap();
        assert!(read_config(exact.as_os_str()).unwrap().is_empty());
        assert!(
            read_text(big.as_os_str(), PROMPTS_LIMIT).is_ok(),
            "a 1 MiB + 1 file is within the prompts limit"
        );
    }
}
