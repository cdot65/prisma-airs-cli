use airs_test_harness::{Cli, FixtureServer, RunOptions, Sandbox, run};
use serde_json::Value;
#[path = "support/history.rs"]
mod history;
fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs-history-driver"))
}

#[test]
fn missing_status_is_unknown_without_files_or_http() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let dir = s.tmp().join("not-created");
    let before = s.snapshot_with(&[&s.tmp()]);
    let out = run(
        &rust(),
        &s,
        &[
            "runtime",
            "history",
            "status",
            "--history-dir",
            dir.to_str().unwrap(),
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    let value: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(value["archive_present"], false);
    assert!(value["events"].is_null());
    assert!(!dir.exists());
    s.assert_unchanged(&before);
    assert_eq!(server.request_count(), 0);
    for leaf in ["coverage", "stats"] {
        let out = run(
            &rust(),
            &s,
            &[
                "runtime",
                "history",
                leaf,
                "--history-dir",
                dir.to_str().unwrap(),
            ],
            &RunOptions::new(),
        );
        out.assert_code(1);
        assert!(!dir.exists());
        s.assert_unchanged(&before);
    }
}

#[test]
fn existing_status_coverage_and_full_stats_do_not_modify_archive() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let (root, _) = history::archive(&s);
    let before = s.snapshot_with(&[&root]);
    let status = run(
        &rust(),
        &s,
        &[
            "runtime",
            "history",
            "status",
            "--history-dir",
            root.to_str().unwrap(),
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    status.assert_success();
    let value: Value = serde_json::from_str(&status.stdout).unwrap();
    assert_eq!(value["events"], 1);
    assert_eq!(value["partial_receipts"], 1);
    assert!(value["distinct_scan_ids"].is_null());
    let stats = run(
        &rust(),
        &s,
        &[
            "runtime",
            "history",
            "stats",
            "--history-dir",
            root.to_str().unwrap(),
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    stats.assert_success();
    let value: Value = serde_json::from_str(&stats.stdout).unwrap();
    assert_eq!(value["distinct_scan_ids"], 1);
    assert_eq!(value["coverage_groups"][0]["state"], "partial");
    for format in ["json", "pretty", "table"] {
        let out = run(
            &rust(),
            &s,
            &[
                "runtime",
                "history",
                "coverage",
                "--history-dir",
                root.to_str().unwrap(),
                "--since",
                "1970-01-01T00:00:00Z",
                "--until",
                "1970-01-01T00:00:01Z",
                "--app-id",
                "app",
                "--output",
                format,
            ],
            &RunOptions::new(),
        );
        out.assert_success();
        assert!(out.stdout.contains("partial"));
        s.assert_unchanged(&before);
    }
    assert_eq!(server.request_count(), 0);
}

#[test]
fn usage_errors_precede_tenant_access_and_repeated_scalars_are_rejected() {
    let s = Sandbox::new();
    for args in [
        vec!["runtime", "history", "status", "--history-dir", "relative"],
        vec![
            "runtime",
            "history",
            "coverage",
            "--since",
            "2026-01-01T00:00:00Z",
        ],
        vec![
            "runtime",
            "history",
            "coverage",
            "--since",
            "2026-01-01T00:00:00+00:00",
            "--until",
            "2026-01-02T00:00:00Z",
        ],
        vec!["runtime", "history", "stats", "--timeout", "0"],
        vec![
            "runtime",
            "history",
            "stats",
            "--timeout",
            "10",
            "--timeout",
            "20",
        ],
    ] {
        let out = run(&rust(), &s, &args, &RunOptions::new());
        out.assert_code(2);
        assert!(!out.stderr.contains("No tenant"));
    }
}

#[test]
fn changed_cloud_and_unsafe_archive_never_become_zero_activity() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let (root, dir) = history::archive(&s);
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        dir.join("archive.sqlite3"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    let before = s.snapshot_with(&[&root]);
    let out = run(
        &rust(),
        &s,
        &[
            "runtime",
            "history",
            "status",
            "--history-dir",
            root.to_str().unwrap(),
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    assert!(out.stdout.is_empty());
    s.assert_unchanged(&before);
    assert_eq!(server.request_count(), 0);
}

#[test]
fn explicit_compaction_and_recovery_keep_existing_data_and_missing_archives_absent() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let missing = s.tmp().join("missing-maintenance");
    for leaf in ["compact", "recover"] {
        let no_force = run(
            &rust(),
            &s,
            &[
                "runtime",
                "history",
                leaf,
                "--history-dir",
                missing.to_str().unwrap(),
            ],
            &RunOptions::new(),
        );
        no_force.assert_code(2);
        assert!(!missing.exists());
        let absent = run(
            &rust(),
            &s,
            &[
                "runtime",
                "history",
                leaf,
                "--force",
                "--history-dir",
                missing.to_str().unwrap(),
            ],
            &RunOptions::new(),
        );
        absent.assert_code(1);
        assert!(!missing.exists());
    }
    let (root, dir) = history::archive(&s);
    let out = run(
        &rust(),
        &s,
        &[
            "runtime",
            "history",
            "compact",
            "--force",
            "--history-dir",
            root.to_str().unwrap(),
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    let value: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(value["durable"], true);
    let before = s.snapshot_with(&[&root]);
    let out = run(
        &rust(),
        &s,
        &[
            "runtime",
            "history",
            "recover",
            "--force",
            "--history-dir",
            root.to_str().unwrap(),
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    let value: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(value["recovery_attempted"], false);
    assert_eq!(value["integrity_verified"], true);
    s.assert_unchanged(&before);
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(dir.join("archive.sqlite3-journal"))
        .unwrap();
    let out = run(
        &rust(),
        &s,
        &[
            "runtime",
            "history",
            "recover",
            "--force",
            "--history-dir",
            root.to_str().unwrap(),
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    let value: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(value["recovery_attempted"], true);
    assert!(
        dir.join(value["private_backups"]["manifest"].as_str().unwrap())
            .exists()
    );
    let out = run(
        &rust(),
        &s,
        &[
            "runtime",
            "history",
            "stats",
            "--history-dir",
            root.to_str().unwrap(),
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    let value: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(value["events"], 1);
    assert_eq!(value["integrity_verified"], true);
    assert_eq!(server.request_count(), 0);
}
