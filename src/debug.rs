//! `--debug` (DESIGN §2.9): the CLI's half of the API log. It picks the directory,
//! creates this run's private file, keeps the newest [`KEEP`] logs, and sends the one
//! write-failure warning to stderr. The record format, sanitization, token holds and
//! the observing transport are airs-runtime-core's (`airs_runtime_core::debug`).

pub use airs_runtime_core::debug::*;

use crate::error::{CliError, SecretSet};
use std::ffi::OsStr;
use std::fs::{self, DirBuilder, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Whether a command writes the log under `--debug` (the planning table, DESIGN §2.9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugPolicy {
    /// The command can send requests: `--debug` opens a log.
    Log,
    /// The command sends nothing (`tenant *`): `--debug` is accepted and ignored.
    None,
    /// The command refuses `--debug` during planning (the environment report, §15.17,
    /// D156), so no log is ever opened for it.
    Refused,
}

/// How many logs the directory keeps, this run's included.
pub const KEEP: usize = 20;

/// The log directory: `$XDG_STATE_HOME/airs-cli/debug` (set and non-empty; it must be
/// absolute), else `<home>/.local/state/airs-cli/debug` (the home must be absolute).
pub fn directory(xdg_state_home: Option<&OsStr>, home: Option<&Path>) -> Result<PathBuf, CliError> {
    if let Some(state) = xdg_state_home.filter(|value| !value.is_empty()) {
        let state = Path::new(state);
        if !state.is_absolute() {
            return Err(CliError::operational(
                "XDG_STATE_HOME must be an absolute path",
            ));
        }
        return Ok(crate::store::normalize(&state.join("airs-cli/debug")));
    }
    match home.filter(|home| home.is_absolute()) {
        Some(home) => Ok(crate::store::normalize(
            &home.join(".local/state/airs-cli/debug"),
        )),
        None => Err(CliError::operational(
            "Cannot create the debug log: HOME is not an absolute path; set XDG_STATE_HOME or omit --debug",
        )),
    }
}

/// The setup failure naming `dir` (the OS error is never echoed; the renderer makes
/// the path terminal-safe).
fn setup_failed(dir: &Path) -> CliError {
    CliError::operational(format!(
        "Cannot create the debug log in {}; fix that directory or omit --debug",
        dir.display()
    ))
}

/// `debug-api-<epoch ms>-<8 hex>.jsonl` (the TS pattern).
pub fn file_name(epoch_ms: u128, hex: &str) -> String {
    format!("debug-api-{epoch_ms}-{hex}.jsonl")
}

/// The timestamp of a log name matching `^debug-api-[0-9]{13}-[0-9a-f]{8}\.jsonl$`.
pub fn log_timestamp(name: &str) -> Option<u64> {
    let rest = name.strip_prefix("debug-api-")?.strip_suffix(".jsonl")?;
    let (stamp, hex) = rest.split_once('-')?;
    let lower_hex = |b: u8| b.is_ascii_digit() || (b'a'..=b'f').contains(&b);
    if stamp.len() == 13
        && stamp.bytes().all(|b| b.is_ascii_digit())
        && hex.len() == 8
        && hex.bytes().all(lower_hex)
    {
        stamp.parse().ok()
    } else {
        None
    }
}

/// Keep `own` plus the [`KEEP`]` - 1` newest other logs in `dir` (by the timestamp in
/// the name, then by name, never by mtime) and remove the rest. Only regular files with
/// log names are counted or touched; every error is ignored.
pub fn prune(dir: &Path, own: &str) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut logs: Vec<(u64, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let stamp = log_timestamp(&name)?;
            let metadata = fs::symlink_metadata(entry.path()).ok()?;
            metadata.file_type().is_file().then_some((stamp, name))
        })
        .collect();
    logs.sort_by(|a, b| b.cmp(a));
    let mut kept = 1;
    for (_, name) in logs {
        if name == own {
            continue;
        }
        if kept < KEEP {
            kept += 1;
        } else {
            let _ = fs::remove_file(dir.join(&name));
        }
    }
}

/// Create `dir` (0700 for each component created; an existing directory is never
/// chmod'ed), check the leaf, create this run's file exclusively at 0600, check its
/// owner, then prune. Any failure is the setup error naming `dir`.
fn create_file(dir: &Path, name: &str) -> Result<(PathBuf, fs::File), CliError> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|_| setup_failed(dir))?;
    let metadata = fs::symlink_metadata(dir).map_err(|_| setup_failed(dir))?;
    if !metadata.file_type().is_dir() || metadata.mode() & 0o022 != 0 {
        return Err(setup_failed(dir));
    }
    let path = dir.join(name);
    let file = OpenOptions::new()
        .append(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|_| setup_failed(dir))?;
    let owned = file
        .metadata()
        .is_ok_and(|file_metadata| file_metadata.uid() == metadata.uid());
    if !owned {
        drop(file);
        let _ = fs::remove_file(&path);
        return Err(setup_failed(dir));
    }
    prune(dir, name);
    Ok((path, file))
}

/// Create the directory and this run's file, then prune old logs. `secrets` is the
/// invocation's set (shared: leases registered later are seen by the line check);
/// `warning_color` is the stderr colour decision.
pub fn open(dir: &Path, secrets: SecretSet, warning_color: bool) -> Result<DebugLog, CliError> {
    let epoch_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    let file = create_file(dir, &file_name(epoch_ms, &uuid[..8]))?;
    Ok(DebugLog::with_sinks(
        file.0,
        Box::new(file.1),
        Box::new(std::io::stderr()),
        warning_color,
        secrets,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderMap;
    use reqwest::{Method, StatusCode};
    use std::os::unix::fs::PermissionsExt;
    use url::Url;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn directory_resolution() {
        let home = Path::new("/home/u");
        assert_eq!(
            directory(Some(OsStr::new("/state//x/")), Some(home)).unwrap(),
            Path::new("/state/x/airs-cli/debug")
        );
        assert_eq!(
            directory(None, Some(home)).unwrap(),
            Path::new("/home/u/.local/state/airs-cli/debug")
        );
        assert_eq!(
            directory(Some(OsStr::new("")), Some(home)).unwrap(),
            Path::new("/home/u/.local/state/airs-cli/debug"),
            "an empty XDG_STATE_HOME counts as unset"
        );
        assert_eq!(
            directory(Some(OsStr::new("rel/state")), Some(home))
                .unwrap_err()
                .message(),
            "XDG_STATE_HOME must be an absolute path"
        );
        for home in [None, Some(Path::new("")), Some(Path::new("rel"))] {
            assert_eq!(
                directory(None, home).unwrap_err().message(),
                "Cannot create the debug log: HOME is not an absolute path; set XDG_STATE_HOME or omit --debug"
            );
        }
    }

    #[test]
    fn log_names() {
        assert_eq!(
            file_name(1_792_062_224_500, "0a1b2c3d"),
            "debug-api-1792062224500-0a1b2c3d.jsonl"
        );
        assert_eq!(
            log_timestamp("debug-api-1792062224500-0a1b2c3d.jsonl"),
            Some(1_792_062_224_500)
        );
        for name in [
            "debug-api-179206222450-0a1b2c3d.jsonl",
            "debug-api-1792062224500-0A1B2C3D.jsonl",
            "debug-api-1792062224500-0a1b2c3.jsonl",
            "debug-api-1792062224500-0a1b2c3d.json",
            "xdebug-api-1792062224500-0a1b2c3d.jsonl",
            "debug-api-1792062224500-0a1b2c3d.jsonl.bak",
            "notes.txt",
        ] {
            assert_eq!(log_timestamp(name), None, "{name}");
        }
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "airs-debug-test-{label}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn retention_keeps_the_newest_twenty_including_this_run() {
        let dir = temp_dir("prune");
        let name = |stamp: u64, hex: &str| file_name(u128::from(stamp), hex);
        // 24 old logs; their mtimes disagree with their names (newest name written first).
        for index in (0..24u64).rev() {
            fs::write(dir.join(name(1_700_000_000_000 + index, "aaaaaaaa")), b"").unwrap();
        }
        // Ties on the timestamp are ordered by name.
        fs::write(dir.join(name(1_700_000_000_023, "bbbbbbbb")), b"").unwrap();
        // This run's file is the oldest by name and is still kept.
        let own = name(1_600_000_000_000, "cccccccc");
        fs::write(dir.join(&own), b"").unwrap();
        // Never touched or counted.
        fs::write(dir.join("notes.txt"), b"keep").unwrap();
        fs::create_dir(dir.join(name(1_000_000_000_000, "dddddddd"))).unwrap();
        std::os::unix::fs::symlink("notes.txt", dir.join(name(1_000_000_000_001, "eeeeeeee")))
            .unwrap();
        fs::write(dir.join("debug-api-1-x.jsonl"), b"").unwrap();
        prune(&dir, &own);
        let remaining = names(&dir);
        let logs: Vec<&String> = remaining
            .iter()
            .filter(|name| {
                log_timestamp(name).is_some()
                    && dir.join(name).is_file()
                    && !dir.join(name).is_symlink()
            })
            .collect();
        assert_eq!(logs.len(), KEEP);
        assert!(remaining.contains(&own));
        assert!(remaining.contains(&name(1_700_000_000_023, "bbbbbbbb")));
        assert!(remaining.contains(&name(1_700_000_000_023, "aaaaaaaa")));
        // 19 others kept: the tie pair (023) plus 006..=022.
        assert!(remaining.contains(&name(1_700_000_000_006, "aaaaaaaa")));
        assert!(!remaining.contains(&name(1_700_000_000_005, "aaaaaaaa")));
        assert!(!remaining.contains(&name(1_700_000_000_000, "aaaaaaaa")));
        for kept in [
            "notes.txt".to_owned(),
            "debug-api-1-x.jsonl".to_owned(),
            name(1_000_000_000_000, "dddddddd"),
            name(1_000_000_000_001, "eeeeeeee"),
        ] {
            assert!(remaining.contains(&kept), "{kept}");
        }
        // A second prune (another run converging) removes nothing more.
        prune(&dir, &own);
        assert_eq!(names(&dir), remaining);
        // A missing directory is ignored.
        prune(&dir.join("missing"), &own);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn open_creates_private_files_and_refuses_unsafe_directories() {
        let root = temp_dir("open");
        let dir = root.join("a/b/airs-cli/debug");
        let log = open(&dir, SecretSet::default(), false).unwrap();
        let mode = |path: &Path| fs::symlink_metadata(path).unwrap().mode() & 0o777;
        assert_eq!(mode(&dir), 0o700 & !umask_bits(&root));
        assert_eq!(mode(&root.join("a")), 0o700 & !umask_bits(&root));
        assert_eq!(mode(log.path()), 0o600);
        let name = log.path().file_name().unwrap().to_str().unwrap().to_owned();
        assert!(log_timestamp(&name).is_some(), "{name}");
        log.begin(
            &Method::GET,
            &url("http://h.example/"),
            &HeaderMap::new(),
            "op",
        )
        .response(StatusCode::OK, &HeaderMap::new());
        assert_eq!(fs::read_to_string(log.path()).unwrap().lines().count(), 1);

        // An existing name (or a symlink at the path) is never opened.
        assert_eq!(
            create_file(&dir, &name).err().unwrap().message(),
            format!(
                "Cannot create the debug log in {}; fix that directory or omit --debug",
                dir.display()
            )
        );
        let target = root.join("target");
        let linked = file_name(1_000_000_000_000, "abcdef01");
        std::os::unix::fs::symlink(&target, dir.join(&linked)).unwrap();
        assert!(create_file(&dir, &linked).is_err());
        assert!(!target.exists(), "the symlink was not followed");

        // An existing 0755 directory is accepted and not chmod'ed.
        let open_dir = root.join("shared");
        fs::create_dir(&open_dir).unwrap();
        fs::set_permissions(&open_dir, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(open(&open_dir, SecretSet::default(), false).is_ok());
        assert_eq!(mode(&open_dir), 0o755);

        let refused = |dir: &Path| {
            let error = open(dir, SecretSet::default(), false).err().unwrap();
            assert_eq!(
                error.message(),
                format!(
                    "Cannot create the debug log in {}; fix that directory or omit --debug",
                    dir.display()
                )
            );
        };
        let writable = root.join("world");
        fs::create_dir(&writable).unwrap();
        fs::set_permissions(&writable, fs::Permissions::from_mode(0o777)).unwrap();
        refused(&writable);
        assert_eq!(
            names(&writable).len(),
            0,
            "nothing created in a refused dir"
        );
        let group = root.join("group");
        fs::create_dir(&group).unwrap();
        fs::set_permissions(&group, fs::Permissions::from_mode(0o770)).unwrap();
        refused(&group);
        let link = root.join("link");
        std::os::unix::fs::symlink(&dir, &link).unwrap();
        refused(&link);
        let file = root.join("plain-file");
        fs::write(&file, b"").unwrap();
        refused(&file);
        let read_only = root.join("read-only");
        fs::create_dir(&read_only).unwrap();
        fs::set_permissions(&read_only, fs::Permissions::from_mode(0o500)).unwrap();
        refused(&read_only);
        refused(&read_only.join("below"));
        fs::set_permissions(&read_only, fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    /// The process umask, observed through a directory created with 0777.
    fn umask_bits(root: &Path) -> u32 {
        let probe = root.join(format!("umask-{}", uuid::Uuid::new_v4().simple()));
        DirBuilder::new().mode(0o777).create(&probe).unwrap();
        let bits = !(fs::metadata(&probe).unwrap().mode() & 0o777) & 0o777;
        fs::remove_dir(&probe).unwrap();
        bits
    }
}
