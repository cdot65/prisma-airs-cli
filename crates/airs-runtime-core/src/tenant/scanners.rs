//! Explicit scanner associations; raw credentials never implement Serialize.
use super::config::{ConfigValue, TenantConfig, js_trim};
use crate::{error::CliError, store::toml_string};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_SCANNERS: usize = 100;
pub const NAME_ERROR: &str = "Scanner name must be 1–64 letters, digits, hyphens or underscores and start with a letter/digit";
pub const INPUT_ERROR: &str = "Invalid scanner credential input";

#[derive(Clone, PartialEq, Eq)]
pub struct ScannerCredential {
    pub application_id: String,
    pub endpoint: String,
    pub num_retries: u32,
    pub revision: String,
    api_key: Option<String>,
    api_token: Option<String>,
}

impl std::fmt::Debug for ScannerCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScannerCredential")
            .field("application_id", &self.application_id)
            .field("endpoint", &self.endpoint)
            .field("num_retries", &self.num_retries)
            .field("revision", &self.revision)
            .field("has_api_key", &self.api_key.is_some())
            .field("has_api_token", &self.api_token.is_some())
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScannerReference {
    pub name: String,
    pub revision: String,
    pub application_id: String,
    pub endpoint: String,
}

impl ScannerReference {
    pub fn valid(&self) -> bool {
        valid_name(&self.name)
            && valid_revision(&self.revision)
            && valid_application(&self.application_id)
            && canonical_endpoint(&self.endpoint).as_deref() == Some(self.endpoint.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerPublic {
    pub application_id: String,
    pub endpoint: String,
    pub num_retries: u32,
    pub revision: String,
    pub has_api_key: bool,
    pub has_api_token: bool,
}

pub fn valid_name(name: &str) -> bool {
    crate::store::valid_tenant_name(name)
}

pub fn validate_name(name: &str) -> Result<(), CliError> {
    valid_name(name)
        .then_some(())
        .ok_or_else(|| CliError::usage(NAME_ERROR))
}

pub fn valid_application(value: &str) -> bool {
    !js_trim(value).is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
}

fn valid_revision(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}

pub fn canonical_endpoint(value: &str) -> Option<String> {
    if value.len() > 4096 || value.chars().any(char::is_control) || js_trim(value).is_empty() {
        return None;
    }
    scm::Endpoint::loopback(value)
        .ok()
        .map(|endpoint| endpoint.url().as_str().trim_end_matches('/').to_owned())
        .filter(|endpoint| endpoint.len() <= 4096)
}

fn secret(value: &toml::Value) -> Option<String> {
    let value = value.as_str()?;
    (!value.is_empty() && value.len() <= 2048 && value.bytes().all(|c| c.is_ascii_graphic()))
        .then(|| value.to_owned())
}

impl ScannerCredential {
    fn parse(table: &toml::Table, input: bool) -> Option<Self> {
        if table.keys().any(|key| {
            ![
                "applicationId",
                "endpoint",
                "numRetries",
                "revision",
                "apiKey",
                "apiToken",
            ]
            .contains(&key.as_str())
        }) || (input && table.contains_key("revision"))
        {
            return None;
        }
        let application_id = table.get("applicationId")?.as_str()?;
        if !valid_application(application_id) {
            return None;
        }
        let endpoint = canonical_endpoint(table.get("endpoint")?.as_str()?)?;
        let retries = match table.get("numRetries") {
            Some(v) => v.as_integer()?,
            None if input => 0,
            None => return None,
        };
        if !(0..=5).contains(&retries) {
            return None;
        }
        let revision = if input {
            uuid::Uuid::new_v4().to_string()
        } else {
            let v = table.get("revision")?.as_str()?;
            if !valid_revision(v) {
                return None;
            }
            v.to_owned()
        };
        let (api_key, api_token) = match (table.get("apiKey"), table.get("apiToken")) {
            (Some(v), None) => (Some(secret(v)?), None),
            (None, Some(v)) => (None, Some(secret(v)?)),
            _ => return None,
        };
        Some(Self {
            application_id: application_id.to_owned(),
            endpoint,
            num_retries: retries as u32,
            revision,
            api_key,
            api_token,
        })
    }

    pub fn input(bytes: &[u8]) -> Result<Self, CliError> {
        if bytes.len() as u64 > crate::store::MAX_FILE_BYTES {
            return Err(CliError::usage(INPUT_ERROR));
        }
        crate::store::parse_toml(bytes)
            .and_then(|table| Self::parse(&table, true))
            .ok_or_else(|| CliError::usage(INPUT_ERROR))
    }

    pub fn reference(&self, name: &str) -> ScannerReference {
        ScannerReference {
            name: name.into(),
            revision: self.revision.clone(),
            application_id: self.application_id.clone(),
            endpoint: self.endpoint.clone(),
        }
    }

    pub fn public(&self) -> ScannerPublic {
        ScannerPublic {
            application_id: self.application_id.clone(),
            endpoint: self.endpoint.clone(),
            num_retries: self.num_retries,
            revision: self.revision.clone(),
            has_api_key: self.api_key.is_some(),
            has_api_token: self.api_token.is_some(),
        }
    }

    pub fn secrets(&self) -> impl Iterator<Item = &str> {
        self.api_key
            .iter()
            .chain(self.api_token.iter())
            .map(String::as_str)
    }

    /// A private scanner-only overlay; never mutates or persists the source config.
    pub fn overlay(&self, base: &TenantConfig) -> TenantConfig {
        let mut config = base.clone();
        config.unset("airsApiKey");
        config.unset("airsApiToken");
        if let Some(key) = &self.api_key {
            config.set("airsApiKey", ConfigValue::Text(key.clone()));
        }
        if let Some(token) = &self.api_token {
            config.set("airsApiToken", ConfigValue::Text(token.clone()));
        }
        config.set("airsApiEndpoint", ConfigValue::Text(self.endpoint.clone()));
        config.set(
            "airsNumRetries",
            ConfigValue::Integer(i64::from(self.num_retries)),
        );
        config
    }

    pub fn fields_toml(&self) -> String {
        let mut text = format!(
            "applicationId = {}\nendpoint = {}\nnumRetries = {}\nrevision = {}\n",
            toml_string(&self.application_id),
            toml_string(&self.endpoint),
            self.num_retries,
            toml_string(&self.revision)
        );
        if let Some(key) = &self.api_key {
            text.push_str(&format!("apiKey = {}\n", toml_string(key)));
        }
        if let Some(token) = &self.api_token {
            text.push_str(&format!("apiToken = {}\n", toml_string(token)));
        }
        text
    }

    pub fn from_flat(
        config: &TenantConfig,
        application: &str,
        kind: Option<&str>,
    ) -> Result<Self, CliError> {
        let key = config
            .effective_str("airsApiKey")
            .map(js_trim)
            .filter(|v| !v.is_empty());
        let token = config
            .effective_str("airsApiToken")
            .map(js_trim)
            .filter(|v| !v.is_empty());
        let selected = match (kind, key, token) {
            (None, Some(_), Some(_)) => {
                return Err(CliError::operational(
                    "Both flat scanner credentials are configured; select --use key or --use token",
                ));
            }
            (Some("key"), Some(key), _) | (None, Some(key), None) => ("apiKey", key),
            (Some("token"), _, Some(token)) | (None, None, Some(token)) => ("apiToken", token),
            (Some(_), _, _) => {
                return Err(CliError::operational(
                    "Selected flat scanner credential is not configured",
                ));
            }
            _ => {
                return Err(CliError::operational(
                    "Flat scanner credentials are not configured",
                ));
            }
        };
        let mut table = toml::Table::new();
        table.insert(
            "applicationId".into(),
            toml::Value::String(application.into()),
        );
        table.insert(
            "endpoint".into(),
            toml::Value::String(
                config
                    .effective_str("airsApiEndpoint")
                    .unwrap_or(crate::clients::DEFAULT_SCAN_ENDPOINT)
                    .into(),
            ),
        );
        table.insert(
            "numRetries".into(),
            toml::Value::Integer(i64::from(config.num_retries().unwrap_or(5))),
        );
        table.insert(selected.0.into(), toml::Value::String(selected.1.into()));
        Self::parse(&table, true).ok_or_else(|| CliError::operational(INPUT_ERROR))
    }
}

pub fn parse_namespace(value: &toml::Value) -> Option<BTreeMap<String, ScannerCredential>> {
    let table = value.as_table()?;
    if table.len() > MAX_SCANNERS {
        return None;
    }
    let mut names = BTreeSet::new();
    let mut apps = BTreeSet::new();
    let mut result = BTreeMap::new();
    for (name, value) in table {
        if !valid_name(name) || !names.insert(name.to_ascii_lowercase()) {
            return None;
        }
        let scanner = ScannerCredential::parse(value.as_table()?, false)?;
        if !apps.insert(scanner.application_id.clone()) {
            return None;
        }
        result.insert(name.clone(), scanner);
    }
    Some(result)
}

/// Public metadata remains file text: scrub credential coincidences and controls.
/// Object-key collisions are refused rather than silently merging scanner names.
pub fn safe_public_value(
    value: &mut serde_json::Value,
    secrets: &crate::error::SecretSet,
) -> Result<(), CliError> {
    match value {
        serde_json::Value::String(text) => {
            *text = crate::text::terminal_safe(secrets.scrub(text)).into_owned()
        }
        serde_json::Value::Array(items) => {
            for item in items {
                safe_public_value(item, secrets)?;
            }
        }
        serde_json::Value::Object(map) => {
            let old = std::mem::take(map);
            for (key, mut v) in old {
                let safe = crate::text::terminal_safe(secrets.scrub(&key)).into_owned();
                if safe != key
                    && [
                        "applicationId",
                        "endpoint",
                        "numRetries",
                        "revision",
                        "hasApiKey",
                        "hasApiToken",
                        "path",
                        "bytes",
                    ]
                    .contains(&key.as_str())
                {
                    return Err(CliError::operational(
                        "Scanner metadata cannot be safely displayed",
                    ));
                }
                safe_public_value(&mut v, secrets)?;
                if map.insert(safe, v).is_some() {
                    return Err(CliError::operational(
                        "Scanner metadata cannot be safely displayed",
                    ));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn select(
    config: &TenantConfig,
    name: Option<&str>,
    tenant: Option<&str>,
) -> Result<(TenantConfig, Option<ScannerReference>), CliError> {
    match name {
        Some(name) => {
            validate_name(name)?;
            let scanner = config
                .scanners()
                .and_then(|scanners| scanners.get(name))
                .ok_or_else(|| CliError::operational("Selected scanner is not configured"))?;
            Ok((scanner.overlay(config), Some(scanner.reference(name))))
        }
        None => {
            if config.scanner_keys().is_empty() && config.scanners().is_some_and(|v| !v.is_empty())
            {
                return Err(CliError::operational(
                    "No flat scanner credential is configured; select a named scanner with --scanner <name>",
                ));
            }
            super::config::require_scanner(config, tenant)?;
            Ok((config.clone(), None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(app: &str) -> String {
        format!(
            "applicationId = {}\nendpoint = \"http://127.0.0.1:1234/scan/\"\napiKey = \"FAKE-NAMED-KEY\"\n",
            toml_string(app)
        )
    }

    #[test]
    fn input_is_strict_single_plane_and_never_debugs_the_secret() {
        let s = ScannerCredential::input(input("app-a").as_bytes()).unwrap();
        assert_eq!(s.endpoint, "http://127.0.0.1:1234/scan");
        assert_eq!(s.num_retries, 0);
        assert!(s.reference("monitor").valid());
        assert!(!format!("{s:?}").contains("FAKE-NAMED-KEY"));
        for extra in [
            "apiToken = \"FAKE-TOKEN\"",
            "numRetries = 6",
            "unknownSecret = \"DO-NOT-ECHO\"",
            "revision = \"manual\"",
        ] {
            let error =
                ScannerCredential::input(format!("{}\n{extra}\n", input("app-a")).as_bytes())
                    .unwrap_err();
            assert_eq!(error, CliError::usage(INPUT_ERROR));
        }
        assert!(ScannerCredential::input(input(" ").as_bytes()).is_err());
        let expanded = format!("https://example.invalid/{}", "é".repeat(1000));
        assert!(expanded.len() < 4096);
        assert!(canonical_endpoint(&expanded).is_none());
        assert!(
            ScannerCredential::input(
                input("app-a")
                    .replace("127.0.0.1", "example.com")
                    .as_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn flat_binding_requires_kind_and_preserves_sdk_default_five() {
        let config = TenantConfig::from_values([
            ("airsApiKey", ConfigValue::Text("FAKE-KEY".into())),
            ("airsApiToken", ConfigValue::Text("FAKE-TOKEN".into())),
        ]);
        assert!(ScannerCredential::from_flat(&config, "app-a", None).is_err());
        let key = ScannerCredential::from_flat(&config, "app-a", Some("key")).unwrap();
        let token = ScannerCredential::from_flat(&config, "app-a", Some("token")).unwrap();
        assert_eq!(key.num_retries, 5);
        assert_eq!(key.secrets().collect::<Vec<_>>(), vec!["FAKE-KEY"]);
        assert_eq!(token.secrets().collect::<Vec<_>>(), vec!["FAKE-TOKEN"]);
        let overlay = key.overlay(&config);
        assert_eq!(overlay.effective_str("airsApiToken"), None);
        assert_eq!(config.effective_str("airsApiToken"), Some("FAKE-TOKEN"));
    }

    #[test]
    fn namespace_round_trip_keeps_legacy_bytes_and_public_views_hide_credentials() {
        let bytes = b"airsApiKey = \"FAKE-FLAT-KEY\"\nmgmtTsgId = \"100\"\n";
        let mut config = TenantConfig::parse(bytes).unwrap();
        assert_eq!(config.to_toml().as_bytes(), bytes);
        let scanner = ScannerCredential::input(input("app-a").as_bytes()).unwrap();
        config
            .set_scanner("monitor", scanner.clone(), false)
            .unwrap();
        config.set("mgmtClientId", ConfigValue::Text("fixture-client".into()));
        let saved = config.to_toml();
        let decoded = TenantConfig::parse(saved.as_bytes()).unwrap();
        assert_eq!(decoded, config);
        assert_eq!(
            decoded.scanner_for_application("app-a").unwrap().0,
            "monitor"
        );
        assert!(
            decoded
                .scanner_for_application("same-display-name")
                .is_none()
        );
        let view = serde_json::to_string(&decoded.shown("scanners")).unwrap();
        assert!(view.contains("hasApiKey"));
        assert!(!view.contains("FAKE-NAMED-KEY"));
        assert!(decoded.secret_values().any(|s| s == "FAKE-NAMED-KEY"));
        let (selected, reference) = select(&decoded, Some("monitor"), None).unwrap();
        assert_eq!(selected.effective_str("airsApiKey"), Some("FAKE-NAMED-KEY"));
        assert_eq!(reference.unwrap().application_id, "app-a");
        assert!(select(&decoded, Some("unknown"), None).is_err());
        assert!(
            decoded
                .clone()
                .set_scanner("other", scanner.clone(), false)
                .is_err()
        );
        assert!(
            decoded
                .clone()
                .set_scanner("MONITOR", scanner, true)
                .is_err()
        );
        assert!(config.remove_scanner("monitor"));
        assert!(config.to_toml().ends_with("\n[scanners]\n"));
        assert!(select(&config, None, None).unwrap().1.is_none());
    }

    #[test]
    fn namespace_rejects_ambiguous_apps_unknown_fields_and_case_conflicts() {
        let a = ScannerCredential::input(input("app-a").as_bytes())
            .unwrap()
            .fields_toml();
        for text in [
            format!("[scanners.a]\n{a}\n[scanners.b]\n{a}"),
            format!(
                "[scanners.a]\n{a}\n[scanners.A]\n{}",
                a.replace("app-a", "app-b")
            ),
            format!("[scanners.a]\n{a}\nunknownSecret = \"DO-NOT-ECHO\"\n"),
            format!("[scanners.a]\n{a}\napiToken = \"FAKE-TOKEN\"\n"),
        ] {
            assert_eq!(
                TenantConfig::parse(text.as_bytes()),
                Err(super::super::config::ContentError::InvalidValues(vec![
                    "scanners"
                ]))
            );
        }
    }
}
