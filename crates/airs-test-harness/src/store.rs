//! The airs-cli-rs TOML store for test sandboxes (DESIGN §3, §10 "M4t additions").
//!
//! [`provision`] derives the Rust store from the TypeScript registry and configs of a
//! sandbox with the `tenant import` conversion of DESIGN §3.12, so both CLIs see the
//! same tenants from the same fixture data. TypeScript files that import would not
//! convert are mapped so that both CLIs fail the same way:
//!
//! - a registry TypeScript rejects → `config.toml` is [`INVALID_TOML`] (the generic
//!   registry message on both sides);
//! - a missing config → no tenant file (`Cannot read a valid tenant config` on both);
//! - a config TypeScript rejects otherwise → the tenant file is [`INVALID_TOML`];
//! - a config that fails the TypeScript pin keeps both TSG values (the pin message on
//!   both).
//!
//! Everything here is pure (bytes in, bytes out) except [`provision`], which writes
//! only under the sandbox's `store/` and reads only files inside the sandbox root.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::sandbox::{is_loopback_url, replace_surrogate_escapes};

/// A TOML syntax error: the bytes provisioned for a file one CLI must reject.
pub const INVALID_TOML: &str = "invalid = [\n";

/// The 27 airs-cli-rs keys in schema order (DESIGN §3.3).
pub const RUST_KEYS: [&str; 27] = [
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
];

/// The 16 airs-cli-rs endpoint keys; the Rust loopback guard checks each.
pub const RUST_ENDPOINT_KEYS: [&str; 16] = [
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
    "supplychainDataEndpoint",
    "supplychainMgmtEndpoint",
    "gatewayDataEndpoint",
    "gatewayAdminEndpoint",
    "iamEndpoint",
    "gatewayInferenceEndpoint",
];

/// The keys whose read rule is "parses as a WHATWG URL" (DESIGN §3.3).
pub const URL_RULE_KEYS: [&str; 3] = [
    "mgmtDashboardEndpoint",
    "supplychainDataEndpoint",
    "supplychainMgmtEndpoint",
];

/// Renamed keys, TypeScript → airs-cli-rs.
pub const RENAMES: [(&str, &str); 8] = [
    ("aiGwDataEndpoint", "gatewayDataEndpoint"),
    ("aiGwAdminEndpoint", "gatewayAdminEndpoint"),
    ("aiGwInferenceEndpoint", "gatewayInferenceEndpoint"),
    ("aiGwInferenceApiKey", "gatewayInferenceApiKey"),
    ("aiGwInferenceModel", "gatewayInferenceModel"),
    ("aiGwEmbeddingModel", "gatewayEmbeddingModel"),
    ("agentGuardDataEndpoint", "supplychainDataEndpoint"),
    ("agentGuardMgmtEndpoint", "supplychainMgmtEndpoint"),
];

/// TypeScript keys airs-cli-rs dropped.
pub const DROPPED: [&str; 4] = [
    "dataDir",
    "typesafeApiKey",
    "typesafeBaseUrl",
    "typesafeModel",
];

/// The airs-cli-rs name of a TypeScript key, unchanged when it was not renamed.
pub fn rust_key(ts_key: &str) -> &str {
    RENAMES
        .iter()
        .find(|(old, _)| *old == ts_key)
        .map_or(ts_key, |(_, new)| new)
}

/// A TOML basic string (DESIGN §3.2).
pub fn toml_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A converted value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreValue {
    Text(String),
    Integer(i64),
}

impl StoreValue {
    fn to_toml(&self) -> String {
        match self {
            Self::Text(text) => toml_string(text),
            Self::Integer(value) => value.to_string(),
        }
    }
}

/// Canonical tenant-file bytes for `values` (any order; written in schema order).
pub fn tenant_toml(values: &[(String, StoreValue)]) -> String {
    let mut out = String::new();
    for key in RUST_KEYS {
        if let Some((_, value)) = values.iter().find(|(name, _)| name == key) {
            out.push_str(&format!("{key} = {}\n", value.to_toml()));
        }
    }
    out
}

/// Canonical `config.toml` bytes (DESIGN §3.2): names in byte order.
pub fn index_toml(tenants: &[(&str, &str)], active: Option<&str>) -> String {
    let mut out = String::from("version = 1\n");
    if let Some(active) = active {
        out.push_str(&format!("active = {}\n", toml_string(active)));
    }
    let sorted: BTreeMap<&str, &str> = tenants.iter().copied().collect();
    for (name, tsg) in sorted {
        out.push_str(&format!(
            "\n[tenants.{name}]\ntsg_id = {}\n",
            toml_string(tsg)
        ));
    }
    out
}

// -- The TypeScript side, as TypeScript reads it ----------------------------------

/// What `readFileSync(path, 'utf8')` + `JSON.parse` give: lossy UTF-8, lone surrogate
/// escapes tolerated; `None` when JSON.parse would throw (a BOM included).
fn ts_json(bytes: &[u8]) -> Option<Value> {
    let text = String::from_utf8_lossy(bytes);
    serde_json::from_str::<Value>(&text)
        .or_else(|_| serde_json::from_str::<Value>(&replace_surrogate_escapes(&text)))
        .ok()
}

/// `^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$`
fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
}

/// A registry entry as TypeScript accepts it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TsEntry {
    pub name: String,
    pub config_path: String,
    pub tsg_id: String,
}

/// The TypeScript registry schema (DESIGN §3.13 without the D27/D44 rejections):
/// `Some((entries, active))` when TypeScript accepts the bytes.
pub fn ts_registry(bytes: &[u8]) -> Option<(Vec<TsEntry>, Option<String>)> {
    if bytes.len() > 1_048_576 {
        return None;
    }
    let Value::Object(map) = ts_json(bytes)? else {
        return None;
    };
    if !map
        .keys()
        .all(|key| matches!(key.as_str(), "version" | "active" | "tenants"))
        || map.get("version").and_then(Value::as_f64) != Some(1.0)
    {
        return None;
    }
    let active = match map.get("active")? {
        Value::Null => None,
        Value::String(name) => Some(name.clone()),
        _ => return None,
    };
    let items = map.get("tenants")?.as_array()?;
    if items.len() > 1000 {
        return None;
    }
    let mut entries: Vec<TsEntry> = Vec::new();
    for item in items {
        let Value::Object(fields) = item else {
            return None;
        };
        if !fields
            .keys()
            .all(|key| matches!(key.as_str(), "name" | "configPath" | "tsgId"))
        {
            return None;
        }
        let name = fields.get("name")?.as_str()?;
        let config_path = fields.get("configPath")?.as_str()?;
        let tsg_id = fields.get("tsgId")?.as_str()?;
        if !valid_name(name)
            || !config_path.starts_with('/')
            || tsg_id.is_empty()
            || entries.iter().any(|entry| entry.name == name)
        {
            return None;
        }
        entries.push(TsEntry {
            name: name.to_owned(),
            config_path: config_path.to_owned(),
            tsg_id: tsg_id.to_owned(),
        });
    }
    if let Some(active) = &active
        && !entries.iter().any(|entry| &entry.name == active)
    {
        return None;
    }
    Some((entries, active))
}

fn js_trim(text: &str) -> &str {
    text.trim_matches(|c: char| {
        matches!(c, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
            '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' |
            '\u{3000}' | '\u{feff}')
    })
}

/// JavaScript `Number(value)`; `None` is NaN.
pub fn js_number(value: &Value) -> Option<f64> {
    match value {
        Value::Null => Some(0.0),
        Value::Bool(flag) => Some(f64::from(u8::from(*flag))),
        Value::Number(number) => number.as_f64(),
        Value::String(text) => js_string_number(text),
        Value::Array(items) => match items.as_slice() {
            [] => Some(0.0),
            [Value::Null] => Some(0.0),
            [Value::Bool(_) | Value::Object(_)] => None,
            [item] => js_number(item),
            _ => None,
        },
        Value::Object(_) => None,
    }
}

fn js_string_number(text: &str) -> Option<f64> {
    let text = js_trim(text);
    if text.is_empty() {
        return Some(0.0);
    }
    for (prefix, radix) in [
        ("0x", 16),
        ("0X", 16),
        ("0o", 8),
        ("0O", 8),
        ("0b", 2),
        ("0B", 2),
    ] {
        if let Some(digits) = text.strip_prefix(prefix) {
            if digits.is_empty() {
                return None;
            }
            let mut value = 0.0_f64;
            for c in digits.chars() {
                value = value * f64::from(radix) + f64::from(c.to_digit(radix)?);
            }
            return Some(value);
        }
    }
    let (sign, unsigned) = match text.as_bytes()[0] {
        b'+' => (1.0, &text[1..]),
        b'-' => (-1.0, &text[1..]),
        _ => (1.0, text),
    };
    if unsigned == "Infinity" {
        return Some(sign * f64::INFINITY);
    }
    let decimal = {
        let bytes = unsigned.as_bytes();
        let mut index = 0;
        let digits = |index: &mut usize| {
            let start = *index;
            while *index < bytes.len() && bytes[*index].is_ascii_digit() {
                *index += 1;
            }
            *index - start
        };
        let integer = digits(&mut index);
        let mut fraction = 0;
        if index < bytes.len() && bytes[index] == b'.' {
            index += 1;
            fraction = digits(&mut index);
        }
        let mut ok = integer + fraction > 0;
        if ok && index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
            index += 1;
            if index < bytes.len() && matches!(bytes[index], b'+' | b'-') {
                index += 1;
            }
            ok = digits(&mut index) > 0;
        }
        ok && index == bytes.len()
    };
    if !decimal {
        return None;
    }
    unsigned.parse::<f64>().ok().map(|value| sign * value)
}

fn coerced(value: &Value, min: i64, max: i64) -> Option<i64> {
    let number = js_number(value)?;
    if !number.is_finite() || number.fract() != 0.0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    ((min as f64..=max as f64).contains(&number)).then_some(number as i64)
}

/// The TypeScript schema keys and their rule kinds.
fn ts_rule(key: &str) -> Option<&'static str> {
    Some(match key {
        "airsNumRetries" => "retries",
        "scanConcurrency" => "concurrency",
        "defaultOutput" => "output",
        "mgmtDashboardEndpoint"
        | "agentGuardDataEndpoint"
        | "agentGuardMgmtEndpoint"
        | "typesafeBaseUrl" => "url",
        "airsApiKey"
        | "airsApiToken"
        | "airsApiEndpoint"
        | "mgmtClientId"
        | "mgmtClientSecret"
        | "mgmtTsgId"
        | "mgmtEndpoint"
        | "mgmtTokenEndpoint"
        | "dlpEndpoint"
        | "redTeamDataEndpoint"
        | "redTeamMgmtEndpoint"
        | "redTeamNetworkBrokerEndpoint"
        | "modelSecDataEndpoint"
        | "modelSecMgmtEndpoint"
        | "aiGwDataEndpoint"
        | "aiGwAdminEndpoint"
        | "iamEndpoint"
        | "aiGwInferenceEndpoint"
        | "aiGwInferenceApiKey"
        | "aiGwInferenceModel"
        | "aiGwEmbeddingModel"
        | "typesafeApiKey"
        | "typesafeModel"
        | "dataDir" => "text",
        _ => return None,
    })
}

/// A config TypeScript accepts, converted for airs-cli-rs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Converted {
    /// The airs-cli-rs values (schema keys only).
    pub values: Vec<(String, StoreValue)>,
    /// The raw TypeScript `mgmtTsgId` string, when present (the TS pin input).
    pub raw_tsg: Option<String>,
}

impl Converted {
    /// The canonical tenant-file bytes.
    pub fn to_toml(&self) -> String {
        tenant_toml(&self.values)
    }
}

/// Validate a TypeScript config as TypeScript does and apply the import conversion
/// (DESIGN §3.12). `None` when TypeScript rejects it.
pub fn convert_config(bytes: &[u8]) -> Option<Converted> {
    if bytes.len() > 1_048_576 {
        return None;
    }
    let Value::Object(map) = ts_json(bytes)? else {
        return None;
    };
    let mut values = Vec::new();
    for (key, value) in &map {
        let Some(kind) = ts_rule(key) else {
            continue;
        };
        let converted = match kind {
            "retries" => StoreValue::Integer(coerced(value, 0, 5)?),
            "concurrency" => StoreValue::Integer(coerced(value, 1, 20)?),
            "output" => {
                let text = value.as_str()?;
                match text {
                    "pretty" | "table" | "json" => StoreValue::Text(text.to_owned()),
                    // Accepted by TypeScript, dropped by import (D51).
                    "markdown" | "csv" | "yaml" => continue,
                    _ => return None,
                }
            }
            "url" => {
                let text = value.as_str()?;
                url::Url::parse(text).ok()?;
                StoreValue::Text(text.to_owned())
            }
            _ => StoreValue::Text(value.as_str()?.to_owned()),
        };
        if DROPPED.contains(&key.as_str()) {
            continue;
        }
        // TypeScript treats "" as unset; import drops it.
        if value.as_str() == Some("") {
            continue;
        }
        values.push((rust_key(key).to_owned(), converted));
    }
    Some(Converted {
        values,
        raw_tsg: map
            .get("mgmtTsgId")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// Why a tenant file that parses as TOML fails the Rust loopback guard, or `None`.
/// Every [`RUST_ENDPOINT_KEYS`] key must be present with a loopback `http(s)` URL or a
/// value the Rust reader rejects (a non-string or an empty string).
pub fn tenant_guard_violation(bytes: &[u8]) -> Option<String> {
    let table = std::str::from_utf8(bytes)
        .ok()?
        .parse::<toml::Table>()
        .ok()?;
    let mut missing = Vec::new();
    let mut external = Vec::new();
    for key in RUST_ENDPOINT_KEYS {
        match table.get(key) {
            None => missing.push(key),
            Some(toml::Value::String(url)) if url.is_empty() || is_loopback_url(url) => {}
            // The three URL-rule keys reject a value that is not a WHATWG URL.
            Some(toml::Value::String(url))
                if URL_RULE_KEYS.contains(&key) && url::Url::parse(url).is_err() => {}
            Some(toml::Value::String(_)) => external.push(key.to_owned()),
            Some(_) => {}
        }
    }
    if let Some(scanners) = table.get("scanners").and_then(toml::Value::as_table) {
        for (name, value) in scanners {
            if let Some(endpoint) = value
                .as_table()
                .and_then(|v| v.get("endpoint"))
                .and_then(toml::Value::as_str)
                && !endpoint.is_empty()
                && !is_loopback_url(endpoint)
            {
                external.push(format!("scanners.{name}.endpoint"));
            }
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

/// The provisioned store, before it is written.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// `config.toml` bytes; `None`: no `config.toml`.
    pub index: Option<String>,
    /// Tenant files: name → bytes.
    pub tenants: BTreeMap<String, String>,
    /// Tenant files derived from configs written with an external-endpoint opt-out.
    pub unguarded: Vec<String>,
}

/// Derive the Rust store from a TypeScript registry's bytes (`None`: no registry) and a
/// reader for config paths (`None`: the config does not exist or is outside the
/// sandbox, so it is never read).
pub fn plan(registry: Option<&[u8]>, mut read_config: impl FnMut(&str) -> Option<Vec<u8>>) -> Plan {
    let Some(bytes) = registry else {
        return Plan::default();
    };
    let Some((entries, active)) = ts_registry(bytes) else {
        return Plan {
            index: Some(INVALID_TOML.to_owned()),
            ..Plan::default()
        };
    };
    let mut tenants = BTreeMap::new();
    for entry in &entries {
        let Some(config) = read_config(&entry.config_path) else {
            continue;
        };
        let file = match convert_config(&config) {
            Some(converted) => converted.to_toml(),
            None => INVALID_TOML.to_owned(),
        };
        tenants.insert(entry.name.clone(), file);
    }
    let pins: Vec<(&str, &str)> = entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry.tsg_id.as_str()))
        .collect();
    Plan {
        index: Some(index_toml(&pins, active.as_deref())),
        tenants,
        unguarded: Vec::new(),
    }
}

/// Replace everything under `store` with `plan` (files 0600, directories 0700).
pub fn write_plan(store: &Path, plan: &Plan) {
    clear(store);
    if let Some(index) = &plan.index {
        write_private(&store.join("config.toml"), index.as_bytes());
    }
    for (name, bytes) in &plan.tenants {
        write_private(&tenant_path(store, name), bytes.as_bytes());
    }
}

/// `store/tenants/<name>.toml`.
pub fn tenant_path(store: &Path, name: &str) -> PathBuf {
    store.join("tenants").join(format!("{name}.toml"))
}

/// Remove every entry inside `store` (the directory itself stays).
pub fn clear(store: &Path) {
    let Ok(entries) = fs::read_dir(store) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_dir());
        let removed = if is_dir {
            make_writable(&path);
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        removed.unwrap_or_else(|err| panic!("store: cannot remove {}: {err}", path.display()));
    }
}

fn make_writable(dir: &Path) {
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                make_writable(&entry.path());
            }
        }
    }
}

/// Write `bytes` at `path` with mode 0600, creating parents 0700.
pub fn write_private(path: &Path, bytes: &[u8]) {
    write_with(path, bytes, 0o600);
}

/// Write `bytes` at `path` with `mode` (an existing file is replaced, not followed),
/// creating missing parents 0700.
pub fn write_with(path: &Path, bytes: &[u8], mode: u32) {
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("store: cannot create {}: {err}", parent.display()));
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|err| panic!("store: cannot chmod {}: {err}", parent.display()));
    }
    if fs::symlink_metadata(path).is_ok() {
        fs::remove_file(path)
            .unwrap_or_else(|err| panic!("store: cannot replace {}: {err}", path.display()));
    }
    fs::write(path, bytes)
        .unwrap_or_else(|err| panic!("store: cannot write {}: {err}", path.display()));
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .unwrap_or_else(|err| panic!("store: cannot chmod {}: {err}", path.display()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn convert(value: Value) -> Option<Converted> {
        convert_config(value.to_string().as_bytes())
    }

    #[test]
    fn conversion_renames_drops_and_coerces() {
        let converted = convert(json!({
            "mgmtTsgId": "100",
            "aiGwDataEndpoint": "http://127.0.0.1:1/gw",
            "agentGuardMgmtEndpoint": "http://127.0.0.1:1/ag",
            "dataDir": "/d",
            "typesafeApiKey": "t",
            "airsNumRetries": "0x2",
            "scanConcurrency": [4],
            "defaultOutput": "markdown",
            "airsApiKey": "",
            "custom": 1,
            "redTeamTokenEndpoint": "x"
        }))
        .unwrap();
        assert_eq!(
            converted.to_toml(),
            "airsNumRetries = 2\nmgmtTsgId = \"100\"\nsupplychainMgmtEndpoint = \"http://127.0.0.1:1/ag\"\ngatewayDataEndpoint = \"http://127.0.0.1:1/gw\"\nscanConcurrency = 4\n"
        );
        assert_eq!(converted.raw_tsg.as_deref(), Some("100"));
        assert!(convert(json!({"scanConcurrency": 99})).is_none());
        assert!(convert(json!({"defaultOutput": "xml"})).is_none());
        assert!(convert(json!({"airsApiKey": 1})).is_none());
        assert!(convert(json!({"mgmtDashboardEndpoint": "not a url"})).is_none());
        assert!(convert(json!([])).is_none());
        assert!(
            convert_config(b"\xef\xbb\xbf{}").is_none(),
            "JSON.parse rejects a BOM"
        );
        assert!(
            convert_config(b"{\"airsApiKey\":\"\xff\"}").is_some(),
            "lossy UTF-8"
        );
        assert!(
            convert_config(br#"{"airsApiKey":"\ud800"}"#).is_some(),
            "lone surrogate"
        );
    }

    #[test]
    fn plans_map_every_failure_class() {
        assert_eq!(plan(None, |_| None), Plan::default());
        assert_eq!(
            plan(Some(b"{"), |_| None).index.as_deref(),
            Some(INVALID_TOML)
        );
        let registry = json!({
            "version": 1,
            "active": "b",
            "tenants": [
                {"name": "b", "configPath": "/c/b.json", "tsgId": "1"},
                {"name": "a", "configPath": "/c/a.json", "tsgId": "2"},
                {"name": "m", "configPath": "/c/missing.json", "tsgId": "3"},
                {"name": "x", "configPath": "/c/x.json", "tsgId": "4"}
            ]
        })
        .to_string();
        let planned = plan(Some(registry.as_bytes()), |path| match path {
            "/c/b.json" => Some(br#"{"mgmtTsgId":"1"}"#.to_vec()),
            "/c/a.json" => Some(br#"{"mgmtTsgId":"other"}"#.to_vec()),
            "/c/x.json" => Some(b"[]".to_vec()),
            _ => None,
        });
        assert_eq!(
            planned.index.as_deref(),
            Some(
                "version = 1\nactive = \"b\"\n\n[tenants.a]\ntsg_id = \"2\"\n\n[tenants.b]\ntsg_id = \"1\"\n\n[tenants.m]\ntsg_id = \"3\"\n\n[tenants.x]\ntsg_id = \"4\"\n"
            )
        );
        assert_eq!(planned.tenants["b"], "mgmtTsgId = \"1\"\n");
        assert_eq!(
            planned.tenants["a"], "mgmtTsgId = \"other\"\n",
            "both TSGs kept"
        );
        assert!(
            !planned.tenants.contains_key("m"),
            "a missing config has no file"
        );
        assert_eq!(planned.tenants["x"], INVALID_TOML);
    }

    #[test]
    fn toml_strings_escape_controls() {
        assert_eq!(toml_string("a\"\\\u{1b}\n"), "\"a\\\"\\\\\\u001B\\n\"");
    }

    #[test]
    fn tenant_guard() {
        let good: String = RUST_ENDPOINT_KEYS
            .iter()
            .map(|key| format!("{key} = \"http://127.0.0.1:9/x\"\n"))
            .collect();
        assert_eq!(tenant_guard_violation(good.as_bytes()), None);
        assert_eq!(tenant_guard_violation(INVALID_TOML.as_bytes()), None);
        let missing = good.replace("iamEndpoint = \"http://127.0.0.1:9/x\"\n", "");
        assert!(
            tenant_guard_violation(missing.as_bytes())
                .unwrap()
                .contains("iamEndpoint")
        );
        let external = good.replace(
            "iamEndpoint = \"http://127.0.0.1:9/x\"",
            "iamEndpoint = \"https://example.com\"",
        );
        assert!(
            tenant_guard_violation(external.as_bytes())
                .unwrap()
                .contains("not loopback")
        );
        let rejected = good.replace("iamEndpoint = \"http://127.0.0.1:9/x\"", "iamEndpoint = 5");
        assert_eq!(tenant_guard_violation(rejected.as_bytes()), None);
        let named = format!("{good}\n[scanners.monitor]\nendpoint = \"http://127.0.0.1:9/scan\"\n");
        assert_eq!(tenant_guard_violation(named.as_bytes()), None);
        let named = named.replace("http://127.0.0.1:9/scan", "https://example.com/scan");
        assert!(
            tenant_guard_violation(named.as_bytes())
                .unwrap()
                .contains("scanners.monitor.endpoint")
        );
    }
}
