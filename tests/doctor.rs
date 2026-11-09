//! Executable-level acceptance tests for `airs-cli-rs doctor` (DESIGN §9).
//!
//! Every test spawns the real binary against a fresh loopback [`FixtureServer`] and
//! [`Sandbox`]. Every fixture tenant points every endpoint key at the fixture server
//! (`FixtureConfig::standard`); the few tests that use a non-loopback endpoint value are
//! Rust-only and prove that zero requests are made for it. After every invocation the
//! tenant files are checked unchanged, and the runner's secret leak check is always on.

use std::path::Path;
use std::time::Duration;

use airs_test_harness::{
    AuthIdentity, Cli, ClosedPort, FixtureConfig, FixtureServer, FixtureTenant, Gw, Planned,
    RecordedRequest, RegistryEntry, Route, Run, RunOptions, Sandbox, fixtures, paths, run,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------------------
// Fixed DESIGN §9 strings
// ---------------------------------------------------------------------------------------

const NAMES: [&str; 8] = [
    "Tenant",
    "Config file",
    "Environment",
    "Scanner credentials",
    "Management credentials",
    "Scanner API",
    "Management OAuth",
    "Gateway API",
];
/// The `Executable` check (DESIGN §21.15) every run starts with. The harness removes
/// every `PATH` entry holding an `airs`, `airs-cli` or `airs-cli-rs`, so it passes unless
/// a test sets `PATH` itself (the `executable` module).
const EXECUTABLE_PASS: &str = "no other airs, airs-cli or airs-cli-rs earlier on PATH";
const NOT_EVALUATED: &str = "not evaluated — fix the failed checks above first";
/// The runner always sets `NO_PROXY=127.0.0.1,localhost`, so the suffix is always there.
const ENV_PASS: &str = "no configuration variables set; proxy variables in effect: NO_PROXY";
const ENV_HINT: &str =
    "Configuration comes only from tenant files ('airs tenant set <name> <key>'); unset these";
const MGMT_PASS: &str = "set: mgmtClientId (file), mgmtClientSecret (file), mgmtTsgId (file)";
const MGMT_MISSING_TAIL: &str =
    " — management, red team, model security, gateway and supplychain commands are unavailable";
const SCANNER_SKIP: &str =
    "not configured — runtime scan, bulk-scan and topics eval are unavailable";
const SCANNER_API_PASS: &str = "endpoint reachable, API key accepted";
const SCANNER_API_SKIP: &str = "skipped — no scanner credentials configured";
const MGMT_OAUTH_SKIP: &str = "skipped — management credentials not configured";
const TIMEOUT: &str = "timed out after 5000ms — network unreachable or endpoint not responding";
const VERIFY_SCANNER: &str = "Verify airsApiKey belongs to this tenant and is not expired";
const VERIFY_MGMT: &str =
    "Verify mgmtClientId / mgmtClientSecret / mgmtTsgId belong to this tenant";
const MGMT_TIMEOUT_HINT: &str =
    "Check network connectivity and the mgmtTokenEndpoint / mgmtEndpoint settings";
const HEADER: &str = "\n  Doctor\n  Prisma AIRS CLI preflight checks\n\n";
const ENDPOINT_RULE: &str = "endpoints must use https (http is allowed only for loopback hosts) and must not contain credentials, a query or a fragment";
const NETWORK_FAILED: &str =
    "AISEC_CLIENT_SIDE_ERROR:Network request failed (connection refused, reset or unreachable)";

// ---------------------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------------------

fn cli() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

/// A fresh server and sandbox.
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

    /// The standard healthy tenant `prod` / TSG 100, registered and active.
    fn healthy() -> (Self, FixtureTenant) {
        Self::with(|_| {})
    }

    /// Tenant `prod` / TSG 100 with an edited standard config, registered and active.
    fn with(edit: impl FnOnce(&mut FixtureConfig)) -> (Self, FixtureTenant) {
        let fx = Self::new();
        let tenant = fx
            .sandbox
            .add_fixture_tenant_with(&fx.server, "prod", "100", edit);
        fx.sandbox.register(&[&tenant], Some("prod"));
        (fx, tenant)
    }

    /// Rust-only: like [`Fx::with`], but the edit may point an endpoint away from loopback
    /// (harness opt-out; the TS CLI can no longer run in this sandbox).
    fn with_external_endpoints(edit: impl FnOnce(&mut FixtureConfig)) -> (Self, FixtureTenant) {
        let fx = Self::new();
        let tenant = fx
            .sandbox
            .add_fixture_tenant_allowing_external_endpoints(&fx.server, "prod", "100", edit);
        fx.sandbox.register(&[&tenant], Some("prod"));
        (fx, tenant)
    }

    fn run(&self, args: &[&str]) -> Run {
        self.run_with(args, RunOptions::new())
    }

    /// Runs doctor and proves the tenant files were not touched.
    fn run_with(&self, args: &[&str], options: RunOptions) -> Run {
        let before = self.sandbox.snapshot();
        let out = run(&cli(), &self.sandbox, args, &options);
        self.sandbox.assert_unchanged(&before);
        self.server.assert_no_unknown_routes();
        out
    }

    fn json(&self) -> Run {
        self.run(&["doctor", "--output", "json"])
    }

    fn root(&self) -> String {
        self.sandbox.root().display().to_string()
    }

    /// `D/config.toml`, the path the Tenant check names (§9).
    fn registry(&self) -> String {
        self.sandbox.store_index_path().display().to_string()
    }
}

/// One doctor check as DESIGN §9 defines it.
#[derive(Clone, Debug, PartialEq)]
struct C {
    name: String,
    status: String,
    detail: String,
    hint: Option<String>,
}

fn c(name: &str, status: &str, detail: impl Into<String>) -> C {
    C {
        name: name.into(),
        status: status.into(),
        detail: detail.into(),
        hint: None,
    }
}

impl C {
    fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

fn exe_pass() -> C {
    c("Executable", "pass", EXECUTABLE_PASS)
}

fn ne(name: &str) -> C {
    c(name, "skip", NOT_EVALUATED)
}

fn tenant_pass(fx: &Fx, name: &str, tsg: &str) -> C {
    c(
        "Tenant",
        "pass",
        format!("{name} (TSG {tsg}) selected in {}", fx.registry()),
    )
}

fn config_pass(tenant: &FixtureTenant) -> C {
    c(
        "Config file",
        "pass",
        format!(
            "valid tenant config at {} (TSG {} matches registration)",
            tenant.store_path.display(),
            tenant.tsg
        ),
    )
}

fn env_pass() -> C {
    c("Environment", "pass", ENV_PASS)
}

fn scanner_creds(detail: &str) -> C {
    c("Scanner credentials", "pass", detail)
}

fn scanner_creds_skip(tenant: &str) -> C {
    c("Scanner credentials", "skip", SCANNER_SKIP).hint(format!(
        "Run 'airs tenant set {tenant} <key>' for airsApiKey (secrets prompt hidden, or use --stdin)"
    ))
}

fn mgmt_creds_pass() -> C {
    c("Management credentials", "pass", MGMT_PASS)
}

fn mgmt_creds_missing(status: &str, tenant: &str, missing: &str) -> C {
    c(
        "Management credentials",
        status,
        format!("missing: {missing}{MGMT_MISSING_TAIL}"),
    )
    .hint(format!(
        "Run 'airs tenant set {tenant} <key>' for {missing} (secrets prompt hidden, or use --stdin)"
    ))
}

fn scanner_api_pass() -> C {
    c("Scanner API", "pass", SCANNER_API_PASS)
}

fn mgmt_oauth_pass(count: &str) -> C {
    c(
        "Management OAuth",
        "pass",
        format!("OAuth token obtained, topics API answered ({count})"),
    )
}

/// The `Gateway API` pass (§19.16): the data-plane workspaces in scope.
fn gateway_pass(count: &str) -> C {
    c(
        "Gateway API",
        "pass",
        format!("endpoint reachable ({count} in scope)"),
    )
}

fn healthy_checks(fx: &Fx, tenant: &FixtureTenant) -> Vec<C> {
    vec![
        tenant_pass(fx, &tenant.name, &tenant.tsg),
        config_pass(tenant),
        env_pass(),
        scanner_creds("airsApiKey (file)"),
        mgmt_creds_pass(),
        scanner_api_pass(),
        mgmt_oauth_pass("0 custom topics"),
        gateway_pass("0 workspaces"),
    ]
}

/// The checks after a failed Tenant check that is not a registry error.
fn after_no_tenant(tenant: C) -> Vec<C> {
    vec![
        tenant,
        ne("Config file"),
        env_pass(),
        ne("Scanner credentials"),
        ne("Management credentials"),
        ne("Scanner API"),
        ne("Management OAuth"),
        ne("Gateway API"),
    ]
}

/// The checks after a registry error: every later check is notEvaluated.
fn after_registry_error(tenant: C) -> Vec<C> {
    let mut all = vec![tenant];
    all.extend(NAMES[1..].iter().map(|name| ne(name)));
    all
}

/// The checks after a failed Config file check.
fn after_config_failure(fx: &Fx, config: C) -> Vec<C> {
    vec![
        tenant_pass(fx, "prod", "100"),
        config,
        env_pass(),
        ne("Scanner credentials"),
        ne("Management credentials"),
        ne("Scanner API"),
        ne("Management OAuth"),
        ne("Gateway API"),
    ]
}

/// The checks after `Executable` (the eight of DESIGN §9): asserts that the first
/// check is the `Executable` pass ([`exe_pass`]) and returns the others.
fn checks(out: &Run) -> Vec<C> {
    let mut all = all_checks(out);
    assert_eq!(
        all.first(),
        Some(&exe_pass()),
        "the first check is the Executable pass"
    );
    all.remove(0);
    all
}

/// Parses doctor JSON, checking each element has exactly `name, status, detail[, hint]`
/// with string values and a known status.
fn all_checks(out: &Run) -> Vec<C> {
    let value = out.json();
    let items = value
        .as_array()
        .unwrap_or_else(|| panic!("doctor JSON is not an array: {value}"));
    items
        .iter()
        .map(|item| {
            let object = item
                .as_object()
                .unwrap_or_else(|| panic!("check is not an object: {item}"));
            for key in object.keys() {
                assert!(
                    ["name", "status", "detail", "hint"].contains(&key.as_str()),
                    "unexpected key {key} in {item}"
                );
            }
            let text = |key: &str| {
                object
                    .get(key)
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| panic!("{key} missing or not a string in {item}"))
                    .to_owned()
            };
            let hint = object.get("hint").map(|hint| {
                hint.as_str()
                    .unwrap_or_else(|| panic!("hint is not a string in {item}"))
                    .to_owned()
            });
            let status = text("status");
            assert!(
                ["pass", "warn", "fail", "skip"].contains(&status.as_str()),
                "bad status in {item}"
            );
            C {
                name: text("name"),
                status,
                detail: text("detail"),
                hint,
            }
        })
        .collect()
}

fn get<'a>(all: &'a [C], name: &str) -> &'a C {
    all.iter()
        .find(|check| check.name == name)
        .unwrap_or_else(|| panic!("check {name} missing from {all:?}"))
}

fn quote(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|err| panic!("cannot quote: {err}"))
}

/// The exact JSON bytes DESIGN §2.2/§9 require for the `Executable` pass and then
/// `all`: 2-space pretty, key order `name, status, detail[, hint]`, one trailing newline.
fn render_json(all: &[C]) -> String {
    let items: Vec<String> = std::iter::once(&exe_pass())
        .chain(all)
        .map(|check| {
            let mut fields = vec![
                format!("    \"name\": {}", quote(&check.name)),
                format!("    \"status\": {}", quote(&check.status)),
                format!("    \"detail\": {}", quote(&check.detail)),
            ];
            if let Some(hint) = &check.hint {
                fields.push(format!("    \"hint\": {}", quote(hint)));
            }
            format!("  {{\n{}\n  }}", fields.join(",\n"))
        })
        .collect();
    format!("[\n{}\n]\n", items.join(",\n"))
}

fn glyph(status: &str) -> &'static str {
    match status {
        "pass" => "✓",
        "warn" => "⚠",
        "fail" => "✗",
        "skip" => "○",
        other => panic!("unknown status {other}"),
    }
}

fn plural(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

/// `(failed, summary text)` per DESIGN §9.
fn summary(all: &[C]) -> (bool, String) {
    let count = |status: &str| all.iter().filter(|check| check.status == status).count();
    let (fails, warns, skips) = (count("fail"), count("warn"), count("skip"));
    if fails > 0 {
        return (true, format!("{} failed", plural(fails, "check")));
    }
    let mut parts = Vec::new();
    if warns > 0 {
        parts.push(plural(warns, "warning"));
    }
    if skips > 0 {
        parts.push(format!("{skips} skipped"));
    }
    if parts.is_empty() {
        (false, "All checks passed".into())
    } else {
        (false, format!("All checks passed ({})", parts.join(", ")))
    }
}

/// The exact pretty `(stdout, stderr)` DESIGN §9 specifies for the `Executable` pass and
/// then `all`.
fn render_pretty(all: &[C], quiet: bool) -> (String, String) {
    let mut out = String::new();
    if !quiet {
        out.push_str(HEADER);
    }
    let executable = exe_pass();
    for check in std::iter::once(&executable).chain(all) {
        out.push_str(&format!(
            "  {} {} — {}\n",
            glyph(&check.status),
            check.name,
            check.detail
        ));
        if let (Some(hint), false) = (&check.hint, quiet) {
            out.push_str(&format!("      {hint}\n"));
        }
    }
    out.push('\n');
    let (failed, text) = summary(all);
    let mut err = String::new();
    if failed {
        err = format!("  ✗ {text}\n");
    } else {
        out.push_str(&format!("  ✓ {text}\n"));
    }
    out.push('\n');
    (out, err)
}

/// Asserts the exact JSON report, exit code and an empty stderr.
fn assert_json_report(out: &Run, expected: &[C], code: i32) {
    out.assert_code(code);
    assert_eq!(checks(out), expected, "checks differ");
    assert_eq!(out.stdout, render_json(expected), "JSON bytes differ");
    assert_eq!(
        out.stderr, "",
        "doctor --output json writes nothing to stderr"
    );
}

fn is_uuid_v4(text: &str) -> bool {
    let bytes = text.as_bytes();
    let hex = |b: &u8| b.is_ascii_digit() || (b'a'..=b'f').contains(b);
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, b)| match index {
            8 | 13 | 18 | 23 => *b == b'-',
            _ => hex(b),
        })
        && bytes[14] == b'4'
        && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
}

/// DESIGN §9 Scanner API: exactly `GET /scan/v1/scan/results?scan_ids=<uuid v4>` with the
/// file's `x-pan-token`, no payload hash, no other credential and no body.
fn assert_scan_probe(request: &RecordedRequest, tsg: &str) -> String {
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, paths::SCAN_RESULTS);
    assert_eq!(request.query.len(), 1, "one query pair: {request:?}");
    let (name, id) = &request.query[0];
    assert_eq!(name, "scan_ids");
    assert!(is_uuid_v4(id), "scan_ids is not a lowercase uuid v4: {id}");
    assert_eq!(
        request.raw_query.as_deref(),
        Some(format!("scan_ids={id}").as_str())
    );
    assert_eq!(
        request.header("x-pan-token"),
        Some(format!("FAKE-APIKEY-{tsg}").as_str())
    );
    assert_eq!(request.header("x-payload-hash"), None);
    assert_eq!(request.header("authorization"), None);
    assert_eq!(request.header("content-type"), None);
    assert!(request.body.is_empty(), "probe has no body");
    assert_eq!(
        request.auth,
        AuthIdentity::ApiKey {
            key_id: Some(tsg.into()),
            hash_valid: true
        }
    );
    id.clone()
}

/// The OAuth client-credentials token request for the tenant's file credentials.
fn assert_token_request(request: &RecordedRequest, tsg: &str) {
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, paths::TOKEN);
    assert!(
        request
            .header("content-type")
            .is_some_and(|value| value.starts_with("application/x-www-form-urlencoded")),
        "form content type: {request:?}"
    );
    let form: Vec<(String, String)> = request
        .form()
        .into_iter()
        .filter(|(name, _)| name != "client_id" && name != "client_secret")
        .collect();
    assert_eq!(
        form,
        vec![
            ("grant_type".to_owned(), "client_credentials".to_owned()),
            ("scope".to_owned(), format!("tsg_id:{tsg}")),
        ]
    );
    assert_eq!(
        request.auth,
        AuthIdentity::Basic {
            client_id: format!("client-{tsg}"),
            tsg: Some(tsg.into()),
            valid: true
        }
    );
}

/// `GET /aisec/v1/mgmt/topics/tsg/{tsg}?offset=<n>&limit=100` with the issued bearer.
fn assert_topics_request(request: &RecordedRequest, tsg: &str, offset: u32) {
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, format!("{}{tsg}", paths::TOPICS_PREFIX));
    assert_eq!(
        request.query,
        vec![
            ("offset".to_owned(), offset.to_string()),
            ("limit".to_owned(), "100".to_owned()),
        ]
    );
    assert_eq!(
        request.auth,
        AuthIdentity::Bearer {
            tsg: Some(tsg.into())
        }
    );
    assert!(request.body.is_empty());
    assert_eq!(request.header("x-pan-token"), None);
    assert_eq!(
        request.header("x-tsg-id"),
        None,
        "no x-tsg-id on topic routes"
    );
}

/// The healthy request log: scan probe, token, one topics page (in that order).
fn assert_healthy_requests(fx: &Fx, tsg: &str) {
    let requests = fx.server.requests();
    assert_eq!(
        fx.server.routes(),
        vec![Route::ScanResults, Route::Token, Route::Topics, GATEWAY],
        "{requests:?}"
    );
    assert_scan_probe(&requests[0], tsg);
    assert_token_request(&requests[1], tsg);
    assert_topics_request(&requests[2], tsg, 0);
    assert_gateway_request(&requests[3], tsg);
}

/// `GET <gatewayDataEndpoint>/workspaces` with the Management OAuth bearer (one token for
/// both probes, choice 52) and `x-tsg-id` (§19.16).
fn assert_gateway_request(request: &RecordedRequest, tsg: &str) {
    assert_eq!(request.line(), "GET /ai_gw/v2/workspaces");
    assert_eq!(
        request.auth,
        AuthIdentity::Bearer {
            tsg: Some(tsg.into())
        }
    );
    assert_eq!(request.header("x-tsg-id"), Some(tsg));
    assert!(request.body.is_empty());
}

fn assert_zero_requests(fx: &Fx) {
    assert_eq!(fx.server.request_count(), 0, "{:?}", fx.server.requests());
}

const PROBE_WAIT: Duration = Duration::from_secs(10);

/// The `Gateway API` probe route (`GET <data>/workspaces`, §19.16).
const GATEWAY: Route = Route::Gateway(Gw::DataWorkspaces);

// ---------------------------------------------------------------------------------------
// Healthy tenant: JSON, requests, pretty
// ---------------------------------------------------------------------------------------

mod healthy {
    use super::*;

    #[test]
    fn json_lists_the_nine_checks_in_order_with_exact_details_and_bytes() {
        let (fx, tenant) = Fx::healthy();
        let out = fx.json();
        let all = all_checks(&out);
        assert_eq!(
            all.iter()
                .map(|check| check.name.as_str())
                .collect::<Vec<_>>(),
            std::iter::once("Executable")
                .chain(NAMES)
                .collect::<Vec<_>>()
        );
        assert_json_report(&out, &healthy_checks(&fx, &tenant), 0);
    }

    #[test]
    fn probes_scan_results_then_token_then_topics_exactly_once_each() {
        let (fx, _) = Fx::healthy();
        fx.json().assert_success();
        assert_healthy_requests(&fx, "100");
    }

    #[test]
    fn probe_scan_id_is_a_fresh_uuid_v4_per_run() {
        let (fx, _) = Fx::healthy();
        fx.json().assert_success();
        let first = assert_scan_probe(&fx.server.requests_for(Route::ScanResults)[0], "100");
        fx.server.clear_requests();
        fx.json().assert_success();
        let second = assert_scan_probe(&fx.server.requests_for(Route::ScanResults)[0], "100");
        assert_ne!(first, second, "the probe id is random per run");
    }

    #[test]
    fn probe_never_submits_a_scan_or_lists_profiles() {
        let (fx, _) = Fx::healthy();
        fx.json().assert_success();
        assert!(fx.server.requests_for(Route::ScanSync).is_empty());
        assert!(fx.server.requests_for(Route::Profiles).is_empty());
        assert_eq!(
            fx.server.requests_for(GATEWAY).len(),
            1,
            "the Gateway API probe lists the data-plane workspaces once"
        );
    }

    #[test]
    fn pretty_is_the_default_with_exact_bytes() {
        let (fx, tenant) = Fx::healthy();
        let out = fx.run(&["doctor"]);
        out.assert_success();
        let registry = fx.registry();
        let config = tenant.store_path.display().to_string();
        let expected = format!(
            "\n  Doctor\n  Prisma AIRS CLI preflight checks\n\n\
             \x20 ✓ Executable — no other airs, airs-cli or airs-cli-rs earlier on PATH\n\
             \x20 ✓ Tenant — prod (TSG 100) selected in {registry}\n\
             \x20 ✓ Config file — valid tenant config at {config} (TSG 100 matches registration)\n\
             \x20 ✓ Environment — no configuration variables set; proxy variables in effect: NO_PROXY\n\
             \x20 ✓ Scanner credentials — airsApiKey (file)\n\
             \x20 ✓ Management credentials — set: mgmtClientId (file), mgmtClientSecret (file), mgmtTsgId (file)\n\
             \x20 ✓ Scanner API — endpoint reachable, API key accepted\n\
             \x20 ✓ Management OAuth — OAuth token obtained, topics API answered (0 custom topics)\n\
             \x20 ✓ Gateway API — endpoint reachable (0 workspaces in scope)\n\
             \n  ✓ All checks passed\n\n"
        );
        assert_eq!(out.stdout, expected);
        assert_eq!(out.stderr, "");
        assert_healthy_requests(&fx, "100");
    }

    #[test]
    fn explicit_pretty_equals_default() {
        let (fx, tenant) = Fx::healthy();
        let out = fx.run(&["doctor", "--output", "pretty"]);
        out.assert_success();
        let (stdout, stderr) = render_pretty(&healthy_checks(&fx, &tenant), false);
        assert_eq!(out.stdout, stdout);
        assert_eq!(out.stderr, stderr);
    }
}

// ---------------------------------------------------------------------------------------
// Pretty layout, summary and --quiet
// ---------------------------------------------------------------------------------------

mod pretty {
    use super::*;

    #[test]
    fn hints_are_six_space_lines_and_summary_counts_warnings_and_skips() {
        let (fx, tenant) = Fx::with(|config| {
            config.remove("airsApiKey");
            config.set("customKey", 1);
        });
        let out = fx.run_with(&["doctor"], RunOptions::new().env("PANW_EXTRA", "1"));
        out.assert_success();
        // The TypeScript-only key is dropped at provisioning (import), so the Config
        // file check passes (an unknown key in a TOML file is a failure, D72).
        let expected = vec![
            tenant_pass(&fx, "prod", "100"),
            config_pass(&tenant),
            c(
                "Environment",
                "warn",
                "ignored 1 variable: PANW_EXTRA; proxy variables in effect: NO_PROXY",
            )
            .hint(ENV_HINT),
            scanner_creds_skip("prod"),
            mgmt_creds_pass(),
            c("Scanner API", "skip", SCANNER_API_SKIP),
            mgmt_oauth_pass("0 custom topics"),
            gateway_pass("0 workspaces"),
        ];
        let (stdout, stderr) = render_pretty(&expected, false);
        assert!(stdout.contains("\n  ✓ All checks passed (1 warning, 2 skipped)\n\n"));
        assert!(stdout.contains("\n      Configuration comes only from tenant files"));
        assert_eq!(out.stdout, stdout);
        assert_eq!(out.stderr, stderr);
        assert_eq!(
            fx.server.routes(),
            vec![Route::Token, Route::Topics, GATEWAY]
        );
    }

    #[test]
    fn summary_singular_warning_with_skips() {
        let (fx, _) = Fx::with(|config| {
            config.remove("airsApiKey");
        });
        let out = fx.run_with(&["doctor"], RunOptions::new().env("PANW_EXTRA", "1"));
        out.assert_success();
        assert!(
            out.stdout
                .ends_with("\n\n  ✓ All checks passed (1 warning, 2 skipped)\n\n"),
            "{}",
            out.stdout
        );
    }

    #[test]
    fn summary_warning_only() {
        let (fx, _) = Fx::healthy();
        let out = fx.run_with(&["doctor"], RunOptions::new().env("PANW_EXTRA", "1"));
        out.assert_success();
        assert!(
            out.stdout
                .ends_with("\n\n  ✓ All checks passed (1 warning)\n\n"),
            "{}",
            out.stdout
        );
    }

    #[test]
    fn single_failure_summary_goes_to_stderr() {
        let fx = Fx::new();
        let out = fx.run(&["doctor"]);
        out.assert_code(1);
        let expected = after_no_tenant(
            c(
                "Tenant",
                "fail",
                format!("no tenants registered in {}", fx.registry()),
            )
            .hint("Run 'airs tenant create <name>' (prompts for TSG ID, client ID and secret), then 'airs tenant switch <name>'"),
        );
        let (stdout, stderr) = render_pretty(&expected, false);
        assert_eq!(out.stdout, stdout);
        assert!(
            out.stdout.ends_with("first\n\n\n"),
            "blank, (stderr summary), blank"
        );
        assert_eq!(out.stderr, "  ✗ 1 check failed\n");
        assert_eq!(stderr, out.stderr);
        assert_zero_requests(&fx);
    }

    #[test]
    fn plural_failure_summary() {
        let (fx, _) = Fx::healthy();
        fx.server
            .set_default(Route::ScanResults, Planned::empty_json(401));
        fx.server
            .set_default(Route::Topics, Planned::empty_json(500));
        let out = fx.run(&["doctor"]);
        out.assert_code(1);
        assert_eq!(out.stderr, "  ✗ 2 checks failed\n");
        assert!(out.stdout.contains(
            "  ✗ Scanner API — API key rejected (HTTP 401): AISEC_CLIENT_SIDE_ERROR:API error 401\n      Verify airsApiKey belongs to this tenant and is not expired\n"
        ));
        assert!(!out.stdout.contains("All checks passed"));
    }

    #[test]
    fn quiet_suppresses_header_and_hints_only() {
        let (fx, tenant) = Fx::with(|config| {
            config.remove("airsApiKey");
            config.set("customKey", 1);
        });
        let out = fx.run(&["doctor", "--quiet"]);
        out.assert_success();
        let expected = vec![
            tenant_pass(&fx, "prod", "100"),
            config_pass(&tenant),
            env_pass(),
            scanner_creds_skip("prod"),
            mgmt_creds_pass(),
            c("Scanner API", "skip", SCANNER_API_SKIP),
            mgmt_oauth_pass("0 custom topics"),
            gateway_pass("0 workspaces"),
        ];
        let (stdout, stderr) = render_pretty(&expected, true);
        assert!(!stdout.contains("Doctor") && !stdout.contains("airs tenant"));
        assert!(stdout.starts_with("  ✓ Executable — "));
        assert!(stdout.contains("\n  ✓ Tenant — "));
        assert_eq!(out.stdout, stdout);
        assert_eq!(out.stderr, stderr);
    }

    #[test]
    fn quiet_keeps_the_failure_summary_on_stderr() {
        let fx = Fx::new();
        let out = fx.run(&["--quiet", "doctor"]);
        out.assert_code(1);
        assert!(out.stdout.starts_with(&format!(
            "  ✓ Executable — {EXECUTABLE_PASS}\n  ✗ Tenant — no tenants registered in "
        )));
        assert!(
            !out.stdout.contains("airs tenant create"),
            "hint suppressed"
        );
        assert_eq!(out.stderr, "  ✗ 1 check failed\n");
    }

    #[test]
    fn quiet_does_not_change_json() {
        let (fx, tenant) = Fx::healthy();
        let out = fx.run(&["doctor", "--quiet", "--output", "json"]);
        assert_json_report(&out, &healthy_checks(&fx, &tenant), 0);
    }
}

// ---------------------------------------------------------------------------------------
// Output selection and usage errors (DESIGN §2.3, §2.4)
// ---------------------------------------------------------------------------------------

mod output {
    use super::*;

    #[test]
    fn tenant_default_output_is_ignored() {
        for value in ["json", "table"] {
            let (fx, tenant) = Fx::with(|config| {
                config.set("defaultOutput", value);
            });
            let out = fx.run(&["doctor"]);
            out.assert_success();
            let (stdout, stderr) = render_pretty(&healthy_checks(&fx, &tenant), false);
            assert_eq!(out.stdout, stdout, "defaultOutput {value}");
            assert_eq!(out.stderr, stderr);
        }
    }

    #[test]
    fn global_output_before_the_subcommand_equals_after() {
        let (fx, tenant) = Fx::healthy();
        let before = fx.run(&["--output", "json", "doctor"]);
        let after = fx.run(&["doctor", "--output", "json"]);
        assert_json_report(&before, &healthy_checks(&fx, &tenant), 0);
        assert_eq!(before.stdout, after.stdout);
        assert_eq!(before.code, after.code);
    }

    #[test]
    fn last_output_occurrence_wins() {
        let (fx, tenant) = Fx::healthy();
        let pretty = fx.run(&["--output", "json", "doctor", "--output", "pretty"]);
        pretty.assert_success();
        assert!(pretty.stdout.starts_with(HEADER));
        let json = fx.run(&["doctor", "--output", "pretty", "--output", "json"]);
        assert_json_report(&json, &healthy_checks(&fx, &tenant), 0);
    }

    #[test]
    fn invalid_output_is_a_usage_error_with_zero_requests() {
        let (fx, _) = Fx::healthy();
        for args in [
            &["doctor", "--output", "bogus"][..],
            &["--output", "bogus", "doctor"][..],
        ] {
            let out = fx.run(args);
            out.assert_code(2);
            assert_eq!(out.stdout, "");
            assert_eq!(
                out.stderr,
                "\n  ✗ Invalid output format 'bogus'. Expected: pretty, table, json\n\n"
            );
        }
        assert_zero_requests(&fx);
    }

    #[test]
    fn invalid_output_needs_no_tenant() {
        let fx = Fx::new();
        fx.sandbox.write_registry_raw(b"{ corrupt");
        let out = fx.run(&["doctor", "--output", "bogus"]);
        out.assert_code(2);
        assert_eq!(out.stdout, "");
        assert!(out.stderr.contains("Invalid output format 'bogus'"));
        assert_zero_requests(&fx);
    }

    #[test]
    fn dropped_formats_are_usage_errors() {
        let (fx, _) = Fx::healthy();
        for format in ["markdown", "csv", "yaml"] {
            let out = fx.run(&["doctor", "--output", format]);
            out.assert_code(2);
            assert_eq!(out.stdout, "");
            assert_eq!(
                out.stderr,
                format!(
                    "\n  ✗ Invalid output format '{format}'. Expected: pretty, table, json\n\n"
                )
            );
        }
        assert_zero_requests(&fx);
    }

    #[test]
    fn excess_argument_is_a_clap_usage_error() {
        let (fx, _) = Fx::healthy();
        let out = fx.run(&["doctor", "extra"]);
        out.assert_code(2);
        assert_eq!(out.stdout, "");
        assert!(
            out.stderr.contains("unexpected argument 'extra'"),
            "{}",
            out.stderr
        );
        assert_zero_requests(&fx);
    }

    #[test]
    fn help_prints_to_stdout_with_zero_requests() {
        let (fx, _) = Fx::healthy();
        let out = fx.run(&["doctor", "--help"]);
        out.assert_success();
        assert!(out.stdout.contains("Usage: airs doctor [OPTIONS]"));
        assert!(out.stdout.contains("$ airs doctor --output json"));
        assert_eq!(out.stderr, "");
        assert_zero_requests(&fx);
    }
}

// ---------------------------------------------------------------------------------------
// Tenant check (registry path, registry errors, selection, --tenant)
// ---------------------------------------------------------------------------------------

mod tenant {
    use super::*;

    const CREATE_HINT: &str = "Run 'airs tenant create <name>' (prompts for TSG ID, client ID and secret), then 'airs tenant switch <name>'";

    fn store_error(fx: &Fx, message: String) -> Vec<C> {
        after_registry_error(
            c("Tenant", "fail", message).hint(format!("Fix or restore {}", fx.registry())),
        )
    }

    #[test]
    fn missing_registry_file_means_no_tenants_registered() {
        let fx = Fx::new();
        let out = fx.json();
        let expected = after_no_tenant(
            c(
                "Tenant",
                "fail",
                format!("no tenants registered in {}", fx.registry()),
            )
            .hint(CREATE_HINT),
        );
        assert_json_report(&out, &expected, 1);
        assert_zero_requests(&fx);
    }

    #[test]
    fn missing_registry_parent_directory_is_an_empty_registry() {
        let fx = Fx::new();
        let dir = format!("{}/tmp/nowhere/deeper/airs-cli", fx.root());
        let out = fx.run_with(
            &["doctor", "--output", "json"],
            RunOptions::new().env("AIRS_CLI_CONFIG_DIR", &dir),
        );
        let expected = after_no_tenant(
            c(
                "Tenant",
                "fail",
                format!("no tenants registered in {dir}/config.toml"),
            )
            .hint(CREATE_HINT),
        );
        assert_json_report(&out, &expected, 1);
        assert!(!Path::new(&dir).exists(), "doctor creates nothing");
        assert_zero_requests(&fx);
    }

    #[test]
    fn empty_registry_file_lists_no_tenants() {
        let fx = Fx::new();
        fx.sandbox.write_registry(&[], None);
        let out = fx.json();
        assert_eq!(
            get(&checks(&out), "Tenant").detail,
            format!("no tenants registered in {}", fx.registry())
        );
        out.assert_code(1);
        assert_zero_requests(&fx);
    }

    #[test]
    fn no_selection_lists_registered_names_in_byte_order() {
        let fx = Fx::new();
        let b = fx.sandbox.add_fixture_tenant(&fx.server, "b", "200");
        let a = fx.sandbox.add_fixture_tenant(&fx.server, "a", "100");
        fx.sandbox.register(&[&b, &a], None);
        let out = fx.json();
        let expected = after_no_tenant(
            c("Tenant", "fail", "no tenant selected (registered: a, b)")
                .hint("Run 'airs tenant switch <name>'"),
        );
        assert_json_report(&out, &expected, 1);
        assert_zero_requests(&fx);
    }

    #[test]
    fn corrupt_registry_marks_every_later_check_not_evaluated() {
        let tenant = "[tenants.prod]\ntsg_id = \"100\"\n";
        let variants: Vec<(&str, Vec<u8>)> = vec![
            ("invalid TOML", b"version = ".to_vec()),
            ("version 2", format!("version = 2\n{tenant}").into_bytes()),
            ("no version", tenant.as_bytes().to_vec()),
            (
                "extra top-level key",
                format!("version = 1\nx = 1\n{tenant}").into_bytes(),
            ),
            (
                "active not registered",
                format!("version = 1\nactive = \"ghost\"\n{tenant}").into_bytes(),
            ),
            (
                "empty tsg_id",
                b"version = 1\n[tenants.prod]\ntsg_id = \"\"\n".to_vec(),
            ),
            (
                "bad name",
                b"version = 1\n[tenants.\"-prod\"]\ntsg_id = \"1\"\n".to_vec(),
            ),
            (
                "case collision",
                format!("version = 1\n{tenant}[tenants.Prod]\ntsg_id = \"1\"\n").into_bytes(),
            ),
            (
                "extra entry key",
                format!("version = 1\n{tenant}x = 1\n").into_bytes(),
            ),
            (
                "over 1 MiB",
                format!("version = 1\n{}", "\n".repeat(1_048_577)).into_bytes(),
            ),
            ("invalid UTF-8", b"version = 1\nx = \"\xff\"\n".to_vec()),
        ];
        for (label, bytes) in variants {
            let fx = Fx::new();
            fx.sandbox.write_store_raw(&bytes, 0o600);
            let out = fx.json();
            let registry = fx.registry();
            let expected = store_error(
                &fx,
                format!(
                    "Cannot read tenant registry at {registry}; restore a valid registry before proceeding"
                ),
            );
            assert_eq!(checks(&out), expected, "variant: {label}");
            assert_json_report(&out, &expected, 1);
            assert_zero_requests(&fx);
        }
    }

    #[test]
    fn symlinked_registry_is_rejected() {
        let (fx, _) = Fx::healthy();
        let index = fx.sandbox.store_index_path();
        let real = fx.sandbox.tmp().join("real.toml");
        std::fs::copy(&index, &real).unwrap_or_else(|err| panic!("copy: {err}"));
        fx.sandbox.detach_store();
        std::fs::remove_file(&index).unwrap_or_else(|err| panic!("remove: {err}"));
        std::os::unix::fs::symlink(&real, &index).unwrap_or_else(|err| panic!("symlink: {err}"));
        let out = fx.json();
        let expected = store_error(
            &fx,
            format!(
                "Refusing to use {}: it must be a regular file (not a symlink) owned by the current user with no group or other permissions (chmod 600)",
                fx.registry()
            ),
        );
        assert_json_report(&out, &expected, 1);
        assert_zero_requests(&fx);
    }

    #[test]
    fn relative_xdg_state_home_fails_the_tenant_check() {
        let fx = Fx::new();
        for (name, remove) in [
            ("AIRS_CLI_CONFIG_DIR", None),
            ("XDG_CONFIG_HOME", Some("AIRS_CLI_CONFIG_DIR")),
        ] {
            let mut options = RunOptions::new().env(name, "relative/dir");
            if let Some(remove) = remove {
                options = options.without_env(remove);
            }
            let out = fx.run_with(&["doctor", "--output", "json"], options);
            // D25: no store directory resolves, so the hint names the variables.
            let expected = after_registry_error(
                c("Tenant", "fail", format!("{name} must be an absolute path"))
                    .hint("Set AIRS_CLI_CONFIG_DIR or XDG_CONFIG_HOME to an absolute path"),
            );
            assert_json_report(&out, &expected, 1);
        }
        // XDG_STATE_HOME only locates the TypeScript registry and the debug log now.
        let out = fx.run_with(
            &["doctor", "--output", "json"],
            RunOptions::new()
                .without_env("PRISMA_AIRS_TENANTS_PATH")
                .env("XDG_STATE_HOME", "relative/state"),
        );
        assert_eq!(
            get(&checks(&out), "Tenant").detail,
            format!("no tenants registered in {}", fx.registry())
        );
        assert_zero_requests(&fx);
    }

    #[test]
    fn absolute_xdg_state_home_is_normalised_and_used() {
        let (fx, _) = Fx::healthy();
        let xdg = format!("{}/tmp/xdg", fx.root());
        let store = format!("{xdg}/airs-cli");
        std::fs::create_dir_all(&store).unwrap_or_else(|err| panic!("mkdir: {err}"));
        let mut permissions = std::fs::metadata(&store).expect("meta").permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o700);
        std::fs::set_permissions(&store, permissions).expect("chmod");
        for name in ["config.toml", "tenants"] {
            let status = std::process::Command::new("cp")
                .args(["-Rp"])
                .arg(fx.sandbox.store_dir().join(name))
                .arg(&store)
                .status()
                .expect("cp");
            assert!(status.success());
        }
        let out = fx.run_with(
            &["doctor", "--output", "json"],
            RunOptions::new()
                .without_env("AIRS_CLI_CONFIG_DIR")
                .env("XDG_CONFIG_HOME", format!("{}/tmp/./xdg//", fx.root())),
        );
        out.assert_success();
        assert_eq!(
            get(&checks(&out), "Tenant").detail,
            format!("prod (TSG 100) selected in {store}/config.toml")
        );
        assert_healthy_requests(&fx, "100");
    }

    #[test]
    fn empty_tenants_path_falls_back_to_xdg_state_home() {
        let fx = Fx::new();
        let out = fx.run_with(
            &["doctor", "--output", "json"],
            RunOptions::new()
                .env("AIRS_CLI_CONFIG_DIR", "")
                .env("XDG_CONFIG_HOME", format!("{}/tmp/xdg", fx.root())),
        );
        out.assert_code(1);
        assert_eq!(
            get(&checks(&out), "Tenant").detail,
            format!(
                "no tenants registered in {}/tmp/xdg/airs-cli/config.toml",
                fx.root()
            )
        );
        assert_zero_requests(&fx);
    }

    #[test]
    fn home_fallback_path_when_no_variable_is_set() {
        let fx = Fx::new();
        let out = fx.run_with(
            &["doctor", "--output", "json"],
            RunOptions::new().without_env("AIRS_CLI_CONFIG_DIR"),
        );
        out.assert_code(1);
        assert_eq!(
            get(&checks(&out), "Tenant").detail,
            format!(
                "no tenants registered in {}/home/.config/airs-cli/config.toml",
                fx.root()
            )
        );
        assert_zero_requests(&fx);
        assert!(
            fx.sandbox.home_listing().is_empty(),
            "nothing created in HOME"
        );
    }

    #[test]
    fn the_typescript_registry_variable_no_longer_selects_tenants() {
        // D70: PRISMA_AIRS_TENANTS_PATH names the TypeScript registry, which doctor does
        // not read; the store decides.
        let (fx, _) = Fx::healthy();
        let out = fx.run_with(
            &["doctor", "--output", "json"],
            RunOptions::new().env("PRISMA_AIRS_TENANTS_PATH", "~/reg/../reg/tenants.json"),
        );
        out.assert_success();
        assert_eq!(
            get(&checks(&out), "Tenant").detail,
            format!("prod (TSG 100) selected in {}", fx.registry())
        );
    }

    #[test]
    fn tenant_flag_selects_another_registered_tenant() {
        let fx = Fx::new();
        let a = fx.sandbox.add_fixture_tenant(&fx.server, "a", "100");
        let b = fx.sandbox.add_fixture_tenant(&fx.server, "b", "200");
        fx.sandbox.register(&[&a, &b], Some("a"));
        for args in [
            &["doctor", "--tenant", "b", "--output", "json"][..],
            &["--tenant", "b", "doctor", "--output", "json"][..],
        ] {
            fx.server.clear_requests();
            let out = fx.run(args);
            let expected = vec![
                c(
                    "Tenant",
                    "pass",
                    format!("b (TSG 200) selected by --tenant in {}", fx.registry()),
                ),
                config_pass(&b),
                env_pass(),
                scanner_creds("airsApiKey (file)"),
                mgmt_creds_pass(),
                scanner_api_pass(),
                mgmt_oauth_pass("0 custom topics"),
                gateway_pass("0 workspaces"),
            ];
            assert_json_report(&out, &expected, 0);
            assert_healthy_requests(&fx, "200");
        }
    }

    #[test]
    fn tenant_flag_works_without_an_active_tenant() {
        let fx = Fx::new();
        let a = fx.sandbox.add_fixture_tenant(&fx.server, "a", "100");
        fx.sandbox.register(&[&a], None);
        let out = fx.run(&["doctor", "--tenant", "a", "--output", "json"]);
        out.assert_success();
        assert_eq!(
            get(&checks(&out), "Tenant").detail,
            format!("a (TSG 100) selected by --tenant in {}", fx.registry())
        );
    }

    #[test]
    fn tenant_flag_unknown_name_fails_without_requests() {
        let fx = Fx::new();
        let a = fx.sandbox.add_fixture_tenant(&fx.server, "a", "100");
        let b = fx.sandbox.add_fixture_tenant(&fx.server, "b", "200");
        fx.sandbox.register(&[&a, &b], Some("a"));
        let out = fx.run(&["doctor", "--tenant", "zzz", "--output", "json"]);
        let expected = after_no_tenant(c(
            "Tenant",
            "fail",
            "Tenant not found: zzz (registered: a, b)",
        ));
        assert_json_report(&out, &expected, 1);
        assert_zero_requests(&fx);
    }

    #[test]
    fn tenant_flag_with_empty_registry() {
        let fx = Fx::new();
        let out = fx.run(&["doctor", "--tenant", "zzz", "--output", "json"]);
        let expected = after_no_tenant(c(
            "Tenant",
            "fail",
            "Tenant not found: zzz (no tenants registered)",
        ));
        assert_json_report(&out, &expected, 1);
        assert_zero_requests(&fx);
    }

    #[test]
    fn tenant_flag_invalid_name_is_a_usage_error() {
        let (fx, _) = Fx::healthy();
        for name in ["bad name", "-x", "", &"a".repeat(65)] {
            let out = fx.run(&["doctor", "--tenant", name]);
            out.assert_code(2);
            assert_eq!(out.stdout, "");
            assert_eq!(
                out.stderr,
                "\n  ✗ Tenant name must be 1–64 letters, digits, hyphens or underscores and start with a letter/digit\n\n",
                "name {name:?}"
            );
        }
        assert_zero_requests(&fx);
    }
}

// ---------------------------------------------------------------------------------------
// Config file check
// ---------------------------------------------------------------------------------------

mod config_file {
    use super::*;

    /// The provisioned tenant file of `prod`, with `edit` applied to its text.
    fn edit_tenant_file(fx: &Fx, tenant: &FixtureTenant, edit: impl FnOnce(String) -> String) {
        let text = std::fs::read_to_string(&tenant.store_path).expect("tenant file");
        fx.sandbox
            .write_tenant_file_raw("prod", edit(text).as_bytes(), 0o600);
    }

    #[test]
    fn missing_file_is_not_found() {
        let fx = Fx::new();
        let path = fx.sandbox.config_path("prod");
        fx.sandbox
            .write_registry(&[RegistryEntry::new("prod", &path, "100")], Some("prod"));
        let store_path = fx.sandbox.store_tenant_path("prod");
        let out = fx.json();
        let expected = after_config_failure(
            &fx,
            c("Config file", "fail", format!("not found at {}", store_path.display())).hint(
                "Restore the file, or delete the tenant with 'airs tenant delete prod' and create it again",
            ),
        );
        assert_json_report(&out, &expected, 1);
        assert_zero_requests(&fx);
    }

    #[test]
    fn unreadable_files_cannot_be_read() {
        // D21: an oversized file and a mode-000 file fail.
        for variant in ["over 1 MiB", "mode 000"] {
            let (fx, tenant) = Fx::healthy();
            match variant {
                "over 1 MiB" => edit_tenant_file(&fx, &tenant, |text| {
                    format!("{text}{}", "\n".repeat(1_048_576))
                }),
                _ => {
                    let bytes = std::fs::read(&tenant.store_path).expect("tenant file");
                    fx.sandbox.write_tenant_file_raw("prod", &bytes, 0o000);
                }
            }
            let out = fx.json();
            let expected = after_config_failure(
                &fx,
                c(
                    "Config file",
                    "fail",
                    format!(
                        "{} cannot be read (not a regular file, larger than 1 MiB, or unreadable)",
                        tenant.store_path.display()
                    ),
                ),
            );
            assert_eq!(checks(&out), expected, "variant {variant}");
            assert_json_report(&out, &expected, 1);
            assert_zero_requests(&fx);
        }
    }

    /// D79: a symlink, a directory, group or other permission bits, and an unsafe
    /// `D/tenants` are "not private".
    #[test]
    fn unsafe_files_and_directories_are_not_private() {
        let hint = "Make it a regular file owned by you with mode 0600 (directories 0700), then re-run doctor";
        let private = |path: &Path| {
            c(
                "Config file",
                "fail",
                format!(
                    "{} is not private: it must be a regular file (not a symlink) owned by the current user with no group or other permissions",
                    path.display()
                ),
            )
            .hint(hint)
        };
        for variant in [
            "symlink",
            "directory",
            "mode 0640",
            "mode 0604",
            "tenants 0777",
        ] {
            let (fx, tenant) = Fx::healthy();
            let bytes = std::fs::read(&tenant.store_path).expect("tenant file");
            fx.sandbox.detach_store();
            let mut named = tenant.store_path.clone();
            match variant {
                "symlink" => {
                    let real = fx.sandbox.write_file("tmp/real.toml", &bytes, 0o600);
                    std::fs::remove_file(&tenant.store_path).expect("remove");
                    std::os::unix::fs::symlink(&real, &tenant.store_path).expect("symlink");
                }
                "directory" => {
                    std::fs::remove_file(&tenant.store_path).expect("remove");
                    std::fs::create_dir(&tenant.store_path).expect("mkdir");
                }
                "mode 0640" => {
                    fx.sandbox.write_tenant_file_raw("prod", &bytes, 0o640);
                }
                "mode 0604" => {
                    fx.sandbox.write_tenant_file_raw("prod", &bytes, 0o604);
                }
                _ => {
                    named = fx.sandbox.store_dir().join("tenants");
                    std::fs::set_permissions(
                        &named,
                        std::os::unix::fs::PermissionsExt::from_mode(0o777),
                    )
                    .expect("chmod");
                }
            }
            let out = fx.json();
            let expected = after_config_failure(&fx, private(&named));
            assert_eq!(checks(&out), expected, "variant {variant}");
            assert_json_report(&out, &expected, 1);
            assert_zero_requests(&fx);
            let _ = std::fs::set_permissions(
                fx.sandbox.store_dir().join("tenants"),
                std::os::unix::fs::PermissionsExt::from_mode(0o700),
            );
        }
    }

    #[test]
    fn invalid_json_and_invalid_utf8() {
        let (fx, tenant) = Fx::healthy();
        for bytes in [
            &b"mgmtTsgId = "[..],
            &b"mgmtTsgId = \"100\"\nx = \"\xff\"\n"[..],
            &b"mgmtTsgId = \"100\"\nmgmtTsgId = \"100\"\n"[..],
        ] {
            fx.sandbox.write_tenant_file_raw("prod", bytes, 0o600);
            let out = fx.json();
            let expected = after_config_failure(
                &fx,
                c(
                    "Config file",
                    "fail",
                    format!("{} is not valid TOML", tenant.store_path.display()),
                )
                .hint("Fix the file, then re-run doctor"),
            );
            assert_json_report(&out, &expected, 1);
        }
        assert_zero_requests(&fx);
    }

    #[test]
    fn invalid_values_list_key_names_in_schema_order_never_values() {
        let (fx, tenant) = Fx::healthy();
        edit_tenant_file(&fx, &tenant, |text| {
            let kept: String = text
                .lines()
                .filter(|line| {
                    !["dlpEndpoint", "mgmtDashboardEndpoint", "airsNumRetries"]
                        .iter()
                        .any(|key| line.starts_with(key))
                })
                .map(|line| format!("{line}\n"))
                .collect();
            format!(
                "{kept}defaultOutput = \"xmlish\"\nscanConcurrency = 987654\nmgmtDashboardEndpoint = \"not a url at all\"\nairsNumRetries = \"abc\"\ndlpEndpoint = 5\ngatewayInferenceModel = 31337\n"
            )
        });
        let out = fx.json();
        let expected = after_config_failure(
            &fx,
            c(
                "Config file",
                "fail",
                format!(
                    "{} has invalid values for: airsNumRetries, mgmtDashboardEndpoint, dlpEndpoint, gatewayInferenceModel, scanConcurrency, defaultOutput",
                    tenant.store_path.display()
                ),
            )
            .hint("Correct them in the file, then re-run doctor (values are never printed here; 'airs tenant set' refuses an invalid file); defaultOutput accepts only pretty, table or json (markdown, csv and yaml were dropped)"),
        );
        assert_json_report(&out, &expected, 1);
        for value in ["xmlish", "987654", "not a url at all", "abc", "31337"] {
            assert!(!out.stdout.contains(value), "value {value} printed");
        }
        assert_zero_requests(&fx);
    }

    /// D51, D72: a dead `defaultOutput` and the TOML type rules are invalid values.
    #[test]
    fn dead_formats_and_wrong_types_are_invalid_values() {
        for (line, key) in [
            ("defaultOutput = \"markdown\"", "defaultOutput"),
            ("scanConcurrency = \"3\"", "scanConcurrency"),
            ("airsApiToken = \"\"", "airsApiToken"),
            ("airsNumRetries = 1.0", "airsNumRetries"),
        ] {
            let (fx, tenant) = Fx::healthy();
            fx.sandbox.set_tenant_line("prod", key, line);
            let out = fx.json();
            let check = get(&checks(&out), "Config file").clone();
            assert_eq!(
                check.detail,
                format!(
                    "{} has invalid values for: {key}",
                    tenant.store_path.display()
                ),
                "{line}"
            );
            assert_eq!(
                check
                    .hint
                    .as_deref()
                    .is_some_and(|hint| hint.ends_with("(markdown, csv and yaml were dropped)")),
                key == "defaultOutput",
                "{line}"
            );
            assert_zero_requests(&fx);
        }
    }

    #[test]
    fn js_coercible_numbers_are_valid() {
        // TypeScript coercion happens once, at provisioning (import); the file holds
        // TOML integers.
        let (fx, tenant) = Fx::with(|config| {
            config.set("airsNumRetries", "0");
            config.set("scanConcurrency", json!([3]));
        });
        let out = fx.json();
        assert_json_report(&out, &healthy_checks(&fx, &tenant), 0);
    }

    #[test]
    fn tsg_mismatch_never_prints_the_file_tsg() {
        let (fx, tenant) = Fx::with(|config| {
            config.set("mgmtTsgId", "987654321");
        });
        let out = fx.json();
        let expected = after_config_failure(
            &fx,
            c(
                "Config file",
                "fail",
                format!(
                    "{} carries a different mgmtTsgId than the registration (TSG 100)",
                    tenant.store_path.display()
                ),
            )
            .hint(
                "Choose the TSG with 'airs tenant set prod mgmtTsgId <tsg>'; it updates both files",
            ),
        );
        assert_json_report(&out, &expected, 1);
        assert!(!out.stdout.contains("987654321"));
        assert_zero_requests(&fx);
    }

    #[test]
    fn missing_mgmt_tsg_id_mismatches_the_registration() {
        let (fx, tenant) = Fx::with(|config| {
            config.remove("mgmtTsgId");
        });
        let out = fx.json();
        out.assert_code(1);
        assert_eq!(
            get(&checks(&out), "Config file").detail,
            format!(
                "{} carries a different mgmtTsgId than the registration (TSG 100)",
                tenant.store_path.display()
            )
        );
        assert_zero_requests(&fx);
    }

    #[test]
    fn numeric_mgmt_tsg_id_is_an_invalid_value() {
        let (fx, tenant) = Fx::healthy();
        fx.sandbox
            .set_tenant_line("prod", "mgmtTsgId", "mgmtTsgId = 100");
        let out = fx.json();
        out.assert_code(1);
        assert_eq!(
            get(&checks(&out), "Config file").detail,
            format!(
                "{} has invalid values for: mgmtTsgId",
                tenant.store_path.display()
            )
        );
        assert_zero_requests(&fx);
    }

    /// D72, D89: unknown keys make the file invalid (TS warns), in byte order.
    #[test]
    fn unknown_keys_fail_in_byte_order() {
        let (fx, tenant) = Fx::healthy();
        fx.sandbox
            .append_tenant_line("prod", "zeta = 1\nalpha = true");
        let out = fx.json();
        let expected = after_config_failure(
            &fx,
            c(
                "Config file",
                "fail",
                format!("{} has unknown keys: alpha, zeta", tenant.store_path.display()),
            )
            .hint("Remove them from the file; 'airs tenant set prod <key>' accepts only the supported keys"),
        );
        assert_json_report(&out, &expected, 1);
        assert_zero_requests(&fx);
    }

    #[test]
    fn typescript_names_get_the_rename_hint() {
        for line in [
            "aiGwInferenceModel = \"m\"",
            "agentGuardDataEndpoint = \"http://127.0.0.1:1/x\"",
            "dataDir = \"/d\"",
            "typesafeModel = \"m\"",
            "redTeamTokenEndpoint = \"http://127.0.0.1:1/t\"",
        ] {
            let (fx, tenant) = Fx::healthy();
            fx.sandbox
                .append_tenant_line("prod", &format!("{line}\ncustom = 1"));
            let key = line.split(' ').next().unwrap_or_default();
            let out = fx.json();
            let check = get(&checks(&out), "Config file").clone();
            let mut keys = [key, "custom"];
            keys.sort_unstable();
            assert_eq!(
                check,
                c(
                    "Config file",
                    "fail",
                    format!(
                        "{} has unknown keys: {}",
                        tenant.store_path.display(),
                        keys.join(", ")
                    ),
                )
                .hint("Remove them from the file; 'airs tenant set prod <key>' accepts only the supported keys"),
                "{line}"
            );
            out.assert_code(1);
        }
    }

    #[test]
    fn typescript_empty_values_are_dropped_before_credential_checks() {
        let (fx, _) = Fx::with(|config| {
            config.set("airsApiToken", "");
        });
        let out = fx.json();
        out.assert_success();
        assert_eq!(
            get(&checks(&out), "Scanner credentials"),
            &scanner_creds("airsApiKey (file)")
        );
    }
}

// ---------------------------------------------------------------------------------------
// Environment check
// ---------------------------------------------------------------------------------------

mod environment {
    use super::*;

    #[test]
    fn ignored_variables_are_sorted_names_only_and_never_change_requests() {
        let (fx, tenant) = Fx::healthy();
        let options = RunOptions::new()
            .env("PANW_MGMT_CLIENT_SECRET", "ENV-SECRET-VALUE-1")
            .env("PANW_MGMT_CLIENT_ID", "env-client-id-1")
            .env("PANW_MGMT_TSG_ID", "4815162342")
            .env("PANW_AI_SEC_API_KEY", "ENV-API-KEY-VALUE-1")
            .env("PANW_AI_SEC_DEBUG", "1")
            .env("PANW_AI_SEC_DEBUG_BODY", "1")
            .env("PANW_AI_SEC_TIMEOUT_MS", "1")
            .env("PRISMA_AIRS_CONFIG_PATH", "/nonexistent/config.json")
            .env("TYPESAFE_API_KEY", "ENV-TYPESAFE-VALUE-1")
            .env("MAX_ACCUMULATED_TESTS", "3")
            .env("DATA_DIR", "/tmp/x")
            .env("PANW_EMPTY", "")
            .env("panw_lower", "1")
            .env(
                "XDG_STATE_HOME",
                fx.sandbox.tmp().join("unused-state").display().to_string(),
            )
            .secret("ENV-SECRET-VALUE-1")
            .secret("env-client-id-1")
            .secret("ENV-API-KEY-VALUE-1")
            .secret("ENV-TYPESAFE-VALUE-1");
        let out = fx.run_with(&["doctor", "--output", "json"], options);
        let mut expected = healthy_checks(&fx, &tenant);
        // D24: PANW_AI_SEC_DEBUG*, PANW_AI_SEC_TIMEOUT_MS are reported as ignored.
        expected[2] = c(
            "Environment",
            "warn",
            "ignored 11 variables: DATA_DIR, MAX_ACCUMULATED_TESTS, PANW_AI_SEC_API_KEY, PANW_AI_SEC_DEBUG, PANW_AI_SEC_DEBUG_BODY, PANW_AI_SEC_TIMEOUT_MS, PANW_MGMT_CLIENT_ID, PANW_MGMT_CLIENT_SECRET, PANW_MGMT_TSG_ID, PRISMA_AIRS_CONFIG_PATH, TYPESAFE_API_KEY; proxy variables in effect: NO_PROXY",
        )
        .hint(ENV_HINT);
        assert_json_report(&out, &expected, 0);
        assert!(!out.stdout.contains("4815162342"));
        // Credentials come only from the tenant file (and a 1 ms PANW timeout is ignored).
        assert_healthy_requests(&fx, "100");
    }

    #[test]
    fn single_ignored_variable_is_singular() {
        let (fx, _) = Fx::healthy();
        let out = fx.run_with(
            &["doctor", "--output", "json"],
            RunOptions::new().env("SCAN_CONCURRENCY", "3"),
        );
        out.assert_success();
        assert_eq!(
            get(&checks(&out), "Environment"),
            &c(
                "Environment",
                "warn",
                "ignored 1 variable: SCAN_CONCURRENCY; proxy variables in effect: NO_PROXY"
            )
            .hint(ENV_HINT)
        );
    }

    #[test]
    fn proxy_variables_are_listed_in_fixed_order_and_loopback_bypasses_them() {
        let (fx, _) = Fx::healthy();
        let closed = ClosedPort::new();
        let proxy = format!("http://127.0.0.1:{}", closed.port());
        let out = fx.run_with(
            &["doctor", "--output", "json"],
            RunOptions::new()
                .env("no_proxy", "127.0.0.1,localhost")
                .env("ALL_PROXY", &proxy)
                .env("HTTP_PROXY", &proxy)
                .env("https_proxy", &proxy)
                .env("http_proxy", ""),
        );
        out.assert_success();
        assert_eq!(
            get(&checks(&out), "Environment"),
            &c(
                "Environment",
                "pass",
                "no configuration variables set; proxy variables in effect: https_proxy, HTTP_PROXY, ALL_PROXY, NO_PROXY, no_proxy"
            )
        );
        assert_healthy_requests(&fx, "100");
    }

    #[test]
    fn no_proxy_suffix_when_no_proxy_variable_is_set() {
        // Zero-request scenario: without NO_PROXY nothing may be sent anywhere.
        let fx = Fx::new();
        let out = fx.run_with(
            &["doctor", "--output", "json"],
            RunOptions::new().without_env("NO_PROXY"),
        );
        out.assert_code(1);
        assert_eq!(
            get(&checks(&out), "Environment"),
            &c("Environment", "pass", "no configuration variables set")
        );
        assert_zero_requests(&fx);
    }
}

// ---------------------------------------------------------------------------------------
// Scanner and management credential checks
// ---------------------------------------------------------------------------------------

mod credentials {
    use super::*;

    #[test]
    fn token_only_scanner_credential_probes_with_bearer() {
        let (fx, _) = Fx::with(|config| {
            config.remove("airsApiKey");
            config.set("airsApiToken", "FAKE-APITOKEN-100");
        });
        let out = fx.json();
        out.assert_success();
        assert_eq!(
            get(&checks(&out), "Scanner credentials"),
            &scanner_creds("airsApiToken (file)")
        );
        assert_eq!(get(&checks(&out), "Scanner API"), &scanner_api_pass());
        let probe = &fx.server.requests_for(Route::ScanResults)[0];
        assert_eq!(
            probe.auth,
            AuthIdentity::ApiToken {
                token_id: "100".into()
            }
        );
        assert_eq!(probe.header("x-pan-token"), None);
        assert_eq!(probe.header("x-payload-hash"), None);
    }

    #[test]
    fn key_and_token_are_both_listed() {
        let (fx, _) = Fx::with(|config| {
            config.set("airsApiToken", "FAKE-APITOKEN-100");
        });
        let out = fx.json();
        out.assert_success();
        assert_eq!(
            get(&checks(&out), "Scanner credentials"),
            &scanner_creds("airsApiKey (file), airsApiToken (file)")
        );
        assert_eq!(fx.server.requests_for(Route::ScanResults).len(), 1);
    }

    #[test]
    fn no_scanner_credentials_skip_without_a_scanner_request() {
        let (fx, tenant) = Fx::with(|config| {
            config.remove("airsApiKey");
        });
        let out = fx.json();
        let mut expected = healthy_checks(&fx, &tenant);
        expected[3] = scanner_creds_skip("prod");
        expected[5] = c("Scanner API", "skip", SCANNER_API_SKIP);
        assert_json_report(&out, &expected, 0);
        assert_eq!(
            fx.server.routes(),
            vec![Route::Token, Route::Topics, GATEWAY]
        );
    }

    #[test]
    fn whitespace_only_scanner_key_counts_as_missing() {
        // D22: presence is trimmed, like the commands.
        let (fx, _) = Fx::with(|config| {
            config.set("airsApiKey", "   ");
        });
        let out = fx.json();
        out.assert_success();
        assert_eq!(
            get(&checks(&out), "Scanner credentials"),
            &scanner_creds_skip("prod")
        );
        assert_eq!(
            get(&checks(&out), "Scanner API"),
            &c("Scanner API", "skip", SCANNER_API_SKIP)
        );
        assert!(fx.server.requests_for(Route::ScanResults).is_empty());
    }

    #[test]
    fn missing_management_credentials_warn_when_scanner_passes() {
        let (fx, tenant) = Fx::with(|config| {
            config.remove("mgmtClientId");
            config.remove("mgmtClientSecret");
        });
        let out = fx.json();
        let mut expected = healthy_checks(&fx, &tenant);
        expected[4] = mgmt_creds_missing("warn", "prod", "mgmtClientId, mgmtClientSecret");
        expected[6] = c("Management OAuth", "skip", MGMT_OAUTH_SKIP);
        expected[7] = c("Gateway API", "skip", MGMT_OAUTH_SKIP);
        assert_json_report(&out, &expected, 0);
        assert_eq!(fx.server.routes(), vec![Route::ScanResults]);
    }

    #[test]
    fn whitespace_only_secret_counts_as_missing() {
        // D22.
        let (fx, _) = Fx::with(|config| {
            config.set("mgmtClientSecret", " \t ");
        });
        let out = fx.json();
        out.assert_success();
        assert_eq!(
            get(&checks(&out), "Management credentials"),
            &mgmt_creds_missing("warn", "prod", "mgmtClientSecret")
        );
        assert!(fx.server.requests_for(Route::Token).is_empty());
    }

    #[test]
    fn missing_client_id_only() {
        let (fx, _) = Fx::with(|config| {
            config.remove("mgmtClientId");
        });
        let out = fx.json();
        out.assert_success();
        assert_eq!(
            get(&checks(&out), "Management credentials"),
            &mgmt_creds_missing("warn", "prod", "mgmtClientId")
        );
    }

    #[test]
    fn missing_management_credentials_fail_without_scanner_credentials() {
        let (fx, tenant) = Fx::with(|config| {
            config.remove("mgmtClientId");
            config.remove("mgmtClientSecret");
            config.remove("airsApiKey");
        });
        let out = fx.json();
        let mut expected = healthy_checks(&fx, &tenant);
        expected[3] = scanner_creds_skip("prod");
        expected[4] = mgmt_creds_missing("fail", "prod", "mgmtClientId, mgmtClientSecret");
        expected[5] = c("Scanner API", "skip", SCANNER_API_SKIP);
        expected[6] = c("Management OAuth", "skip", MGMT_OAUTH_SKIP);
        expected[7] = c("Gateway API", "skip", MGMT_OAUTH_SKIP);
        assert_json_report(&out, &expected, 1);
        assert_zero_requests(&fx);
    }
}

// ---------------------------------------------------------------------------------------
// Scanner API probe outcomes
// ---------------------------------------------------------------------------------------

mod scanner_api {
    use super::*;

    /// Runs doctor with the scan results route answering `planned`; returns the Scanner
    /// API check and the exit code, and proves exactly one probe request was sent.
    fn probe(planned: Planned) -> (Fx, C, i32) {
        let (fx, _) = Fx::healthy();
        fx.server.set_default(Route::ScanResults, planned);
        let out = fx.json();
        assert_eq!(out.stderr, "");
        let check = get(&checks(&out), "Scanner API").clone();
        assert_eq!(
            get(&checks(&out), "Management OAuth"),
            &mgmt_oauth_pass("0 custom topics")
        );
        let requests = fx.server.requests();
        assert_eq!(
            fx.server.routes(),
            vec![Route::ScanResults, Route::Token, Route::Topics, GATEWAY],
            "{requests:?}"
        );
        assert_scan_probe(&requests[0], "100");
        (fx, check, out.code)
    }

    #[test]
    fn http_401_rejects_the_key_without_echoing_server_text() {
        let (_, check, code) = probe(Planned::json(401, &json!({"message": "bad key text"})));
        assert_eq!(
            check,
            c(
                "Scanner API",
                "fail",
                "API key rejected (HTTP 401): AISEC_CLIENT_SIDE_ERROR:API error 401"
            )
            .hint(VERIFY_SCANNER)
        );
        assert_eq!(code, 1);
    }

    #[test]
    fn http_403_rejects_the_key() {
        let (_, check, code) = probe(Planned::empty_json(403));
        assert_eq!(
            check,
            c(
                "Scanner API",
                "fail",
                "API key rejected (HTTP 403): AISEC_CLIENT_SIDE_ERROR:API error 403"
            )
            .hint(VERIFY_SCANNER)
        );
        assert_eq!(code, 1);
    }

    #[test]
    fn http_404_is_reachable_and_accepted() {
        let (_, check, code) = probe(Planned::empty_json(404));
        assert_eq!(
            check,
            c(
                "Scanner API",
                "pass",
                "endpoint reachable, API key accepted (HTTP 404 on probe query)"
            )
        );
        assert_eq!(code, 0);
    }

    #[test]
    fn http_400_is_reachable_and_accepted() {
        let (_, check, code) = probe(Planned::empty_json(400));
        assert_eq!(
            check,
            c(
                "Scanner API",
                "pass",
                "endpoint reachable, API key accepted (HTTP 400 on probe query)"
            )
        );
        assert_eq!(code, 0);
    }

    #[test]
    fn http_429_warns_and_exits_zero() {
        // D20.
        let (_, check, code) =
            probe(Planned::json(429, &json!({"message": "slow"})).with_retry_after(0));
        assert_eq!(
            check,
            c(
                "Scanner API",
                "warn",
                "endpoint reachable, but rate limited (HTTP 429): AISEC_CLIENT_SIDE_ERROR:API error 429"
            )
            .hint("Retry later")
        );
        assert_eq!(code, 0);
    }

    #[test]
    fn http_500_fails() {
        // D20.
        let (_, check, code) = probe(Planned::json(500, &json!({"message": "boom"})));
        assert_eq!(
            check,
            c(
                "Scanner API",
                "fail",
                "scanner API error (HTTP 500): AISEC_SERVER_SIDE_ERROR:API error 500"
            )
            .hint("Retry later; check the AIRS service status")
        );
        assert_eq!(code, 1);
    }

    #[test]
    fn http_503_fails_after_one_request_with_zero_retries() {
        // airsNumRetries is 0 in the fixture config: exactly one GET (asserted by probe()).
        let (_, check, code) = probe(Planned::empty_json(503).with_retry_after(0));
        assert_eq!(
            check,
            c(
                "Scanner API",
                "fail",
                "scanner API error (HTTP 503): AISEC_SERVER_SIDE_ERROR:API error 503"
            )
            .hint("Retry later; check the AIRS service status")
        );
        assert_eq!(code, 1);
    }

    #[test]
    fn airs_num_retries_does_not_apply_to_the_probe() {
        // D49: the probe makes one attempt whatever airsNumRetries says.
        let (fx, _) = Fx::with(|config| {
            config.set("airsNumRetries", 2);
        });
        fx.server.set_default(
            Route::ScanResults,
            Planned::empty_json(503).with_retry_after(0),
        );
        let out = fx.json();
        out.assert_code(1);
        assert_eq!(
            get(&checks(&out), "Scanner API").detail,
            "scanner API error (HTTP 503): AISEC_SERVER_SIDE_ERROR:API error 503"
        );
        assert_eq!(fx.server.requests_for(Route::ScanResults).len(), 1);
    }

    #[test]
    fn hang_times_out_after_five_seconds_and_cancels_the_request() {
        let (fx, _) = Fx::healthy();
        fx.server.set_default(Route::ScanResults, Planned::hang());
        let out = fx.json();
        out.assert_code(1);
        assert_eq!(
            get(&checks(&out), "Scanner API"),
            &c("Scanner API", "fail", TIMEOUT)
                .hint("Check network connectivity and the airsApiEndpoint setting")
        );
        assert!(
            out.elapsed >= Duration::from_millis(4900),
            "{:?}",
            out.elapsed
        );
        assert!(out.elapsed < Duration::from_secs(12), "{:?}", out.elapsed);
        assert!(
            fx.server.wait_for_disconnects(1, PROBE_WAIT),
            "the hung probe was really cancelled"
        );
        // The management probe still runs after the scanner timeout.
        assert_eq!(
            get(&checks(&out), "Management OAuth"),
            &mgmt_oauth_pass("0 custom topics")
        );
        assert_eq!(
            fx.server.routes(),
            vec![Route::ScanResults, Route::Token, Route::Topics, GATEWAY]
        );
    }

    #[test]
    fn connection_refused_is_network_unreachable() {
        let closed = ClosedPort::new();
        let port = closed.port();
        let (fx, _) = Fx::with(|config| {
            config.set("airsApiEndpoint", format!("http://127.0.0.1:{port}/scan"));
        });
        let out = fx.json();
        out.assert_code(1);
        assert_eq!(
            get(&checks(&out), "Scanner API"),
            &c(
                "Scanner API",
                "fail",
                format!("network unreachable: {NETWORK_FAILED}")
            )
            .hint("Check network connectivity, proxy settings, and DNS")
        );
        assert_eq!(
            fx.server.routes(),
            vec![Route::Token, Route::Topics, GATEWAY]
        );
    }

    #[test]
    fn non_loopback_http_endpoint_fails_locally_without_echoing_it() {
        // Rust-only: http://example.test is rejected by endpoint admission (zero requests
        // to it; the fixture only sees the management probe).
        for value in [
            "http://example.test/scan",
            "https://user:pw@127.0.0.1/scan",
            "https://127.0.0.1/scan?x=1",
        ] {
            let (fx, _) = Fx::with_external_endpoints(|config| {
                config.set("airsApiEndpoint", value);
            });
            let out = fx.json();
            out.assert_code(1);
            assert_eq!(
                get(&checks(&out), "Scanner API"),
                &c(
                    "Scanner API",
                    "fail",
                    format!("Invalid airsApiEndpoint: {ENDPOINT_RULE}")
                )
                .hint("Fix airsApiEndpoint in the tenant file"),
                "value {value}"
            );
            assert!(!out.stdout.contains("example.test") && !out.stdout.contains("user:pw"));
            assert_eq!(
                fx.server.routes(),
                vec![Route::Token, Route::Topics, GATEWAY]
            );
        }
    }

    #[test]
    fn non_json_success_body_is_a_response_validation_failure() {
        let (_, check, code) = probe(Planned::raw(200, "application/json", "{not json"));
        assert_eq!(
            check,
            c(
                "Scanner API",
                "fail",
                "AISEC_RESPONSE_VALIDATION:Response did not match the expected schema"
            )
            .hint("Fix airsApiEndpoint in the tenant file")
        );
        assert_eq!(code, 1);
    }

    #[test]
    fn redirect_is_not_followed() {
        // D30: no request reaches the redirect target.
        let (fx, _) = Fx::healthy();
        let target = fx.server.url("/elsewhere");
        fx.server
            .set_default(Route::ScanResults, Planned::redirect(302, &target));
        let out = fx.json();
        out.assert_code(1);
        assert_eq!(
            get(&checks(&out), "Scanner API"),
            &c(
                "Scanner API",
                "fail",
                "AISEC_CLIENT_SIDE_ERROR:Redirect responses are not followed"
            )
            .hint("Fix airsApiEndpoint in the tenant file")
        );
        assert_eq!(
            fx.server.routes(),
            vec![Route::ScanResults, Route::Token, Route::Topics, GATEWAY]
        );
    }
}

// ---------------------------------------------------------------------------------------
// Management OAuth probe outcomes
// ---------------------------------------------------------------------------------------

mod management_oauth {
    use super::*;

    fn oauth(fx: &Fx) -> (C, Run) {
        let out = fx.json();
        assert_eq!(out.stderr, "");
        assert_eq!(get(&checks(&out), "Scanner API"), &scanner_api_pass());
        (get(&checks(&out), "Management OAuth").clone(), out)
    }

    #[test]
    fn token_401_uses_the_ts_management_api_error_wording() {
        // DESIGN §9: token non-2xx → `management API error (HTTP <s>): <message>`.
        let (fx, _) = Fx::healthy();
        fx.server
            .set_default(Route::Token, Planned::empty_json(401));
        let (check, out) = oauth(&fx);
        assert_eq!(
            check,
            c(
                "Management OAuth",
                "fail",
                "management API error (HTTP 401): AISEC_OAUTH_ERROR:Token request failed with status 401"
            )
            .hint(VERIFY_MGMT)
        );
        out.assert_code(1);
    }

    #[test]
    fn token_401_sends_one_token_request_and_no_topics_request() {
        let (fx, _) = Fx::healthy();
        fx.server
            .set_default(Route::Token, Planned::empty_json(401));
        let (check, out) = oauth(&fx);
        assert_eq!(check.status, "fail");
        assert_eq!(check.hint.as_deref(), Some(VERIFY_MGMT));
        out.assert_code(1);
        let requests = fx.server.requests();
        assert_eq!(fx.server.routes(), vec![Route::ScanResults, Route::Token]);
        assert_token_request(&requests[1], "100");
    }

    #[test]
    fn token_schema_mismatch_is_an_authentication_failure() {
        let (fx, _) = Fx::healthy();
        fx.server.set_default(
            Route::Token,
            Planned::json(200, &json!({"token_type": "Bearer"})),
        );
        let (check, out) = oauth(&fx);
        assert_eq!(
            check,
            c(
                "Management OAuth",
                "fail",
                "authentication failed: AISEC_OAUTH_ERROR:Token response did not match the OAuth schema"
            )
            .hint(VERIFY_MGMT)
        );
        out.assert_code(1);
        assert_eq!(fx.server.routes(), vec![Route::ScanResults, Route::Token]);
    }

    #[test]
    fn token_hang_times_out_and_cancels() {
        let (fx, _) = Fx::healthy();
        fx.server.set_default(Route::Token, Planned::hang());
        let (check, out) = oauth(&fx);
        assert_eq!(
            check,
            c("Management OAuth", "fail", TIMEOUT).hint(MGMT_TIMEOUT_HINT)
        );
        out.assert_code(1);
        assert!(
            out.elapsed >= Duration::from_millis(4900),
            "{:?}",
            out.elapsed
        );
        assert!(out.elapsed < Duration::from_secs(12), "{:?}", out.elapsed);
        assert!(fx.server.wait_for_disconnects(1, PROBE_WAIT));
        assert_eq!(fx.server.routes(), vec![Route::ScanResults, Route::Token]);
    }

    #[test]
    fn token_connection_refused_is_network_unreachable() {
        let closed = ClosedPort::new();
        let port = closed.port();
        let (fx, _) = Fx::with(|config| {
            config.set(
                "mgmtTokenEndpoint",
                format!("http://127.0.0.1:{port}/oauth/token"),
            );
        });
        let (check, out) = oauth(&fx);
        assert_eq!(
            check,
            c(
                "Management OAuth",
                "fail",
                "network unreachable: AISEC_OAUTH_ERROR:Token request failed: network error"
            )
            .hint(VERIFY_MGMT)
        );
        out.assert_code(1);
        assert_eq!(fx.server.routes(), vec![Route::ScanResults]);
    }

    #[test]
    fn topics_403_is_a_management_api_error() {
        let (fx, _) = Fx::healthy();
        fx.server
            .set_default(Route::Topics, Planned::empty_json(403));
        let (check, out) = oauth(&fx);
        assert_eq!(
            check,
            c(
                "Management OAuth",
                "fail",
                "management API error (HTTP 403): AISEC_CLIENT_SIDE_ERROR:API error 403"
            )
            .hint(VERIFY_MGMT)
        );
        out.assert_code(1);
        // D49: one attempt per probe request, so no token refresh and replay (TS
        // refreshes and replays once). The refused lease is dropped, so the Gateway API
        // probe fetches its own token (choice 99).
        assert_eq!(
            fx.server.routes(),
            vec![
                Route::ScanResults,
                Route::Token,
                Route::Topics,
                Route::Token,
                GATEWAY
            ]
        );
    }

    #[test]
    fn topics_500_is_a_management_api_error_without_retry() {
        let (fx, _) = Fx::healthy();
        fx.server.set_default(
            Route::Topics,
            Planned::json(500, &json!({"message": "server text"})),
        );
        let (check, out) = oauth(&fx);
        assert_eq!(
            check,
            c(
                "Management OAuth",
                "fail",
                "management API error (HTTP 500): AISEC_SERVER_SIDE_ERROR:API error 500"
            )
            .hint(VERIFY_MGMT)
        );
        out.assert_code(1);
        // D10: the SDK retries 429/502/503/504, not 500.
        assert_eq!(
            fx.server.routes(),
            vec![Route::ScanResults, Route::Token, Route::Topics, GATEWAY]
        );
    }

    #[test]
    fn topics_hang_times_out_with_the_management_hint() {
        let (fx, _) = Fx::healthy();
        fx.server.set_default(Route::Topics, Planned::hang());
        let (check, out) = oauth(&fx);
        assert_eq!(
            check,
            c("Management OAuth", "fail", TIMEOUT).hint(MGMT_TIMEOUT_HINT)
        );
        out.assert_code(1);
        assert!(fx.server.wait_for_disconnects(1, PROBE_WAIT));
        assert_eq!(
            fx.server.routes(),
            vec![Route::ScanResults, Route::Token, Route::Topics, GATEWAY]
        );
    }

    #[test]
    fn token_and_topics_share_one_five_second_budget() {
        let (fx, _) = Fx::healthy();
        fx.server.push_once(
            Route::Token,
            Planned::json(
                200,
                &json!({"access_token": "mock-token-100", "token_type": "Bearer", "expires_in": 3600}),
            )
            .with_delay(Duration::from_millis(3000)),
        );
        fx.server.set_default(
            Route::Topics,
            Planned::json(200, &json!({"custom_topics": []}))
                .with_delay(Duration::from_millis(3000)),
        );
        let (check, out) = oauth(&fx);
        assert_eq!(
            check,
            c("Management OAuth", "fail", TIMEOUT).hint(MGMT_TIMEOUT_HINT)
        );
        out.assert_code(1);
        assert!(
            out.elapsed < Duration::from_millis(5900),
            "{:?}",
            out.elapsed
        );
    }

    #[test]
    fn pass_count_is_singular_for_one_topic() {
        let (fx, _) = Fx::healthy();
        fx.server
            .set_topics("100", vec![fixtures::topic(1, "fin", 1, "finance")]);
        let (check, out) = oauth(&fx);
        assert_eq!(check, mgmt_oauth_pass("1 custom topic"));
        out.assert_success();
    }

    #[test]
    fn pass_count_is_plural_for_two_topics() {
        let (fx, _) = Fx::healthy();
        fx.server.set_topics(
            "100",
            vec![
                fixtures::topic(1, "fin", 1, "finance"),
                fixtures::topic(2, "legal", 1, "legal"),
            ],
        );
        let (check, out) = oauth(&fx);
        assert_eq!(check, mgmt_oauth_pass("2 custom topics"));
        out.assert_success();
    }

    #[test]
    fn revisions_are_counted_as_rows() {
        let (fx, _) = Fx::healthy();
        fx.server.set_topics(
            "100",
            vec![
                fixtures::topic(1, "fin", 1, "finance"),
                fixtures::topic(1, "fin", 2, "finance"),
                fixtures::topic(2, "legal", 1, "legal"),
            ],
        );
        let (check, _) = oauth(&fx);
        assert_eq!(check, mgmt_oauth_pass("3 custom topics"));
    }

    #[test]
    fn multi_page_walk_counts_every_row_with_one_token() {
        let (fx, _) = Fx::healthy();
        fx.server.set_topics(
            "100",
            (1..=250)
                .map(|n| fixtures::topic(n, &format!("topic-{n}"), 1, "d"))
                .collect(),
        );
        let (check, out) = oauth(&fx);
        assert_eq!(check, mgmt_oauth_pass("250 custom topics"));
        out.assert_success();
        let requests = fx.server.requests();
        assert_eq!(
            fx.server.routes(),
            vec![
                Route::ScanResults,
                Route::Token,
                Route::Topics,
                Route::Topics,
                Route::Topics,
                GATEWAY
            ]
        );
        assert_token_request(&requests[1], "100");
        assert_topics_request(&requests[2], "100", 0);
        assert_topics_request(&requests[3], "100", 100);
        assert_topics_request(&requests[4], "100", 200);
    }

    #[test]
    fn non_digit_tsg_fails_locally_without_a_token_request() {
        // D28.
        let fx = Fx::new();
        let tenant = fx.sandbox.add_fixture_tenant(&fx.server, "prod", "abc");
        fx.sandbox.register(&[&tenant], Some("prod"));
        let (check, out) = oauth(&fx);
        assert_eq!(
            check,
            c(
                "Management OAuth",
                "fail",
                "Invalid mgmtTsgId: the Rust SDK requires 1-32 ASCII digits"
            )
            .hint(VERIFY_MGMT)
        );
        out.assert_code(1);
        assert_eq!(fx.server.routes(), vec![Route::ScanResults]);
    }

    #[test]
    fn non_loopback_management_endpoints_fail_locally() {
        // Rust-only: the invalid endpoint is rejected before any management request.
        for key in ["mgmtEndpoint", "mgmtTokenEndpoint"] {
            let (fx, _) = Fx::with_external_endpoints(|config| {
                config.set(key, "http://example.test/aisec");
            });
            let (check, out) = oauth(&fx);
            assert_eq!(
                check,
                c(
                    "Management OAuth",
                    "fail",
                    format!("Invalid {key}: {ENDPOINT_RULE}")
                )
                .hint(VERIFY_MGMT),
                "key {key}"
            );
            assert!(!out.stdout.contains("example.test"));
            out.assert_code(1);
            assert_eq!(fx.server.routes(), vec![Route::ScanResults]);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Exit code rules and combined probes
// ---------------------------------------------------------------------------------------

mod exit_codes {
    use super::*;

    #[test]
    fn warnings_and_skips_alone_exit_zero() {
        let (fx, _) = Fx::with(|config| {
            config.remove("mgmtClientId");
            config.set("unknownKey", 1);
        });
        fx.server.set_default(
            Route::ScanResults,
            Planned::empty_json(429).with_retry_after(0),
        );
        let out = fx.run_with(
            &["doctor", "--output", "json"],
            RunOptions::new().env("PANW_X", "1"),
        );
        let all = checks(&out);
        assert!(all.iter().all(|check| check.status != "fail"), "{all:?}");
        // Environment, Scanner API (429) and Management credentials; the unknown
        // TypeScript key was dropped at provisioning.
        assert_eq!(
            all.iter().filter(|check| check.status == "warn").count(),
            3,
            "{all:?}"
        );
        out.assert_success();
    }

    #[test]
    fn any_failure_exits_one_with_all_checks_reported() {
        let (fx, _) = Fx::healthy();
        fx.server
            .set_default(Route::Token, Planned::empty_json(401));
        let out = fx.json();
        out.assert_code(1);
        assert_eq!(all_checks(&out).len(), 9);
        assert_eq!(out.stderr, "");
    }

    #[test]
    fn both_probes_hanging_take_two_budgets() {
        let (fx, _) = Fx::healthy();
        fx.server.set_default(Route::ScanResults, Planned::hang());
        fx.server.set_default(Route::Token, Planned::hang());
        let out = fx.json();
        out.assert_code(1);
        let all = checks(&out);
        assert_eq!(get(&all, "Scanner API").detail, TIMEOUT);
        assert_eq!(get(&all, "Management OAuth").detail, TIMEOUT);
        assert!(
            out.elapsed >= Duration::from_millis(9800),
            "{:?}",
            out.elapsed
        );
        assert!(out.elapsed < Duration::from_secs(20), "{:?}", out.elapsed);
        assert!(fx.server.wait_for_disconnects(2, PROBE_WAIT));
        assert_eq!(fx.server.routes(), vec![Route::ScanResults, Route::Token]);
    }
}

// ---------------------------------------------------------------------------------------
// Review fixes: retried 429/5xx under the probe budget, unusable 2xx bodies, echoed
// credentials, JSON serde cannot represent (D44)
// ---------------------------------------------------------------------------------------

mod review_fixes {
    use super::*;

    /// The standard tenant without `airsNumRetries`: the SDK's default retries apply.
    fn default_retries() -> (Fx, FixtureTenant) {
        Fx::with(|config| {
            config.remove("airsNumRetries");
        })
    }

    /// §9, D20, D49: a 429 is "rate limited" (warn, exit 0) after exactly one request,
    /// even with the SDK's default retries, and well inside the 5 s budget.
    #[test]
    fn persistent_429_warns_after_one_request() {
        let (fx, _) = default_retries();
        fx.server
            .set_default(Route::ScanResults, Planned::empty_json(429));
        let out = fx.json();
        out.assert_success();
        assert_eq!(
            *get(&checks(&out), "Scanner API"),
            c(
                "Scanner API",
                "warn",
                "endpoint reachable, but rate limited (HTTP 429): AISEC_CLIENT_SIDE_ERROR:API error 429"
            )
            .hint("Retry later")
        );
        assert_eq!(
            fx.server.requests_for(Route::ScanResults).len(),
            1,
            "not retried (D49)"
        );
        assert!(out.elapsed < Duration::from_secs(4), "{:?}", out.elapsed);
    }

    #[test]
    fn persistent_503_with_retry_after_is_a_scanner_api_error() {
        let (fx, _) = default_retries();
        fx.server.set_default(
            Route::ScanResults,
            Planned::empty_json(503).with_retry_after(3),
        );
        let out = fx.json();
        out.assert_code(1);
        assert_eq!(
            *get(&checks(&out), "Scanner API"),
            c(
                "Scanner API",
                "fail",
                "scanner API error (HTTP 503): AISEC_SERVER_SIDE_ERROR:API error 503"
            )
            .hint("Retry later; check the AIRS service status")
        );
    }

    #[test]
    fn persistent_503_on_topics_is_a_management_api_error() {
        let (fx, _) = Fx::healthy();
        fx.server
            .set_default(Route::Topics, Planned::empty_json(503));
        let out = fx.json();
        out.assert_code(1);
        assert_eq!(
            *get(&checks(&out), "Management OAuth"),
            c(
                "Management OAuth",
                "fail",
                "management API error (HTTP 503): AISEC_SERVER_SIDE_ERROR:API error 503"
            )
            .hint(VERIFY_MGMT)
        );
    }

    /// A token that worked and a 2xx the CLI cannot use is an endpoint problem: no
    /// "HTTP 200" error and no credential hint.
    #[test]
    fn malformed_2xx_topics_body_points_at_the_endpoint() {
        let (fx, _) = Fx::healthy();
        fx.server.set_default(
            Route::Topics,
            Planned::json(200, &json!({"custom_topics": "nope"})),
        );
        let out = fx.json();
        out.assert_code(1);
        assert_eq!(
            *get(&checks(&out), "Management OAuth"),
            c(
                "Management OAuth",
                "fail",
                "management API error: AISEC_RESPONSE_VALIDATION:Response did not match the expected schema"
            )
            .hint("Fix mgmtEndpoint in the tenant file")
        );
    }

    /// §6: the HTTP Basic client pair (base64, standard and URL-safe, unpadded) is
    /// registered for doctor exactly as for commands.
    #[test]
    fn basic_credentials_echoed_in_an_error_code_are_withheld() {
        let pair = "Y2xpZW50LTEwMDpGQUtFLVNFQ1JFVC0xMDA"; // base64("client-100:FAKE-SECRET-100")
        for code in [pair.to_owned(), format!("abc_{pair}")] {
            let (fx, _) = Fx::healthy();
            fx.server.set_default(
                Route::Topics,
                Planned::json(409, &json!({"errorCode": code})).with_header("x-request-id", &code),
            );
            let out = fx.json();
            out.assert_code(1);
            assert_eq!(
                get(&checks(&out), "Management OAuth").detail,
                "management API error (HTTP 409): AISEC_CLIENT_SIDE_ERROR:API error 409"
            );
        }
    }

    /// §6: the probe's token echoed by a 401 is withheld. D49: the probe makes one
    /// attempt, so there is no refresh and no second token.
    #[test]
    fn a_token_echoed_by_a_401_topics_probe_is_withheld_without_a_refresh() {
        let (fx, _) = Fx::healthy();
        fx.server.set_rotating_tokens(true);
        fx.server.set_default(
            Route::Topics,
            Planned::json(401, &json!({"code": Planned::BEARER}))
                .with_header("x-request-id", Planned::BEARER)
                .echoing_bearer(),
        );
        let out = fx.json();
        out.assert_code(1);
        assert_eq!(
            get(&checks(&out), "Management OAuth").detail,
            "management API error (HTTP 401): AISEC_CLIENT_SIDE_ERROR:API error 401"
        );
        // No refresh for the topics probe; the Gateway API probe fetches a new token
        // after the refused lease was dropped (choice 99).
        assert_eq!(
            fx.server.routes()[..4],
            [
                Route::ScanResults,
                Route::Token,
                Route::Topics,
                Route::Token
            ],
            "no refresh before the gateway probe"
        );
        assert_eq!(fx.server.requests_for(Route::Topics).len(), 1, "no replay");
    }

    /// D44: JSON that `JSON.parse` accepts but serde_json cannot represent is named as
    /// such instead of "not valid JSON".
    #[test]
    fn unrepresentable_json_is_named_as_such() {
        // D44 now applies only to `tenant import` (§3.13): doctor reads the TOML store,
        // so TypeScript JSON that serde_json cannot represent is never parsed here.
        for extra in [
            r#""x": 1e400"#.to_owned(),
            r#""x": "\ud800""#.to_owned(),
            format!(r#""x": {}{}"#, "[".repeat(200), "]".repeat(200)),
        ] {
            let (fx, tenant) = Fx::healthy();
            fx.sandbox.detach_store();
            let text = tenant.config.to_json();
            let body = text.trim_end().trim_end_matches('}').trim_end();
            fx.sandbox
                .write_config_raw("prod", format!("{body},\n  {extra}\n}}\n").as_bytes());
            let out = fx.json();
            out.assert_success();
            assert_eq!(
                *get(&checks(&out), "Config file"),
                config_pass(&tenant),
                "{extra:.30}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Gateway API (DESIGN §19.16, D250; TS `unit/cli/doctor.spec.ts` AI Gateway cases)
// ---------------------------------------------------------------------------------------

mod gateway_api {
    use super::*;

    const GRANT_HINT: &str = "The service account is missing a workspace-scope grant (data plane, /ai_gw/v2). SCM Access Management edits the existing role row by default — use \"Add Role\" so the account ends up with both role rows, not one row moved.";
    const VERIFY: &str = "Verify the tenant credentials and network connectivity";

    fn gateway(fx: &Fx) -> (C, Run) {
        let out = fx.json();
        assert_eq!(out.stderr, "");
        (get(&checks(&out), "Gateway API").clone(), out)
    }

    /// The pass counts the data-plane workspaces in scope (`1 workspace`); the probe shares
    /// the Management OAuth token (one token request, choice 52).
    #[test]
    fn a_2xx_counts_the_workspaces_in_scope_with_one_token() {
        let (fx, _) = Fx::healthy();
        fx.server.seed_gateway();
        let (check, out) = gateway(&fx);
        assert_eq!(check, gateway_pass("1 workspace"));
        out.assert_success();
        assert_eq!(fx.server.requests_for(Route::Token).len(), 1);
        assert_gateway_request(&fx.server.requests_for(GATEWAY)[0], "100");
        let (fx, _) = Fx::healthy();
        fx.server.seed_gateway();
        fx.server.gateway(|data| {
            data.data_plane
                .insert(airs_test_harness::fixtures::gateway::uuid(2));
        });
        assert_eq!(gateway(&fx).0, gateway_pass("2 workspaces"));
    }

    /// D250: every 403 warns with the data-plane grant hint, whatever the service code (TS:
    /// the admin-plane hint unless `AB03` is in the message), and the probe is sent once
    /// (TS refreshes and replays).
    #[test]
    fn a_403_warns_with_the_data_plane_hint() {
        for body in [
            json!({"errorCode": "AB03", "message": "PRIVATE-TEXT"}),
            json!({"data": {"errorCode": "AB04"}}),
            json!({}),
        ] {
            let (fx, _) = Fx::healthy();
            fx.server.set_default(GATEWAY, Planned::json(403, &body));
            let (check, out) = gateway(&fx);
            assert_eq!(check.status, "warn", "{body}");
            assert!(
                check
                    .detail
                    .starts_with("endpoint reachable, but access denied (HTTP 403): "),
                "{check:?}"
            );
            assert_eq!(check.hint.as_deref(), Some(GRANT_HINT));
            assert!(!out.stdout.contains("PRIVATE-TEXT"));
            out.assert_success();
            assert_eq!(fx.server.requests_for(GATEWAY).len(), 1);
        }
    }

    /// D49, D250: one attempt; 429 warns, 5xx and other statuses fail at once.
    #[test]
    fn statuses_are_classified_after_one_attempt() {
        for (status, expected, hint, code) in [
            (429, "warn", "Retry later", 0),
            (500, "fail", "Retry later; check the AIRS service status", 1),
            (503, "fail", "Retry later; check the AIRS service status", 1),
            (401, "fail", VERIFY, 1),
            (404, "fail", VERIFY, 1),
        ] {
            let (fx, _) = Fx::healthy();
            fx.server.set_default(
                GATEWAY,
                Planned::json(status, &json!({"message": "PRIVATE-TEXT"})).with_retry_after(0),
            );
            let (check, out) = gateway(&fx);
            out.assert_code(code);
            assert_eq!(check.status, expected, "{status}");
            assert_eq!(check.hint.as_deref(), Some(hint), "{status}");
            let prefix = if status == 429 {
                "endpoint reachable, but rate limited (HTTP 429): ".to_owned()
            } else {
                format!("gateway API error (HTTP {status}): ")
            };
            assert!(check.detail.starts_with(&prefix), "{check:?}");
            assert!(!check.detail.contains("PRIVATE-TEXT"));
            assert_eq!(
                fx.server.requests_for(GATEWAY).len(),
                1,
                "{status}: one attempt"
            );
        }
    }

    /// A 2xx that does not decode is a gateway API error, not `network unreachable` (TS).
    #[test]
    fn a_malformed_2xx_names_the_endpoint_setting() {
        let (fx, _) = Fx::healthy();
        fx.server
            .set_default(GATEWAY, Planned::json(200, &json!({"object": 5})));
        let (check, out) = gateway(&fx);
        out.assert_code(1);
        assert_eq!(check.status, "fail");
        assert!(check.detail.starts_with("gateway API error: "), "{check:?}");
        assert_eq!(
            check.hint.as_deref(),
            Some("Fix gatewayDataEndpoint in the tenant file")
        );
    }

    #[test]
    fn transport_failures_and_timeouts() {
        let closed = ClosedPort::new();
        let (fx, _) = Fx::with(|config| {
            config.set(
                "aiGwDataEndpoint",
                format!("http://127.0.0.1:{}/ai_gw/v2", closed.port()),
            );
        });
        let (check, _) = gateway(&fx);
        assert_eq!(
            check,
            c(
                "Gateway API",
                "fail",
                format!("network unreachable: {NETWORK_FAILED}")
            )
            .hint("Check network connectivity, proxy settings, and DNS")
        );
        let (fx, _) = Fx::healthy();
        fx.server.set_default(GATEWAY, Planned::hang());
        let (check, out) = gateway(&fx);
        out.assert_code(1);
        assert_eq!(
            check,
            c("Gateway API", "fail", TIMEOUT)
                .hint("Check network connectivity and the gatewayDataEndpoint setting")
        );
        assert!(out.elapsed < Duration::from_secs(15));
    }

    /// An inadmissible `gatewayDataEndpoint` fails locally with the D48 message (the value
    /// never echoed) and sends no gateway request.
    #[test]
    fn an_inadmissible_endpoint_fails_without_a_request() {
        let (fx, _) = Fx::with_external_endpoints(|config| {
            config.set("aiGwDataEndpoint", "http://gateway.example.test/ai_gw/v2");
        });
        let (check, _) = gateway(&fx);
        assert_eq!(
            check,
            c(
                "Gateway API",
                "fail",
                format!("Invalid gatewayDataEndpoint: {ENDPOINT_RULE}")
            )
            .hint("Fix gatewayDataEndpoint in the tenant file")
        );
        assert!(fx.server.requests_for(GATEWAY).is_empty());
    }

    /// Choice 52: a token failure leaves the probe not evaluated (no second token
    /// request); missing management credentials skip it.
    #[test]
    fn gating_follows_the_management_checks() {
        let (fx, _) = Fx::healthy();
        fx.server
            .set_default(Route::Token, Planned::empty_json(401));
        let (check, _) = gateway(&fx);
        assert_eq!(check, ne("Gateway API"));
        assert_eq!(fx.server.requests_for(Route::Token).len(), 1);
        assert!(fx.server.requests_for(GATEWAY).is_empty());
        let (fx, _) = Fx::with(|config| {
            config.remove("mgmtClientSecret");
        });
        let (check, _) = gateway(&fx);
        assert_eq!(check, c("Gateway API", "skip", MGMT_OAUTH_SKIP));
        assert!(fx.server.requests_for(GATEWAY).is_empty());
        // The topics probe failing does not stop the gateway probe.
        let (fx, _) = Fx::healthy();
        fx.server
            .set_default(Route::Topics, Planned::empty_json(500));
        assert_eq!(gateway(&fx).0, gateway_pass("0 workspaces"));
    }

    /// The table and pretty layouts carry the eighth row.
    #[test]
    fn layouts_list_the_eighth_check() {
        let (fx, _) = Fx::healthy();
        let out = fx.run(&["doctor", "--output", "table"]);
        out.assert_success();
        assert!(
            out.stdout
                .lines()
                .any(|line| line.starts_with(" Gateway API ")
                    && line.contains("endpoint reachable (0 workspaces in scope)")),
            "{}",
            out.stdout
        );
        let (fx, _) = Fx::healthy();
        fx.server.set_default(GATEWAY, Planned::empty_json(403));
        let out = fx.run(&["doctor"]);
        out.assert_success();
        assert!(
            out.stdout
                .contains("  ⚠ Gateway API — endpoint reachable, but access denied (HTTP 403): ")
        );
        assert!(out.stdout.contains(&format!("\n      {GRANT_HINT}\n")));
        assert!(out.stdout.contains("All checks passed (1 warning)"));
    }
}

// ---------------------------------------------------------------------------------------
// The Executable check (8.0, DESIGN §21.15, D331)
// ---------------------------------------------------------------------------------------

mod executable {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    const HINT: &str = "Remove the other installation or put this one first on PATH; install airs through one channel only (Homebrew, npm, cargo or a release tarball)";

    /// An executable file at `dir/name` (a shell script that would print `other`).
    fn program(dir: &Path, name: &str) -> std::path::PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\necho other\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn canonical(path: &Path) -> String {
        std::fs::canonicalize(path).unwrap().display().to_string()
    }

    /// The directory of the binary under test, which holds `airs`, `airs-cli` and
    /// `airs-cli-rs` (one copy per `[[bin]]`).
    fn own_dir() -> String {
        Path::new(env!("CARGO_BIN_EXE_airs"))
            .parent()
            .unwrap()
            .display()
            .to_string()
    }

    fn run_doctor(fx: &Fx, path: &str, extra: &[(&str, String)]) -> Run {
        let mut options = RunOptions::new().env("PATH", path);
        for (name, value) in extra {
            options = options.env(name, value.as_str());
        }
        fx.run_with(&["doctor", "--output", "json"], options)
    }

    /// Another `airs` earlier on `PATH` is a warning naming its path; it never fails the
    /// run, and the other checks are unchanged.
    #[test]
    fn another_airs_first_on_path_warns() {
        let (fx, tenant) = Fx::healthy();
        let bin = fx.sandbox.bin_dir();
        let other = program(&bin, "airs");
        let out = run_doctor(
            &fx,
            &format!("{}:{}:/usr/bin:/bin", bin.display(), own_dir()),
            &[],
        );
        out.assert_success();
        let all = all_checks(&out);
        assert_eq!(
            all[0],
            c(
                "Executable",
                "warn",
                format!("airs on PATH is another program: {}", canonical(&other))
            )
            .hint(HINT)
        );
        assert_eq!(all[1..], healthy_checks(&fx, &tenant)[..]);
    }

    /// The binary's own directory (all three names) and a link to the binary under
    /// another name pass; a link to another file names both paths.
    #[test]
    fn its_own_names_and_links_to_it_pass() {
        let (fx, _) = Fx::healthy();
        let out = run_doctor(&fx, &format!("{}:/usr/bin:/bin", own_dir()), &[]);
        out.assert_success();
        assert_eq!(all_checks(&out)[0], exe_pass());

        let links = fx.sandbox.bin_dir();
        std::fs::create_dir_all(&links).unwrap();
        symlink(env!("CARGO_BIN_EXE_airs"), links.join("airs-cli")).unwrap();
        let out = run_doctor(&fx, &format!("{}:/usr/bin:/bin", links.display()), &[]);
        assert_eq!(all_checks(&out)[0], exe_pass());

        let other = program(&fx.sandbox.tmp().join("elsewhere"), "tool");
        symlink(&other, links.join("airs-cli-rs")).unwrap();
        let out = run_doctor(&fx, &format!("{}:/usr/bin:/bin", links.display()), &[]);
        out.assert_success();
        assert_eq!(
            all_checks(&out)[0],
            c(
                "Executable",
                "warn",
                format!(
                    "airs-cli-rs on PATH is another program: {} ({})",
                    links.join("airs-cli-rs").display(),
                    canonical(&other)
                )
            )
            .hint(HINT)
        );
    }

    /// Started by the npm launcher, the npm links to that launcher are this program
    /// (`AIRS_NPM_LAUNCHER`, §21.16); without the variable they are another program.
    #[test]
    fn the_npm_launcher_that_started_it_is_this_program() {
        let (fx, _) = Fx::healthy();
        let launcher = program(&fx.sandbox.tmp().join("npm/lib/bin"), "airs.js");
        let bin = fx.sandbox.bin_dir();
        std::fs::create_dir_all(&bin).unwrap();
        symlink(&launcher, bin.join("airs")).unwrap();
        symlink(&launcher, bin.join("airs-cli")).unwrap();
        let path = format!("{}:/usr/bin:/bin", bin.display());
        let out = run_doctor(
            &fx,
            &path,
            &[("AIRS_NPM_LAUNCHER", launcher.display().to_string())],
        );
        assert_eq!(all_checks(&out)[0], exe_pass());
        let out = run_doctor(&fx, &path, &[]);
        let check = &all_checks(&out)[0];
        assert_eq!(check.status, "warn");
        assert_eq!(
            check.detail,
            format!(
                "airs on PATH is another program: {} ({launcher}); airs-cli on PATH is another program: {} ({launcher})",
                bin.join("airs").display(),
                bin.join("airs-cli").display(),
                launcher = canonical(&launcher)
            )
        );
        // A relative launcher path is ignored.
        let out = run_doctor(&fx, &path, &[("AIRS_NPM_LAUNCHER", "airs.js".to_owned())]);
        assert_eq!(all_checks(&out)[0].status, "warn");
    }

    /// The check is evaluated before the tenant is read, so a store failure does not
    /// hide it; pretty output shows it first with the warning glyph and counts it.
    #[test]
    fn it_runs_before_the_tenant_and_shows_first() {
        let fx = Fx::new();
        fx.sandbox.write_store_raw(b"version = ", 0o600);
        let bin = fx.sandbox.bin_dir();
        program(&bin, "airs-cli");
        let path = format!("{}:/usr/bin:/bin", bin.display());
        let out = run_doctor(&fx, &path, &[]);
        out.assert_code(1);
        let all = all_checks(&out);
        assert_eq!(all[0].status, "warn");
        assert_eq!(all[1].name, "Tenant");
        assert_eq!(all[1].status, "fail");
        assert!(all[2..].iter().all(|check| check.detail == NOT_EVALUATED));

        let (fx, _) = Fx::healthy();
        let bin = fx.sandbox.bin_dir();
        program(&bin, "airs-cli");
        let out = fx.run_with(
            &["doctor"],
            RunOptions::new().env("PATH", format!("{}:/usr/bin:/bin", bin.display())),
        );
        out.assert_success();
        let first = format!(
            "{HEADER}  ⚠ Executable — airs-cli on PATH is another program: {}\n      {HINT}\n  ✓ Tenant — ",
            canonical(&bin.join("airs-cli"))
        );
        assert!(out.stdout.starts_with(&first), "{}", out.stdout);
        assert!(
            out.stdout
                .ends_with("\n\n  ✓ All checks passed (1 warning)\n\n"),
            "{}",
            out.stdout
        );
    }
}
