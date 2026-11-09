//! Comparison helpers for TS-vs-Rust parity: normalised requests, JS-number JSON
//! equality and stderr line extraction.

use std::collections::BTreeMap;

use serde_json::{Number, Value};

use crate::sandbox::Sandbox;
use crate::server::{AuthIdentity, RecordedRequest, Route};

/// Headers added by the HTTP stack rather than the SDKs; ignored when comparing.
pub const TRANSPORT_HEADERS: &[&str] = &[
    "user-agent",
    "accept",
    "accept-encoding",
    "accept-language",
    "sec-fetch-mode",
    "host",
    "content-length",
    "connection",
    "transfer-encoding",
];

/// Credential headers; represented by [`NormalizedRequest::auth`] instead.
pub const CREDENTIAL_HEADERS: &[&str] = &["authorization", "x-pan-token", "x-payload-hash"];

/// A request body in comparable form.
#[derive(Clone, Debug, PartialEq)]
pub enum NormalizedBody {
    /// No body.
    None,
    /// JSON, numbers JS-normalised.
    Json(Value),
    /// Form fields in order (`client_secret` values replaced by `<redacted>`).
    Form(Vec<(String, String)>),
    /// Anything else, as text.
    Text(String),
}

/// What two CLIs must agree on for one request.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedRequest {
    /// Method.
    pub method: String,
    /// Path.
    pub path: String,
    /// Route.
    pub route: Route,
    /// Query pairs in wire order (decoded).
    pub query: Vec<(String, String)>,
    /// Non-transport, non-credential headers (lowercase names).
    pub headers: BTreeMap<String, String>,
    /// Body.
    pub body: NormalizedBody,
    /// Client identity.
    pub auth: AuthIdentity,
}

/// Normalises one recorded request.
pub fn normalize(request: &RecordedRequest) -> NormalizedRequest {
    let headers = request
        .headers
        .iter()
        .filter(|(name, _)| {
            !TRANSPORT_HEADERS.contains(&name.as_str())
                && !CREDENTIAL_HEADERS.contains(&name.as_str())
        })
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    NormalizedRequest {
        method: request.method.clone(),
        path: request.path.clone(),
        route: request.route,
        query: request.query.clone(),
        headers,
        body: normalize_body(request),
        auth: request.auth.clone(),
    }
}

/// Normalises a request log.
pub fn normalize_all(requests: &[RecordedRequest]) -> Vec<NormalizedRequest> {
    requests.iter().map(normalize).collect()
}

fn normalize_body(request: &RecordedRequest) -> NormalizedBody {
    if request.body.is_empty() {
        return NormalizedBody::None;
    }
    let form_typed = request
        .header("content-type")
        .is_some_and(|value| value.starts_with("application/x-www-form-urlencoded"));
    if form_typed || request.route == Route::Token {
        let fields = request
            .form()
            .into_iter()
            .map(|(name, value)| {
                let value = if name == "client_secret" {
                    "<redacted>".to_owned()
                } else {
                    value
                };
                (name, value)
            })
            .collect();
        return NormalizedBody::Form(fields);
    }
    match request.body_json() {
        Some(value) => NormalizedBody::Json(js_normalize(&value)),
        None => NormalizedBody::Text(request.body_text()),
    }
}

/// Largest integer JavaScript represents exactly (2^53).
const JS_SAFE: f64 = 9_007_199_254_740_992.0;

/// Rewrites numbers into JavaScript form: an integral float with `|x| <= 2^53` becomes an
/// integer (`2.0` -> `2`, `-0.0` -> `0`). Arrays and objects are rewritten recursively.
pub fn js_normalize(value: &Value) -> Value {
    match value {
        Value::Number(number) => Value::Number(js_number(number)),
        Value::Array(items) => Value::Array(items.iter().map(js_normalize).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), js_normalize(value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn js_number(number: &Number) -> Number {
    if number.is_i64() || number.is_u64() {
        return number.clone();
    }
    match number.as_f64() {
        Some(float) if float.fract() == 0.0 && float.abs() <= JS_SAFE => {
            // In range, so the cast is exact.
            Number::from(float as i64)
        }
        _ => number.clone(),
    }
}

/// Semantic JSON equality: object key order ignored, numbers compared in JS form.
pub fn json_semantic_eq(left: &Value, right: &Value) -> bool {
    js_normalize(left) == js_normalize(right)
}

/// Panics with both documents pretty-printed unless they are semantically equal.
pub fn assert_json_semantic_eq(left: &Value, right: &Value) {
    let (left, right) = (js_normalize(left), js_normalize(right));
    assert!(
        left == right,
        "JSON differs\n--- left ---\n{}\n--- right ---\n{}",
        serde_json::to_string_pretty(&left).unwrap_or_default(),
        serde_json::to_string_pretty(&right).unwrap_or_default()
    );
}

/// The message after `✗ Error: ` on the first such stderr line.
pub fn error_line(stderr: &str) -> Option<String> {
    stderr.lines().find_map(|line| {
        line.trim_start()
            .strip_prefix("✗ Error: ")
            .map(str::to_owned)
    })
}

/// The message of a usage error line (`✗ <message>` without `Error:`).
pub fn usage_line(stderr: &str) -> Option<String> {
    stderr.lines().find_map(|line| {
        line.trim_start()
            .strip_prefix("✗ ")
            .filter(|rest| !rest.starts_with("Error: "))
            .map(str::to_owned)
    })
}

/// The status of the first `HTTP <n>` stderr line.
pub fn http_status(stderr: &str) -> Option<u16> {
    stderr.lines().find_map(|line| {
        line.trim()
            .strip_prefix("HTTP ")
            .and_then(|status| status.parse().ok())
    })
}

/// Normalises tenant-command output of either CLI for comparison (DESIGN §10, M4t):
/// the TypeScript registry path and the Rust `D/config.toml` become `<REGISTRY>`, each
/// tenant's TypeScript config path and its Rust `D/tenants/<name>.toml` become
/// `<CONFIG:name>`, and the Rust program name `airs` before a command word becomes
/// `airs-cli` ([`normalise_program`], D73).
pub fn normalise_tenant_output(text: &str, sandbox: &Sandbox) -> String {
    let mut replacements: Vec<(String, String)> = Vec::new();
    if let Some((entries, _)) = std::fs::read(sandbox.registry_path())
        .ok()
        .and_then(|bytes| crate::store::ts_registry(&bytes))
    {
        for entry in entries {
            replacements.push((
                entry.config_path.clone(),
                format!("<CONFIG:{}>", entry.name),
            ));
            replacements.push((
                sandbox.store_tenant_path(&entry.name).display().to_string(),
                format!("<CONFIG:{}>", entry.name),
            ));
        }
    }
    if let Ok(files) = std::fs::read_dir(sandbox.store_dir().join("tenants")) {
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            if let Some(tenant) = name.strip_suffix(".toml") {
                replacements.push((
                    file.path().display().to_string(),
                    format!("<CONFIG:{tenant}>"),
                ));
            }
        }
    }
    // Fixture configs (`configs/<name>.json`) wherever a registry names them.
    if let Ok(files) = std::fs::read_dir(sandbox.configs_dir()) {
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            if let Some(tenant) = name.strip_suffix(".json") {
                replacements.push((
                    file.path().display().to_string(),
                    format!("<CONFIG:{tenant}>"),
                ));
            }
        }
    }
    replacements.push((
        sandbox.store_index_path().display().to_string(),
        "<REGISTRY>".into(),
    ));
    replacements.push((
        sandbox.registry_path().display().to_string(),
        "<REGISTRY>".into(),
    ));
    // Longest first, so a path never matches inside a longer one.
    replacements.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));
    let mut out = text.to_owned();
    for (from, to) in &replacements {
        if !from.is_empty() {
            out = out.replace(from.as_str(), to);
        }
    }
    normalise_program(&out)
}

/// The words after which `airs` is the Rust program name (DESIGN §21.6): the command
/// groups, `help` and the global flags the hints and examples put first. The one list:
/// the docs scan (`tests/program_name.rs`) builds its invocation words from it, and the
/// help walk there checks that its command words are exactly the root's commands.
pub const PROGRAM_NEXT: &[&str] = &[
    "tenant",
    "doctor",
    "runtime",
    "redteam",
    "model-security",
    "gateway",
    "supplychain",
    // The optional runtime MCP client (D350–D354): Rust-only, so no TypeScript text
    // names it, but the root's command words include it.
    "mcp",
    "help",
    "--tenant",
    "--output",
    "--debug",
    "--quiet",
];

/// The Rust program name `airs` as the TypeScript `airs-cli` (D73, D326, DESIGN §21.6),
/// so Rust text compares with the TypeScript text. Only in context: an `airs` at the
/// start of the text or after whitespace, `'`, `` ` ``, `"` or `(`, followed by one
/// space and a command word of [`PROGRAM_NEXT`] (a maximal run of `[A-Za-z0-9-]`), so
/// `repairs tenant`, `airs.management…` and `airs-cli tenant` are left alone.
pub fn normalise_program(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let mut copied = 0;
    let mut from = 0;
    while let Some(found) = text[from..].find("airs ") {
        let at = from + found;
        from = at + "airs ".len();
        let before = text[..at].chars().next_back();
        let starts =
            before.is_none_or(|c| c.is_whitespace() || matches!(c, '\'' | '`' | '"' | '('));
        let rest = &text[from..];
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .unwrap_or(rest.len());
        if starts && PROGRAM_NEXT.contains(&&rest[..end]) {
            out.push_str(&text[copied..at]);
            out.push_str("airs-cli");
            copied = at + "airs".len();
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// The banner the TypeScript CLI prints above every M4 command's output, on stdout
/// (`renderRuntimeConfigHeader`).
pub const RUNTIME_BANNER: &str =
    "\n  Prisma AIRS — Runtime Configuration\n  Security profile and topic management\n\n";

/// Splits TypeScript stdout into its outcome lines (`  ✓ …`, `  ⚠ …`, `  ℹ …`, `  ✗ …`,
/// every line kept with its newline) and the rest, for comparison with airs-cli-rs,
/// which writes the outcome lines on stderr and only data on stdout (D106, DESIGN §10
/// M4).
pub fn split_outcome_lines(stdout: &str) -> (String, String) {
    let mut outcome = String::new();
    let mut rest = String::new();
    for line in stdout.split_inclusive('\n') {
        let is_outcome = ["  ✓ ", "  ⚠ ", "  ℹ ", "  ✗ "]
            .iter()
            .any(|prefix| line.starts_with(prefix));
        if is_outcome {
            outcome.push_str(line);
        } else {
            rest.push_str(line);
        }
    }
    (outcome, rest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn js_number_normalisation() {
        let float: Value = serde_json::from_str(r#"{"a": 2.0, "b": [1.5, -0.0, 3], "c": 1e300}"#)
            .unwrap_or_default();
        assert_eq!(
            js_normalize(&float),
            json!({"a": 2, "b": [1.5, 0, 3], "c": 1e300})
        );
        assert!(json_semantic_eq(
            &json!({"x": 2.0, "y": 1}),
            &json!({"y": 1, "x": 2})
        ));
        assert!(!json_semantic_eq(&json!(2.5), &json!(2)));
    }

    #[test]
    fn outcome_lines_are_split_from_the_rest() {
        let (outcome, rest) = split_outcome_lines(&format!(
            "{RUNTIME_BANNER}  ✓ Profile created: x\n\n  Profile Detail:\n  ✗ gone\n  ℹ Aborted\n"
        ));
        assert_eq!(outcome, "  ✓ Profile created: x\n  ✗ gone\n  ℹ Aborted\n");
        assert_eq!(rest, format!("{RUNTIME_BANNER}\n  Profile Detail:\n"));
        assert_eq!(split_outcome_lines(""), (String::new(), String::new()));
    }

    #[test]
    fn stderr_lines() {
        let stderr = "\n  ✗ Error: AISEC_CLIENT_SIDE_ERROR:nope\n    HTTP 404\n    Re-run\n\n";
        assert_eq!(
            error_line(stderr).as_deref(),
            Some("AISEC_CLIENT_SIDE_ERROR:nope")
        );
        assert_eq!(http_status(stderr), Some(404));
        assert_eq!(usage_line(stderr), None);
        assert_eq!(
            usage_line("\n  ✗ --limit must be a positive integer\n\n").as_deref(),
            Some("--limit must be a positive integer")
        );
    }

    /// DESIGN §21.6: `airs` before a command word becomes `airs-cli`; nothing else.
    #[test]
    fn the_program_name_is_normalised_in_context_only() {
        for (rust, ts) in [
            ("'airs tenant import'", "'airs-cli tenant import'"),
            ("  $ airs runtime scan", "  $ airs-cli runtime scan"),
            ("Use airs runtime sessions", "Use airs-cli runtime sessions"),
            ("airs doctor", "airs-cli doctor"),
            (
                "(airs gateway scopes get s)",
                "(airs-cli gateway scopes get s)",
            ),
            (
                "`airs redteam status <jobId>`",
                "`airs-cli redteam status <jobId>`",
            ),
            (
                "\"airs --tenant dev doctor\"",
                "\"airs-cli --tenant dev doctor\"",
            ),
            ("a\tairs model-security x", "a\tairs-cli model-security x"),
            (
                "x airs supplychain, airs help",
                "x airs-cli supplychain, airs-cli help",
            ),
            (
                "Usage: airs tenant list [OPTIONS]",
                "Usage: airs-cli tenant list [OPTIONS]",
            ),
        ] {
            assert_eq!(normalise_program(rust), ts, "{rust}");
        }
        for kept in [
            "repairs tenant",
            "chairs runtime",
            "airs.management.profiles.list",
            "airs-cli tenant",
            "AIRS runtime",
            "airs-gateway-report-x.html",
            "airs tenants",
            "airs  tenant",
            "airs [OPTIONS]",
            "xairs doctor",
            "airs",
            "",
        ] {
            assert_eq!(normalise_program(kept), kept);
        }
    }
}
