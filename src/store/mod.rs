//! The airs tenant store (DESIGN §3): its location (§3.1), the `config.toml`
//! index with its canonical bytes (§3.2), the safety checks for every store directory
//! and file (§3.8) and the TOML basic-string writer. Locking and atomic writes live in
//! [`lock`] and [`atomic`].
//!
//! Readers never create, lock, chmod or rename anything; only a write transaction
//! ([`atomic::Transaction`]) does.

pub mod atomic;
pub mod lock;

use crate::error::CliError;
use crate::output::terminal_safe;
pub use airs_runtime_core::store::{
    MAX_FILE_BYTES, PROGRAM, euid, normalize, parse_toml, toml_string, valid_tenant_name,
};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The most tenants `config.toml` may list.
pub const MAX_TENANTS: usize = 1000;
/// `D/config.toml`.
pub const INDEX_FILE: &str = "config.toml";
/// `D/tenants`.
pub const TENANTS_DIR: &str = "tenants";
/// `D/config.lock`.
pub const LOCK_FILE: &str = "config.lock";

/// A path for a message: absolute, normalised by construction, through `terminal_safe`.
pub fn shown(path: &Path) -> String {
    terminal_safe(&path.display().to_string()).into_owned()
}

/// The store directory `D` (§3.1). Every failure is an operational error.
pub fn store_dir(
    config_dir: Option<&OsStr>,
    xdg_config_home: Option<&OsStr>,
    home: Option<&Path>,
) -> Result<PathBuf, CliError> {
    if let Some(dir) = config_dir.filter(|value| !value.is_empty()) {
        let dir = Path::new(dir);
        if !dir.has_root() {
            return Err(CliError::operational(
                "AIRS_CLI_CONFIG_DIR must be an absolute path",
            ));
        }
        return Ok(normalize(dir));
    }
    if let Some(config) = xdg_config_home.filter(|value| !value.is_empty()) {
        let config = Path::new(config);
        if !config.has_root() {
            return Err(CliError::operational(
                "XDG_CONFIG_HOME must be an absolute path",
            ));
        }
        return Ok(normalize(&config.join("airs-cli")));
    }
    match home {
        Some(home) if home.has_root() => Ok(normalize(&home.join(".config/airs-cli"))),
        _ => Err(CliError::operational(
            "HOME is not an absolute path; set AIRS_CLI_CONFIG_DIR or XDG_CONFIG_HOME",
        )),
    }
}

/// `Refusing to use <dir>: …` (§3.8).
pub fn unsafe_dir_error(dir: &Path) -> CliError {
    CliError::operational(format!(
        "Refusing to use {}: it must be a directory (not a symlink) owned by the current user and not writable by group or others",
        shown(dir)
    ))
}

/// `Refusing to use <path>: …` (§3.8).
pub fn unsafe_file_error(path: &Path) -> CliError {
    CliError::operational(format!(
        "Refusing to use {}: it must be a regular file (not a symlink) owned by the current user with no group or other permissions (chmod 600)",
        shown(path)
    ))
}

/// The TypeScript registry text with the `config.toml` path (§3.2).
pub fn index_error(path: &Path) -> CliError {
    CliError::operational(format!(
        "Cannot read tenant registry at {}; restore a valid registry before proceeding",
        shown(path)
    ))
}

/// The TypeScript config text with the tenant-file path (§3.3).
pub fn tenant_file_error(path: &Path) -> CliError {
    CliError::operational(format!(
        "Cannot read a valid tenant config at {}",
        shown(path)
    ))
}

/// The §3.8 rule for a directory's metadata.
pub fn dir_metadata_ok(is_dir: bool, uid: u32, mode: u32, euid: u32) -> bool {
    is_dir && uid == euid && mode & 0o022 == 0
}

/// The §3.8 rule for a file's metadata.
pub fn file_metadata_ok(is_file: bool, uid: u32, mode: u32, euid: u32) -> bool {
    is_file && uid == euid && mode & 0o077 == 0
}

/// A store directory that exists or does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirState {
    Missing,
    Present,
}

/// Why a store directory cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirError {
    /// A symlink, a non-directory, a foreign owner or a group/other-writable mode.
    Unsafe,
    /// `symlink_metadata` failed other than with `NotFound`.
    Io,
}

/// The §3.8 directory checks, by `symlink_metadata` (not followed).
pub fn check_dir(path: &Path) -> Result<DirState, DirError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if dir_metadata_ok(
                metadata.file_type().is_dir(),
                metadata.uid(),
                metadata.permissions().mode(),
                euid(),
            ) {
                Ok(DirState::Present)
            } else {
                Err(DirError::Unsafe)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(DirState::Missing),
        Err(_) => Err(DirError::Io),
    }
}

/// Why a store file cannot be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileError {
    NotFound,
    /// A symlink, a non-regular file, a foreign owner, group/other mode bits, or a file
    /// swapped between the `lstat` and the open.
    Unsafe,
    /// Larger than [`MAX_FILE_BYTES`].
    TooLarge,
    /// Any other I/O failure (permissions included).
    Io,
}

fn io_kind(error: &std::io::Error) -> FileError {
    if error.kind() == std::io::ErrorKind::NotFound {
        FileError::NotFound
    } else {
        FileError::Io
    }
}

/// Open an existing store file with the §3.8 checks: `symlink_metadata`, an
/// `O_NOFOLLOW | O_CLOEXEC` open with `flags`, then `fstat` with the same device and
/// inode, a regular file, owned by the effective uid, `mode & 0o077 == 0`.
pub fn open_checked(path: &Path, flags: rustix::fs::OFlags) -> Result<File, FileError> {
    use rustix::fs::{Mode, OFlags};
    let before = std::fs::symlink_metadata(path).map_err(|error| io_kind(&error))?;
    if !file_metadata_ok(
        before.file_type().is_file(),
        before.uid(),
        before.permissions().mode(),
        euid(),
    ) {
        return Err(FileError::Unsafe);
    }
    let fd = rustix::fs::open(
        path,
        flags | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|errno| {
        if errno == rustix::io::Errno::NOENT {
            FileError::NotFound
        } else if errno == rustix::io::Errno::LOOP {
            FileError::Unsafe
        } else {
            FileError::Io
        }
    })?;
    let file = File::from(fd);
    let after = file.metadata().map_err(|_| FileError::Io)?;
    if after.dev() != before.dev()
        || after.ino() != before.ino()
        || !file_metadata_ok(
            after.file_type().is_file(),
            after.uid(),
            after.permissions().mode(),
            euid(),
        )
    {
        return Err(FileError::Unsafe);
    }
    Ok(file)
}

/// Read a whole store file of at most [`MAX_FILE_BYTES`] with the §3.8 checks.
pub fn read_checked(path: &Path) -> Result<Vec<u8>, FileError> {
    let file = open_checked(path, rustix::fs::OFlags::RDONLY)?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| FileError::Io)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(FileError::TooLarge);
    }
    Ok(bytes)
}

/// The validated `config.toml` (§3.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Index {
    /// Whether `config.toml` exists (a missing file or directory is an empty store).
    pub exists: bool,
    /// The selected tenant; always a key of `tenants`.
    pub active: Option<String>,
    /// Tenant name → pinned TSG, in byte order of the names (D86).
    pub tenants: BTreeMap<String, String>,
}

impl Index {
    /// Validate the strict schema. `None` for every failure.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let table = parse_toml(bytes)?;
        if !table
            .keys()
            .all(|key| matches!(key.as_str(), "version" | "active" | "tenants"))
        {
            return None;
        }
        if !matches!(table.get("version"), Some(toml::Value::Integer(1))) {
            return None;
        }
        let mut tenants = BTreeMap::new();
        match table.get("tenants") {
            None => {}
            Some(toml::Value::Table(entries)) => {
                if entries.len() > MAX_TENANTS {
                    return None;
                }
                for (name, entry) in entries {
                    let toml::Value::Table(fields) = entry else {
                        return None;
                    };
                    let tsg = match (fields.len(), fields.get("tsg_id")) {
                        (1, Some(toml::Value::String(tsg))) if !tsg.is_empty() => tsg,
                        _ => return None,
                    };
                    if !valid_tenant_name(name) {
                        return None;
                    }
                    tenants.insert(name.clone(), tsg.clone());
                }
            }
            Some(_) => return None,
        }
        let index = Self {
            exists: true,
            active: match table.get("active") {
                None => None,
                Some(toml::Value::String(name)) if tenants.contains_key(name) => Some(name.clone()),
                Some(_) => return None,
            },
            tenants,
        };
        let mut lowered: Vec<String> = index
            .tenants
            .keys()
            .map(|name| name.to_ascii_lowercase())
            .collect();
        lowered.sort();
        lowered.dedup();
        (lowered.len() == index.tenants.len()).then_some(index)
    }

    /// The canonical bytes (§3.2).
    pub fn to_toml(&self) -> String {
        let mut out = String::from("version = 1\n");
        if let Some(active) = &self.active {
            out.push_str(&format!("active = {}\n", toml_string(active)));
        }
        for (name, tsg) in &self.tenants {
            out.push_str(&format!(
                "\n[tenants.{name}]\ntsg_id = {}\n",
                toml_string(tsg)
            ));
        }
        out
    }

    /// Names in byte order.
    pub fn names(&self) -> Vec<&str> {
        self.tenants.keys().map(String::as_str).collect()
    }

    /// The pinned TSG of `name` (exact match).
    pub fn tsg(&self, name: &str) -> Option<&str> {
        self.tenants.get(name).map(String::as_str)
    }

    /// A registered name equal to `name` ignoring ASCII case but not exactly (D85).
    pub fn case_conflict(&self, name: &str) -> Option<&str> {
        self.tenants
            .keys()
            .find(|other| other.as_str() != name && other.eq_ignore_ascii_case(name))
            .map(String::as_str)
    }

    /// `(registered: a, b)` or `(no tenants registered)`.
    pub fn registered_suffix(&self) -> String {
        if self.tenants.is_empty() {
            "(no tenants registered)".to_owned()
        } else {
            format!("(registered: {})", self.names().join(", "))
        }
    }
}

/// Why a tenant file could not be read (before it is parsed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TenantFileRead {
    /// `D/tenants` or the file is missing.
    NotFound,
    /// The §3.8 checks refused this directory or file.
    Unsafe(PathBuf),
    /// Larger than 1 MiB, or unreadable.
    Unreadable,
}

/// The located store `D`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// The store at `dir` (already resolved and normalised).
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Resolve `D` (§3.1).
    pub fn locate(
        config_dir: Option<&OsStr>,
        xdg_config_home: Option<&OsStr>,
        home: Option<&Path>,
    ) -> Result<Self, CliError> {
        store_dir(config_dir, xdg_config_home, home).map(Self::at)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// `D/config.toml`.
    pub fn index_path(&self) -> PathBuf {
        self.dir.join(INDEX_FILE)
    }

    /// `D/tenants`.
    pub fn tenants_dir(&self) -> PathBuf {
        self.dir.join(TENANTS_DIR)
    }

    /// `D/tenants/<name>.toml` (whether or not it exists).
    pub fn tenant_path(&self, name: &str) -> PathBuf {
        self.tenants_dir().join(format!("{name}.toml"))
    }

    /// `D/config.lock`.
    pub fn lock_path(&self) -> PathBuf {
        self.dir.join(LOCK_FILE)
    }

    /// Read and validate `config.toml` (§3.2, §3.8). A missing `D` or file is the empty
    /// store (`exists: false`); nothing is ever created.
    pub fn read_index(&self) -> Result<Index, CliError> {
        let path = self.index_path();
        match check_dir(&self.dir) {
            Ok(DirState::Missing) => return Ok(Index::default()),
            Ok(DirState::Present) => {}
            Err(DirError::Unsafe) => return Err(unsafe_dir_error(&self.dir)),
            Err(DirError::Io) => return Err(index_error(&path)),
        }
        let bytes = match read_checked(&path) {
            Ok(bytes) => bytes,
            Err(FileError::NotFound) => return Ok(Index::default()),
            Err(FileError::Unsafe) => return Err(unsafe_file_error(&path)),
            Err(FileError::TooLarge | FileError::Io) => return Err(index_error(&path)),
        };
        Index::parse(&bytes).ok_or_else(|| index_error(&path))
    }

    /// The bytes of `D/tenants/<name>.toml` after the §3.8 directory and file checks.
    pub fn read_tenant_bytes(&self, name: &str) -> Result<Vec<u8>, TenantFileRead> {
        let dir = self.tenants_dir();
        match check_dir(&dir) {
            Ok(DirState::Missing) => return Err(TenantFileRead::NotFound),
            Ok(DirState::Present) => {}
            Err(DirError::Unsafe) => return Err(TenantFileRead::Unsafe(dir)),
            Err(DirError::Io) => return Err(TenantFileRead::Unreadable),
        }
        let path = self.tenant_path(name);
        read_checked(&path).map_err(|error| match error {
            FileError::NotFound => TenantFileRead::NotFound,
            FileError::Unsafe => TenantFileRead::Unsafe(path),
            FileError::TooLarge | FileError::Io => TenantFileRead::Unreadable,
        })
    }
}

#[cfg(test)]
pub(crate) mod testdir {
    //! A private temporary directory for store unit tests (under the system temp
    //! directory, never the home directory), removed on drop.

    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    pub struct TestDir(PathBuf);

    impl TestDir {
        pub fn new(tag: &str) -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let base = std::env::temp_dir()
                .canonicalize()
                .unwrap_or_else(|_| std::env::temp_dir());
            let dir = base.join(format!(
                "airs-{tag}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir(&dir).unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
            Self(dir)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }

        /// Write `relative` (parents created 0700) with `mode`.
        pub fn write(&self, relative: &str, bytes: &[u8], mode: u32) -> PathBuf {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
            if path.exists() {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            }
            std::fs::write(&path, bytes).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testdir::TestDir;
    use super::*;
    use std::os::unix::fs::symlink;

    fn dir(config: Option<&str>, xdg: Option<&str>, home: Option<&str>) -> Result<PathBuf, String> {
        store_dir(
            config.map(OsStr::new),
            xdg.map(OsStr::new),
            home.map(Path::new),
        )
        .map_err(|error| error.message())
    }

    #[test]
    fn store_location_forms() {
        assert_eq!(
            dir(None, None, Some("/home/u")),
            Ok(PathBuf::from("/home/u/.config/airs-cli"))
        );
        assert_eq!(
            dir(Some("/a/./b//c/../d/"), Some("/x"), Some("/h")),
            Ok(PathBuf::from("/a/b/d"))
        );
        assert_eq!(
            dir(None, Some("/x/../cfg"), Some("/h")),
            Ok(PathBuf::from("/cfg/airs-cli"))
        );
        assert_eq!(
            dir(Some(""), Some(""), Some("/h")),
            Ok(PathBuf::from("/h/.config/airs-cli")),
            "empty values are unset"
        );
        assert_eq!(
            dir(Some("rel"), None, Some("/h")),
            Err("AIRS_CLI_CONFIG_DIR must be an absolute path".into())
        );
        assert_eq!(
            dir(Some("~/x"), None, Some("/h")),
            Err("AIRS_CLI_CONFIG_DIR must be an absolute path".into()),
            "~ is not expanded"
        );
        assert_eq!(
            dir(None, Some("rel"), Some("/h")),
            Err("XDG_CONFIG_HOME must be an absolute path".into())
        );
        for home in [Some(""), Some("rel"), None] {
            assert_eq!(
                dir(None, None, home),
                Err(
                    "HOME is not an absolute path; set AIRS_CLI_CONFIG_DIR or XDG_CONFIG_HOME"
                        .into()
                )
            );
        }
    }

    #[test]
    fn lexical_normalisation() {
        assert_eq!(normalize(Path::new("/")), PathBuf::from("/"));
        assert_eq!(normalize(Path::new("/..")), PathBuf::from("/"));
        assert_eq!(normalize(Path::new("a/../..")), PathBuf::from(".."));
        assert_eq!(normalize(Path::new("./a/")), PathBuf::from("a"));
        assert_eq!(normalize(Path::new("")), PathBuf::from("."));
    }

    #[test]
    fn tenant_names() {
        for name in ["a", "dev", "A-b_9", &"x".repeat(64)] {
            assert!(valid_tenant_name(name), "{name}");
        }
        for name in ["", "-bad", "_bad", "a b", "é", &"x".repeat(65), "a.b"] {
            assert!(!valid_tenant_name(name), "{name}");
        }
    }

    #[test]
    fn toml_basic_strings() {
        assert_eq!(toml_string("plain"), "\"plain\"");
        assert_eq!(toml_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(
            toml_string("\u{8}\t\n\u{c}\r"),
            "\"\\b\\t\\n\\f\\r\"",
            "the five short escapes"
        );
        assert_eq!(
            toml_string("\u{0}\u{1b}\u{7f}\u{85}\u{9f}"),
            "\"\\u0000\\u001B\\u007F\\u0085\\u009F\""
        );
        assert_eq!(toml_string("é✓😀"), "\"é✓😀\"", "raw UTF-8");
        for text in ["", "a\"b", "\u{0}x\u{7f}\u{9f}\t", "é\\"] {
            let table = format!("k = {}\n", toml_string(text))
                .parse::<toml::Table>()
                .unwrap();
            assert_eq!(table["k"].as_str(), Some(text), "round trip {text:?}");
        }
    }

    #[test]
    fn index_schema_accepts_valid_files() {
        let index = Index::parse(
            b"version = 1\nactive = \"dev\"\n\n[tenants.dev]\ntsg_id = \"100\"\n\n[tenants.Prod-2]\ntsg_id = \" \"\n",
        )
        .unwrap();
        assert!(index.exists);
        assert_eq!(index.active.as_deref(), Some("dev"));
        assert_eq!(index.names(), ["Prod-2", "dev"], "byte order (D86)");
        assert_eq!(index.tsg("Prod-2"), Some(" "));
        assert_eq!(index.tsg("prod-2"), None, "exact lookups");
        assert_eq!(index.case_conflict("DEV"), Some("dev"));
        assert_eq!(index.case_conflict("dev"), None);
        assert_eq!(Index::parse(b"version = 1\n").unwrap().tenants.len(), 0);
        assert!(Index::parse(b"version = 1\ntenants = {}\n").is_some());
        assert!(Index::parse(b"version = 1\n[tenants]\n").is_some());
        assert!(
            Index::parse(b"version = 1\ntenants.\"a-b\".tsg_id = \"1\"\n").is_some(),
            "any TOML spelling of the same table"
        );
    }

    #[test]
    fn index_schema_rejections() {
        let thousand_and_one: String = std::iter::once("version = 1\n".to_owned())
            .chain((0..1001).map(|i| format!("[tenants.t{i}]\ntsg_id = \"1\"\n")))
            .collect();
        let cases: Vec<String> = vec![
            String::new(),
            "version = 2\n".into(),
            "version = \"1\"\n".into(),
            "version = 1.0\n".into(),
            "active = \"dev\"\n".into(),
            "version = 1\nextra = 1\n".into(),
            "version = 1\nactive = \"ghost\"\n".into(),
            "version = 1\nactive = 5\n".into(),
            "version = 1\ntenants = []\n".into(),
            "version = 1\n[tenants.dev]\n".into(),
            "version = 1\n[tenants.dev]\ntsg_id = \"\"\n".into(),
            "version = 1\n[tenants.dev]\ntsg_id = 100\n".into(),
            "version = 1\n[tenants.dev]\ntsg_id = \"1\"\nextra = 1\n".into(),
            "version = 1\n[tenants.\"-bad\"]\ntsg_id = \"1\"\n".into(),
            "version = 1\n[tenants.\"a b\"]\ntsg_id = \"1\"\n".into(),
            format!(
                "version = 1\n[tenants.{}]\ntsg_id = \"1\"\n",
                "x".repeat(65)
            ),
            "version = 1\n[tenants.dev]\ntsg_id = \"1\"\n[tenants.Dev]\ntsg_id = \"2\"\n".into(),
            "version = 1\nversion = 1\n".into(),
            "version = 1\n[tenants.dev]\ntsg_id = \"1\"\n[tenants.dev]\ntsg_id = \"1\"\n".into(),
            "version = 1\ntenants = 1\n".into(),
            "version = 1\n[tenants]\ndev = \"1\"\n".into(),
            "[unclosed\n".into(),
            thousand_and_one,
        ];
        for text in cases {
            assert!(Index::parse(text.as_bytes()).is_none(), "{text:.80}");
        }
        assert!(
            Index::parse(b"version = 1\nx = \"\xff\"\n").is_none(),
            "invalid UTF-8"
        );
        assert!(
            Index::parse(b"\xef\xbb\xbfversion = 1\n").is_some(),
            "the toml crate skips a leading BOM"
        );
    }

    #[test]
    fn index_canonical_bytes() {
        assert_eq!(Index::default().to_toml(), "version = 1\n");
        let mut index = Index::default();
        index.tenants.insert("prod".into(), "200".into());
        index.tenants.insert("dev".into(), "1\"0\\0".into());
        index.tenants.insert("B".into(), "\u{1b}".into());
        index.active = Some("dev".into());
        let bytes = index.to_toml();
        assert_eq!(
            bytes,
            "version = 1\nactive = \"dev\"\n\n[tenants.B]\ntsg_id = \"\\u001B\"\n\n[tenants.dev]\ntsg_id = \"1\\\"0\\\\0\"\n\n[tenants.prod]\ntsg_id = \"200\"\n"
        );
        let parsed = Index::parse(bytes.as_bytes()).unwrap();
        assert_eq!(parsed.tenants, index.tenants);
        assert_eq!(parsed.active, index.active);
    }

    #[test]
    fn metadata_rules() {
        let me = 501;
        assert!(dir_metadata_ok(true, me, 0o40700, me));
        assert!(dir_metadata_ok(true, me, 0o40755, me));
        assert!(!dir_metadata_ok(true, me, 0o40770, me), "group-writable");
        assert!(!dir_metadata_ok(true, me, 0o40702, me), "other-writable");
        assert!(!dir_metadata_ok(true, 0, 0o40700, me), "foreign owner");
        assert!(!dir_metadata_ok(false, me, 0o40700, me), "not a directory");
        assert!(file_metadata_ok(true, me, 0o100600, me));
        assert!(file_metadata_ok(true, me, 0o100400, me));
        for mode in [0o100640, 0o100604, 0o100610, 0o100601, 0o100660] {
            assert!(!file_metadata_ok(true, me, mode, me), "{mode:o}");
        }
        assert!(
            !file_metadata_ok(true, me + 1, 0o100600, me),
            "foreign owner"
        );
        assert!(
            !file_metadata_ok(false, me, 0o100600, me),
            "not a regular file"
        );
    }

    #[test]
    fn checked_reads() {
        let dir = TestDir::new("store-read");
        let ok = dir.write("ok", b"x", 0o600);
        assert_eq!(read_checked(&ok), Ok(b"x".to_vec()));
        let ro = dir.write("ro", b"y", 0o400);
        assert_eq!(read_checked(&ro), Ok(b"y".to_vec()), "readers accept 0400");
        for mode in [0o640, 0o604, 0o644] {
            let path = dir.write(&format!("m{mode:o}"), b"z", mode);
            assert_eq!(read_checked(&path), Err(FileError::Unsafe), "{mode:o}");
        }
        let link = dir.path().join("link");
        symlink(&ok, &link).unwrap();
        assert_eq!(read_checked(&link), Err(FileError::Unsafe), "symlink");
        assert_eq!(
            read_checked(dir.path()),
            Err(FileError::Unsafe),
            "directory"
        );
        assert_eq!(
            read_checked(&dir.path().join("missing")),
            Err(FileError::NotFound)
        );
        assert_eq!(
            read_checked(&dir.path().join("missing/deeper")),
            Err(FileError::NotFound)
        );
        let mut big = vec![b' '; MAX_FILE_BYTES as usize];
        let exact = dir.write("exact", &big, 0o600);
        assert_eq!(read_checked(&exact).map(|b| b.len()), Ok(big.len()));
        big.push(b' ');
        let over = dir.write("over", &big, 0o600);
        assert_eq!(read_checked(&over), Err(FileError::TooLarge));
        let unreadable = dir.write("unreadable", b"q", 0o200);
        if euid() != 0 {
            assert_eq!(read_checked(&unreadable), Err(FileError::Io));
        }
    }

    #[test]
    fn index_reads() {
        let root = TestDir::new("store-index");
        let store = Store::at(root.path().join("D"));
        assert_eq!(store.read_index(), Ok(Index::default()), "missing D");
        assert!(!store.dir().exists(), "reading creates nothing");
        let deeper = Store::at(root.path().join("x/y/D"));
        assert_eq!(deeper.read_index(), Ok(Index::default()));
        root.write("D/other", b"", 0o600);
        assert_eq!(store.read_index(), Ok(Index::default()), "missing file");
        root.write("D/config.toml", b"version = 1\n", 0o600);
        assert!(store.read_index().unwrap().exists);
        root.write("D/config.toml", b"version = 1\n", 0o644);
        assert_eq!(
            store.read_index().unwrap_err().message(),
            format!(
                "Refusing to use {}: it must be a regular file (not a symlink) owned by the current user with no group or other permissions (chmod 600)",
                store.index_path().display()
            )
        );
        root.write(
            "D/config.toml",
            b"version = 1\nsecret = \"SUPER-SECRET\"\n",
            0o600,
        );
        let message = store.read_index().unwrap_err().message();
        assert_eq!(
            message,
            format!(
                "Cannot read tenant registry at {}; restore a valid registry before proceeding",
                store.index_path().display()
            )
        );
        assert!(!message.contains("SUPER-SECRET"));
        std::fs::set_permissions(store.dir(), std::fs::Permissions::from_mode(0o770)).unwrap();
        assert_eq!(
            store.read_index().unwrap_err().message(),
            format!(
                "Refusing to use {}: it must be a directory (not a symlink) owned by the current user and not writable by group or others",
                store.dir().display()
            )
        );
        std::fs::set_permissions(store.dir(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let link = Store::at(root.path().join("L"));
        symlink(store.dir(), link.dir()).unwrap();
        assert!(
            link.read_index()
                .unwrap_err()
                .message()
                .starts_with("Refusing to use"),
            "a symlinked D is refused"
        );
    }

    #[test]
    fn tenant_file_reads() {
        let root = TestDir::new("store-tenant");
        let store = Store::at(root.path());
        assert_eq!(
            store.read_tenant_bytes("dev"),
            Err(TenantFileRead::NotFound)
        );
        root.write("tenants/dev.toml", b"mgmtTsgId = \"1\"\n", 0o600);
        assert_eq!(
            store.read_tenant_bytes("dev"),
            Ok(b"mgmtTsgId = \"1\"\n".to_vec())
        );
        assert_eq!(
            store.read_tenant_bytes("ghost"),
            Err(TenantFileRead::NotFound)
        );
        root.write("tenants/dev.toml", b"x", 0o660);
        assert_eq!(
            store.read_tenant_bytes("dev"),
            Err(TenantFileRead::Unsafe(store.tenant_path("dev")))
        );
        std::fs::set_permissions(store.tenants_dir(), std::fs::Permissions::from_mode(0o777))
            .unwrap();
        assert_eq!(
            store.read_tenant_bytes("dev"),
            Err(TenantFileRead::Unsafe(store.tenants_dir()))
        );
    }
}
