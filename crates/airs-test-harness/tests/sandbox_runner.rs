//! Self-tests of the sandbox and the runner invariants, using `/bin/sh` as the child.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use airs_test_harness::{
    Cli, ENDPOINT_KEYS, FixtureServer, PATH_VARIABLES, Planned, RunOptions, Sandbox, base64_encode,
    run, spawn, store,
};
use serde_json::json;

fn sh() -> Cli {
    Cli::rust("/bin/sh")
}

fn mode(path: &std::path::Path) -> u32 {
    fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

#[test]
fn fixture_tenant_layout_modes_and_endpoints() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let tenant = sandbox.single_tenant(&server, "prod", "100");

    assert_eq!(tenant.config_path, sandbox.config_path("prod"));
    assert_eq!(mode(&tenant.config_path), 0o400);
    assert_eq!(mode(&sandbox.registry_path()), 0o600);
    let registry: serde_json::Value =
        serde_json::from_slice(&fs::read(sandbox.registry_path()).expect("registry"))
            .expect("json");
    assert_eq!(
        registry,
        json!({"version": 1, "active": "prod", "tenants": [
            {"name": "prod", "configPath": tenant.config_path.display().to_string(), "tsgId": "100"}
        ]})
    );

    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(&tenant.config_path).expect("config")).expect("json");
    for key in ENDPOINT_KEYS {
        let value = config[key]
            .as_str()
            .unwrap_or_else(|| panic!("{key} missing"));
        assert!(value.starts_with(&server.base_url()), "{key} = {value}");
    }
    assert_eq!(config["airsNumRetries"], 0);
    assert_eq!(config["mgmtTsgId"], "100");

    let secrets = sandbox.secrets();
    // Base64 forms are unpadded: a substring of the padded encoding, so both match.
    let unpadded = |bytes: &[u8]| base64_encode(bytes).trim_end_matches('=').to_owned();
    for expected in [
        "FAKE-SECRET-100".to_owned(),
        "FAKE-APIKEY-100".to_owned(),
        unpadded(b"FAKE-SECRET-100"),
        unpadded(b"client-100:FAKE-SECRET-100"),
        "mock-token-".to_owned(),
        // The base64 of every issued token starts with this.
        "bW9jay10b2tl".to_owned(),
    ] {
        assert!(
            secrets.contains(&expected),
            "missing secret form {expected}"
        );
    }
    assert!(!secrets.iter().any(|secret| secret == "client-100"));

    // Rewriting a read-only config works and keeps mode 0400.
    let mut rewritten = tenant.config.to_value();
    rewritten["mgmtTsgId"] = json!("200");
    sandbox.write_config("prod", &rewritten);
    assert_eq!(mode(&tenant.config_path), 0o400);
}

#[test]
fn snapshot_detects_content_mode_and_new_files() {
    let sandbox = Sandbox::new();
    sandbox.write_registry(&[], None);
    sandbox.write_config_raw("a", b"[]");
    let before = sandbox.snapshot();
    sandbox.assert_unchanged(&before);

    let changed = |mutate: &dyn Fn()| {
        let before = sandbox.snapshot();
        mutate();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sandbox.assert_unchanged(&before)
        }))
        .is_err()
    };
    assert!(changed(&|| sandbox.write_registry_raw(b"{}")));
    assert!(changed(&|| {
        fs::set_permissions(sandbox.config_path("a"), fs::Permissions::from_mode(0o600))
            .expect("chmod");
    }));
    assert!(changed(&|| {
        fs::write(sandbox.state_dir().join("tenants.json.lock"), b"").expect("lock");
    }));
    assert!(changed(&|| fs::remove_file(
        sandbox.state_dir().join("tenants.json.lock")
    )
    .expect("rm")));
    // HOME and cwd are not part of the tenant snapshot.
    assert!(!changed(&|| fs::write(
        sandbox.cwd().join("out.jsonl"),
        b""
    )
    .expect("write")));
    assert_eq!(
        sandbox.cwd_listing(),
        vec![std::path::PathBuf::from("out.jsonl")]
    );
}

#[test]
fn runner_uses_allowlisted_environment_null_stdin_and_sandbox_cwd() {
    let sandbox = Sandbox::new();
    let options = RunOptions::new()
        .env("EXTRA_VAR", "1")
        .without_env("TMPDIR");
    let out = run(
        &sh(),
        &sandbox,
        &["-c", "env | sort; pwd; cat; echo err >&2"],
        &options,
    );
    out.assert_success();
    let mut names: Vec<&str> = out
        .stdout
        .lines()
        .filter_map(|line| line.split_once('=').map(|(name, _)| name))
        .collect();
    // sh itself may export PWD/SHLVL/_ ; ignore those shell-maintained names.
    names.retain(|name| !matches!(*name, "PWD" | "SHLVL" | "_" | "OLDPWD"));
    assert_eq!(
        names,
        vec![
            "AIRS_CLI_CONFIG_DIR",
            "EXTRA_VAR",
            "HOME",
            "NO_COLOR",
            "NO_PROXY",
            "PATH",
            "PRISMA_AIRS_TENANTS_PATH",
            "XDG_DATA_HOME"
        ]
    );
    // Nothing reaches the real `~/.local/share/airs-cli` (DESIGN §10 M6c).
    assert!(
        out.stdout
            .contains(&format!("XDG_DATA_HOME={}", sandbox.data_dir().display()))
    );
    assert!(out.stdout.contains(&format!(
        "AIRS_CLI_CONFIG_DIR={}",
        sandbox.store_dir().display()
    )));
    assert!(
        out.stdout
            .contains(&format!("HOME={}", sandbox.home().display()))
    );
    assert!(out.stdout.contains("NO_PROXY=127.0.0.1,localhost"));
    assert!(
        out.stdout
            .contains(&format!("\n{}\n", sandbox.cwd().display()))
    );
    assert_eq!(out.stderr, "err\n");
}

#[test]
fn runner_reports_exit_codes() {
    let sandbox = Sandbox::new();
    let out = run(&sh(), &sandbox, &["-c", "exit 3"], &RunOptions::new());
    assert_eq!((out.code, out.signal), (3, None));
}

#[test]
#[should_panic(expected = "died by signal")]
fn runner_rejects_unexpected_signal_death() {
    let sandbox = Sandbox::new();
    run(
        &sh(),
        &sandbox,
        &["-c", "kill -TERM $$"],
        &RunOptions::new(),
    );
}

#[test]
fn runner_accepts_expected_signal_death() {
    let sandbox = Sandbox::new();
    let out = run(
        &sh(),
        &sandbox,
        &["-c", "kill -TERM $$"],
        &RunOptions::new().expect_signal(),
    );
    assert_eq!((out.code, out.signal), (143, Some(15)));
}

#[test]
#[should_panic(expected = "timed out")]
fn runner_kills_and_fails_on_timeout() {
    let sandbox = Sandbox::new();
    run(
        &sh(),
        &sandbox,
        &["-c", "sleep 30"],
        &RunOptions::new().timeout(Duration::from_millis(200)),
    );
}

#[test]
#[should_panic(expected = "prod mgmtClientSecret (base64) appeared in stderr")]
fn runner_detects_secret_leaks() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    let leak = format!("echo {} >&2", base64_encode(b"FAKE-SECRET-100"));
    run(&sh(), &sandbox, &["-c", &leak], &RunOptions::new());
}

#[test]
fn runner_leak_check_can_be_disabled_and_extended() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    let out = run(
        &sh(),
        &sandbox,
        &["-c", "echo FAKE-SECRET-100"],
        &RunOptions::new().allow_secret_output(),
    );
    assert_eq!(out.stdout, "FAKE-SECRET-100\n");
    let leaked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run(
            &sh(),
            &sandbox,
            &["-c", "echo ENV-SECRET-VALUE"],
            &RunOptions::new().secret("ENV-SECRET-VALUE"),
        )
    }));
    assert!(leaked.is_err());
}

#[test]
fn spawned_process_receives_signals() {
    let sandbox = Sandbox::new();
    let running = spawn(
        &sh(),
        &sandbox,
        &[
            "-c",
            "trap 'echo got-int; exit 130' INT; echo ready; while :; do sleep 0.05; done",
        ],
        &RunOptions::new().timeout(Duration::from_secs(10)),
    );
    std::thread::sleep(Duration::from_millis(300));
    running.send_signal("SIGINT");
    let out = running.wait();
    assert_eq!(out.code, 130);
    assert!(out.stdout.contains("got-int"), "{out:?}");
}

fn panics(f: impl FnOnce()) -> Option<String> {
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).err()?;
    Some(
        err.downcast_ref::<String>()
            .cloned()
            .or_else(|| err.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_default(),
    )
}

#[test]
fn config_writes_require_every_endpoint_on_loopback() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let standard = Sandbox::fixture_config(&server, "100");

    // Missing key, empty value, non-loopback host, non-string: all panic at write time.
    for (config, fragment) in [
        (
            standard.clone().without("dlpEndpoint"),
            "lacks endpoint keys dlpEndpoint",
        ),
        (
            standard.clone().with("iamEndpoint", ""),
            "not loopback http(s) URLs: iamEndpoint",
        ),
        (
            standard
                .clone()
                .with("mgmtEndpoint", "https://example.test/aisec"),
            "not loopback http(s) URLs: mgmtEndpoint",
        ),
        (
            standard.clone().with("airsApiEndpoint", "127.0.0.1/scan"),
            "not loopback http(s) URLs: airsApiEndpoint",
        ),
        (
            standard
                .clone()
                .with("typesafeBaseUrl", "https://example.test"),
            "not loopback http(s) URLs: typesafeBaseUrl",
        ),
    ] {
        let message = panics(|| {
            sandbox.write_fixture_config("bad", &config);
        })
        .expect("guard must panic");
        assert!(message.contains(fragment), "{message}");
        assert!(!sandbox.config_path("bad").exists());
    }
    let missing_all = panics(|| {
        sandbox.write_config("bad", &json!({"mgmtTsgId": "100"}));
    })
    .expect("guard must panic");
    assert!(missing_all.contains("airsApiEndpoint"), "{missing_all}");

    // Raw bytes that Node would decode into an object are guarded too (lossy UTF-8).
    assert!(
        panics(|| {
            sandbox.write_config_raw("bad", b"{\"mgmtTsgId\":\"100\",\"x\":\"\xff\"}");
        })
        .is_some()
    );
    assert!(
        panics(|| {
            sandbox.write_config_raw("bad", b"\xef\xbb\xbf{\"x\":\"\\ud800\"}");
        })
        .is_some()
    );
    // Documents that are not an object cannot configure a client: accepted.
    for bytes in [&b"{oops"[..], b"null", b"[]", b"[1, 2]", b"", b"\"x\""] {
        sandbox.write_config_raw("inert", bytes);
    }
    // Values that make both CLIs reject the whole config pass: non-strings, and
    // scheme-less strings for keys TS validates with z.string().url().
    sandbox.write_fixture_config(
        "invalid",
        &standard
            .clone()
            .with("airsApiEndpoint", 5)
            .with("dlpEndpoint", serde_json::Value::Null)
            .with("mgmtDashboardEndpoint", "not a url")
            .with("agentGuardDataEndpoint", ""),
    );
    // Loopback variants pass.
    sandbox.write_fixture_config(
        "ok",
        &standard
            .clone()
            .with("dlpEndpoint", "http://localhost:1/dlp")
            .with("iamEndpoint", "https://[::1]:443/iam")
            .with("typesafeBaseUrl", "http://127.9.9.9"),
    );
    assert!(sandbox.external_endpoint_configs().is_empty());
}

#[test]
fn loopback_url_rule() {
    for url in [
        "http://127.0.0.1:8080/x",
        "https://127.0.0.1",
        "http://LOCALHOST:1/",
        "http://[::1]:9/a?b",
        "http://127.1.2.3#f",
    ] {
        assert!(airs_test_harness::is_loopback_url(url), "{url}");
    }
    for url in [
        "",
        "127.0.0.1:1",
        "ftp://127.0.0.1/",
        "http://example.test",
        "http://0.0.0.0:1",
        "http://127.0.0.1@example.test/",
        "http://user:pw@127.0.0.1/",
        "http://127.0.0.1.example.test/",
        "http://127.1/",
        "http://127.0.0.%31/",
        "http://localhost./",
        "http://127.0.0.1\\@example.test/",
        "http://127.0.0.1\n.example.test/",
        " http://127.0.0.1/",
        "http://[::2]/",
        "http://127.0.0.1:/",
        "http://127.0.0.1:8a/",
    ] {
        assert!(!airs_test_harness::is_loopback_url(url), "{url:?}");
    }
}

#[test]
fn opted_out_sandbox_refuses_the_typescript_cli() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let config =
        Sandbox::fixture_config(&server, "100").with("mgmtEndpoint", "http://example.test");
    let path = sandbox.write_config_allowing_external_endpoints("prod", &config);
    assert_eq!(sandbox.external_endpoint_configs(), vec![path]);
    // The Rust side (here /bin/sh) still runs.
    run(&sh(), &sandbox, &["-c", "true"], &RunOptions::new()).assert_success();
    // Any TypeScript invocation is refused before a process starts (node need not exist).
    let ts = Cli::Ts {
        node: "/nonexistent/node".into(),
        entry: "/nonexistent/index.js".into(),
    };
    let message = panics(|| {
        run(&ts, &sandbox, &["--version"], &RunOptions::new());
    })
    .expect("TS run must be refused");
    assert!(
        message.contains("write_config_allowing_external_endpoints"),
        "{message}"
    );
}

#[test]
fn typescript_runs_refuse_non_loopback_endpoint_and_proxy_variables() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    let ts = Cli::Ts {
        node: "/nonexistent/node".into(),
        entry: "/nonexistent/index.js".into(),
    };
    for (name, value) in [
        ("PANW_MGMT_ENDPOINT", "http://example.test"),
        (
            "PANW_AI_SEC_API_ENDPOINT",
            "https://service.api.aisecurity.paloaltonetworks.com",
        ),
        ("panw_mgmt_token_endpoint", "http://10.0.0.1/token"),
        ("HTTPS_PROXY", "proxy.example.test:8080"),
        ("http_proxy", "http://example.test:3128"),
        ("ALL_PROXY", "socks5://127.0.0.1:1080"),
        ("NODE_OPTIONS", "--require ./x.js"),
    ] {
        let message = panics(|| {
            run(
                &ts,
                &sandbox,
                &["--version"],
                &RunOptions::new().env(name, value),
            );
        })
        .unwrap_or_else(|| panic!("TS run with {name}={value} must be refused"));
        assert!(message.contains(name), "{name}: {message}");
    }
    // Loopback or empty values, and NO_PROXY, pass the environment check.
    let mut env = std::collections::BTreeMap::new();
    env.insert("PANW_MGMT_ENDPOINT".to_owned(), server.url("/env-mgmt"));
    env.insert("HTTP_PROXY".to_owned(), String::new());
    env.insert("NO_PROXY".to_owned(), "example.test".to_owned());
    assert_eq!(airs_test_harness::env_guard_violation(&env), None);
    // Rust-side runs are not affected (they prove zero requests themselves).
    run(
        &sh(),
        &sandbox,
        &["-c", "true"],
        &RunOptions::new().env("PANW_MGMT_ENDPOINT", "http://example.test"),
    )
    .assert_success();
}

#[test]
fn typescript_runs_recheck_configs_written_around_the_guard() {
    let server = FixtureServer::start();
    let ts = Cli::Ts {
        node: "/nonexistent/node".into(),
        entry: "/nonexistent/index.js".into(),
    };
    let refused = |sandbox: &Sandbox, options: &RunOptions| {
        panics(|| {
            run(&ts, sandbox, &["--version"], options);
        })
        .expect("TS run must be refused")
    };

    // A config file written directly under configs/.
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    fs::write(sandbox.configs_dir().join("stray.json"), b"{}").expect("write");
    assert!(refused(&sandbox, &RunOptions::new()).contains("stray.json"));

    // A registered config elsewhere in the sandbox, through an XDG registry.
    let sandbox = Sandbox::new();
    let tenant = sandbox.add_fixture_tenant(&server, "prod", "100");
    let outside = sandbox.write_file("home/cfg.json", b"{\"mgmtTsgId\":\"100\"}", 0o600);
    sandbox.write_file(
        "state/xdg/prisma-airs/tenants.json",
        airs_test_harness::registry_json(
            &[airs_test_harness::RegistryEntry::new(
                "prod", &outside, "100",
            )],
            Some("prod"),
        )
        .as_bytes(),
        0o600,
    );
    let xdg = RunOptions::new()
        .without_env("PRISMA_AIRS_TENANTS_PATH")
        .env(
            "XDG_STATE_HOME",
            format!("{}/state/xdg", sandbox.root().display()),
        );
    assert!(refused(&sandbox, &xdg).contains("cfg.json"));
    // The default registry (standard tenant) is fine: the refusal above came from the
    // registry selected by the environment. A spawn failure (no node) is a different panic.
    sandbox.register(&[&tenant], Some("prod"));
    let message = refused(&sandbox, &RunOptions::new());
    assert!(message.contains("cannot start"), "{message}");

    // A registry that points outside the sandbox is never read.
    let sandbox = Sandbox::new();
    let tenant = sandbox.add_fixture_tenant(&server, "prod", "100");
    sandbox.register(&[&tenant], Some("prod"));
    let elsewhere = RunOptions::new().env("PRISMA_AIRS_TENANTS_PATH", "/etc/hosts");
    assert!(refused(&sandbox, &elsewhere).contains("outside the sandbox"));
}

#[test]
fn snapshot_with_covers_extra_paths() {
    let sandbox = Sandbox::new();
    let registry = sandbox.write_file("home/.local/state/prisma-airs/tenants.json", b"{}", 0o600);
    let before = sandbox.snapshot_with(&[std::path::Path::new("home")]);
    sandbox.assert_unchanged(&before);
    fs::write(&registry, b"[]").expect("write");
    assert!(panics(|| sandbox.assert_unchanged(&before)).is_some());
    // The plain snapshot does not cover home/.
    let plain = sandbox.snapshot();
    fs::write(&registry, b"{}").expect("write");
    sandbox.assert_unchanged(&plain);
    assert!(
        panics(|| {
            sandbox.snapshot_with(&[std::path::Path::new("/etc")]);
        })
        .is_some()
    );
}

#[test]
fn close_stdout_after_reads_only_the_first_bytes() {
    let sandbox = Sandbox::new();
    let options = RunOptions::new().close_stdout_after(3);
    let out = run(
        &sh(),
        &sandbox,
        &["-c", "printf abcdefgh; echo done >&2"],
        &options,
    );
    out.assert_success();
    assert_eq!(out.stdout, "abc");
    assert_eq!(out.stderr, "done\n");
    let out = run(&sh(), &sandbox, &["-c", "printf ab"], &options);
    assert_eq!(out.stdout, "ab");
}

#[test]
fn runner_fails_a_run_that_changes_any_sandbox_file() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    sandbox.write_file("xdg/prisma-airs/tenants.json", b"{}", 0o600);
    // A registry outside state/ and configs/, a lock file next to it, a file in home/:
    // each is caught by the runner itself, without a test-side snapshot.
    for script in [
        "printf x >> \"$0/xdg/prisma-airs/tenants.json\"",
        ": > \"$0/xdg/prisma-airs/tenants.json.lock\"",
        "chmod 0644 \"$0/state/tenants.json\"",
        ": > \"$HOME/reg.json\"",
        ": > new-in-cwd",
    ] {
        let root = sandbox.root().display().to_string();
        let problem = panics(|| {
            run(&sh(), &sandbox, &["-c", script, &root], &RunOptions::new());
        })
        .unwrap_or_else(|| panic!("{script}: no panic"));
        assert!(
            problem.contains("changed sandbox files"),
            "{script}: {problem}"
        );
    }
    // TMPDIR is scratch space; the opt-out admits deliberate writes.
    run(
        &sh(),
        &sandbox,
        &["-c", ": > \"$TMPDIR/scratch\""],
        &RunOptions::new(),
    )
    .assert_success();
    run(
        &sh(),
        &sandbox,
        &["-c", ": > \"$HOME/deliberate\""],
        &RunOptions::new().allow_tenant_writes(),
    )
    .assert_success();
}

#[test]
fn extra_and_trimmed_secrets_are_checked_in_every_form() {
    let sandbox = Sandbox::new();
    let encoded = base64_encode(b"ENV-SECRET-VALUE");
    let problem = panics(|| {
        run(
            &sh(),
            &sandbox,
            &["-c", &format!("echo {encoded}")],
            &RunOptions::new().secret("ENV-SECRET-VALUE"),
        );
    })
    .expect("base64 of a RunOptions::secret is caught");
    assert!(
        problem.contains("test-supplied secret (base64)"),
        "{problem}"
    );

    let server = FixtureServer::start();
    let padded = Sandbox::new();
    padded.add_fixture_tenant_with(&server, "prod", "100", |config| {
        config.set("airsApiKey", "  FAKE-APIKEY-100\t");
    });
    let problem = panics(|| {
        run(
            &sh(),
            &padded,
            &["-c", "echo key=FAKE-APIKEY-100"],
            &RunOptions::new(),
        );
    })
    .expect("the trimmed key is caught");
    assert!(
        problem.contains("prod airsApiKey, trimmed (raw)"),
        "{problem}"
    );
    let problem = panics(|| {
        run(
            &sh(),
            &padded,
            &["-c", "echo bW9jay10b2tlbi0xMDA="],
            &RunOptions::new(),
        );
    })
    .expect("base64 of an issued token is caught");
    assert!(
        problem.contains("issued access token prefix (base64)"),
        "{problem}"
    );
}

#[test]
fn stall_stdout_blocks_the_writer_until_the_child_exits() {
    let sandbox = Sandbox::new();
    let running = spawn(
        &sh(),
        &sandbox,
        &["-c", "exec head -c 400000 /dev/zero"],
        &RunOptions::new()
            .stall_stdout()
            .expect_signal()
            .timeout(Duration::from_secs(10)),
    );
    std::thread::sleep(Duration::from_millis(500));
    running.send_signal("TERM");
    let out = running.wait();
    assert_eq!(out.signal, Some(15), "{}", out.stderr);
    assert!(
        out.stdout.len() < 400_000,
        "the writer was blocked until the signal"
    );
}

/// A stalled stderr blocks the child's writes until it is released; `wait` releases it.
#[test]
fn stall_stderr_blocks_the_writer_until_released() {
    let sandbox = Sandbox::new();
    let marker = sandbox.tmp().join("after");
    let script = format!("head -c 400000 /dev/zero >&2; : > '{}'", marker.display());
    let mut running = spawn(
        &sh(),
        &sandbox,
        &["-c", &script],
        &RunOptions::new()
            .stall_stderr()
            .timeout(Duration::from_secs(10)),
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(!marker.exists(), "the writer was blocked");
    running.release_stderr();
    let out = running.wait();
    out.assert_success();
    assert_eq!(out.stderr.len(), 400_000);
    assert!(marker.exists());
    let out = spawn(
        &sh(),
        &sandbox,
        &["-c", "head -c 400000 /dev/zero >&2"],
        &RunOptions::new().stall_stderr(),
    )
    .wait();
    out.assert_success();
    assert_eq!(out.stderr.len(), 400_000, "wait releases a stalled stderr");
}

/// A filled stderr blocks the child's first write, however short, until it is released;
/// the runner's filler is not part of the captured stderr, and `wait` releases it.
#[test]
fn fill_stderr_blocks_the_first_write_until_released() {
    let sandbox = Sandbox::new();
    let marker = sandbox.tmp().join("after");
    let script = format!("printf x >&2; : > '{}'", marker.display());
    let mut running = spawn(
        &sh(),
        &sandbox,
        &["-c", &script],
        &RunOptions::new()
            .fill_stderr()
            .timeout(Duration::from_secs(10)),
    );
    // A pass never depends on this pause: a slow child only leaves the marker unwritten.
    std::thread::sleep(Duration::from_millis(500));
    assert!(!marker.exists(), "a one-byte write was blocked");
    running.release_stderr();
    let out = running.wait();
    out.assert_success();
    assert_eq!(out.stderr, "x");
    assert!(marker.exists());
    let out = spawn(
        &sh(),
        &sandbox,
        &["-c", "printf y >&2"],
        &RunOptions::new().fill_stderr(),
    )
    .wait();
    out.assert_success();
    assert_eq!(out.stderr, "y", "wait releases a filled stderr");
}

#[test]
fn planned_redirects_must_stay_on_loopback() {
    for location in [
        "https://api.apps.paloaltonetworks.com/aisec",
        "//example.com/x",
        "http://example.com/",
    ] {
        let problem = panics(|| {
            let _ = Planned::redirect(302, location);
        })
        .unwrap_or_else(|| panic!("{location}: accepted"));
        assert!(
            problem.contains("refusing to plan a non-loopback redirect"),
            "{problem}"
        );
        assert!(
            panics(|| {
                let _ = Planned::status(200).with_header("Location", location);
            })
            .is_some()
        );
    }
    for location in ["/elsewhere", "http://127.0.0.1:9/x", "https://localhost/y"] {
        assert_eq!(
            Planned::redirect(307, location).headers,
            vec![("location".to_owned(), location.to_owned())]
        );
    }
}

/// Every `--debug` log a run leaves (under `XDG_STATE_HOME`, the default state
/// directory or the working directory) is leak-checked like stdout and stderr.
#[test]
fn runner_leak_checks_debug_logs_in_every_location() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    let xdg = sandbox.tmp().join("xdg").display().to_string();
    let name = "debug-api-1000000000000-0000000a.jsonl";
    for (dir, options) in [
        (
            "$XDG_STATE_HOME/airs-cli/debug",
            RunOptions::new().env("XDG_STATE_HOME", xdg.clone()),
        ),
        (
            "$HOME/.local/state/airs-cli/debug",
            RunOptions::new().allow_writes_under("home/.local/state/airs-cli"),
        ),
        (".", RunOptions::new().allow_writes_under("cwd")),
    ] {
        let script = format!("mkdir -p \"{dir}\" && echo FAKE-SECRET-100 > \"{dir}/{name}\"");
        let problem = panics(|| {
            run(&sh(), &sandbox, &["-c", &script], &options);
        })
        .unwrap_or_else(|| panic!("{dir}: no panic"));
        assert!(
            problem.contains("prod mgmtClientSecret (raw) appeared in the debug log"),
            "{dir}: {problem}"
        );
        // A clean log in the same place passes, and other names are not logs.
        let script = format!(
            "mkdir -p \"{dir}\" && echo clean > \"{dir}/{name}\" && echo FAKE-SECRET-100 > \"{dir}/notes.txt\""
        );
        run(&sh(), &sandbox, &["-c", &script], &options).assert_success();
    }
}

/// `allow_writes_under` admits changes at or under one path (and the directories
/// created on the way to it) and nothing else.
#[test]
fn allow_writes_under_is_narrow() {
    let sandbox = Sandbox::new();
    let options = RunOptions::new().allow_writes_under("home/.local/state/airs-cli");
    run(
        &sh(),
        &sandbox,
        &["-c", "mkdir -p \"$HOME/.local/state/airs-cli/debug\" && : > \"$HOME/.local/state/airs-cli/debug/x\""],
        &options,
    )
    .assert_success();
    for script in [
        ": > \"$HOME/.local/state/other\"",
        "mkdir -p \"$HOME/.local/state/prisma-airs\"",
        ": > \"$HOME/elsewhere\"",
    ] {
        let problem = panics(|| {
            run(&sh(), &sandbox, &["-c", script], &options);
        })
        .unwrap_or_else(|| panic!("{script}: no panic"));
        assert!(
            problem.contains("changed sandbox files"),
            "{script}: {problem}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// M4t: the Rust store in every sandbox, and escape prevention (DESIGN §10)
// ---------------------------------------------------------------------------------------

#[test]
fn every_typescript_tenant_is_provisioned_into_the_rust_store() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    assert_eq!(mode(&sandbox.store_dir()), 0o700);
    assert!(
        !sandbox.store_index_path().exists(),
        "no registry, no config.toml"
    );
    let prod = sandbox.add_fixture_tenant_with(&server, "prod", "100", |config| {
        config
            .set("scanConcurrency", "3")
            .set("aiGwInferenceModel", "m")
            .set("dataDir", "/d")
            .set("airsApiToken", "")
            .set("defaultOutput", "yaml")
            .set("custom", 1);
    });
    let dev = sandbox.add_fixture_tenant(&server, "dev", "200");
    sandbox.register(&[&prod, &dev], Some("prod"));
    assert_eq!(
        fs::read_to_string(sandbox.store_index_path()).expect("index"),
        "version = 1\nactive = \"prod\"\n\n[tenants.dev]\ntsg_id = \"200\"\n\n[tenants.prod]\ntsg_id = \"100\"\n"
    );
    assert_eq!(mode(&sandbox.store_index_path()), 0o600);
    assert_eq!(mode(&sandbox.store_dir().join("tenants")), 0o700);
    assert_eq!(prod.store_path, sandbox.store_tenant_path("prod"));
    let text = fs::read_to_string(&prod.store_path).expect("tenant file");
    assert_eq!(mode(&prod.store_path), 0o600);
    // The import conversion: coerced, renamed, dropped, empty values dropped.
    assert!(text.contains("scanConcurrency = 3\n"), "{text}");
    assert!(text.contains("gatewayInferenceModel = \"m\"\n"), "{text}");
    for absent in [
        "dataDir",
        "airsApiToken",
        "defaultOutput",
        "custom",
        "typesafe",
        "aiGw",
    ] {
        assert!(!text.contains(absent), "{absent}: {text}");
    }
    let keys: Vec<&str> = text
        .lines()
        .filter_map(|line| line.split(" = ").next())
        .collect();
    let mut schema_order: Vec<&str> = store::RUST_KEYS
        .iter()
        .copied()
        .filter(|key| keys.contains(key))
        .collect();
    schema_order.dedup();
    assert_eq!(keys, schema_order, "canonical bytes are in schema order");
    for key in store::RUST_ENDPOINT_KEYS {
        assert!(
            text.contains(&format!("{key} = \"{}", server.base_url())),
            "{key}"
        );
    }
    // A later TypeScript write re-derives the store.
    sandbox.register(&[&dev], None);
    assert!(!sandbox.store_tenant_path("prod").exists());
    assert_eq!(
        fs::read_to_string(sandbox.store_index_path()).expect("index"),
        "version = 1\n\n[tenants.dev]\ntsg_id = \"200\"\n"
    );
    // TypeScript failures map to the same failure class.
    sandbox.write_registry_raw(b"{ corrupt");
    assert_eq!(
        fs::read_to_string(sandbox.store_index_path()).expect("index"),
        store::INVALID_TOML
    );
}

#[test]
fn rust_only_and_typescript_only_states() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let prod = sandbox.single_tenant(&server, "prod", "100");
    let dev = sandbox.add_fixture_tenant(&server, "dev", "200");
    // Rust-only: the store holds exactly these tenants; the TS registry is untouched.
    let registry = fs::read(sandbox.registry_path()).expect("registry");
    sandbox.rust_only(&[&dev], Some("dev"));
    assert!(!sandbox.mirrors_store());
    assert_eq!(
        fs::read(sandbox.registry_path()).expect("registry"),
        registry
    );
    assert!(sandbox.store_tenant_path("dev").exists());
    assert!(!sandbox.store_tenant_path("prod").exists());
    sandbox.register(&[&prod], Some("prod"));
    assert!(
        !sandbox.store_tenant_path("prod").exists(),
        "a detached store is not re-derived"
    );
    // TypeScript-only: an empty store.
    sandbox.ts_only();
    assert_eq!(fs::read_dir(sandbox.store_dir()).expect("store").count(), 0);
    // Raw store writes keep their modes and detach.
    let fresh = Sandbox::new();
    let path = fresh.write_store_raw(b"version = 1\n", 0o644);
    assert_eq!(mode(&path), 0o644);
    assert!(!fresh.mirrors_store());
}

#[test]
fn rust_tenant_files_must_stay_on_loopback() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let tenant = sandbox.single_tenant(&server, "prod", "100");
    let good = fs::read_to_string(&tenant.store_path).expect("tenant file");
    let external = good.replace(
        &format!("iamEndpoint = \"{}/iam\"", server.base_url()),
        "iamEndpoint = \"https://example.com/iam\"",
    );
    assert_ne!(good, external);
    let problem = panics(|| {
        sandbox.write_tenant_file_raw("prod", external.as_bytes(), 0o600);
    })
    .expect("panics");
    assert!(problem.contains("not loopback"), "{problem}");
    let missing: String = good
        .lines()
        .filter(|line| !line.starts_with("dlpEndpoint"))
        .map(|line| format!("{line}\n"))
        .collect();
    let problem = panics(|| {
        sandbox.write_tenant_file_raw("prod", missing.as_bytes(), 0o600);
    })
    .expect("panics");
    assert!(
        problem.contains("lacks endpoint keys dlpEndpoint"),
        "{problem}"
    );
    // A file written behind the harness's back is caught before a (Rust-kind) run.
    fs::write(&tenant.store_path, external.as_bytes()).expect("write");
    let problem = panics(|| {
        run(&sh(), &sandbox, &["-c", "true"], &RunOptions::new());
    })
    .expect("panics");
    assert!(
        problem.contains("refusing to run airs-cli-rs: tenant file")
            && problem.contains("not loopback"),
        "{problem}"
    );
    // A recorded opt-out is exempt.
    sandbox.write_tenant_file_allowing_external_endpoints("prod", external.as_bytes(), 0o600);
    run(&sh(), &sandbox, &["-c", "true"], &RunOptions::new()).assert_success();
}

#[test]
fn every_path_variable_and_absolute_argument_must_stay_inside_the_sandbox() {
    let sandbox = Sandbox::new();
    let root = sandbox.root().display().to_string();
    for name in PATH_VARIABLES {
        for value in ["/elsewhere", "/", "../../../../../../../../elsewhere"] {
            let problem = panics(|| {
                run(
                    &sh(),
                    &sandbox,
                    &["-c", "true"],
                    &RunOptions::new().env(name, value),
                );
            })
            .unwrap_or_else(|| panic!("{name}={value}: no panic"));
            assert!(
                problem.contains(&format!("{name} points outside the sandbox")),
                "{name}={value}: {problem}"
            );
        }
        // Inside (absolute or relative to the sandbox cwd) and empty are fine.
        for value in [
            format!("{root}/tmp/x"),
            "relative/x".to_owned(),
            String::new(),
        ] {
            run(
                &sh(),
                &sandbox,
                &["-c", "true"],
                &RunOptions::new().env(name, &value),
            )
            .assert_success();
        }
    }
    let problem = panics(|| {
        run(
            &sh(),
            &sandbox,
            &["-c", "true"],
            &RunOptions::new().env("PRISMA_AIRS_TENANTS_PATH", "~/../../../../../x"),
        );
    })
    .expect("panics");
    assert!(problem.contains("PRISMA_AIRS_TENANTS_PATH"), "{problem}");
    // Removing HOME needs the store and registry variables (the CLIs would otherwise use
    // the password-database home).
    for remove in ["AIRS_CLI_CONFIG_DIR", "PRISMA_AIRS_TENANTS_PATH"] {
        let problem = panics(|| {
            run(
                &sh(),
                &sandbox,
                &["-c", "true"],
                &RunOptions::new().without_env("HOME").without_env(remove),
            );
        })
        .unwrap_or_else(|| panic!("without HOME and {remove}: no panic"));
        assert!(problem.contains("HOME is removed"), "{problem}");
    }
    let problem = panics(|| {
        run(
            &sh(),
            &sandbox,
            &["--debug"],
            &RunOptions::new().without_env("HOME"),
        );
    })
    .expect("panics");
    assert!(problem.contains("HOME is removed"), "{problem}");
    run(
        &sh(),
        &sandbox,
        &["-c", "true"],
        &RunOptions::new().without_env("HOME"),
    )
    .assert_success();
    // Absolute-path arguments.
    for arg in ["/etc/passwd", "--from=/etc/hosts", "--from-config=/x.json"] {
        let problem = panics(|| {
            run(&sh(), &sandbox, &["-c", "true", arg], &RunOptions::new());
        })
        .unwrap_or_else(|| panic!("{arg}: no panic"));
        assert!(problem.contains("outside the sandbox"), "{arg}: {problem}");
    }
    run(
        &sh(),
        &sandbox,
        &["-c", "true", &format!("--from={root}/state/tenants.json")],
        &RunOptions::new(),
    )
    .assert_success();
}

#[test]
fn a_change_to_the_store_fails_the_run() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    for script in [
        "printf x >> \"$AIRS_CLI_CONFIG_DIR/config.toml\"",
        ": > \"$AIRS_CLI_CONFIG_DIR/config.lock\"",
        "chmod 0400 \"$AIRS_CLI_CONFIG_DIR/tenants/prod.toml\"",
        "rm \"$AIRS_CLI_CONFIG_DIR/tenants/prod.toml\"",
    ] {
        let problem = panics(|| {
            run(&sh(), &sandbox, &["-c", script], &RunOptions::new());
        })
        .unwrap_or_else(|| panic!("{script}: no panic"));
        assert!(
            problem.contains("changed sandbox files"),
            "{script}: {problem}"
        );
        sandbox.single_tenant(&server, "prod", "100");
    }
    run(
        &sh(),
        &sandbox,
        &["-c", ": > \"$AIRS_CLI_CONFIG_DIR/config.lock\""],
        &RunOptions::new().allow_writes_under("store"),
    )
    .assert_success();
}

#[test]
fn tenant_commands_skip_only_the_loopback_guards() {
    use airs_test_harness::is_tenant_command;
    for (args, tenant) in [
        (&["tenant", "list"][..], true),
        (&["--output", "json", "tenant", "create", "dev"], true),
        (&["--quiet", "--debug", "tenant", "set", "dev", "k"], true),
        (&["--output=json", "tenant", "switch", "dev"], true),
        (&["--tenant", "tenant", "doctor"], false),
        (&["--output", "tenant", "doctor"], false),
        (&["doctor", "tenant"], false),
        (&["runtime", "scan", "--profile", "p", "tenant"], false),
        (&["--", "tenant"], false),
        (&[], false),
    ] {
        assert_eq!(is_tenant_command(args), tenant, "{args:?}");
    }
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let tenant = sandbox.single_tenant(&server, "prod", "100");
    // A CLI-created Rust tenant file holds only credentials (no endpoints).
    fs::write(&tenant.store_path, b"mgmtTsgId = \"100\"\n").expect("write");
    let problem = panics(|| {
        run(&sh(), &sandbox, &["-c", "true"], &RunOptions::new());
    })
    .expect("a non-tenant run is refused");
    assert!(
        problem.contains("refusing to run airs-cli-rs: tenant file"),
        "{problem}"
    );
    // `sh tenant …` fails to open a script named `tenant`; what matters is that it ran.
    let out = run(&sh(), &sandbox, &["tenant", "list"], &RunOptions::new());
    assert_ne!(out.code, 0);
    // The path guards still apply to tenant commands.
    let problem = panics(|| {
        run(
            &sh(),
            &sandbox,
            &["tenant", "import", "--from", "/etc/hosts"],
            &RunOptions::new(),
        );
    })
    .expect("an absolute argument outside the sandbox is refused");
    assert!(problem.contains("outside the sandbox"), "{problem}");
    // TypeScript: a CLI-created config without endpoints is fine for tenant commands
    // (the spawn fails because node does not exist), but a registry naming a config
    // outside the sandbox is still refused.
    let ts = Cli::Ts {
        node: "/nonexistent/node".into(),
        entry: "/nonexistent/index.js".into(),
    };
    fs::write(
        sandbox.configs_dir().join("bare.json"),
        b"{\"mgmtTsgId\":\"1\"}",
    )
    .expect("write");
    let message = panics(|| {
        run(&ts, &sandbox, &["tenant", "list"], &RunOptions::new());
    })
    .expect("no node");
    assert!(message.contains("cannot start"), "{message}");
    let message = panics(|| {
        run(&ts, &sandbox, &["doctor"], &RunOptions::new());
    })
    .expect("guarded");
    assert!(message.contains("bare.json"), "{message}");
    sandbox.write_registry_raw(
        airs_test_harness::registry_json(
            &[airs_test_harness::RegistryEntry::new(
                "prod",
                "/etc/hosts",
                "100",
            )],
            None,
        )
        .as_bytes(),
    );
    let message = panics(|| {
        run(
            &ts,
            &sandbox,
            &["tenant", "read", "prod"],
            &RunOptions::new(),
        );
    })
    .expect("outside");
    assert!(message.contains("outside the sandbox"), "{message}");
}

#[test]
fn stdin_can_be_piped_or_held_open() {
    let sandbox = Sandbox::new();
    let out = run(
        &sh(),
        &sandbox,
        &["-c", "cat"],
        &RunOptions::new().stdin("piped\nbytes"),
    );
    out.assert_success();
    assert_eq!(out.stdout, "piped\nbytes");
    let out = run(&sh(), &sandbox, &["-c", "cat"], &RunOptions::new());
    assert_eq!(out.stdout, "", "the default stays /dev/null");
    let running = spawn(
        &sh(),
        &sandbox,
        &["-c", "cat"],
        &RunOptions::new().stdin_held_open().expect_signal(),
    );
    std::thread::sleep(Duration::from_millis(300));
    running.send_signal("TERM");
    let out = running.wait();
    assert_eq!(out.signal, Some(15), "cat was still waiting for stdin");
}

#[test]
fn pty_sessions_put_stdin_and_stderr_on_a_terminal() {
    let sandbox = Sandbox::new();
    let script = "test -t 0 && test -t 2 && ! test -t 1 || exit 9; \
                  printf 'name? ' >&2; read answer; printf 'got %s\\n' \"$answer\" >&2; echo data";
    let mut session =
        airs_test_harness::spawn_pty(&sh(), &sandbox, &["-c", script], &RunOptions::new());
    session.expect("name? ");
    session.send(b"ada\n");
    session.expect("got ada\n");
    let out = session.wait();
    out.run.assert_success();
    assert_eq!(out.run.stdout, "data\n");
    assert_eq!(out.transcript, "name? ada\r\ngot ada\r\n");
    assert!(out.echo_restored());
    let out = airs_test_harness::spawn_pty_with(
        &sh(),
        &sandbox,
        &["-c", "test -t 0 && ! test -t 2 && echo split >&2"],
        &RunOptions::new(),
        false,
    )
    .wait();
    out.run.assert_success();
    assert_eq!(out.run.stderr, "split\n");
    // The transcript gets the leak check too.
    let problem = panics(|| {
        let mut session = airs_test_harness::spawn_pty(
            &sh(),
            &sandbox,
            &["-c", "read x"],
            &RunOptions::new().secret("FAKE-PTY-SECRET"),
        );
        session.send(b"FAKE-PTY-SECRET\n");
        session.wait();
    })
    .expect("an echoed secret is caught");
    assert!(problem.contains("appeared on the terminal"), "{problem}");
}

#[test]
fn an_import_never_reads_a_typescript_file_outside_the_sandbox() {
    use airs_test_harness::{RegistryEntry, child_environment, import_source_violation};
    let sandbox = Sandbox::new();
    let env = child_environment(&sandbox, &RunOptions::new());
    let cwd = sandbox.cwd();
    let check = |args: &[&str]| import_source_violation(sandbox.root(), &cwd, &env, args);
    let inside = sandbox.configs_dir().join("dev.json");
    sandbox.write_registry(&[RegistryEntry::new("dev", &inside, "1")], None);
    assert_eq!(check(&["tenant", "import"]), None);
    assert_eq!(check(&["--quiet", "tenant", "import", "--dry-run"]), None);
    // Not an import: not this guard's business.
    assert_eq!(check(&["tenant", "list"]), None);
    // A registry entry outside the sandbox, directly or through a symlink.
    sandbox.write_registry(
        &[RegistryEntry::new("dev", "/elsewhere/dev.json", "1")],
        None,
    );
    assert!(
        check(&["tenant", "import"])
            .is_some_and(|problem| problem.contains("'dev' names a config outside the sandbox"))
    );
    let link = sandbox.configs_dir().join("link.json");
    std::os::unix::fs::symlink("/etc/hosts", &link).expect("symlink");
    sandbox.write_registry(&[RegistryEntry::new("dev", &link, "1")], None);
    assert!(check(&["tenant", "import"]).is_some());
    // The source arguments, relative ones included.
    for args in [
        &[
            "tenant",
            "import",
            "--from",
            "../../../../../../../../elsewhere.json",
        ][..],
        &[
            "tenant",
            "import",
            "--from-config",
            "../../../../../../../../x.json",
            "--name",
            "a",
        ],
        &[
            "tenant",
            "import",
            "--from-config",
            "../configs/link.json",
            "--name",
            "a",
        ],
    ] {
        assert!(check(args).is_some(), "{args:?}");
    }
    assert_eq!(
        check(&[
            "tenant",
            "import",
            "--from-config",
            "../configs/dev.json",
            "--name",
            "a"
        ]),
        None
    );
    // clap keeps the last of a repeated option (`args_override_self`): every
    // occurrence is checked, in both forms, so a second source cannot slip past.
    sandbox.write_registry(&[RegistryEntry::new("dev", &inside, "1")], None);
    let outside_registry =
        "../../../../../../../../Users/someone/.local/state/prisma-airs/tenants.json";
    for args in [
        &[
            "tenant",
            "import",
            "--from",
            "../state/tenants.json",
            "--from",
            outside_registry,
            "--dry-run",
        ][..],
        &[
            "tenant",
            "import",
            "--from=../state/tenants.json",
            &format!("--from={outside_registry}"),
        ],
        &[
            "tenant",
            "import",
            "--from-config",
            "../configs/dev.json",
            "--from-config",
            "../../../../../../../../Users/someone/c.json",
            "--name",
            "a",
        ],
    ] {
        assert!(check(args).is_some(), "{args:?}");
        assert!(
            airs_test_harness::escape_violation(sandbox.root(), &cwd, &env, args).is_some(),
            "the path guard catches relative path-flag values too: {args:?}"
        );
    }
    assert_eq!(
        check(&[
            "tenant",
            "import",
            "--from",
            "../state/tenants.json",
            "--from",
            "../state/tenants.json"
        ]),
        None
    );
    assert_eq!(
        airs_test_harness::escape_violation(
            sandbox.root(),
            &cwd,
            &env,
            &["tenant", "create", "x", "--config", "../configs/dev.json"]
        ),
        None
    );
    assert!(
        airs_test_harness::escape_violation(
            sandbox.root(),
            &cwd,
            &env,
            &[
                "tenant",
                "create",
                "x",
                "--config",
                "../../../../../../../../c.json"
            ]
        )
        .is_some()
    );
    // The runner refuses to start such a run.
    sandbox.write_registry(
        &[RegistryEntry::new("dev", "/elsewhere/dev.json", "1")],
        None,
    );
    let problem = panics(|| {
        run(&sh(), &sandbox, &["tenant", "import"], &RunOptions::new());
    })
    .expect("refused");
    assert!(problem.contains("refusing to run"), "{problem}");
}

/// A TypeScript `runtime` run gets every SDK endpoint variable pointed at the trap unless
/// the test set it (D93, DESIGN §10 M4); other TypeScript runs and Rust runs do not.
#[test]
fn typescript_runtime_runs_get_the_endpoint_trap() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    let script = sandbox.tmp().join("env.sh");
    std::fs::write(&script, "env | grep '^PANW_' | sort\n").unwrap();
    let ts = Cli::Ts {
        node: "/bin/sh".into(),
        entry: script.clone(),
    };
    let lines = |args: &[&str], options: RunOptions| -> Vec<String> {
        let out = run(&ts, &sandbox, args, &options);
        out.stdout.lines().map(str::to_owned).collect()
    };
    let trap = server.ts_env_trap_url();
    let trapped = lines(&["runtime", "topics", "list"], RunOptions::new());
    let expected: Vec<String> = {
        let mut names = airs_test_harness::TS_ENDPOINT_VARIABLES.to_vec();
        names.sort_unstable();
        names.iter().map(|name| format!("{name}={trap}")).collect()
    };
    assert_eq!(trapped, expected);
    let explicit = lines(
        &["--quiet", "runtime", "topics", "create"],
        RunOptions::new().env("PANW_MGMT_ENDPOINT", server.ts_env_mgmt_url()),
    );
    assert!(explicit.contains(&format!("PANW_MGMT_ENDPOINT={}", server.ts_env_mgmt_url())));
    assert_eq!(explicit.len(), expected.len());
    assert!(lines(&["tenant", "list"], RunOptions::new()).is_empty());
    assert!(lines(&["doctor"], RunOptions::new()).is_empty());
    let rust = run(
        &Cli::rust("/bin/sh"),
        &sandbox,
        &[script.to_str().unwrap(), "runtime"],
        &RunOptions::new(),
    );
    assert_eq!(rust.stdout, "", "airs-cli-rs runs never get the variables");
    // Removing a trapped variable would leave TypeScript on its production default: the
    // run is refused before it starts, also when the test sets the variable too (a
    // removal wins). Other runs may remove it.
    for name in ["PANW_MGMT_ENDPOINT", "PANW_MGMT_TOKEN_ENDPOINT"] {
        for options in [
            RunOptions::new().without_env(name),
            RunOptions::new()
                .env(name, server.ts_env_mgmt_url())
                .without_env(name),
        ] {
            let problem = panics(|| {
                run(
                    &ts,
                    &sandbox,
                    &["--quiet", "runtime", "topics", "create"],
                    &options,
                );
            })
            .expect("refused");
            assert!(
                problem.contains(&format!(
                    "refusing to run the TypeScript CLI [\"--quiet\", \"runtime\", \"topics\", \"create\"]: removing {name} would fall back to production"
                )),
                "{problem}"
            );
        }
    }
    let removed = lines(
        &["tenant", "list"],
        RunOptions::new().without_env("PANW_MGMT_ENDPOINT"),
    );
    assert!(removed.is_empty(), "not a runtime run: allowed");
    // Without a fixture tenant the trap is a closed loopback port.
    let empty = Sandbox::new();
    let script = empty.tmp().join("env.sh");
    std::fs::write(&script, "env | grep '^PANW_MGMT_ENDPOINT='\n").unwrap();
    let ts = Cli::Ts {
        node: "/bin/sh".into(),
        entry: script,
    };
    let out = run(&ts, &empty, &["runtime", "scan"], &RunOptions::new());
    let value = out
        .stdout
        .trim()
        .strip_prefix("PANW_MGMT_ENDPOINT=")
        .unwrap()
        .to_owned();
    assert!(airs_test_harness::is_loopback_url(&value), "{value}");
    assert!(value.ends_with("/ts-env-trap"));
    assert!(airs_test_harness::is_runtime_command(&[
        "--output", "json", "runtime", "scan"
    ]));
    assert!(!airs_test_harness::is_runtime_command(&[
        "--tenant", "runtime", "doctor"
    ]));
}

/// DESIGN §10 escape prevention for the M5 commands: with `HOME` removed a bulk-scan or
/// resume-poll run needs `XDG_STATE_HOME` inside the sandbox (its job state would
/// otherwise go under the password-database home), a TypeScript one is refused, and
/// the `resume-poll` state argument, relative ones included, must lie inside it.
#[test]
fn bulk_runs_never_reach_the_real_state_home() {
    let sandbox = Sandbox::new();
    let root = sandbox.root().to_path_buf();
    let cwd = sandbox.cwd();
    let inside = |rel: &str| root.join(rel).display().to_string();
    let mut env = std::collections::BTreeMap::new();
    env.insert("AIRS_CLI_CONFIG_DIR".to_owned(), inside("store"));
    env.insert(
        "PRISMA_AIRS_TENANTS_PATH".to_owned(),
        inside("state/tenants.json"),
    );
    env.insert("TMPDIR".to_owned(), inside("tmp"));
    // Automatic metadata retention needs its own safe data root. This test
    // isolates the independent bulk-state fallback guard, not data-root admission.
    env.insert("XDG_DATA_HOME".to_owned(), inside("tmp/data"));
    let input = inside("tmp/p.txt");
    let output = inside("tmp/o.csv");
    let state = inside("tmp/x.bulk-scan.json");
    let bulk = [
        "runtime",
        "bulk-scan",
        "--profile",
        "p",
        "--file",
        input.as_str(),
        "--output-file",
        output.as_str(),
    ];
    let resume = ["runtime", "resume-poll", state.as_str()];
    let resume_after_flags = ["--output", "json", "runtime", "resume-poll", state.as_str()];
    for args in [&bulk[..], &resume[..], &resume_after_flags[..]] {
        let problem = airs_test_harness::escape_violation(&root, &cwd, &env, args)
            .unwrap_or_else(|| panic!("{args:?}: no violation without XDG_STATE_HOME"));
        assert!(problem.contains("HOME is removed"), "{problem}");
        assert!(
            airs_test_harness::ts_bulk_home_violation(&env, args).is_some(),
            "{args:?}"
        );
    }
    // A non-bulk scan does not need the bulk-state path control.
    assert_eq!(
        airs_test_harness::escape_violation(&root, &cwd, &env, &["runtime", "scan", "x"]),
        None
    );
    assert_eq!(
        airs_test_harness::ts_bulk_home_violation(&env, &["runtime", "scan", "x"]),
        None
    );
    env.insert("XDG_STATE_HOME".to_owned(), inside("tmp/xdg"));
    for args in [&bulk[..], &resume[..]] {
        assert_eq!(
            airs_test_harness::escape_violation(&root, &cwd, &env, args),
            None,
            "{args:?}"
        );
    }
    // The state argument: relative ones resolved against the cwd.
    env.insert("HOME".to_owned(), inside("home"));
    assert_eq!(
        airs_test_harness::ts_bulk_home_violation(&env, &resume),
        None
    );
    for state in [
        "../../../../../../../../Users/someone/.local/state/prisma-airs/tenants.json",
        "/etc/passwd",
    ] {
        let args = [
            "runtime",
            "resume-poll",
            state,
            "--output-file",
            output.as_str(),
        ];
        let problem = airs_test_harness::escape_violation(&root, &cwd, &env, &args)
            .unwrap_or_else(|| panic!("{state}: no violation"));
        assert!(problem.contains("outside the sandbox"), "{problem}");
    }
    assert_eq!(
        airs_test_harness::escape_violation(
            &root,
            &cwd,
            &env,
            &["runtime", "resume-poll", "../tmp/x.bulk-scan.json"]
        ),
        None
    );
    assert_eq!(
        airs_test_harness::resume_state_argument(&[
            "--tenant",
            "prod",
            "runtime",
            "resume-poll",
            "--output-file",
            "o.csv",
            "s.json"
        ]),
        Some("s.json")
    );
    assert_eq!(
        airs_test_harness::bulk_command(&["--output", "runtime", "bulk-scan"]),
        None,
        "`runtime` here is --output's value"
    );
    // The runner refuses such a run before it starts.
    let problem = panics(|| {
        run(
            &sh(),
            &sandbox,
            &resume,
            &RunOptions::new().without_env("HOME"),
        );
    })
    .expect("panics");
    assert!(problem.contains("HOME is removed"), "{problem}");
}

/// DESIGN §10 M6a: the red team fixture markers are secrets for airs-cli-rs red team runs
/// (a run that prints one fails), not for the TypeScript CLI (which prints them, D140) or
/// other commands; the red team path flags are guarded like the others.
#[test]
fn red_team_runs_check_the_fixture_markers_and_path_flags() {
    use std::os::unix::fs::PermissionsExt;
    assert!(airs_test_harness::is_redteam_command(&[
        "--output", "json", "redteam", "targets", "get"
    ]));
    assert!(!airs_test_harness::is_redteam_command(&[
        "--tenant", "redteam", "doctor"
    ]));
    let sandbox = Sandbox::new();
    let script = sandbox.tmp().join("leak.sh");
    let marker = airs_test_harness::fixtures::redteam::secret(1);
    std::fs::write(&script, format!("#!/bin/sh\necho 'token {marker}'\n")).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let rust = Cli::rust(&script);
    let leaked = std::panic::catch_unwind(|| {
        run(
            &rust,
            &sandbox,
            &["redteam", "targets", "get"],
            &RunOptions::new(),
        )
    });
    assert!(
        leaked.is_err(),
        "a marker on a red team run's stdout fails the run"
    );
    let other = run(&rust, &sandbox, &["doctor"], &RunOptions::new());
    assert!(
        other.stdout.contains(&marker),
        "markers are not secrets for other commands"
    );
    let ts = Cli::Ts {
        node: "/bin/sh".into(),
        entry: script.clone(),
    };
    let printed = run(
        &ts,
        &sandbox,
        &["redteam", "targets", "get"],
        &RunOptions::new(),
    );
    assert!(
        printed.stdout.contains(&marker),
        "the TypeScript side may print them"
    );
    let root = sandbox.root().to_path_buf();
    let env: std::collections::BTreeMap<String, String> = [
        ("HOME", root.join("home")),
        ("AIRS_CLI_CONFIG_DIR", root.join("store")),
    ]
    .into_iter()
    .map(|(name, path)| (name.to_owned(), path.display().to_string()))
    .collect();
    for flag in [
        "--output-dir",
        "--input-dir",
        "--goals",
        "--script-file",
        "--variables-file",
    ] {
        assert!(
            airs_test_harness::escape_violation(
                &root,
                &sandbox.cwd(),
                &env,
                &["redteam", "x", flag, "../../../../../../etc/passwd"]
            )
            .is_some(),
            "{flag}"
        );
        assert!(
            airs_test_harness::escape_violation(
                &root,
                &sandbox.cwd(),
                &env,
                &["redteam", "x", flag, "inside"]
            )
            .is_none(),
            "{flag}"
        );
    }
}

/// The `redteam prompt-sets upload <uuid> <file>` positional is guarded like a path
/// flag (DESIGN §10 M6a step 2).
#[test]
fn red_team_upload_files_stay_in_the_sandbox() {
    let sandbox = Sandbox::new();
    let root = sandbox.root().to_path_buf();
    let env: std::collections::BTreeMap<String, String> = [
        ("HOME", root.join("home")),
        ("AIRS_CLI_CONFIG_DIR", root.join("store")),
    ]
    .into_iter()
    .map(|(name, path)| (name.to_owned(), path.display().to_string()))
    .collect();
    let args = |file: &'static str| ["redteam", "prompt-sets", "upload", "id", file];
    assert_eq!(
        airs_test_harness::upload_file_argument(&args("p.csv")),
        Some("p.csv")
    );
    assert_eq!(
        airs_test_harness::upload_file_argument(&["redteam", "prompt-sets", "get", "id"]),
        None
    );
    assert!(
        airs_test_harness::escape_violation(
            &root,
            &sandbox.cwd(),
            &env,
            &args("../../../../../../etc/passwd")
        )
        .is_some()
    );
    assert!(
        airs_test_harness::escape_violation(&root, &sandbox.cwd(), &env, &args("p.csv")).is_none()
    );
}

/// DESIGN §10 M6b: the DLP content markers fail an airs-cli-rs `runtime dlp` run that
/// prints one on stderr or in a debug log; stdout may hold them (the tests assert where),
/// and other commands and the TypeScript side are not checked.
#[test]
fn dlp_runs_check_the_content_markers_on_stderr_and_the_debug_log() {
    use std::os::unix::fs::PermissionsExt;
    assert!(airs_test_harness::is_dlp_command(&[
        "--output", "json", "runtime", "dlp", "patterns", "list"
    ]));
    assert!(airs_test_harness::is_dlp_command(&[
        "--tenant", "dev", "--debug", "runtime", "dlp", "patterns"
    ]));
    assert!(!airs_test_harness::is_dlp_command(&[
        "--tenant", "runtime", "dlp", "x"
    ]));
    assert!(!airs_test_harness::is_dlp_command(&[
        "runtime", "scan", "dlp"
    ]));
    assert!(!airs_test_harness::is_dlp_command(&["redteam", "dlp"]));
    let sandbox = Sandbox::new();
    let marker = airs_test_harness::fixtures::dlp::content(3);
    let script = |name: &str, body: &str| {
        let path = sandbox.tmp().join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    };
    let to_stdout = Cli::rust(script("out.sh", &format!("echo 'regex {marker}'")));
    let printed = run(
        &to_stdout,
        &sandbox,
        &["runtime", "dlp", "patterns", "get", "p"],
        &RunOptions::new(),
    );
    assert!(printed.stdout.contains(&marker), "stdout may hold a marker");
    let to_stderr = Cli::rust(script("err.sh", &format!("echo 'regex {marker}' >&2")));
    let leaked = std::panic::catch_unwind(|| {
        run(
            &to_stderr,
            &sandbox,
            &["runtime", "dlp", "patterns", "get", "p"],
            &RunOptions::new(),
        )
    });
    assert!(
        leaked.is_err(),
        "a marker on a DLP run's stderr fails the run"
    );
    let other = run(&to_stderr, &sandbox, &["doctor"], &RunOptions::new());
    assert!(
        other.stderr.contains(&marker),
        "other commands are not checked"
    );
    let ts = Cli::Ts {
        node: "/bin/sh".into(),
        entry: sandbox.tmp().join("err.sh"),
    };
    let ts_run = run(
        &ts,
        &sandbox,
        &["runtime", "dlp", "patterns", "get", "p"],
        &RunOptions::new(),
    );
    assert!(
        ts_run.stderr.contains(&marker),
        "the TypeScript side is not checked"
    );
    let log_dir = airs_test_harness::debug_log::default_log_dir(&sandbox);
    let to_log = Cli::rust(script(
        "log.sh",
        &format!(
            "mkdir -p '{dir}' && echo '{{\"x\":\"{marker}\"}}' > '{dir}/debug-api-1700000000000-0a1b2c3d.jsonl'",
            dir = log_dir.display()
        ),
    ));
    let logged = std::panic::catch_unwind(|| {
        run(
            &to_log,
            &sandbox,
            &["runtime", "dlp", "patterns", "list"],
            &RunOptions::new().allow_writes_under("home"),
        )
    });
    assert!(
        logged.is_err(),
        "a marker in a DLP run's debug log fails the run"
    );
    // The control: the same log without a marker passes.
    let clean = Cli::rust(script(
        "clean.sh",
        &format!(
            "mkdir -p '{dir}' && echo '{{\"x\":\"ok\"}}' > '{dir}/debug-api-1700000000001-0a1b2c3d.jsonl'",
            dir = log_dir.display()
        ),
    ));
    std::fs::remove_dir_all(&log_dir).unwrap();
    run(
        &clean,
        &sandbox,
        &["runtime", "dlp", "patterns", "list"],
        &RunOptions::new().allow_writes_under("home"),
    )
    .assert_success();
}

/// The `runtime dlp restore <file>` positional is guarded like a path flag, the values of
/// the restore and backup options skipped (DESIGN §10 M6b).
#[test]
fn dlp_restore_files_stay_in_the_sandbox() {
    let sandbox = Sandbox::new();
    let root = sandbox.root().to_path_buf();
    let env: std::collections::BTreeMap<String, String> = [
        ("HOME", root.join("home")),
        ("AIRS_CLI_CONFIG_DIR", root.join("store")),
    ]
    .into_iter()
    .map(|(name, path)| (name.to_owned(), path.display().to_string()))
    .collect();
    assert_eq!(
        airs_test_harness::restore_file_argument(&[
            "runtime",
            "dlp",
            "restore",
            "--name-prefix",
            "x-",
            "b.json"
        ]),
        Some("b.json")
    );
    assert_eq!(
        airs_test_harness::restore_file_argument(&["runtime", "dlp", "backup"]),
        None
    );
    for args in [
        &[
            "runtime",
            "dlp",
            "restore",
            "../../../../../../etc/passwd",
            "--dry-run",
        ][..],
        &[
            "runtime",
            "dlp",
            "restore",
            "--expect-tsg",
            "1",
            "../../../../../../etc/passwd",
        ],
        &[
            "runtime",
            "dlp",
            "backup",
            "--output-file",
            "../../../../../../tmp/x.json",
        ],
    ] {
        assert!(
            airs_test_harness::escape_violation(&root, &sandbox.cwd(), &env, args).is_some(),
            "{args:?}"
        );
    }
    assert!(
        airs_test_harness::escape_violation(
            &root,
            &sandbox.cwd(),
            &env,
            &[
                "runtime",
                "dlp",
                "restore",
                "inside.json",
                "--max-pages",
                "5"
            ]
        )
        .is_none()
    );
}

/// DESIGN §10 M6c: the runner fails an airs-cli-rs model-security run that prints a
/// `FAKE-MS-SECRET-<nn>` marker on stderr (or in a debug log); stdout may hold one where
/// §17.15 prints it (the tests assert where); other commands and the TypeScript side are
/// not checked.
#[test]
fn model_security_runs_check_the_markers_on_stderr_and_the_debug_log() {
    use std::os::unix::fs::PermissionsExt;
    assert!(airs_test_harness::is_model_security_command(&[
        "--output",
        "json",
        "model-security",
        "groups",
        "list"
    ]));
    assert!(airs_test_harness::is_model_security_command(&[
        "--tenant",
        "dev",
        "--debug",
        "model-security",
        "scans"
    ]));
    assert!(!airs_test_harness::is_model_security_command(&[
        "--tenant",
        "model-security",
        "groups"
    ]));
    assert!(!airs_test_harness::is_model_security_command(&[
        "runtime",
        "model-security"
    ]));
    let sandbox = Sandbox::new();
    let marker = airs_test_harness::fixtures::modelsec::secret(7);
    let script = |name: &str, body: &str| {
        let path = sandbox.tmp().join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    };
    let args = ["model-security", "labels", "values", "env"];
    let to_stdout = Cli::rust(script("ms-out.sh", &format!("echo 'value {marker}'")));
    let printed = run(&to_stdout, &sandbox, &args, &RunOptions::new());
    assert!(printed.stdout.contains(&marker), "stdout may hold a marker");
    let to_stderr = Cli::rust(script("ms-err.sh", &format!("echo 'value {marker}' >&2")));
    let leaked = std::panic::catch_unwind(|| run(&to_stderr, &sandbox, &args, &RunOptions::new()));
    assert!(
        leaked.is_err(),
        "a marker on a model-security run's stderr fails the run"
    );
    let other = run(&to_stderr, &sandbox, &["doctor"], &RunOptions::new());
    assert!(
        other.stderr.contains(&marker),
        "other commands are not checked"
    );
    let ts = Cli::Ts {
        node: "/bin/sh".into(),
        entry: sandbox.tmp().join("ms-err.sh"),
    };
    let ts_run = run(&ts, &sandbox, &args, &RunOptions::new());
    assert!(
        ts_run.stderr.contains(&marker),
        "the TypeScript side is not checked"
    );
}

/// DESIGN §10 M6d: an airs-cli-rs runtime-extras run fails when an API key marker appears
/// on any stream, scan content outside `scan-content --show-content`'s stdout, or an auth
/// code on stderr; other commands and the TypeScript side are not checked.
#[test]
fn runtime_extras_runs_check_the_markers() {
    use airs_test_harness::fixtures::runtime as fx;
    use std::os::unix::fs::PermissionsExt;
    assert!(airs_test_harness::is_runtime_extras_command(&[
        "--output", "json", "runtime", "api-keys", "list"
    ]));
    assert!(airs_test_harness::is_runtime_extras_command(&[
        "--tenant", "dev", "runtime", "sessions", "chart"
    ]));
    assert!(!airs_test_harness::is_runtime_extras_command(&[
        "runtime", "profiles", "list"
    ]));
    assert!(!airs_test_harness::is_runtime_extras_command(&[
        "--tenant", "runtime", "api-keys"
    ]));
    let sandbox = Sandbox::new();
    let script = |name: &str, body: &str| {
        let path = sandbox.tmp().join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    };
    let key = fx::key_secret("100", "list");
    let list = ["runtime", "api-keys", "list"];
    let to_stdout = Cli::rust(script("rx-key.sh", &format!("echo 'key {key}'")));
    let leaked = std::panic::catch_unwind(|| run(&to_stdout, &sandbox, &list, &RunOptions::new()));
    assert!(leaked.is_err(), "an API key on stdout fails the run");
    let code = fx::auth_code(1);
    let code_out = Cli::rust(script("rx-code.sh", &format!("echo 'code {code}'")));
    let printed = run(&code_out, &sandbox, &list, &RunOptions::new());
    assert!(
        printed.stdout.contains(&code),
        "auth codes may reach stdout"
    );
    let code_err = Cli::rust(script("rx-code-err.sh", &format!("echo 'code {code}' >&2")));
    let leaked = std::panic::catch_unwind(|| run(&code_err, &sandbox, &list, &RunOptions::new()));
    assert!(leaked.is_err(), "an auth code on stderr fails the run");
    let content = fx::prompt_content("100");
    let shows = Cli::rust(script("rx-content.sh", &format!("echo '{content}'")));
    let show = [
        "runtime",
        "sessions",
        "scan-content",
        "--scan-id",
        "s",
        "--scan-sub-req-id",
        "0",
        "--show-content",
    ];
    let printed = run(&shows, &sandbox, &show, &RunOptions::new());
    assert!(
        printed.stdout.contains(&content),
        "--show-content may print content"
    );
    let chart = ["runtime", "sessions", "chart"];
    let leaked = std::panic::catch_unwind(|| run(&shows, &sandbox, &chart, &RunOptions::new()));
    assert!(leaked.is_err(), "scan content elsewhere fails the run");
    let other = run(&to_stdout, &sandbox, &["doctor"], &RunOptions::new());
    assert!(
        other.stdout.contains(&key),
        "other commands are not checked"
    );
    let ts = Cli::Ts {
        node: "/bin/sh".into(),
        entry: sandbox.tmp().join("rx-key.sh"),
    };
    let ts_run = run(&ts, &sandbox, &list, &RunOptions::new());
    assert!(
        ts_run.stdout.contains(&key),
        "the TypeScript side is not checked"
    );
}

#[test]
fn history_path_controls_cannot_escape_or_fall_back_to_the_real_home() {
    let sandbox = Sandbox::new();
    let root = sandbox.root();
    let cwd = sandbox.cwd();
    let mut env = std::collections::BTreeMap::from([
        (
            "AIRS_CLI_CONFIG_DIR".into(),
            sandbox.store_dir().display().to_string(),
        ),
        (
            "PRISMA_AIRS_TENANTS_PATH".into(),
            sandbox
                .state_dir()
                .join("tenants.json")
                .display()
                .to_string(),
        ),
    ]);
    let args = ["runtime", "history", "status"];
    let error = airs_test_harness::escape_violation(root, &cwd, &env, &args).unwrap();
    assert!(error.contains("XDG_DATA_HOME"));
    env.insert(
        "XDG_DATA_HOME".into(),
        sandbox.tmp().join("data").display().to_string(),
    );
    assert!(airs_test_harness::escape_violation(root, &cwd, &env, &args).is_none());
    let inside = sandbox.tmp().join("history");
    let inside = inside.to_str().unwrap();
    env.remove("XDG_DATA_HOME");
    assert!(
        airs_test_harness::escape_violation(
            root,
            &cwd,
            &env,
            &["runtime", "history", "status", "--history-dir", inside]
        )
        .is_none()
    );
    assert!(
        airs_test_harness::escape_violation(
            root,
            &cwd,
            &env,
            &[
                "runtime",
                "history",
                "status",
                "--history-dir",
                "/outside-sandbox"
            ]
        )
        .is_some()
    );
    env.insert(
        "AIRS_CLI_TEST_HISTORY_BATCH_MARKER".into(),
        "/outside-sandbox/marker".into(),
    );
    assert!(
        airs_test_harness::escape_violation(
            root,
            &cwd,
            &env,
            &["runtime", "history", "status", "--history-dir", inside]
        )
        .is_some()
    );
}

/// The history driver's `runtime sync` writes an archive under the default data
/// directory: without HOME it needs XDG_DATA_HOME or an explicit sandbox --history-dir,
/// and an outside --history-dir is refused. Since M1 the CLI's `scan`, `bulk-scan` and
/// `resume-poll` keep no history, so they need neither.
#[test]
fn automatic_history_cannot_use_real_home_or_escape_through_explicit_controls() {
    let s = Sandbox::new();
    let mut env = std::collections::BTreeMap::from([
        (
            "AIRS_CLI_CONFIG_DIR".into(),
            s.store_dir().display().to_string(),
        ),
        (
            "PRISMA_AIRS_TENANTS_PATH".into(),
            s.registry_path().display().to_string(),
        ),
        (
            "XDG_STATE_HOME".into(),
            s.tmp().join("state").display().to_string(),
        ),
    ]);
    let violation = |env: &std::collections::BTreeMap<String, String>, args: &[&str]| {
        airs_test_harness::escape_violation(s.root(), &s.cwd(), env, args)
    };
    assert!(
        violation(&env, &["runtime", "sync"])
            .unwrap()
            .contains("XDG_DATA_HOME")
    );
    let inside = s.tmp().join("history");
    assert!(
        violation(
            &env,
            &["runtime", "sync", "--history-dir", inside.to_str().unwrap()]
        )
        .is_none()
    );
    assert!(
        violation(
            &env,
            &["runtime", "sync", "--history-dir", "/outside-sandbox"]
        )
        .is_some()
    );
    for command in ["scan", "bulk-scan", "resume-poll"] {
        assert!(
            violation(&env, &["runtime", command]).is_none(),
            "{command}"
        );
    }
    env.insert(
        "XDG_DATA_HOME".into(),
        s.tmp().join("data").display().to_string(),
    );
    assert!(violation(&env, &["runtime", "sync"]).is_none());
}
