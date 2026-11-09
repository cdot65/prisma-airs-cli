//! Executable acceptance tests for tenant loading and the read-only tenant commands
//! (DESIGN §3 registry/config/selection/credentials, §3.6 tenant commands, §4 environment,
//! §5 endpoint admission).
//!
//! Every invocation runs the real `airs-cli-rs` binary inside a harness sandbox whose
//! tenants point every endpoint key at the loopback fixture server. After every command
//! the sandbox tenant files must be byte-, mode- and mtime-identical, HOME and cwd must
//! not gain entries, and no request may reach an unknown fixture route. The runner's
//! secret-leak check stays on everywhere.

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use airs_test_harness::{
    AuthIdentity, Cli, FixtureConfig, FixtureServer, FixtureTenant, RecordedRequest, RegistryEntry,
    Route, Run, RunOptions, Sandbox, fixtures, is_secret_key, run,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------------------

/// The 27 airs-cli-rs schema keys in schema order (DESIGN §3.3).
const KEYS: [&str; 27] = airs_test_harness::store::RUST_KEYS;

const LIST: &[&str] = &["runtime", "profiles", "list", "--output", "json"];
const SCAN: &[&str] = &[
    "runtime",
    "scan",
    "--profile",
    "fixture-profile",
    "hello world",
];
const SCAN_BODY: &str =
    r#"{"ai_profile":{"profile_name":"fixture-profile"},"contents":[{"prompt":"hello world"}]}"#;

const PIN_MSG: &str =
    "Tenant config TSG differs from its registered identity; register it under a new name";
const NO_TENANT_NONE: &str =
    "No tenant selected. Run 'airs tenant create <name>' and then 'airs tenant switch <name>'";
const TENANT_NAME_USAGE: &str = "Tenant name must be 1–64 letters, digits, hyphens or underscores and start with a letter/digit";
const ENDPOINT_RULE: &str = "endpoints must use https (http is allowed only for loopback hosts) and must not contain credentials, a query or a fragment";
const ONE_MIB: usize = 1_048_576;

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

fn registry_msg(path: &Path) -> String {
    format!(
        "Cannot read tenant registry at {}; restore a valid registry before proceeding",
        path.display()
    )
}

fn config_msg(path: &Path) -> String {
    format!("Cannot read a valid tenant config at {}", path.display())
}

fn mgmt_missing(tenant: &str, keys: &str) -> String {
    format!(
        "Management credentials are not configured (missing {keys}). Run 'airs tenant set {tenant} <key>' for {keys} (secrets prompt hidden, or use --stdin)"
    )
}

fn scanner_missing(tenant: &str) -> String {
    format!(
        "Scanner credentials are not configured (airsApiKey or airsApiToken). Run 'airs tenant set {tenant} <key>' for airsApiKey (secrets prompt hidden, or use --stdin)"
    )
}

fn unknown_key_msg() -> String {
    format!(
        "Unknown configuration key. Supported keys: {}",
        KEYS.join(", ")
    )
}

/// Exit 1, empty stdout, stderr exactly `\n  ✗ Error: <msg>\n\n` (DESIGN §2.2).
fn assert_op_error(out: &Run, msg: &str) {
    assert_eq!(
        (out.code, out.stdout.as_str(), out.stderr.as_str()),
        (1, "", format!("\n  ✗ Error: {msg}\n\n").as_str()),
        "{} {:?}",
        out.cli,
        out.args
    );
}

/// Exit 2, empty stdout, stderr exactly `\n  ✗ <msg>\n\n` (DESIGN §2.2).
fn assert_usage(out: &Run, msg: &str) {
    assert_eq!(
        (out.code, out.stdout.as_str(), out.stderr.as_str()),
        (2, "", format!("\n  ✗ {msg}\n\n").as_str()),
        "{} {:?}",
        out.cli,
        out.args
    );
}

fn set_mode(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("chmod");
}

/// Removes whatever is at `path` (file, symlink or directory), making it writable first.
fn clear(path: &Path) {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.is_dir() {
            fs::remove_dir_all(path).expect("remove dir");
        } else {
            if meta.is_file() {
                set_mode(path, 0o600);
            }
            fs::remove_file(path).expect("remove file");
        }
    }
}

/// One server + sandbox; `run` enforces the read-only and loopback invariants.
struct Fx {
    server: FixtureServer,
    sandbox: Sandbox,
}

impl Fx {
    fn new() -> Self {
        Self {
            server: FixtureServer::start(),
            sandbox: Sandbox::new(),
        }
    }

    /// The common setup: tenant `prod` (TSG 100) registered and active.
    fn prod() -> (Self, FixtureTenant) {
        let fx = Self::new();
        let tenant = fx.sandbox.single_tenant(&fx.server, "prod", "100");
        fx.server
            .set_profiles("100", vec![fixtures::profile(1, "alpha", 1)]);
        (fx, tenant)
    }

    /// Writes a standard config for `name`/`tsg` edited by `edit` (server learns it).
    fn tenant_with(
        &self,
        name: &str,
        tsg: &str,
        edit: impl FnOnce(&mut FixtureConfig),
    ) -> FixtureTenant {
        self.sandbox
            .add_fixture_tenant_with(&self.server, name, tsg, edit)
    }

    /// Rust-only: like [`Fx::prod_with`], but the edit may point an endpoint away from
    /// loopback (harness opt-out; the TS CLI can no longer run in this sandbox).
    fn prod_with_external_endpoints(
        edit: impl FnOnce(&mut FixtureConfig),
    ) -> (Self, FixtureTenant) {
        let fx = Self::new();
        let tenant = fx
            .sandbox
            .add_fixture_tenant_allowing_external_endpoints(&fx.server, "prod", "100", edit);
        fx.sandbox.register(&[&tenant], Some("prod"));
        fx.server
            .set_profiles("100", vec![fixtures::profile(1, "alpha", 1)]);
        (fx, tenant)
    }

    /// `prod`/100 active with an edited standard config.
    fn prod_with(edit: impl FnOnce(&mut FixtureConfig)) -> (Self, FixtureTenant) {
        let fx = Self::new();
        let tenant = fx.tenant_with("prod", "100", edit);
        fx.sandbox.register(&[&tenant], Some("prod"));
        fx.server
            .set_profiles("100", vec![fixtures::profile(1, "alpha", 1)]);
        (fx, tenant)
    }

    fn run(&self, args: &[&str]) -> Run {
        self.run_with(args, RunOptions::new())
    }

    fn run_with(&self, args: &[&str], options: RunOptions) -> Run {
        let before = self.sandbox.snapshot();
        let home = self.sandbox.home_listing();
        let cwd = self.sandbox.cwd_listing();
        let out = run(&rust(), &self.sandbox, args, &options);
        self.sandbox.assert_unchanged(&before);
        assert_eq!(self.sandbox.home_listing(), home, "CLI changed HOME");
        assert_eq!(self.sandbox.cwd_listing(), cwd, "CLI changed cwd");
        self.server.assert_no_unknown_routes();
        out
    }

    fn assert_no_requests(&self) {
        let lines: Vec<String> = self
            .server
            .requests()
            .iter()
            .map(RecordedRequest::line)
            .collect();
        assert!(lines.is_empty(), "unexpected requests: {lines:?}");
    }

    /// Exactly `[Token, Profiles]` for `tsg`, carrying only that tenant's identities.
    fn assert_mgmt_requests_for(&self, tsg: &str) {
        let requests = self.server.requests();
        assert_eq!(
            requests.iter().map(|r| r.route).collect::<Vec<_>>(),
            vec![Route::Token, Route::Profiles],
            "{requests:?}"
        );
        assert_eq!(
            requests[0].auth,
            AuthIdentity::Basic {
                client_id: format!("client-{tsg}"),
                tsg: Some(tsg.to_owned()),
                valid: true,
            }
        );
        assert_eq!(
            requests[0].form(),
            vec![
                ("grant_type".to_owned(), "client_credentials".to_owned()),
                ("scope".to_owned(), format!("tsg_id:{tsg}")),
            ]
        );
        assert_eq!(
            requests[1].auth,
            AuthIdentity::Bearer {
                tsg: Some(tsg.to_owned())
            }
        );
        assert_eq!(requests[1].path_tsg.as_deref(), Some(tsg));
        assert_eq!(
            requests[1].header("authorization"),
            Some(format!("Bearer mock-token-{tsg}").as_str())
        );
        assert_eq!(
            requests[1].line(),
            format!("GET /aisec/v1/mgmt/profiles/tsg/{tsg}?offset=0&limit=100&latest=true")
        );
        for request in &requests {
            assert!(
                request.header("x-pan-token").is_none()
                    && request.header("x-payload-hash").is_none(),
                "management request carried scanner credentials: {request:?}"
            );
        }
    }

    /// Exactly one sync scan POST with `tsg`'s API key (no bearer).
    fn assert_scan_request_for(&self, tsg: &str) {
        let requests = self.server.requests();
        assert_eq!(
            requests.iter().map(|r| r.route).collect::<Vec<_>>(),
            vec![Route::ScanSync],
            "{requests:?}"
        );
        assert_eq!(
            requests[0].auth,
            AuthIdentity::ApiKey {
                key_id: Some(tsg.to_owned()),
                hash_valid: true
            }
        );
        assert!(requests[0].header("authorization").is_none());
        assert_eq!(requests[0].body_text(), SCAN_BODY);
    }
}

fn entry(name: &str, path: &Path, tsg: &str) -> RegistryEntry {
    RegistryEntry::new(name, path, tsg)
}

/// The exact `[{key, value}]` 2-space JSON of `tenant read` / `tenant get --output json`.
fn rows_json(rows: &[(&str, Value)]) -> String {
    if rows.is_empty() {
        return "[]\n".into();
    }
    let items: Vec<String> = rows
        .iter()
        .map(|(key, value)| {
            format!(
                "  {{\n    \"key\": {},\n    \"value\": {}\n  }}",
                Value::String((*key).into()),
                serde_json::to_string(value).expect("value")
            )
        })
        .collect();
    format!("[\n{}\n]\n", items.join(",\n"))
}

/// The exact `tenant list` JSON for `(name, configPath, tsgId, active)` rows.
fn list_json(rows: &[(&str, &Path, &str, bool)]) -> String {
    if rows.is_empty() {
        return "[]\n".into();
    }
    let items: Vec<String> = rows
        .iter()
        .map(|(name, path, tsg, active)| {
            format!(
                "  {{\n    \"name\": {},\n    \"configPath\": {},\n    \"tsgId\": {},\n    \"active\": {active}\n  }}",
                Value::String((*name).into()),
                Value::String(path.display().to_string()),
                Value::String((*tsg).into()),
            )
        })
        .collect();
    format!("[\n{}\n]\n", items.join(",\n"))
}

/// The `tenant read` rows DESIGN §3.6 expects for the TypeScript fixture `config` as
/// the harness provisions it (the import conversion): present schema keys under their
/// Rust names in schema order, `""` dropped, numbers coerced, plus the defaulted
/// `scanConcurrency`; non-empty secrets redacted.
fn expected_read_rows(config: &FixtureConfig) -> Vec<(&'static str, Value)> {
    let converted = airs_test_harness::store::convert_config(config.to_json().as_bytes())
        .expect("a config TypeScript accepts");
    KEYS.iter()
        .filter_map(|key| {
            let value = match (converted.values.iter().find(|(name, _)| name == key), *key) {
                (Some((_, airs_test_harness::store::StoreValue::Text(text))), _) => json!(text),
                (Some((_, airs_test_harness::store::StoreValue::Integer(number))), _) => {
                    json!(number)
                }
                (None, "scanConcurrency") => json!(5),
                (None, _) => return None,
            };
            let value = match value.as_str() {
                Some(text) if is_secret_key(key) && !text.is_empty() => json!("[REDACTED]"),
                _ => value,
            };
            Some((*key, value))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// §3.1 Store location
// ---------------------------------------------------------------------------------------
mod store_path {
    use super::*;

    fn store_env(pairs: &[(&str, &str)], remove: &[&str]) -> RunOptions {
        let mut options = RunOptions::new();
        for (name, value) in pairs {
            options = options.env(name, *value);
        }
        for name in remove {
            options = options.without_env(name);
        }
        options
    }

    /// Writes a corrupt `config.toml` in `dir` so the error reveals the resolved `D`.
    fn corrupt_index_in(dir: &Path) {
        fs::create_dir_all(dir).expect("mkdir");
        set_mode(dir, 0o700);
        let path = dir.join("config.toml");
        fs::write(&path, b"version = ").expect("write");
        set_mode(&path, 0o600);
    }

    #[test]
    fn config_dir_is_absolute_and_lexically_normalised() {
        let (fx, _) = Fx::prod();
        let target = fx.sandbox.tmp().join("cfg");
        corrupt_index_in(&target);
        let dotted = format!("{}/./x/../cfg//", fx.sandbox.tmp().display());
        let out = fx.run_with(
            &["tenant", "list", "--output", "json"],
            store_env(&[("AIRS_CLI_CONFIG_DIR", &dotted)], &[]),
        );
        assert_op_error(&out, &registry_msg(&target.join("config.toml")));
        fx.assert_no_requests();
    }

    #[test]
    fn relative_values_are_refused_with_zero_requests() {
        let (fx, _) = Fx::prod();
        for (name, message) in [
            (
                "AIRS_CLI_CONFIG_DIR",
                "AIRS_CLI_CONFIG_DIR must be an absolute path",
            ),
            (
                "XDG_CONFIG_HOME",
                "XDG_CONFIG_HOME must be an absolute path",
            ),
        ] {
            let remove: &[&str] = if name == "XDG_CONFIG_HOME" {
                &["AIRS_CLI_CONFIG_DIR"]
            } else {
                &[]
            };
            for args in [LIST, &["tenant", "list"][..], &["tenant", "read"]] {
                let out = fx.run_with(args, store_env(&[(name, "relative/dir")], remove));
                assert_op_error(&out, message);
            }
            let out = fx.run_with(SCAN, store_env(&[(name, "~/x")], remove));
            assert_op_error(&out, message);
        }
        fx.assert_no_requests();
    }

    #[test]
    fn xdg_config_home_and_home_defaults() {
        let (fx, tenant) = Fx::prod();
        let xdg = fx.sandbox.tmp().join("xdg");
        corrupt_index_in(&xdg.join("airs-cli"));
        let out = fx.run_with(
            &["tenant", "list"],
            store_env(
                &[("XDG_CONFIG_HOME", &xdg.display().to_string())],
                &["AIRS_CLI_CONFIG_DIR"],
            ),
        );
        assert_op_error(&out, &registry_msg(&xdg.join("airs-cli/config.toml")));
        // An empty AIRS_CLI_CONFIG_DIR is unset: XDG_CONFIG_HOME applies.
        let out = fx.run_with(
            &["tenant", "list"],
            store_env(
                &[
                    ("AIRS_CLI_CONFIG_DIR", ""),
                    ("XDG_CONFIG_HOME", &xdg.display().to_string()),
                ],
                &[],
            ),
        );
        assert_op_error(&out, &registry_msg(&xdg.join("airs-cli/config.toml")));
        // Neither: HOME/.config/airs-cli (an empty store there; nothing is created).
        let out = fx.run_with(
            &["tenant", "list", "--output", "json"],
            store_env(&[], &["AIRS_CLI_CONFIG_DIR"]),
        );
        assert_eq!((out.code, out.stdout.as_str()), (0, "[]\n"));
        // The TypeScript registry variable no longer selects tenants (D70).
        let out = fx.run_with(
            &["tenant", "list", "--output", "json"],
            store_env(
                &[(
                    "PRISMA_AIRS_TENANTS_PATH",
                    &fx.sandbox.tmp().join("nope.json").display().to_string(),
                )],
                &[],
            ),
        );
        assert_eq!(
            out.stdout,
            list_json(&[("prod", &tenant.store_path, "100", true)])
        );
        fx.assert_no_requests();
    }

    #[test]
    fn an_empty_or_relative_home_without_store_variables_is_refused() {
        let (fx, _) = Fx::prod();
        for home in ["", "rel"] {
            for args in [
                LIST,
                &["tenant", "list"][..],
                &["doctor", "--output", "json"],
            ] {
                let out = fx.run_with(args, store_env(&[("HOME", home)], &["AIRS_CLI_CONFIG_DIR"]));
                let message =
                    "HOME is not an absolute path; set AIRS_CLI_CONFIG_DIR or XDG_CONFIG_HOME";
                if args[0] == "doctor" {
                    assert_eq!(out.code, 1);
                    // The first check is Executable (DESIGN §21.15); the Tenant check
                    // follows.
                    assert_eq!(out.json()[1]["name"], "Tenant");
                    assert_eq!(out.json()[1]["detail"], message);
                    assert_eq!(
                        out.json()[1]["hint"],
                        "Set AIRS_CLI_CONFIG_DIR or XDG_CONFIG_HOME to an absolute path"
                    );
                } else {
                    assert_op_error(&out, message);
                }
            }
        }
        fx.assert_no_requests();
    }

    #[test]
    fn a_missing_store_directory_is_an_empty_store_and_nothing_is_created() {
        let fx = Fx::new();
        for dir in [
            fx.sandbox.tmp().join("absent"),
            fx.sandbox.tmp().join("absent/deeper/still"),
        ] {
            let options = store_env(&[("AIRS_CLI_CONFIG_DIR", &dir.display().to_string())], &[]);
            let out = fx.run_with(&["tenant", "list", "--output", "json"], options.clone());
            assert_eq!((out.code, out.stdout.as_str()), (0, "[]\n"));
            assert_op_error(&fx.run_with(LIST, options), NO_TENANT_NONE);
            assert!(!dir.exists(), "readers never create the store");
        }
        fx.assert_no_requests();
    }
}

// ---------------------------------------------------------------------------------------
// §3.2 config.toml
// ---------------------------------------------------------------------------------------
mod index_file {
    use super::*;

    #[test]
    fn every_rejection_case_prints_the_generic_message_with_zero_requests() {
        let (fx, _) = Fx::prod();
        let index = fx.sandbox.store_index_path();
        let tenant = "[tenants.prod]\ntsg_id = \"100\"\n";
        let thousand_and_one: String = std::iter::once("version = 1\n".to_owned())
            .chain((0..1001).map(|i| format!("[tenants.t{i}]\ntsg_id = \"1\"\n")))
            .collect();
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("empty", Vec::new()),
            ("syntax", b"version = ".to_vec()),
            ("invalid utf-8", b"version = 1\nx = \"\xff\"\n".to_vec()),
            ("no version", tenant.as_bytes().to_vec()),
            ("version 2", format!("version = 2\n{tenant}").into_bytes()),
            (
                "string version",
                format!("version = \"1\"\n{tenant}").into_bytes(),
            ),
            (
                "unknown key",
                format!("version = 1\nextra = 1\n{tenant}").into_bytes(),
            ),
            (
                "ghost active",
                format!("version = 1\nactive = \"ghost\"\n{tenant}").into_bytes(),
            ),
            (
                "tsg number",
                b"version = 1\n[tenants.prod]\ntsg_id = 100\n".to_vec(),
            ),
            (
                "empty tsg",
                b"version = 1\n[tenants.prod]\ntsg_id = \"\"\n".to_vec(),
            ),
            (
                "extra field",
                format!("version = 1\n{tenant}x = 1\n").into_bytes(),
            ),
            (
                "bad name",
                b"version = 1\n[tenants.\"-bad\"]\ntsg_id = \"1\"\n".to_vec(),
            ),
            (
                "case collision (D85)",
                format!("version = 1\n{tenant}[tenants.PROD]\ntsg_id = \"1\"\n").into_bytes(),
            ),
            ("duplicate key", b"version = 1\nversion = 1\n".to_vec()),
            ("too many tenants", thousand_and_one.into_bytes()),
            (
                "secret-bearing garbage",
                b"version = 1\nkey = \"SUPER-SECRET-VALUE\" oops\n".to_vec(),
            ),
        ];
        for (label, bytes) in cases {
            fx.sandbox.write_store_raw(&bytes, 0o600);
            for args in [
                LIST,
                SCAN,
                &["tenant", "list", "--output", "json"][..],
                &["tenant", "read"],
                &["tenant", "path", "prod"],
                &["tenant", "get", "prod", "mgmtTsgId"],
                &["--tenant", "prod", "runtime", "profiles", "list"],
            ] {
                let out = fx.run(args);
                assert_eq!(
                    (out.code, out.stderr.as_str()),
                    (
                        1,
                        format!("\n  ✗ Error: {}\n\n", registry_msg(&index)).as_str()
                    ),
                    "{label}: {args:?}"
                );
                assert!(!out.stderr.contains("SUPER-SECRET"));
            }
        }
        fx.assert_no_requests();
    }

    #[test]
    fn exactly_one_mib_and_one_thousand_tenants_are_accepted() {
        let (fx, _) = Fx::prod();
        let mut bytes =
            b"version = 1\nactive = \"prod\"\n[tenants.prod]\ntsg_id = \"100\"\n".to_vec();
        bytes.resize(ONE_MIB, b'\n');
        fx.sandbox.write_store_raw(&bytes, 0o600);
        fx.run(LIST).assert_success();
        bytes.push(b'\n');
        fx.sandbox.write_store_raw(&bytes, 0o600);
        assert_op_error(&fx.run(LIST), &registry_msg(&fx.sandbox.store_index_path()));
        let pins: Vec<(String, String)> = (0..1000)
            .map(|i| (format!("t{i:04}"), "1".to_owned()))
            .collect();
        let refs: Vec<(&str, &str)> = pins
            .iter()
            .map(|(name, tsg)| (name.as_str(), tsg.as_str()))
            .collect();
        fx.sandbox.write_store_index(&refs, None);
        let out = fx.run(&["tenant", "list", "--output", "json"]);
        out.assert_success();
        assert_eq!(out.json().as_array().map(Vec::len), Some(1000));
        assert_eq!(out.json()[0]["name"], "t0000", "byte order (D86)");
        assert_eq!(out.json()[0]["active"], false);
    }

    #[test]
    fn unsafe_files_and_directories_are_refused() {
        let (fx, _) = Fx::prod();
        let index = fx.sandbox.store_index_path();
        let file_msg = format!(
            "Refusing to use {}: it must be a regular file (not a symlink) owned by the current user with no group or other permissions (chmod 600)",
            index.display()
        );
        let good = fs::read(&index).expect("index");
        for mode in [0o640, 0o604, 0o644, 0o660] {
            fx.sandbox.write_store_raw(&good, mode);
            assert_op_error(&fx.run(LIST), &file_msg);
            assert_op_error(&fx.run(&["tenant", "list"]), &file_msg);
        }
        fx.sandbox.write_store_raw(&good, 0o400);
        fx.run(LIST).assert_success();
        // A symlinked config.toml is refused, even to a valid private file.
        let elsewhere = fx.sandbox.write_file("tmp/index.toml", &good, 0o600);
        clear(&index);
        symlink(&elsewhere, &index).expect("symlink");
        assert_op_error(&fx.run(LIST), &file_msg);
        clear(&index);
        fs::create_dir(&index).expect("dir");
        assert_op_error(&fx.run(LIST), &file_msg);
        clear(&index);
        fx.sandbox.write_store_raw(&good, 0o600);
        fx.server.clear_requests();
        // D itself: group-writable, or a symlink.
        let store = fx.sandbox.store_dir();
        let dir_msg = |dir: &Path| {
            format!(
                "Refusing to use {}: it must be a directory (not a symlink) owned by the current user and not writable by group or others",
                dir.display()
            )
        };
        set_mode(&store, 0o770);
        assert_op_error(&fx.run(LIST), &dir_msg(&store));
        set_mode(&store, 0o755);
        fx.run(LIST).assert_success();
        set_mode(&store, 0o700);
        let link = fx.sandbox.tmp().join("store-link");
        symlink(&store, &link).expect("symlink");
        let out = fx.run_with(
            LIST,
            RunOptions::new().env("AIRS_CLI_CONFIG_DIR", link.display().to_string()),
        );
        assert_op_error(&out, &dir_msg(&link));
    }

    #[test]
    fn a_missing_config_toml_with_tenant_files_is_an_empty_store() {
        let (fx, tenant) = Fx::prod();
        fx.sandbox.detach_store();
        clear(&fx.sandbox.store_index_path());
        assert!(tenant.store_path.exists(), "an orphan tenant file");
        let out = fx.run(&["tenant", "list", "--output", "json"]);
        assert_eq!((out.code, out.stdout.as_str()), (0, "[]\n"));
        fx.assert_no_requests();
    }
}

// ---------------------------------------------------------------------------------------
// §3.3 Tenant files
// ---------------------------------------------------------------------------------------
mod tenant_file {
    use super::*;

    const COMMANDS: [&[&str]; 5] = [
        LIST,
        SCAN,
        &["tenant", "read", "--output", "json"],
        &["tenant", "get", "prod", "mgmtTsgId"],
        &["--tenant", "prod", "runtime", "topics", "list"],
    ];

    fn assert_generic(fx: &Fx, path: &Path, label: &str) {
        for args in COMMANDS {
            let out = fx.run(args);
            assert_eq!(
                (out.code, out.stdout.as_str(), out.stderr.as_str()),
                (
                    1,
                    "",
                    format!("\n  ✗ Error: {}\n\n", config_msg(path)).as_str()
                ),
                "{label}: {args:?}"
            );
        }
    }

    #[test]
    fn every_invalid_document_is_rejected_generically_with_zero_requests() {
        let (fx, tenant) = Fx::prod();
        let good = fs::read_to_string(&tenant.store_path).expect("tenant file");
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("syntax", format!("{good}nope").into_bytes()),
            (
                "duplicate key",
                format!("{good}mgmtTsgId = \"100\"\n").into_bytes(),
            ),
            (
                "unknown key (D72)",
                format!("{good}custom = \"x\"\n").into_bytes(),
            ),
            (
                "TypeScript name (D71)",
                format!("{good}aiGwInferenceModel = \"m\"\n").into_bytes(),
            ),
            (
                "dropped key (D71)",
                format!("{good}dataDir = \"/d\"\n").into_bytes(),
            ),
            (
                "retired key",
                format!("{good}redTeamTokenEndpoint = \"http://127.0.0.1:1/t\"\n").into_bytes(),
            ),
            (
                "empty string (D72)",
                format!("{good}airsApiToken = \"\"\n").into_bytes(),
            ),
            (
                "string integer (D72)",
                format!("{good}scanConcurrency = \"5\"\n").into_bytes(),
            ),
            (
                "float integer",
                format!("{good}scanConcurrency = 5.0\n").into_bytes(),
            ),
            (
                "boolean integer",
                format!("{good}scanConcurrency = true\n").into_bytes(),
            ),
            (
                "out of range",
                format!("{good}scanConcurrency = 21\n").into_bytes(),
            ),
            (
                "integer string",
                good.replace("mgmtTsgId = \"100\"", "mgmtTsgId = 100")
                    .into_bytes(),
            ),
            (
                "dead defaultOutput (D51)",
                format!("{good}defaultOutput = \"yaml\"\n").into_bytes(),
            ),
            (
                "unknown defaultOutput",
                format!("{good}defaultOutput = \"xml\"\n").into_bytes(),
            ),
            (
                "URL rule",
                good.lines()
                    .map(|line| {
                        if line.starts_with("mgmtDashboardEndpoint") {
                            "mgmtDashboardEndpoint = \"not a url\"\n".to_owned()
                        } else {
                            format!("{line}\n")
                        }
                    })
                    .collect::<String>()
                    .into_bytes(),
            ),
            (
                "secret-bearing garbage",
                format!("{good}mgmtClientSecret = \"SUPER-SECRET-VALUE\" oops\n").into_bytes(),
            ),
        ];
        for (label, bytes) in cases {
            fx.sandbox.write_tenant_file_raw("prod", &bytes, 0o600);
            assert_generic(&fx, &tenant.store_path, label);
        }
        let mut invalid_utf8 = good.clone().into_bytes();
        invalid_utf8.extend_from_slice(b"airsApiToken = \"\xff\"\n");
        fx.sandbox
            .write_tenant_file_raw("prod", &invalid_utf8, 0o600);
        assert_generic(&fx, &tenant.store_path, "invalid UTF-8");
        fx.assert_no_requests();
    }

    #[test]
    fn missing_files_and_directories_are_the_generic_message() {
        let (fx, tenant) = Fx::prod();
        clear(&tenant.store_path);
        fx.sandbox.detach_store();
        assert_generic(&fx, &tenant.store_path, "missing file");
        clear(&fx.sandbox.store_dir().join("tenants"));
        assert_generic(&fx, &tenant.store_path, "missing tenants/");
        fx.assert_no_requests();
    }

    #[test]
    fn unsafe_files_and_directories_are_refused() {
        let (fx, tenant) = Fx::prod();
        let good = fs::read(&tenant.store_path).expect("tenant file");
        let file_msg = format!(
            "Refusing to use {}: it must be a regular file (not a symlink) owned by the current user with no group or other permissions (chmod 600)",
            tenant.store_path.display()
        );
        for mode in [0o640, 0o604, 0o644, 0o610] {
            fx.sandbox.write_tenant_file_raw("prod", &good, mode);
            for args in COMMANDS {
                assert_op_error(&fx.run(args), &file_msg);
            }
        }
        fx.sandbox.write_tenant_file_raw("prod", &good, 0o400);
        fx.run(LIST).assert_success();
        let elsewhere = fx.sandbox.write_file("tmp/prod.toml", &good, 0o600);
        clear(&tenant.store_path);
        symlink(&elsewhere, &tenant.store_path).expect("symlink");
        for args in COMMANDS {
            assert_op_error(&fx.run(args), &file_msg);
        }
        clear(&tenant.store_path);
        fx.sandbox.write_tenant_file_raw("prod", &good, 0o600);
        let tenants = fx.sandbox.store_dir().join("tenants");
        set_mode(&tenants, 0o777);
        assert_op_error(
            &fx.run(LIST),
            &format!(
                "Refusing to use {}: it must be a directory (not a symlink) owned by the current user and not writable by group or others",
                tenants.display()
            ),
        );
        set_mode(&tenants, 0o700);
        fx.run(LIST).assert_success();
    }

    #[test]
    fn exactly_one_mib_is_accepted() {
        let (fx, tenant) = Fx::prod();
        let mut bytes = fs::read(&tenant.store_path).expect("tenant file");
        bytes.resize(ONE_MIB, b'\n');
        fx.sandbox.write_tenant_file_raw("prod", &bytes, 0o600);
        fx.run(LIST).assert_success();
        bytes.push(b'\n');
        fx.sandbox.write_tenant_file_raw("prod", &bytes, 0o600);
        fx.server.clear_requests();
        assert_generic(&fx, &tenant.store_path, "one byte over");
        fx.assert_no_requests();
    }

    #[test]
    fn typescript_numbers_are_converted_once_and_then_strict() {
        // The harness provisions with the import conversion: coerced TypeScript values
        // become TOML integers, `""` is dropped, renamed keys are renamed.
        let (fx, tenant) = Fx::prod_with(|config| {
            config
                .set("scanConcurrency", "3")
                .set("airsNumRetries", true)
                .set("aiGwInferenceModel", "m")
                .set("airsApiToken", "");
        });
        let text = fs::read_to_string(&tenant.store_path).expect("tenant file");
        assert!(text.contains("airsNumRetries = 1\n"), "{text}");
        assert!(text.contains("scanConcurrency = 3\n"), "{text}");
        assert!(text.contains("gatewayInferenceModel = \"m\"\n"), "{text}");
        assert!(!text.contains("airsApiToken"), "{text}");
        let out = fx.run(&[
            "tenant",
            "get",
            "prod",
            "scanConcurrency",
            "--output",
            "json",
        ]);
        assert_eq!(out.json(), json!([{"key": "scanConcurrency", "value": 3}]));
        fx.assert_no_requests();
    }
}

// ---------------------------------------------------------------------------------------
// §3.3 step 4: TSG pin
// ---------------------------------------------------------------------------------------
mod tsg_pin {
    use super::*;

    fn assert_pinned_out(fx: &Fx) {
        for args in [
            LIST,
            SCAN,
            &["tenant", "read", "--output", "json"][..],
            &["tenant", "read", "prod", "--output", "json"][..],
            &["tenant", "get", "prod", "mgmtTsgId"][..],
            &["--tenant", "prod", "runtime", "profiles", "list"][..],
        ] {
            // Exact stderr equality: neither TSG value is printed.
            assert_op_error(&fx.run(args), PIN_MSG);
        }
        fx.assert_no_requests();
    }

    #[test]
    fn mismatched_tsg_fails_before_any_request() {
        let (fx, _) = Fx::prod_with(|config| {
            config.set("mgmtTsgId", "200");
        });
        assert_pinned_out(&fx);
    }

    #[test]
    fn missing_mgmt_tsg_id_is_a_pin_mismatch_not_a_credential_error() {
        let (fx, _) = Fx::prod_with(|config| {
            config.remove("mgmtTsgId");
        });
        assert_pinned_out(&fx);
    }

    #[test]
    fn pin_comparison_is_exact_without_trimming() {
        let (fx, _) = Fx::prod_with(|config| {
            config.set("mgmtTsgId", " 100");
        });
        assert_pinned_out(&fx);
        let (fx, _) = Fx::prod_with(|config| {
            config.set("mgmtTsgId", "");
        });
        assert_pinned_out(&fx);
    }

    #[test]
    fn tenant_path_and_list_never_check_the_pin() {
        let (fx, tenant) = Fx::prod_with(|config| {
            config.set("mgmtTsgId", "200");
        });
        let out = fx.run(&["tenant", "path"]);
        out.assert_success();
        assert_eq!(out.stdout, format!("{}\n", tenant.store_path.display()));
        fx.run(&["tenant", "list", "--output", "json"])
            .assert_success();
        fx.assert_no_requests();
    }

    #[test]
    fn whitespace_pin_on_both_sides_reaches_the_missing_tsg_credential_check() {
        let fx = Fx::new();
        let tenant = fx.tenant_with("prod", "100", |config| {
            config.set("mgmtTsgId", "  ").remove("mgmtClientId");
            config.remove("mgmtClientSecret");
        });
        fx.sandbox
            .write_registry(&[entry("prod", &tenant.config_path, "  ")], Some("prod"));
        assert_op_error(
            &fx.run(LIST),
            &mgmt_missing("prod", "mgmtClientId, mgmtClientSecret, mgmtTsgId"),
        );
        fx.assert_no_requests();
    }
}

// ---------------------------------------------------------------------------------------
// §3.5 Credential checks
// ---------------------------------------------------------------------------------------
mod credentials {
    use super::*;

    #[test]
    fn missing_management_secret_names_key_and_remedy() {
        let (fx, _) = Fx::prod_with(|config| {
            config.remove("mgmtClientSecret");
        });
        for args in [
            LIST,
            &["runtime", "topics", "list"][..],
            &["runtime", "profiles", "get", "alpha"][..],
        ] {
            assert_op_error(&fx.run(args), &mgmt_missing("prod", "mgmtClientSecret"));
        }
        fx.assert_no_requests();
    }

    #[test]
    fn whitespace_or_empty_values_are_missing_in_check_order() {
        let (fx, _) = Fx::prod_with(|config| {
            config
                .set("mgmtClientId", "  \t")
                .set("mgmtClientSecret", "");
        });
        assert_op_error(
            &fx.run(LIST),
            &mgmt_missing("prod", "mgmtClientId, mgmtClientSecret"),
        );
        let (fx, _) = Fx::prod_with(|config| {
            config.set("mgmtClientSecret", "\u{a0}\u{2028}");
        });
        assert_op_error(&fx.run(LIST), &mgmt_missing("prod", "mgmtClientSecret"));
        fx.assert_no_requests();
    }

    #[test]
    fn credential_check_precedes_endpoint_admission() {
        // Rust-only: the invalid endpoint must not be reached; credentials fail first.
        let (fx, _) = Fx::prod_with_external_endpoints(|config| {
            config.remove("mgmtClientSecret");
            config.set("mgmtEndpoint", "http://example.test/aisec");
        });
        assert_op_error(&fx.run(LIST), &mgmt_missing("prod", "mgmtClientSecret"));
        fx.assert_no_requests();
    }

    #[test]
    fn missing_scanner_credentials_even_with_env_credentials() {
        let (fx, _) = Fx::prod_with(|config| {
            config.remove("airsApiKey");
            config.set("airsApiToken", "   ");
        });
        let options = RunOptions::new()
            .env("PANW_AI_SEC_API_KEY", "FAKE-APIKEY-100")
            .env("PANW_AI_SEC_API_TOKEN", "ENV-SCANNER-TOKEN-1");
        fx.sandbox.add_secret("env token", "ENV-SCANNER-TOKEN-1");
        assert_op_error(&fx.run_with(SCAN, options), &scanner_missing("prod"));
        fx.assert_no_requests();
    }

    #[test]
    fn scanner_only_tenant_can_scan_but_not_manage() {
        let (fx, _) = Fx::prod_with(|config| {
            config.remove("mgmtClientId");
            config.remove("mgmtClientSecret");
        });
        fx.run(SCAN).assert_success();
        fx.assert_scan_request_for("100");
        fx.server.clear_requests();
        assert_op_error(
            &fx.run(LIST),
            &mgmt_missing("prod", "mgmtClientId, mgmtClientSecret"),
        );
        fx.assert_no_requests();
    }

    #[test]
    fn management_only_tenant_can_list_but_not_scan() {
        let (fx, _) = Fx::prod_with(|config| {
            config.remove("airsApiKey");
        });
        let out = fx.run(LIST);
        out.assert_success();
        assert_eq!(out.json()[0]["profileName"], "alpha");
        fx.assert_mgmt_requests_for("100");
        fx.server.clear_requests();
        assert_op_error(&fx.run(SCAN), &scanner_missing("prod"));
        fx.assert_no_requests();
    }

    #[test]
    fn token_only_tenant_scans_with_bearer_only() {
        let (fx, _) = Fx::prod_with(|config| {
            config.remove("airsApiKey");
            config.set("airsApiToken", "FAKE-SCANTOKEN-100");
        });
        fx.run(SCAN).assert_success();
        let requests = fx.server.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].auth,
            AuthIdentity::ApiToken {
                token_id: "100".into()
            }
        );
        assert!(requests[0].header("x-pan-token").is_none());
        assert!(requests[0].header("x-payload-hash").is_none());
    }
}

// ---------------------------------------------------------------------------------------
// §4 Environment is ignored
// ---------------------------------------------------------------------------------------
mod environment {
    use super::*;

    /// prod (100) active; dev (200) has a config file and server credentials but is not
    /// registered. Every credential/endpoint variable points at dev or at unknown
    /// loopback routes, so honouring any of them would be visible.
    fn hostile_env(fx: &Fx, dev: &FixtureTenant) -> RunOptions {
        fx.sandbox
            .add_secret("env scanner token", "ENV-SCANNER-TOKEN-200");
        RunOptions::new()
            .env("PANW_MGMT_CLIENT_ID", "client-200")
            .env("PANW_MGMT_CLIENT_SECRET", "FAKE-SECRET-200")
            .env("PANW_MGMT_TSG_ID", "200")
            .env("PANW_MGMT_ENDPOINT", fx.server.url("/env-mgmt"))
            .env("PANW_MGMT_TOKEN_ENDPOINT", fx.server.url("/env-token"))
            .env("PANW_AI_SEC_API_KEY", "FAKE-APIKEY-200")
            .env("PANW_AI_SEC_API_TOKEN", "ENV-SCANNER-TOKEN-200")
            .env("PANW_AI_SEC_API_ENDPOINT", fx.server.url("/env-scan"))
            .env(
                "PRISMA_AIRS_CONFIG_PATH",
                dev.config_path.display().to_string(),
            )
            .env("PANW_CLI_OUTPUT", "yaml")
            .env("SCAN_CONCURRENCY", "99")
            .env("DATA_DIR", "/env/data")
    }

    fn setup() -> (Fx, FixtureTenant) {
        let (fx, _) = Fx::prod();
        let dev = fx.sandbox.add_fixture_tenant(&fx.server, "dev", "200");
        fx.server
            .set_profiles("200", vec![fixtures::profile(2, "beta", 1)]);
        (fx, dev)
    }

    #[test]
    fn management_requests_use_only_the_file() {
        let (fx, dev) = setup();
        let out = fx.run_with(LIST, hostile_env(&fx, &dev));
        out.assert_success();
        assert_eq!(out.json()[0]["profileName"], "alpha");
        fx.assert_mgmt_requests_for("100");
    }

    #[test]
    fn panw_cli_output_does_not_change_the_format() {
        let (fx, dev) = setup();
        let out = fx.run_with(
            &["runtime", "profiles", "list", "--output", "json"],
            hostile_env(&fx, &dev),
        );
        out.assert_success();
        assert_eq!(out.json()[0]["profileName"], "alpha");
        assert_eq!(out.stderr, "  Showing 1\n");
        let out = fx.run_with(
            &["tenant", "get", "prod", "mgmtTsgId"],
            hostile_env(&fx, &dev),
        );
        assert_eq!(out.stdout, "100\n");
    }

    #[test]
    fn scan_requests_use_only_the_file() {
        let (fx, dev) = setup();
        fx.run_with(SCAN, hostile_env(&fx, &dev)).assert_success();
        fx.assert_scan_request_for("100");
    }

    #[test]
    fn tenant_read_shows_file_values_not_env() {
        let (fx, dev) = setup();
        let out = fx.run_with(
            &["tenant", "read", "--output", "json"],
            hostile_env(&fx, &dev),
        );
        out.assert_success();
        let base = Sandbox::fixture_config(&fx.server, "100");
        assert_eq!(out.stdout, rows_json(&expected_read_rows(&base)));
        let out = fx.run_with(
            &["tenant", "get", "prod", "scanConcurrency"],
            hostile_env(&fx, &dev),
        );
        assert_eq!(out.stdout, "5\n", "SCAN_CONCURRENCY is ignored");
        fx.assert_no_requests();
    }

    #[test]
    fn config_path_variable_does_not_bypass_an_empty_registry() {
        let fx = Fx::new();
        let dev = fx.sandbox.add_fixture_tenant(&fx.server, "dev", "200");
        let out = fx.run_with(LIST, hostile_env(&fx, &dev));
        assert_op_error(&out, NO_TENANT_NONE);
        let out = fx.run_with(SCAN, hostile_env(&fx, &dev));
        assert_op_error(&out, NO_TENANT_NONE);
        fx.assert_no_requests();
    }
}

// ---------------------------------------------------------------------------------------
// §3.4 Two-tenant isolation and --tenant (D33)
// ---------------------------------------------------------------------------------------
mod isolation {
    use super::*;

    fn two() -> (Fx, FixtureTenant, FixtureTenant) {
        let fx = Fx::new();
        let prod = fx.sandbox.add_fixture_tenant(&fx.server, "prod", "100");
        let dev = fx.sandbox.add_fixture_tenant(&fx.server, "dev", "200");
        fx.server
            .set_profiles("100", vec![fixtures::profile(1, "alpha", 1)]);
        fx.server
            .set_profiles("200", vec![fixtures::profile(2, "beta", 1)]);
        (fx, prod, dev)
    }

    #[test]
    fn switching_the_registry_active_switches_every_request() {
        let (fx, prod, dev) = two();
        fx.sandbox.register(&[&prod, &dev], Some("prod"));
        let out = fx.run(LIST);
        assert_eq!(out.json()[0]["profileName"], "alpha");
        fx.assert_mgmt_requests_for("100");
        fx.server.clear_requests();
        fx.run(SCAN).assert_success();
        fx.assert_scan_request_for("100");
        fx.server.clear_requests();

        fx.sandbox.register(&[&prod, &dev], Some("dev"));
        let out = fx.run(LIST);
        assert_eq!(out.json()[0]["profileName"], "beta");
        fx.assert_mgmt_requests_for("200");
        fx.server.clear_requests();
        fx.run(SCAN).assert_success();
        fx.assert_scan_request_for("200");
    }

    #[test]
    fn tenant_flag_selects_without_touching_the_registry() {
        let (fx, prod, dev) = two();
        fx.sandbox.register(&[&prod, &dev], Some("prod"));
        let out = fx.run(&[
            "--tenant", "dev", "runtime", "profiles", "list", "--output", "json",
        ]);
        assert_eq!(out.json()[0]["profileName"], "beta");
        fx.assert_mgmt_requests_for("200");
        fx.server.clear_requests();
        // Global flag also accepted after the subcommand.
        let out = fx.run(&[
            "runtime", "profiles", "list", "--tenant", "dev", "--output", "json",
        ]);
        assert_eq!(out.json()[0]["profileName"], "beta");
        fx.assert_mgmt_requests_for("200");
        fx.server.clear_requests();
        let mut scan = vec!["--tenant", "dev"];
        scan.extend_from_slice(SCAN);
        fx.run(&scan).assert_success();
        fx.assert_scan_request_for("200");
        fx.server.clear_requests();

        fx.sandbox.register(&[&prod, &dev], Some("dev"));
        let out = fx.run(&[
            "--tenant", "prod", "runtime", "profiles", "list", "--output", "json",
        ]);
        assert_eq!(out.json()[0]["profileName"], "alpha");
        fx.assert_mgmt_requests_for("100");
    }

    #[test]
    fn tenant_flag_works_with_no_active_tenant() {
        let (fx, prod, dev) = two();
        fx.sandbox.register(&[&prod, &dev], None);
        let out = fx.run(&[
            "--tenant", "dev", "runtime", "profiles", "list", "--output", "json",
        ]);
        assert_eq!(out.json()[0]["profileName"], "beta");
        fx.assert_mgmt_requests_for("200");
    }

    #[test]
    fn tenant_flag_drives_tenant_read_and_path() {
        let (fx, prod, dev) = two();
        fx.sandbox.register(&[&prod, &dev], Some("prod"));
        let out = fx.run(&["--tenant", "dev", "tenant", "read", "--output", "json"]);
        out.assert_success();
        assert_eq!(
            out.stderr,
            format!("  Tenant dev (TSG 200): {}\n", dev.store_path.display())
        );
        assert_eq!(out.stdout, rows_json(&expected_read_rows(&dev.config)));
        let out = fx.run(&["tenant", "path", "--tenant", "dev"]);
        assert_eq!(out.stdout, format!("{}\n", dev.store_path.display()));
        // A positional name wins over --tenant.
        let out = fx.run(&["--tenant", "dev", "tenant", "path", "prod"]);
        assert_eq!(out.stdout, format!("{}\n", prod.store_path.display()));
        let out = fx.run(&[
            "--tenant", "dev", "tenant", "read", "prod", "--output", "json",
        ]);
        assert_eq!(
            out.stderr,
            format!("  Tenant prod (TSG 100): {}\n", prod.store_path.display())
        );
        fx.assert_no_requests();
    }

    #[test]
    fn tenant_flag_still_applies_the_pin() {
        let fx = Fx::new();
        let prod = fx.sandbox.add_fixture_tenant(&fx.server, "prod", "100");
        let dev = fx.tenant_with("dev", "200", |config| {
            config.set("mgmtTsgId", "100");
        });
        fx.sandbox.register(&[&prod, &dev], Some("prod"));
        assert_op_error(
            &fx.run(&["--tenant", "dev", "runtime", "profiles", "list"]),
            PIN_MSG,
        );
        fx.assert_no_requests();
    }
}

mod tenant_flag_validation {
    use super::*;

    #[test]
    fn bad_names_are_usage_errors_before_the_registry_is_read() {
        let fx = Fx::new();
        fx.sandbox.write_registry_raw(b"{ corrupt");
        let long = "x".repeat(65);
        for name in ["bad name", "-bad", "_bad", "", "é", long.as_str(), "a/b"] {
            let flag = format!("--tenant={name}");
            for args in [
                vec![flag.as_str(), "runtime", "profiles", "list"],
                vec![flag.as_str(), "tenant", "list", "--output", "json"],
                vec![flag.as_str(), "tenant", "read", "--output", "json"],
                vec![flag.as_str(), "runtime", "scan", "--profile", "p", "hi"],
            ] {
                assert_usage(&fx.run(&args), TENANT_NAME_USAGE);
            }
        }
        fx.assert_no_requests();
    }

    #[test]
    fn sixty_four_character_name_is_valid_format() {
        let fx = Fx::new();
        fx.sandbox.write_registry(&[], None);
        let name = "x".repeat(64);
        let out = fx.run(&["--tenant", &name, "runtime", "profiles", "list"]);
        assert_op_error(
            &out,
            &format!("Tenant not found: {name} (no tenants registered)"),
        );
    }

    #[test]
    fn unknown_tenant_lists_registered_names() {
        let fx = Fx::new();
        let prod = fx.sandbox.add_fixture_tenant(&fx.server, "prod", "100");
        let dev = fx.sandbox.add_fixture_tenant(&fx.server, "dev", "200");
        fx.sandbox.register(&[&prod, &dev], Some("prod"));
        for args in [
            &["--tenant", "ghost", "runtime", "profiles", "list"][..],
            &[
                "--tenant",
                "ghost",
                "runtime",
                "scan",
                "--profile",
                "p",
                "hi",
            ][..],
            &["--tenant", "ghost", "tenant", "read", "--output", "json"][..],
            &["--tenant", "ghost", "tenant", "path"][..],
        ] {
            assert_op_error(
                &fx.run(args),
                "Tenant not found: ghost (registered: dev, prod)",
            );
        }
        fx.assert_no_requests();
    }

    #[test]
    fn unknown_tenant_with_empty_registry() {
        let fx = Fx::new();
        let out = fx.run(&["--tenant", "ghost", "runtime", "profiles", "list"]);
        assert_op_error(&out, "Tenant not found: ghost (no tenants registered)");
        fx.assert_no_requests();
    }
}

// ---------------------------------------------------------------------------------------
// §5 Endpoint admission (Rust-only: values a TS run must never see)
// ---------------------------------------------------------------------------------------
mod endpoints {
    use super::*;

    fn rejected(key: &str, value: &str, args: &[&str]) {
        // Rust-only (harness opt-out): the value never reaches the network.
        let (fx, _) = Fx::prod_with_external_endpoints(|config| {
            config.set(key, value);
        });
        let out = fx.run(args);
        assert_op_error(&out, &format!("Invalid {key}: {ENDPOINT_RULE}"));
        assert!(!out.stderr.contains(value), "the value was echoed");
        fx.assert_no_requests();
    }

    #[test]
    fn non_loopback_http_management_endpoint() {
        rejected("mgmtEndpoint", "http://example.test/aisec", LIST);
    }

    #[test]
    fn non_loopback_http_token_endpoint() {
        rejected("mgmtTokenEndpoint", "http://example.test/oauth/token", LIST);
    }

    #[test]
    fn non_loopback_http_scanner_endpoint() {
        rejected("airsApiEndpoint", "http://example.test/scan", SCAN);
    }

    #[test]
    fn unspecified_address_is_not_loopback() {
        rejected("mgmtEndpoint", "http://0.0.0.0:1/aisec", LIST);
    }

    #[test]
    fn endpoint_with_query_is_rejected() {
        let server = FixtureServer::start();
        rejected("mgmtEndpoint", &server.url("/aisec?tsg=1"), LIST);
        rejected("airsApiEndpoint", &server.url("/scan?x=1"), SCAN);
    }

    #[test]
    fn endpoint_with_fragment_is_rejected() {
        let server = FixtureServer::start();
        rejected("mgmtTokenEndpoint", &server.url("/oauth/token#frag"), LIST);
    }

    #[test]
    fn endpoint_with_userinfo_is_rejected() {
        let server = FixtureServer::start();
        let with_user = server.base_url().replace("http://", "http://user:pass@");
        rejected(
            "mgmtTokenEndpoint",
            &format!("{with_user}/oauth/token"),
            LIST,
        );
        rejected("airsApiEndpoint", &format!("{with_user}/scan"), SCAN);
    }

    #[test]
    fn non_digit_mgmt_tsg_id_is_a_local_error() {
        // D28: TS sends a non-numeric TSG to the token endpoint; airs-cli-rs stops locally.
        for tsg in ["abc", "12a", &"1".repeat(33)] {
            let fx = Fx::new();
            let tenant = fx.tenant_with("prod", tsg, |_| {});
            fx.sandbox.register(&[&tenant], Some("prod"));
            assert_op_error(
                &fx.run(LIST),
                "Invalid mgmtTsgId: the Rust SDK requires 1-32 ASCII digits",
            );
            fx.assert_no_requests();
        }
    }

    #[test]
    fn thirty_two_digit_tsg_is_accepted() {
        let tsg = "1".repeat(32);
        let fx = Fx::new();
        let tenant = fx.tenant_with("prod", &tsg, |_| {});
        fx.sandbox.register(&[&tenant], Some("prod"));
        fx.run(LIST).assert_success();
        fx.assert_mgmt_requests_for(&tsg);
    }
}

// ---------------------------------------------------------------------------------------
// §3 Read-only guarantees
// ---------------------------------------------------------------------------------------
mod read_only {
    use super::*;

    /// Restores writable directories on drop so the sandbox can be removed.
    struct Writable(Vec<PathBuf>);
    impl Drop for Writable {
        fn drop(&mut self) {
            for dir in &self.0 {
                let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
            }
        }
    }

    #[test]
    fn read_only_files_and_foreign_locks_are_left_alone() {
        let (fx, tenant) = Fx::prod();
        let registry_lock = fx.sandbox.state_dir().join("tenants.json.lock");
        let config_lock = PathBuf::from(format!("{}.lock", tenant.config_path.display()));
        fs::write(&registry_lock, b"12345\n").expect("lock");
        fs::write(&config_lock, b"67890\n").expect("lock");
        set_mode(&registry_lock, 0o600);
        set_mode(&config_lock, 0o600);
        set_mode(&fx.sandbox.registry_path(), 0o400);
        assert_eq!(
            fs::metadata(&tenant.config_path)
                .expect("meta")
                .permissions()
                .mode()
                & 0o777,
            0o400
        );
        // Read-only directories: nothing can be created next to the files either.
        let _guard = Writable(vec![fx.sandbox.state_dir(), fx.sandbox.configs_dir()]);
        set_mode(&fx.sandbox.state_dir(), 0o500);
        set_mode(&fx.sandbox.configs_dir(), 0o500);

        let before = fx.sandbox.snapshot();
        let mut scans = 0;
        let mut lists = 0;
        for args in [
            &["tenant", "list", "--output", "json"][..],
            &["tenant", "ls", "--output", "json"][..],
            &["tenant", "path"][..],
            &["tenant", "path", "prod"][..],
            &["tenant", "read", "--output", "json"][..],
            &["tenant", "get", "prod", "mgmtClientSecret"][..],
            LIST,
            SCAN,
            &["--tenant", "prod", "runtime", "profiles", "list"][..],
        ] {
            let out = fx.run(args);
            out.assert_success();
            if args == SCAN {
                scans += 1;
            }
            if args.contains(&"profiles") {
                lists += 1;
            }
        }
        fx.sandbox.assert_unchanged(&before);
        assert_eq!(fx.sandbox.snapshot().paths(), before.paths());
        assert_eq!(fx.server.requests_for(Route::ScanSync).len(), scans);
        assert_eq!(fx.server.requests_for(Route::Profiles).len(), lists);
    }
}

// ---------------------------------------------------------------------------------------
// §3.6 tenant list | ls
// ---------------------------------------------------------------------------------------
mod tenant_list {
    use super::*;

    #[test]
    fn single_active_tenant_exact_json() {
        let (fx, tenant) = Fx::prod();
        let expected = list_json(&[("prod", &tenant.store_path, "100", true)]);
        for args in [
            &["tenant", "list", "--output", "json"][..],
            &["tenant", "ls", "--output", "json"][..],
            &["tenant", "list", "--output", "json"][..],
            &["--output", "json", "tenant", "ls", "--output", "json"][..],
            &["--output", "yaml", "tenant", "list", "--output", "json"][..],
        ] {
            let out = fx.run(args);
            assert_eq!(
                (out.code, out.stdout.as_str(), out.stderr.as_str()),
                (0, expected.as_str(), ""),
                "{args:?}"
            );
        }
        fx.assert_no_requests();
    }

    #[test]
    fn registry_order_and_active_flags() {
        let fx = Fx::new();
        let zeta = fx.sandbox.add_fixture_tenant(&fx.server, "zeta", "300");
        let alpha = fx.sandbox.add_fixture_tenant(&fx.server, "alpha", "100");
        fx.sandbox.register(&[&zeta, &alpha], Some("alpha"));
        let out = fx.run(&["tenant", "list", "--output", "json"]);
        out.assert_success();
        assert_eq!(
            out.stdout,
            list_json(&[
                // Byte order of the names (D86), the store paths (D70).
                ("alpha", &alpha.store_path, "100", true),
                ("zeta", &zeta.store_path, "300", false),
            ])
        );
        assert_eq!(out.stderr, "");
    }

    #[test]
    fn no_active_tenant_prints_a_quiet_suppressible_status() {
        let fx = Fx::new();
        let prod = fx.sandbox.add_fixture_tenant(&fx.server, "prod", "100");
        fx.sandbox.register(&[&prod], None);
        let expected = list_json(&[("prod", &prod.store_path, "100", false)]);
        let out = fx.run(&["tenant", "list", "--output", "json"]);
        assert_eq!(
            (out.code, out.stdout.as_str(), out.stderr.as_str()),
            (
                0,
                expected.as_str(),
                "  No tenant selected; run 'airs tenant switch <name>'.\n"
            )
        );
        let out = fx.run(&["tenant", "list", "--quiet", "--output", "json"]);
        assert_eq!(
            (out.code, out.stdout.as_str(), out.stderr.as_str()),
            (0, expected.as_str(), "")
        );
    }

    #[test]
    fn empty_registry_prints_an_empty_array() {
        let fx = Fx::new();
        fx.sandbox.write_registry(&[], None);
        let out = fx.run(&["tenant", "list", "--output", "json"]);
        assert_eq!(
            (out.code, out.stdout.as_str(), out.stderr.as_str()),
            (0, "[]\n", "")
        );
    }

    #[test]
    fn never_opens_config_files() {
        let fx = Fx::new();
        let missing = fx.sandbox.configs_dir().join("missing.json");
        let invalid = fx
            .sandbox
            .write_config_raw("invalid", b"{oops SUPER-SECRET");
        let mismatched = fx.tenant_with("mismatched", "100", |config| {
            config.set("mgmtTsgId", "999");
        });
        fx.sandbox.write_registry(
            &[
                entry("missing", &missing, "1"),
                entry("invalid", &invalid, "2"),
                mismatched.entry(),
            ],
            Some("invalid"),
        );
        let out = fx.run_with(
            &["tenant", "list", "--output", "json"],
            RunOptions::new().secret("SUPER-SECRET"),
        );
        out.assert_success();
        assert_eq!(
            out.stdout,
            list_json(&[
                (
                    "invalid",
                    &fx.sandbox.store_tenant_path("invalid"),
                    "2",
                    true
                ),
                ("mismatched", &mismatched.store_path, "100", false),
                (
                    "missing",
                    &fx.sandbox.store_tenant_path("missing"),
                    "1",
                    false
                ),
            ])
        );
        fx.assert_no_requests();
    }

    #[test]
    fn dropped_and_invalid_formats_are_usage_errors_before_the_registry_is_read() {
        let fx = Fx::new();
        fx.sandbox.write_registry_raw(b"not json");
        for (command, format) in [
            ("list", "markdown"),
            ("ls", "yaml"),
            ("list", "csv"),
            ("list", "xml"),
        ] {
            assert_usage(
                &fx.run(&["tenant", command, "--output", format]),
                &format!("Invalid output format '{format}'. Expected: pretty, table, json"),
            );
        }
        fx.assert_no_requests();
    }

    #[test]
    fn registry_errors_exit_one() {
        let fx = Fx::new();
        // A TypeScript registry TypeScript rejects is provisioned as a corrupt
        // config.toml: the same message, with the store path (D70).
        fx.sandbox.write_registry_raw(b"not json");
        assert_op_error(
            &fx.run(&["tenant", "list", "--output", "json"]),
            &registry_msg(&fx.sandbox.store_index_path()),
        );
    }

    #[test]
    fn bare_tenant_group_prints_help_to_stderr_with_exit_two() {
        let fx = Fx::new();
        let out = fx.run(&["tenant"]);
        assert_eq!(out.code, 2);
        assert_eq!(out.stdout, "");
        assert!(out.stderr.contains("Usage:"), "{}", out.stderr);
        for sub in ["list", "path", "read", "get"] {
            assert!(out.stderr.contains(sub), "help lacks {sub}: {}", out.stderr);
        }
        fx.assert_no_requests();
    }
}

// ---------------------------------------------------------------------------------------
// §3.6 tenant path
// ---------------------------------------------------------------------------------------
mod tenant_path {
    use super::*;

    #[test]
    fn prints_the_registered_path_for_active_and_named_tenants() {
        let fx = Fx::new();
        let prod = fx.sandbox.add_fixture_tenant(&fx.server, "prod", "100");
        let dev = fx.sandbox.add_fixture_tenant(&fx.server, "dev", "200");
        fx.sandbox.register(&[&prod, &dev], Some("prod"));
        for (args, path) in [
            (&["tenant", "path"][..], &prod.store_path),
            (&["tenant", "path", "dev"][..], &dev.store_path),
            (
                &["tenant", "path", "--output", "table"][..],
                &prod.store_path,
            ),
            (
                &["tenant", "path", "--output", "json"][..],
                &prod.store_path,
            ),
            (&["tenant", "path", "--quiet"][..], &prod.store_path),
        ] {
            let out = fx.run(args);
            assert_eq!(
                (out.code, out.stdout.clone(), out.stderr.as_str()),
                (0, format!("{}\n", path.display()), ""),
                "{args:?}"
            );
        }
        // D59: a name outside the three is still a usage error.
        assert_usage(
            &fx.run(&["tenant", "path", "--output", "xml"]),
            "Invalid output format 'xml'. Expected: pretty, table, json",
        );
        fx.assert_no_requests();
    }

    #[test]
    fn never_opens_the_config() {
        let fx = Fx::new();
        let missing = fx.sandbox.configs_dir().join("missing.json");
        let odd = Path::new("/not/normalised/../path.json");
        fx.sandbox.write_registry(
            &[entry("gone", &missing, "1"), entry("odd", odd, "2")],
            Some("gone"),
        );
        // The store path, whether or not the tenant file exists (§3.6).
        let out = fx.run(&["tenant", "path"]);
        assert_eq!(
            out.stdout,
            format!("{}\n", fx.sandbox.store_tenant_path("gone").display())
        );
        let out = fx.run(&["tenant", "path", "odd"]);
        assert_eq!(
            out.stdout,
            format!("{}\n", fx.sandbox.store_tenant_path("odd").display())
        );
    }

    #[test]
    fn name_resolution_errors() {
        let fx = Fx::new();
        let prod = fx.sandbox.add_fixture_tenant(&fx.server, "prod", "100");
        let dev = fx.sandbox.add_fixture_tenant(&fx.server, "dev", "200");
        fx.sandbox.register(&[&prod, &dev], None);
        assert_op_error(
            &fx.run(&["tenant", "path"]),
            "No tenant selected; pass a name or run 'airs tenant switch <name>' (registered: dev, prod)",
        );
        assert_op_error(&fx.run(&["tenant", "path", "ghost"]), "Tenant not found");
        fx.sandbox.write_registry(&[], None);
        assert_op_error(
            &fx.run(&["tenant", "path"]),
            "No tenant selected; run 'airs tenant create <name>' first",
        );
        assert_op_error(&fx.run(&["tenant", "path", "ghost"]), "Tenant not found");
        fx.assert_no_requests();
    }
}

// ---------------------------------------------------------------------------------------
// §3.6 tenant read
// ---------------------------------------------------------------------------------------
mod tenant_read {
    use super::*;

    #[test]
    fn standard_config_exact_rows_and_status() {
        let (fx, tenant) = Fx::prod();
        let expected = rows_json(&expected_read_rows(&tenant.config));
        // Sanity: every endpoint key is shown verbatim, secrets redacted.
        for key in airs_test_harness::store::RUST_ENDPOINT_KEYS {
            assert!(expected.contains(&format!("\"key\": \"{key}\"")), "{key}");
        }
        let status = format!("  Tenant prod (TSG 100): {}\n", tenant.store_path.display());
        for args in [
            &["tenant", "read", "--output", "json"][..],
            &["tenant", "read", "prod", "--output", "json"][..],
            &["tenant", "read", "--output", "json"][..],
        ] {
            let out = fx.run(args);
            assert_eq!(
                (out.code, out.stdout.as_str(), out.stderr.as_str()),
                (0, expected.as_str(), status.as_str()),
                "{args:?}"
            );
        }
        let out = fx.run(&["tenant", "read", "--quiet", "--output", "json"]);
        assert_eq!(
            (out.stdout.as_str(), out.stderr.as_str()),
            (expected.as_str(), "")
        );
        fx.assert_no_requests();
    }

    #[test]
    fn effective_values_redaction_defaults_and_hidden_keys() {
        let (fx, tenant) = Fx::prod_with(|config| {
            config
                .set("airsApiToken", "")
                .set("aiGwInferenceApiKey", "FAKE-INFERENCE-KEY")
                .set("typesafeApiKey", "  ")
                .set("typesafeModel", "")
                .set("scanConcurrency", "3")
                .set("airsNumRetries", "")
                .set("defaultOutput", "table")
                .set("dataDir", "~/runs")
                .set("custom", "hidden")
                .set("redTeamTokenEndpoint", "hidden-too");
        });
        let mut rows = expected_read_rows(&tenant.config);
        for (key, value) in rows.iter_mut() {
            match *key {
                "scanConcurrency" => *value = json!(3),
                "airsNumRetries" => *value = json!(0),
                // A whitespace-only secret is non-empty, so it is redacted.
                "typesafeApiKey" => assert_eq!(value, &json!("[REDACTED]")),
                "airsApiToken" | "typesafeModel" => assert_eq!(value, &json!("")),
                _ => {}
            }
        }
        let out = fx.run(&["tenant", "read", "--output", "json"]);
        out.assert_success();
        assert_eq!(out.stdout, rows_json(&rows));
        assert!(!out.stdout.contains("hidden"));
        assert!(!out.stdout.contains("custom"));
        let keys: Vec<String> = out
            .json()
            .as_array()
            .expect("array")
            .iter()
            .map(|row| row["key"].as_str().unwrap_or_default().to_owned())
            .collect();
        let schema_order: Vec<String> = KEYS
            .iter()
            .filter(|key| keys.contains(&key.to_string()))
            .map(|key| key.to_string())
            .collect();
        assert_eq!(keys, schema_order, "rows are in schema order");
    }

    #[test]
    fn minimal_config_shows_only_present_keys_plus_defaults() {
        // Rust-only (harness opt-out): no endpoint keys at all; `tenant read` builds no
        // client.
        let fx = Fx::new();
        let path = fx.sandbox.write_config_allowing_external_endpoints(
            "mini",
            &FixtureConfig::from_value(
                &json!({"mgmtTsgId": "100", "mgmtClientId": "client-100", "mgmtClientSecret": "X-SECRET-VALUE"}),
            ),
        );
        fx.sandbox
            .write_registry(&[entry("mini", &path, "100")], Some("mini"));
        let out = fx.run(&["tenant", "read", "--output", "json"]);
        out.assert_success();
        assert_eq!(
            out.stdout,
            rows_json(&[
                ("mgmtClientId", json!("client-100")),
                ("mgmtClientSecret", json!("[REDACTED]")),
                ("mgmtTsgId", json!("100")),
                ("scanConcurrency", json!(5)),
            ])
        );
        assert_eq!(
            out.stderr,
            format!(
                "  Tenant mini (TSG 100): {}\n",
                fx.sandbox.store_tenant_path("mini").display()
            )
        );
        let _ = path;
        fx.assert_no_requests();
    }

    #[test]
    fn name_resolution_errors() {
        let fx = Fx::new();
        let prod = fx.sandbox.add_fixture_tenant(&fx.server, "prod", "100");
        let dev = fx.sandbox.add_fixture_tenant(&fx.server, "dev", "200");
        fx.sandbox.register(&[&prod, &dev], None);
        assert_op_error(
            &fx.run(&["tenant", "read", "--output", "json"]),
            "No tenant selected; pass a name or run 'airs tenant switch <name>' (registered: dev, prod)",
        );
        assert_op_error(
            &fx.run(&["tenant", "read", "ghost", "--output", "json"]),
            "Tenant not found",
        );
        let out = fx.run(&["tenant", "read", "dev", "--output", "json"]);
        out.assert_success();
        assert_eq!(out.stdout, rows_json(&expected_read_rows(&dev.config)));
        fx.sandbox.write_registry(&[], None);
        assert_op_error(
            &fx.run(&["tenant", "read", "--output", "json"]),
            "No tenant selected; run 'airs tenant create <name>' first",
        );
        fx.assert_no_requests();
    }

    #[test]
    fn dropped_formats_are_usage_errors() {
        let (fx, _) = Fx::prod();
        for format in ["markdown", "csv", "yaml"] {
            assert_usage(
                &fx.run(&["tenant", "read", "--output", format]),
                &format!("Invalid output format '{format}'. Expected: pretty, table, json"),
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// §3.6 tenant get
// ---------------------------------------------------------------------------------------
mod tenant_get {
    use super::*;

    #[test]
    fn pretty_values() {
        let (fx, _) = Fx::prod_with(|config| {
            config
                .set("airsApiToken", "")
                .set("aiGwInferenceModel", "model-x");
        });
        let base = fx.server.base_url();
        let cases = [
            ("mgmtTsgId", "100".to_owned()),
            ("mgmtClientId", "client-100".to_owned()),
            ("mgmtClientSecret", "[REDACTED]".to_owned()),
            ("airsApiKey", "[REDACTED]".to_owned()),
            ("airsApiToken", String::new()),
            ("gatewayInferenceApiKey", String::new()),
            ("airsNumRetries", "0".to_owned()),
            ("scanConcurrency", "5".to_owned()),
            ("defaultOutput", String::new()),
            ("gatewayInferenceModel", "model-x".to_owned()),
            ("mgmtEndpoint", format!("{base}/aisec")),
        ];
        for (key, shown) in cases {
            let out = fx.run(&["tenant", "get", "prod", key]);
            assert_eq!(
                (out.code, out.stdout.clone(), out.stderr.as_str()),
                (0, format!("{shown}\n"), ""),
                "{key}"
            );
            let out = fx.run(&["tenant", "get", "prod", key, "--output", "pretty"]);
            assert_eq!(out.stdout, format!("{shown}\n"), "{key}");
        }
        fx.assert_no_requests();
    }

    #[test]
    fn json_values_are_one_element_arrays() {
        let (fx, _) = Fx::prod_with(|config| {
            config.set("airsApiToken", "");
        });
        let cases = [
            ("mgmtTsgId", json!("100")),
            ("mgmtClientSecret", json!("[REDACTED]")),
            ("airsApiToken", json!("")),
            ("gatewayInferenceApiKey", json!("")),
            ("airsNumRetries", json!(0)),
            ("scanConcurrency", json!(5)),
            ("defaultOutput", json!("")),
        ];
        for (key, value) in cases {
            let out = fx.run(&["tenant", "get", "prod", key, "--output", "json"]);
            assert_eq!(
                (out.code, out.stdout.clone(), out.stderr.as_str()),
                (0, rows_json(&[(key, value.clone())]), ""),
                "{key}"
            );
        }
        fx.assert_no_requests();
    }

    #[test]
    fn unknown_key_is_a_usage_error_before_the_registry_is_read() {
        // D34: TS exits 1 for an unknown key; DESIGN makes it a usage error (exit 2).
        let fx = Fx::new();
        fx.sandbox.write_registry_raw(b"{ corrupt");
        for key in [
            "notAKey",
            "__proto__",
            "constructor",
            "MGMTTSGID",
            "custom",
            "redTeamTokenEndpoint",
            "",
        ] {
            assert_usage(&fx.run(&["tenant", "get", "prod", key]), &unknown_key_msg());
            assert_usage(
                &fx.run(&["tenant", "get", "ghost", key, "--output", "xml"]),
                &unknown_key_msg(),
            );
        }
        fx.assert_no_requests();
    }

    #[test]
    fn unsupported_format_is_a_usage_error() {
        let (fx, _) = Fx::prod();
        for format in ["yaml", "markdown", "csv", "xml"] {
            assert_usage(
                &fx.run(&["tenant", "get", "prod", "mgmtTsgId", "--output", format]),
                &format!("Invalid output format '{format}'. Expected: pretty, table, json"),
            );
        }
    }

    #[test]
    fn tenant_name_is_required_and_resolved_exactly() {
        let fx = Fx::new();
        let prod = fx.sandbox.add_fixture_tenant(&fx.server, "prod", "100");
        let dev = fx.sandbox.add_fixture_tenant(&fx.server, "dev", "200");
        fx.sandbox.register(&[&prod, &dev], Some("prod"));
        assert_op_error(
            &fx.run(&["tenant", "get", "ghost", "mgmtTsgId"]),
            "Tenant not found",
        );
        assert_op_error(
            &fx.run(&["tenant", "get", "PROD", "mgmtTsgId"]),
            "Tenant not found",
        );
        assert_eq!(
            fx.run(&["tenant", "get", "dev", "mgmtTsgId"]).stdout,
            "200\n"
        );
        // --tenant does not replace the positional name.
        assert_eq!(
            fx.run(&["--tenant", "dev", "tenant", "get", "prod", "mgmtTsgId"])
                .stdout,
            "100\n"
        );
        let missing = fx.run(&["tenant", "get", "prod"]);
        assert_eq!((missing.code, missing.stdout.as_str()), (2, ""));
        fx.assert_no_requests();
    }

    #[test]
    fn applies_the_pin_and_config_validation() {
        let (fx, tenant) = Fx::prod_with(|config| {
            config.set("mgmtTsgId", "200");
        });
        assert_op_error(&fx.run(&["tenant", "get", "prod", "mgmtTsgId"]), PIN_MSG);
        fx.sandbox.write_config_raw("prod", b"[]");
        assert_op_error(
            &fx.run(&["tenant", "get", "prod", "mgmtTsgId"]),
            &config_msg(&tenant.store_path),
        );
        fx.assert_no_requests();
    }
}

// ---------------------------------------------------------------------------------------
// Review fixes: empty HOME (§3.1), empty positional names (§3.6), endpoint redaction (D45)
// ---------------------------------------------------------------------------------------

mod review_fixes {
    use super::*;

    /// §3.1, D90: with an empty `HOME` the store still comes from
    /// `AIRS_CLI_CONFIG_DIR`; the TypeScript registry, which an empty `HOME` puts at
    /// `.local/state/prisma-airs/tenants.json` relative to the cwd, is only looked at for
    /// the import suffix, never read.
    #[test]
    fn empty_home_reads_the_registry_relative_to_the_cwd() {
        let fx = Fx::new();
        let tenant = fx.sandbox.single_tenant(&fx.server, "prod", "100");
        let options = || {
            RunOptions::new()
                .env("HOME", "")
                .without_env("PRISMA_AIRS_TENANTS_PATH")
                .without_env("XDG_STATE_HOME")
        };
        let out = fx.run_with(&["tenant", "list", "--output", "json"], options());
        assert_eq!(
            out.stdout,
            list_json(&[("prod", &tenant.store_path, "100", true)])
        );
        let out = fx.run_with(&["tenant", "path"], options());
        assert_eq!(out.stdout, format!("{}\n", tenant.store_path.display()));
        fx.assert_no_requests();
    }

    /// §3.6: an empty positional name selects nothing (TS `name ?? store.active` is
    /// falsy); neither the active tenant nor `--tenant` stands in.
    #[test]
    fn empty_positional_name_is_no_selection() {
        let (fx, _) = Fx::prod();
        for args in [
            &["tenant", "path", ""][..],
            &["tenant", "read", ""],
            &["tenant", "get", "", "mgmtTsgId"],
            &["--tenant", "prod", "tenant", "read", ""],
        ] {
            assert_op_error(
                &fx.run(args),
                "No tenant selected; pass a name or run 'airs tenant switch <name>' (registered: prod)",
            );
        }
        fx.sandbox.write_registry(&[], None);
        for args in [
            &["tenant", "path", ""][..],
            &["tenant", "get", "", "mgmtTsgId"],
        ] {
            assert_op_error(
                &fx.run(args),
                "No tenant selected; run 'airs tenant create <name>' first",
            );
        }
        fx.assert_no_requests();
    }

    /// D45: endpoint values are shown with userinfo, query and fragment redacted.
    #[test]
    fn endpoint_credentials_and_queries_are_redacted() {
        let (fx, _) = Fx::prod_with_external_endpoints(|config| {
            config.set(
                "airsApiEndpoint",
                "https://svc-user:FAKE-PW-999@example.invalid/?api_key=FAKE-Q-777",
            );
            config.set(
                "mgmtEndpoint",
                "https://u:FAKE-PW-222@127.0.0.1/x?token=FAKE-Q-222#f",
            );
        });
        for secret in ["FAKE-PW-999", "FAKE-Q-777", "FAKE-PW-222", "FAKE-Q-222"] {
            fx.sandbox.add_secret("endpoint credential", secret);
        }
        let out = fx.run(&["tenant", "get", "prod", "airsApiEndpoint"]);
        out.assert_success();
        assert_eq!(
            out.stdout,
            "https://[REDACTED]@example.invalid/?[REDACTED]\n"
        );
        let out = fx.run(&["tenant", "get", "prod", "mgmtEndpoint", "--output", "json"]);
        assert_eq!(
            serde_json::from_str::<Value>(&out.stdout).expect("json")[0]["value"],
            "https://[REDACTED]@127.0.0.1/x?[REDACTED]#[REDACTED]"
        );
        let out = fx.run(&["tenant", "read", "prod", "--output", "json"]);
        out.assert_success();
        let rows: Value = serde_json::from_str(&out.stdout).expect("json");
        let value = |key: &str| {
            rows.as_array()
                .expect("rows")
                .iter()
                .find(|row| row["key"] == key)
                .map(|row| row["value"].clone())
        };
        assert_eq!(
            value("airsApiEndpoint"),
            Some(json!("https://[REDACTED]@example.invalid/?[REDACTED]"))
        );
        // A clean endpoint is shown byte for byte.
        assert!(
            value("mgmtTokenEndpoint")
                .and_then(|v| v.as_str().map(str::to_owned))
                .is_some_and(|v| v.ends_with("/oauth/token")),
            "{rows}"
        );
        fx.assert_no_requests();
    }
}
