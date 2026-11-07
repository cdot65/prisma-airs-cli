//! D350–D361 (M7): the optional runtime MCP client is Rust-only. Each test runs one
//! representative invocation on both CLIs: the Rust CLI behaves as DESIGN states (its
//! exit code and message, before any network access), and the TypeScript reference
//! refuses the command or option as unknown. Neither sends a request to the fixture
//! service. The behaviour itself through a gateway is covered by `tests/mcp_client.rs`.

use airs_test_harness::{Cli, FixtureServer, RunOptions, Sandbox, require_ts, run};

/// Runs `args` on both CLIs with one configured tenant and no MCP connection.
fn both(test: &str, args: &[&str], rust: (i32, &str), reference: &str) {
    let Some(ts) = require_ts(test) else {
        return;
    };
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    let state = sandbox.tmp().join("xdg-state");
    std::fs::create_dir_all(&state).unwrap();
    let o = RunOptions::new()
        .env("XDG_STATE_HOME", state.to_str().unwrap())
        .allow_writes_under("store");
    let before = server.request_count();
    let r = run(&Cli::rust(env!("CARGO_BIN_EXE_airs")), &sandbox, args, &o);
    assert_eq!(r.code, rust.0, "{args:?}: {}", r.stderr);
    assert!(
        r.stderr.contains(rust.1) || r.stdout.contains(rust.1),
        "{args:?}: {}",
        r.stderr
    );
    let t = run(&ts, &sandbox, args, &o);
    t.assert_code(1);
    assert!(t.stdout.is_empty());
    assert!(t.stderr.contains(reference), "{args:?}: {}", t.stderr);
    assert_eq!(server.request_count(), before, "no request to the service");
}

const NO_CONNECTION: &str = "No such MCP connection in the selected tenant";

#[test]
fn d350_mcp_connections_are_rust_only() {
    both(
        "d350_mcp_connections_are_rust_only",
        &["mcp", "connections", "list", "--output", "json"],
        (0, "{}"),
        "unknown command 'mcp'",
    );
}

#[test]
fn d351_mcp_sign_in_is_rust_only() {
    both(
        "d351_mcp_sign_in_is_rust_only",
        &["mcp", "login", "gw"],
        (1, NO_CONNECTION),
        "unknown command 'mcp'",
    );
}

#[test]
fn d352_service_reads_are_rust_only() {
    both(
        "d352_service_reads_are_rust_only",
        &["runtime", "jobs", "list", "--mcp", "gw"],
        (1, NO_CONNECTION),
        "unknown command 'jobs'",
    );
}

#[test]
fn d353_service_commands_need_an_explicit_connection() {
    both(
        "d353_service_commands_need_an_explicit_connection",
        &["runtime", "history", "status"],
        (2, "select a connection with --mcp CONNECTION"),
        "unknown command 'history'",
    );
}

#[test]
fn d354_operation_receipts_are_rust_only() {
    both(
        "d354_operation_receipts_are_rust_only",
        &["mcp", "status", "--output", "json"],
        (0, "{}"),
        "unknown command 'mcp'",
    );
}

#[test]
fn d355_service_reports_are_rust_only() {
    both(
        "d355_service_reports_are_rust_only",
        &[
            "runtime",
            "report",
            "--mcp",
            "gw",
            "--from-evidence",
            "evidence.json",
        ],
        (2, "cannot be used with --mcp"),
        "unknown option '--mcp'",
    );
}

#[test]
fn d356_report_downloads_are_rust_only() {
    both(
        "d356_report_downloads_are_rust_only",
        &[
            "runtime",
            "history",
            "download",
            "--mcp",
            "gw",
            "--version-id",
            "x",
            "--output-dir",
            "out",
        ],
        (2, "--version-id must be a report version ID"),
        "unknown command 'history'",
    );
}

#[test]
fn d357_lifecycle_confirmation_is_rust_only() {
    both(
        "d357_lifecycle_confirmation_is_rust_only",
        &["mcp", "confirm", "--connection", "gw", "x"],
        (2, "PREVIEW_ID must be a preview ID"),
        "unknown command 'mcp'",
    );
}

#[test]
fn d358_archive_transfer_is_rust_only() {
    both(
        "d358_archive_transfer_is_rust_only",
        &[
            "runtime",
            "history",
            "export",
            "--mcp",
            "gw",
            "--file",
            "archive.jsonl",
        ],
        (1, NO_CONNECTION),
        "unknown command 'history'",
    );
}

#[test]
fn d359_receipt_upload_is_rust_only() {
    both(
        "d359_receipt_upload_is_rust_only",
        &[
            "runtime",
            "receipts",
            "upload",
            "--mcp",
            "gw",
            "--from-bulk-state",
            "state.json",
        ],
        (1, NO_CONNECTION),
        "unknown command 'receipts'",
    );
}

#[test]
fn d360_record_history_is_rust_only() {
    both(
        "d360_record_history_is_rust_only",
        &[
            "runtime",
            "scan",
            "--profile",
            "p",
            "hello",
            "--record-history",
            "gw",
        ],
        (2, "--record-history needs a named scanner"),
        "unknown option '--record-history'",
    );
}

#[test]
fn d361_keyed_submission_recovery_is_rust_only() {
    both(
        "d361_keyed_submission_recovery_is_rust_only",
        &["runtime", "sync", "--mcp", "gw", "--no-wait"],
        (1, NO_CONNECTION),
        "unknown command 'sync'",
    );
}
