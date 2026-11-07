//! Named scanner config is local, transactional and independent of live AIRS.
use airs_test_harness::{Cli, Run, RunOptions, Sandbox, run};
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

const KEY: &str = "FAKE-NAMED-SCANNER-KEY";
const FLAT: &str = "FAKE-LEGACY-SCANNER-KEY";
fn cli() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}
fn setup() -> (Sandbox, String) {
    let sandbox = Sandbox::new();
    sandbox.write_store_index(&[("prod", "100")], Some("prod"));
    let original = format!(
        "airsApiKey = \"{FLAT}\"\nairsApiEndpoint = \"http://127.0.0.1:1234/scan\"\nmgmtTsgId = \"100\"\n"
    );
    sandbox.write_tenant_file_allowing_external_endpoints("prod", original.as_bytes(), 0o600);
    (sandbox, original)
}
fn options(write: bool) -> RunOptions {
    let value = RunOptions::new().secret(KEY).secret(FLAT);
    if write {
        value.allow_writes_under("store")
    } else {
        value
    }
}
fn rs(s: &Sandbox, args: &[&str], opts: RunOptions) -> Run {
    run(&cli(), s, args, &opts)
}
fn input(app: &str) -> String {
    format!(
        "applicationId = \"{app}\"\nendpoint = \"http://127.0.0.1:1234/scan/\"\napiKey = \"{KEY}\"\n"
    )
}
fn snapshots(s: &Sandbox) -> Vec<std::path::PathBuf> {
    fs::read_dir(s.store_tenant_path("prod").parent().unwrap())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".prod.pre-")
        })
        .collect()
}
fn set(s: &Sandbox, name: &str, app: &str) -> Run {
    rs(
        s,
        &["tenant", "scanners", "set", "prod", name, "--stdin"],
        options(true).stdin(input(app)),
    )
}

#[test]
fn set_snapshots_old_format_and_all_read_views_are_masked_without_writes() {
    let (s, original) = setup();
    let out = set(&s, "monitor", "app-a");
    out.assert_success();
    assert!(out.stdout.is_empty());
    let saved = fs::read_to_string(s.store_tenant_path("prod")).unwrap();
    assert!(saved.contains("[scanners.monitor]"));
    assert!(saved.contains("numRetries = 0"));
    let copies = snapshots(&s);
    assert_eq!(copies.len(), 1);
    assert_eq!(fs::read_to_string(&copies[0]).unwrap(), original);
    assert_eq!(
        fs::metadata(&copies[0]).unwrap().permissions().mode() & 0o777,
        0o600
    );
    for args in [
        vec!["tenant", "scanners", "list", "prod", "--output", "json"],
        vec!["tenant", "get", "prod", "scanners", "--output", "json"],
        vec!["tenant", "read", "prod", "--output", "json"],
    ] {
        let out = rs(&s, &args, options(false));
        out.assert_success();
        assert!(!out.stdout.contains(KEY));
        assert!(out.stdout.contains("hasApiKey"));
    }
    rs(
        &s,
        &["tenant", "set", "prod", "mgmtClientId", "fixture-client"],
        options(true),
    )
    .assert_success();
    assert!(
        fs::read_to_string(s.store_tenant_path("prod"))
            .unwrap()
            .contains("[scanners.monitor]")
    );
    assert_eq!(snapshots(&s).len(), 1);
}

#[test]
fn bind_default_requires_explicit_kind_and_copies_effective_retry_five() {
    let (s, original) = setup();
    rs(
        &s,
        &[
            "tenant",
            "scanners",
            "bind-default",
            "prod",
            "missing-kind",
            "--app-id",
            "app-a",
            "--use",
            "token",
        ],
        options(false),
    )
    .assert_code(1);
    assert!(snapshots(&s).is_empty());
    let both = format!("airsApiToken = \"FAKE-SCANNER-TOKEN\"\n{original}");
    s.write_tenant_file_allowing_external_endpoints("prod", both.as_bytes(), 0o600);
    let args = [
        "tenant",
        "scanners",
        "bind-default",
        "prod",
        "monitor",
        "--app-id",
        "app-a",
    ];
    let out = rs(&s, &args, options(false).secret("FAKE-SCANNER-TOKEN"));
    out.assert_code(1);
    assert!(out.stderr.contains("select --use key or --use token"));
    assert!(snapshots(&s).is_empty());
    let mut chosen = args.to_vec();
    chosen.extend(["--use", "key"]);
    rs(&s, &chosen, options(true).secret("FAKE-SCANNER-TOKEN")).assert_success();
    let saved = fs::read_to_string(s.store_tenant_path("prod")).unwrap();
    assert!(saved.contains("numRetries = 5"));
    let named = saved.split("[scanners.monitor]").nth(1).unwrap();
    assert!(named.contains("apiKey ="));
    assert!(!named.contains("apiToken ="));
    let mut token = args.to_vec();
    token.extend(["--use", "token", "--replace"]);
    rs(&s, &token, options(true).secret("FAKE-SCANNER-TOKEN")).assert_success();
    let saved = fs::read_to_string(s.store_tenant_path("prod")).unwrap();
    let named = saved.split("[scanners.monitor]").nth(1).unwrap();
    assert!(named.contains("apiToken ="));
    assert!(!named.contains("apiKey ="));
    assert_eq!(snapshots(&s).len(), 1);
}

#[test]
fn duplicate_bindings_and_secret_input_errors_never_change_existing_store() {
    let (s, _) = setup();
    set(&s, "monitor", "app-a").assert_success();
    let out = rs(
        &s,
        &["tenant", "scanners", "set", "prod", "other", "--stdin"],
        options(false).stdin(input("app-a")),
    );
    out.assert_code(1);
    assert!(out.stderr.contains("already bound"));
    for body in [
        format!("{}apiToken = \"FAKE-TOKEN\"\n", input("app-b")),
        "apiKey = \"DO-NOT-ECHO\"\nunknown = [\n".into(),
    ] {
        let out = rs(
            &s,
            &["tenant", "scanners", "set", "prod", "other", "--stdin"],
            options(false).secret("DO-NOT-ECHO").stdin(body),
        );
        out.assert_code(2);
        assert!(out.stderr.contains("Invalid scanner credential input"));
    }
    rs(
        &s,
        &["tenant", "scanners", "remove", "prod", "monitor", "--force"],
        options(true),
    )
    .assert_success();
    let saved = fs::read_to_string(s.store_tenant_path("prod")).unwrap();
    assert!(saved.ends_with("[scanners]\n"));
    assert!(!saved.contains(KEY));
}

#[test]
fn expanded_endpoint_cannot_publish_an_unreadable_namespace() {
    let (s, original) = setup();
    let endpoint = format!("https://example.invalid/{}", "é".repeat(1000));
    let body = format!(
        "applicationId = \"app-a\"\nendpoint = {}\napiKey = \"{KEY}\"\n",
        serde_json::json!(endpoint)
    );
    rs(
        &s,
        &["tenant", "scanners", "set", "prod", "monitor", "--stdin"],
        options(false).stdin(body),
    )
    .assert_code(2);
    assert_eq!(
        fs::read_to_string(s.store_tenant_path("prod")).unwrap(),
        original
    );
    assert!(snapshots(&s).is_empty());
    rs(
        &s,
        &["tenant", "read", "prod", "--output", "json"],
        options(false),
    )
    .assert_success();
}

#[test]
fn restore_is_pinned_and_private_and_can_recover_both_formats() {
    let (s, original) = setup();
    set(&s, "monitor", "app-a").assert_success();
    let first = snapshots(&s).remove(0);
    let path = first.to_str().unwrap();
    let out = rs(
        &s,
        &[
            "tenant",
            "scanners",
            "restore",
            "prod",
            "--file",
            path,
            "--expect-tsg",
            "200",
            "--force",
        ],
        options(false),
    );
    out.assert_code(1);
    rs(
        &s,
        &[
            "tenant",
            "scanners",
            "restore",
            "prod",
            "--file",
            path,
            "--expect-tsg",
            "100",
            "--force",
        ],
        options(true),
    )
    .assert_success();
    assert_eq!(
        fs::read_to_string(s.store_tenant_path("prod")).unwrap(),
        original
    );
    let named = snapshots(&s)
        .into_iter()
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("pre-restore")
        })
        .unwrap();
    rs(
        &s,
        &[
            "tenant",
            "scanners",
            "restore",
            "prod",
            "--file",
            named.to_str().unwrap(),
            "--expect-tsg",
            "100",
            "--force",
        ],
        options(true),
    )
    .assert_success();
    assert!(
        fs::read_to_string(s.store_tenant_path("prod"))
            .unwrap()
            .contains("[scanners.monitor]")
    );
    let out = rs(
        &s,
        &[
            "tenant",
            "scanners",
            "snapshots",
            "prod",
            "--output",
            "json",
        ],
        options(false),
    );
    out.assert_success();
    let list: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 3);
    rs(
        &s,
        &[
            "tenant",
            "scanners",
            "delete-snapshot",
            "prod",
            "--file",
            path,
            "--force",
        ],
        options(true),
    )
    .assert_success();
    assert!(!first.exists());
}

#[test]
fn unsafe_input_and_scalar_namespace_writes_are_refused() {
    let (s, _) = setup();
    let file = s.tmp().join("input.toml");
    fs::write(&file, input("app-a")).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
    let args = [
        "tenant",
        "scanners",
        "set",
        "prod",
        "monitor",
        "--file",
        file.to_str().unwrap(),
    ];
    rs(&s, &args, options(false)).assert_code(1);
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    let link = s.tmp().join("link.toml");
    symlink(&file, &link).unwrap();
    rs(
        &s,
        &[
            "tenant",
            "scanners",
            "set",
            "prod",
            "monitor",
            "--file",
            link.to_str().unwrap(),
        ],
        options(false),
    )
    .assert_code(1);
    for leaf in ["set", "unset"] {
        let out = rs(&s, &["tenant", leaf, "prod", "scanners"], options(false));
        out.assert_code(2);
        assert!(out.stderr.contains("Use 'airs tenant scanners'"));
    }
    rs(&s, &args, options(true)).assert_success();
}

#[test]
fn repeated_scanner_options_are_usage_errors_before_input_or_store_access() {
    let s = Sandbox::new();
    let one = s.tmp().join("missing-one.toml");
    let two = s.tmp().join("missing-two.toml");
    for args in [
        vec![
            "tenant",
            "scanners",
            "set",
            "prod",
            "monitor",
            "--file",
            one.to_str().unwrap(),
            "--file",
            two.to_str().unwrap(),
        ],
        vec![
            "tenant",
            "scanners",
            "bind-default",
            "prod",
            "monitor",
            "--app-id",
            "app-a",
            "--app-id",
            "app-b",
        ],
        vec![
            "tenant",
            "scanners",
            "bind-default",
            "prod",
            "monitor",
            "--app-id",
            "app-a",
            "--use",
            "key",
            "--use",
            "token",
        ],
        vec![
            "runtime",
            "scan",
            "--profile",
            "p",
            "--scanner",
            "alpha",
            "--scanner",
            "beta",
            "Synthetic",
        ],
        vec![
            "runtime",
            "bulk-scan",
            "--profile",
            "p",
            "--file",
            one.to_str().unwrap(),
            "--scanner",
            "alpha",
            "--scanner",
            "beta",
        ],
        vec![
            "runtime",
            "scan-logs",
            "results",
            "--scan-id",
            "00000000-0000-4000-8000-000000000001",
            "--scanner",
            "alpha",
            "--scanner",
            "beta",
        ],
    ] {
        rs(&s, &args, options(false)).assert_code(2);
    }
}

#[test]
fn submitted_credential_is_masked_in_later_store_path_errors() {
    let s = Sandbox::new();
    let root = s.tmp().join(KEY);
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::create_dir(root.join("tenants")).unwrap();
    fs::set_permissions(root.join("tenants"), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        root.join("config.toml"),
        "version = 1\nactive = \"prod\"\n\n[tenants.prod]\ntsg_id = \"100\"\n",
    )
    .unwrap();
    fs::set_permissions(root.join("config.toml"), fs::Permissions::from_mode(0o600)).unwrap();
    let out = rs(
        &s,
        &["tenant", "scanners", "set", "prod", "monitor", "--stdin"],
        options(false)
            .env("AIRS_CLI_CONFIG_DIR", root.to_str().unwrap())
            .stdin(input("app-a")),
    );
    out.assert_code(1);
    assert!(!out.stderr.contains(KEY));
    assert!(out.stderr.contains("withheld"));
    assert!(!root.join("config.lock").exists());
}

#[test]
fn named_only_doctor_masks_config_path_coincidences_in_every_format() {
    let s = Sandbox::new();
    let root = s.tmp().join(KEY);
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::create_dir(root.join("tenants")).unwrap();
    fs::set_permissions(root.join("tenants"), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        root.join("config.toml"),
        "version = 1\nactive = \"prod\"\n\n[tenants.prod]\ntsg_id = \"100\"\n",
    )
    .unwrap();
    let endpoints: String = airs_test_harness::store::RUST_ENDPOINT_KEYS
        .iter()
        .map(|key| format!("{key} = \"http://127.0.0.1:1234/fixture\"\n"))
        .collect();
    fs::write(root.join("tenants/prod.toml"), format!("mgmtTsgId = \"100\"\n{endpoints}\n[scanners]\n[scanners.monitor]\n{}revision = \"00000000-0000-4000-8000-000000000001\"\nnumRetries = 0\n", input("app-a"))).unwrap();
    for file in [root.join("config.toml"), root.join("tenants/prod.toml")] {
        fs::set_permissions(file, fs::Permissions::from_mode(0o600)).unwrap();
    }
    for format in ["json", "pretty", "table"] {
        let out = rs(
            &s,
            &["doctor", "--output", format],
            options(false).env("AIRS_CLI_CONFIG_DIR", root.to_str().unwrap()),
        );
        out.assert_success();
        assert!(!out.stdout.contains(KEY));
        assert!(!out.stderr.contains(KEY));
        assert!(out.stdout.contains("withheld"));
    }
}
