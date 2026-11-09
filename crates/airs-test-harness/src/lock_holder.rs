//! A process that holds the airs-cli-rs store lock (DESIGN §3.9, §10 "Lock tests").
//!
//! A test binary runs itself as the holder: an `#[ignore]`d test calls
//! [`run_from_env`], which does nothing unless [`ENV_STORE`] is set, and the test spawns
//! it with [`spawn`]. The holder takes `flock(LOCK_EX)` on `<store>/config.lock` exactly
//! as the CLI does (creating the file 0600), writes the §3.9 holder record with its own
//! pid and host, prints [`READY`] and sleeps for [`ENV_HOLD_MS`] (default 60 s) before
//! releasing. Killing it releases the lock at once (the kernel drops a dead process's
//! `flock`) and leaves its record in the file.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustix::fs::{FlockOperation, Mode, OFlags};

/// The store directory the holder locks.
pub const ENV_STORE: &str = "AIRS_HARNESS_LOCK_STORE";
/// Any lock file the holder locks instead (the M5 job lock, `<S>/<job>.lock`).
pub const ENV_FILE: &str = "AIRS_HARNESS_LOCK_FILE";
/// How long the holder keeps the lock, in milliseconds.
pub const ENV_HOLD_MS: &str = "AIRS_HARNESS_LOCK_HOLD_MS";
/// The line the holder prints once it holds the lock.
pub const READY: &str = "LOCK-HELD";

/// `YYYY-MM-DDTHH:MM:SSZ` for `time` (UTC).
pub fn utc_seconds(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    let (days, rest) = ((seconds / 86_400) as i64, seconds % 86_400);
    // Civil from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The nodename, as the CLI records it.
pub fn host_name() -> String {
    rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .into_owned()
}

/// Take the store lock in this process (blocking) and write the holder record. The
/// lock is held until the returned file is dropped.
pub fn hold(store: &Path) -> std::io::Result<File> {
    hold_file(&store.join("config.lock"))
}

/// Take `flock(LOCK_EX)` on the lock file at `path` (created 0600 when missing) and
/// write the holder record, as the CLI does for any of its lock files.
pub fn hold_file(path: &Path) -> std::io::Result<File> {
    let fd = rustix::fs::open(
        path,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )?;
    let file = File::from(fd);
    rustix::fs::fchmod(&file, Mode::from_raw_mode(0o600))?;
    rustix::fs::flock(&file, FlockOperation::LockExclusive)?;
    rustix::fs::ftruncate(&file, 0)?;
    let record = format!(
        "pid={}\nhost={}\nsince={}\n",
        std::process::id(),
        host_name(),
        utc_seconds(SystemTime::now())
    );
    rustix::io::pwrite(&file, record.as_bytes(), 0)?;
    Ok(file)
}

/// The holder's body, for an `#[ignore]`d helper test: without [`ENV_STORE`] it returns
/// `false` at once.
pub fn run_from_env() -> bool {
    let target = match (std::env::var_os(ENV_FILE), std::env::var_os(ENV_STORE)) {
        (Some(file), _) => std::path::PathBuf::from(file),
        (None, Some(store)) => Path::new(&store).join("config.lock"),
        (None, None) => return false,
    };
    let hold_for = std::env::var(ENV_HOLD_MS)
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map_or(Duration::from_secs(60), Duration::from_millis);
    let lock = hold_file(&target).unwrap_or_else(|err| panic!("lock holder: {err}"));
    println!("{READY}");
    let _ = std::io::stdout().flush();
    std::thread::sleep(hold_for);
    drop(lock);
    true
}

/// A running holder process; dropping it kills the process.
#[derive(Debug)]
pub struct Holder {
    child: Child,
}

impl Holder {
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// `pid <pid> on <host> since ` (the start of the holder text the CLI prints).
    pub fn holder_prefix(&self) -> String {
        format!("pid {} on {} since ", self.child.id(), host_name())
    }

    /// SIGKILL the holder and reap it.
    pub fn kill(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Wait for the holder to release (its hold time ended) and exit.
    pub fn wait(mut self) {
        let _ = self.child.wait();
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Run the current test binary's helper test `helper` (its full name, for `--exact`)
/// as a holder of `store`'s lock for `hold_for`, and return once it holds the lock.
///
/// # Panics
///
/// When the helper exits before holding the lock.
pub fn spawn(helper: &str, store: &Path, hold_for: Duration) -> Holder {
    spawn_with(helper, (ENV_STORE, store), hold_for)
}

/// [`spawn`] for any lock file (`path`), such as a bulk-scan job lock.
pub fn spawn_file(helper: &str, path: &Path, hold_for: Duration) -> Holder {
    spawn_with(helper, (ENV_FILE, path), hold_for)
}

fn spawn_with(helper: &str, (name, target): (&str, &Path), hold_for: Duration) -> Holder {
    let exe = std::env::current_exe().unwrap_or_else(|err| panic!("lock holder: {err}"));
    let mut child = Command::new(exe)
        .args([
            "--exact",
            helper,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(name, target)
        .env(ENV_HOLD_MS, hold_for.as_millis().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|err| panic!("lock holder: cannot start: {err}"));
    let stdout = child.stdout.take().expect("piped stdout");
    let mut lines = BufReader::new(stdout).lines();
    loop {
        match lines.next() {
            Some(Ok(line)) if line.contains(READY) => break,
            Some(Ok(_)) => {}
            _ => panic!("lock holder exited before it held the lock"),
        }
    }
    // Keep reading in the background so the helper never blocks on a full pipe.
    std::thread::spawn(move || for _ in lines {});
    Holder { child }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_takes_the_flock_and_writes_the_record() {
        let dir = tempfile::tempdir().expect("tempdir");
        let lock = hold(dir.path()).expect("hold");
        let path = dir.path().join("config.lock");
        let record = std::fs::read_to_string(&path).expect("record");
        let lines: Vec<&str> = record.lines().collect();
        assert_eq!(lines[0], format!("pid={}", std::process::id()));
        assert_eq!(lines[1], format!("host={}", host_name()));
        assert!(
            lines[2].starts_with("since=") && lines[2].ends_with('Z'),
            "{record}"
        );
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).expect("meta").permissions().mode() & 0o777,
            0o600
        );
        // flock locks belong to open file descriptions: a second open conflicts.
        let other = File::open(&path).expect("open");
        assert!(rustix::fs::flock(&other, FlockOperation::NonBlockingLockExclusive).is_err());
        drop(lock);
        // A child forked by a concurrent test can hold the description until it execs.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while let Err(error) = rustix::fs::flock(&other, FlockOperation::NonBlockingLockExclusive) {
            assert!(std::time::Instant::now() < deadline, "released: {error:?}");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn utc_formatting() {
        assert_eq!(utc_seconds(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        assert_eq!(
            utc_seconds(UNIX_EPOCH + Duration::from_secs(1_792_000_000)),
            "2026-10-14T17:46:40Z"
        );
        assert_eq!(
            utc_seconds(UNIX_EPOCH + Duration::from_secs(951_782_400)),
            "2000-02-29T00:00:00Z"
        );
    }
}
