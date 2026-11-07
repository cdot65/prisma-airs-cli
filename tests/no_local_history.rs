//! M1 (RUNTIME_MCP_IMPLEMENTATION_PLAN): the `airs` CLI owns no history database. The
//! unpublished 8.2-development `runtime history`/`runtime sync` commands and the
//! `--history-dir`/`--no-history` flags are gone; `scan`, `bulk-scan` and `resume-poll`
//! retain nothing. Persistent history is the optional `airs-mcp` service's
//! (airs-history-store); explicit `--mcp`/`--record-history` selection arrives in M7.
use airs_test_harness::{Cli, FixtureServer, Route, RunOptions, Sandbox, run};

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

/// The removed grammar is a usage error before any tenant, archive or network access.
#[test]
fn removed_history_commands_and_flags_are_usage_errors_before_any_access() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let before = s.snapshot_with(&[&s.tmp(), &s.data_dir()]);
    let root = s.tmp().join("history");
    let root = root.to_str().unwrap();
    for args in [
        vec!["runtime", "sync"],
        vec!["runtime", "sync", "--output", "json"],
        vec!["runtime", "history", "status"],
        vec!["runtime", "history", "coverage"],
        vec!["runtime", "scan", "--profile", "p", "--no-history", "hi"],
        vec![
            "runtime",
            "scan",
            "--profile",
            "p",
            "--history-dir",
            root,
            "hi",
        ],
        vec![
            "runtime",
            "bulk-scan",
            "--profile",
            "p",
            "--input",
            "x",
            "--no-history",
        ],
        vec![
            "runtime",
            "resume-poll",
            "state.json",
            "--history-dir",
            root,
        ],
    ] {
        let out = run(&rust(), &s, &args, &RunOptions::new());
        out.assert_code(2);
        assert!(!out.stderr.contains("Runtime history"), "{args:?}");
    }
    s.assert_unchanged(&before);
    assert_eq!(server.request_count(), 0);
}

/// A successful scan sends exactly its scan and leaves the data directory (the former
/// history root) and HOME untouched.
#[test]
fn a_successful_scan_retains_nothing_locally() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let before = s.snapshot_with(&[&s.data_dir()]);
    run(
        &rust(),
        &s,
        &["runtime", "scan", "--profile", "Monitor", "hello"],
        &RunOptions::new(),
    )
    .assert_success();
    s.assert_unchanged(&before);
    assert!(!s.data_dir().join("airs-cli").exists());
    assert!(s.home_listing().is_empty(), "CLI wrote into HOME");
    assert_eq!(server.requests_for(Route::ScanSync).len(), 1);
    assert_eq!(server.request_count(), 1);
}

/// Plan M1 exit gate: the CLI package depends on neither SQLite nor the server-owned
/// history store (the resolved graph is checked in the phase receipts with `cargo tree`).
#[test]
fn the_cli_manifest_names_no_sqlite_or_history_store() {
    let manifest =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    let dependencies = manifest
        .split("\n[dependencies]\n")
        .nth(1)
        .and_then(|rest| rest.split("\n[").next())
        .unwrap();
    // Exact crate names: the CLI may depend on the client (`airs-mcp-client`, M7),
    // never on the server crate `airs-mcp`.
    let names: Vec<&str> = dependencies
        .lines()
        .filter_map(|line| line.split_once('=').map(|(name, _)| name.trim()))
        .collect();
    for forbidden in ["rusqlite", "libsqlite3", "airs-history-store", "airs-mcp"] {
        assert!(
            !names.contains(&forbidden),
            "the CLI's [dependencies] name {forbidden}"
        );
    }
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();
    assert!(!source.contains("mod history"));
}
