//! `checkpoint(state)`, the only way the state is written (DESIGN §14.10, D118):
//! invariants, a temporary created relative to the checked state directory, `fsync`,
//! the identity check of the file in place, `renameat`, and a directory `fsync` whose
//! failure fails the checkpoint.

use super::state::{JobState, LoadError};
use super::{Identity, MAX_STATE_BYTES, STATE_SUFFIX, StateDir, private_regular};
use crate::error::CliError;
use crate::store::{PROGRAM, shown};
use rustix::fs::{AtFlags, Mode, OFlags};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

/// The steps a unit test can make fail (§3.9 style).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    TempCreate,
    Write,
    Fsync,
    Rename,
    DirSync,
    /// The output CSV temporary (§14.12 rewrite step 1).
    OutputTemp,
    /// The output rename (§14.12 rewrite step 4).
    OutputRename,
}

#[cfg(test)]
thread_local! {
    static FAIL_AT: std::cell::Cell<Option<Step>> = const { std::cell::Cell::new(None) };
}

/// Make `step` fail in this thread from now on (tests only; `None` clears it).
#[cfg(test)]
pub fn fail_at(step: Option<Step>) {
    FAIL_AT.with(|cell| cell.set(step));
}

#[cfg(test)]
pub fn injected(step: Step) -> std::io::Result<()> {
    if FAIL_AT.with(std::cell::Cell::get) == Some(step) {
        Err(std::io::Error::other("injected failure"))
    } else {
        Ok(())
    }
}

#[cfg(not(test))]
#[inline]
pub fn injected(_: Step) -> std::io::Result<()> {
    Ok(())
}

/// What a failed checkpoint means for the user (§14.10 step 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Consequence {
    /// At setup, before a submission (C1), and after a C3 outcome other than accepted.
    NothingSubmitted,
    /// After an accepted receipt (C3).
    Accepted { ranges: String, scan_id: String },
    /// After results.
    Results,
}

impl Consequence {
    fn text(&self) -> String {
        match self {
            Self::NothingSubmitted => "no further prompts were submitted".to_owned(),
            Self::Accepted { ranges, scan_id } => format!(
                "prompts {ranges} were accepted under scan {scan_id}, but the job state could not be saved"
            ),
            Self::Results => {
                "results since the last save will be polled again by resume-poll".to_owned()
            }
        }
    }
}

/// `^\.<state name>\.[0-9]{1,10}\.[0-9a-f]{16}\.tmp$` (a checkpoint temporary).
pub fn is_stale_temp(name: &str, state_name: &str) -> bool {
    let Some(rest) = name
        .strip_prefix('.')
        .and_then(|rest| rest.strip_prefix(state_name))
        .and_then(|rest| rest.strip_prefix('.'))
        .and_then(|rest| rest.strip_suffix(".tmp"))
    else {
        return false;
    };
    let Some((pid, hex)) = rest.split_once('.') else {
        return false;
    };
    (1..=10).contains(&pid.len())
        && pid.bytes().all(|byte| byte.is_ascii_digit())
        && hex.len() == 16
        && hex
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// The §14.9 loading message of `error` for the state at `path`.
pub fn load_error(path: &std::path::Path, error: LoadError) -> CliError {
    let path = shown(path);
    CliError::operational(match error {
        LoadError::Malformed => format!("Invalid bulk-scan state {path}: malformed JSON"),
        LoadError::Newer => {
            format!(
                "{path} was written by a newer version of {PROGRAM}; upgrade {PROGRAM} to resume it"
            )
        }
        LoadError::Invalid(reason) => format!("Invalid bulk-scan state {path}: {reason}"),
    })
}

/// The state file of one job, written through its checked directory.
#[derive(Debug)]
pub struct StateFile {
    dir: StateDir,
    name: String,
    /// The device and inode this process last renamed into place (or loaded).
    identity: Option<Identity>,
}

impl StateFile {
    /// The state of job `job` in `dir` (not yet written).
    pub fn new(dir: StateDir, job: &str) -> Self {
        Self {
            dir,
            name: format!("{job}{STATE_SUFFIX}"),
            identity: None,
        }
    }

    pub fn dir(&self) -> &StateDir {
        &self.dir
    }

    /// `<canonical S>/<job>.bulk-scan.json`.
    pub fn path(&self) -> PathBuf {
        self.dir.path().join(&self.name)
    }

    /// The path as messages show it.
    pub fn shown(&self) -> String {
        shown(&self.path())
    }

    fn cannot_save(&self, consequence: &Consequence) -> CliError {
        CliError::operational(format!(
            "Cannot save {}; {}",
            self.shown(),
            consequence.text()
        ))
    }

    /// Write `state` durably (§14.10): invariants, `sequence += 1`, `updatedAt`, a
    /// fsynced temporary, the identity check of the file in place, `renameat`, and
    /// the directory fsync. On failure the temporary is removed and the in-memory
    /// state keeps its new sequence (the next checkpoint counts on).
    pub fn checkpoint(
        &mut self,
        state: &mut JobState,
        consequence: &Consequence,
    ) -> Result<(), CliError> {
        if state.validate().is_err() {
            return Err(self.cannot_save(consequence));
        }
        state.sequence += 1;
        state.updated_at = super::now_stamp();
        let bytes = state
            .to_bytes()
            .ok_or_else(|| self.cannot_save(consequence))?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            return Err(CliError::operational(format!(
                "Cannot save {}; the job state would exceed 128 MiB",
                self.shown()
            )));
        }
        let temp = format!(
            ".{}.{}.{}.tmp",
            self.name,
            std::process::id(),
            super::random_hex()
        );
        let dir = self.dir.fd();
        let written = (|| -> std::io::Result<Identity> {
            injected(Step::TempCreate)?;
            let fd = rustix::fs::openat(
                dir,
                temp.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )?;
            let mut file = File::from(fd);
            rustix::fs::fchmod(&file, Mode::from_raw_mode(0o600))?;
            injected(Step::Write)?;
            file.write_all(&bytes)?;
            injected(Step::Fsync)?;
            file.sync_all()?;
            Identity::of_file(&file)
        })();
        let remove_temp = || {
            let _ = rustix::fs::unlinkat(dir, temp.as_str(), AtFlags::empty());
        };
        let fresh = match written {
            Ok(identity) => identity,
            Err(_) => {
                remove_temp();
                return Err(self.cannot_save(consequence));
            }
        };
        // The file in place must be the one this process last wrote.
        let in_place = rustix::fs::statat(dir, self.name.as_str(), AtFlags::SYMLINK_NOFOLLOW);
        let ours = match in_place {
            Ok(stat) => private_regular(&stat) && Some(Identity::of_stat(&stat)) == self.identity,
            Err(rustix::io::Errno::NOENT) => self.identity.is_none(),
            Err(_) => false,
        };
        if !ours {
            remove_temp();
            return Err(CliError::operational(format!(
                "{} was changed by another program; the job was stopped",
                self.shown()
            )));
        }
        let renamed = injected(Step::Rename).and_then(|()| {
            Ok(rustix::fs::renameat(
                dir,
                temp.as_str(),
                dir,
                self.name.as_str(),
            )?)
        });
        if renamed.is_err() {
            remove_temp();
            return Err(self.cannot_save(consequence));
        }
        self.identity = Some(fresh);
        if !self.dir.sync() {
            return Err(self.cannot_save(consequence));
        }
        Ok(())
    }

    /// Read and validate the state of job `job` in `dir` (`resume-poll` under the job
    /// lock, §14.9 "Loading"): the §3.8 file rule on a descriptor opened relative to the
    /// checked directory, a bounded read, the strict parser, and the job id of the
    /// name. The loaded identity is what the next checkpoint expects in place.
    pub fn load(dir: StateDir, job: &str) -> Result<(Self, JobState), CliError> {
        let mut file = Self::new(dir, job);
        let path = file.path();
        let cannot_read = || {
            CliError::operational(format!(
                "Cannot read {} (missing, not a regular file, unreadable or larger than 128 MiB)",
                shown(&path)
            ))
        };
        let fd = file.dir.fd();
        let before = rustix::fs::statat(fd, file.name.as_str(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| cannot_read())?;
        if !private_regular(&before) {
            return Err(crate::store::unsafe_file_error(&path));
        }
        let opened = rustix::fs::openat(
            fd,
            file.name.as_str(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|errno| {
            if errno == rustix::io::Errno::LOOP {
                crate::store::unsafe_file_error(&path)
            } else {
                cannot_read()
            }
        })?;
        let handle = File::from(opened);
        let after = rustix::fs::fstat(&handle).map_err(|_| cannot_read())?;
        if !private_regular(&after) || Identity::of_stat(&after) != Identity::of_stat(&before) {
            return Err(crate::store::unsafe_file_error(&path));
        }
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(
            &mut std::io::Read::take(&handle, MAX_STATE_BYTES + 1),
            &mut bytes,
        )
        .map_err(|_| cannot_read())?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            return Err(cannot_read());
        }
        let state = super::state::parse(&bytes).map_err(|error| load_error(&path, error))?;
        if state.job_id != job {
            return Err(load_error(
                &path,
                LoadError::Invalid("jobId does not match the file name".into()),
            ));
        }
        file.identity = Some(Identity::of_stat(&after));
        Ok((file, state))
    }

    /// Remove the job's stale temporaries (`.<job>.bulk-scan.json.<pid>.<16 hex>.tmp`,
    /// regular and owned by the effective uid); only the lock holder writes them
    /// (§14.10). Returns how many were removed.
    pub fn sweep_temps(&self) -> usize {
        let Ok(entries) = std::fs::read_dir(self.dir.path()) else {
            return 0;
        };
        let fd = self.dir.fd();
        let mut removed = 0;
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if !is_stale_temp(name, &self.name) {
                continue;
            }
            if rustix::fs::statat(fd, name, AtFlags::SYMLINK_NOFOLLOW)
                .is_ok_and(|stat| private_regular(&stat))
                && rustix::fs::unlinkat(fd, name, AtFlags::empty()).is_ok()
            {
                removed += 1;
            }
        }
        removed
    }

    /// Remove the state file when it is still the one this process wrote (a failed
    /// setup, §14.4).
    pub fn remove_if_ours(&self) {
        let Some(identity) = self.identity else {
            return;
        };
        let dir = self.dir.fd();
        if rustix::fs::statat(dir, self.name.as_str(), AtFlags::SYMLINK_NOFOLLOW)
            .is_ok_and(|stat| Identity::of_stat(&stat) == identity)
        {
            let _ = rustix::fs::unlinkat(dir, self.name.as_str(), AtFlags::empty());
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::super::state::tests::sample;
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A private temporary state directory (removed when dropped).
    pub(crate) struct TempState {
        pub dir: tempfile_dir::Dir,
    }

    pub(crate) mod tempfile_dir {
        use std::path::{Path, PathBuf};

        pub struct Dir(PathBuf);

        impl Dir {
            pub fn new() -> Self {
                let base = std::env::temp_dir()
                    .canonicalize()
                    .unwrap()
                    .join(format!("airs-bulk-test-{}", uuid::Uuid::new_v4().simple()));
                std::fs::create_dir_all(&base).unwrap();
                Self(base)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    impl TempState {
        pub fn new() -> Self {
            Self {
                dir: tempfile_dir::Dir::new(),
            }
        }
        pub fn bulk_dir(&self) -> std::path::PathBuf {
            self.dir.path().join("airs-cli/bulk-scans")
        }
        pub fn state_file(&self, job: &str) -> StateFile {
            StateFile::new(StateDir::ensure(&self.bulk_dir()).unwrap(), job)
        }
        pub fn names(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(self.bulk_dir())
                .unwrap()
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    fn message(error: CliError) -> String {
        error.message()
    }

    /// state-spec #2, #3, #5: one `<job>.bulk-scan.json` in a created 0700 directory,
    /// rewritten in place (0600) with no temporary left.
    #[test]
    fn checkpoints_rewrite_one_private_file() {
        let temp = TempState::new();
        let mut state = sample();
        let mut file = temp.state_file(&state.job_id);
        for _ in 0..3 {
            file.checkpoint(&mut state, &Consequence::NothingSubmitted)
                .unwrap();
        }
        assert_eq!(state.sequence, 3);
        assert_eq!(temp.names(), [format!("{}.bulk-scan.json", state.job_id)]);
        let mode =
            |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&temp.bulk_dir()), 0o700);
        assert_eq!(mode(&file.path()), 0o600);
        let saved = super::super::state::parse(&std::fs::read(file.path()).unwrap()).unwrap();
        assert_eq!(saved, state);
    }

    /// state-spec #4: an invalid state is refused and nothing is written.
    #[test]
    fn an_invalid_state_writes_nothing() {
        let temp = TempState::new();
        let mut state = sample();
        state.batch_size = 0;
        let mut file = temp.state_file(&state.job_id);
        let error = file
            .checkpoint(&mut state, &Consequence::NothingSubmitted)
            .unwrap_err();
        assert_eq!(
            message(error),
            format!(
                "Cannot save {}; no further prompts were submitted",
                file.shown()
            )
        );
        assert!(temp.names().is_empty());
    }

    /// Every failing step leaves the previous state and no temporary, with the
    /// consequence of the checkpoint (§14.10 step 5).
    #[test]
    fn failed_steps_keep_the_previous_state() {
        let temp = TempState::new();
        let mut state = sample();
        let mut file = temp.state_file(&state.job_id);
        file.checkpoint(&mut state, &Consequence::NothingSubmitted)
            .unwrap();
        let before = std::fs::read(file.path()).unwrap();
        let accepted = Consequence::Accepted {
            ranges: "0-1".into(),
            scan_id: "S".into(),
        };
        for step in [Step::TempCreate, Step::Write, Step::Fsync, Step::Rename] {
            fail_at(Some(step));
            let error = file.checkpoint(&mut state, &accepted).unwrap_err();
            fail_at(None);
            assert_eq!(
                message(error),
                format!(
                    "Cannot save {}; prompts 0-1 were accepted under scan S, but the job state could not be saved",
                    file.shown()
                ),
                "{step:?}"
            );
            assert_eq!(std::fs::read(file.path()).unwrap(), before, "{step:?}");
            assert_eq!(temp.names().len(), 1, "{step:?}");
        }
        // A failed directory fsync fails the checkpoint after the rename.
        fail_at(Some(Step::DirSync));
        let error = file
            .checkpoint(&mut state, &Consequence::Results)
            .unwrap_err();
        fail_at(None);
        assert!(
            message(error)
                .ends_with("results since the last save will be polled again by resume-poll")
        );
        assert_ne!(std::fs::read(file.path()).unwrap(), before);
        // The next checkpoint accepts the file it renamed.
        file.checkpoint(&mut state, &Consequence::Results).unwrap();
    }

    /// state-spec #6 (the load half): a saved state loads back under its job id with the
    /// identity the next checkpoint expects; foreign, newer, malformed, invalid,
    /// renamed and unsafe files are refused with the §14.9 texts.
    #[test]
    fn loads_round_trip_and_refuse_foreign_files() {
        let temp = TempState::new();
        let mut state = sample();
        let job = state.job_id.clone();
        let mut file = temp.state_file(&job);
        file.checkpoint(&mut state, &Consequence::NothingSubmitted)
            .unwrap();
        let path = file.path();
        let dir = || StateDir::open(&temp.bulk_dir()).unwrap();
        let (mut loaded, again) = StateFile::load(dir(), &job).unwrap();
        assert_eq!(again, state);
        // The loaded identity is accepted by the next checkpoint.
        let mut again = again;
        loaded
            .checkpoint(&mut again, &Consequence::NothingSubmitted)
            .unwrap();
        let shown = path.display().to_string();
        let refused = |bytes: &[u8]| {
            std::fs::write(&path, bytes).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            StateFile::load(dir(), &job).unwrap_err().message()
        };
        assert_eq!(
            refused(b"{"),
            format!("Invalid bulk-scan state {shown}: malformed JSON")
        );
        assert_eq!(
            refused(br#"{"format":"airs-cli-rs.bulk-scan","version":9}"#),
            format!("{shown} was written by a newer version of airs; upgrade airs to resume it")
        );
        let mut renamed = sample();
        renamed.job_id = "20261017T120000Z-0000000000000009".into();
        assert_eq!(
            refused(&renamed.to_bytes().unwrap()),
            format!("Invalid bulk-scan state {shown}: jobId does not match the file name")
        );
        let mut broken = sample();
        broken.items[1].req_id = 5;
        assert_eq!(
            refused(&broken.to_bytes().unwrap()),
            format!("Invalid bulk-scan state {shown}: reqId must match the stable input index")
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            StateFile::load(dir(), &job)
                .unwrap_err()
                .message()
                .starts_with("Refusing to use ")
        );
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(temp.dir.path().join("elsewhere"), &path).unwrap();
        assert!(
            StateFile::load(dir(), &job)
                .unwrap_err()
                .message()
                .starts_with("Refusing to use ")
        );
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            StateFile::load(dir(), &job).unwrap_err().message(),
            format!(
                "Cannot read {shown} (missing, not a regular file, unreadable or larger than 128 MiB)"
            )
        );
    }

    /// §14.10: the stale temporaries of the job (and only those) are swept.
    #[test]
    fn stale_temporaries_of_the_job_are_swept() {
        let temp = TempState::new();
        let mut state = sample();
        let job = state.job_id.clone();
        let mut file = temp.state_file(&job);
        file.checkpoint(&mut state, &Consequence::NothingSubmitted)
            .unwrap();
        let name = format!("{job}{STATE_SUFFIX}");
        let dir = temp.bulk_dir();
        let stale = [
            format!(".{name}.123.0123456789abcdef.tmp"),
            format!(".{name}.4294967295.fedcba9876543210.tmp"),
        ];
        let kept = [
            format!(".{name}.123.0123456789ABCDEE.tmp"),
            format!(".{name}.12345678901.0123456789abcdef.tmp"),
            format!(".{name}..0123456789abcdef.tmp"),
            ".20261017T120000Z-0000000000000001.bulk-scan.json.1.0123456789abcdef.tmp".to_owned(),
            format!("{name}.1.0123456789abcdef.tmp"),
        ];
        for name in stale.iter().chain(&kept) {
            std::fs::write(dir.join(name), "x").unwrap();
            std::fs::set_permissions(dir.join(name), std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }
        let public = format!(".{name}.7.0000000000000000.tmp");
        std::fs::write(dir.join(&public), "x").unwrap();
        std::fs::set_permissions(dir.join(&public), std::fs::Permissions::from_mode(0o644))
            .unwrap();
        assert_eq!(file.sweep_temps(), 2);
        for name in &stale {
            assert!(!dir.join(name).exists(), "{name}");
        }
        for name in kept.iter().chain([&public]) {
            assert!(dir.join(name).exists(), "{name}");
        }
        assert!(file.path().exists());
        assert!(is_stale_temp(&stale[0], &name));
        assert!(!is_stale_temp(&name, &name));
    }

    /// A state replaced by another program stops the job (§14.10 step 3).
    #[test]
    fn a_foreign_state_stops_the_job() {
        let temp = TempState::new();
        let mut state = sample();
        let mut file = temp.state_file(&state.job_id);
        file.checkpoint(&mut state, &Consequence::NothingSubmitted)
            .unwrap();
        let path = file.path();
        let copy = path.with_extension("copy");
        std::fs::copy(&path, &copy).unwrap();
        std::fs::rename(&copy, &path).unwrap();
        let error = file
            .checkpoint(&mut state, &Consequence::NothingSubmitted)
            .unwrap_err();
        assert_eq!(
            message(error),
            format!(
                "{} was changed by another program; the job was stopped",
                file.shown()
            )
        );
        assert_eq!(temp.names().len(), 1, "the temporary is removed");
        // A removed state is a change too.
        std::fs::remove_file(&path).unwrap();
        assert!(
            file.checkpoint(&mut state, &Consequence::NothingSubmitted)
                .is_err()
        );
        // A fresh job finds no file and writes one.
        let mut other = temp.state_file("20261017T120000Z-0000000000000001");
        let mut fresh = sample();
        fresh.job_id = "20261017T120000Z-0000000000000001".into();
        other
            .checkpoint(&mut fresh, &Consequence::NothingSubmitted)
            .unwrap();
        other.remove_if_ours();
        assert!(!other.path().exists());
    }
}
