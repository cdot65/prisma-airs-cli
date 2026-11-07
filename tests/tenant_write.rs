//! Executable tests of the tenant write commands (DESIGN §3.9–§3.11): every command and
//! validation branch, prompts and the delete confirmation under a pty, the store lock,
//! concurrent writers, file modes and atomicity.
//!
//! Every run is sandboxed: `AIRS_CLI_CONFIG_DIR=<sandbox>/store`. A run that may write
//! is granted `store/` only (`allow_writes_under("store")`); every other run must leave
//! the whole sandbox unchanged, which is how "a failing command creates nothing" (D87)
//! is checked. Tenant commands never send requests, so no fixture server is needed.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::Path;
use std::time::{Duration, Instant};

use airs_test_harness::{
    Cli, PtyRun, PtySession, Run, RunOptions, Sandbox, lock_holder, run, spawn, spawn_pty,
    spawn_pty_with,
};

const SECRET: &str = "FAKE-SECRET-W1";
const NAME_RULE: &str = "Tenant name must be 1–64 letters, digits, hyphens or underscores and start with a letter/digit";
const INTERACTIVE: &str = "Interactive setup requires a terminal. Use --tsg-id and --client-id with --client-secret-stdin. For tenant set, pass a value or use --stdin.";
const PIN: &str =
    "Tenant config TSG differs from its registered identity; register it under a new name";
const ONE_VALUE: &str = "Provide one nonempty configuration value on stdin";
const INVALID: &str = "Invalid value for configuration setting";
const BLANK: &str = "Tenant config requires nonempty mgmtTsgId, mgmtClientId and mgmtClientSecret";
const CANCELLED: &str = "Cancelled; no configuration saved.";
const NOT_FOUND_REGISTER: &str = "Tenant not found; register a named tenant first";

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

/// A run that may change `store/` (and nothing else).
fn writes() -> RunOptions {
    RunOptions::new().allow_writes_under("store").secret(SECRET)
}

/// A run that must change nothing in the sandbox.
fn reads() -> RunOptions {
    RunOptions::new().secret(SECRET)
}

fn rs(sandbox: &Sandbox, args: &[&str], options: RunOptions) -> Run {
    run(&rust(), sandbox, args, &options)
}

fn usage(message: &str) -> String {
    format!("\n  ✗ {message}\n\n")
}

fn op_error(message: &str) -> String {
    format!("\n  ✗ Error: {message}\n\n")
}

fn assert_usage(out: &Run, message: &str) {
    out.assert_code(2);
    assert_eq!(out.stderr, usage(message), "{:?}", out.args);
    assert_eq!(out.stdout, "");
}

fn assert_failure(out: &Run, message: &str) {
    out.assert_code(1);
    assert_eq!(out.stderr, op_error(message), "{:?}", out.args);
    assert_eq!(out.stdout, "");
}

/// The canonical tenant file `create` writes for `tsg` (client `client-<tsg>`).
fn created_bytes(tsg: &str) -> String {
    format!(
        "mgmtClientId = \"client-{tsg}\"\nmgmtClientSecret = \"{SECRET}\"\nmgmtTsgId = \"{tsg}\"\n"
    )
}

/// `tenant create <name>` through the stdin flags; asserts success.
fn create(sandbox: &Sandbox, name: &str, tsg: &str) -> Run {
    let client = format!("client-{tsg}");
    let out = rs(
        sandbox,
        &[
            "tenant",
            "create",
            name,
            "--tsg-id",
            tsg,
            "--client-id",
            &client,
            "--client-secret-stdin",
        ],
        writes().stdin(format!("{SECRET}\n")),
    );
    out.assert_success();
    out
}

fn ok(sandbox: &Sandbox, args: &[&str]) -> Run {
    let out = rs(sandbox, args, writes());
    out.assert_success();
    assert_eq!(out.stdout, "", "write commands print no data");
    out
}

fn index(sandbox: &Sandbox) -> String {
    fs::read_to_string(sandbox.store_index_path()).unwrap_or_default()
}

fn tenant_file(sandbox: &Sandbox, name: &str) -> String {
    fs::read_to_string(sandbox.store_tenant_path(name)).unwrap()
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
}

fn inode(path: &Path) -> u64 {
    fs::metadata(path).unwrap().ino()
}

fn set_mode(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

/// Every entry under `dir`, relative, sorted.
fn listing(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            out.push(path.strip_prefix(root).unwrap().display().to_string());
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                walk(root, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// A Rust-only store with these tenants (name, pin, tenant-file bytes) and `active`.
fn store_with(sandbox: &Sandbox, tenants: &[(&str, &str, &str)], active: Option<&str>) {
    let pins: Vec<(&str, &str)> = tenants.iter().map(|(name, tsg, _)| (*name, *tsg)).collect();
    sandbox.write_store_index(&pins, active);
    for (name, _, bytes) in tenants {
        sandbox.write_tenant_file_allowing_external_endpoints(name, bytes.as_bytes(), 0o600);
    }
}

fn pty(sandbox: &Sandbox, args: &[&str], options: RunOptions) -> PtySession {
    spawn_pty(&rust(), sandbox, args, &options)
}

/// The transcript with the terminal's CRLF as LF.
fn transcript(out: &PtyRun) -> String {
    out.transcript.replace("\r\n", "\n")
}

mod create {
    use super::*;

    #[test]
    fn stdin_flags_register_without_selecting() {
        let sandbox = Sandbox::new();
        let out = create(&sandbox, "dev", "100");
        assert_eq!(out.stdout, "");
        assert_eq!(
            out.stderr,
            "  ✓ Registered dev (TSG 100); private config created. Use airs tenant switch dev.\n"
        );
        assert_eq!(
            index(&sandbox),
            "version = 1\n\n[tenants.dev]\ntsg_id = \"100\"\n"
        );
        assert_eq!(tenant_file(&sandbox, "dev"), created_bytes("100"));
        let store = sandbox.store_dir();
        assert_eq!(
            listing(&store),
            ["config.lock", "config.toml", "tenants", "tenants/dev.toml"]
        );
        for file in ["config.toml", "config.lock", "tenants/dev.toml"] {
            assert_eq!(mode(&store.join(file)), 0o600, "{file}");
        }
        assert_eq!(mode(&store.join("tenants")), 0o700);
        assert_eq!(
            fs::read(store.join("config.lock")).unwrap(),
            b"",
            "released"
        );
        let list = rs(&sandbox, &["tenant", "list", "--output", "json"], reads());
        list.assert_success();
        assert_eq!(list.json()[0]["active"], false);
        assert_eq!(
            list.stderr,
            "  No tenant selected; run 'airs tenant switch <name>'.\n"
        );
    }

    #[test]
    fn missing_store_directories_are_created_0700() {
        let sandbox = Sandbox::new();
        let dir = sandbox.store_dir().join("a/b/airs-cli");
        let out = rs(
            &sandbox,
            &[
                "tenant",
                "create",
                "dev",
                "--tsg-id",
                "100",
                "--client-id",
                "client-100",
                "--client-secret-stdin",
            ],
            writes()
                .stdin(format!("{SECRET}\n"))
                .env("AIRS_CLI_CONFIG_DIR", dir.display().to_string()),
        );
        out.assert_success();
        for path in ["a", "a/b", "a/b/airs-cli", "a/b/airs-cli/tenants"] {
            assert_eq!(mode(&sandbox.store_dir().join(path)), 0o700, "{path}");
        }
        assert_eq!(
            fs::read_to_string(dir.join("tenants/dev.toml")).unwrap(),
            created_bytes("100")
        );
    }

    #[test]
    fn stdin_strips_one_line_break_and_keeps_spaces() {
        for (input, stored) in [
            ("FAKE-SECRET-W1\r\n", "FAKE-SECRET-W1"),
            ("FAKE-SECRET-W1", "FAKE-SECRET-W1"),
            ("  FAKE SECRET W2  \n", "  FAKE SECRET W2  "),
        ] {
            let sandbox = Sandbox::new();
            let out = rs(
                &sandbox,
                &[
                    "tenant",
                    "create",
                    "dev",
                    "--tsg-id",
                    "100",
                    "--client-id",
                    "c",
                    "--client-secret-stdin",
                ],
                writes().stdin(input).secret(stored),
            );
            out.assert_success();
            assert!(
                tenant_file(&sandbox, "dev")
                    .contains(&format!("mgmtClientSecret = \"{stored}\"\n")),
                "{input:?}"
            );
        }
        // Exactly 64 KiB is accepted.
        let sandbox = Sandbox::new();
        let big = "S".repeat(65_536);
        let out = rs(
            &sandbox,
            &[
                "tenant",
                "create",
                "dev",
                "--tsg-id",
                "1",
                "--client-id",
                "c",
                "--client-secret-stdin",
            ],
            writes().stdin(big.clone()).allow_secret_output(),
        );
        out.assert_success();
        assert!(tenant_file(&sandbox, "dev").contains(&big));
    }

    #[test]
    fn stdin_rejections_are_usage_errors_and_create_nothing() {
        let args = [
            "tenant",
            "create",
            "dev",
            "--tsg-id",
            "100",
            "--client-id",
            "c",
            "--client-secret-stdin",
        ];
        let mut cases: Vec<(Vec<u8>, &str)> = [
            &b""[..],
            b"\n",
            b"   \n",
            b"FAKE\nsecond",
            b"FAKE\n\n",
            b"FAKE\0SECRET",
            b"FAKE\r",
            b"\xff\xfe\n",
        ]
        .iter()
        .map(|bytes| (bytes.to_vec(), ONE_VALUE))
        .collect();
        let mut over = vec![b'S'; 65_536];
        over.push(b'\n');
        cases.push((over, "Configuration input exceeds 64 KiB"));
        cases.push((vec![b'S'; 200_000], "Configuration input exceeds 64 KiB"));
        // Control characters other than CR, LF and NUL fail the write rule.
        cases.push((b"FAKE\x1bSECRET\n".to_vec(), INVALID));
        for (bytes, message) in cases {
            let sandbox = Sandbox::new();
            let out = rs(&sandbox, &args, RunOptions::new().stdin(bytes.clone()));
            assert_usage(&out, message);
            assert!(listing(&sandbox.store_dir()).is_empty(), "{bytes:?}");
        }
    }

    #[test]
    fn without_a_terminal_every_missing_value_is_a_usage_error() {
        for args in [
            &["tenant", "create", "dev"][..],
            &["tenant", "create", "dev", "--tsg-id", "100"],
            &[
                "tenant",
                "create",
                "dev",
                "--tsg-id",
                "100",
                "--client-id",
                "c",
            ],
            &["tenant", "create", "dev", "--client-secret-stdin"],
            &[
                "tenant",
                "create",
                "dev",
                "--client-id",
                "c",
                "--client-secret-stdin",
            ],
        ] {
            let sandbox = Sandbox::new();
            let out = rs(
                &sandbox,
                args,
                RunOptions::new().stdin(format!("{SECRET}\n")),
            );
            assert_usage(&out, INTERACTIVE);
        }
    }

    #[test]
    fn the_secret_flag_needs_piped_stdin() {
        let sandbox = Sandbox::new();
        let session = spawn_pty_with(
            &rust(),
            &sandbox,
            &[
                "tenant",
                "create",
                "dev",
                "--tsg-id",
                "100",
                "--client-id",
                "c",
                "--client-secret-stdin",
            ],
            &RunOptions::new(),
            false,
        );
        let out = session.wait();
        assert_usage(&out.run, "--stdin requires piped input");
    }

    #[test]
    fn argument_checks_come_first_and_never_read_the_store() {
        let sandbox = Sandbox::new();
        sandbox.write_store_raw(b"not toml at all [", 0o600);
        for name in ["a/b", "", "_x", "é", &"a".repeat(65), "space name", "-bad"] {
            let out = rs(
                &sandbox,
                &["tenant", "create", "--tsg-id", "abc", "--", name],
                RunOptions::new(),
            );
            assert_usage(&out, NAME_RULE);
        }
        for (flags, message) in [
            (
                &["--tsg-id", " 100"][..],
                "Invalid mgmtTsgId: the Rust SDK requires 1-32 ASCII digits",
            ),
            (
                &["--tsg-id", "12abc"],
                "Invalid mgmtTsgId: the Rust SDK requires 1-32 ASCII digits",
            ),
            (&["--tsg-id", ""], BLANK),
            (&["--tsg-id", "100", "--client-id", "  "], BLANK),
            (&["--tsg-id", "100", "--client-id", "a\u{1b}[31m"], INVALID),
        ] {
            let mut args = vec!["tenant", "create", "dev"];
            args.extend_from_slice(flags);
            args.push("--client-secret-stdin");
            // Empty stdin would be refused too: the flag values are checked first.
            let out = rs(&sandbox, &args, RunOptions::new().stdin(""));
            assert_usage(&out, message);
        }
        // A valid invocation reaches the store and reports it.
        let out = rs(
            &sandbox,
            &[
                "tenant",
                "create",
                "dev",
                "--tsg-id",
                "100",
                "--client-id",
                "c",
                "--client-secret-stdin",
            ],
            reads().stdin(format!("{SECRET}\n")),
        );
        assert_failure(
            &out,
            &format!(
                "Cannot read tenant registry at {}; restore a valid registry before proceeding",
                sandbox.store_index_path().display()
            ),
        );
    }

    #[test]
    fn existing_and_case_conflicting_names_are_refused() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let before = (index(&sandbox), tenant_file(&sandbox, "dev"));
        for (name, message) in [
            ("dev", "Tenant name already exists".to_owned()),
            (
                "DEV",
                "Tenant name conflicts with existing tenant dev; names may not differ only in case"
                    .to_owned(),
            ),
            (
                "Dev",
                "Tenant name conflicts with existing tenant dev; names may not differ only in case"
                    .to_owned(),
            ),
        ] {
            let out = rs(
                &sandbox,
                &[
                    "tenant",
                    "create",
                    name,
                    "--tsg-id",
                    "200",
                    "--client-id",
                    "c",
                    "--client-secret-stdin",
                ],
                reads().stdin(format!("{SECRET}\n")),
            );
            assert_failure(&out, &message);
        }
        assert_eq!((index(&sandbox), tenant_file(&sandbox, "dev")), before);
    }

    #[test]
    fn the_store_holds_at_most_1000_tenants() {
        let sandbox = Sandbox::new();
        let names: Vec<String> = (0..1000).map(|n| format!("t{n}")).collect();
        let pins: Vec<(&str, &str)> = names.iter().map(|name| (name.as_str(), "1")).collect();
        sandbox.write_store_index(&pins, None);
        let out = rs(
            &sandbox,
            &[
                "tenant",
                "create",
                "dev",
                "--tsg-id",
                "100",
                "--client-id",
                "c",
                "--client-secret-stdin",
            ],
            reads().stdin(format!("{SECRET}\n")),
        );
        assert_failure(
            &out,
            "Cannot create tenant dev: at most 1000 tenants can be registered",
        );
    }

    #[test]
    fn a_new_index_over_1_mib_is_refused_before_writing() {
        let sandbox = Sandbox::new();
        // 39 bytes of TOML around the TSG: the index is 1,048,569 bytes, and the new
        // entry (30 bytes) would take it past 1 MiB.
        let big = "9".repeat(1_048_530);
        sandbox.write_store_index(&[("big", &big)], None);
        let out = rs(
            &sandbox,
            &[
                "tenant",
                "create",
                "dev",
                "--tsg-id",
                "100",
                "--client-id",
                "c",
                "--client-secret-stdin",
            ],
            writes().stdin(format!("{SECRET}\n")),
        );
        assert_failure(
            &out,
            &format!(
                "Cannot save {}: it would exceed 1 MiB",
                sandbox.store_index_path().display()
            ),
        );
        assert!(
            !sandbox.store_tenant_path("dev").exists(),
            "nothing written"
        );
    }

    #[test]
    fn an_orphan_tenant_file_is_replaced() {
        let sandbox = Sandbox::new();
        sandbox.write_tenant_file_allowing_external_endpoints("dev", b"mgmtTsgId = \"9\"\n", 0o600);
        create(&sandbox, "dev", "100");
        assert_eq!(tenant_file(&sandbox, "dev"), created_bytes("100"));
    }

    #[test]
    fn a_read_only_index_is_refused_and_nothing_is_written() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        set_mode(&sandbox.store_index_path(), 0o400);
        let out = rs(
            &sandbox,
            &[
                "tenant",
                "create",
                "prod",
                "--tsg-id",
                "200",
                "--client-id",
                "c",
                "--client-secret-stdin",
            ],
            writes().stdin(format!("{SECRET}\n")),
        );
        assert_failure(
            &out,
            &format!(
                "{} is read-only; restore write permission (chmod 600) to change it",
                sandbox.store_index_path().display()
            ),
        );
        assert!(!sandbox.store_tenant_path("prod").exists());
        assert_eq!(mode(&sandbox.store_index_path()), 0o400);
    }

    #[test]
    fn store_location_errors_come_after_the_input_checks() {
        let sandbox = Sandbox::new();
        let args = [
            "tenant",
            "create",
            "dev",
            "--tsg-id",
            "100",
            "--client-id",
            "c",
            "--client-secret-stdin",
        ];
        let relative = RunOptions::new().env("AIRS_CLI_CONFIG_DIR", "relative/store");
        let out = rs(&sandbox, &args, relative.clone().stdin(""));
        assert_usage(&out, ONE_VALUE);
        let out = rs(
            &sandbox,
            &args,
            relative.stdin(format!("{SECRET}\n")).secret(SECRET),
        );
        assert_failure(&out, "AIRS_CLI_CONFIG_DIR must be an absolute path");
    }
}

mod create_prompts {
    use super::*;

    #[test]
    fn guided_create_asks_in_order_and_hides_the_secret() {
        let sandbox = Sandbox::new();
        let mut session = pty(&sandbox, &["tenant", "create", "dev"], writes());
        session.expect("? Tenant service group ID (mgmtTsgId): ");
        session.send(b"100\n");
        session.expect("? OAuth client ID (mgmtClientId): ");
        session.send(b"client-100\n");
        session.expect("? OAuth client secret (mgmtClientSecret): [input is masked] ");
        session.send(format!("{SECRET}\n").as_bytes());
        let out = session.wait();
        out.run.assert_success();
        assert_eq!(out.run.stdout, "");
        assert_eq!(
            transcript(&out),
            "? Tenant service group ID (mgmtTsgId): 100\n? OAuth client ID (mgmtClientId): client-100\n? OAuth client secret (mgmtClientSecret): [input is masked] \n  ✓ Registered dev (TSG 100); private config created. Use airs tenant switch dev.\n"
        );
        assert!(
            !out.transcript.contains('\u{1b}'),
            "no escape sequences (D88)"
        );
        assert!(out.echo_restored());
        assert_eq!(tenant_file(&sandbox, "dev"), created_bytes("100"));
    }

    #[test]
    fn blank_and_invalid_answers_are_asked_again() {
        let sandbox = Sandbox::new();
        let mut session = pty(&sandbox, &["tenant", "create", "dev"], writes());
        session.expect("(mgmtTsgId): ");
        session.send(b"\n");
        session.expect("> A nonempty value is required\n");
        session.expect("(mgmtTsgId): ");
        session.send(b"abc\n");
        session.expect("> Invalid mgmtTsgId: the Rust SDK requires 1-32 ASCII digits\n");
        session.send(b" 100 \n");
        session.expect("> Invalid mgmtTsgId");
        session.send(b"100\n");
        session.expect("(mgmtClientId): ");
        session.send(b"bad\x1bid\n");
        session.expect("> Invalid value for configuration setting\n");
        session.send(b" client-100 \n");
        session.expect("[input is masked] ");
        session.send(b"   \n");
        session.expect("> A nonempty value is required\n");
        session.expect("[input is masked] ");
        session.send(format!("{SECRET}\n").as_bytes());
        let out = session.wait();
        out.run.assert_success();
        assert_eq!(
            tenant_file(&sandbox, "dev"),
            format!(
                "mgmtClientId = \" client-100 \"\nmgmtClientSecret = \"{SECRET}\"\nmgmtTsgId = \"100\"\n"
            ),
            "answers are stored untrimmed"
        );
    }

    #[test]
    fn only_missing_values_are_asked() {
        let sandbox = Sandbox::new();
        let mut session = pty(
            &sandbox,
            &["tenant", "create", "dev", "--tsg-id", "100"],
            writes(),
        );
        session.expect("? OAuth client ID (mgmtClientId): ");
        session.send(b"client-100\n");
        session.expect("[input is masked] ");
        session.send(format!("{SECRET}\n").as_bytes());
        let out = session.wait();
        out.run.assert_success();
        assert!(!out.transcript.contains("(mgmtTsgId)"));
        assert_eq!(tenant_file(&sandbox, "dev"), created_bytes("100"));
    }

    #[test]
    fn ctrl_c_at_any_prompt_exits_130_and_writes_nothing() {
        for (answers, prompt) in [
            (&[][..], "(mgmtTsgId): "),
            (&["100"], "(mgmtClientId): "),
            (&["100", "client-100"], "[input is masked] "),
        ] {
            let sandbox = Sandbox::new();
            let mut session = pty(&sandbox, &["tenant", "create", "dev"], RunOptions::new());
            for (answer, expected) in answers.iter().zip(["(mgmtTsgId): ", "(mgmtClientId): "]) {
                session.expect(expected);
                session.send(format!("{answer}\n").as_bytes());
            }
            session.expect(prompt);
            session.signal("INT");
            let out = session.wait();
            out.run.assert_code(130);
            assert!(
                transcript(&out).ends_with(&format!("{prompt}\n  {CANCELLED}\n")),
                "{:?}",
                out.transcript
            );
            assert!(out.echo_restored(), "the terminal mode was restored");
            assert!(listing(&sandbox.store_dir()).is_empty());
        }
    }

    #[test]
    fn sigterm_and_eof_cancel_too() {
        let sandbox = Sandbox::new();
        let mut session = pty(
            &sandbox,
            &[
                "tenant",
                "create",
                "dev",
                "--tsg-id",
                "1",
                "--client-id",
                "c",
            ],
            RunOptions::new(),
        );
        session.expect("[input is masked] ");
        session.signal("TERM");
        let out = session.wait();
        out.run.assert_code(143);
        assert!(transcript(&out).contains(CANCELLED));
        assert!(out.echo_restored());

        let mut session = pty(&sandbox, &["tenant", "create", "dev"], RunOptions::new());
        session.expect("(mgmtTsgId): ");
        session.send(b"\x04");
        let out = session.wait();
        out.run.assert_code(130);
        // The terminal may echo the Ctrl-D as `^D` (ECHOCTL); the line is then ended.
        let shown = transcript(&out);
        assert!(
            shown.starts_with("? Tenant service group ID (mgmtTsgId): ")
                && shown.ends_with(&format!("\n  {CANCELLED}\n")),
            "{shown:?}"
        );

        let mut session = pty(
            &sandbox,
            &[
                "tenant",
                "create",
                "dev",
                "--tsg-id",
                "1",
                "--client-id",
                "c",
            ],
            RunOptions::new(),
        );
        session.expect("[input is masked] ");
        session.send(b"\x04");
        let out = session.wait();
        out.run.assert_code(130);
        assert!(out.echo_restored());
        assert!(listing(&sandbox.store_dir()).is_empty());
    }

    #[test]
    fn quiet_hides_the_cancelled_status() {
        let sandbox = Sandbox::new();
        let mut session = pty(
            &sandbox,
            &["--quiet", "tenant", "create", "dev"],
            RunOptions::new(),
        );
        session.expect("(mgmtTsgId): ");
        session.signal("INT");
        let out = session.wait();
        out.run.assert_code(130);
        assert!(!out.transcript.contains(CANCELLED));
        assert!(
            out.transcript.contains("? Tenant service group ID"),
            "prompts are never quiet"
        );
    }

    #[test]
    fn the_prompt_gate_needs_stdin_and_stderr_terminals() {
        let sandbox = Sandbox::new();
        let out = spawn_pty_with(
            &rust(),
            &sandbox,
            &["tenant", "create", "dev"],
            &RunOptions::new(),
            false,
        )
        .wait();
        assert_usage(&out.run, INTERACTIVE);
        assert_eq!(out.transcript, "", "nothing was asked");
    }

    #[test]
    fn state_checks_run_before_any_prompt() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let out = pty(&sandbox, &["tenant", "create", "dev"], reads()).wait();
        out.run.assert_code(1);
        assert_eq!(transcript(&out), op_error("Tenant name already exists"));
        let out = pty(&sandbox, &["tenant", "create", "DEV"], reads()).wait();
        out.run.assert_code(1);
        assert!(
            !out.transcript.contains('?'),
            "no prompt: {:?}",
            out.transcript
        );
    }
}

mod set {
    use super::*;

    #[test]
    fn every_value_class_is_stored_canonically() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let before = index(&sandbox);
        for (key, value) in [
            ("defaultOutput", "json"),
            ("scanConcurrency", "007"),
            ("airsNumRetries", "0"),
            ("mgmtEndpoint", "http://127.0.0.1:9/m"),
            ("supplychainDataEndpoint", "http://[::1]:9/sc"),
            ("gatewayInferenceModel", " model x "),
            ("mgmtClientId", "client-200"),
        ] {
            let out = ok(&sandbox, &["tenant", "set", "dev", key, value]);
            assert_eq!(
                out.stderr,
                format!("  ✓ Updated {key} for tenant dev; selection unchanged.\n")
            );
        }
        assert_eq!(
            tenant_file(&sandbox, "dev"),
            format!(
                "airsNumRetries = 0\nmgmtClientId = \"client-200\"\nmgmtClientSecret = \"{SECRET}\"\nmgmtTsgId = \"100\"\nmgmtEndpoint = \"http://127.0.0.1:9/m\"\nsupplychainDataEndpoint = \"http://[::1]:9/sc\"\ngatewayInferenceModel = \" model x \"\nscanConcurrency = 7\ndefaultOutput = \"json\"\n"
            )
        );
        assert_eq!(
            index(&sandbox),
            before,
            "the selection and pin are unchanged"
        );
        let read = rs(
            &sandbox,
            &[
                "tenant",
                "get",
                "dev",
                "scanConcurrency",
                "--output",
                "json",
            ],
            reads(),
        );
        assert_eq!(read.json()[0]["value"], 7);
    }

    #[test]
    fn secrets_come_from_stdin_or_a_prompt_and_are_never_printed() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let out = rs(
            &sandbox,
            &["tenant", "set", "dev", "airsApiKey", "--stdin"],
            writes().stdin("KEY-W1\n").secret("KEY-W1"),
        );
        out.assert_success();
        assert!(tenant_file(&sandbox, "dev").starts_with("airsApiKey = \"KEY-W1\"\n"));
        let get = rs(
            &sandbox,
            &["tenant", "get", "dev", "airsApiKey"],
            reads().secret("KEY-W1"),
        );
        assert_eq!(get.stdout, "[REDACTED]\n");
        // stdin works for any key and follows its write rules.
        let out = rs(
            &sandbox,
            &["tenant", "set", "dev", "scanConcurrency", "--stdin"],
            writes().stdin("9\n"),
        );
        out.assert_success();
        assert!(tenant_file(&sandbox, "dev").contains("scanConcurrency = 9\n"));
        let out = rs(
            &sandbox,
            &["tenant", "set", "dev", "scanConcurrency", "--stdin"],
            reads().stdin("0x9\n"),
        );
        assert_usage(&out, INVALID);
    }

    #[test]
    fn the_tenant_file_is_always_rewritten() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let path = sandbox.store_tenant_path("dev");
        let first = inode(&path);
        ok(&sandbox, &["tenant", "set", "dev", "defaultOutput", "json"]);
        let second = inode(&path);
        ok(&sandbox, &["tenant", "set", "dev", "defaultOutput", "json"]);
        assert!(
            first != second && second != inode(&path),
            "a new file each time"
        );
        assert_eq!(mode(&path), 0o600);
    }

    #[test]
    fn input_errors_are_usage_errors_before_the_store_is_read() {
        let sandbox = Sandbox::new();
        sandbox.write_store_raw(b"garbage [", 0o600);
        let unknown = format!(
            "Unknown configuration key. Supported keys: {}",
            [
                "airsApiKey",
                "airsApiToken",
                "airsApiEndpoint",
                "airsNumRetries",
                "mgmtClientId",
                "mgmtClientSecret",
                "mgmtTsgId",
                "mgmtEndpoint",
                "mgmtDashboardEndpoint",
                "mgmtTokenEndpoint",
                "dlpEndpoint",
                "redTeamDataEndpoint",
                "redTeamMgmtEndpoint",
                "redTeamNetworkBrokerEndpoint",
                "modelSecDataEndpoint",
                "modelSecMgmtEndpoint",
                "supplychainDataEndpoint",
                "supplychainMgmtEndpoint",
                "gatewayDataEndpoint",
                "gatewayAdminEndpoint",
                "iamEndpoint",
                "gatewayInferenceEndpoint",
                "gatewayInferenceApiKey",
                "gatewayInferenceModel",
                "gatewayEmbeddingModel",
                "scanConcurrency",
                "defaultOutput",
            ]
            .join(", ")
        );
        let endpoint = "Invalid mgmtEndpoint: endpoints must use https (http is allowed only for loopback hosts) and must not contain credentials, a query or a fragment";
        let secret_arg = "Do not pass credentials as arguments. Omit the value for a hidden prompt, or use --stdin.";
        for (args, message) in [
            (&["notAKey", "v"][..], unknown.as_str()),
            (&["aiGwDataEndpoint", "v"], &unknown),
            (&["__proto__", "v"], &unknown),
            (&["dataDir", "runs"], &unknown),
            (
                &["defaultOutput", "json", "--stdin"],
                "Use a value or --stdin, not both",
            ),
            (
                &["airsApiKey", "X", "--stdin"],
                "Use a value or --stdin, not both",
            ),
            (&["airsApiKey", "FAKE-ARG"], secret_arg),
            (&["airsApiToken", ""], secret_arg),
            (&["mgmtClientSecret", "x"], secret_arg),
            (&["gatewayInferenceApiKey", "x"], secret_arg),
            (&["scanConcurrency", "0x5"], INVALID),
            (&["scanConcurrency", "21"], INVALID),
            (&["scanConcurrency", " 7 "], INVALID),
            (&["scanConcurrency", "-1"], INVALID),
            (&["scanConcurrency", "1e1"], INVALID),
            (
                &["airsNumRetries", ""],
                "Empty values are not stored; use 'airs tenant unset dev airsNumRetries' to remove a setting",
            ),
            (&["defaultOutput", "yaml"], INVALID),
            (&["defaultOutput", "JSON"], INVALID),
            (
                &["mgmtTsgId", " 100"],
                "Invalid mgmtTsgId: the Rust SDK requires 1-32 ASCII digits",
            ),
            (&["mgmtClientId", "  "], BLANK),
            (&["mgmtEndpoint", "http://example.test"], endpoint),
            (&["mgmtEndpoint", "https://u:p@example.test"], endpoint),
            (&["mgmtEndpoint", "not a url"], endpoint),
            (
                &["mgmtEndpoint", ""],
                "Empty values are not stored; use 'airs tenant unset dev mgmtEndpoint' to remove a setting",
            ),
            (&["gatewayInferenceModel", "a\tb"], INVALID),
            (&["defaultOutput"], INTERACTIVE),
        ] {
            let mut full = vec!["tenant", "set", "dev"];
            full.extend_from_slice(args);
            let out = rs(&sandbox, &full, RunOptions::new().secret("FAKE-ARG"));
            assert_usage(&out, message);
        }
        for (input, message) in [
            (&b"FAKE\nx"[..], ONE_VALUE),
            (b"", ONE_VALUE),
            (b"a\x1bb", INVALID),
        ] {
            let out = rs(
                &sandbox,
                &["tenant", "set", "dev", "airsApiKey", "--stdin"],
                RunOptions::new().stdin(input.to_vec()),
            );
            assert_usage(&out, message);
        }
    }

    #[test]
    fn unknown_tenants_and_unusable_files_are_operational_errors() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let out = rs(
            &sandbox,
            &["tenant", "set", "nope", "defaultOutput", "json"],
            reads(),
        );
        assert_failure(&out, NOT_FOUND_REGISTER);
        let path = sandbox.store_tenant_path("dev");
        let invalid = format!("Cannot read a valid tenant config at {}", path.display());
        for bytes in [
            format!(
                "{}aiGwDataEndpoint = \"http://127.0.0.1:1\"\n",
                created_bytes("100")
            ),
            format!("{}defaultOutput = \"markdown\"\n", created_bytes("100")),
            "mgmtTsgId = ".to_owned(),
        ] {
            sandbox.write_tenant_file_allowing_external_endpoints("dev", bytes.as_bytes(), 0o600);
            let out = rs(
                &sandbox,
                &["tenant", "set", "dev", "defaultOutput", "json"],
                reads(),
            );
            assert_failure(&out, &invalid);
            assert_eq!(tenant_file(&sandbox, "dev"), bytes, "unchanged");
        }
        sandbox.write_tenant_file_allowing_external_endpoints(
            "dev",
            created_bytes("200").as_bytes(),
            0o600,
        );
        let out = rs(
            &sandbox,
            &["tenant", "set", "dev", "defaultOutput", "json"],
            reads(),
        );
        assert_failure(&out, PIN);
        fs::remove_file(&path).unwrap();
        let out = rs(
            &sandbox,
            &["tenant", "set", "dev", "defaultOutput", "json"],
            reads(),
        );
        assert_failure(&out, &invalid);
        assert!(!path.exists(), "nothing created");
    }

    #[test]
    fn mgmt_tsg_id_changes_the_pin_and_repairs_a_mismatch() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        ok(&sandbox, &["tenant", "switch", "dev"]);
        let out = ok(&sandbox, &["tenant", "set", "dev", "mgmtTsgId", "200"]);
        assert_eq!(
            out.stderr,
            "  ✓ Updated mgmtTsgId for tenant dev; selection unchanged.\n"
        );
        assert_eq!(
            index(&sandbox),
            "version = 1\nactive = \"dev\"\n\n[tenants.dev]\ntsg_id = \"200\"\n"
        );
        assert_eq!(
            tenant_file(&sandbox, "dev"),
            created_bytes("100").replace("mgmtTsgId = \"100\"", "mgmtTsgId = \"200\"")
        );
        // A hand edit makes a mismatch: every other key is refused, mgmtTsgId repairs it.
        sandbox.write_tenant_file_allowing_external_endpoints(
            "dev",
            created_bytes("300").as_bytes(),
            0o600,
        );
        let out = rs(&sandbox, &["tenant", "read", "dev"], reads());
        assert_failure(&out, PIN);
        let out = rs(
            &sandbox,
            &["tenant", "set", "dev", "defaultOutput", "json"],
            reads(),
        );
        assert_failure(&out, PIN);
        ok(&sandbox, &["tenant", "set", "dev", "mgmtTsgId", "300"]);
        assert!(index(&sandbox).contains("tsg_id = \"300\""));
        rs(&sandbox, &["tenant", "read", "dev"], reads()).assert_success();
    }

    #[test]
    fn tenants_without_management_credentials_can_be_edited() {
        let sandbox = Sandbox::new();
        store_with(
            &sandbox,
            &[(
                "scan",
                "100",
                "airsApiKey = \"K-W1\"\nmgmtTsgId = \"100\"\n",
            )],
            None,
        );
        let options = || writes().secret("K-W1");
        rs(
            &sandbox,
            &["tenant", "set", "scan", "scanConcurrency", "3"],
            options(),
        )
        .assert_success();
        rs(
            &sandbox,
            &["tenant", "set", "scan", "mgmtClientId", "c"],
            options(),
        )
        .assert_success();
        rs(
            &sandbox,
            &["tenant", "unset", "scan", "scanConcurrency"],
            options(),
        )
        .assert_success();
        assert_eq!(
            tenant_file(&sandbox, "scan"),
            "airsApiKey = \"K-W1\"\nmgmtClientId = \"c\"\nmgmtTsgId = \"100\"\n"
        );
    }

    #[test]
    fn a_read_only_tenant_file_is_refused() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let path = sandbox.store_tenant_path("dev");
        set_mode(&path, 0o400);
        let out = rs(
            &sandbox,
            &["tenant", "set", "dev", "defaultOutput", "json"],
            writes(),
        );
        assert_failure(
            &out,
            &format!(
                "{} is read-only; restore write permission (chmod 600) to change it",
                path.display()
            ),
        );
        assert_eq!(tenant_file(&sandbox, "dev"), created_bytes("100"));
        set_mode(&path, 0o640);
        let out = rs(
            &sandbox,
            &["tenant", "set", "dev", "defaultOutput", "json"],
            reads(),
        );
        assert!(out.stderr.contains("Refusing to use"), "{}", out.stderr);
    }

    /// DESIGN §3.10: on a terminal (canonical mode) the first Ctrl-D after typing only
    /// hands the typed text to the reader; the prompt keeps waiting until Enter or a
    /// second Ctrl-D, which ends the answer.
    #[test]
    fn ctrl_d_after_typing_needs_a_second_ctrl_d() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let mut session = pty(
            &sandbox,
            &["tenant", "set", "dev", "defaultOutput"],
            writes(),
        );
        session.expect("? defaultOutput: ");
        session.send(b"json\x04");
        std::thread::sleep(Duration::from_millis(400));
        assert!(
            !session.transcript().contains("Updated"),
            "one Ctrl-D does not end the answer: {:?}",
            session.transcript()
        );
        assert!(!tenant_file(&sandbox, "dev").contains("defaultOutput"));
        session.send(b"\x04");
        let out = session.wait();
        out.run.assert_success();
        assert!(
            transcript(&out)
                .ends_with("  ✓ Updated defaultOutput for tenant dev; selection unchanged.\n"),
            "{:?}",
            out.transcript
        );
        assert!(tenant_file(&sandbox, "dev").contains("defaultOutput = \"json\"\n"));
    }

    #[test]
    fn prompts_validate_and_hide_secret_keys() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let mut session = pty(
            &sandbox,
            &["tenant", "set", "dev", "scanConcurrency"],
            writes(),
        );
        session.expect("? scanConcurrency: ");
        session.send(b"0x5\n");
        session.expect("> Invalid value for configuration setting\n");
        session.expect("? scanConcurrency: ");
        session.send(b"7\n");
        let out = session.wait();
        out.run.assert_success();
        assert_eq!(
            transcript(&out),
            "? scanConcurrency: 0x5\n> Invalid value for configuration setting\n? scanConcurrency: 7\n  ✓ Updated scanConcurrency for tenant dev; selection unchanged.\n"
        );
        let mut session = pty(
            &sandbox,
            &["tenant", "set", "dev", "airsApiToken"],
            writes().secret("TOKEN-W1"),
        );
        session.expect("? airsApiToken: [input is masked] ");
        session.send(b"TOKEN-W1\n");
        let out = session.wait();
        out.run.assert_success();
        assert!(out.echo_restored());
        assert!(tenant_file(&sandbox, "dev").contains("airsApiToken = \"TOKEN-W1\"\n"));
        // Ctrl-C: nothing written.
        let before = tenant_file(&sandbox, "dev");
        let mut session = pty(&sandbox, &["tenant", "set", "dev", "airsApiKey"], reads());
        session.expect("[input is masked] ");
        session.signal("INT");
        let out = session.wait();
        out.run.assert_code(130);
        assert!(transcript(&out).ends_with(&format!("\n  {CANCELLED}\n")));
        assert!(out.echo_restored());
        assert_eq!(tenant_file(&sandbox, "dev"), before);
        // The tenant is checked before the prompt.
        let out = pty(&sandbox, &["tenant", "set", "nope", "airsApiKey"], reads()).wait();
        out.run.assert_code(1);
        assert_eq!(transcript(&out), op_error(NOT_FOUND_REGISTER));
    }
}

mod unset {
    use super::*;

    #[test]
    fn a_present_key_is_removed() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        ok(&sandbox, &["tenant", "set", "dev", "defaultOutput", "json"]);
        let out = ok(&sandbox, &["tenant", "unset", "dev", "defaultOutput"]);
        assert_eq!(
            out.stderr,
            "  ✓ Removed defaultOutput from tenant dev; selection unchanged.\n"
        );
        assert_eq!(tenant_file(&sandbox, "dev"), created_bytes("100"));
        // A secret that is not a credential can be cleared.
        rs(
            &sandbox,
            &["tenant", "set", "dev", "airsApiKey", "--stdin"],
            writes().stdin("K\n"),
        )
        .assert_success();
        ok(&sandbox, &["tenant", "unset", "dev", "airsApiKey"]);
        assert_eq!(tenant_file(&sandbox, "dev"), created_bytes("100"));
    }

    #[test]
    fn an_absent_key_writes_nothing() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let path = sandbox.store_tenant_path("dev");
        let (bytes, ino, mtime) = (
            tenant_file(&sandbox, "dev"),
            inode(&path),
            fs::metadata(&path).unwrap().mtime_nsec(),
        );
        for key in ["defaultOutput", "scanConcurrency"] {
            let out = ok(&sandbox, &["tenant", "unset", "dev", key]);
            assert_eq!(
                out.stderr,
                format!("  ℹ {key} is not set for tenant dev — nothing to do\n")
            );
        }
        assert_eq!(tenant_file(&sandbox, "dev"), bytes);
        assert_eq!(
            (inode(&path), fs::metadata(&path).unwrap().mtime_nsec()),
            (ino, mtime)
        );
        // A read-only file still allows the no-op, but not a removal.
        ok(&sandbox, &["tenant", "set", "dev", "defaultOutput", "json"]);
        set_mode(&path, 0o400);
        ok(&sandbox, &["tenant", "unset", "dev", "scanConcurrency"]);
        let out = rs(
            &sandbox,
            &["tenant", "unset", "dev", "defaultOutput"],
            writes(),
        );
        assert_failure(
            &out,
            &format!(
                "{} is read-only; restore write permission (chmod 600) to change it",
                path.display()
            ),
        );
    }

    #[test]
    fn credentials_unknown_keys_and_unknown_tenants() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        for key in ["mgmtTsgId", "mgmtClientId", "mgmtClientSecret"] {
            let out = rs(&sandbox, &["tenant", "unset", "nope", key], reads());
            assert_usage(
                &out,
                &format!("{key} is a credential and cannot be cleared; set a new value instead"),
            );
        }
        for key in ["custom", "redTeamTokenEndpoint", "aiGwInferenceModel"] {
            let out = rs(&sandbox, &["tenant", "unset", "dev", key], reads());
            out.assert_code(2);
            assert!(
                out.stderr
                    .starts_with("\n  ✗ Unknown configuration key. Supported keys: airsApiKey,")
            );
        }
        let out = rs(
            &sandbox,
            &["tenant", "unset", "nope", "defaultOutput"],
            reads(),
        );
        assert_failure(&out, NOT_FOUND_REGISTER);
        sandbox.write_tenant_file_allowing_external_endpoints(
            "dev",
            created_bytes("999").as_bytes(),
            0o600,
        );
        let out = rs(
            &sandbox,
            &["tenant", "unset", "dev", "defaultOutput"],
            writes(),
        );
        assert_failure(&out, PIN);
    }
}

mod switch {
    use super::*;

    #[test]
    fn selects_and_always_rewrites_the_index() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let out = ok(&sandbox, &["tenant", "switch", "dev"]);
        assert_eq!(out.stderr, "  ✓ Selected dev (TSG 100)\n");
        assert_eq!(
            index(&sandbox),
            "version = 1\nactive = \"dev\"\n\n[tenants.dev]\ntsg_id = \"100\"\n"
        );
        let first = inode(&sandbox.store_index_path());
        ok(&sandbox, &["tenant", "switch", "dev"]);
        assert_ne!(inode(&sandbox.store_index_path()), first);
        let list = rs(&sandbox, &["tenant", "list", "--output", "json"], reads());
        assert_eq!(list.json()[0]["active"], true);
        assert_eq!(list.stderr, "");
    }

    #[test]
    fn failures_leave_the_selection_unchanged() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        create(&sandbox, "prod", "200");
        ok(&sandbox, &["tenant", "switch", "dev"]);
        let selected = index(&sandbox);
        let path = sandbox.store_tenant_path("prod");
        fs::remove_file(&path).unwrap();
        let out = rs(&sandbox, &["tenant", "switch", "prod"], writes());
        assert_failure(
            &out,
            &format!("Cannot read a valid tenant config at {}", path.display()),
        );
        sandbox.write_tenant_file_allowing_external_endpoints(
            "prod",
            created_bytes("201").as_bytes(),
            0o600,
        );
        let out = rs(&sandbox, &["tenant", "switch", "prod"], writes());
        assert_failure(&out, PIN);
        let out = rs(&sandbox, &["tenant", "switch", "ghost"], reads());
        assert_failure(&out, "Tenant not found");
        let out = rs(&sandbox, &["tenant", "switch", "DEV"], reads());
        assert_failure(&out, "Tenant not found");
        assert_eq!(index(&sandbox), selected);
    }

    #[test]
    fn an_empty_or_missing_store_creates_nothing() {
        let sandbox = Sandbox::new();
        let out = rs(&sandbox, &["tenant", "switch", "dev"], RunOptions::new());
        assert_failure(&out, "Tenant not found");
        let missing = sandbox.store_dir().join("missing");
        let out = rs(
            &sandbox,
            &["tenant", "switch", "dev"],
            RunOptions::new().env("AIRS_CLI_CONFIG_DIR", missing.display().to_string()),
        );
        assert_failure(&out, "Tenant not found");
        assert!(!missing.exists());
    }

    #[test]
    fn a_tenant_with_only_mgmt_tsg_id_can_be_selected() {
        let sandbox = Sandbox::new();
        store_with(&sandbox, &[("bare", "100", "mgmtTsgId = \"100\"\n")], None);
        let out = ok(&sandbox, &["tenant", "switch", "bare"]);
        assert_eq!(out.stderr, "  ✓ Selected bare (TSG 100)\n");
    }

    #[test]
    fn a_read_only_index_is_refused() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        set_mode(&sandbox.store_index_path(), 0o400);
        let out = rs(&sandbox, &["tenant", "switch", "dev"], writes());
        assert_failure(
            &out,
            &format!(
                "{} is read-only; restore write permission (chmod 600) to change it",
                sandbox.store_index_path().display()
            ),
        );
        assert!(!index(&sandbox).contains("active"));
    }
}

mod delete {
    use super::*;

    #[test]
    fn force_removes_the_entry_the_file_and_the_selection() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        create(&sandbox, "prod", "200");
        ok(&sandbox, &["tenant", "switch", "dev"]);
        let out = ok(&sandbox, &["tenant", "delete", "dev", "--force"]);
        assert_eq!(
            out.stderr,
            "  ✓ Deleted dev and its config file; no tenant is selected\n"
        );
        assert_eq!(
            index(&sandbox),
            "version = 1\n\n[tenants.prod]\ntsg_id = \"200\"\n"
        );
        assert!(!sandbox.store_tenant_path("dev").exists());
        let out = ok(&sandbox, &["tenant", "rm", "prod", "--force"]);
        assert_eq!(out.stderr, "  ✓ Deleted prod and its config file\n");
        assert_eq!(index(&sandbox), "version = 1\n");
        assert_eq!(
            listing(&sandbox.store_dir()),
            ["config.lock", "config.toml", "tenants"]
        );
        let out = rs(&sandbox, &["tenant", "read"], reads());
        assert_failure(
            &out,
            "No tenant selected; run 'airs tenant create <name>' first",
        );
    }

    #[test]
    fn an_unselected_tenant_keeps_the_selection() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        create(&sandbox, "prod", "200");
        ok(&sandbox, &["tenant", "switch", "dev"]);
        let out = ok(&sandbox, &["tenant", "delete", "prod", "--force"]);
        assert_eq!(out.stderr, "  ✓ Deleted prod and its config file\n");
        assert_eq!(
            index(&sandbox),
            "version = 1\nactive = \"dev\"\n\n[tenants.dev]\ntsg_id = \"100\"\n"
        );
    }

    #[test]
    fn the_non_interactive_refusal_comes_before_the_store() {
        let sandbox = Sandbox::new();
        sandbox.write_store_raw(b"garbage [", 0o600);
        for args in [&["tenant", "delete", "nope"][..], &["tenant", "rm", "dev"]] {
            let out = rs(&sandbox, args, RunOptions::new());
            assert_usage(
                &out,
                &format!(
                    "refusing to delete tenant {} without --force in non-interactive mode",
                    args[2]
                ),
            );
        }
        let sandbox = Sandbox::new();
        let out = rs(
            &sandbox,
            &["tenant", "delete", "nope", "--force"],
            RunOptions::new(),
        );
        assert_failure(&out, "Tenant not found");
    }

    #[test]
    fn broken_tenant_files_can_be_deleted() {
        let sandbox = Sandbox::new();
        let path = sandbox.store_tenant_path("dev");
        for prepare in [
            &(|_: &Sandbox, path: &Path| fs::remove_file(path).unwrap())
                as &dyn Fn(&Sandbox, &Path),
            &|sandbox: &Sandbox, _: &Path| {
                sandbox.write_tenant_file_allowing_external_endpoints("dev", b"not toml [", 0o600);
            },
            &|sandbox: &Sandbox, _: &Path| {
                sandbox.write_tenant_file_allowing_external_endpoints(
                    "dev",
                    b"mgmtTsgId = \"999\"\n",
                    0o644,
                );
            },
        ] {
            create(&sandbox, "dev", "100");
            prepare(&sandbox, &path);
            let out = ok(&sandbox, &["tenant", "delete", "dev", "--force"]);
            assert_eq!(out.stderr, "  ✓ Deleted dev and its config file\n");
            assert!(!path.exists());
        }
        // A symlink in place of the file is removed; its target is not.
        create(&sandbox, "dev", "100");
        let target = sandbox.store_dir().join("kept.toml");
        fs::rename(&path, &target).unwrap();
        symlink(&target, &path).unwrap();
        ok(&sandbox, &["tenant", "delete", "dev", "--force"]);
        assert!(fs::symlink_metadata(&path).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), created_bytes("100"));
    }

    #[test]
    fn a_file_that_cannot_be_removed_is_reported_after_the_commit() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let path = sandbox.store_tenant_path("dev");
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        fs::write(path.join("x"), b"x").unwrap();
        let out = rs(&sandbox, &["tenant", "delete", "dev", "--force"], writes());
        assert_failure(
            &out,
            &format!(
                "Deleted tenant dev, but {} could not be removed; remove it by hand",
                path.display()
            ),
        );
        assert_eq!(index(&sandbox), "version = 1\n", "the entry is gone");
    }

    /// DESIGN §3.8, D79: a symlinked or group/other-writable `D/tenants` is refused by
    /// every write, before `config.toml` is written and before any stale temporary file
    /// is swept, so neither delete nor the sweep reaches a file outside the store.
    #[test]
    fn an_unsafe_tenants_directory_is_refused_before_anything_changes() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        create(&sandbox, "prod", "200");
        ok(&sandbox, &["tenant", "switch", "dev"]);
        let tenants = sandbox.store_dir().join("tenants");
        let elsewhere = sandbox.root().join("elsewhere");
        fs::rename(&tenants, &elsewhere).unwrap();
        symlink(&elsewhere, &tenants).unwrap();
        let stale = elsewhere.join(".notes.toml.1.0123456789abcdef.tmp");
        fs::write(&stale, b"x").unwrap();
        set_mode(&stale, 0o600);
        let refusal = format!(
            "Refusing to use {}: it must be a directory (not a symlink) owned by the current user and not writable by group or others",
            tenants.display()
        );
        let index_before = index(&sandbox);
        // `reads()`: the refusals change nothing anywhere in the sandbox.
        for args in [
            &["tenant", "delete", "dev", "--force"][..],
            &["tenant", "switch", "prod"],
            &["tenant", "unset", "dev", "defaultOutput"],
            &["tenant", "set", "dev", "defaultOutput", "json"],
        ] {
            let out = rs(&sandbox, args, reads());
            assert_failure(&out, &refusal);
        }
        assert_eq!(index(&sandbox), index_before);
        assert_eq!(
            fs::read_to_string(elsewhere.join("dev.toml")).unwrap(),
            created_bytes("100")
        );
        assert!(stale.exists(), "the sweep never follows the symlink");
        // A group-writable directory is refused the same way.
        fs::remove_file(&tenants).unwrap();
        fs::remove_file(&stale).unwrap();
        fs::rename(&elsewhere, &tenants).unwrap();
        set_mode(&tenants, 0o770);
        let out = rs(&sandbox, &["tenant", "delete", "dev", "--force"], reads());
        assert_failure(&out, &refusal);
        assert_eq!(tenant_file(&sandbox, "dev"), created_bytes("100"));
        set_mode(&tenants, 0o700);
        ok(&sandbox, &["tenant", "delete", "dev", "--force"]);
        assert!(!sandbox.store_tenant_path("dev").exists());
    }

    #[test]
    fn the_confirmation_accepts_only_y_and_yes() {
        for (answer, deleted) in [
            (&b"y\n"[..], true),
            (b"yes\n", true),
            (b"Y\n", true),
            (b" YES \n", true),
            (b"\n", false),
            (b"n\n", false),
            (b"yolo\n", false),
            (b"\x04", false),
        ] {
            let sandbox = Sandbox::new();
            create(&sandbox, "dev", "100");
            ok(&sandbox, &["tenant", "switch", "dev"]);
            let options = if deleted { writes() } else { reads() };
            let mut session = pty(&sandbox, &["tenant", "delete", "dev"], options);
            session.expect("? Delete tenant dev? It is the selected tenant; no tenant will be selected afterwards. Its config file will be deleted. (y/N) ");
            session.send(answer);
            let out = session.wait();
            out.run.assert_success();
            assert_eq!(out.run.stdout, "");
            let shown = transcript(&out);
            if deleted {
                assert!(
                    shown.ends_with("  ✓ Deleted dev and its config file; no tenant is selected\n"),
                    "{shown:?}"
                );
                assert!(!sandbox.store_tenant_path("dev").exists());
            } else {
                assert!(shown.ends_with("  ℹ Aborted\n"), "{answer:?}: {shown:?}");
                assert!(sandbox.store_tenant_path("dev").exists());
            }
        }
    }

    #[test]
    fn the_confirmation_names_an_unselected_tenant_and_ctrl_c_cancels() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let mut session = pty(&sandbox, &["tenant", "delete", "dev"], reads());
        session.expect("? Delete tenant dev? Its config file will be deleted. (y/N) ");
        session.signal("INT");
        let out = session.wait();
        out.run.assert_code(130);
        assert!(transcript(&out).ends_with(&format!("(y/N) \n  {CANCELLED}\n")));
        // Under --quiet the decline prints nothing.
        let mut session = pty(&sandbox, &["--quiet", "tenant", "delete", "dev"], reads());
        session.expect("(y/N) ");
        session.send(b"n\n");
        let out = session.wait();
        out.run.assert_success();
        assert!(!out.transcript.contains("Aborted"));
    }
}

mod streams {
    use super::*;

    #[test]
    fn quiet_hides_every_outcome_line() {
        let sandbox = Sandbox::new();
        let out = rs(
            &sandbox,
            &[
                "--quiet",
                "tenant",
                "create",
                "dev",
                "--tsg-id",
                "1",
                "--client-id",
                "c",
                "--client-secret-stdin",
            ],
            writes().stdin(format!("{SECRET}\n")),
        );
        out.assert_success();
        assert_eq!((out.stdout.as_str(), out.stderr.as_str()), ("", ""));
        for args in [
            &["tenant", "switch", "dev", "--quiet"][..],
            &["tenant", "set", "dev", "defaultOutput", "json", "--quiet"],
            &["tenant", "unset", "dev", "scanConcurrency", "--quiet"],
            &["tenant", "delete", "dev", "--force", "--quiet"],
        ] {
            let out = ok(&sandbox, args);
            assert_eq!(out.stderr, "", "{args:?}");
        }
    }

    #[test]
    fn outcome_lines_are_coloured_by_the_stderr_decision() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let colour = || writes().without_env("NO_COLOR").env("CLICOLOR_FORCE", "1");
        let out = rs(&sandbox, &["tenant", "switch", "dev"], colour());
        assert_eq!(
            out.stderr,
            "  \u{1b}[32m✓ Selected dev (TSG 100)\u{1b}[39m\n"
        );
        let out = rs(
            &sandbox,
            &["tenant", "unset", "dev", "defaultOutput"],
            colour(),
        );
        assert_eq!(
            out.stderr,
            "  \u{1b}[36mℹ defaultOutput is not set for tenant dev — nothing to do\u{1b}[39m\n"
        );
        assert_eq!(out.stdout, "");
    }

    #[test]
    fn output_is_validated_then_ignored_and_debug_writes_no_log() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        for format in ["json", "table", "pretty"] {
            let out = ok(&sandbox, &["tenant", "switch", "dev", "--output", format]);
            assert_eq!(out.stderr, "  ✓ Selected dev (TSG 100)\n");
        }
        let out = rs(
            &sandbox,
            &["tenant", "switch", "dev", "--output", "xml"],
            reads(),
        );
        assert_usage(
            &out,
            "Invalid output format 'xml'. Expected: pretty, table, json",
        );
        let home = sandbox.home_listing();
        ok(
            &sandbox,
            &["--debug", "tenant", "set", "dev", "defaultOutput", "json"],
        );
        assert_eq!(sandbox.home_listing(), home, "no debug log");
        let out = rs(
            &sandbox,
            &["--tenant", "dev", "tenant", "switch", "dev"],
            reads(),
        );
        assert_usage(
            &out,
            "--tenant cannot be used with 'tenant switch'; name the tenant as an argument",
        );
    }

    #[test]
    fn outcome_lines_are_terminal_safe() {
        let sandbox = Sandbox::new();
        store_with(
            &sandbox,
            &[("dev", "1\u{1b}[31m", "mgmtTsgId = \"1\\u001B[31m\"\n")],
            None,
        );
        let out = ok(&sandbox, &["tenant", "switch", "dev"]);
        assert_eq!(out.stderr, "  ✓ Selected dev (TSG 1\u{fffd}[31m)\n");
    }

    #[test]
    fn help_lists_the_write_commands() {
        let out = rs(&Sandbox::new(), &["tenant", "--help"], RunOptions::new());
        out.assert_success();
        for line in [
            "create    Create a tenant with guided prompts; does not select it",
            "set       Update one tenant setting; prompt when omitted, hide secrets",
            "unset     Remove one tenant setting so the default applies; credentials cannot be cleared",
            "switch    Select the tenant for subsequent commands",
            "delete    Delete a tenant and its config file; clears the selection if it was selected [aliases: rm]",
        ] {
            assert!(out.stdout.contains(line), "{line}\n{}", out.stdout);
        }
        let out = rs(
            &Sandbox::new(),
            &["tenant", "set", "--help"],
            RunOptions::new(),
        );
        assert!(out.stdout.contains(
            "Read one value from piped stdin (recommended for automated secret updates)"
        ));
        assert!(out.stdout.contains(
            "Ignored by this command (it prints no data); must still be one of pretty, table, json"
        ));
        let out = rs(
            &Sandbox::new(),
            &["tenant", "create", "dev", "--config", "x.json"],
            RunOptions::new(),
        );
        out.assert_code(2);
        assert!(
            out.stderr.contains("unexpected argument '--config'"),
            "{}",
            out.stderr
        );
    }
}

mod locking {
    use super::*;

    const HELPER: &str = "locking::lock_holder_helper";

    /// Not a test: the lock-holder process the tests below start (it returns at once
    /// without the harness variables).
    #[test]
    #[ignore = "helper process for the lock tests"]
    fn lock_holder_helper() {
        lock_holder::run_from_env();
    }

    fn waiting_line(holder: &lock_holder::Holder) -> String {
        format!(
            "  Waiting for another airs change to finish ({}",
            holder.holder_prefix()
        )
    }

    #[test]
    fn a_held_lock_is_waited_for_and_its_holder_named() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let holder = lock_holder::spawn(HELPER, &sandbox.store_dir(), Duration::from_secs(30));
        let started = Instant::now();
        let out = rs(&sandbox, &["tenant", "switch", "dev"], writes());
        let elapsed = started.elapsed();
        out.assert_code(1);
        assert!(
            elapsed >= Duration::from_secs(5) && elapsed < Duration::from_secs(9),
            "{elapsed:?}"
        );
        let lines: Vec<&str> = out.stderr.lines().collect();
        assert!(
            lines[0].starts_with(&waiting_line(&holder)) && lines[0].ends_with(')'),
            "{lines:?}"
        );
        assert_eq!(
            out.stderr.matches("Waiting for another").count(),
            1,
            "printed once"
        );
        let error = format!(
            "\n  ✗ Error: Another change is in progress: {} is held by {}",
            sandbox.store_dir().join("config.lock").display(),
            holder.holder_prefix()
        );
        assert!(out.stderr.contains(&error), "{}", out.stderr);
        assert!(out.stderr.ends_with("; try again when it finishes\n\n"));
        assert!(!index(&sandbox).contains("active"), "nothing written");
        holder.kill();
    }

    #[test]
    fn a_lock_released_within_the_bound_is_taken() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let holder = lock_holder::spawn(HELPER, &sandbox.store_dir(), Duration::from_millis(1000));
        let prefix = waiting_line(&holder);
        let out = ok(&sandbox, &["tenant", "switch", "dev"]);
        assert!(out.stderr.starts_with(&prefix), "{}", out.stderr);
        assert!(
            out.stderr.ends_with(")\n  ✓ Selected dev (TSG 100)\n"),
            "{}",
            out.stderr
        );
        holder.wait();
        // --quiet hides the waiting line.
        let holder = lock_holder::spawn(HELPER, &sandbox.store_dir(), Duration::from_millis(500));
        let out = ok(&sandbox, &["--quiet", "tenant", "switch", "dev"]);
        assert_eq!(out.stderr, "");
        holder.wait();
    }

    #[test]
    fn a_killed_holder_never_blocks_and_its_record_is_overwritten() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let lock = sandbox.store_dir().join("config.lock");
        let holder = lock_holder::spawn(HELPER, &sandbox.store_dir(), Duration::from_secs(30));
        let prefix = holder
            .holder_prefix()
            .replace("pid ", "pid=")
            .replace(" on ", "\nhost=");
        holder.kill();
        let stale = fs::read_to_string(&lock).unwrap();
        assert!(
            stale.starts_with(prefix.trim_end_matches(" since ")),
            "{stale:?}"
        );
        let started = Instant::now();
        let out = ok(&sandbox, &["tenant", "switch", "dev"]);
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(out.stderr, "  ✓ Selected dev (TSG 100)\n", "no wait");
        assert_eq!(fs::read(&lock).unwrap(), b"", "the stale record is gone");
    }

    #[test]
    fn a_signal_while_waiting_for_the_lock_changes_nothing() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let holder = lock_holder::spawn(HELPER, &sandbox.store_dir(), Duration::from_secs(30));
        let running = spawn(&rust(), &sandbox, &["tenant", "switch", "dev"], &writes());
        std::thread::sleep(Duration::from_millis(1000));
        running.send_signal("INT");
        let out = running.wait();
        out.assert_code(130);
        assert!(
            out.stderr
                .starts_with("  Waiting for another airs change to finish")
        );
        assert!(
            out.stderr
                .ends_with("\n  ✗ Error: Interrupted; no configuration was changed\n\n"),
            "{}",
            out.stderr
        );
        assert!(!index(&sandbox).contains("active"));
        holder.kill();
    }

    #[test]
    fn stale_temporary_files_are_removed_under_the_lock() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let store = sandbox.store_dir();
        for name in [
            ".config.toml.12.0123456789abcdef.tmp",
            "tenants/.dev.toml.3.fedcba9876543210.tmp",
            "notes.tmp",
            ".config.toml.12.0123456789abcdeg.tmp",
            "tenants/.dev.json.3.fedcba9876543210.tmp",
        ] {
            fs::write(store.join(name), b"x").unwrap();
            set_mode(&store.join(name), 0o600);
        }
        ok(&sandbox, &["tenant", "switch", "dev"]);
        assert_eq!(
            listing(&store),
            [
                ".config.toml.12.0123456789abcdeg.tmp",
                "config.lock",
                "config.toml",
                "notes.tmp",
                "tenants",
                "tenants/.dev.json.3.fedcba9876543210.tmp",
                "tenants/dev.toml",
            ]
        );
    }

    #[test]
    fn concurrent_sets_of_different_keys_lose_no_update() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        let settings = [
            ("airsNumRetries", "3"),
            ("mgmtEndpoint", "http://127.0.0.1:9/m"),
            ("dlpEndpoint", "http://127.0.0.1:9/d"),
            ("gatewayInferenceModel", "model-x"),
            ("scanConcurrency", "7"),
            ("defaultOutput", "json"),
        ];
        let running: Vec<_> = settings
            .iter()
            .map(|(key, value)| {
                spawn(
                    &rust(),
                    &sandbox,
                    &["tenant", "set", "dev", key, value],
                    &writes(),
                )
            })
            .collect();
        for run in running {
            run.wait().assert_success();
        }
        assert_eq!(
            tenant_file(&sandbox, "dev"),
            format!(
                "airsNumRetries = 3\nmgmtClientId = \"client-100\"\nmgmtClientSecret = \"{SECRET}\"\nmgmtTsgId = \"100\"\nmgmtEndpoint = \"http://127.0.0.1:9/m\"\ndlpEndpoint = \"http://127.0.0.1:9/d\"\ngatewayInferenceModel = \"model-x\"\nscanConcurrency = 7\ndefaultOutput = \"json\"\n"
            )
        );
    }

    #[test]
    fn concurrent_creates_all_register() {
        let sandbox = Sandbox::new();
        let names = ["a1", "b2", "c3", "d4", "e5", "f6"];
        let running: Vec<_> = names
            .iter()
            .enumerate()
            .map(|(n, name)| {
                let tsg = format!("{}", 100 + n);
                spawn(
                    &rust(),
                    &sandbox,
                    &[
                        "tenant",
                        "create",
                        name,
                        "--tsg-id",
                        &tsg,
                        "--client-id",
                        "c",
                        "--client-secret-stdin",
                    ],
                    &writes().stdin(format!("{SECRET}\n")),
                )
            })
            .collect();
        for run in running {
            run.wait().assert_success();
        }
        let list = rs(&sandbox, &["tenant", "list", "--output", "json"], reads());
        let listed: Vec<String> = list
            .json()
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(listed, names);
        for name in names {
            assert!(sandbox.store_tenant_path(name).exists());
        }
    }

    #[test]
    fn readers_never_see_a_partial_file() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        ok(&sandbox, &["tenant", "switch", "dev"]);
        std::thread::scope(|scope| {
            let writer = scope.spawn(|| {
                for n in 0..15 {
                    let format = ["json", "table", "pretty"][n % 3];
                    ok(&sandbox, &["tenant", "set", "dev", "defaultOutput", format]);
                    ok(&sandbox, &["tenant", "switch", "dev"]);
                }
            });
            for _ in 0..30 {
                let out = rs(
                    &sandbox,
                    &["tenant", "read", "--output", "json"],
                    reads().allow_writes_under("store"),
                );
                out.assert_success();
            }
            writer.join().unwrap();
        });
    }
}

mod signals {
    use super::*;

    #[test]
    fn a_signal_while_reading_stdin_changes_nothing() {
        let sandbox = Sandbox::new();
        create(&sandbox, "dev", "100");
        for (signal, code) in [("INT", 130), ("TERM", 143)] {
            let running = spawn(
                &rust(),
                &sandbox,
                &["tenant", "set", "dev", "airsApiKey", "--stdin"],
                &reads().stdin_held_open(),
            );
            std::thread::sleep(Duration::from_millis(500));
            running.send_signal(signal);
            let out = running.wait();
            out.assert_code(code);
            assert_eq!(
                out.stderr,
                op_error("Interrupted; no configuration was changed")
            );
            assert_eq!(tenant_file(&sandbox, "dev"), created_bytes("100"));
        }
    }
}
