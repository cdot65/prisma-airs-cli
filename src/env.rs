//! Environment handling (DESIGN §4, §2.6).
//!
//! Credentials and endpoints come only from tenant files. The CLI reads the variable
//! *names* for doctor's ignored-variable report. Values are never reported.

use std::ffi::{OsStr, OsString};

/// Retired variables the TypeScript CLI used to read.
const RETIRED: [&str; 11] = [
    "PRISMA_AIRS_CONFIG_PATH",
    "TYPESAFE_API_KEY",
    "TYPESAFE_BASE_URL",
    "TYPESAFE_DEFAULT_MODEL",
    "SCAN_CONCURRENCY",
    "DATA_DIR",
    "MEMORY_ENABLED",
    "MEMORY_DIR",
    "MAX_MEMORY_CHARS",
    "ACCUMULATE_TESTS",
    "MAX_ACCUMULATED_TESTS",
];

/// Proxy variables honoured by the SDK transport, in report order.
const PROXY: [&str; 8] = [
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
    "ALL_PROXY",
    "all_proxy",
    "NO_PROXY",
    "no_proxy",
];

/// Names of non-empty variables that look like configuration but are ignored:
/// every `PANW_*` name plus the retired names, sorted by byte order.
pub fn ignored_variables<I>(vars: I) -> Vec<String>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let mut names: Vec<String> = vars
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .filter_map(|(name, _)| name.into_string().ok())
        .filter(|name| name.starts_with("PANW_") || RETIRED.contains(&name.as_str()))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Names of non-empty proxy variables, in the fixed report order.
pub fn proxy_variables<I>(vars: I) -> Vec<&'static str>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let set: Vec<(OsString, OsString)> = vars.into_iter().collect();
    PROXY
        .into_iter()
        .filter(|name| {
            set.iter()
                .any(|(key, value)| key.as_os_str() == OsStr::new(name) && !value.is_empty())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(k), OsString::from(v)))
            .collect()
    }

    #[test]
    fn reports_panw_and_retired_names_sorted() {
        let report = ignored_variables(vars(&[
            ("SCAN_CONCURRENCY", "3"),
            ("PANW_MGMT_CLIENT_SECRET", "ENV-SECRET"),
            ("PANW_AI_SEC_DEBUG", "1"),
            ("PANW_AI_SEC_TIMEOUT_MS", "100"),
            ("PRISMA_AIRS_CONFIG_PATH", "/x"),
            ("PRISMA_AIRS_TENANTS_PATH", "/t.json"),
            ("XDG_STATE_HOME", "/s"),
            ("PANW_EMPTY", ""),
            ("HOME", "/h"),
            ("panw_lower", "x"),
        ]));
        assert_eq!(
            report,
            [
                "PANW_AI_SEC_DEBUG",
                "PANW_AI_SEC_TIMEOUT_MS",
                "PANW_MGMT_CLIENT_SECRET",
                "PRISMA_AIRS_CONFIG_PATH",
                "SCAN_CONCURRENCY"
            ]
        );
        assert!(!report.iter().any(|name| name.contains("SECRET-")));
    }

    #[test]
    fn every_retired_name_is_reported() {
        let pairs: Vec<(&str, &str)> = RETIRED.iter().map(|name| (*name, "1")).collect();
        let mut expected: Vec<String> = RETIRED.iter().map(|s| s.to_string()).collect();
        expected.sort();
        assert_eq!(ignored_variables(vars(&pairs)), expected);
    }

    #[test]
    fn proxies_in_fixed_order() {
        let found = proxy_variables(vars(&[
            ("no_proxy", "x"),
            ("NO_PROXY", "127.0.0.1"),
            ("HTTPS_PROXY", "http://p"),
            ("HTTP_PROXY", ""),
        ]));
        assert_eq!(found, ["HTTPS_PROXY", "NO_PROXY", "no_proxy"]);
        assert!(proxy_variables(vars(&[])).is_empty());
    }
}
