//! Tenant files (`D/tenants/<name>.toml`): the 27-key schema, the strict TOML read
//! rules, the effective view, redaction, the credential checks and the canonical
//! writer (DESIGN §3.3, §3.5, §3.6).

use super::scanners::{self, ScannerCredential, ScannerPublic};
use crate::error::CliError;
use crate::store::{PROGRAM, toml_string};
use serde::{Serialize, Serializer};
use std::collections::BTreeMap;

/// The 27 schema keys, in schema order (`tenant read`, the canonical bytes and the
/// unknown-key message, D71).
pub const KEYS: [&str; 27] = [
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

/// The TypeScript names of the renamed keys (§3.3), old → new.
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

/// TypeScript keys airs dropped (§3.3).
pub const DROPPED_KEYS: [&str; 4] = [
    "dataDir",
    "typesafeApiKey",
    "typesafeBaseUrl",
    "typesafeModel",
];

pub const DEFAULT_SCAN_CONCURRENCY: i64 = 5;

/// Management credentials, in check order.
pub const MANAGEMENT_KEYS: [&str; 3] = ["mgmtClientId", "mgmtClientSecret", "mgmtTsgId"];
/// Scanner credentials; either one suffices.
pub const SCANNER_KEYS: [&str; 2] = ["airsApiKey", "airsApiToken"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    Text,
    Url,
    Retries,
    Concurrency,
    Output,
}

fn rule(key: &str) -> Rule {
    match key {
        "airsNumRetries" => Rule::Retries,
        "scanConcurrency" => Rule::Concurrency,
        "defaultOutput" => Rule::Output,
        "mgmtDashboardEndpoint" | "supplychainDataEndpoint" | "supplychainMgmtEndpoint" => {
            Rule::Url
        }
        _ => Rule::Text,
    }
}

/// The schema key with this exact name.
pub fn schema_key(name: &str) -> Option<&'static str> {
    KEYS.iter().copied().find(|key| *key == name)
}

pub fn read_schema_key(name: &str) -> Option<&'static str> {
    if name == "scanners" {
        Some("scanners")
    } else if name == "mcp" {
        Some("mcp")
    } else {
        schema_key(name)
    }
}

/// Whether an unknown key is a TypeScript name (renamed, dropped or a retired
/// `*TokenEndpoint` key), for doctor's hint (§9).
pub fn is_typescript_name(key: &str) -> bool {
    key.starts_with("aiGw")
        || key.starts_with("agentGuard")
        || key.starts_with("typesafe")
        || key == "dataDir"
        || key.ends_with("TokenEndpoint")
}

/// `/(?:key|secret|token|password)$/i` on the key name.
pub fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    ["key", "secret", "token", "password"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

/// The 16 keys whose name ends in `Endpoint`.
pub fn is_endpoint_key(key: &str) -> bool {
    key.ends_with("Endpoint")
}

/// ECMAScript `WhiteSpace` and `LineTerminator` code points (`String.prototype.trim`).
pub fn is_js_whitespace(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
        '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' |
        '\u{3000}' | '\u{feff}')
}

pub fn js_trim(value: &str) -> &str {
    value.trim_matches(is_js_whitespace)
}

/// A validated value: a TOML string or integer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigValue {
    Text(String),
    Integer(i64),
}

impl ConfigValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Integer(_) => None,
        }
    }
    /// `String(value)` for pretty printing.
    pub fn display(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Integer(value) => value.to_string(),
        }
    }
    /// The canonical TOML value (§3.3).
    pub fn to_toml(&self) -> String {
        match self {
            Self::Text(text) => toml_string(text),
            Self::Integer(value) => value.to_string(),
        }
    }
}

impl Serialize for ConfigValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Text(text) => serializer.serialize_str(text),
            Self::Integer(value) => serializer.serialize_i64(*value),
        }
    }
}

/// The §3.3 read rule for `key`. `None` when the value is invalid.
pub fn read_rule(key: &str, value: &toml::Value) -> Option<ConfigValue> {
    let text = || match value {
        toml::Value::String(text) if !text.is_empty() => Some(text.clone()),
        _ => None,
    };
    let integer = |min: i64, max: i64| match value {
        toml::Value::Integer(number) if (min..=max).contains(number) => {
            Some(ConfigValue::Integer(*number))
        }
        _ => None,
    };
    match rule(key) {
        Rule::Text => text().map(ConfigValue::Text),
        Rule::Url => text()
            .filter(|text| url::Url::parse(text).is_ok())
            .map(ConfigValue::Text),
        Rule::Output => text()
            .filter(|text| crate::output_format::Format::parse(text).is_some())
            .map(ConfigValue::Text),
        Rule::Retries => integer(0, 5),
        Rule::Concurrency => integer(1, 20),
    }
}

/// `Invalid value for configuration setting` (TypeScript text).
pub const INVALID_VALUE: &str = "Invalid value for configuration setting";

/// The blank-credential message (TypeScript text, §3.3 write rule 1).
pub const BLANK_CREDENTIAL: &str =
    "Tenant config requires nonempty mgmtTsgId, mgmtClientId and mgmtClientSecret";

/// Whether `key` is one of the three management credentials.
pub fn is_credential_key(key: &str) -> bool {
    MANAGEMENT_KEYS.contains(&key)
}

/// A decimal integer `^[0-9]+$` (no sign, whitespace or other forms; leading zeros
/// allowed) within `min..=max`.
fn decimal(value: &str, min: i64, max: i64) -> Option<i64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number: u64 = value.parse().ok()?;
    i64::try_from(number)
        .ok()
        .filter(|number| (min..=max).contains(number))
}

/// The §3.3 write rules for a value `create` or `set` accepts from argv, stdin or a
/// prompt (in addition to the read rules). The error is the message: a usage error for
/// argv and stdin, an inline re-ask for a prompt (D76). The value is stored untrimmed.
pub fn write_rule(tenant: &str, key: &'static str, value: &str) -> Result<ConfigValue, String> {
    if js_trim(value).is_empty() {
        return Err(if is_credential_key(key) {
            BLANK_CREDENTIAL.to_owned()
        } else {
            format!(
                "Empty values are not stored; use '{PROGRAM} tenant unset {tenant} {key}' to remove a setting"
            )
        });
    }
    if value.chars().any(char::is_control) {
        return Err(INVALID_VALUE.to_owned());
    }
    let text = || Ok(ConfigValue::Text(value.to_owned()));
    if key == "mgmtTsgId" {
        return if (1..=32).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_digit()) {
            text()
        } else {
            Err("Invalid mgmtTsgId: the Rust SDK requires 1-32 ASCII digits".to_owned())
        };
    }
    if is_endpoint_key(key) {
        let admitted = scm::Endpoint::loopback(value).is_ok()
            && (rule(key) != Rule::Url || url::Url::parse(value).is_ok());
        return if admitted {
            text()
        } else {
            Err(format!(
                "Invalid {key}: endpoints must use https (http is allowed only for loopback hosts) and must not contain credentials, a query or a fragment"
            ))
        };
    }
    let integer = |min, max| {
        decimal(value, min, max)
            .map(ConfigValue::Integer)
            .ok_or_else(|| INVALID_VALUE.to_owned())
    };
    match rule(key) {
        Rule::Retries => integer(0, 5),
        Rule::Concurrency => integer(1, 20),
        Rule::Output => match crate::output_format::Format::parse(value) {
            Some(_) => text(),
            None => Err(INVALID_VALUE.to_owned()),
        },
        Rule::Text | Rule::Url => text(),
    }
}

/// Why a tenant file's content is invalid, in check order (§3.3 step 2; doctor
/// reports each case, commands collapse them).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentError {
    /// Not UTF-8 TOML (the parser's message quotes the input, so it is dropped).
    InvalidToml,
    /// Keys outside the schema, in byte order.
    UnknownKeys(Vec<String>),
    /// Schema keys whose values fail the read rule, in schema order.
    InvalidValues(Vec<&'static str>),
}

/// A validated tenant file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TenantConfig {
    /// Schema keys present in the file, in schema order.
    present: Vec<(&'static str, ConfigValue)>,
    scanners: Option<BTreeMap<String, ScannerCredential>>,
    /// The optional runtime MCP connections (RUNTIME_MCP_CONTRACT §7.3).
    mcp: Option<BTreeMap<String, super::mcp::McpConnection>>,
}

impl TenantConfig {
    /// Parse and validate a tenant file's bytes (§3.3 step 2).
    pub fn parse(bytes: &[u8]) -> Result<Self, ContentError> {
        let table = crate::store::parse_toml(bytes).ok_or(ContentError::InvalidToml)?;
        let unknown: Vec<String> = table
            .keys()
            .filter(|key| read_schema_key(key).is_none())
            .cloned()
            .collect();
        if !unknown.is_empty() {
            return Err(ContentError::UnknownKeys(unknown));
        }
        let mut present = Vec::new();
        let mut invalid = Vec::new();
        for key in KEYS {
            let Some(value) = table.get(key) else {
                continue;
            };
            match read_rule(key, value) {
                Some(value) => present.push((key, value)),
                None => invalid.push(key),
            }
        }
        let scanners = match table.get("scanners") {
            None => None,
            Some(value) => match scanners::parse_namespace(value) {
                Some(map) => Some(map),
                None => {
                    invalid.push("scanners");
                    None
                }
            },
        };
        let mcp = match table.get("mcp") {
            None => None,
            Some(value) => match super::mcp::parse_namespace(value) {
                Some(map) => Some(map),
                None => {
                    invalid.push("mcp");
                    None
                }
            },
        };
        if invalid.is_empty() {
            Ok(Self {
                present,
                scanners,
                mcp,
            })
        } else {
            Err(ContentError::InvalidValues(invalid))
        }
    }

    /// A config from already-validated values (any order; later values win).
    pub fn from_values(values: impl IntoIterator<Item = (&'static str, ConfigValue)>) -> Self {
        let mut config = Self::default();
        for (key, value) in values {
            config.set(key, value);
        }
        config
    }

    /// Set `key` (a schema key) to an already-validated value.
    pub fn set(&mut self, key: &'static str, value: ConfigValue) {
        self.present.retain(|(name, _)| *name != key);
        self.present.push((key, value));
        self.present
            .sort_by_key(|(name, _)| KEYS.iter().position(|key| key == name));
    }

    /// Remove `key`; whether it was present.
    pub fn unset(&mut self, key: &str) -> bool {
        let before = self.present.len();
        self.present.retain(|(name, _)| *name != key);
        before != self.present.len()
    }

    /// The canonical bytes (§3.3): one `<key> = <value>` line per present key, schema
    /// order.
    pub fn to_toml(&self) -> String {
        let mut text: String = self
            .present
            .iter()
            .map(|(key, value)| format!("{key} = {}\n", value.to_toml()))
            .collect();
        if let Some(scanners) = &self.scanners {
            text.push_str("\n[scanners]\n");
            for (name, scanner) in scanners {
                text.push_str(&format!("\n[scanners.{name}]\n{}", scanner.fields_toml()));
            }
        }
        if let Some(mcp) = &self.mcp {
            text.push_str("\n[mcp]\n");
            for (name, connection) in mcp {
                text.push_str(&format!(
                    "\n[mcp.connections.{name}]\n{}",
                    connection.fields_toml()
                ));
            }
        }
        text
    }

    /// The optional runtime MCP connections (contract §7.3).
    pub fn mcp(&self) -> Option<&BTreeMap<String, super::mcp::McpConnection>> {
        self.mcp.as_ref()
    }

    /// Add or replace a connection (`--replace` required to change an existing one).
    pub fn set_mcp(
        &mut self,
        name: &str,
        connection: super::mcp::McpConnection,
        replace: bool,
    ) -> Result<(), CliError> {
        if !super::mcp::valid_name(name) {
            return Err(CliError::usage(
                "Connection names use tenant-name syntax: 1-63 of [a-z0-9-]",
            ));
        }
        connection
            .validate()
            .map_err(|reason| CliError::usage(format!("Invalid MCP connection: {reason}")))?;
        let map = self.mcp.get_or_insert_with(BTreeMap::new);
        if map.contains_key(name) && !replace {
            return Err(CliError::operational(
                "Connection name already exists; use --replace",
            ));
        }
        if !map.contains_key(name) && map.len() >= super::mcp::MAX_CONNECTIONS {
            return Err(CliError::operational(
                "A tenant holds at most 16 MCP connections",
            ));
        }
        map.insert(name.to_owned(), connection);
        Ok(())
    }

    /// Remove a connection; whether it existed.
    pub fn remove_mcp(&mut self, name: &str) -> bool {
        self.mcp
            .as_mut()
            .is_some_and(|map| map.remove(name).is_some())
    }

    /// The stored value.
    pub fn raw(&self, key: &str) -> Option<&ConfigValue> {
        self.present
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value)
    }

    /// The stored string value (values are never empty).
    pub fn effective_str(&self, key: &str) -> Option<&str> {
        self.raw(key).and_then(ConfigValue::as_str)
    }

    /// `airsNumRetries`, when set.
    pub fn num_retries(&self) -> Option<u32> {
        match self.raw("airsNumRetries") {
            Some(ConfigValue::Integer(value)) => u32::try_from(*value).ok(),
            _ => None,
        }
    }

    /// The configured `defaultOutput`.
    pub fn default_output(&self) -> Option<&str> {
        self.effective_str("defaultOutput")
    }

    /// The effective value: the stored one, or the one default (`scanConcurrency` = 5).
    pub fn parsed(&self, key: &str) -> Option<ConfigValue> {
        self.raw(key)
            .cloned()
            .or((key == "scanConcurrency")
                .then_some(ConfigValue::Integer(DEFAULT_SCAN_CONCURRENCY)))
    }

    /// `tenant read` rows: present keys plus `scanConcurrency`, schema order, redacted.
    pub fn read_rows(&self) -> Vec<(&'static str, Shown)> {
        let mut rows: Vec<_> = KEYS
            .iter()
            .filter_map(|key| self.parsed(key).map(|value| (*key, redact(key, value))))
            .collect();
        if self.scanners.is_some() {
            rows.push(("scanners", self.shown("scanners")));
        }
        rows
    }

    /// `tenant get` value: redacted; an unset key shows as `""`.
    pub fn shown(&self, key: &str) -> Shown {
        if key == "scanners" {
            return Shown::Scanners(
                self.scanners
                    .iter()
                    .flat_map(|map| map.iter())
                    .map(|(name, scanner)| (name.clone(), scanner.public()))
                    .collect(),
            );
        }
        redact(
            key,
            self.parsed(key)
                .unwrap_or_else(|| ConfigValue::Text(String::new())),
        )
    }

    /// Values of every secret-named key, for output scrubbing.
    pub fn secret_values(&self) -> impl Iterator<Item = &str> {
        self.present
            .iter()
            .filter(|(key, _)| is_secret_key(key))
            .filter_map(|(_, value)| value.as_str())
            .chain(
                self.scanners
                    .iter()
                    .flat_map(|map| map.values())
                    .flat_map(|scanner| scanner.secrets()),
            )
    }

    pub fn scanners(&self) -> Option<&BTreeMap<String, ScannerCredential>> {
        self.scanners.as_ref()
    }

    pub fn set_scanner(
        &mut self,
        name: &str,
        scanner: ScannerCredential,
        replace: bool,
    ) -> Result<(), CliError> {
        scanners::validate_name(name)?;
        if let Some(map) = &self.scanners {
            if map.contains_key(name) && !replace {
                return Err(CliError::operational(
                    "Scanner name already exists; use --replace",
                ));
            }
            if map
                .keys()
                .any(|key| key != name && key.eq_ignore_ascii_case(name))
            {
                return Err(CliError::operational(
                    "Scanner names may not differ only in case",
                ));
            }
            if map
                .iter()
                .any(|(key, value)| key != name && value.application_id == scanner.application_id)
            {
                return Err(CliError::operational(
                    "Application ID is already bound to another scanner",
                ));
            }
            if !map.contains_key(name) && map.len() >= scanners::MAX_SCANNERS {
                return Err(CliError::operational(
                    "At most 100 named scanners can be configured",
                ));
            }
        }
        self.scanners
            .get_or_insert_with(BTreeMap::new)
            .insert(name.into(), scanner);
        Ok(())
    }

    pub fn remove_scanner(&mut self, name: &str) -> bool {
        self.scanners
            .as_mut()
            .is_some_and(|map| map.remove(name).is_some())
    }

    pub fn scanner_for_application(&self, application: &str) -> Option<(&str, &ScannerCredential)> {
        self.scanners
            .as_ref()?
            .iter()
            .find(|(_, value)| value.application_id == application)
            .map(|(name, scanner)| (name.as_str(), scanner))
    }

    /// A credential is present when it has non-whitespace content (§3.5).
    pub fn is_present(&self, key: &str) -> bool {
        self.effective_str(key)
            .is_some_and(|text| !js_trim(text).is_empty())
    }

    /// Missing management credentials, in check order.
    pub fn missing_management(&self) -> Vec<&'static str> {
        MANAGEMENT_KEYS
            .into_iter()
            .filter(|key| !self.is_present(key))
            .collect()
    }

    /// Configured scanner credentials, in key order.
    pub fn scanner_keys(&self) -> Vec<&'static str> {
        SCANNER_KEYS
            .into_iter()
            .filter(|key| self.is_present(key))
            .collect()
    }
}

/// A value as `tenant read` / `tenant get` show it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Shown {
    Value(ConfigValue),
    Redacted(&'static str),
    Scanners(BTreeMap<String, ScannerPublic>),
}

impl Shown {
    pub fn display(&self) -> String {
        match self {
            Self::Value(value) => value.display(),
            Self::Redacted(text) => (*text).to_owned(),
            Self::Scanners(scanners) => {
                serde_json::to_string(scanners).unwrap_or_else(|_| "{}".into())
            }
        }
    }
}

const REDACTED: &str = "[REDACTED]";

/// A URL with any userinfo, query or fragment replaced by `[REDACTED]` (they can carry
/// credentials, DESIGN §3.6, D45); other URLs are returned byte for byte. A value
/// that is not a URL but has `@`, `?` or `#` is redacted whole.
pub fn redact_url(text: &str) -> Option<String> {
    let Ok(url) = url::Url::parse(text) else {
        return text.contains(['@', '?', '#']).then(|| REDACTED.to_owned());
    };
    let credentials = !url.username().is_empty() || url.password().is_some();
    if !credentials && url.query().is_none() && url.fragment().is_none() {
        return None;
    }
    let Some(host) = url.host_str() else {
        return Some(REDACTED.to_owned());
    };
    let mut out = format!("{}://", url.scheme());
    if credentials {
        out.push_str("[REDACTED]@");
    }
    out.push_str(host);
    if let Some(port) = url.port() {
        out.push_str(&format!(":{port}"));
    }
    out.push_str(url.path());
    if url.query().is_some() {
        out.push_str("?[REDACTED]");
    }
    if url.fragment().is_some() {
        out.push_str("#[REDACTED]");
    }
    Some(out)
}

fn redact(key: &str, value: ConfigValue) -> Shown {
    match &value {
        ConfigValue::Text(text) if is_secret_key(key) && !text.is_empty() => {
            Shown::Redacted(REDACTED)
        }
        ConfigValue::Text(text) if is_endpoint_key(key) => match redact_url(text) {
            Some(redacted) => Shown::Value(ConfigValue::Text(redacted)),
            None => Shown::Value(value),
        },
        _ => Shown::Value(value),
    }
}

/// How to set the given keys (§3.5).
pub fn setting_remedy(tenant: Option<&str>, keys: &[&str]) -> String {
    match tenant {
        Some(name) => format!(
            "Run '{PROGRAM} tenant set {name} <key>' for {} (secrets prompt hidden, or use --stdin)",
            keys.join(", ")
        ),
        None => format!(
            "Run '{PROGRAM} tenant create <name>' (prompts for the OAuth credentials), then '{PROGRAM} tenant switch <name>'"
        ),
    }
}

/// Fail before any client is built when management credentials are missing.
pub fn require_management(config: &TenantConfig, tenant: Option<&str>) -> Result<(), CliError> {
    let missing = config.missing_management();
    if missing.is_empty() {
        return Ok(());
    }
    Err(CliError::operational(format!(
        "Management credentials are not configured (missing {}). {}",
        missing.join(", "),
        setting_remedy(tenant, &missing)
    )))
}

/// Fail before any client is built when neither scanner credential is present.
pub fn require_scanner(config: &TenantConfig, tenant: Option<&str>) -> Result<(), CliError> {
    if !config.scanner_keys().is_empty() {
        return Ok(());
    }
    Err(CliError::operational(format!(
        "Scanner credentials are not configured (airsApiKey or airsApiToken). {}",
        setting_remedy(tenant, &["airsApiKey"])
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(text: &str) -> Result<TenantConfig, ContentError> {
        TenantConfig::parse(text.as_bytes())
    }

    #[test]
    fn keys_are_27_in_schema_order() {
        assert_eq!(KEYS.len(), 27);
        assert_eq!(KEYS[0], "airsApiKey");
        assert_eq!(KEYS[16], "supplychainDataEndpoint");
        assert_eq!(KEYS[18], "gatewayDataEndpoint");
        assert_eq!(KEYS[26], "defaultOutput");
        assert_eq!(KEYS.iter().filter(|key| is_endpoint_key(key)).count(), 16);
        for (old, new) in RENAMES {
            assert_eq!(schema_key(old), None, "{old}");
            assert!(schema_key(new).is_some(), "{new}");
            assert!(is_typescript_name(old));
        }
        for key in DROPPED_KEYS {
            assert_eq!(schema_key(key), None);
            assert!(is_typescript_name(key));
        }
        assert!(is_typescript_name("redTeamTokenEndpoint"));
        assert!(!is_typescript_name("custom"));
        assert_eq!(schema_key("__proto__"), None);
    }

    #[test]
    fn secret_key_names() {
        for key in [
            "airsApiKey",
            "airsApiToken",
            "mgmtClientSecret",
            "gatewayInferenceApiKey",
            "dbPASSWORD",
        ] {
            assert!(is_secret_key(key), "{key}");
        }
        for key in ["mgmtTokenEndpoint", "mgmtTsgId", "keys"] {
            assert!(!is_secret_key(key), "{key}");
        }
        assert_eq!(KEYS.iter().filter(|key| is_secret_key(key)).count(), 4);
    }

    #[test]
    fn invalid_toml_unknown_keys_and_invalid_values_in_check_order() {
        assert_eq!(config("mgmtTsgId = "), Err(ContentError::InvalidToml));
        assert_eq!(
            TenantConfig::parse(b"mgmtTsgId = \"\xff\"\n"),
            Err(ContentError::InvalidToml)
        );
        assert_eq!(
            config("a = 1\na = 2\n"),
            Err(ContentError::InvalidToml),
            "a duplicate key is a TOML error"
        );
        assert_eq!(
            config("zeta = 1\naiGwDataEndpoint = \"x\"\nscanConcurrency = 99\ndataDir = \"/d\"\n"),
            Err(ContentError::UnknownKeys(vec![
                "aiGwDataEndpoint".into(),
                "dataDir".into(),
                "zeta".into()
            ])),
            "unknown keys first, in byte order"
        );
        assert_eq!(
            config("defaultOutput = \"x\"\nscanConcurrency = 99\nairsApiKey = 1\n"),
            Err(ContentError::InvalidValues(vec![
                "airsApiKey",
                "scanConcurrency",
                "defaultOutput"
            ]))
        );
    }

    #[test]
    fn read_rules_are_strict_toml_types() {
        for (text, key) in [
            ("scanConcurrency = 0", "scanConcurrency"),
            ("scanConcurrency = 21", "scanConcurrency"),
            ("scanConcurrency = \"5\"", "scanConcurrency"),
            ("scanConcurrency = 5.0", "scanConcurrency"),
            ("scanConcurrency = true", "scanConcurrency"),
            ("scanConcurrency = [5]", "scanConcurrency"),
            ("airsNumRetries = -1", "airsNumRetries"),
            ("airsNumRetries = 6", "airsNumRetries"),
            ("airsNumRetries = \"0\"", "airsNumRetries"),
            ("mgmtTsgId = 100", "mgmtTsgId"),
            ("mgmtTsgId = \"\"", "mgmtTsgId"),
            ("airsApiKey = \"\"", "airsApiKey"),
            ("mgmtEndpoint = { a = 1 }", "mgmtEndpoint"),
            ("mgmtEndpoint = 1979-05-27", "mgmtEndpoint"),
            (
                "mgmtDashboardEndpoint = \"not a url\"",
                "mgmtDashboardEndpoint",
            ),
            (
                "supplychainDataEndpoint = \"/relative\"",
                "supplychainDataEndpoint",
            ),
            ("defaultOutput = \"markdown\"", "defaultOutput"),
            ("defaultOutput = \"csv\"", "defaultOutput"),
            ("defaultOutput = \"yaml\"", "defaultOutput"),
            ("defaultOutput = \"JSON\"", "defaultOutput"),
            ("defaultOutput = \"\"", "defaultOutput"),
        ] {
            assert_eq!(
                config(text),
                Err(ContentError::InvalidValues(vec![key])),
                "{text}"
            );
        }
        let cfg = config(
            "scanConcurrency = 20\nairsNumRetries = 0\nmgmtDashboardEndpoint = \"localhost:8080\"\nsupplychainMgmtEndpoint = \"mailto:x\"\ndefaultOutput = \"table\"\nmgmtTsgId = \" \"\nmgmtEndpoint = \"not a url either\"\n",
        )
        .unwrap();
        assert_eq!(cfg.raw("scanConcurrency"), Some(&ConfigValue::Integer(20)));
        assert_eq!(cfg.num_retries(), Some(0));
        assert_eq!(cfg.default_output(), Some("table"));
        assert_eq!(
            cfg.effective_str("mgmtTsgId"),
            Some(" "),
            "whitespace is a value"
        );
        assert!(!cfg.is_present("mgmtTsgId"), "but not a present credential");
    }

    #[test]
    fn canonical_bytes_are_schema_ordered() {
        let cfg = config(
            "mgmtTsgId = \"100\"\nmgmtClientSecret = \"FAKE-SECRET\"\nmgmtClientId = \"client-100\"\n",
        )
        .unwrap();
        assert_eq!(
            cfg.to_toml(),
            "mgmtClientId = \"client-100\"\nmgmtClientSecret = \"FAKE-SECRET\"\nmgmtTsgId = \"100\"\n"
        );
        let mut built = TenantConfig::from_values([
            ("scanConcurrency", ConfigValue::Integer(3)),
            ("airsApiKey", ConfigValue::Text("k\"\\\u{1b}".into())),
        ]);
        assert_eq!(
            built.to_toml(),
            "airsApiKey = \"k\\\"\\\\\\u001B\"\nscanConcurrency = 3\n"
        );
        assert_eq!(
            TenantConfig::parse(built.to_toml().as_bytes()),
            Ok(built.clone())
        );
        assert!(built.unset("airsApiKey"));
        assert!(!built.unset("airsApiKey"));
        assert_eq!(built.to_toml(), "scanConcurrency = 3\n");
        assert_eq!(TenantConfig::default().to_toml(), "");
    }

    #[test]
    fn write_rules_in_order() {
        let ok = |key, value: &str| write_rule("dev", key, value).unwrap();
        let err = |key, value: &str| write_rule("dev", key, value).unwrap_err();
        // 1. Blank: credentials have the TypeScript text, other keys point at unset.
        for key in MANAGEMENT_KEYS {
            assert_eq!(err(key, ""), BLANK_CREDENTIAL);
            assert_eq!(err(key, " \u{a0}\t"), BLANK_CREDENTIAL);
        }
        assert_eq!(
            err("defaultOutput", "  "),
            "Empty values are not stored; use 'airs tenant unset dev defaultOutput' to remove a setting"
        );
        // 2. Control characters, anywhere, for every class.
        for key in [
            "mgmtClientId",
            "airsApiKey",
            "mgmtEndpoint",
            "scanConcurrency",
        ] {
            for value in ["a\u{1b}b", "a\tb", "a\u{7f}", "a\u{85}", "a\nb"] {
                assert_eq!(err(key, value), INVALID_VALUE, "{key} {value:?}");
            }
        }
        // 3. Per class.
        assert_eq!(
            ok("mgmtClientId", " client "),
            ConfigValue::Text(" client ".into())
        );
        assert_eq!(ok("mgmtTsgId", "1001"), ConfigValue::Text("1001".into()));
        for tsg in [
            " 100",
            "100 ",
            "1e3",
            "-1",
            "１００",
            &"1".repeat(33),
            "0x10",
        ] {
            assert_eq!(
                err("mgmtTsgId", tsg),
                "Invalid mgmtTsgId: the Rust SDK requires 1-32 ASCII digits",
                "{tsg:?}"
            );
        }
        assert_eq!(
            ok("mgmtTsgId", &"9".repeat(32)),
            ConfigValue::Text("9".repeat(32))
        );
        for (key, value) in [
            ("mgmtEndpoint", "https://api.example.test/x"),
            ("mgmtEndpoint", "http://127.0.0.1:8080"),
            ("mgmtEndpoint", "http://localhost:1/"),
            ("mgmtEndpoint", "http://[::1]:9/"),
            ("supplychainDataEndpoint", "https://sc.example.test"),
        ] {
            assert_eq!(ok(key, value), ConfigValue::Text(value.into()), "{value}");
        }
        for value in [
            "http://example.test",
            "https://user:pw@example.test",
            "https://example.test/?q=1",
            "https://example.test/#f",
            "not a url",
            "localhost:8080",
            "ftp://example.test",
        ] {
            assert!(
                err("mgmtEndpoint", value)
                    .starts_with("Invalid mgmtEndpoint: endpoints must use https"),
                "{value}"
            );
        }
        assert_eq!(ok("scanConcurrency", "7"), ConfigValue::Integer(7));
        assert_eq!(ok("scanConcurrency", "007"), ConfigValue::Integer(7));
        assert_eq!(ok("scanConcurrency", "20"), ConfigValue::Integer(20));
        assert_eq!(ok("airsNumRetries", "0"), ConfigValue::Integer(0));
        for value in [
            "0",
            "21",
            "0x5",
            "1e1",
            "+3",
            " 7 ",
            "5.0",
            "-1",
            "99999999999999999999999",
        ] {
            assert_eq!(err("scanConcurrency", value), INVALID_VALUE, "{value:?}");
        }
        assert_eq!(err("airsNumRetries", "6"), INVALID_VALUE);
        for value in ["pretty", "table", "json"] {
            assert_eq!(ok("defaultOutput", value), ConfigValue::Text(value.into()));
        }
        for value in ["markdown", "csv", "yaml", "JSON", "xml"] {
            assert_eq!(err("defaultOutput", value), INVALID_VALUE, "{value}");
        }
        assert_eq!(
            ok("airsApiKey", "  FAKE SECRET  "),
            ConfigValue::Text("  FAKE SECRET  ".into())
        );
        assert!(is_credential_key("mgmtClientSecret") && !is_credential_key("airsApiKey"));
    }

    #[test]
    fn read_rows_redact_and_default() {
        let cfg = config(
            "mgmtTsgId = \"100\"\nmgmtClientId = \"client-100\"\nmgmtClientSecret = \"X\"\ngatewayInferenceApiKey = \"  \"\n",
        )
        .unwrap();
        let keys: Vec<&str> = cfg.read_rows().iter().map(|(key, _)| *key).collect();
        assert_eq!(
            keys,
            [
                "mgmtClientId",
                "mgmtClientSecret",
                "mgmtTsgId",
                "gatewayInferenceApiKey",
                "scanConcurrency"
            ]
        );
        assert_eq!(cfg.shown("mgmtClientSecret").display(), "[REDACTED]");
        assert_eq!(cfg.shown("gatewayInferenceApiKey").display(), "[REDACTED]");
        assert_eq!(cfg.shown("airsApiKey").display(), "", "unset");
        assert_eq!(cfg.shown("scanConcurrency").display(), "5");
        assert_eq!(cfg.secret_values().collect::<Vec<_>>(), ["X", "  "]);
    }

    #[test]
    fn endpoint_credentials_queries_and_fragments_are_redacted() {
        let cfg = config(
            r#"mgmtTsgId = "100"
airsApiEndpoint = "https://svc-user:FAKE-PW-999@example.invalid/?api_key=FAKE-Q-777"
mgmtEndpoint = "https://u@[::1]:8443/x#frag"
mgmtTokenEndpoint = "https://auth.example.invalid/oauth2/access_token"
dlpEndpoint = "user:pw@host/x?y"
iamEndpoint = "plain text"
supplychainDataEndpoint = "http://127.0.0.1:1/t?k=v"
"#,
        )
        .unwrap();
        let shown = |key: &str| cfg.shown(key).display();
        assert_eq!(
            shown("airsApiEndpoint"),
            "https://[REDACTED]@example.invalid/?[REDACTED]"
        );
        assert_eq!(
            shown("mgmtEndpoint"),
            "https://[REDACTED]@[::1]:8443/x#[REDACTED]"
        );
        assert_eq!(
            shown("mgmtTokenEndpoint"),
            "https://auth.example.invalid/oauth2/access_token"
        );
        assert_eq!(shown("dlpEndpoint"), "[REDACTED]");
        assert_eq!(shown("iamEndpoint"), "plain text");
        assert_eq!(
            shown("supplychainDataEndpoint"),
            "http://127.0.0.1:1/t?[REDACTED]"
        );
        let rows = format!("{:?}", cfg.read_rows());
        assert!(!rows.contains("FAKE-PW-999") && !rows.contains("FAKE-Q-777"));
    }

    #[test]
    fn credential_checks_trim_and_order() {
        let cfg =
            config("mgmtClientId = \"  \"\nmgmtTsgId = \"100\"\nairsApiToken = \"\\t\"\n").unwrap();
        assert_eq!(
            cfg.missing_management(),
            ["mgmtClientId", "mgmtClientSecret"]
        );
        assert!(cfg.scanner_keys().is_empty());
        assert_eq!(
            require_management(&cfg, Some("dev")),
            Err(CliError::operational(
                "Management credentials are not configured (missing mgmtClientId, mgmtClientSecret). Run 'airs tenant set dev <key>' for mgmtClientId, mgmtClientSecret (secrets prompt hidden, or use --stdin)"
            ))
        );
        assert_eq!(
            require_scanner(&cfg, Some("dev")),
            Err(CliError::operational(
                "Scanner credentials are not configured (airsApiKey or airsApiToken). Run 'airs tenant set dev <key>' for airsApiKey (secrets prompt hidden, or use --stdin)"
            ))
        );
        assert_eq!(
            setting_remedy(None, &["airsApiKey"]),
            "Run 'airs tenant create <name>' (prompts for the OAuth credentials), then 'airs tenant switch <name>'"
        );
        let cfg = config("airsApiKey = \"k\"\nairsApiToken = \"t\"\n").unwrap();
        assert_eq!(cfg.scanner_keys(), ["airsApiKey", "airsApiToken"]);
        assert!(require_scanner(&cfg, None).is_ok());
    }
}
