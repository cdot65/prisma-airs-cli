//! Per-test tenant sandbox: a temp directory holding the registry, configs, HOME and cwd.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use serde_json::{Map, Value};
use tempfile::TempDir;

use crate::encoding::{base64_encode, percent_encode};
use crate::server::{FixtureServer, TOKEN_PREFIX, paths};

/// Config keys that hold endpoints; the fixture config points every one at the server.
pub const ENDPOINT_KEYS: &[&str] = &[
    "airsApiEndpoint",
    "mgmtEndpoint",
    "mgmtDashboardEndpoint",
    "mgmtTokenEndpoint",
    "dlpEndpoint",
    "redTeamDataEndpoint",
    "redTeamMgmtEndpoint",
    "redTeamNetworkBrokerEndpoint",
    "modelSecDataEndpoint",
    "modelSecMgmtEndpoint",
    "agentGuardDataEndpoint",
    "agentGuardMgmtEndpoint",
    "aiGwDataEndpoint",
    "aiGwAdminEndpoint",
    "iamEndpoint",
    "aiGwInferenceEndpoint",
    "typesafeBaseUrl",
];

/// Endpoint keys the TS schema validates with `z.string().url()`.
const TS_URL_VALIDATED_KEYS: &[&str] = &[
    "mgmtDashboardEndpoint",
    "agentGuardDataEndpoint",
    "agentGuardMgmtEndpoint",
    "typesafeBaseUrl",
];

/// Minimum length of a config value treated as a secret for leak checks.
const MIN_SECRET_LEN: usize = 8;

/// A tenant config with insertion-ordered keys (written as 2-space JSON + newline).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FixtureConfig {
    entries: Vec<(String, Value)>,
}

impl FixtureConfig {
    /// An empty config.
    pub fn new() -> Self {
        Self::default()
    }

    /// The standard fixture config for `tsg`: every endpoint key on `base_url`,
    /// `airsNumRetries: 0`, `mgmtClientId: client-<tsg>`,
    /// `mgmtClientSecret: FAKE-SECRET-<tsg>`, `airsApiKey: FAKE-APIKEY-<tsg>`.
    pub fn standard(base_url: &str, tsg: &str) -> Self {
        let base = base_url.trim_end_matches('/');
        let endpoint = |path: &str| Value::String(format!("{base}{path}"));
        let mut config = Self::new();
        config
            .set("airsApiKey", format!("FAKE-APIKEY-{tsg}"))
            .set("airsApiEndpoint", endpoint(paths::SCAN_PREFIX))
            .set("airsNumRetries", 0)
            .set("mgmtClientId", format!("client-{tsg}"))
            .set("mgmtClientSecret", format!("FAKE-SECRET-{tsg}"))
            .set("mgmtTsgId", tsg)
            .set("mgmtEndpoint", endpoint(paths::MGMT_PREFIX))
            .set("mgmtDashboardEndpoint", endpoint("/dashboard"))
            .set("mgmtTokenEndpoint", endpoint(paths::TOKEN))
            .set("dlpEndpoint", endpoint("/dlp"))
            .set("redTeamDataEndpoint", endpoint("/redteam/data"))
            .set("redTeamMgmtEndpoint", endpoint("/redteam/mgmt"))
            .set("redTeamNetworkBrokerEndpoint", endpoint("/redteam/broker"))
            .set("modelSecDataEndpoint", endpoint("/aims/data"))
            .set("modelSecMgmtEndpoint", endpoint("/aims/mgmt"))
            .set("agentGuardDataEndpoint", endpoint("/agentguard/data"))
            .set("agentGuardMgmtEndpoint", endpoint("/agentguard/mgmt"))
            .set("aiGwDataEndpoint", endpoint(paths::AIGW_DATA_PREFIX))
            .set("aiGwAdminEndpoint", endpoint("/ai_gw/admin/v2"))
            .set("iamEndpoint", endpoint("/iam"))
            .set("aiGwInferenceEndpoint", endpoint("/ai_gw/inference"))
            .set("typesafeBaseUrl", endpoint("/typesafe"));
        config
    }

    /// Sets `key` (in place when present, else appended).
    pub fn set(&mut self, key: &str, value: impl Into<Value>) -> &mut Self {
        let value = value.into();
        match self.entries.iter_mut().find(|(name, _)| name == key) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((key.to_owned(), value)),
        }
        self
    }

    /// Builder form of [`FixtureConfig::set`].
    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.set(key, value);
        self
    }

    /// Removes `key`, returning its value.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let index = self.entries.iter().position(|(name, _)| name == key)?;
        Some(self.entries.remove(index).1)
    }

    /// Builder form of [`FixtureConfig::remove`].
    pub fn without(mut self, key: &str) -> Self {
        self.remove(key);
        self
    }

    /// The value of `key`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// The string value of `key`.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// Entries in order.
    pub fn entries(&self) -> &[(String, Value)] {
        &self.entries
    }

    /// As a JSON object (keys sorted; use [`FixtureConfig::to_json`] for file bytes).
    pub fn to_value(&self) -> Value {
        Value::Object(self.entries.iter().cloned().collect::<Map<_, _>>())
    }

    /// A config from a JSON object (keys in `serde_json` map order).
    ///
    /// # Panics
    ///
    /// When `value` is not an object.
    pub fn from_value(value: &Value) -> Self {
        match value {
            Value::Object(map) => Self {
                entries: map.clone().into_iter().collect(),
            },
            other => panic!("a fixture config must be a JSON object, got {other}"),
        }
    }

    /// `JSON.stringify(config, null, 2) + "\n"` with the insertion key order.
    pub fn to_json(&self) -> String {
        js_pretty_object(&self.entries)
    }
}

/// 2-space pretty JSON of an ordered object, as `JSON.stringify(_, null, 2)` + newline.
fn js_pretty_object(entries: &[(String, Value)]) -> String {
    if entries.is_empty() {
        return "{}\n".into();
    }
    let body: Vec<String> = entries
        .iter()
        .map(|(key, value)| {
            let value = serde_json::to_string_pretty(value)
                .unwrap_or_else(|err| panic!("cannot serialise config value: {err}"))
                .replace('\n', "\n  ");
            format!("  {}: {value}", Value::String(key.clone()))
        })
        .collect();
    format!("{{\n{}\n}}\n", body.join(",\n"))
}

/// One registry entry (`{name, configPath, tsgId}`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryEntry {
    /// Tenant name.
    pub name: String,
    /// Config path as written (normally absolute).
    pub config_path: String,
    /// Registered TSG.
    pub tsg_id: String,
}

impl RegistryEntry {
    /// An entry from parts.
    pub fn new(name: &str, config_path: impl AsRef<Path>, tsg_id: &str) -> Self {
        Self {
            name: name.to_owned(),
            config_path: config_path.as_ref().display().to_string(),
            tsg_id: tsg_id.to_owned(),
        }
    }
}

/// A tenant written by [`Sandbox::add_fixture_tenant`].
#[derive(Clone, Debug)]
pub struct FixtureTenant {
    /// Tenant name.
    pub name: String,
    /// TSG.
    pub tsg: String,
    /// Absolute TypeScript config path.
    pub config_path: PathBuf,
    /// The airs-cli-rs tenant file this tenant is provisioned at
    /// (`store/tenants/<name>.toml`; it exists once the tenant is registered).
    pub store_path: PathBuf,
    /// The config as written.
    pub config: FixtureConfig,
}

impl FixtureTenant {
    /// The registry entry for this tenant (`tsgId` = its TSG).
    pub fn entry(&self) -> RegistryEntry {
        RegistryEntry::new(&self.name, &self.config_path, &self.tsg)
    }
}

/// A labelled secret form used by leak checks (`label` never contains the value).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretForm {
    /// What the value is, e.g. `prod mgmtClientSecret (base64)`.
    pub label: String,
    /// The text that must never appear in output.
    pub value: String,
}

/// Temp directory layout: `home/`, `state/` (registry at `state/tenants.json`),
/// `configs/`, `store/` (the airs-cli-rs store, `AIRS_CLI_CONFIG_DIR`), `cwd/` and
/// `tmp/`. Removed on drop.
///
/// **Both CLIs from the same fixture data.** While the sandbox *mirrors* (the default),
/// every write of a TypeScript registry or config re-derives `store/` from
/// `state/tenants.json` and the configs it names ([`crate::store::plan`]: the
/// `tenant import` conversion, with the failure mapping of DESIGN §10). A test that
/// writes the Rust store itself ([`Sandbox::write_store_raw`],
/// [`Sandbox::write_tenant_file_raw`], [`Sandbox::write_rust_tenant`],
/// [`Sandbox::write_store_index`]) or calls [`Sandbox::detach_store`] /
/// [`Sandbox::ts_only`] stops the mirroring, so Rust-only and TypeScript-only states
/// can be built.
pub struct Sandbox {
    _dir: TempDir,
    root: PathBuf,
    secrets: Mutex<Vec<SecretForm>>,
    /// Configs written through an `*_allowing_external_endpoints` method.
    external_endpoint_configs: Mutex<Vec<PathBuf>>,
    /// Whether `store/` is re-derived from the TypeScript files on every write.
    mirror: Mutex<bool>,
    /// Store tenant files exempt from the loopback guard (opt-outs).
    unguarded_store_files: Mutex<Vec<PathBuf>>,
    /// The base URL of the fixture server the last fixture tenant was written for: the
    /// TypeScript endpoint-variable trap lives there (DESIGN §10 M4).
    fixture_base: Mutex<Option<String>>,
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Sandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sandbox")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl Sandbox {
    /// Creates the directory tree (`state/` and `configs/` are mode 0700).
    pub fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("airs-harness-")
            .tempdir()
            .unwrap_or_else(|err| panic!("sandbox: cannot create temp dir: {err}"));
        // Canonical root so paths match what the CLIs resolve (macOS /var -> /private/var).
        let root = dir
            .path()
            .canonicalize()
            .unwrap_or_else(|err| panic!("sandbox: cannot canonicalise temp dir: {err}"));
        for (sub, mode) in [
            ("home", 0o700),
            ("state", 0o700),
            ("configs", 0o700),
            ("store", 0o700),
            ("cwd", 0o700),
            ("tmp", 0o700),
            // `XDG_DATA_HOME` of every run (the `--fetch-uv` tools directory, M6c).
            ("data", 0o700),
        ] {
            let path = root.join(sub);
            fs::create_dir(&path)
                .unwrap_or_else(|err| panic!("sandbox: cannot create {sub}/: {err}"));
            set_mode(&path, mode);
        }
        Self {
            _dir: dir,
            root,
            secrets: Mutex::new(vec![
                SecretForm {
                    label: "issued access token prefix".into(),
                    value: TOKEN_PREFIX.into(),
                },
                // Base64 of every issued token starts with this: the first 9 bytes of
                // the prefix are three whole base64 groups (URL-encoding changes nothing).
                SecretForm {
                    label: "issued access token prefix (base64)".into(),
                    value: base64_unpadded(&TOKEN_PREFIX.as_bytes()[..9]),
                },
            ]),
            external_endpoint_configs: Mutex::new(Vec::new()),
            mirror: Mutex::new(true),
            unguarded_store_files: Mutex::new(Vec::new()),
            fixture_base: Mutex::new(None),
        }
    }

    /// Where the runner points the TypeScript SDK endpoint variables of a `runtime` run
    /// (DESIGN §10 M4): `<fixture server>/ts-env-trap` once a fixture tenant was written
    /// for a server, else a closed loopback port (nothing answers, nothing leaves the
    /// host).
    pub fn ts_env_trap_url(&self) -> String {
        let base = self
            .fixture_base
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        match base {
            Some(base) => format!("{base}{}", paths::TS_ENV_TRAP),
            None => format!(
                "http://127.0.0.1:{}{}",
                crate::ClosedPort::new().port(),
                paths::TS_ENV_TRAP
            ),
        }
    }

    /// Sandbox root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `HOME` for spawned CLIs.
    pub fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    /// Directory holding the registry.
    pub fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    /// `PRISMA_AIRS_TENANTS_PATH` for spawned CLIs.
    pub fn registry_path(&self) -> PathBuf {
        self.state_dir().join("tenants.json")
    }

    /// Directory holding tenant configs.
    pub fn configs_dir(&self) -> PathBuf {
        self.root.join("configs")
    }

    /// The airs-cli-rs store directory `D` (`AIRS_CLI_CONFIG_DIR` for spawned CLIs).
    pub fn store_dir(&self) -> PathBuf {
        self.root.join("store")
    }

    /// `store/config.toml`.
    pub fn store_index_path(&self) -> PathBuf {
        self.store_dir().join("config.toml")
    }

    /// `store/tenants/<name>.toml`.
    pub fn store_tenant_path(&self, name: &str) -> PathBuf {
        crate::store::tenant_path(&self.store_dir(), name)
    }

    /// Whether `store/` still mirrors the TypeScript files.
    pub fn mirrors_store(&self) -> bool {
        *self.mirror.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Stop mirroring: `store/` keeps its current content and later TypeScript writes
    /// no longer change it.
    pub fn detach_store(&self) {
        *self.mirror.lock().unwrap_or_else(PoisonError::into_inner) = false;
    }

    /// A TypeScript-only state: empties `store/` (no `config.toml`) and stops mirroring.
    pub fn ts_only(&self) {
        self.detach_store();
        crate::store::clear(&self.store_dir());
    }

    /// Re-derive `store/` from the TypeScript registry and configs (while mirroring).
    /// Only files inside the sandbox root are read.
    pub fn sync_store(&self) {
        if !self.mirrors_store() {
            return;
        }
        let registry = fs::read(self.registry_path()).ok();
        let opted_out = self.external_endpoint_configs();
        let mut unguarded = Vec::new();
        let mut planned = crate::store::plan(registry.as_deref(), |config_path| {
            let resolved = fs::canonicalize(config_path).ok()?;
            if !resolved.starts_with(&self.root) {
                return None;
            }
            // TypeScript reads the canonical target; a non-regular one is invalid.
            let metadata = fs::metadata(&resolved).ok()?;
            if opted_out
                .iter()
                .any(|path| fs::canonicalize(path).ok().as_ref() == Some(&resolved))
            {
                unguarded.push(config_path.to_owned());
            }
            if !metadata.is_file() {
                return Some(b"<not a regular file>".to_vec());
            }
            fs::read(&resolved).ok()
        });
        planned.unguarded = unguarded;
        let registry_entries = registry
            .as_deref()
            .and_then(crate::store::ts_registry)
            .map(|(entries, _)| entries)
            .unwrap_or_default();
        let mut exempt = Vec::new();
        for (name, bytes) in &planned.tenants {
            let path = self.store_tenant_path(name);
            let opted = registry_entries
                .iter()
                .find(|entry| &entry.name == name)
                .is_some_and(|entry| planned.unguarded.contains(&entry.config_path));
            if opted {
                exempt.push(path);
            } else if let Some(problem) = crate::store::tenant_guard_violation(bytes.as_bytes()) {
                panic!("sandbox: provisioned tenant file '{name}' {problem}");
            }
        }
        crate::store::write_plan(&self.store_dir(), &planned);
        *self
            .unguarded_store_files
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = exempt;
    }

    /// Writes `store/config.toml` (raw bytes, `mode`) and stops mirroring.
    pub fn write_store_raw(&self, bytes: &[u8], mode: u32) -> PathBuf {
        self.detach_store();
        let path = self.store_index_path();
        crate::store::write_with(&path, bytes, mode);
        path
    }

    /// Writes the canonical `store/config.toml` for `tenants` (name, pinned TSG) and
    /// `active`, and stops mirroring.
    pub fn write_store_index(&self, tenants: &[(&str, &str)], active: Option<&str>) -> PathBuf {
        self.write_store_raw(crate::store::index_toml(tenants, active).as_bytes(), 0o600)
    }

    /// Writes `store/tenants/<name>.toml` (raw bytes, `mode`; `tenants/` created 0700)
    /// and stops mirroring. No secret capture.
    ///
    /// # Panics
    ///
    /// When the bytes parse as TOML and fail the Rust loopback guard
    /// ([`crate::store::tenant_guard_violation`]).
    pub fn write_tenant_file_raw(&self, name: &str, bytes: &[u8], mode: u32) -> PathBuf {
        if let Some(problem) = crate::store::tenant_guard_violation(bytes) {
            panic!(
                "sandbox: tenant file '{name}' {problem}. Every Rust endpoint key must point at \
                 the loopback fixture server; a Rust-only test that proves zero requests may \
                 opt out with write_tenant_file_allowing_external_endpoints"
            );
        }
        self.write_tenant_file_unchecked(name, bytes, mode)
    }

    /// **Opt-out of the loopback guard** for a Rust tenant file (Rust-only tests that
    /// prove zero requests). Records the file as exempt for the runner's check.
    pub fn write_tenant_file_allowing_external_endpoints(
        &self,
        name: &str,
        bytes: &[u8],
        mode: u32,
    ) -> PathBuf {
        let path = self.write_tenant_file_unchecked(name, bytes, mode);
        self.unguarded_store_files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(path.clone());
        path
    }

    fn write_tenant_file_unchecked(&self, name: &str, bytes: &[u8], mode: u32) -> PathBuf {
        self.detach_store();
        let path = self.store_tenant_path(name);
        crate::store::write_with(&path, bytes, mode);
        path
    }

    /// Writes `config` (TypeScript key names, as a [`FixtureConfig`]) converted to the
    /// canonical Rust tenant file, records its secrets and stops mirroring.
    ///
    /// # Panics
    ///
    /// When TypeScript would reject the config (there is no conversion), or the result
    /// fails the Rust loopback guard.
    pub fn write_rust_tenant(&self, name: &str, config: &FixtureConfig) -> PathBuf {
        self.record_config_secrets(name, config);
        let converted = crate::store::convert_config(config.to_json().as_bytes())
            .unwrap_or_else(|| panic!("sandbox: config '{name}' has no Rust conversion"));
        self.write_tenant_file_raw(name, converted.to_toml().as_bytes(), 0o600)
    }

    /// Explicit fixture-only app association, copied from the specified flat kind.
    /// Returns false when that kind/endpoint is absent (missing-credential tests).
    /// Does not alter the TypeScript config or weaken the runtime endpoint guard.
    pub fn bind_fixture_scanner(
        &self,
        tenant: &str,
        name: &str,
        application: &str,
        kind: &str,
    ) -> bool {
        let path = self.store_tenant_path(tenant);
        let mut bytes = fs::read_to_string(&path).expect("fixture tenant");
        let table: toml::Table = bytes.parse().expect("valid fixture TOML");
        let (flat, named) = match kind {
            "key" => ("airsApiKey", "apiKey"),
            "token" => ("airsApiToken", "apiToken"),
            _ => panic!("explicit key/token fixture kind required"),
        };
        let Some(secret) = table.get(flat).and_then(toml::Value::as_str) else {
            return false;
        };
        let Some(endpoint) = table.get("airsApiEndpoint").and_then(toml::Value::as_str) else {
            return false;
        };
        assert!(
            !table
                .get("scanners")
                .and_then(toml::Value::as_table)
                .is_some_and(|v| v.contains_key(name)),
            "fixture name already bound"
        );
        if !table.contains_key("scanners") {
            bytes.push_str("\n[scanners]\n");
        }
        bytes.push_str(&format!("\n[scanners.{}]\napplicationId = {}\nendpoint = {}\nnumRetries = 0\nrevision = \"00000000-0000-4000-8000-000000000001\"\n{named} = {}\n",crate::store::toml_string(name),crate::store::toml_string(application),crate::store::toml_string(endpoint),crate::store::toml_string(secret)));
        self.write_tenant_file_raw(tenant, bytes.as_bytes(), 0o600);
        true
    }

    /// Appends `line` (plus `\n`) to the provisioned `store/tenants/<name>.toml` (for a
    /// Rust-side state TypeScript cannot express: a TOML type, an unknown or renamed
    /// key, a dead `defaultOutput`) and stops mirroring. The file keeps mode 0600.
    ///
    /// # Panics
    ///
    /// When the tenant file does not exist.
    pub fn append_tenant_line(&self, name: &str, line: &str) -> PathBuf {
        let path = self.store_tenant_path(name);
        let mut bytes = fs::read(&path)
            .unwrap_or_else(|err| panic!("sandbox: no tenant file for '{name}': {err}"));
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
        let exempt = self.unguarded_store_files().contains(&path);
        if exempt {
            self.write_tenant_file_allowing_external_endpoints(name, &bytes, 0o600)
        } else {
            self.write_tenant_file_raw(name, &bytes, 0o600)
        }
    }

    /// Replaces `key = …` in the provisioned tenant file with `line` (or appends `line`
    /// when the key is absent); see [`Sandbox::append_tenant_line`].
    pub fn set_tenant_line(&self, name: &str, key: &str, line: &str) -> PathBuf {
        let path = self.store_tenant_path(name);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("sandbox: no tenant file for '{name}': {err}"));
        let kept: String = text
            .lines()
            .filter(|existing| {
                existing
                    .split_once('=')
                    .is_none_or(|(name, _)| name.trim() != key)
            })
            .map(|existing| format!("{existing}\n"))
            .collect();
        let exempt = self.unguarded_store_files().contains(&path);
        let bytes = format!("{kept}{line}\n");
        if exempt {
            self.write_tenant_file_allowing_external_endpoints(name, bytes.as_bytes(), 0o600)
        } else {
            self.write_tenant_file_raw(name, bytes.as_bytes(), 0o600)
        }
    }

    /// Paths of Rust tenant files exempt from the loopback guard.
    pub fn unguarded_store_files(&self) -> Vec<PathBuf> {
        self.unguarded_store_files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Panics unless every tenant file under `store` (a Rust run's store directory) that
    /// parses as TOML passes the Rust loopback guard or is a recorded opt-out. A store
    /// outside the sandbox root panics too (it is never read).
    pub fn assert_store_safe_for_rust(&self, store: &Path) {
        self.assert_store_safe_for_rust_with(store, true);
    }

    /// [`Sandbox::assert_store_safe_for_rust`]; with `check_endpoints` false (a `tenant`
    /// command, which builds no client: [`crate::is_tenant_command`]) only the store's
    /// location is checked.
    pub fn assert_store_safe_for_rust_with(&self, store: &Path, check_endpoints: bool) {
        let store = crate::runner::lexical(store);
        assert!(
            store.starts_with(&self.root),
            "refusing to run airs-cli-rs: its store {} is outside the sandbox",
            store.display()
        );
        if !check_endpoints {
            return;
        }
        let exempt = self.unguarded_store_files();
        let Ok(entries) = fs::read_dir(store.join("tenants")) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if exempt.contains(&path) || !entry.file_type().is_ok_and(|kind| kind.is_file()) {
                continue;
            }
            if let Ok(bytes) = fs::read(&path)
                && let Some(problem) = crate::store::tenant_guard_violation(&bytes)
            {
                panic!(
                    "refusing to run airs-cli-rs: tenant file {} {problem}",
                    path.display()
                );
            }
        }
    }

    /// Working directory for spawned CLIs.
    pub fn data_dir(&self) -> PathBuf {
        self.root.join("data")
    }

    /// `<root>/bin`: the fake tools of the `install` tests ([`crate::fake_uv`]).
    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }

    pub fn cwd(&self) -> PathBuf {
        self.root.join("cwd")
    }

    /// `TMPDIR` for spawned CLIs.
    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }

    /// `configs/<name>.json`.
    pub fn config_path(&self, name: &str) -> PathBuf {
        self.configs_dir().join(format!("{name}.json"))
    }

    /// Writes `configs/<name>.json` as 2-space JSON (mode 0400) and records its secret
    /// values for leak checks.
    ///
    /// # Panics
    ///
    /// When the config fails the loopback-endpoint guard ([`endpoint_guard_violation`]).
    pub fn write_config(&self, name: &str, config: &Value) -> PathBuf {
        self.write_fixture_config(name, &FixtureConfig::from_value(config))
    }

    /// Writes an ordered config (mode 0400) and records its secrets.
    ///
    /// # Panics
    ///
    /// When the config fails the loopback-endpoint guard ([`endpoint_guard_violation`]):
    /// every [`ENDPOINT_KEYS`] key must be present and name a loopback `http(s)` URL.
    /// Use [`Sandbox::write_config_allowing_external_endpoints`] for a Rust-only test that
    /// needs a missing or non-loopback endpoint.
    pub fn write_fixture_config(&self, name: &str, config: &FixtureConfig) -> PathBuf {
        self.record_config_secrets(name, config);
        self.write_config_raw(name, config.to_json().as_bytes())
    }

    /// Writes arbitrary bytes to `configs/<name>.json` (mode 0400); no secret capture.
    ///
    /// # Panics
    ///
    /// When the bytes decode (lossily, as Node does) to a JSON object that fails the
    /// loopback-endpoint guard ([`endpoint_guard_violation`]). Bytes that are not a JSON
    /// object (invalid JSON, `null`, an array, …) are accepted: neither CLI builds a
    /// client from them.
    pub fn write_config_raw(&self, name: &str, bytes: &[u8]) -> PathBuf {
        if let Some(problem) = endpoint_guard_violation(bytes) {
            panic!(
                "sandbox: config '{name}' {problem}. Every ENDPOINT_KEYS key must point at the \
                 loopback fixture server (an omitted key falls back to a production default); \
                 a Rust-only test that proves zero requests may opt out with \
                 write_config_allowing_external_endpoints"
            );
        }
        self.write_config_unchecked(name, bytes)
    }

    /// **Opt-out of the loopback-endpoint guard** for Rust-only tests that need a config
    /// with a missing or non-loopback endpoint (and prove zero requests). Records the
    /// config's secrets like [`Sandbox::write_fixture_config`]. Afterwards the runner
    /// refuses to start the TypeScript CLI in this sandbox.
    pub fn write_config_allowing_external_endpoints(
        &self,
        name: &str,
        config: &FixtureConfig,
    ) -> PathBuf {
        self.record_config_secrets(name, config);
        self.write_config_raw_allowing_external_endpoints(name, config.to_json().as_bytes())
    }

    /// Raw-bytes form of [`Sandbox::write_config_allowing_external_endpoints`] (no secret
    /// capture). Afterwards the runner refuses to start the TypeScript CLI here.
    pub fn write_config_raw_allowing_external_endpoints(
        &self,
        name: &str,
        bytes: &[u8],
    ) -> PathBuf {
        let path = self.config_path(name);
        self.external_endpoint_configs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(path.clone());
        self.write_config_unchecked(name, bytes)
    }

    /// Configs written through an `*_allowing_external_endpoints` method.
    pub fn external_endpoint_configs(&self) -> Vec<PathBuf> {
        self.external_endpoint_configs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn write_config_unchecked(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.config_path(name);
        write_with_mode(&path, bytes, 0o400);
        self.sync_store();
        path
    }

    /// Panics unless the TypeScript CLI may run here: no config was written through an
    /// `*_allowing_external_endpoints` method, and every config the CLI could load (each
    /// file under `configs/` and each `configPath` of the registry it would read, given
    /// the run's environment `env`) passes [`endpoint_guard_violation`]. A registry or
    /// config outside the sandbox root also panics (it is never read).
    pub fn assert_safe_for_ts(&self, env: &BTreeMap<String, String>) {
        self.assert_safe_for_ts_with(env, true);
    }

    /// [`Sandbox::assert_safe_for_ts`]; with `check_configs` false (a `tenant` command,
    /// which builds no client: [`crate::is_tenant_command`]) the environment guard still
    /// applies, and the registry and configs are only required to lie inside the
    /// sandbox, not to name loopback endpoints (a CLI-created config has none).
    pub fn assert_safe_for_ts_with(&self, env: &BTreeMap<String, String>, check_configs: bool) {
        let opted_out = self.external_endpoint_configs();
        assert!(
            !check_configs || opted_out.is_empty(),
            "refusing to run the TypeScript CLI: this sandbox holds configs written with \
             write_config_allowing_external_endpoints ({opted_out:?}); such tests must be \
             Rust-only"
        );
        if let Some(problem) = env_guard_violation(env) {
            panic!("refusing to run the TypeScript CLI: {problem}");
        }
        if check_configs {
            let mut configs = BTreeMap::new();
            walk(&self.root, &self.configs_dir(), &mut configs);
            for (relative, state) in &configs {
                if let EntryState::File { bytes, .. } = state
                    && let Some(problem) = endpoint_guard_violation(bytes)
                {
                    panic!(
                        "refusing to run the TypeScript CLI: {} {problem}",
                        relative.display()
                    );
                }
            }
        }
        let Some(registry) = ts_registry_path(env, &self.cwd()) else {
            return;
        };
        let Some(bytes) = self.read_inside(&registry, "registry") else {
            return;
        };
        let Ok(Value::Object(document)) =
            serde_json::from_str::<Value>(&String::from_utf8_lossy(&bytes))
        else {
            return;
        };
        let Some(Value::Array(tenants)) = document.get("tenants") else {
            return;
        };
        for tenant in tenants {
            let Some(config_path) = tenant.get("configPath").and_then(Value::as_str) else {
                continue;
            };
            let path = self.cwd().join(expand_home(config_path, env));
            // Every registered config must lie inside the sandbox (real tenant files are
            // never read); only commands that can send requests need loopback endpoints.
            if let Some(bytes) = self.read_inside(&path, "config")
                && check_configs
                && let Some(problem) = endpoint_guard_violation(&bytes)
            {
                panic!(
                    "refusing to run the TypeScript CLI: registered config {} {problem}",
                    path.display()
                );
            }
        }
    }

    /// Bytes of the regular file at `path` when it exists; panics when it resolves
    /// outside the sandbox root (real tenant files are never read).
    fn read_inside(&self, path: &Path, what: &str) -> Option<Vec<u8>> {
        let resolved = fs::canonicalize(path).ok()?;
        assert!(
            resolved.starts_with(&self.root),
            "refusing to run the TypeScript CLI: its {what} path resolves outside the sandbox"
        );
        fs::metadata(&resolved)
            .ok()
            .filter(fs::Metadata::is_file)
            .and_then(|_| fs::read(&resolved).ok())
    }

    /// Writes the registry in the TS `writeTestRegistry` format (mode 0600).
    pub fn write_registry(&self, entries: &[RegistryEntry], active: Option<&str>) {
        self.write_registry_raw(registry_json(entries, active).as_bytes());
    }

    /// Writes arbitrary registry bytes (mode 0600).
    pub fn write_registry_raw(&self, bytes: &[u8]) {
        write_with_mode(&self.registry_path(), bytes, 0o600);
        self.sync_store();
    }

    /// Writes any file under the sandbox root (parents created) with `mode`.
    pub fn write_file(&self, relative: &str, bytes: &[u8], mode: u32) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|err| panic!("sandbox: cannot create {}: {err}", parent.display()));
        }
        write_with_mode(&path, bytes, mode);
        if path.starts_with(self.store_dir()) {
            self.detach_store();
        } else {
            self.sync_store();
        }
        path
    }

    /// The standard fixture config for `tsg` on `server` (see [`FixtureConfig::standard`]).
    pub fn fixture_config(server: &FixtureServer, tsg: &str) -> FixtureConfig {
        FixtureConfig::standard(&server.base_url(), tsg)
    }

    /// Writes the standard config for `name`/`tsg` and registers its credentials on the
    /// server. Does not write the registry.
    pub fn add_fixture_tenant(
        &self,
        server: &FixtureServer,
        name: &str,
        tsg: &str,
    ) -> FixtureTenant {
        self.add_fixture_tenant_with(server, name, tsg, |_| {})
    }

    /// Like [`Sandbox::add_fixture_tenant`], letting `edit` change the config first; the
    /// server learns the credentials of the edited config.
    pub fn add_fixture_tenant_with(
        &self,
        server: &FixtureServer,
        name: &str,
        tsg: &str,
        edit: impl FnOnce(&mut FixtureConfig),
    ) -> FixtureTenant {
        self.add_edited_tenant(server, name, tsg, edit, false)
    }

    /// **Opt-out of the loopback-endpoint guard**: like
    /// [`Sandbox::add_fixture_tenant_with`], but the edited config may lack an endpoint
    /// key or point one away from loopback (written with
    /// [`Sandbox::write_config_allowing_external_endpoints`]). For Rust-only tests that
    /// prove zero requests; afterwards the runner refuses to start the TypeScript CLI in
    /// this sandbox.
    pub fn add_fixture_tenant_allowing_external_endpoints(
        &self,
        server: &FixtureServer,
        name: &str,
        tsg: &str,
        edit: impl FnOnce(&mut FixtureConfig),
    ) -> FixtureTenant {
        self.add_edited_tenant(server, name, tsg, edit, true)
    }

    fn add_edited_tenant(
        &self,
        server: &FixtureServer,
        name: &str,
        tsg: &str,
        edit: impl FnOnce(&mut FixtureConfig),
        allow_external_endpoints: bool,
    ) -> FixtureTenant {
        let mut config = Self::fixture_config(server, tsg);
        edit(&mut config);
        *self
            .fixture_base
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(server.base_url());
        server.add_tenant(
            tsg,
            config.get_str("mgmtClientId").unwrap_or_default(),
            config.get_str("mgmtClientSecret").unwrap_or_default(),
            config.get_str("airsApiKey"),
            config.get_str("airsApiToken"),
        );
        let config_path = if allow_external_endpoints {
            self.write_config_allowing_external_endpoints(name, &config)
        } else {
            self.write_fixture_config(name, &config)
        };
        FixtureTenant {
            name: name.to_owned(),
            tsg: tsg.to_owned(),
            config_path,
            store_path: self.store_tenant_path(name),
            config,
        }
    }

    /// Writes the registry for `tenants` with `active` selected.
    pub fn register(&self, tenants: &[&FixtureTenant], active: Option<&str>) {
        let entries: Vec<RegistryEntry> = tenants.iter().map(|tenant| tenant.entry()).collect();
        self.write_registry(&entries, active);
    }

    /// A Rust-only state: `store/` holds exactly `tenants` (converted from their
    /// fixture configs) with `active` selected, the TypeScript registry is left alone,
    /// and mirroring stops.
    pub fn rust_only(&self, tenants: &[&FixtureTenant], active: Option<&str>) {
        self.ts_only();
        for tenant in tenants {
            if self
                .external_endpoint_configs()
                .contains(&tenant.config_path)
            {
                let converted = crate::store::convert_config(tenant.config.to_json().as_bytes())
                    .unwrap_or_else(|| panic!("sandbox: '{}' has no Rust conversion", tenant.name));
                self.write_tenant_file_allowing_external_endpoints(
                    &tenant.name,
                    converted.to_toml().as_bytes(),
                    0o600,
                );
            } else {
                self.write_rust_tenant(&tenant.name, &tenant.config);
            }
        }
        let pins: Vec<(&str, &str)> = tenants
            .iter()
            .map(|tenant| (tenant.name.as_str(), tenant.tsg.as_str()))
            .collect();
        self.write_store_index(&pins, active);
    }

    /// One standard tenant (`name`, `tsg`) registered and active: the common setup.
    pub fn single_tenant(&self, server: &FixtureServer, name: &str, tsg: &str) -> FixtureTenant {
        let tenant = self.add_fixture_tenant(server, name, tsg);
        self.register(&[&tenant], Some(name));
        tenant
    }

    /// Adds an extra string that must never appear in CLI output (raw, URL-encoded and
    /// base64 forms are all checked).
    pub fn add_secret(&self, label: &str, value: &str) {
        let forms = secret_forms(label, value);
        self.lock_secrets().extend(forms);
    }

    /// Every secret form recorded so far, labelled.
    pub fn secret_forms(&self) -> Vec<SecretForm> {
        self.lock_secrets().clone()
    }

    /// Every secret form recorded so far (raw, URL-encoded, base64, Basic pairs, and the
    /// `mock-token-` prefix).
    pub fn secrets(&self) -> Vec<String> {
        self.lock_secrets()
            .iter()
            .map(|form| form.value.clone())
            .collect()
    }

    /// Captures bytes, mode and mtime of everything under `state/`, `configs/` and
    /// `store/`.
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot_with(&[])
    }

    /// Like [`Sandbox::snapshot`], also covering each of `extra` (a file or directory,
    /// relative to the sandbox root or absolute inside it, e.g. a registry under `home/`
    /// or an XDG directory). [`Sandbox::assert_unchanged`] re-captures the same set.
    ///
    /// # Panics
    ///
    /// When a path lies outside the sandbox root.
    pub fn snapshot_with(&self, extra: &[&Path]) -> Snapshot {
        let extra: Vec<PathBuf> = extra
            .iter()
            .map(|path| {
                let path = self.root.join(path);
                assert!(
                    path.starts_with(&self.root)
                        && !path
                            .components()
                            .any(|part| part == std::path::Component::ParentDir),
                    "snapshot_with: {} is outside the sandbox",
                    path.display()
                );
                path
            })
            .collect();
        let mut entries = BTreeMap::new();
        for sub in ["state", "configs", "store"] {
            walk(&self.root, &self.root.join(sub), &mut entries);
        }
        for path in &extra {
            walk(&self.root, path, &mut entries);
        }
        Snapshot { entries, extra }
    }

    /// Panics, naming the paths, when `state/`, `configs/` (or the extra paths of a
    /// [`Sandbox::snapshot_with`] snapshot) differ from `before` (content, mode, mtime,
    /// added or removed entries). Contents are never printed.
    pub fn assert_unchanged(&self, before: &Snapshot) {
        let extra: Vec<&Path> = before.extra.iter().map(PathBuf::as_path).collect();
        let after = self.snapshot_with(&extra);
        let problems = differences(before, &after);
        assert!(
            problems.is_empty(),
            "sandbox tenant files changed: {problems:?}"
        );
    }

    /// Bytes, mode and mtime of everything in the sandbox except `tmp/` (`TMPDIR`):
    /// the registry wherever a test put it (`state/`, `home/`, `cwd/`, an XDG
    /// directory, …), every config, and any file a CLI might create next to them. The
    /// runner takes it before every run and checks it after (DESIGN §10).
    pub fn full_snapshot(&self) -> Snapshot {
        full_snapshot_of(&self.root)
    }

    /// What changed under `root` since `before` (a [`Sandbox::full_snapshot`]).
    pub fn changes_under(root: &Path, before: &Snapshot) -> Vec<String> {
        differences(before, &full_snapshot_of(root))
    }

    /// Relative paths of every entry under `home/` (to see what a CLI created there).
    pub fn home_listing(&self) -> Vec<PathBuf> {
        let mut entries = BTreeMap::new();
        walk(&self.home(), &self.home(), &mut entries);
        entries
            .into_keys()
            .filter(|path| !path.as_os_str().is_empty())
            .collect()
    }

    /// Relative paths of every entry under `cwd/`.
    pub fn cwd_listing(&self) -> Vec<PathBuf> {
        let mut entries = BTreeMap::new();
        walk(&self.cwd(), &self.cwd(), &mut entries);
        entries
            .into_keys()
            .filter(|path| !path.as_os_str().is_empty())
            .collect()
    }

    fn lock_secrets(&self) -> std::sync::MutexGuard<'_, Vec<SecretForm>> {
        self.secrets.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn record_config_secrets(&self, name: &str, config: &FixtureConfig) {
        let mut forms = Vec::new();
        for (key, value) in config.entries() {
            if let Some(value) = value.as_str().filter(|_| is_secret_key(key)) {
                forms.extend(secret_forms(&format!("{name} {key}"), value));
            }
        }
        if let (Some(id), Some(secret)) = (
            config.get_str("mgmtClientId"),
            config.get_str("mgmtClientSecret"),
        ) && secret.len() >= MIN_SECRET_LEN
        {
            forms.push(SecretForm {
                label: format!("{name} Basic credentials (base64)"),
                value: base64_unpadded(format!("{id}:{secret}").as_bytes()),
            });
            forms.push(SecretForm {
                label: format!("{name} Basic credentials, form-encoded (base64)"),
                value: base64_unpadded(
                    format!("{}:{}", percent_encode(id), percent_encode(secret)).as_bytes(),
                ),
            });
        }
        let mut secrets = self.lock_secrets();
        for form in forms {
            if !secrets.contains(&form) {
                secrets.push(form);
            }
        }
    }
}

/// Why a config fails the loopback-endpoint guard, or `None` when it passes.
///
/// The bytes are decoded lossily (as Node's `readFileSync(path, 'utf8')` does), a
/// leading BOM is ignored and lone-surrogate escapes are tolerated (as `JSON.parse`
/// does). A document that is not a JSON object passes: neither CLI builds a client
/// from it. A JSON object passes only when every [`ENDPOINT_KEYS`] key is present, and
/// each is a loopback URL ([`is_loopback_url`]) or a value that makes both CLIs reject
/// the whole config before building any client: a non-string (TS `z.string()`), or a
/// string without `:` for a key TS validates with `z.string().url()`. An omitted key
/// (or, for the other keys, an empty string) would make either CLI fall back to a
/// production default.
pub fn endpoint_guard_violation(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let parsed = serde_json::from_str::<Value>(text)
        .or_else(|_| serde_json::from_str::<Value>(&replace_surrogate_escapes(text)));
    let Ok(Value::Object(map)) = parsed else {
        return None;
    };
    let mut missing = Vec::new();
    let mut external = Vec::new();
    for key in ENDPOINT_KEYS {
        match map.get(*key) {
            None => missing.push(*key),
            Some(Value::String(url)) if is_loopback_url(url) => {}
            // `new URL` needs a scheme, so TS's `z.string().url()` rejects the config.
            Some(Value::String(url))
                if TS_URL_VALIDATED_KEYS.contains(key) && !url.contains(':') => {}
            Some(Value::String(_)) => external.push(*key),
            // Not a string: both CLIs reject the whole config (TS `z.string()`).
            Some(_) => {}
        }
    }
    let mut problems = Vec::new();
    if !missing.is_empty() {
        problems.push(format!("lacks endpoint keys {}", missing.join(", ")));
    }
    if !external.is_empty() {
        problems.push(format!(
            "has endpoint keys that are not loopback http(s) URLs: {}",
            external.join(", ")
        ));
    }
    (!problems.is_empty()).then(|| problems.join("; "))
}

/// Why a TypeScript run's environment could send a request off loopback, or `None`.
///
/// The TS SDK reads endpoint overrides from the environment (`PANW_MGMT_ENDPOINT`,
/// `PANW_AI_SEC_API_ENDPOINT`, `PANW_*_TOKEN_ENDPOINT`, …), and Node can be told to use
/// a proxy (`HTTP_PROXY`, `https_proxy`, `ALL_PROXY`, …). Every variable whose name
/// contains `ENDPOINT`, or ends in `_PROXY` other than `NO_PROXY` (any case), must be
/// empty or a loopback URL ([`is_loopback_url`]). `NODE_OPTIONS` is refused outright: it
/// can preload code or change the network stack.
pub fn env_guard_violation(env: &BTreeMap<String, String>) -> Option<String> {
    let mut bad = Vec::new();
    for (name, value) in env {
        let upper = name.to_ascii_uppercase();
        let network =
            upper.contains("ENDPOINT") || (upper.ends_with("_PROXY") && upper != "NO_PROXY");
        if upper == "NODE_OPTIONS" || (network && !value.is_empty() && !is_loopback_url(value)) {
            bad.push(name.as_str());
        }
    }
    (!bad.is_empty()).then(|| {
        format!(
            "environment variables {} could send requests away from loopback",
            bad.join(", ")
        )
    })
}

/// `http://` or `https://` with host `localhost`, `127.0.0.0/8` or `[::1]`, an optional
/// numeric port, and no credentials, whitespace, control characters or backslashes.
/// Anything a URL parser could read differently (percent-encoded or shorthand hosts,
/// trailing dots) is rejected.
pub fn is_loopback_url(url: &str) -> bool {
    if url.chars().any(|c| c <= ' ' || c == '\u{7f}' || c == '\\') {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    let Some(rest) = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return false;
    }
    let (host, port) = match authority.strip_prefix('[') {
        Some(v6) => match v6.split_once(']') {
            Some((host, after)) => (host, after.strip_prefix(':')),
            None => return false,
        },
        None => match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        },
    };
    if port.is_some_and(|port| port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit())) {
        return false;
    }
    if authority.starts_with('[') {
        return host == "::1";
    }
    host == "localhost"
        || host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Replaces `\uD800`–`\uDFFF` escapes with `\uFFFD` so `serde_json` accepts what
/// `JSON.parse` accepts.
pub(crate) fn replace_surrogate_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find("\\u") {
        out.push_str(&rest[..index]);
        let escape = rest.get(index..index + 6).unwrap_or(&rest[index..]);
        let surrogate = escape.len() == 6
            && u16::from_str_radix(&escape[2..], 16)
                .is_ok_and(|unit| (0xD800..=0xDFFF).contains(&unit));
        if surrogate {
            out.push_str("\\uFFFD");
        } else {
            out.push_str(escape);
        }
        rest = &rest[index + escape.len()..];
    }
    out.push_str(rest);
    out
}

/// The registry path the TypeScript CLI resolves under `env` (`tenantStorePath`), or
/// `None` when it would refuse (relative `XDG_STATE_HOME`) or `HOME` is unset (Node
/// would use the password database). An empty `HOME` is `os.homedir() === ""`, so the
/// default path is relative to `cwd`.
pub(crate) fn ts_registry_path(env: &BTreeMap<String, String>, cwd: &Path) -> Option<PathBuf> {
    let non_empty = |name: &str| env.get(name).filter(|value| !value.is_empty());
    if let Some(explicit) = non_empty("PRISMA_AIRS_TENANTS_PATH") {
        return Some(cwd.join(expand_home(explicit, env)));
    }
    match non_empty("XDG_STATE_HOME") {
        Some(state) if !Path::new(state).is_absolute() => None,
        Some(state) => Some(Path::new(state).join("prisma-airs/tenants.json")),
        None => env
            .get("HOME")
            .map(|home| cwd.join(home).join(".local/state/prisma-airs/tenants.json")),
    }
}

/// TS `expandConfigPath`: `~` and `~/…` against `HOME`.
fn expand_home(path: &str, env: &BTreeMap<String, String>) -> PathBuf {
    let home = env.get("HOME").map(String::as_str).unwrap_or_default();
    if path == "~" {
        PathBuf::from(home)
    } else if let Some(rest) = path.strip_prefix("~/") {
        Path::new(home).join(rest)
    } else {
        PathBuf::from(path)
    }
}

/// `/(?:key|secret|token|password)$/i` on the key name (the TS secret-key rule).
pub fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    ["key", "secret", "token", "password"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

/// Raw, URL-encoded and (unpadded) base64 forms of `value`, and of its JS-trimmed form
/// when that differs (both CLIs trim scanner credentials before use). Values shorter
/// than 8 bytes are skipped: they would match ordinary output (`"   "`, `"s"`, …), so a
/// test that needs a short secret checked asserts on the output itself.
pub(crate) fn secret_forms(label: &str, value: &str) -> Vec<SecretForm> {
    let trimmed = js_trim(value);
    let mut forms = Vec::new();
    for (candidate, suffix) in [(value, ""), (trimmed, ", trimmed")] {
        if candidate.len() < MIN_SECRET_LEN || (!suffix.is_empty() && candidate == value) {
            continue;
        }
        forms.push(SecretForm {
            label: format!("{label}{suffix} (raw)"),
            value: candidate.to_owned(),
        });
        let encoded = percent_encode(candidate);
        if encoded != candidate {
            forms.push(SecretForm {
                label: format!("{label}{suffix} (url-encoded)"),
                value: encoded,
            });
        }
        forms.push(SecretForm {
            label: format!("{label}{suffix} (base64)"),
            value: base64_unpadded(candidate.as_bytes()),
        });
    }
    forms
}

/// `String.prototype.trim` (ECMAScript white space and line terminators).
fn js_trim(value: &str) -> &str {
    value.trim_matches(|c: char| {
        matches!(c, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
            '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' |
            '\u{3000}' | '\u{feff}')
    })
}

/// Standard base64 without `=` padding: a substring of the padded form, so output
/// that carries either is caught.
fn base64_unpadded(bytes: &[u8]) -> String {
    base64_encode(bytes).trim_end_matches('=').to_owned()
}

/// The registry document: `JSON.stringify({version, active, tenants}, null, 2) + "\n"`.
pub fn registry_json(entries: &[RegistryEntry], active: Option<&str>) -> String {
    // serde_json maps sort keys, so the TS key order is written by hand.
    let tenants_json = if entries.is_empty() {
        "[]".to_owned()
    } else {
        let items: Vec<String> = entries
            .iter()
            .map(|entry| {
                let ordered = [
                    ("name".to_owned(), Value::String(entry.name.clone())),
                    (
                        "configPath".to_owned(),
                        Value::String(entry.config_path.clone()),
                    ),
                    ("tsgId".to_owned(), Value::String(entry.tsg_id.clone())),
                ];
                format!(
                    "    {}",
                    js_pretty_object(&ordered)
                        .trim_end()
                        .replace('\n', "\n    ")
                )
            })
            .collect();
        format!("[\n{}\n  ]", items.join(",\n"))
    };
    let active = match active {
        Some(name) => Value::String(name.to_owned()).to_string(),
        None => "null".to_owned(),
    };
    format!("{{\n  \"version\": 1,\n  \"active\": {active},\n  \"tenants\": {tenants_json}\n}}\n")
}

/// State of one file-system entry in a [`Snapshot`].
#[derive(Clone, Debug, PartialEq, Eq)]
enum EntryState {
    Dir {
        mode: u32,
    },
    File {
        bytes: Vec<u8>,
        mode: u32,
        modified: Option<SystemTime>,
    },
    Symlink {
        target: PathBuf,
        modified: Option<SystemTime>,
    },
    Other,
}

/// Bytes, modes and mtimes of the sandbox tenant files at one point in time.
#[derive(Clone, PartialEq, Eq)]
pub struct Snapshot {
    entries: BTreeMap<PathBuf, EntryState>,
    extra: Vec<PathBuf>,
}

impl Snapshot {
    /// Relative paths captured, in order.
    pub fn paths(&self) -> Vec<PathBuf> {
        self.entries.keys().cloned().collect()
    }
}

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Snapshot")
            .field("paths", &self.paths())
            .finish()
    }
}

/// Every entry under `root` except `tmp/`.
fn full_snapshot_of(root: &Path) -> Snapshot {
    let mut entries = BTreeMap::new();
    if let Ok(children) = fs::read_dir(root) {
        for child in children.flatten() {
            if child.file_name() != "tmp" {
                walk(root, &child.path(), &mut entries);
            }
        }
    }
    Snapshot {
        entries,
        extra: Vec::new(),
    }
}

/// Removed, changed and added entries between two snapshots (paths only).
fn differences(before: &Snapshot, after: &Snapshot) -> Vec<String> {
    let mut problems = Vec::new();
    for (path, old) in &before.entries {
        match after.entries.get(path) {
            None => problems.push(format!("removed {}", path.display())),
            Some(new) if new != old => problems.push(format!("changed {}", path.display())),
            Some(_) => {}
        }
    }
    for path in after.entries.keys() {
        if !before.entries.contains_key(path) {
            problems.push(format!("added {}", path.display()));
        }
    }
    problems
}

fn walk(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, EntryState>) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
    let mode = metadata.permissions().mode() & 0o7777;
    let modified = metadata.modified().ok();
    let file_type = metadata.file_type();
    let state = if file_type.is_symlink() {
        EntryState::Symlink {
            target: fs::read_link(path).unwrap_or_default(),
            modified,
        }
    } else if file_type.is_dir() {
        if let Ok(children) = fs::read_dir(path) {
            for child in children.flatten() {
                walk(root, &child.path(), entries);
            }
        }
        EntryState::Dir { mode }
    } else if file_type.is_file() {
        EntryState::File {
            bytes: fs::read(path).unwrap_or_default(),
            mode,
            modified,
        }
    } else {
        EntryState::Other
    };
    entries.insert(relative, state);
}

fn set_mode(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .unwrap_or_else(|err| panic!("sandbox: cannot chmod {}: {err}", path.display()));
}

/// Writes `bytes` to `path` with `mode`, first making an existing read-only file writable.
fn write_with_mode(path: &Path, bytes: &[u8], mode: u32) {
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file()) {
        set_mode(path, 0o600);
    }
    fs::write(path, bytes)
        .unwrap_or_else(|err| panic!("sandbox: cannot write {}: {err}", path.display()));
    set_mode(path, mode);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_matches_ts_write_test_registry_format() {
        let entries = [RegistryEntry::new("prod", "/abs/prod.json", "100")];
        assert_eq!(
            registry_json(&entries, Some("prod")),
            "{\n  \"version\": 1,\n  \"active\": \"prod\",\n  \"tenants\": [\n    {\n      \"name\": \"prod\",\n      \"configPath\": \"/abs/prod.json\",\n      \"tsgId\": \"100\"\n    }\n  ]\n}\n"
        );
        assert_eq!(
            registry_json(&[], None),
            "{\n  \"version\": 1,\n  \"active\": null,\n  \"tenants\": []\n}\n"
        );
    }

    #[test]
    fn config_json_keeps_insertion_order() {
        let config = FixtureConfig::new()
            .with("b", 1)
            .with("a", serde_json::json!({"x": [1]}))
            .with("b", 2);
        assert_eq!(
            config.to_json(),
            "{\n  \"b\": 2,\n  \"a\": {\n    \"x\": [\n      1\n    ]\n  }\n}\n"
        );
        assert_eq!(FixtureConfig::new().to_json(), "{}\n");
    }

    #[test]
    fn ts_registry_path_models_an_empty_home() {
        let env = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect()
        };
        let cwd = Path::new("/sandbox/cwd");
        assert_eq!(
            ts_registry_path(&env(&[("HOME", "")]), cwd),
            Some(PathBuf::from(
                "/sandbox/cwd/.local/state/prisma-airs/tenants.json"
            ))
        );
        assert_eq!(
            ts_registry_path(&env(&[("HOME", "/h")]), cwd),
            Some(PathBuf::from("/h/.local/state/prisma-airs/tenants.json"))
        );
        assert_eq!(ts_registry_path(&env(&[]), cwd), None);
        assert_eq!(
            ts_registry_path(
                &env(&[("HOME", ""), ("PRISMA_AIRS_TENANTS_PATH", "~/r.json")]),
                cwd
            ),
            Some(PathBuf::from("/sandbox/cwd/r.json"))
        );
    }

    #[test]
    fn secret_key_rule() {
        for key in [
            "airsApiKey",
            "mgmtClientSecret",
            "airsApiToken",
            "typesafeApiKey",
        ] {
            assert!(is_secret_key(key), "{key}");
        }
        for key in ["mgmtClientId", "mgmtTsgId", "airsApiEndpoint"] {
            assert!(!is_secret_key(key), "{key}");
        }
    }
}
