//! Private sibling staging + durable no-replace directory publication.
use super::invalid;
use crate::error::CliError;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use std::{
    ffi::OsString,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

pub fn preflight(path: &Path) -> Result<PathBuf, CliError> {
    if !crate::files::names_a_file(path) || path.as_os_str().is_empty() {
        return Err(invalid());
    }
    let path = std::path::absolute(path).map_err(|_| invalid())?;
    if std::fs::symlink_metadata(&path).is_ok()
        || !matches!(std::fs::symlink_metadata(&path),Err(e) if e.kind()==std::io::ErrorKind::NotFound)
    {
        return Err(invalid());
    }
    if !path
        .parent()
        .is_some_and(|p| std::fs::metadata(p).is_ok_and(|m| m.is_dir()))
    {
        return Err(invalid());
    }
    Ok(path)
}
/// Own only the named empty directory until its read descriptor can be opened.
struct EarlyDirectory<'a> {
    parent: &'a File,
    name: &'a std::ffi::OsStr,
    identity: rustix::fs::Stat,
    armed: bool,
}
impl Drop for EarlyDirectory<'_> {
    fn drop(&mut self) {
        if self.armed
            && rustix::fs::statat(self.parent, self.name, AtFlags::SYMLINK_NOFOLLOW)
                .is_ok_and(|s| s.st_ino == self.identity.st_ino && s.st_dev == self.identity.st_dev)
        {
            let _ = rustix::fs::unlinkat(self.parent, self.name, AtFlags::REMOVEDIR);
        }
    }
}
struct Staging {
    parent: File,
    dir: File,
    name: OsString,
    files: Vec<(&'static str, rustix::fs::Stat)>,
    armed: bool,
}
impl Drop for Staging {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        for (name, original) in &self.files {
            if rustix::fs::statat(&self.dir, *name, AtFlags::SYMLINK_NOFOLLOW)
                .is_ok_and(|s| s.st_ino == original.st_ino && s.st_dev == original.st_dev)
            {
                let _ = rustix::fs::unlinkat(&self.dir, *name, AtFlags::empty());
            }
        }
        if let (Ok(named), Ok(opened)) = (
            rustix::fs::statat(&self.parent, &self.name, AtFlags::SYMLINK_NOFOLLOW),
            rustix::fs::fstat(&self.dir),
        ) && named.st_ino == opened.st_ino
            && named.st_dev == opened.st_dev
        {
            let _ = rustix::fs::unlinkat(&self.parent, &self.name, AtFlags::REMOVEDIR);
        }
    }
}
/// Returns true if the final parent directory sync could not confirm durability.
pub fn publish(path: &Path, html: &str, evidence: &str, review: &str) -> Result<bool, CliError> {
    publish_with(path, html, evidence, review, |file| file.sync_all())
}
fn publish_with(
    path: &Path,
    html: &str,
    evidence: &str,
    review: &str,
    sync: impl Fn(&File) -> std::io::Result<()>,
) -> Result<bool, CliError> {
    publish_operations(path, html, evidence, review, sync, |stage, path| {
        rustix::fs::renameat_with(
            &stage.parent,
            &stage.name,
            &stage.parent,
            path.file_name().ok_or_else(invalid)?,
            RenameFlags::NOREPLACE,
        )
        .map_err(|_| invalid())
    })
}
fn publish_operations(
    path: &Path,
    html: &str,
    evidence: &str,
    review: &str,
    sync: impl Fn(&File) -> std::io::Result<()>,
    rename: impl Fn(&Staging, &Path) -> Result<(), CliError>,
) -> Result<bool, CliError> {
    let path = preflight(path)?;
    if html.len() > 32 * 1024 * 1024
        || evidence.len() > 32 * 1024 * 1024
        || review.len() > 4 * 1024 * 1024
    {
        return Err(invalid());
    }
    let parent = File::from(
        rustix::fs::open(
            path.parent().ok_or_else(invalid)?,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| invalid())?,
    );
    let name = OsString::from(format!(".airs-{}.bundle", uuid::Uuid::new_v4().simple()));
    rustix::fs::mkdirat(&parent, &name, Mode::from_raw_mode(0o700)).map_err(|_| invalid())?;
    let mut early = EarlyDirectory {
        parent: &parent,
        name: &name,
        identity: rustix::fs::statat(&parent, &name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| invalid())?,
        armed: true,
    };
    let dir = File::from(
        rustix::fs::openat(
            &parent,
            &name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| invalid())?,
    );
    early.armed = false;
    drop(early);
    let mut stage = Staging {
        parent,
        dir,
        name,
        files: vec![],
        armed: true,
    };
    rustix::fs::fchmod(&stage.dir, Mode::from_raw_mode(0o700)).map_err(|_| invalid())?;
    for (name, text) in [
        ("report.html", html),
        ("evidence.json", evidence),
        ("review.md", review),
    ] {
        let mut file = File::from(
            rustix::fs::openat(
                &stage.dir,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .map_err(|_| invalid())?,
        );
        stage
            .files
            .push((name, rustix::fs::fstat(&file).map_err(|_| invalid())?));
        rustix::fs::fchmod(&file, Mode::from_raw_mode(0o600)).map_err(|_| invalid())?;
        file.write_all(text.as_bytes()).map_err(|_| invalid())?;
        sync(&file).map_err(|_| invalid())?;
    }
    sync(&stage.dir).map_err(|_| invalid())?;
    // Never use an overwrite-capable rename fallback, even on unsupported filesystems.
    let named = rustix::fs::statat(&stage.parent, &stage.name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| invalid())?;
    let opened = rustix::fs::fstat(&stage.dir).map_err(|_| invalid())?;
    if named.st_ino != opened.st_ino || named.st_dev != opened.st_dev {
        return Err(invalid());
    }
    rename(&stage, &path)?;
    stage.armed = false;
    Ok(sync(&stage.parent).is_err())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, os::unix::fs::PermissionsExt};
    #[test]
    fn complete_private_bundle_and_existing_destination_refusal() {
        let temp = crate::test_support::TempDir::new("signal-bundle");
        let path = temp.join("brief");
        assert!(!publish(&path, "html", "json", "markdown").unwrap());
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        for name in ["report.html", "evidence.json", "review.md"] {
            assert_eq!(
                std::fs::metadata(path.join(name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        assert!(publish(&path, "other", "other", "other").is_err());
        assert_eq!(
            std::fs::read_to_string(path.join("report.html")).unwrap(),
            "html"
        );
    }
    #[test]
    fn sync_failure_cleans_staging_but_post_publish_failure_keeps_saved_bundle() {
        for fail_at in 0..5 {
            let temp = crate::test_support::TempDir::new("signal-bundle-sync");
            let path = temp.join("brief");
            let calls = Cell::new(0);
            let out = publish_with(&path, "html", "json", "markdown", |file| {
                let n = calls.get();
                calls.set(n + 1);
                if n == fail_at {
                    Err(std::io::Error::other("synthetic sync failure"))
                } else {
                    file.sync_all()
                }
            });
            if fail_at == 4 {
                assert!(out.unwrap());
                assert!(path.join("evidence.json").is_file());
            } else {
                assert!(out.is_err());
                assert!(!path.exists());
            }
            assert!(std::fs::read_dir(temp.path()).unwrap().all(|e| {
                !e.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".airs-")
            }));
        }
    }
    #[test]
    fn unsupported_atomic_rename_leaves_no_artifacts() {
        let temp = crate::test_support::TempDir::new("signal-bundle-unsupported");
        let path = temp.join("brief");
        let out = publish_operations(
            &path,
            "html",
            "json",
            "md",
            |f| f.sync_all(),
            |_, _| Err(invalid()),
        );
        assert!(out.is_err());
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    }
    #[test]
    fn raced_final_directory_is_never_overwritten_or_deleted() {
        let temp = crate::test_support::TempDir::new("signal-bundle-race");
        let path = temp.join("brief");
        let calls = Cell::new(0);
        let out = publish_with(&path, "html", "json", "markdown", |file| {
            calls.set(calls.get() + 1);
            if calls.get() == 4 {
                std::fs::create_dir(&path)?;
                std::fs::write(path.join("owned-by-other"), "preserve")?;
            }
            file.sync_all()
        });
        assert!(out.is_err());
        assert_eq!(
            std::fs::read_to_string(path.join("owned-by-other")).unwrap(),
            "preserve"
        );
        assert!(!path.join("report.html").exists());
    }
}
