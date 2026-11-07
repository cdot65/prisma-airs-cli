//! Checked private archive directories/descriptors. Readers never create or chmod.
use super::failure;
use airs_runtime_core::error::CliError;
use rustix::fs::{Mode, OFlags};
use std::{
    ffi::OsStr,
    fs::{self, File, Metadata},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

pub const DATABASE: &str = "archive.sqlite3";
pub const WRITER: &str = "writer.lock";
pub const GATE: &str = "maintenance.lock";

pub fn validate_override(path: &Path) -> Result<(), CliError> {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(CliError::usage(
            "--history-dir must be a nonempty absolute path",
        ));
    }
    Ok(())
}

pub fn root(
    explicit: Option<&Path>,
    xdg: Option<&OsStr>,
    home: Option<&Path>,
) -> Result<PathBuf, CliError> {
    if let Some(path) = explicit {
        validate_override(path)?;
        return Ok(airs_runtime_core::store::normalize(path));
    }
    if let Some(path) = xdg {
        let path = Path::new(path);
        if path.as_os_str().is_empty() || !path.is_absolute() {
            return Err(failure("XDG_DATA_HOME must be a nonempty absolute path"));
        }
        return Ok(airs_runtime_core::store::normalize(
            &path.join("airs-cli/history"),
        ));
    }
    match home {
        Some(path) if path.is_absolute() => Ok(airs_runtime_core::store::normalize(
            &path.join(".local/share/airs-cli/history"),
        )),
        _ => Err(failure(
            "HOME must be absolute; set --history-dir or XDG_DATA_HOME",
        )),
    }
}

fn identity(m: &Metadata) -> (u64, u64) {
    (m.dev(), m.ino())
}
fn private(m: &Metadata, directory: bool) -> bool {
    m.uid() == airs_runtime_core::store::euid()
        && m.mode() & 0o077 == 0
        && if directory {
            m.is_dir()
        } else {
            m.is_file() && m.nlink() == 1
        }
}

fn ancestors(path: &Path) -> Result<(), CliError> {
    for component in path.ancestors() {
        let m = fs::symlink_metadata(component)
            .map_err(|_| failure("cannot inspect archive ancestors"))?;
        let sticky_system = m.uid() == 0 && m.mode() & 0o1000 != 0;
        if !m.is_dir()
            || (m.uid() != 0 && m.uid() != airs_runtime_core::store::euid())
            || (m.mode() & 0o022 != 0 && !sticky_system)
        {
            return Err(failure("unsafe archive ancestor ownership or permissions"));
        }
    }
    Ok(())
}

/// Resolve only parents, never a symlink at the controlled root itself.
fn admitted_root(path: &Path, create: bool) -> Result<Option<PathBuf>, CliError> {
    let path = airs_runtime_core::store::normalize(path);
    let parent = path
        .parent()
        .ok_or_else(|| failure("the filesystem root cannot be the archive root"))?;
    if parent == path || path.file_name().is_none() {
        return Err(failure("invalid archive root"));
    }
    let mut missing = Vec::new();
    let mut cursor = parent;
    loop {
        match fs::symlink_metadata(cursor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    cursor
                        .file_name()
                        .ok_or_else(|| failure("invalid archive parent"))?
                        .to_os_string(),
                );
                cursor = cursor
                    .parent()
                    .ok_or_else(|| failure("invalid archive parent"))?;
            }
            Err(_) => return Err(failure("cannot inspect archive parent")),
        }
    }
    let mut canonical = cursor
        .canonicalize()
        .map_err(|_| failure("cannot resolve archive parents"))?;
    ancestors(&canonical)?;
    if !missing.is_empty() && !create {
        return Ok(None);
    }
    for name in missing.iter().rev() {
        let opened = ancestor_directory(&canonical)?;
        match rustix::fs::mkdirat(&opened, name, Mode::from_raw_mode(0o700)) {
            Ok(()) => {}
            Err(rustix::io::Errno::EXIST) => {}
            Err(_) => return Err(failure("cannot create private archive parent")),
        }
        canonical.push(name);
        let _ = checked_directory(&canonical)?;
        opened
            .sync_all()
            .map_err(|_| failure("archive parent durability could not be confirmed"))?;
    }
    Ok(Some(
        canonical.join(
            path.file_name()
                .ok_or_else(|| failure("invalid archive root"))?,
        ),
    ))
}

pub(super) fn ancestor_directory(path: &Path) -> Result<File, CliError> {
    ancestors(path)?;
    let before =
        fs::symlink_metadata(path).map_err(|_| failure("cannot inspect archive ancestor"))?;
    let file: File = rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| failure("cannot open archive ancestor"))?
    .into();
    if identity(
        &file
            .metadata()
            .map_err(|_| failure("cannot inspect archive ancestor"))?,
    ) != identity(&before)
    {
        return Err(failure("archive ancestor identity changed"));
    }
    local_filesystem(&file)?;
    Ok(file)
}

pub(super) fn checked_directory(path: &Path) -> Result<File, CliError> {
    let before = fs::symlink_metadata(path)
        .map_err(|_| failure("cannot inspect private archive directory"))?;
    if !private(&before, true) {
        return Err(failure(
            "archive directories must be owned private directories, without symlinks",
        ));
    }
    let file: File = rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| failure("cannot open private archive directory"))?
    .into();
    let after = file
        .metadata()
        .map_err(|_| failure("cannot inspect archive directory descriptor"))?;
    if !private(&after, true) || identity(&before) != identity(&after) {
        return Err(failure("archive directory identity changed"));
    }
    local_filesystem(&file)?;
    Ok(file)
}

fn local_filesystem(file: &File) -> Result<(), CliError> {
    let stat = rustix::fs::fstatfs(file)
        .map_err(|_| failure("cannot determine archive filesystem type"))?;
    #[cfg(target_os = "linux")]
    let remote = [
        rustix::fs::NFS_SUPER_MAGIC as u64,
        0xff534d42,
        0xfe534d42,
        0x517b,
    ]
    .contains(&(stat.f_type as u64));
    #[cfg(target_os = "macos")]
    let remote = {
        let name = stat
            .f_fstypename
            .iter()
            .take_while(|c| **c != 0)
            .map(|c| *c as u8)
            .collect::<Vec<_>>();
        [
            b"nfs".as_slice(),
            b"smbfs".as_slice(),
            b"afpfs".as_slice(),
            b"webdav".as_slice(),
        ]
        .contains(&name.as_slice())
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let remote = {
        let _ = stat;
        true
    };
    if remote {
        Err(failure(
            "live archives require a supported local filesystem; use logical export/import for transfer",
        ))
    } else {
        Ok(())
    }
}

pub struct Directory {
    pub path: PathBuf,
    root_path: PathBuf,
    root: File,
    pub cloud: File,
}

impl Directory {
    pub fn open(root: &Path, cloud_id: &str, create: bool) -> Result<Option<Self>, CliError> {
        if cloud_id.len() != 64
            || !cloud_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(failure("invalid cloud namespace"));
        }
        let Some(root_path) = admitted_root(root, create)? else {
            return Ok(None);
        };
        if !root_path.exists() {
            if fs::symlink_metadata(&root_path).is_ok() {
                return Err(failure("archive root is a symlink"));
            }
            if !create {
                return Ok(None);
            }
            let parent = root_path
                .parent()
                .ok_or_else(|| failure("invalid archive root"))?;
            // Ancestors may be public-read directories; creation is always private.
            ancestors(parent)?;
            let parent_handle = ancestor_directory(parent)?;
            match rustix::fs::mkdirat(
                &parent_handle,
                root_path
                    .file_name()
                    .ok_or_else(|| failure("invalid archive root"))?,
                Mode::from_raw_mode(0o700),
            ) {
                Ok(()) => parent_handle
                    .sync_all()
                    .map_err(|_| failure("archive root durability could not be confirmed"))?,
                Err(rustix::io::Errno::EXIST) => {}
                Err(_) => return Err(failure("cannot create private archive root")),
            }
        }
        let root = checked_directory(&root_path)?;
        let path = root_path.join(cloud_id);
        if fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
            if !create {
                return Ok(None);
            }
            match rustix::fs::mkdirat(&root, cloud_id, Mode::from_raw_mode(0o700)) {
                Ok(()) => root
                    .sync_all()
                    .map_err(|_| failure("cloud directory durability could not be confirmed"))?,
                Err(rustix::io::Errno::EXIST) => {}
                Err(_) => return Err(failure("cannot create private cloud directory")),
            }
        }
        let cloud = checked_directory(&path)?;
        let value = Self {
            path,
            root_path,
            root,
            cloud,
        };
        value.recheck()?;
        Ok(Some(value))
    }

    pub fn recheck(&self) -> Result<(), CliError> {
        for (path, file) in [(&self.root_path, &self.root), (&self.path, &self.cloud)] {
            let current =
                fs::symlink_metadata(path).map_err(|_| failure("archive directory disappeared"))?;
            let pinned = file
                .metadata()
                .map_err(|_| failure("archive directory descriptor failed"))?;
            if !private(&current, true) || identity(&current) != identity(&pinned) {
                return Err(failure("archive directory identity changed"));
            }
        }
        Ok(())
    }

    pub fn exists(&self, name: &str) -> Result<bool, CliError> {
        if name.is_empty() || name.contains('/') || name == "." || name == ".." {
            return Err(failure("invalid archive file name"));
        }
        self.recheck()?;
        match rustix::fs::statat(&self.cloud, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => Ok(true),
            Err(rustix::io::Errno::NOENT) => Ok(false),
            Err(_) => Err(failure("cannot inspect archive file")),
        }
    }

    pub fn file(&self, name: &str, write: bool, create: bool) -> Result<File, CliError> {
        self.recheck()?;
        if name.contains('/') || name == "." || name == ".." {
            return Err(failure("invalid archive file name"));
        }
        let flags = if write { OFlags::RDWR } else { OFlags::RDONLY };
        if !create {
            let before = fs::symlink_metadata(self.path.join(name))
                .map_err(|_| failure("cannot inspect archive file before open"))?;
            if !private(&before, false) {
                return Err(failure(
                    "archive files must be private owned regular files without symlinks or hard links",
                ));
            }
        }
        // A regular-to-FIFO replacement must not block before descriptor checks.
        let flags = flags | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
        let file: File = match rustix::fs::openat(
            &self.cloud,
            name,
            flags
                | if create {
                    OFlags::CREATE | OFlags::EXCL
                } else {
                    OFlags::empty()
                },
            Mode::from_raw_mode(0o600),
        ) {
            Ok(fd) => fd.into(),
            Err(_) => return Err(failure("cannot open private archive file")),
        };
        let metadata = file
            .metadata()
            .map_err(|_| failure("cannot inspect archive file descriptor"))?;
        if !private(&metadata, false) {
            return Err(failure(
                "archive files must be owned regular files with no group/other permissions or hard links",
            ));
        }
        let current = fs::symlink_metadata(self.path.join(name))
            .map_err(|_| failure("archive file disappeared"))?;
        if !private(&current, false) || identity(&current) != identity(&metadata) {
            return Err(failure("archive file identity changed"));
        }
        self.recheck()?;
        Ok(file)
    }

    pub fn check_file(&self, name: &str, file: &File) -> Result<(), CliError> {
        let current = self.file(name, false, false)?;
        if identity(
            &current
                .metadata()
                .map_err(|_| failure("cannot inspect archive file"))?,
        ) != identity(
            &file
                .metadata()
                .map_err(|_| failure("cannot inspect archive file"))?,
        ) {
            return Err(failure("archive file identity changed"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestDir;
    use std::os::unix::fs::{PermissionsExt, symlink};
    const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    #[test]
    fn roots_and_nonwriting_missing_reads() {
        let _isolation = crate::test_isolation();
        let d = TestDir::new("history-paths");
        let path = d.path().join("missing/parents/history");
        assert!(Directory::open(&path, ID, false).unwrap().is_none());
        assert!(!path.exists());
        assert!(root(Some(Path::new("relative")), None, None).is_err());
        assert!(root(None, Some(OsStr::new("")), None).is_err());
        assert!(root(None, Some(OsStr::new("relative")), None).is_err());
        assert_eq!(
            root(None, Some(d.path().as_os_str()), None).unwrap(),
            d.path().join("airs-cli/history")
        );
        let dir = Directory::open(&path, ID, true).unwrap().unwrap();
        assert_eq!(dir.cloud.metadata().unwrap().mode() & 0o777, 0o700);
        assert!(Directory::open(&path, ID, false).unwrap().is_some());
    }
    #[test]
    fn rejects_symlinks_loose_files_and_replaced_identities() {
        let _isolation = crate::test_isolation();
        let d = TestDir::new("history-path-refusals");
        let root = d.path().join("history");
        let dir = Directory::open(&root, ID, true).unwrap().unwrap();
        let file = dir.file(DATABASE, true, true).unwrap();
        fs::set_permissions(dir.path.join(DATABASE), fs::Permissions::from_mode(0o644)).unwrap();
        assert!(dir.file(DATABASE, false, false).is_err());
        fs::set_permissions(dir.path.join(DATABASE), fs::Permissions::from_mode(0o600)).unwrap();
        fs::hard_link(dir.path.join(DATABASE), dir.path.join("hardlink")).unwrap();
        assert!(dir.file(DATABASE, false, false).is_err());
        fs::remove_file(dir.path.join("hardlink")).unwrap();
        fs::rename(dir.path.join(DATABASE), dir.path.join("old")).unwrap();
        let _ = dir.file(DATABASE, true, true).unwrap();
        assert!(dir.check_file(DATABASE, &file).is_err());
        fs::remove_file(dir.path.join(DATABASE)).unwrap();
        symlink(dir.path.join("old"), dir.path.join(DATABASE)).unwrap();
        assert!(dir.file(DATABASE, false, false).is_err());
        let alias = d.path().join("alias");
        symlink(&root, &alias).unwrap();
        assert!(Directory::open(&alias, ID, false).is_err());
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(Directory::open(&root, ID, false).is_err());
    }
}
