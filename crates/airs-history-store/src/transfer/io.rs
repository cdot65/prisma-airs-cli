//! Descriptor-bound private staging and atomic no-clobber artifact publication.
use crate::{failure, paths};
use airs_runtime_core::error::CliError;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use std::{
    ffi::OsString,
    fs::File,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
/// A file's (device, inode) identity. Both sides of every comparison come from the same
/// `stat` structure, so the field types are the platform's own (`dev_t` is `u64` on Linux
/// but `i32` on macOS, where mixing std's `u64` with rustix's native field did not compile).
type Identity = (rustix::fs::Dev, u64);
fn identity(file: &File) -> Result<Identity, CliError> {
    let s = rustix::fs::fstat(file)
        .map_err(|_| failure("cannot inspect transfer staging descriptor"))?;
    Ok((s.st_dev, s.st_ino))
}
fn named_identity(parent: &File, name: &std::ffi::OsStr) -> Result<Identity, CliError> {
    let s = rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| failure("transfer staging entry is unavailable"))?;
    Ok((s.st_dev, s.st_ino))
}
fn check_named(parent: &File, name: &std::ffi::OsStr, file: &File) -> Result<(), CliError> {
    if identity(file)? != named_identity(parent, name)? {
        return Err(failure("transfer staging identity changed"));
    }
    Ok(())
}

pub fn preflight(path: &Path) -> Result<PathBuf, CliError> {
    if path.as_os_str().is_empty() || !airs_runtime_core::files::names_a_file(path) {
        return Err(CliError::usage("--file must name a nonempty file path"));
    }
    let absolute =
        std::path::absolute(path).map_err(|_| failure("cannot resolve transfer output path"))?;
    if !matches!(std::fs::symlink_metadata(&absolute),Err(e) if e.kind()==std::io::ErrorKind::NotFound)
    {
        return Err(failure(
            "transfer output already exists or cannot be inspected; no file overwritten",
        ));
    }
    let parent = absolute
        .parent()
        .ok_or_else(|| failure("invalid transfer output parent"))?
        .canonicalize()
        .map_err(|_| failure("transfer output requires an existing directory parent"))?;
    let _parent = paths::ancestor_directory(&parent)?;
    Ok(parent.join(
        absolute
            .file_name()
            .ok_or_else(|| failure("invalid transfer output name"))?,
    ))
}

pub struct Output {
    parent: File,
    parent_path: PathBuf,
    name: OsString,
    destination: OsString,
    pub file: File,
    armed: bool,
}
impl Output {
    pub fn new(path: &Path) -> Result<Self, CliError> {
        let path = preflight(path)?;
        let parent = paths::ancestor_directory(
            path.parent()
                .ok_or_else(|| failure("invalid transfer output parent"))?,
        )?;
        let name = OsString::from(format!(
            ".airs-history-export.{}",
            uuid::Uuid::new_v4().simple()
        ));
        let file: File = rustix::fs::openat(
            &parent,
            &name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|_| failure("cannot create private transfer output staging"))?
        .into();
        let output = Self {
            parent_path: path
                .parent()
                .ok_or_else(|| failure("invalid transfer parent"))?
                .to_path_buf(),
            parent,
            name,
            destination: path
                .file_name()
                .ok_or_else(|| failure("invalid transfer destination"))?
                .to_owned(),
            file,
            armed: true,
        };
        rustix::fs::fchmod(&output.file, Mode::from_raw_mode(0o600))
            .map_err(|_| failure("cannot confirm private transfer output mode"))?;
        output.check()?;
        Ok(output)
    }
    pub fn check(&self) -> Result<(), CliError> {
        let current_parent = paths::ancestor_directory(&self.parent_path)?;
        if identity(&current_parent)? != identity(&self.parent)? {
            return Err(failure("transfer output parent identity changed"));
        }
        check_named(&self.parent, &self.name, &self.file)?;
        let m = self
            .file
            .metadata()
            .map_err(|_| failure("cannot inspect transfer output"))?;
        if !m.is_file()
            || m.uid() != airs_runtime_core::store::euid()
            || m.mode() & 0o077 != 0
            || m.nlink() != 1
        {
            return Err(failure(
                "transfer staging must remain a private owned single-link regular file",
            ));
        }
        Ok(())
    }
    /// True denotes post-publication directory durability uncertainty. The saved
    /// artifact remains in place; it is never overwritten or silently rolled back.
    pub fn publish(mut self) -> Result<bool, CliError> {
        self.check()?;
        self.file
            .sync_all()
            .map_err(|_| failure("transfer output fsync failed; no final file published"))?;
        self.check()?;
        rustix::fs::renameat_with(
            &self.parent,
            &self.name,
            &self.parent,
            &self.destination,
            RenameFlags::NOREPLACE,
        )
        .map_err(|_| {
            failure(
                "cannot atomically publish transfer without overwriting; final target not replaced",
            )
        })?;
        self.armed = false;
        if check_named(&self.parent, &self.destination, &self.file).is_err() {
            return Err(failure(
                "transfer was published but its destination identity changed",
            ));
        }
        Ok(self.parent.sync_all().is_err())
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        if self.armed && check_named(&self.parent, &self.name, &self.file).is_ok() {
            let _ = rustix::fs::unlinkat(&self.parent, &self.name, AtFlags::empty());
        }
    }
}

/// A bounded regular input, held while its exact bytes are copied into private
/// staging. The external pathname is never consulted for a second parsing pass.
pub struct Input {
    pub file: File,
    pub bytes: u64,
    before: std::fs::Metadata,
}
impl Input {
    pub fn open(path: &Path) -> Result<Self, CliError> {
        let before = std::fs::symlink_metadata(path)
            .map_err(|_| failure("cannot inspect logical transfer input"))?;
        if !before.is_file() || before.nlink() != 1 || before.len() > super::codec::MAX_TOTAL {
            return Err(failure(
                "logical transfer input must be a single-link regular file at most16 GiB",
            ));
        }
        let file: File = rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| failure("cannot open logical transfer input"))?
        .into();
        let admitted = file
            .metadata()
            .map_err(|_| failure("cannot inspect admitted transfer input"))?;
        if !admitted.is_file()
            || admitted.nlink() != 1
            || (before.dev(), before.ino()) != (admitted.dev(), admitted.ino())
            || before.len() != admitted.len()
        {
            return Err(failure("logical transfer input changed at admission"));
        }
        Ok(Self {
            bytes: admitted.len(),
            file,
            before: admitted,
        })
    }
    pub fn check(&self) -> Result<(), CliError> {
        let m = self
            .file
            .metadata()
            .map_err(|_| failure("cannot recheck transfer input descriptor"))?;
        let stamp = |m: &std::fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
                m.nlink(),
            )
        };
        if stamp(&m) != stamp(&self.before) {
            return Err(failure("logical transfer input changed while being staged"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        os::unix::fs::{PermissionsExt, symlink},
    };
    /// The staging identity check binds a name to the descriptor that was opened: a name
    /// replaced by another file (a different inode) is refused, the original is accepted.
    /// Both sides come from one `stat` type, so this holds on Linux and macOS alike.
    #[test]
    fn a_replaced_staging_name_is_refused_by_its_identity() {
        let dir = crate::test_support::TestDir::new("transfer-identity");
        let parent = File::open(dir.path()).unwrap();
        let name = std::ffi::OsStr::new("staged");
        std::fs::write(dir.path().join("staged"), b"one").unwrap();
        let opened = File::open(dir.path().join("staged")).unwrap();
        assert!(check_named(&parent, name, &opened).is_ok());
        assert_eq!(
            identity(&opened).unwrap(),
            named_identity(&parent, name).unwrap()
        );
        std::fs::write(dir.path().join("other"), b"two").unwrap();
        std::fs::rename(dir.path().join("other"), dir.path().join("staged")).unwrap();
        assert!(check_named(&parent, name, &opened).is_err());
        assert!(named_identity(&parent, std::ffi::OsStr::new("missing")).is_err());
    }
    #[test]
    fn logical_transfer_publication_is_private_atomic_and_refuses_replacement() {
        let dir = crate::test_support::TestDir::new("transfer-output");
        let target = dir.path().join("export.jsonl");
        let mut out = Output::new(&target).unwrap();
        out.file.write_all(b"synthetic\n").unwrap();
        assert!(!target.exists());
        assert!(!out.publish().unwrap());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic\n");
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(Output::new(&target).is_err());
        let next = dir.path().join("raced.jsonl");
        let mut out = Output::new(&next).unwrap();
        out.file.write_all(b"staged").unwrap();
        symlink(&target, &next).unwrap();
        assert!(out.publish().is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic\n");
    }
    #[test]
    fn logical_transfer_input_refuses_symlinks_and_detects_mutation() {
        let dir = crate::test_support::TestDir::new("transfer-input");
        let path = dir.path().join("input.jsonl");
        std::fs::write(&path, b"synthetic\n").unwrap();
        let input = Input::open(&path).unwrap();
        assert_eq!(input.bytes, 10);
        input.check().unwrap();
        let link = dir.path().join("link");
        symlink(&path, &link).unwrap();
        assert!(Input::open(&link).is_err());
        std::fs::write(&path, b"changed").unwrap();
        assert!(input.check().is_err());
    }
}
