//! Running a CLI with stdin and stderr on a pseudo-terminal (DESIGN §10, M4t
//! "Interactive tests"): prompts, hidden input and the delete confirmation.
//!
//! The child gets the pty's slave side as stdin and stderr (both are terminals) and a
//! pipe as stdout. It is not made the pty's session leader (that needs `pre_exec`,
//! which is `unsafe`), so the terminal sends it no signals: Ctrl-C is simulated with
//! [`PtySession::signal`]. Everything the child writes to the terminal, and everything
//! the terminal echoes, is the session's transcript. Every runner invariant still
//! applies (the path guard, the timeout, the leak check, now also on the transcript,
//! and the unchanged-sandbox check).

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rustix::fs::{Mode, OFlags};
use rustix::pty::OpenptFlags;
use rustix::termios::{LocalModes, Termios};

use crate::runner::{Running, Terminal};
use crate::{Cli, Run, RunOptions, Sandbox};

/// How long [`PtySession::expect`] waits for text.
pub const EXPECT_TIMEOUT: Duration = Duration::from_secs(15);

/// A CLI running on a pty.
pub struct PtySession {
    running: Running,
    master: File,
    /// The parent's copy of the slave side: kept open so the terminal mode can be read
    /// after the child exits.
    slave: OwnedFd,
    transcript: Arc<Mutex<Vec<u8>>>,
    stop: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
    cursor: usize,
}

impl std::fmt::Debug for PtySession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtySession")
            .field("running", &self.running)
            .finish_non_exhaustive()
    }
}

/// The outcome of a pty run.
#[derive(Debug)]
pub struct PtyRun {
    /// Exit code, signal and stdout (stderr went to the terminal, so `run.stderr` is
    /// empty unless [`spawn_pty_with`] piped it).
    pub run: Run,
    /// Everything that appeared on the terminal (lossy UTF-8).
    pub transcript: String,
    /// The terminal mode after the child exited.
    pub termios_after: Option<Termios>,
}

impl PtyRun {
    /// Whether the terminal still echoes input after the run (a hidden prompt must
    /// restore it, also when cancelled).
    pub fn echo_restored(&self) -> bool {
        self.termios_after
            .as_ref()
            .is_some_and(|termios| termios.local_modes.contains(LocalModes::ECHO))
    }
}

fn open_pty() -> std::io::Result<(File, OwnedFd)> {
    let master = rustix::pty::openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)?;
    rustix::pty::grantpt(&master)?;
    rustix::pty::unlockpt(&master)?;
    let name = rustix::pty::ptsname(&master, Vec::new())?;
    let slave = rustix::fs::open(
        name.as_c_str(),
        OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let master = File::from(master);
    let flags = rustix::fs::fcntl_getfl(&master)?;
    rustix::fs::fcntl_setfl(&master, flags | OFlags::NONBLOCK)?;
    Ok((master, slave))
}

/// Start `cli` with stdin and stderr on a new pty (see the module docs). `options` work
/// as for [`crate::spawn`], except that `options.stdin` is replaced by the terminal.
///
/// # Panics
///
/// When no pty can be opened, or any [`crate::spawn`] guard refuses the run.
pub fn spawn_pty(cli: &Cli, sandbox: &Sandbox, args: &[&str], options: &RunOptions) -> PtySession {
    spawn_pty_with(cli, sandbox, args, options, true)
}

/// Like [`spawn_pty`], with only stdin on the pty when `stderr_on_pty` is false (stderr
/// is then piped and captured in `run.stderr`, so the prompt gate, which needs both
/// terminals, fails).
pub fn spawn_pty_with(
    cli: &Cli,
    sandbox: &Sandbox,
    args: &[&str],
    options: &RunOptions,
    stderr_on_pty: bool,
) -> PtySession {
    spawn_on_pty(cli, sandbox, args, options, stderr_on_pty, false)
}

/// Like [`spawn_pty`], with stdout on the pty as well (all three streams are the
/// terminal, so `run.stdout` stays empty and everything is in the transcript). The
/// TypeScript CLI asks for confirmation only when stdout is a terminal, so its D105
/// tests use this mode (DESIGN §10 M4).
pub fn spawn_pty_all(
    cli: &Cli,
    sandbox: &Sandbox,
    args: &[&str],
    options: &RunOptions,
) -> PtySession {
    spawn_on_pty(cli, sandbox, args, options, true, true)
}

fn spawn_on_pty(
    cli: &Cli,
    sandbox: &Sandbox,
    args: &[&str],
    options: &RunOptions,
    stderr_on_pty: bool,
    stdout_on_pty: bool,
) -> PtySession {
    let (master, slave) = open_pty().unwrap_or_else(|err| panic!("cannot open a pty: {err}"));
    let child_side = |fd: &OwnedFd| {
        Stdio::from(
            fd.try_clone()
                .unwrap_or_else(|err| panic!("cannot duplicate the pty: {err}")),
        )
    };
    let terminal = Terminal {
        stdin: child_side(&slave),
        stderr: if stderr_on_pty {
            child_side(&slave)
        } else {
            Stdio::piped()
        },
        stdout: stdout_on_pty.then(|| child_side(&slave)),
    };
    let transcript = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let mut reading = master
        .try_clone()
        .unwrap_or_else(|err| panic!("cannot duplicate the pty master: {err}"));
    let (buffer, stopping) = (Arc::clone(&transcript), Arc::clone(&stop));
    let reader = std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        let mut stopping_seen = false;
        loop {
            match reading.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => buffer
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .extend_from_slice(&chunk[..read]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    // Drain once more after the stop request: the child wrote
                    // everything before it exited.
                    if stopping_seen {
                        break;
                    }
                    stopping_seen = stopping.load(Ordering::SeqCst);
                    if !stopping_seen {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    });
    let running = crate::runner::spawn_with(cli, sandbox, args, options, Some(terminal));
    PtySession {
        running,
        master,
        slave,
        transcript,
        stop,
        reader: Some(reader),
        cursor: 0,
    }
}

impl PtySession {
    /// The child's pid.
    pub fn pid(&self) -> u32 {
        self.running.pid()
    }

    /// The transcript so far (lossy UTF-8).
    pub fn transcript(&self) -> String {
        String::from_utf8_lossy(
            &self
                .transcript
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
        .into_owned()
    }

    /// Wait until `text` appears in the transcript after the previous match, and move
    /// past it. A `\n` in `text` matches the terminal's `\r\n` (output post-processing
    /// turns every line feed into CR LF).
    ///
    /// # Panics
    ///
    /// After [`EXPECT_TIMEOUT`], showing the transcript.
    pub fn expect(&mut self, text: &str) {
        let started = Instant::now();
        loop {
            let found = {
                let bytes = self
                    .transcript
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                let needle = text.replace("\r\n", "\n").replace('\n', "\r\n");
                let needle = needle.as_bytes();
                bytes
                    .get(self.cursor..)
                    .unwrap_or_default()
                    .windows(needle.len().max(1))
                    .position(|window| window == needle)
                    .map(|index| self.cursor + index + needle.len())
            };
            if let Some(cursor) = found {
                self.cursor = cursor;
                return;
            }
            assert!(
                started.elapsed() < EXPECT_TIMEOUT,
                "pty: {text:?} did not appear; transcript so far: {:?}",
                self.transcript()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Type `bytes` into the terminal (`\n` is Enter, `\x04` is Ctrl-D).
    pub fn send(&mut self, bytes: &[u8]) {
        let mut written = 0;
        let started = Instant::now();
        while written < bytes.len() {
            match self.master.write(&bytes[written..]) {
                Ok(count) => written += count,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(started.elapsed() < EXPECT_TIMEOUT, "pty: input blocked");
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => panic!("pty: cannot type into the terminal: {error}"),
            }
        }
    }

    /// Send a signal by name (`INT` simulates Ctrl-C, see the module docs).
    pub fn signal(&self, name: &str) {
        self.running.send_signal(name);
    }

    /// Wait for the child (with the runner's timeout and invariants), then collect the
    /// transcript and the terminal mode. The transcript gets the runner's leak check.
    pub fn wait(mut self) -> PtyRun {
        // The terminal carries stdin and stderr: the stderr-only forms apply too.
        let mut secrets = self.running.secrets.clone();
        secrets.extend(self.running.stderr_secrets.iter().cloned());
        let run = self.running.wait();
        let termios_after = rustix::termios::tcgetattr(&self.slave).ok();
        self.stop.store(true, Ordering::SeqCst);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let transcript = String::from_utf8_lossy(
            &self
                .transcript
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
        .into_owned();
        for form in &secrets {
            assert!(
                !transcript.contains(&form.value),
                "{} {:?}: {} appeared on the terminal",
                run.cli,
                run.args,
                form.label
            );
        }
        PtyRun {
            run,
            transcript,
            termios_after,
        }
    }
}
