//! Executable tests of `tenant import` (DESIGN §3.12, §3.13): every validation and
//! source branch, the key conversion and its notes, the per-entry actions, the
//! selection rule, `--dry-run`, `--from-config`, the report in every format, the
//! TypeScript files left untouched, and the provisioner/importer cross-check
//! (`store::provision_matches_import`).
//!
//! Every run is sandboxed (`AIRS_CLI_CONFIG_DIR=<sandbox>/store`,
//! `PRISMA_AIRS_TENANTS_PATH=<sandbox>/state/tenants.json`). The TypeScript side is
//! written first and the Rust store then emptied (`ts_only`), so the store holds only
//! what the import writes. A run that may write is granted `store/` only; the
//! unchanged-sandbox rule then proves the TypeScript `state/` and `configs/` trees are
//! byte-, mode- and mtime-identical, and the tests below assert it explicitly too.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;
use std::time::{Duration, SystemTime};

use airs_test_harness::{
    Cli, FixtureConfig, RegistryEntry, Run, RunOptions, Sandbox, lock_holder, run,
};
use serde_json::{Value, json};

const BASE: &str = "http://127.0.0.1:9";
const NAME_RULE: &str = "Tenant name must be 1–64 letters, digits, hyphens or underscores and start with a letter/digit";
const PIN_NOTE: &str = "config TSG differs from its registered identity";
const EXISTS_NOTE: &str = "already exists; use --replace to overwrite";
/// The conversion notes of [`FixtureConfig::standard`], in its file order.
const STANDARD_NOTES: [&str; 6] = [
    "renamed agentGuardDataEndpoint to supplychainDataEndpoint",
    "renamed agentGuardMgmtEndpoint to supplychainMgmtEndpoint",
    "renamed aiGwDataEndpoint to gatewayDataEndpoint",
    "renamed aiGwAdminEndpoint to gatewayAdminEndpoint",
    "renamed aiGwInferenceEndpoint to gatewayInferenceEndpoint",
    "dropped typesafeBaseUrl (no longer used)",
];

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

/// A run that may change `store/` (and nothing else).
fn writes() -> RunOptions {
    RunOptions::new().allow_writes_under("store")
}

fn rs(sandbox: &Sandbox, args: &[&str], options: RunOptions) -> Run {
    run(&rust(), sandbox, args, &options)
}

/// `tenant import <args>` that may write the store.
fn import(sandbox: &Sandbox, args: &[&str]) -> Run {
    let mut full = vec!["tenant", "import"];
    full.extend_from_slice(args);
    rs(sandbox, &full, writes())
}

/// `tenant import <args> --output json`: the exit code and the report rows.
fn import_json(sandbox: &Sandbox, args: &[&str]) -> (Run, Value) {
    let mut full = args.to_vec();
    full.extend_from_slice(&["--output", "json"]);
    let out = import(sandbox, &full);
    let rows = out.json();
    (out, rows)
}

fn usage(message: &str) -> String {
    format!("\n  ✗ {message}\n\n")
}

fn op_error(message: &str) -> String {
    format!("\n  ✗ Error: {message}\n\n")
}

fn standard(tsg: &str) -> FixtureConfig {
    FixtureConfig::standard(BASE, tsg)
}

/// The TypeScript side: each `(name, tsg, config)` written to `configs/<name>.json` and
/// registered with `active`; then the Rust store is emptied and detached.
fn ts_tenants(sandbox: &Sandbox, tenants: &[(&str, &str, FixtureConfig)], active: Option<&str>) {
    let entries: Vec<RegistryEntry> = tenants
        .iter()
        .enumerate()
        // Numbered file names: `Dev` and `dev` must not share a file on a
        // case-insensitive file system.
        .map(|(index, (name, tsg, config))| {
            let path = sandbox.write_fixture_config(&format!("{name}-{index}"), config);
            RegistryEntry::new(name, path, tsg)
        })
        .collect();
    sandbox.write_registry(&entries, active);
    sandbox.ts_only();
}

/// A report row as JSON.
fn row(name: &str, tsg: &str, action: &str, notes: &[&str]) -> Value {
    json!({"name": name, "tsgId": tsg, "action": action, "notes": notes})
}

fn index(sandbox: &Sandbox) -> String {
    fs::read_to_string(sandbox.store_index_path()).unwrap_or_default()
}

fn tenant_file(sandbox: &Sandbox, name: &str) -> String {
    fs::read_to_string(sandbox.store_tenant_path(name)).unwrap()
}

/// The canonical tenant file of `config` (the harness provisioner's conversion; the
/// cross-check below proves it equals the importer's).
fn converted(config: &FixtureConfig) -> String {
    airs_test_harness::store::convert_config(config.to_json().as_bytes())
        .unwrap()
        .to_toml()
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
}

/// Every entry under `dir`, relative, sorted (empty when it does not exist).
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

/// Bytes, mode and mtime of every file under `state/` and `configs/` (the TypeScript
/// side), plus the entry names (a new `*.lock` would show).
type Fingerprint = BTreeMap<String, (Vec<u8>, u32, SystemTime)>;

fn ts_fingerprint(sandbox: &Sandbox) -> Fingerprint {
    let mut out = BTreeMap::new();
    for dir in [sandbox.state_dir(), sandbox.configs_dir()] {
        for relative in listing(&dir) {
            let path = dir.join(&relative);
            let metadata = fs::symlink_metadata(&path).unwrap();
            let bytes = if metadata.is_file() {
                fs::read(&path).unwrap()
            } else {
                Vec::new()
            };
            out.insert(
                path.display().to_string(),
                (
                    bytes,
                    metadata.permissions().mode(),
                    metadata.modified().unwrap(),
                ),
            );
        }
    }
    out
}

fn assert_ts_untouched(sandbox: &Sandbox, before: &Fingerprint) {
    let after = ts_fingerprint(sandbox);
    assert_eq!(
        after.keys().collect::<Vec<_>>(),
        before.keys().collect::<Vec<_>>(),
        "no TypeScript file appeared or disappeared"
    );
    for (path, (bytes, mode, mtime)) in before {
        let (now_bytes, now_mode, now_mtime) = &after[path];
        assert!(now_bytes == bytes, "{path}: bytes changed");
        assert_eq!(now_mode, mode, "{path}: mode changed");
        assert_eq!(now_mtime, mtime, "{path}: mtime changed");
    }
}

// ---------------------------------------------------------------------------------------

mod validation {
    use super::*;

    #[test]
    fn usage_errors_in_order_before_anything_is_read() {
        let sandbox = Sandbox::new();
        // A corrupt registry and store: usage errors never read either.
        sandbox.write_registry_raw(b"{");
        sandbox.write_store_raw(b"[", 0o600);
        for (args, message) in [
            (
                &["--from", "", "--from-config", "", "--name", "-x"][..],
                "--from and --from-config cannot be combined",
            ),
            (&["--from", "", "--name", "-x"], "--from needs a path"),
            (
                &["--from-config", "", "--name", "-x"],
                "--from-config needs a path",
            ),
            (
                &["--from-config", "c.json"],
                "--from-config requires --name <name>",
            ),
            (&["--name", "-x"], NAME_RULE),
            (&["--name", ""], NAME_RULE),
            (&["--name", "a/b"], NAME_RULE),
        ] {
            let mut full = vec!["tenant", "import"];
            full.extend_from_slice(args);
            let out = rs(&sandbox, &full, RunOptions::new());
            out.assert_code(2);
            assert_eq!(out.stderr, usage(message), "{args:?}");
            assert_eq!(out.stdout, "");
        }
    }

    #[test]
    fn output_then_tenant_flag_are_checked_first() {
        let sandbox = Sandbox::new();
        let out = rs(
            &sandbox,
            &[
                "--tenant", "dev", "tenant", "import", "--output", "yaml", "--name", "-x",
            ],
            RunOptions::new(),
        );
        out.assert_code(2);
        assert!(
            out.stderr.contains("Invalid output format 'yaml'"),
            "{}",
            out.stderr
        );
        let out = rs(
            &sandbox,
            &["--tenant", "dev", "tenant", "import", "--name", "-x"],
            RunOptions::new(),
        );
        out.assert_code(2);
        assert_eq!(
            out.stderr,
            usage("--tenant cannot be used with 'tenant import'; name the tenant as an argument")
        );
    }

    #[test]
    fn help_describes_the_command() {
        let sandbox = Sandbox::new();
        let out = rs(&sandbox, &["tenant", "import", "--help"], RunOptions::new());
        out.assert_success();
        for text in [
            "Copy tenants from the TypeScript airs-cli; its files are not changed",
            "--from <path>",
            "TypeScript tenant registry to read (default: the one the TypeScript CLI uses)",
            "--from-config <path>",
            "Import one TypeScript JSON config file as the tenant named by --name",
            "--name <name>",
            "--replace",
            "Overwrite existing tenants of the same name",
            "--dry-run",
            "Report what would be imported; write nothing",
        ] {
            assert!(out.stdout.contains(text), "{text}:\n{}", out.stdout);
        }
        let group = rs(&sandbox, &["tenant", "--help"], RunOptions::new());
        assert!(
            group.stdout.contains(
                "import    Copy tenants from the TypeScript airs-cli; its files are not changed"
            ),
            "{}",
            group.stdout
        );
        assert!(
            out.stdout.contains("Output format: pretty, table, json"),
            "{}",
            out.stdout
        );
    }
}

mod source {
    use super::*;

    #[test]
    fn a_missing_registry_names_both_options() {
        let sandbox = Sandbox::new();
        let out = import(&sandbox, &[]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            op_error(&format!(
                "No TypeScript tenant registry at {}; pass --from <registry> or --from-config <file> --name <name>",
                sandbox.registry_path().display()
            ))
        );
        assert_eq!(out.stdout, "");
        assert_eq!(listing(&sandbox.store_dir()), Vec::<String>::new());
        let missing = sandbox.root().join("state/nope/tenants.json");
        let out = import(&sandbox, &["--from", &missing.display().to_string()]);
        out.assert_code(1);
        assert!(
            out.stderr.contains(&format!(
                "No TypeScript tenant registry at {}",
                missing.display()
            )),
            "{}",
            out.stderr
        );
    }

    #[test]
    fn from_is_resolved_against_the_current_directory() {
        let sandbox = Sandbox::new();
        let config = sandbox.write_fixture_config("dev", &standard("100"));
        let registry =
            airs_test_harness::registry_json(&[RegistryEntry::new("dev", &config, "100")], None);
        sandbox.write_file("cwd/sub/r.json", registry.as_bytes(), 0o600);
        sandbox.ts_only();
        let (out, rows) = import_json(&sandbox, &["--from", "./sub/../sub//r.json"]);
        out.assert_success();
        assert_eq!(rows, json!([row("dev", "100", "import", &STANDARD_NOTES)]));
        // A path that does not exist is named absolute and normalised.
        let out = import(&sandbox, &["--from", "x/../missing.json"]);
        out.assert_code(1);
        assert!(
            out.stderr.contains(&format!(
                "No TypeScript tenant registry at {}; ",
                sandbox.cwd().join("missing.json").display()
            )),
            "{}",
            out.stderr
        );
    }

    #[test]
    fn an_invalid_registry_gets_the_generic_message_and_writes_nothing() {
        let sandbox = Sandbox::new();
        let generic = op_error(&format!(
            "Cannot read tenant registry at {}; restore a valid registry before proceeding",
            sandbox.registry_path().display()
        ));
        // D44: a lone surrogate escape (in a config path inside the sandbox).
        let surrogate = format!(
            r#"{{"version":1,"active":null,"tenants":[{{"name":"dev","configPath":"{}/c\ud800","tsgId":"1"}}]}}"#,
            sandbox.configs_dir().display()
        );
        for bytes in [
            &b"{"[..],
            b"",
            br#"{"version":2,"active":null,"tenants":[]}"#,
            br#"{"version":1,"tenants":[]}"#,
            // D27: invalid UTF-8 (TypeScript decodes it lossily).
            b"{\"version\":1,\"active\":null,\"tenants\":[],\"x\":\"\xff\"}",
            surrogate.as_bytes(),
        ] {
            sandbox.write_registry_raw(bytes);
            sandbox.ts_only();
            let out = import(&sandbox, &[]);
            out.assert_code(1);
            assert_eq!(out.stderr, generic, "{:?}", String::from_utf8_lossy(bytes));
            assert_eq!(out.stdout, "");
            assert_eq!(listing(&sandbox.store_dir()), Vec::<String>::new());
        }
    }

    #[test]
    fn a_symlinked_registry_is_refused_and_a_symlinked_config_is_followed() {
        let sandbox = Sandbox::new();
        let real = sandbox.write_fixture_config("real", &standard("100"));
        let link = sandbox.configs_dir().join("linked.json");
        symlink(&real, &link).unwrap();
        sandbox.write_registry(&[RegistryEntry::new("dev", &link, "100")], None);
        sandbox.ts_only();
        let (out, rows) = import_json(&sandbox, &[]);
        out.assert_success();
        assert_eq!(
            rows[0]["action"], "import",
            "config symlinks are followed (TS)"
        );
        assert_eq!(tenant_file(&sandbox, "dev"), converted(&standard("100")));
    }

    #[test]
    fn a_registry_symlink_is_rejected() {
        let sandbox = Sandbox::new();
        let config = sandbox.write_fixture_config("dev", &standard("100"));
        sandbox.write_registry(&[RegistryEntry::new("dev", &config, "100")], None);
        sandbox.ts_only();
        let linked = sandbox.state_dir().join("linked.json");
        symlink(sandbox.registry_path(), &linked).unwrap();
        let before = ts_fingerprint(&sandbox);
        let out = import(&sandbox, &["--from", &linked.display().to_string()]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            op_error(&format!(
                "Cannot read tenant registry at {}; restore a valid registry before proceeding",
                linked.display()
            ))
        );
        assert_eq!(listing(&sandbox.store_dir()), Vec::<String>::new());
        assert_ts_untouched(&sandbox, &before);
    }

    #[test]
    fn the_default_registry_follows_the_typescript_resolution() {
        let sandbox = Sandbox::new();
        let config = sandbox.write_fixture_config("dev", &standard("100"));
        let registry =
            airs_test_harness::registry_json(&[RegistryEntry::new("dev", &config, "100")], None);
        sandbox.write_file(
            "tmp/xdg/prisma-airs/tenants.json",
            registry.as_bytes(),
            0o600,
        );
        sandbox.ts_only();
        let xdg = sandbox.tmp().join("xdg").display().to_string();
        let options = || {
            writes()
                .without_env("PRISMA_AIRS_TENANTS_PATH")
                .env("XDG_STATE_HOME", xdg.clone())
        };
        let out = rs(
            &sandbox,
            &["tenant", "import", "--output", "json"],
            options(),
        );
        out.assert_success();
        assert_eq!(out.json()[0]["name"], "dev");
        let out = rs(
            &sandbox,
            &["tenant", "import"],
            writes()
                .without_env("PRISMA_AIRS_TENANTS_PATH")
                .env("XDG_STATE_HOME", "relative"),
        );
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            op_error("XDG_STATE_HOME must be an absolute path")
        );
    }

    #[test]
    fn name_selects_one_entry_or_fails() {
        let sandbox = Sandbox::new();
        ts_tenants(
            &sandbox,
            &[
                ("dev", "100", standard("100")),
                ("prod", "200", standard("200")),
            ],
            Some("dev"),
        );
        let out = import(&sandbox, &["--name", "ghost"]);
        out.assert_code(1);
        assert_eq!(out.stderr, op_error("Tenant not found"));
        assert_eq!(listing(&sandbox.store_dir()), Vec::<String>::new());
        let (out, rows) = import_json(&sandbox, &["--name", "prod"]);
        out.assert_success();
        assert_eq!(rows, json!([row("prod", "200", "import", &STANDARD_NOTES)]));
        assert!(
            !out.stderr.contains("Selected"),
            "the active entry was not imported"
        );
        assert_eq!(
            index(&sandbox),
            "version = 1\n\n[tenants.prod]\ntsg_id = \"200\"\n"
        );
    }
}

mod conversion {
    use super::*;

    #[test]
    fn keys_are_renamed_dropped_and_reported_in_file_order() {
        let sandbox = Sandbox::new();
        let config = standard("100")
            .with("defaultOutput", "markdown")
            .with("dataDir", "/runs")
            .with("typesafeApiKey", "FAKE-TYPESAFE-KEY")
            .with("typesafeModel", "m")
            .with("aiGwInferenceApiKey", "FAKE-GATEWAY-KEY")
            .with("aiGwInferenceModel", "")
            .with("airsApiToken", "")
            .with("scanConcurrency", "7")
            .with("airsNumRetries", true)
            .with("customSetting", "FAKE-CUSTOM-VALUE")
            .with("redTeamTokenEndpoint", "http://127.0.0.1:9/old");
        ts_tenants(&sandbox, &[("dev", "100", config.clone())], None);
        let before = ts_fingerprint(&sandbox);
        let (out, rows) = import_json(&sandbox, &[]);
        out.assert_success();
        let mut notes: Vec<&str> = STANDARD_NOTES.to_vec();
        notes.extend([
            "dropped defaultOutput markdown (format no longer supported)",
            "dropped dataDir (no longer used)",
            "dropped typesafeApiKey (no longer used)",
            "dropped typesafeModel (no longer used)",
            "renamed aiGwInferenceApiKey to gatewayInferenceApiKey",
            "dropped empty aiGwInferenceModel",
            "dropped empty airsApiToken",
            "ignored unknown key customSetting",
            "ignored unknown key redTeamTokenEndpoint",
        ]);
        assert_eq!(rows, json!([row("dev", "100", "import", &notes)]));
        let file = tenant_file(&sandbox, "dev");
        assert_eq!(file, converted(&config));
        assert!(file.contains("gatewayInferenceApiKey = \"FAKE-GATEWAY-KEY\"\n"));
        assert!(file.contains("airsNumRetries = 1\nmgmtClientId"), "{file}");
        assert!(
            file.ends_with("gatewayInferenceApiKey = \"FAKE-GATEWAY-KEY\"\nscanConcurrency = 7\n"),
            "{file}"
        );
        for gone in [
            "typesafe",
            "dataDir",
            "aiGw",
            "agentGuard",
            "customSetting",
            "defaultOutput",
            "TokenEndpoint = \"http://127.0.0.1:9/old",
        ] {
            assert!(!file.contains(gone), "{gone}: {file}");
        }
        // The imported file is valid for every reader.
        let read = rs(
            &sandbox,
            &["tenant", "read", "dev", "--output", "json"],
            RunOptions::new(),
        );
        read.assert_success();
        assert_ts_untouched(&sandbox, &before);
    }

    #[test]
    fn the_exact_tenant_file_bytes() {
        let sandbox = Sandbox::new();
        ts_tenants(&sandbox, &[("dev", "100", standard("100"))], None);
        import(&sandbox, &[]).assert_success();
        let b = BASE;
        assert_eq!(
            tenant_file(&sandbox, "dev"),
            format!(
                "airsApiKey = \"FAKE-APIKEY-100\"\n\
                 airsApiEndpoint = \"{b}/scan\"\n\
                 airsNumRetries = 0\n\
                 mgmtClientId = \"client-100\"\n\
                 mgmtClientSecret = \"FAKE-SECRET-100\"\n\
                 mgmtTsgId = \"100\"\n\
                 mgmtEndpoint = \"{b}/aisec\"\n\
                 mgmtDashboardEndpoint = \"{b}/dashboard\"\n\
                 mgmtTokenEndpoint = \"{b}/oauth/token\"\n\
                 dlpEndpoint = \"{b}/dlp\"\n\
                 redTeamDataEndpoint = \"{b}/redteam/data\"\n\
                 redTeamMgmtEndpoint = \"{b}/redteam/mgmt\"\n\
                 redTeamNetworkBrokerEndpoint = \"{b}/redteam/broker\"\n\
                 modelSecDataEndpoint = \"{b}/aims/data\"\n\
                 modelSecMgmtEndpoint = \"{b}/aims/mgmt\"\n\
                 supplychainDataEndpoint = \"{b}/agentguard/data\"\n\
                 supplychainMgmtEndpoint = \"{b}/agentguard/mgmt\"\n\
                 gatewayDataEndpoint = \"{b}/ai_gw/v2\"\n\
                 gatewayAdminEndpoint = \"{b}/ai_gw/admin/v2\"\n\
                 iamEndpoint = \"{b}/iam\"\n\
                 gatewayInferenceEndpoint = \"{b}/ai_gw/inference\"\n"
            )
        );
        assert_eq!(
            index(&sandbox),
            "version = 1\n\n[tenants.dev]\ntsg_id = \"100\"\n"
        );
    }

    #[test]
    fn unreadable_and_mismatched_sources_are_skipped_and_fail_the_run() {
        let sandbox = Sandbox::new();
        let good = sandbox.write_fixture_config("good", &standard("100"));
        let pinned = sandbox.write_fixture_config("pinned", &standard("200"));
        let invalid = sandbox.write_config_raw("invalid", br#"{"mgmtClientSecret":"FAKE-S1",oops"#);
        // Standard configs (every endpoint on loopback) with one defect each.
        let defect = |replacement: &[u8]| {
            let text = standard("1").to_json();
            let (before, after) = text.split_once("FAKE-APIKEY-1").unwrap();
            [before.as_bytes(), replacement, after.as_bytes()].concat()
        };
        let utf8 = sandbox.write_config_raw("utf8", &defect(b"FAKE-\xff"));
        let surrogate = sandbox.write_config_raw("surrogate", &defect(br"FAKE-\ud800"));
        let schema =
            sandbox.write_fixture_config("schema", &standard("1").with("scanConcurrency", 99));
        let missing = sandbox.configs_dir().join("missing.json");
        let no_tsg = sandbox.write_fixture_config("notsg", &standard("1").without("mgmtTsgId"));
        let entries = [
            RegistryEntry::new("good", &good, "100"),
            RegistryEntry::new("pinned", &pinned, "201"),
            RegistryEntry::new("invalid", &invalid, "1"),
            RegistryEntry::new("utf8", &utf8, "1"),
            RegistryEntry::new("surrogate", &surrogate, "1"),
            RegistryEntry::new("schema", &schema, "1"),
            RegistryEntry::new("missing", &missing, "1"),
            RegistryEntry::new("notsg", &no_tsg, "1"),
        ];
        sandbox.write_registry(&entries, Some("good"));
        sandbox.ts_only();
        let before = ts_fingerprint(&sandbox);
        let (out, rows) = import_json(&sandbox, &[]);
        out.assert_code(1);
        let unreadable =
            |path: &Path| format!("cannot read a valid tenant config at {}", path.display());
        assert_eq!(
            rows,
            json!([
                row("good", "100", "import", &STANDARD_NOTES),
                row("invalid", "1", "skip", &[&unreadable(&invalid)]),
                row("missing", "1", "skip", &[&unreadable(&missing)]),
                row("notsg", "1", "skip", &[PIN_NOTE]),
                row("pinned", "201", "skip", &[PIN_NOTE]),
                row("schema", "1", "skip", &[&unreadable(&schema)]),
                row("surrogate", "1", "skip", &[&unreadable(&surrogate)]),
                row("utf8", "1", "skip", &[&unreadable(&utf8)]),
            ])
        );
        assert_eq!(
            out.stderr,
            format!(
                "  Imported 1, replaced 0, skipped 7 into {}\n  Selected good (selected in the TypeScript registry)\n{}",
                sandbox.store_dir().display(),
                op_error("7 tenants could not be imported")
            )
        );
        assert_eq!(
            index(&sandbox),
            "version = 1\nactive = \"good\"\n\n[tenants.good]\ntsg_id = \"100\"\n"
        );
        assert_eq!(listing(&sandbox.store_dir().join("tenants")), ["good.toml"]);
        assert_ts_untouched(&sandbox, &before);
        // One failure: the singular message.
        sandbox.write_registry(&entries[1..2], None);
        sandbox.ts_only();
        let out = import(&sandbox, &[]);
        out.assert_code(1);
        assert!(
            out.stderr
                .ends_with(&op_error("1 tenant could not be imported"))
        );
    }
}

mod store_rules {
    use super::*;

    #[test]
    fn an_existing_tenant_is_kept_without_replace_and_overwritten_with_it() {
        let sandbox = Sandbox::new();
        let fresh = standard("100").with("defaultOutput", "json");
        ts_tenants(&sandbox, &[("dev", "100", fresh.clone())], Some("dev"));
        sandbox.write_rust_tenant("dev", &standard("900"));
        sandbox.write_store_index(&[("dev", "900")], None);
        let old_file = tenant_file(&sandbox, "dev");
        let old_index = index(&sandbox);
        let (out, rows) = import_json(&sandbox, &[]);
        out.assert_success();
        assert_eq!(rows, json!([row("dev", "100", "skip", &[EXISTS_NOTE])]));
        assert_eq!(tenant_file(&sandbox, "dev"), old_file);
        assert_eq!(index(&sandbox), old_index, "nothing written");
        assert!(
            !sandbox.store_dir().join("config.lock").exists(),
            "no lock taken"
        );
        let (out, rows) = import_json(&sandbox, &["--replace"]);
        out.assert_success();
        assert_eq!(rows, json!([row("dev", "100", "replace", &STANDARD_NOTES)]));
        assert_eq!(tenant_file(&sandbox, "dev"), converted(&fresh));
        assert_eq!(
            index(&sandbox),
            "version = 1\nactive = \"dev\"\n\n[tenants.dev]\ntsg_id = \"100\"\n",
            "the pin follows the replacement; the selection is imported"
        );
        assert!(
            out.stderr
                .contains("  Imported 0, replaced 1, skipped 0 into")
        );
    }

    #[test]
    fn names_differing_only_in_case_are_skipped_even_with_replace() {
        let sandbox = Sandbox::new();
        ts_tenants(
            &sandbox,
            &[
                ("Dev", "100", standard("100")),
                ("dev", "200", standard("200")),
            ],
            None,
        );
        sandbox.write_rust_tenant("DEV", &standard("300"));
        sandbox.write_store_index(&[("DEV", "300")], None);
        for args in [&[][..], &["--replace"]] {
            let (out, rows) = import_json(&sandbox, args);
            out.assert_success();
            let note = "conflicts with existing tenant DEV; names may not differ only in case";
            assert_eq!(
                rows,
                json!([
                    row("Dev", "100", "skip", &[note]),
                    row("dev", "200", "skip", &[note])
                ])
            );
        }
        // Without a Rust twin, the first in byte order is imported and the other skipped.
        let sandbox = Sandbox::new();
        ts_tenants(
            &sandbox,
            &[
                ("dev", "200", standard("200")),
                ("Dev", "100", standard("100")),
            ],
            None,
        );
        let (out, rows) = import_json(&sandbox, &[]);
        out.assert_success();
        assert_eq!(
            rows,
            json!([
                row("Dev", "100", "import", &STANDARD_NOTES),
                row(
                    "dev",
                    "200",
                    "skip",
                    &["conflicts with existing tenant Dev; names may not differ only in case"]
                ),
            ])
        );
        assert_eq!(listing(&sandbox.store_dir().join("tenants")), ["Dev.toml"]);
    }

    #[test]
    fn dry_run_reports_the_same_plan_and_writes_nothing() {
        let sandbox = Sandbox::new();
        ts_tenants(
            &sandbox,
            &[
                ("dev", "100", standard("100")),
                ("prod", "200", standard("200")),
            ],
            Some("prod"),
        );
        let before = ts_fingerprint(&sandbox);
        let out = rs(
            &sandbox,
            &["tenant", "import", "--dry-run", "--output", "json"],
            RunOptions::new(),
        );
        out.assert_success();
        assert_eq!(
            listing(&sandbox.store_dir()),
            Vec::<String>::new(),
            "no directory, lock or temporary file"
        );
        assert_eq!(
            out.stderr,
            "  Dry run: would import 2, replace 0, skip 0; nothing was written\n  Would select prod (selected in the TypeScript registry)\n"
        );
        let dry = out.json();
        // The store directory itself may be missing too.
        fs::remove_dir(sandbox.store_dir()).unwrap();
        let out = rs(
            &sandbox,
            &["tenant", "import", "--dry-run"],
            RunOptions::new(),
        );
        out.assert_success();
        assert!(!sandbox.store_dir().exists(), "the store is not created");
        let (out, rows) = import_json(&sandbox, &[]);
        out.assert_success();
        assert_eq!(rows, dry, "the same rows with and without --dry-run");
        assert_ts_untouched(&sandbox, &before);
    }

    #[test]
    fn the_selection_is_imported_only_into_a_store_without_one() {
        // No Rust selection, the TS active row imported: selected.
        let sandbox = Sandbox::new();
        ts_tenants(
            &sandbox,
            &[
                ("dev", "100", standard("100")),
                ("prod", "200", standard("200")),
            ],
            Some("prod"),
        );
        let out = import(&sandbox, &[]);
        out.assert_success();
        assert!(
            out.stderr
                .ends_with("  Selected prod (selected in the TypeScript registry)\n")
        );
        assert!(index(&sandbox).starts_with("version = 1\nactive = \"prod\"\n"));
        // A Rust selection exists: kept.
        let sandbox = Sandbox::new();
        ts_tenants(&sandbox, &[("dev", "100", standard("100"))], Some("dev"));
        sandbox.write_rust_tenant("mine", &standard("300"));
        sandbox.write_store_index(&[("mine", "300")], Some("mine"));
        let out = import(&sandbox, &[]);
        out.assert_success();
        assert!(!out.stderr.contains("Selected"), "{}", out.stderr);
        assert!(index(&sandbox).starts_with("version = 1\nactive = \"mine\"\n"));
        // The TS active row is skipped: nothing is selected.
        let sandbox = Sandbox::new();
        ts_tenants(
            &sandbox,
            &[
                ("dev", "100", standard("100")),
                ("prod", "200", standard("200")),
            ],
            Some("prod"),
        );
        sandbox.write_rust_tenant("prod", &standard("200"));
        sandbox.write_store_index(&[("prod", "200")], None);
        let out = import(&sandbox, &[]);
        out.assert_success();
        assert!(!out.stderr.contains("Selected"), "{}", out.stderr);
        assert!(index(&sandbox).starts_with("version = 1\n\n"));
        // With --replace the active row is written, so it is selected.
        let out = import(&sandbox, &["--replace"]);
        out.assert_success();
        assert!(
            out.stderr.contains("  Selected prod (selected"),
            "{}",
            out.stderr
        );
    }

    #[test]
    fn an_orphan_tenant_file_is_replaced() {
        let sandbox = Sandbox::new();
        ts_tenants(&sandbox, &[("dev", "100", standard("100"))], None);
        sandbox.write_tenant_file_raw("dev", b"invalid = [\n", 0o600);
        let out = import(&sandbox, &[]);
        out.assert_success();
        assert_eq!(tenant_file(&sandbox, "dev"), converted(&standard("100")));
    }

    #[test]
    fn files_are_private_and_unsafe_targets_write_nothing() {
        let sandbox = Sandbox::new();
        ts_tenants(
            &sandbox,
            &[
                ("dev", "100", standard("100")),
                ("prod", "200", standard("200")),
            ],
            None,
        );
        fs::remove_dir(sandbox.store_dir()).unwrap();
        import(&sandbox, &[]).assert_success();
        assert_eq!(mode(&sandbox.store_dir()), 0o700);
        assert_eq!(mode(&sandbox.store_dir().join("tenants")), 0o700);
        assert_eq!(mode(&sandbox.store_index_path()), 0o600);
        assert_eq!(mode(&sandbox.store_tenant_path("dev")), 0o600);
        assert_eq!(mode(&sandbox.store_tenant_path("prod")), 0o600);
        // A read-only replace target: refused before anything is written.
        let prod = sandbox.store_tenant_path("prod");
        fs::set_permissions(&prod, fs::Permissions::from_mode(0o400)).unwrap();
        let index_before = index(&sandbox);
        let dev_before = tenant_file(&sandbox, "dev");
        let out = import(&sandbox, &["--replace"]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            op_error(&format!(
                "{} is read-only; restore write permission (chmod 600) to change it",
                prod.display()
            ))
        );
        assert_eq!(index(&sandbox), index_before);
        assert_eq!(tenant_file(&sandbox, "dev"), dev_before);
        // A symlinked replace target is refused too.
        fs::remove_file(&prod).unwrap();
        symlink(sandbox.store_tenant_path("dev"), &prod).unwrap();
        let out = import(&sandbox, &["--replace"]);
        out.assert_code(1);
        assert!(
            out.stderr.contains(&format!(
                "Refusing to use {}: it must be a regular file",
                prod.display()
            )),
            "{}",
            out.stderr
        );
        assert_eq!(tenant_file(&sandbox, "dev"), dev_before);
    }

    #[test]
    fn an_import_waits_for_the_store_lock() {
        let sandbox = Sandbox::new();
        ts_tenants(&sandbox, &[("dev", "100", standard("100"))], None);
        let holder = lock_holder::spawn(
            "store_rules::lock_holder_helper",
            &sandbox.store_dir(),
            Duration::from_secs(1),
        );
        // A dry run takes no lock, so it does not wait.
        let dry = rs(&sandbox, &["tenant", "import", "--dry-run"], writes());
        dry.assert_success();
        assert!(!dry.stderr.contains("Waiting"), "{}", dry.stderr);
        let out = import(&sandbox, &[]);
        out.assert_success();
        assert!(
            out.stderr.starts_with(&format!(
                "  Waiting for another airs change to finish ({}",
                holder.holder_prefix()
            )),
            "{}",
            out.stderr
        );
        holder.wait();
        assert!(sandbox.store_tenant_path("dev").exists());
    }

    /// Not a test: the lock-holder process `an_import_waits_for_the_store_lock` starts.
    #[test]
    #[ignore = "helper process for the lock test"]
    fn lock_holder_helper() {
        lock_holder::run_from_env();
    }
}

mod from_config {
    use super::*;

    #[test]
    fn a_config_file_is_copied_without_management_credentials() {
        let sandbox = Sandbox::new();
        let scanner_only = standard("100")
            .without("mgmtClientId")
            .without("mgmtClientSecret");
        let path = sandbox.write_fixture_config("scanner", &scanner_only);
        // A TS active tenant exists, the Rust store has no selection: still not selected.
        sandbox.write_registry(&[RegistryEntry::new("other", &path, "100")], Some("other"));
        sandbox.ts_only();
        let before = ts_fingerprint(&sandbox);
        let (out, rows) = import_json(
            &sandbox,
            &[
                "--from-config",
                &path.display().to_string(),
                "--name",
                "scan",
            ],
        );
        out.assert_success();
        assert_eq!(rows, json!([row("scan", "100", "import", &STANDARD_NOTES)]));
        assert!(
            !out.stderr.contains("Selected"),
            "--from-config never selects"
        );
        assert_eq!(
            index(&sandbox),
            "version = 1\n\n[tenants.scan]\ntsg_id = \"100\"\n"
        );
        assert_eq!(tenant_file(&sandbox, "scan"), converted(&scanner_only));
        // Relative to the current directory, and the existing name rules apply.
        let relative = format!("../configs/{}", path.file_name().unwrap().to_string_lossy());
        let (out, rows) = import_json(&sandbox, &["--from-config", &relative, "--name", "scan"]);
        out.assert_success();
        assert_eq!(rows, json!([row("scan", "100", "skip", &[EXISTS_NOTE])]));
        let (out, rows) = import_json(
            &sandbox,
            &["--from-config", &relative, "--name", "scan", "--replace"],
        );
        out.assert_success();
        assert_eq!(rows[0]["action"], "replace");
        assert_ts_untouched(&sandbox, &before);
    }

    #[test]
    fn file_errors_in_typescript_order() {
        let sandbox = Sandbox::new();
        sandbox.ts_only();
        let missing = sandbox.configs_dir().join("missing.json");
        let out = import(
            &sandbox,
            &[
                "--from-config",
                &missing.display().to_string(),
                "--name",
                "dev",
            ],
        );
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            op_error("Tenant config file does not exist or cannot be resolved")
        );
        assert_eq!(out.stdout, "");
        let real = sandbox.write_config_raw("bad", br#"{"mgmtClientSecret":"FAKE-S2",oops"#);
        let link = sandbox.configs_dir().join("link.json");
        symlink(&real, &link).unwrap();
        let out = import(
            &sandbox,
            &[
                "--from-config",
                &link.display().to_string(),
                "--name",
                "dev",
            ],
        );
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            op_error(&format!(
                "Cannot read a valid tenant config at {}",
                real.display()
            )),
            "the canonical path"
        );
        for (name, config) in [
            ("notsg", standard("1").without("mgmtTsgId")),
            ("blank", standard("1").with("mgmtTsgId", " ")),
            ("empty", standard("1").with("mgmtTsgId", "")),
        ] {
            let path = sandbox.write_fixture_config(name, &config);
            let out = import(
                &sandbox,
                &[
                    "--from-config",
                    &path.display().to_string(),
                    "--name",
                    "dev",
                ],
            );
            out.assert_code(1);
            assert_eq!(
                out.stderr,
                op_error(&format!(
                    "{} has no mgmtTsgId; it cannot be imported",
                    path.display()
                ))
            );
        }
        let numeric =
            sandbox.write_fixture_config("numeric", &standard("1").with("mgmtTsgId", 100));
        let out = import(
            &sandbox,
            &[
                "--from-config",
                &numeric.display().to_string(),
                "--name",
                "dev",
            ],
        );
        out.assert_code(1);
        assert!(
            out.stderr.contains("Cannot read a valid tenant config at"),
            "{}",
            out.stderr
        );
        assert_eq!(listing(&sandbox.store_dir()), Vec::<String>::new());
    }
}

mod report {
    use super::*;

    fn two(sandbox: &Sandbox) {
        ts_tenants(
            sandbox,
            &[
                ("dev", "100", standard("100")),
                ("prod", "200", standard("200")),
            ],
            Some("dev"),
        );
        sandbox.write_rust_tenant("prod", &standard("200"));
        sandbox.write_store_index(&[("prod", "200")], None);
    }

    #[test]
    fn pretty_and_table_are_the_same_table() {
        let notes = STANDARD_NOTES.join("; ");
        let expected = |status: &str| {
            let rows = [
                ["Tenant", "TSG ID", "Action", "Notes"],
                ["dev", "100", "import", notes.as_str()],
                ["prod", "200", "skip", EXISTS_NOTE],
            ];
            let widths: Vec<usize> = (0..4)
                .map(|column| {
                    rows.iter()
                        .map(|row| row[column].chars().count())
                        .max()
                        .unwrap()
                })
                .collect();
            let line = |row: &[&str; 4]| {
                row.iter()
                    .zip(&widths)
                    .map(|(cell, width)| format!(" {cell:<width$} "))
                    .collect::<Vec<_>>()
                    .join("│")
            };
            let rule = widths
                .iter()
                .map(|width| "─".repeat(width + 2))
                .collect::<Vec<_>>()
                .join("┼");
            (
                format!(
                    "{}\n{rule}\n{}\n{}\n",
                    line(&rows[0]),
                    line(&rows[1]),
                    line(&rows[2])
                ),
                status.to_owned(),
            )
        };
        for format in [None, Some("pretty"), Some("table")] {
            let sandbox = Sandbox::new();
            two(&sandbox);
            let mut args = vec!["tenant", "import"];
            if let Some(format) = format {
                args.extend(["--output", format]);
            }
            let out = rs(&sandbox, &args, writes());
            out.assert_success();
            let (stdout, _) = expected("");
            assert_eq!(out.stdout, stdout, "{format:?}");
            assert_eq!(
                out.stderr,
                format!(
                    "  Imported 1, replaced 0, skipped 1 into {}\n  Selected dev (selected in the TypeScript registry)\n",
                    sandbox.store_dir().display()
                )
            );
        }
    }

    #[test]
    fn quiet_hides_the_status_lines_not_the_data_or_the_error() {
        let sandbox = Sandbox::new();
        two(&sandbox);
        let out = rs(
            &sandbox,
            &["--quiet", "tenant", "import", "--output", "json"],
            writes(),
        );
        out.assert_success();
        assert_eq!(out.stderr, "");
        assert_eq!(out.json().as_array().unwrap().len(), 2);
        let sandbox = Sandbox::new();
        let missing = sandbox.configs_dir().join("missing.json");
        sandbox.write_registry(&[RegistryEntry::new("dev", &missing, "1")], None);
        sandbox.ts_only();
        let out = rs(&sandbox, &["--quiet", "tenant", "import"], writes());
        out.assert_code(1);
        assert_eq!(out.stderr, op_error("1 tenant could not be imported"));
        assert!(
            out.stdout
                .contains(" dev    │ 1      │ skip   │ cannot read"),
            "{}",
            out.stdout
        );
    }

    #[test]
    fn an_empty_registry_follows_the_empty_list_rules() {
        for (format, stdout) in [
            ("pretty", "  No tenants found\n"),
            ("table", "\n"),
            ("json", "[]\n"),
        ] {
            let sandbox = Sandbox::new();
            sandbox.write_registry(&[], None);
            sandbox.ts_only();
            let out = import(&sandbox, &["--output", format]);
            out.assert_success();
            assert_eq!(out.stdout, stdout, "{format}");
            assert!(
                out.stderr
                    .starts_with("  Imported 0, replaced 0, skipped 0 into ")
            );
            assert_eq!(
                listing(&sandbox.store_dir()),
                Vec::<String>::new(),
                "nothing to write"
            );
        }
    }

    #[test]
    fn the_tenant_default_output_is_ignored() {
        let sandbox = Sandbox::new();
        ts_tenants(
            &sandbox,
            &[("dev", "100", standard("100").with("defaultOutput", "json"))],
            Some("dev"),
        );
        import(&sandbox, &[]).assert_success();
        let out = import(&sandbox, &["--replace"]);
        out.assert_success();
        assert!(out.stdout.starts_with(" Tenant │"), "{}", out.stdout);
    }

    #[test]
    fn control_characters_in_file_text_are_replaced_in_pretty_and_table() {
        let sandbox = Sandbox::new();
        let tsg = "1\u{1b}[31m";
        let config = standard("100")
            .with("mgmtTsgId", tsg)
            .with("bad\u{7}key", 1);
        ts_tenants(&sandbox, &[("dev", tsg, config)], None);
        let out = rs(
            &sandbox,
            &["tenant", "import", "--dry-run"],
            RunOptions::new(),
        );
        out.assert_success();
        assert!(
            !out.stdout.contains('\u{1b}') && !out.stdout.contains('\u{7}'),
            "{:?}",
            out.stdout
        );
        assert!(out.stdout.contains("1\u{fffd}[31m"), "{:?}", out.stdout);
        assert!(
            out.stdout.contains("ignored unknown key bad\u{fffd}key"),
            "{:?}",
            out.stdout
        );
        let out = rs(
            &sandbox,
            &["tenant", "import", "--dry-run", "--output", "json"],
            RunOptions::new(),
        );
        assert_eq!(out.json()[0]["tsgId"], tsg, "JSON keeps the text, escaped");
        assert!(out.stdout.contains("\\u001b"), "{}", out.stdout);
        // Imported as it is (import applies the read rules only), escaped in the file.
        import(&sandbox, &[]).assert_success();
        assert!(
            index(&sandbox).contains("tsg_id = \"1\\u001B[31m\"\n"),
            "{}",
            index(&sandbox)
        );
    }

    #[test]
    fn no_secret_reaches_any_output() {
        let sandbox = Sandbox::new();
        let config = standard("100")
            .with("aiGwInferenceApiKey", "FAKE-GATEWAY-SECRET-1")
            .with("typesafeApiKey", "FAKE-TYPESAFE-SECRET-1")
            .with("airsApiToken", "FAKE-TOKEN-SECRET-1");
        ts_tenants(&sandbox, &[("dev", "100", config)], Some("dev"));
        for args in [
            &["--dry-run"][..],
            &["--dry-run", "--output", "table"],
            &["--dry-run", "--output", "json"],
            &[],
            &["--replace", "--output", "json"],
        ] {
            let out = import(&sandbox, args);
            out.assert_success();
            for secret in [
                "FAKE-GATEWAY-SECRET-1",
                "FAKE-TYPESAFE-SECRET-1",
                "FAKE-TOKEN-SECRET-1",
                "FAKE-SECRET-100",
                "FAKE-APIKEY-100",
            ] {
                assert!(
                    !out.stdout.contains(secret) && !out.stderr.contains(secret),
                    "{args:?}"
                );
            }
        }
    }
}

mod after_import {
    use super::*;

    #[test]
    fn every_reader_uses_the_imported_tenant() {
        let sandbox = Sandbox::new();
        ts_tenants(&sandbox, &[("dev", "100", standard("100"))], Some("dev"));
        // Before the import: the read commands point at it (D90), doctor too (D89).
        let out = rs(&sandbox, &["tenant", "list"], RunOptions::new());
        assert!(
            out.stderr.contains("run 'airs tenant import' to copy it"),
            "{}",
            out.stderr
        );
        import(&sandbox, &[]).assert_success();
        let out = rs(
            &sandbox,
            &["tenant", "list", "--output", "json"],
            RunOptions::new(),
        );
        out.assert_success();
        assert_eq!(
            out.json(),
            json!([{
                "name": "dev",
                "configPath": sandbox.store_tenant_path("dev").display().to_string(),
                "tsgId": "100",
                "active": true
            }])
        );
        let out = rs(
            &sandbox,
            &["tenant", "get", "dev", "gatewayDataEndpoint"],
            RunOptions::new(),
        );
        assert_eq!(out.stdout, format!("{BASE}/ai_gw/v2\n"));
        let out = rs(&sandbox, &["tenant", "path"], RunOptions::new());
        assert_eq!(
            out.stdout,
            format!("{}\n", sandbox.store_tenant_path("dev").display())
        );
    }

    #[test]
    fn imported_tenants_are_independent_copies() {
        let sandbox = Sandbox::new();
        let shared = sandbox.write_fixture_config("shared", &standard("100"));
        sandbox.write_registry(
            &[
                RegistryEntry::new("one", &shared, "100"),
                RegistryEntry::new("two", &shared, "100"),
            ],
            None,
        );
        sandbox.ts_only();
        import(&sandbox, &[]).assert_success();
        assert_eq!(tenant_file(&sandbox, "one"), tenant_file(&sandbox, "two"));
        // Changing one copy changes neither the other nor the TypeScript file.
        let before = ts_fingerprint(&sandbox);
        rs(
            &sandbox,
            &["tenant", "set", "one", "defaultOutput", "json"],
            writes(),
        )
        .assert_success();
        assert!(tenant_file(&sandbox, "one").contains("defaultOutput"));
        assert!(!tenant_file(&sandbox, "two").contains("defaultOutput"));
        assert_ts_untouched(&sandbox, &before);
    }
}

mod store {
    //! The harness provisioner (`airs_test_harness::store::provision`, used by every
    //! sandbox) and `tenant import` must produce byte-identical store files.
    use super::*;

    /// One corpus case: TypeScript configs (name, tsg in the registry, raw JSON) and the
    /// registry's active name.
    struct Case {
        label: &'static str,
        tenants: Vec<(&'static str, &'static str, Vec<u8>)>,
        active: Option<&'static str>,
    }

    fn json_of(config: &FixtureConfig) -> Vec<u8> {
        config.to_json().into_bytes()
    }

    fn corpus() -> Vec<Case> {
        vec![
            Case {
                label: "renamed and dropped keys",
                tenants: vec![("dev", "100", json_of(&standard("100")))],
                active: Some("dev"),
            },
            Case {
                label: "empty values, unknown keys and coercion",
                tenants: vec![(
                    "dev",
                    "100",
                    json_of(
                        &standard("100")
                            .with("airsApiToken", "")
                            .with("aiGwInferenceModel", "")
                            .with("airsNumRetries", "0x2")
                            .with("scanConcurrency", json!([4]))
                            .with("custom", "x")
                            .with("redTeamTokenEndpoint", "http://127.0.0.1:9/t"),
                    ),
                )],
                active: None,
            },
            Case {
                label: "dead defaultOutput",
                tenants: vec![
                    (
                        "csv",
                        "1",
                        json_of(&standard("1").with("defaultOutput", "csv")),
                    ),
                    (
                        "json",
                        "2",
                        json_of(&standard("2").with("defaultOutput", "json")),
                    ),
                ],
                active: Some("json"),
            },
            Case {
                label: "two names, one config",
                tenants: vec![
                    ("a", "100", json_of(&standard("100"))),
                    ("b", "100", json_of(&standard("100"))),
                ],
                active: Some("b"),
            },
            Case {
                label: "mismatched pin (skipped by import; provisioned with both TSGs)",
                tenants: vec![
                    ("ok", "100", json_of(&standard("100"))),
                    ("pin", "999", json_of(&standard("200"))),
                ],
                active: None,
            },
        ]
    }

    /// Every file under `store/` (relative path → bytes), without the lock file.
    fn store_files(sandbox: &Sandbox) -> BTreeMap<String, Vec<u8>> {
        listing(&sandbox.store_dir())
            .into_iter()
            .filter(|path| path != "config.lock")
            .filter_map(|path| {
                fs::read(sandbox.store_dir().join(&path))
                    .ok()
                    .map(|bytes| (path, bytes))
            })
            .collect()
    }

    #[test]
    fn provision_matches_import() {
        for case in corpus() {
            let sandbox = Sandbox::new();
            let entries: Vec<RegistryEntry> = case
                .tenants
                .iter()
                .map(|(name, tsg, bytes)| {
                    let value: Value = serde_json::from_slice(bytes).unwrap();
                    let path =
                        sandbox.write_fixture_config(name, &FixtureConfig::from_value(&value));
                    RegistryEntry::new(name, path, tsg)
                })
                .collect();
            // Mirroring provisions the store from the TypeScript files…
            sandbox.write_registry(&entries, case.active);
            let provisioned = store_files(&sandbox);
            // …and the importer must write the same bytes.
            sandbox.ts_only();
            let (out, rows) = import_json(&sandbox, &[]);
            let imported = store_files(&sandbox);
            let skipped: Vec<&str> = rows
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["action"] == "skip")
                .map(|row| row["name"].as_str().unwrap())
                .collect();
            if skipped.is_empty() {
                out.assert_success();
                assert_eq!(imported, provisioned, "{}", case.label);
                continue;
            }
            // A mismatched pin: the provisioner keeps both TSGs so both CLIs report the
            // pin; the importer skips the entry. Every imported tenant file still matches.
            out.assert_code(1);
            assert_eq!(skipped, ["pin"], "{}", case.label);
            for (path, bytes) in &imported {
                if path.starts_with("tenants/") {
                    assert_eq!(Some(bytes), provisioned.get(path), "{}: {path}", case.label);
                }
            }
            assert!(!imported.contains_key("tenants/pin.toml"));
            assert!(provisioned.contains_key("tenants/pin.toml"));
        }
    }
}
