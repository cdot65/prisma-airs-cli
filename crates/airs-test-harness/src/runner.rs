//! Spawning `airs-cli-rs` or the TypeScript reference CLI inside a [`Sandbox`].

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::compare::{error_line, http_status};
use crate::sandbox::{Sandbox, SecretForm, Snapshot, secret_forms};

/// Default per-run timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Which executable to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cli {
    /// The native binary (normally `env!("CARGO_BIN_EXE_airs")`).
    Rust(PathBuf),
}

impl Cli {
    /// The native binary at `path`.
    pub fn rust(path: impl Into<PathBuf>) -> Self {
        Self::Rust(path.into())
    }

    /// The label used in failure messages.
    pub fn label(&self) -> &'static str {
        "airs-cli-rs"
    }

    pub(crate) fn command(&self) -> Command {
        match self {
            Self::Rust(path) => Command::new(path),
        }
    }
}

/// `<crate>/../..`, the workspace root.
pub fn workspace_root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.canonicalize().unwrap_or(root)
}

/// Per-run options.
#[derive(Clone, Debug)]
pub struct RunOptions {
    /// Extra or overriding environment variables.
    pub env: Vec<(String, String)>,
    /// Variables removed after the allowlist and `env` were applied.
    pub remove_env: Vec<String>,
    /// Kill the child and panic after this long.
    pub timeout: Duration,
    /// Allow death by signal (a signalled run reports `code = 128 + signal`).
    pub expect_signal: bool,
    /// Check stdout/stderr for fixture secrets (default on).
    pub check_secrets: bool,
    /// Additional strings that must not appear in output.
    pub extra_secrets: Vec<String>,
    /// Read at most this many stdout bytes, then close the pipe (the child's next write
    /// gets `EPIPE`); `None` drains stdout to EOF.
    pub stdout_limit: Option<usize>,
    /// Do not read stdout until the child has exited (a stalled reader: once the pipe
    /// buffer is full the child's writes block).
    pub stall_stdout: bool,
    /// Do not read stderr until [`Running::release_stderr`] or [`Running::wait`] (a
    /// stalled reader: once the pipe buffer is full the child's stderr writes block, so a
    /// signal can be sent while the child is between two known steps).
    pub stall_stderr: bool,
    /// Start the child with its stderr pipe already full (see [`RunOptions::fill_stderr`]).
    pub fill_stderr: bool,
    /// Check that the run changed no file in the sandbox (default on; see
    /// [`RunOptions::allow_tenant_writes`]).
    pub check_unchanged: bool,
    /// Sandbox-relative paths under which the unchanged check ignores changes (see
    /// [`RunOptions::allow_writes_under`]).
    pub writable: Vec<PathBuf>,
    /// The child's stdin (default `/dev/null`; see [`RunOptions::stdin`] and
    /// [`RunOptions::stdin_held_open`]).
    pub stdin: StdinSource,
}

/// What a child reads on stdin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum StdinSource {
    /// `/dev/null` (the harness default).
    #[default]
    Null,
    /// A pipe that receives these bytes and is then closed.
    Bytes(Vec<u8>),
    /// A pipe that stays open, without data, until the child exits (a CLI waiting for
    /// stdin can then be signalled).
    HeldOpen,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            env: Vec::new(),
            remove_env: Vec::new(),
            timeout: DEFAULT_TIMEOUT,
            expect_signal: false,
            check_secrets: true,
            extra_secrets: Vec::new(),
            stdout_limit: None,
            stall_stdout: false,
            stall_stderr: false,
            fill_stderr: false,
            check_unchanged: true,
            writable: Vec::new(),
            stdin: StdinSource::Null,
        }
    }
}

impl RunOptions {
    /// Default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds (or overrides) an environment variable.
    pub fn env(mut self, name: &str, value: impl Into<String>) -> Self {
        self.env.push((name.to_owned(), value.into()));
        self
    }

    /// Removes an environment variable (including allowlisted ones).
    pub fn without_env(mut self, name: &str) -> Self {
        self.remove_env.push(name.to_owned());
        self
    }

    /// Sets the timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Allows the child to die by a signal.
    pub fn expect_signal(mut self) -> Self {
        self.expect_signal = true;
        self
    }

    /// Disables the secret leak check (for tests that assert on output themselves).
    pub fn allow_secret_output(mut self) -> Self {
        self.check_secrets = false;
        self
    }

    /// Adds a string that must not appear in output (its raw, trimmed, URL-encoded and
    /// base64 forms are all checked, like [`Sandbox::add_secret`]).
    pub fn secret(mut self, value: &str) -> Self {
        self.extra_secrets.push(value.to_owned());
        self
    }

    /// Never reads stdout while the child runs (see [`RunOptions::stall_stdout`]).
    pub fn stall_stdout(mut self) -> Self {
        self.stall_stdout = true;
        self
    }

    /// Names the airs-cli-rs signal marker ([`SIGNAL_MARKER_VAR`]); `path` must not exist
    /// yet and lies inside the sandbox (the path guard checks it). See
    /// [`Running::send_signal_recorded`].
    pub fn signal_marker(self, path: &Path) -> Self {
        self.env(SIGNAL_MARKER_VAR, path.display().to_string())
    }

    /// Reads stderr only once released (see [`RunOptions::stall_stderr`]).
    pub fn stall_stderr(mut self) -> Self {
        self.stall_stderr = true;
        self
    }

    /// [`RunOptions::stall_stderr`] on a pipe that is already full when the child starts
    /// (the runner fills it with its own bytes, which the captured stderr leaves out): the
    /// child's *first* stderr write blocks until [`Running::release_stderr`]. A command that
    /// writes nothing to stderr before a known step is then held right after that step,
    /// however little it writes there (a stalled reader alone holds it only once its writes
    /// exceed the pipe buffer). Not for [`crate::spawn_pty`] runs.
    pub fn fill_stderr(mut self) -> Self {
        self.stall_stderr = true;
        self.fill_stderr = true;
        self
    }

    /// **Opt-out** of the per-run check that the sandbox (registry, configs, `home/`,
    /// `cwd/` and every other file outside `tmp/`) is byte-, mode- and mtime-identical
    /// after the run. Only for a test that changes those files itself while the CLI runs.
    pub fn allow_tenant_writes(mut self) -> Self {
        self.check_unchanged = false;
        self
    }

    /// A narrow exception to the unchanged-sandbox check: changes at or under the
    /// sandbox-relative `path` (for example `home/.local/state/airs-cli`, the default
    /// `--debug` log directory, or `cwd`, where the TypeScript CLI writes its log) are
    /// ignored. Everything else is still checked.
    pub fn allow_writes_under(mut self, path: impl Into<PathBuf>) -> Self {
        self.writable.push(path.into());
        self
    }

    /// Pipes `bytes` to the child's stdin, then closes it (instead of `/dev/null`).
    pub fn stdin(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.stdin = StdinSource::Bytes(bytes.into());
        self
    }

    /// Gives the child a stdin pipe that stays open (no data, no EOF) until it exits.
    pub fn stdin_held_open(mut self) -> Self {
        self.stdin = StdinSource::HeldOpen;
        self
    }

    /// Reads only the first `bytes` stdout bytes (fewer at EOF), then closes the read end
    /// so the child's further writes fail with `EPIPE` (broken-pipe tests). `Run::stdout`
    /// holds the bytes read; every other invariant is unchanged.
    pub fn close_stdout_after(mut self, bytes: usize) -> Self {
        self.stdout_limit = Some(bytes);
        self
    }
}

/// The outcome of one CLI invocation.
#[derive(Clone)]
pub struct Run {
    /// Which CLI ran (`airs-cli-rs` / `airs-cli`).
    pub cli: &'static str,
    /// Arguments passed.
    pub args: Vec<String>,
    /// Exit code (`128 + signal` when killed by an expected signal).
    pub code: i32,
    /// Terminating signal, when any.
    pub signal: Option<i32>,
    /// Captured stdout (lossy UTF-8).
    pub stdout: String,
    /// Captured stderr (lossy UTF-8).
    pub stderr: String,
    /// Wall time.
    pub elapsed: Duration,
}

impl std::fmt::Debug for Run {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Run")
            .field("cli", &self.cli)
            .field("args", &self.args)
            .field("code", &self.code)
            .field("signal", &self.signal)
            .field("stdout", &self.stdout)
            .field("stderr", &self.stderr)
            .field("elapsed", &self.elapsed)
            .finish()
    }
}

impl Run {
    /// Panics (showing both streams; they passed the leak check) unless the exit code
    /// is `expected`.
    pub fn assert_code(&self, expected: i32) -> &Self {
        assert_eq!(
            self.code, expected,
            "{} {:?}: exit code {} (expected {expected})\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.cli, self.args, self.code, self.stdout, self.stderr
        );
        self
    }

    /// `assert_code(0)`.
    pub fn assert_success(&self) -> &Self {
        self.assert_code(0)
    }

    /// stdout parsed as JSON (panics when it is not JSON).
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|err| {
            panic!(
                "{} {:?}: stdout is not JSON ({err}):\n{}",
                self.cli, self.args, self.stdout
            )
        })
    }

    /// The text after `✗ Error: ` on stderr.
    pub fn error_line(&self) -> Option<String> {
        error_line(&self.stderr)
    }

    /// The status from an `HTTP <n>` stderr line.
    pub fn http_status(&self) -> Option<u16> {
        http_status(&self.stderr)
    }
}

/// Runs `cli` with `args` in `sandbox` and waits for it (see [`spawn`]).
pub fn run(cli: &Cli, sandbox: &Sandbox, args: &[&str], options: &RunOptions) -> Run {
    spawn(cli, sandbox, args, options).wait()
}

/// Starts `cli` with stdin `/dev/null`, cwd `sandbox.cwd()` and exactly this
/// environment: `PATH` (inherited, without the entries that hold an `airs`, `airs-cli`
/// or `airs-cli-rs`, [`path_without_programs`]), `HOME=<sandbox>/home`, `TMPDIR=<sandbox>/tmp`,
/// `PRISMA_AIRS_TENANTS_PATH=<sandbox>/state/tenants.json`,
/// `AIRS_CLI_CONFIG_DIR=<sandbox>/store`, `NO_COLOR=1`,
/// `NO_PROXY=127.0.0.1,localhost`, then `options.env`, minus `options.remove_env`.
///
/// # Panics
///
/// Before spawning, when a path variable or an absolute-path argument points outside
/// the sandbox ([`escape_violation`]), and, for airs-cli-rs, when a tenant file in the
/// store it would read fails the Rust loopback guard
/// ([`Sandbox::assert_store_safe_for_rust`]).
///
/// For the TypeScript CLI, unless [`Sandbox::assert_safe_for_ts`] accepts the sandbox
/// under this environment: no config written with an `*_allowing_external_endpoints`
/// opt-out, and every loadable config points every endpoint key at loopback.
pub fn spawn(cli: &Cli, sandbox: &Sandbox, args: &[&str], options: &RunOptions) -> Running {
    spawn_with(cli, sandbox, args, options, None)
}

/// Whether `args` run a `tenant` subcommand (the first positional argument, after the
/// global options and their values, is `tenant`). Tenant commands of either CLI never
/// build an API client, so no request can leave the host, and the loopback-endpoint
/// guards on configs and tenant files do not apply to them (a CLI-created tenant has
/// only credentials). The path guards always apply.
pub fn is_tenant_command(args: &[&str]) -> bool {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--output" | "--tenant" => {
                args.next();
            }
            "--" => return false,
            flag if flag.starts_with('-') => {}
            positional => return positional == "tenant",
        }
    }
    false
}

/// The history driver's `runtime history` leaves are local archive operations. All
/// path/mutation guards still apply; only API endpoint admission is unnecessary for them.
fn is_history_command(args: &[&str]) -> bool {
    let mut positionals = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--output" | "--tenant" => {
                args.next();
            }
            "--" => return false,
            flag if flag.starts_with('-') => {}
            value => {
                positionals.push(value);
                if positionals.len() == 2 {
                    break;
                }
            }
        }
    }
    positionals == ["runtime", "history"]
}

/// The history store's test driver (`airs-history-driver runtime sync`) writes its
/// archive under the default data directory; the runner keeps it under tmp. Since M1 the
/// `airs` CLI writes no history, so `runtime scan`/`bulk-scan`/`resume-poll` run with the
/// sandbox's own data directory and its unchanged-sandbox checks.
fn automatic_history_command(args: &[&str]) -> bool {
    matches!(positionals(args).as_slice(), ["runtime", "sync", ..])
}

/// The child's stdin and stderr when a pty supplies them ([`crate::pty`]).
pub(crate) struct Terminal {
    pub stdin: Stdio,
    pub stderr: Stdio,
    /// `Some` puts stdout on the terminal too ([`crate::pty::spawn_pty_all`]).
    pub stdout: Option<Stdio>,
}

/// The TypeScript SDK endpoint variables (every `process.env.PANW_*ENDPOINT*` name in
/// the pinned SDK bundle). For a TypeScript `runtime` run the runner points each one the
/// test did not set at the trap ([`Sandbox::ts_env_trap_url`]): `topics create`,
/// `apply` and `revert` ignore the tenant's `mgmtEndpoint` and would otherwise fall back
/// to production (D93, DESIGN §10 M4).
pub const TS_ENDPOINT_VARIABLES: [&str; 15] = [
    "PANW_AGENT_GUARD_DATA_ENDPOINT",
    "PANW_AGENT_GUARD_MGMT_ENDPOINT",
    "PANW_AI_GW_ADMIN_ENDPOINT",
    "PANW_AI_GW_DATA_ENDPOINT",
    "PANW_AI_GW_INFERENCE_ENDPOINT",
    "PANW_AI_SEC_API_ENDPOINT",
    "PANW_IAM_ENDPOINT",
    "PANW_MGMT_ENDPOINT",
    "PANW_MGMT_TOKEN_ENDPOINT",
    "PANW_MODEL_SEC_DATA_ENDPOINT",
    "PANW_MODEL_SEC_MGMT_ENDPOINT",
    "PANW_MODEL_SEC_TOKEN_ENDPOINT",
    "PANW_RED_TEAM_DATA_ENDPOINT",
    "PANW_RED_TEAM_MGMT_ENDPOINT",
    "PANW_RED_TEAM_NETWORK_BROKER_ENDPOINT",
];

/// Whether `args` run a `redteam` subcommand (the first positional argument).
pub fn is_redteam_command(args: &[&str]) -> bool {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--output" | "--tenant" => {
                args.next();
            }
            "--" => return false,
            flag if flag.starts_with('-') => {}
            positional => return positional == "redteam",
        }
    }
    false
}

/// Whether `args` run a `runtime dlp` subcommand (the first two positional arguments).
pub fn is_dlp_command(args: &[&str]) -> bool {
    let mut positionals = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--output" | "--tenant" => {
                args.next();
            }
            "--" => break,
            flag if flag.starts_with('-') => {}
            positional => {
                positionals.push(positional);
                if positionals.len() == 2 {
                    break;
                }
            }
        }
    }
    positionals == ["runtime", "dlp"]
}

/// Whether `args` run a `model-security` subcommand (the first positional argument).
pub fn is_model_security_command(args: &[&str]) -> bool {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--output" | "--tenant" => {
                args.next();
            }
            "--" => return false,
            flag if flag.starts_with('-') => {}
            positional => return positional == "model-security",
        }
    }
    false
}

/// Whether `args` run a `gateway` subcommand (the first positional argument, DESIGN §10
/// M6e).
pub fn is_gateway_command(args: &[&str]) -> bool {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--output" | "--tenant" => {
                args.next();
            }
            "--" => return false,
            flag if flag.starts_with('-') => {}
            positional => return positional == "gateway" || positional == "aigateway",
        }
    }
    false
}

/// Whether `args` run a `supplychain` command (or, for TypeScript, `agentguard`; DESIGN
/// §10 M6f).
pub fn is_supplychain_command(args: &[&str]) -> bool {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--output" | "--tenant" => {
                args.next();
            }
            "--" => return false,
            flag if flag.starts_with('-') => {}
            positional => return positional == "supplychain" || positional == "agentguard",
        }
    }
    false
}

/// The `runtime` groups of the M6d runtime extras (DESIGN §10 M6d).
const RUNTIME_EXTRAS: [&str; 7] = [
    "api-keys",
    "customer-apps",
    "dashboard",
    "deployment-profiles",
    "report",
    "scan-logs",
    "sessions",
];

/// Whether `args` run an M6d runtime extras command (`runtime <group> …`, the first two
/// positional arguments).
pub fn is_runtime_extras_command(args: &[&str]) -> bool {
    let mut positionals = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--output" | "--tenant" => {
                args.next();
            }
            "--" => break,
            flag if flag.starts_with('-') => {}
            positional => {
                positionals.push(positional);
                if positionals.len() == 2 {
                    break;
                }
            }
        }
    }
    matches!(positionals.as_slice(), ["runtime", group] if RUNTIME_EXTRAS.contains(group))
}

/// Whether `args` run a `runtime` subcommand (the first positional argument).
pub fn is_runtime_command(args: &[&str]) -> bool {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--output" | "--tenant" => {
                args.next();
            }
            "--" => return false,
            flag if flag.starts_with('-') => {}
            positional => return positional == "runtime",
        }
    }
    false
}

pub(crate) fn spawn_with(
    cli: &Cli,
    sandbox: &Sandbox,
    args: &[&str],
    options: &RunOptions,
    terminal: Option<Terminal>,
) -> Running {
    let mut env = child_env(sandbox, options);
    // Automatic archive writes remain under tmp; every existing mutation,
    // endpoint and path guard still applies. Explicit controls are never replaced.
    if automatic_history_command(args)
        && !options.env.iter().any(|(key, _)| key == "XDG_DATA_HOME")
        && env.contains_key("XDG_DATA_HOME")
    {
        env.insert(
            "XDG_DATA_HOME".into(),
            sandbox
                .tmp()
                .join("history-data")
                .to_string_lossy()
                .into_owned(),
        );
    }
    if let Some(problem) = escape_violation(sandbox.root(), &sandbox.cwd(), &env, args) {
        panic!("refusing to run {} {args:?}: {problem}", cli.label());
    }
    if let Some(problem) = uv_hook_violation(&env) {
        panic!("refusing to run {} {args:?}: {problem}", cli.label());
    }
    // A `--fetch-uv` run without the loopback hook would download from github.com.
    if args.contains(&"--fetch-uv")
        && !args.contains(&"--dry-run")
        && !env.contains_key("AIRS_CLI_TEST_UV_ARCHIVE_URL")
    {
        panic!(
            "refusing to run {} {args:?}: --fetch-uv needs AIRS_CLI_TEST_UV_ARCHIVE_URL (the fixture's archive)",
            cli.label()
        );
    }
    if let Some(problem) = import_source_violation(sandbox.root(), &sandbox.cwd(), &env, args) {
        panic!("refusing to run {} {args:?}: {problem}", cli.label());
    }
    let guarded = !is_tenant_command(args) && !is_history_command(args);
    if let Some(store) = rust_store_dir(&env, &sandbox.cwd()) {
        sandbox.assert_store_safe_for_rust_with(&store, guarded);
    }
    let mut full_stderr = None;
    let mut command = cli.command();
    command
        .args(args)
        .env_clear()
        .envs(&env)
        .current_dir(sandbox.cwd())
        .stdout(Stdio::piped());
    match terminal {
        Some(Terminal {
            stdin,
            stderr,
            stdout,
        }) => {
            assert!(
                !options.fill_stderr,
                "RunOptions::fill_stderr needs a piped stderr, not a terminal"
            );
            command.stdin(stdin).stderr(stderr);
            if let Some(stdout) = stdout {
                command.stdout(stdout);
            }
        }
        None => {
            command.stdin(match options.stdin {
                StdinSource::Null => Stdio::null(),
                StdinSource::Bytes(_) | StdinSource::HeldOpen => Stdio::piped(),
            });
            if options.fill_stderr {
                let (reader, writer, filler) = full_pipe();
                command.stderr(writer);
                full_stderr = Some((reader, filler));
            } else {
                command.stderr(Stdio::piped());
            }
        }
    }
    let before = options.check_unchanged.then(|| sandbox.full_snapshot());
    let started = Instant::now();
    let spawn_deadline = started + options.timeout.min(Duration::from_secs(1));
    let mut child = spawn_ready(|| command.spawn(), spawn_deadline)
        .unwrap_or_else(|err| panic!("cannot start {} {args:?}: {err}", cli.label()));
    // The command holds the parent's copy of a filled stderr's write end: closing it lets
    // the reader see the end of the stream once the child exits.
    drop(command);
    let mut held_stdin = None;
    if let Some(mut pipe) = child.stdin.take() {
        match &options.stdin {
            StdinSource::Bytes(bytes) => {
                let bytes = bytes.clone();
                // A writer thread: a child that never reads cannot block the harness.
                std::thread::spawn(move || {
                    let _ = pipe.write_all(&bytes);
                });
            }
            StdinSource::HeldOpen => held_stdin = Some(pipe),
            StdinSource::Null => {}
        }
    }
    let (release, gate) = if options.stall_stdout {
        let (release, gate) = std::sync::mpsc::channel();
        (Some(release), Some(gate))
    } else {
        (None, None)
    };
    let stdout = drain(child.stdout.take(), options.stdout_limit, gate);
    let (stderr_release, stderr_gate) = if options.stall_stderr {
        let (release, gate) = std::sync::mpsc::channel();
        (Some(release), Some(gate))
    } else {
        (None, None)
    };
    let stderr_seen = Arc::new(Mutex::new(Vec::new()));
    let (stderr_pipe, filler): (Option<Box<dyn Read + Send>>, usize) = match full_stderr {
        Some((reader, filler)) => (Some(Box::new(reader)), filler),
        None => (
            child
                .stderr
                .take()
                .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
            0,
        ),
    };
    let stderr = drain_seen(stderr_pipe, Arc::clone(&stderr_seen), stderr_gate, filler);
    let debug_dirs =
        crate::debug_log::candidate_dirs(sandbox, env.get("XDG_STATE_HOME").map(String::as_str));
    let mut secrets = if options.check_secrets {
        sandbox.secret_forms()
    } else {
        Vec::new()
    };
    if options.check_secrets {
        for value in &options.extra_secrets {
            secrets.extend(secret_forms("test-supplied secret", value));
        }
        // airs-cli-rs redacts every red team credential (D140); the TypeScript CLI prints
        // them, so the markers are checked for airs-cli-rs runs only.
        if is_redteam_command(args) {
            for marker in crate::fixtures::redteam::markers() {
                secrets.extend(secret_forms("red team fixture credential", &marker));
            }
        }
    }
    // DLP detection content may reach stdout where a read prints the record (DESIGN
    // §16.9; the tests assert where), never stderr or the debug log of an airs-cli-rs run.
    let mut stderr_secrets = Vec::new();
    if options.check_secrets && is_dlp_command(args) {
        for marker in crate::fixtures::dlp::markers() {
            stderr_secrets.extend(secret_forms("DLP fixture content", &marker));
        }
    }
    // Model-security sensitive values (label values, field values, URI userinfo and
    // queries) may reach stdout only where §17.15 prints them (the tests assert where),
    // never stderr or the debug log of an airs-cli-rs run (DESIGN §10 M6c).
    if options.check_secrets && is_model_security_command(args) {
        for marker in crate::fixtures::modelsec::markers() {
            stderr_secrets.extend(secret_forms("model-security fixture value", &marker));
        }
        // The PyPI password never appears anywhere outside an explicit `--url-file` file
        // (D194): not on stdout, stderr or in the debug log.
        secrets.push(SecretForm {
            label: "PyPI fixture credential".into(),
            value: crate::fixtures::modelsec::PYPI_MARKER.into(),
        });
    }
    // Runtime extras (DESIGN §10 M6d): an API key value never appears in any output of an
    // airs-cli-rs run; scan content only on `scan-content --show-content`'s stdout; auth
    // codes only on stdout.
    if options.check_secrets && is_runtime_extras_command(args) {
        let raw = |label: &str, value: &str| SecretForm {
            label: format!("{label} (prefix)"),
            value: value.to_owned(),
        };
        secrets.push(raw("API key marker", crate::fixtures::runtime::KEY_MARKER));
        for tsg in ["100", "200"] {
            for marker in crate::fixtures::runtime::key_markers(tsg) {
                secrets.extend(secret_forms("API key fixture value", &marker));
            }
        }
        let shows_content = args.contains(&"scan-content") && args.contains(&"--show-content");
        for marker in crate::fixtures::runtime::CONTENT_MARKERS {
            let form = raw("scan content marker", marker);
            if shows_content {
                stderr_secrets.push(form);
            } else {
                secrets.push(form);
            }
        }
        stderr_secrets.push(raw(
            "auth code marker",
            crate::fixtures::runtime::AUTHCODE_MARKER,
        ));
    }
    // Gateway credentials (DESIGN §10 M6e): a `FAKE-GW-SECRET-*` marker never appears in
    // any output or debug log of an airs-cli-rs run (TypeScript prints some, D238).
    if options.check_secrets && is_gateway_command(args) {
        for marker in crate::fixtures::gateway::markers() {
            secrets.extend(secret_forms("gateway fixture credential", &marker));
        }
    }
    // Supplychain (DESIGN §10 M6f, §20.11): a `PRIVATE-` marker never appears in any output
    // or debug log of an airs-cli-rs run; with `--include-content` the finding content may
    // reach stdout only (the tests assert where), never stderr or the debug log.
    if options.check_secrets && is_supplychain_command(args) {
        let form = SecretForm {
            label: "supplychain fixture marker (prefix)".into(),
            value: crate::fixtures::supplychain::MARKER.into(),
        };
        if args.contains(&"--include-content") {
            stderr_secrets.push(form);
        } else {
            secrets.push(form);
        }
    }
    Running {
        child,
        started,
        stdout,
        stderr,
        stderr_seen,
        release,
        stderr_release,
        unchanged: before.map(|before| (sandbox.root().to_path_buf(), before)),
        options: options.clone(),
        secrets,
        stderr_secrets,
        debug_dirs,
        cli: cli.label(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        held_stdin,
    }
}

/// The `--fetch-uv` test hook (DESIGN §17.14) may only name a loopback http URL: a run
/// whose `AIRS_CLI_TEST_UV_ARCHIVE_URL` points elsewhere is refused.
pub fn uv_hook_violation(env: &BTreeMap<String, String>) -> Option<String> {
    let url = env.get("AIRS_CLI_TEST_UV_ARCHIVE_URL")?;
    let loopback = (url.starts_with("http://127.0.0.1:") || url.starts_with("http://[::1]:"))
        && crate::sandbox::is_loopback_url(url);
    (!loopback).then(|| "AIRS_CLI_TEST_UV_ARCHIVE_URL must be the fixture's loopback URL".into())
}

/// Variables that name a path; a child's value must stay inside the sandbox.
pub const PATH_VARIABLES: [&str; 10] = [
    "HOME",
    "TMPDIR",
    "AIRS_CLI_CONFIG_DIR",
    "XDG_CONFIG_HOME",
    "XDG_STATE_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "PRISMA_AIRS_TENANTS_PATH",
    SIGNAL_MARKER_VAR,
    "AIRS_CLI_TEST_HISTORY_BATCH_MARKER",
];

/// The airs-cli-rs signal thread creates the file this variable names once it has
/// recorded a signal (`src/signals.rs`, DESIGN §2.5): the synchronisation point of
/// [`Running::send_signal_recorded`].
pub const SIGNAL_MARKER_VAR: &str = "AIRS_CLI_TEST_SIGNAL_MARKER";

/// Lexical normalisation (`.`, `..`, duplicate and trailing `/`), never above `/`.
pub(crate) fn lexical(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    let mut depth = 0usize;
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if depth > 0 {
                    out.pop();
                    depth -= 1;
                } else if !out.has_root() {
                    out.push("..");
                }
            }
            Component::Normal(name) => {
                out.push(name);
                depth += 1;
            }
        }
    }
    out
}

/// Why a child environment or argument list could reach a path outside the sandbox
/// (DESIGN §10 "Escape prevention"), or `None`.
///
/// - Every [`PATH_VARIABLES`] value that is set and non-empty must resolve (absolute,
///   or relative to `cwd`; `~` in `PRISMA_AIRS_TENANTS_PATH` against `HOME`) inside
///   `root`, lexically normalised.
/// - `HOME` may be removed (the CLIs would then use the password database, the real
///   home) only when `AIRS_CLI_CONFIG_DIR` or `XDG_CONFIG_HOME` and
///   `PRISMA_AIRS_TENANTS_PATH` (without `~`) or `XDG_STATE_HOME` are set, and, for a
///   `--debug`, `runtime bulk-scan` or `runtime resume-poll` run, `XDG_STATE_HOME`
///   (a TypeScript bulk run with `HOME` removed is refused, [`ts_bulk_home_violation`]).
/// - The `runtime resume-poll` state argument must lie inside `root`, a relative one
///   resolved against `cwd`.
/// - Every argument that is an absolute path (`/…`, or `--flag=/…`) must lie inside
///   `root`, and so must every value of a [`PATH_FLAGS`] flag, relative ones resolved
///   against `cwd`.
pub fn escape_violation(
    root: &Path,
    cwd: &Path,
    env: &BTreeMap<String, String>,
    args: &[&str],
) -> Option<String> {
    escape_violation_within(&[root], cwd, env, args)
}

/// The flags whose value is a file the CLIs read or write: `tenant import --from` and
/// `--from-config` (airs-cli-rs), `tenant create --config` (TypeScript), the M4
/// `--config` and `topics eval --prompts` inputs, `topics sample --output-file`, the
/// M5 `bulk-scan --file`/`--input` inputs and `--output-file` outputs, and the M6a red
/// team files (`--output-dir`, `--input-dir`, `--goals`, `--script-file`,
/// `--variables-file`), the M6b DLP inputs (`--body-file`, `--metadata-file`; the
/// keyword `--file` is already here), the M6c `--url-file` and `--dir`, the M6d
/// `--key-file` (also the M6e integration credential input) and the M6e `--secret-output`
/// (DESIGN §10).
pub const PATH_FLAGS: [&str; 22] = [
    "--from",
    "--from-config",
    "--config",
    "--prompts",
    "--output-file",
    "--file",
    "--input",
    "--output-dir",
    "--input-dir",
    "--goals",
    "--script-file",
    "--variables-file",
    "--body-file",
    "--metadata-file",
    "--url-file",
    "--dir",
    "--key-file",
    "--secret-output",
    "--from-evidence",
    "--bundle-dir",
    "--review-file",
    "--history-dir",
];

/// [`escape_violation`] where a path may lie inside any of `roots` (the live tier's
/// session sandbox and its target sandbox).
pub fn escape_violation_within(
    roots: &[&Path],
    cwd: &Path,
    env: &BTreeMap<String, String>,
    args: &[&str],
) -> Option<String> {
    let set = |name: &str| env.get(name).filter(|value| !value.is_empty());
    let inside = |path: &Path| {
        let path = lexical(&cwd.join(path));
        roots.iter().any(|root| path.starts_with(root))
    };
    for name in PATH_VARIABLES {
        let Some(value) = set(name) else {
            continue;
        };
        let path = match (name, value.strip_prefix('~')) {
            ("PRISMA_AIRS_TENANTS_PATH", Some(rest))
                if rest.is_empty() || rest.starts_with('/') =>
            {
                match env.get("HOME") {
                    Some(home) => PathBuf::from(format!("{home}{rest}")),
                    None => return Some(format!("{name} uses ~ while HOME is unset")),
                }
            }
            _ => PathBuf::from(value),
        };
        if !inside(&path) {
            return Some(format!("{name} points outside the sandbox"));
        }
    }
    if !env.contains_key("HOME") {
        if (is_history_command(args) || automatic_history_command(args))
            && !env.contains_key("XDG_DATA_HOME")
            && !args
                .iter()
                .any(|arg| *arg == "--history-dir" || arg.starts_with("--history-dir="))
        {
            return Some("HOME and XDG_DATA_HOME are removed; local history needs an explicit sandbox --history-dir".into());
        }
        let store = set("AIRS_CLI_CONFIG_DIR")
            .or(set("XDG_CONFIG_HOME"))
            .is_some();
        let registry = set("PRISMA_AIRS_TENANTS_PATH")
            .or(set("XDG_STATE_HOME"))
            .is_some();
        // `--debug` logs and the bulk-scan state go under the state home.
        let state_home = !(args.contains(&"--debug") || bulk_command(args).is_some())
            || set("XDG_STATE_HOME").is_some();
        if !(store && registry && state_home) {
            return Some(
                "HOME is removed, so the CLI would resolve the real home directory; set \
                 AIRS_CLI_CONFIG_DIR, PRISMA_AIRS_TENANTS_PATH (and XDG_STATE_HOME for \
                 --debug, bulk-scan and resume-poll) inside the sandbox"
                    .into(),
            );
        }
    }
    // The `resume-poll` state argument (read to classify it), relative ones resolved
    // against `cwd`.
    if let Some(state) = resume_state_argument(args)
        && !inside(Path::new(state))
    {
        return Some(format!(
            "the resume-poll state {state} points outside the sandbox"
        ));
    }
    for arg in args {
        let value = match arg.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => value,
            _ => arg,
        };
        if value.starts_with('/') && !inside(Path::new(value)) {
            return Some(format!(
                "argument {value} is an absolute path outside the sandbox"
            ));
        }
    }
    // The `redteam prompt-sets upload <uuid> <file>` argument, relative ones resolved
    // against `cwd`.
    if let Some(file) = upload_file_argument(args)
        && !inside(Path::new(file))
    {
        return Some(format!("the upload file {file} points outside the sandbox"));
    }
    // The `runtime dlp restore <file>` argument (M6b), relative ones resolved against
    // `cwd`.
    if let Some(file) = restore_file_argument(args)
        && !inside(Path::new(file))
    {
        return Some(format!(
            "the restore file {file} points outside the sandbox"
        ));
    }
    // A relative value of a path flag, resolved against `cwd` (every occurrence).
    for flag in PATH_FLAGS {
        if let Some(value) = path_flag_values(args, flag)
            .into_iter()
            .find(|value| !inside(Path::new(value)))
        {
            return Some(format!("{flag} {value} points outside the sandbox"));
        }
    }
    None
}

/// The positional arguments of `args`, skipping the values of the options that take
/// one (the global `--output` and `--tenant`, the [`PATH_FLAGS`], the bulk-scan options
/// and the DLP backup and restore options), up to a literal `--` (after which every
/// argument is positional).
fn positionals<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let mut found = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match *arg {
            "--" => {
                found.extend(iter.copied());
                break;
            }
            "--output" | "--tenant" | "--profile" | "--session-id" | "--batch-size"
            | "--name-prefix" | "--expect-tsg" | "--pattern-map" | "--on-conflict"
            | "--max-pages" | "--resources" | "--file-format" | "--dlp-map"
            | "--on-missing-dlp" => {
                iter.next();
            }
            flag if PATH_FLAGS.contains(&flag) => {
                iter.next();
            }
            flag if flag.starts_with('-') => {}
            positional => found.push(positional),
        }
    }
    found
}

/// `Some("bulk-scan")` or `Some("resume-poll")` when `args` run one of the M5 commands
/// (`runtime bulk-scan`, `runtime resume-poll`), which keep their job state under the
/// state home (airs-cli-rs) or the tenant's `dataDir` (TypeScript, `~` by default).
pub fn bulk_command(args: &[&str]) -> Option<&'static str> {
    match positionals(args).as_slice() {
        ["runtime", "bulk-scan", ..] => Some("bulk-scan"),
        ["runtime", "resume-poll", ..] => Some("resume-poll"),
        _ => None,
    }
}

/// The `<file>` argument of a `redteam prompt-sets upload` run.
pub fn upload_file_argument<'a>(args: &[&'a str]) -> Option<&'a str> {
    match positionals(args).as_slice() {
        ["redteam", "prompt-sets", "upload", _, file, ..] => Some(file),
        _ => None,
    }
}

/// The `<file>` argument of a `runtime dlp restore` or `runtime profiles restore` run.
pub fn restore_file_argument<'a>(args: &[&'a str]) -> Option<&'a str> {
    match positionals(args).as_slice() {
        ["runtime", "dlp" | "profiles", "restore", file, ..] => Some(file),
        _ => None,
    }
}

/// The `<stateFile>` argument of a `runtime resume-poll` run.
pub fn resume_state_argument<'a>(args: &[&'a str]) -> Option<&'a str> {
    match positionals(args).as_slice() {
        ["runtime", "resume-poll", state, ..] => Some(state),
        _ => None,
    }
}

/// Whether `args` run `tenant import` (the positional arguments start `tenant import`).
pub fn is_import_command(args: &[&str]) -> bool {
    let mut positionals = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--output" | "--tenant" | "--from" | "--from-config" | "--name" => {
                args.next();
            }
            "--" => break,
            flag if flag.starts_with('-') => {}
            positional => positionals.push(positional),
        }
        if positionals.len() == 2 {
            break;
        }
    }
    positionals == ["tenant", "import"]
}

/// Why an airs-cli-rs `tenant import` run could read a TypeScript file outside the
/// sandbox, or `None` (DESIGN §10 "Escape prevention"). The source it would read
/// (`--from <path>`, `--from-config <path>`, else the registry `env` resolves) must lie
/// inside `root`, symlinks resolved, and so must every `configPath` of that registry
/// (a missing one lexically). A registry the TypeScript schema rejects is never read
/// further by airs-cli-rs, so its contents are not checked.
pub fn import_source_violation(
    root: &Path,
    cwd: &Path,
    env: &BTreeMap<String, String>,
    args: &[&str],
) -> Option<String> {
    if !is_import_command(args) {
        return None;
    }
    // Every occurrence counts: clap keeps the last one (`args_override_self`), and a
    // run that names two sources must have both inside the sandbox.
    let values = |flag: &str| path_flag_values(args, flag);
    let inside = |path: &Path| {
        let path = lexical(&cwd.join(path));
        match std::fs::canonicalize(&path) {
            Ok(resolved) => resolved.starts_with(root),
            Err(_) => path.starts_with(root),
        }
    };
    let configs = values("--from-config");
    if configs.iter().any(|config| !inside(Path::new(config))) {
        return Some("--from-config names a file outside the sandbox".to_owned());
    }
    let mut registries: Vec<PathBuf> = values("--from")
        .into_iter()
        .map(|from| cwd.join(from))
        .collect();
    if registries.is_empty() && configs.is_empty() {
        registries.push(crate::sandbox::ts_registry_path(env, cwd)?);
    }
    registries.iter().find_map(|registry| {
        if !inside(registry) {
            return Some("the TypeScript registry it would import lies outside the sandbox".into());
        }
        registry_config_violation(registry, &inside)
    })
}

/// The non-empty values of every occurrence of `flag` in `args`, in both the
/// `--flag value` and the `--flag=value` form (before a literal `--`).
fn path_flag_values<'a>(args: &[&'a str], flag: &str) -> Vec<&'a str> {
    let prefix = format!("{flag}=");
    let mut values = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if *arg == "--" {
            break;
        }
        let value = if *arg == flag {
            iter.next().copied()
        } else {
            arg.strip_prefix(prefix.as_str())
        };
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            values.push(value);
        }
    }
    values
}

/// The first entry of the registry at `registry` whose config is not `inside`.
fn registry_config_violation(registry: &Path, inside: &dyn Fn(&Path) -> bool) -> Option<String> {
    let entries = std::fs::read(registry)
        .ok()
        .and_then(|bytes| crate::store::ts_registry(&bytes))
        .map(|(entries, _)| entries)
        .unwrap_or_default();
    entries
        .iter()
        .find(|entry| !inside(Path::new(&entry.config_path)))
        .map(|entry| {
            format!(
                "the registry entry '{}' names a config outside the sandbox",
                entry.name
            )
        })
}

/// The store directory airs-cli-rs resolves under `env` (DESIGN §3.1), when it is
/// absolute; `None` when it would refuse to resolve one.
pub(crate) fn rust_store_dir(env: &BTreeMap<String, String>, cwd: &Path) -> Option<PathBuf> {
    let set = |name: &str| env.get(name).filter(|value| !value.is_empty());
    let absolute = |value: &str| Path::new(value).has_root().then(|| PathBuf::from(value));
    let _ = cwd;
    if let Some(dir) = set("AIRS_CLI_CONFIG_DIR") {
        return absolute(dir);
    }
    if let Some(config) = set("XDG_CONFIG_HOME") {
        return absolute(config).map(|dir| dir.join("airs-cli"));
    }
    env.get("HOME")
        .and_then(|home| absolute(home))
        .map(|home| home.join(".config/airs-cli"))
}

/// The program names the doctor's `Executable` check looks for on `PATH` (DESIGN §21.15).
pub const PROGRAM_NAMES: [&str; 3] = ["airs", "airs-cli", "airs-cli-rs"];

/// `path` without the entries that hold an `airs`, `airs-cli` or `airs-cli-rs` (any
/// file type): an installed copy, or an unrelated program of that name, on the
/// developer's `PATH` would otherwise change the doctor's `Executable` check (DESIGN
/// §21.15), so the inherited `PATH` never carries one. A test that wants one sets `PATH`
/// in its [`RunOptions`].
pub fn path_without_programs(path: &std::ffi::OsStr) -> OsString {
    let kept: Vec<PathBuf> = std::env::split_paths(path)
        .filter(|dir| {
            !PROGRAM_NAMES
                .iter()
                .any(|name| std::fs::symlink_metadata(dir.join(name)).is_ok())
        })
        .collect();
    std::env::join_paths(kept).unwrap_or_else(|_| OsString::from("/usr/bin:/bin"))
}

/// The exact environment [`spawn`] gives a child in `sandbox`: the allowlist
/// (`PATH`, `HOME`, `TMPDIR`, `PRISMA_AIRS_TENANTS_PATH`, `AIRS_CLI_CONFIG_DIR`,
/// `NO_COLOR`, `NO_PROXY`), then
/// `options.env`, minus `options.remove_env`. For tools (such as the benchmark example)
/// that start the CLIs themselves and must use the same scrubbed environment; a
/// TypeScript run still has to pass [`Sandbox::assert_safe_for_ts`] with it first.
pub fn child_environment(sandbox: &Sandbox, options: &RunOptions) -> BTreeMap<String, String> {
    child_env(sandbox, options)
}

/// The exact child environment: the allowlist, then `options.env`, minus
/// `options.remove_env`.
fn child_env(sandbox: &Sandbox, options: &RunOptions) -> BTreeMap<String, String> {
    let path = std::env::var_os("PATH").unwrap_or_else(|| OsString::from("/usr/bin:/bin"));
    let path = path_without_programs(&path);
    let mut env = BTreeMap::new();
    for (name, value) in [
        ("PATH", path),
        ("HOME", sandbox.home().into_os_string()),
        ("TMPDIR", sandbox.tmp().into_os_string()),
        (
            "PRISMA_AIRS_TENANTS_PATH",
            sandbox.registry_path().into_os_string(),
        ),
        ("AIRS_CLI_CONFIG_DIR", sandbox.store_dir().into_os_string()),
        // Nothing reaches the real `~/.local/share/airs-cli` (DESIGN §10 M6c).
        ("XDG_DATA_HOME", sandbox.data_dir().into_os_string()),
        ("NO_COLOR", "1".into()),
        ("NO_PROXY", "127.0.0.1,localhost".into()),
    ] {
        env.insert(name.to_owned(), value.to_string_lossy().into_owned());
    }
    for (name, value) in &options.env {
        env.insert(name.clone(), value.clone());
    }
    for name in &options.remove_env {
        env.remove(name);
    }
    env
}

/// A started CLI process.
pub struct Running {
    child: Child,
    started: Instant,
    stdout: JoinHandle<Vec<u8>>,
    stderr: JoinHandle<Vec<u8>>,
    /// stderr as read so far ([`Running::wait_for_stderr`]).
    stderr_seen: Arc<Mutex<Vec<u8>>>,
    /// Lets a stalled stdout reader start reading once the child exited.
    release: Option<Sender<()>>,
    /// Lets a stalled stderr reader start reading ([`Running::release_stderr`]).
    stderr_release: Option<Sender<()>>,
    /// The sandbox root and its state before the run.
    unchanged: Option<(PathBuf, Snapshot)>,
    options: RunOptions,
    pub(crate) secrets: Vec<SecretForm>,
    /// Forms forbidden on stderr and in the debug log only (the DLP content markers).
    pub(crate) stderr_secrets: Vec<SecretForm>,
    /// Where `--debug` logs of this run can be (leak-checked like stdout and stderr).
    debug_dirs: Vec<PathBuf>,
    cli: &'static str,
    args: Vec<String>,
    /// An open stdin pipe ([`StdinSource::HeldOpen`]), closed once the child exited.
    held_stdin: Option<ChildStdin>,
}

impl std::fmt::Debug for Running {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Running")
            .field("cli", &self.cli)
            .field("args", &self.args)
            .field("pid", &self.child.id())
            .finish_non_exhaustive()
    }
}

impl Running {
    /// The stderr the child printed so far (an interactive flow reads a printed URL).
    pub fn stderr_now(&self) -> String {
        self.stderr_seen
            .lock()
            .map(|seen| String::from_utf8_lossy(&seen).into_owned())
            .unwrap_or_default()
    }

    /// Process id.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Waits (on the child's output, polled every 5 ms) until its stderr contains
    /// `needle`; `false` after `timeout` or once the child exited without printing it.
    pub fn wait_for_stderr(&mut self, needle: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            let seen = self
                .stderr_seen
                .lock()
                .map(|seen| String::from_utf8_lossy(&seen).contains(needle))
                .unwrap_or(false);
            if seen {
                return true;
            }
            let exited = matches!(self.child.try_wait(), Ok(Some(_)));
            if exited || Instant::now() >= deadline {
                // One last look: the child may have printed it just before exiting.
                return self
                    .stderr_seen
                    .lock()
                    .map(|seen| String::from_utf8_lossy(&seen).contains(needle))
                    .unwrap_or(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Sends a signal (as [`Running::send_signal`]) to an airs-cli-rs run spawned with
    /// [`RunOptions::signal_marker`], then waits, polling every 5 ms, until the CLI's
    /// signal thread has recorded it (the marker file exists). Panics when the run has no
    /// marker, the marker exists before the signal, or the child exits or 20 s pass
    /// without it: a test then never continues on the assumption that a signal was seen.
    pub fn send_signal_recorded(&mut self, name: &str) {
        let marker = self
            .options
            .env
            .iter()
            .rev()
            .find(|(key, _)| key == SIGNAL_MARKER_VAR)
            .map(|(_, value)| PathBuf::from(value))
            .expect("send_signal_recorded needs RunOptions::signal_marker");
        assert!(
            !marker.exists(),
            "{} exists before the signal",
            marker.display()
        );
        self.send_signal(name);
        let deadline = Instant::now() + Duration::from_secs(20);
        while !marker.exists() {
            let exited = matches!(self.child.try_wait(), Ok(Some(_)));
            assert!(
                !exited && Instant::now() < deadline,
                "the CLI did not record SIG{name} (no {})",
                marker.display()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Starts reading a stalled stderr ([`RunOptions::stall_stderr`]); a no-op otherwise.
    pub fn release_stderr(&mut self) {
        if let Some(release) = self.stderr_release.take() {
            let _ = release.send(());
        }
    }

    /// Sends a signal by name (`INT`, `SIGTERM`, ...) with `kill(1)`.
    pub fn send_signal(&self, name: &str) {
        let name = name.strip_prefix("SIG").unwrap_or(name);
        let status = Command::new("kill")
            .args(["-s", name, &self.child.id().to_string()])
            .stdin(Stdio::null())
            .status()
            .unwrap_or_else(|err| panic!("cannot run kill -s {name}: {err}"));
        assert!(
            status.success(),
            "kill -s {name} {} failed: {status}",
            self.child.id()
        );
    }

    /// Waits for exit (killing and panicking after the timeout), then enforces the
    /// secret, signal and unchanged-sandbox invariants.
    pub fn wait(mut self) -> Run {
        // A child blocked on a stalled stderr could never exit.
        self.release_stderr();
        let deadline = self.started + self.options.timeout;
        let status = loop {
            match self.child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    drop(self.held_stdin.take());
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    panic!(
                        "{} {:?} timed out after {:?}; the process was killed",
                        self.cli, self.args, self.options.timeout
                    );
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(err) => panic!("{} {:?}: wait failed: {err}", self.cli, self.args),
            }
        };
        let elapsed = self.started.elapsed();
        drop(self.held_stdin.take());
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        let stdout = join_output(self.stdout);
        let stderr = join_output(self.stderr);
        for form in &self.secrets {
            for (stream, text) in [("stdout", &stdout), ("stderr", &stderr)] {
                assert!(
                    !text.contains(&form.value),
                    "{} {:?}: {} appeared in {stream}",
                    self.cli,
                    self.args,
                    form.label
                );
            }
        }
        for form in &self.stderr_secrets {
            assert!(
                !stderr.contains(&form.value),
                "{} {:?}: {} appeared in stderr",
                self.cli,
                self.args,
                form.label
            );
        }
        crate::debug_log::check_forms(
            &self.debug_dirs,
            &self.secrets,
            &format!("{} {:?}", self.cli, self.args),
        );
        crate::debug_log::check_forms(
            &self.debug_dirs,
            &self.stderr_secrets,
            &format!("{} {:?}", self.cli, self.args),
        );
        if let Some((root, before)) = &self.unchanged {
            let writable = &self.options.writable;
            let changes: Vec<String> = Sandbox::changes_under(root, before)
                .into_iter()
                .filter(|change| {
                    let (kind, path) = change.split_once(' ').unwrap_or(("", ""));
                    let path = Path::new(path);
                    // Under an allowed path, or a directory created on the way to one.
                    !writable.iter().any(|allowed| {
                        path.starts_with(allowed)
                            || (kind == "added"
                                && !path.as_os_str().is_empty()
                                && allowed.starts_with(path))
                    })
                })
                .collect();
            assert!(
                changes.is_empty(),
                "{} {:?} changed sandbox files (only granted paths may change): {changes:?}",
                self.cli,
                self.args
            );
        }
        let (code, signal) = exit_parts(status);
        if let Some(signal) = signal {
            assert!(
                self.options.expect_signal,
                "{} {:?} died by signal {signal}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
                self.cli, self.args
            );
        }
        Run {
            cli: self.cli,
            args: self.args,
            code,
            signal,
            stdout,
            stderr,
            elapsed,
        }
    }
}

pub(crate) fn exit_parts(status: ExitStatus) -> (i32, Option<i32>) {
    match (status.code(), status.signal()) {
        (Some(code), _) => (code, None),
        (None, Some(signal)) => (128 + signal, Some(signal)),
        (None, None) => (-1, None),
    }
}

/// Reads `pipe` to EOF, or only its first `limit` bytes and then drops it; with a
/// `gate`, reading starts only once the gate opens (or its sender is dropped).
pub(crate) fn drain(
    pipe: Option<impl Read + Send + 'static>,
    limit: Option<usize>,
    gate: Option<Receiver<()>>,
) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        if let Some(gate) = gate {
            let _ = gate.recv();
        }
        let mut buffer = Vec::new();
        if let Some(pipe) = pipe {
            // A read error just truncates the capture; the exit status still decides.
            let _ = match limit {
                Some(limit) => pipe.take(limit as u64).read_to_end(&mut buffer),
                None => {
                    let mut pipe = pipe;
                    pipe.read_to_end(&mut buffer)
                }
            };
        }
        buffer
    })
}

/// Reads `pipe` to EOF, appending each chunk to `seen` as it arrives; the handle
/// returns everything read. With a `gate`, reading starts only once the gate opens (or
/// its sender is dropped). The first `skip` bytes (the runner's own filler,
/// [`RunOptions::fill_stderr`]) are read but left out.
fn drain_seen(
    pipe: Option<impl Read + Send + 'static>,
    seen: Arc<Mutex<Vec<u8>>>,
    gate: Option<Receiver<()>>,
    mut skip: usize,
) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        if let Some(gate) = gate {
            let _ = gate.recv();
        }
        if let Some(mut pipe) = pipe {
            let mut chunk = [0u8; 4096];
            // A read error just truncates the capture; the exit status still decides.
            while let Ok(read) = pipe.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                let filler = skip.min(read);
                skip -= filler;
                if let Ok(mut seen) = seen.lock() {
                    seen.extend_from_slice(&chunk[filler..read]);
                }
            }
        }
        seen.lock().map(|seen| seen.clone()).unwrap_or_default()
    })
}

/// A pipe whose buffer is already full ([`RunOptions::fill_stderr`]): the read end, the
/// write end (blocking again, for the child) and the number of filler bytes in it. The
/// write end is filled without blocking until not even one more byte fits; the large
/// writes come first, so a pipe that grows on demand (macOS) has reached its largest
/// buffer before it is full.
fn full_pipe() -> (std::io::PipeReader, std::io::PipeWriter, usize) {
    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
    let (reader, mut writer) =
        std::io::pipe().unwrap_or_else(|err| panic!("cannot create a pipe: {err}"));
    let flags = fcntl_getfl(&writer).unwrap_or_else(|err| panic!("F_GETFL: {err}"));
    fcntl_setfl(&writer, flags | OFlags::NONBLOCK)
        .unwrap_or_else(|err| panic!("F_SETFL O_NONBLOCK: {err}"));
    let mut filler = 0;
    for size in [65_536, 4_096, 1] {
        let block = vec![b'.'; size];
        loop {
            match writer.write(&block) {
                Ok(0) => break,
                Ok(written) => filler += written,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
                Err(err) => panic!("cannot fill a pipe: {err}"),
            }
        }
    }
    // O_NONBLOCK belongs to the open file the child inherits: its writes must block.
    fcntl_setfl(&writer, flags).unwrap_or_else(|err| panic!("F_SETFL: {err}"));
    assert!(filler > 0, "the pipe took no filler");
    (reader, writer, filler)
}

/// Another test's fork can briefly inherit a fixture script's writable descriptor
/// before close-on-exec. ETXTBSY proves exec failed; no CLI has run to replay.
fn spawn_ready(
    mut spawn: impl FnMut() -> std::io::Result<Child>,
    deadline: Instant,
) -> std::io::Result<Child> {
    loop {
        match spawn() {
            Err(error)
                if error.raw_os_error() == Some(rustix::io::Errno::TXTBSY.raw_os_error())
                    && Instant::now() < deadline =>
            {
                std::thread::sleep(
                    Duration::from_millis(1)
                        .min(deadline.saturating_duration_since(Instant::now())),
                );
                if Instant::now() >= deadline {
                    return Err(error);
                }
            }
            result => return result,
        }
    }
}

pub(crate) fn join_output(handle: JoinHandle<Vec<u8>>) -> String {
    let bytes = handle.join().unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod spawn_tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn text_busy_fixture_retries_after_the_writer_closes_without_replaying_a_child() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fixture.sh");
        let mut writer = std::fs::File::create(&script).unwrap();
        writer.write_all(b"#!/bin/sh\nprintf ready\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut writer = Some(writer);
        let mut calls = 0;
        let mut command = Command::new(&script);
        command.stdout(Stdio::piped());
        let mut child = spawn_ready(
            || {
                calls += 1;
                let result = command.spawn();
                if calls == 1 {
                    assert_eq!(
                        result.as_ref().unwrap_err().raw_os_error(),
                        Some(rustix::io::Errno::TXTBSY.raw_os_error())
                    );
                    drop(writer.take());
                }
                result
            },
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(calls, 2);
        let mut output = String::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut output)
            .unwrap();
        assert!(child.wait().unwrap().success());
        assert_eq!(output, "ready");
    }

    #[test]
    fn other_spawn_errors_and_expired_text_busy_fail_without_retry() {
        for code in [rustix::io::Errno::ACCESS, rustix::io::Errno::TXTBSY] {
            let mut calls = 0;
            let deadline = if code == rustix::io::Errno::ACCESS {
                Instant::now() + Duration::from_secs(1)
            } else {
                Instant::now()
            };
            let result = spawn_ready(
                || {
                    calls += 1;
                    Err(std::io::Error::from_raw_os_error(code.raw_os_error()))
                },
                deadline,
            );
            assert_eq!(
                result.unwrap_err().raw_os_error(),
                Some(code.raw_os_error())
            );
            assert_eq!(calls, 1);
        }
    }
}
