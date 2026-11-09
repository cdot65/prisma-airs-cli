//! `doctor`: nine scoped preflight checks (DESIGN §9; `Gateway API` from 0.10, §19.16;
//! `Executable` from 8.0, §21.15).
//!
//! Probes run sequentially, each bounded by one 5 s `RequestOptions` budget that
//! really cancels the request. Details carry key names, paths and the registered
//! TSG only; never values from the file or server text.

use super::Invocation;
use crate::clients::{self, LastStatus, ManagementClients, PROBE_BUDGET, RecordingProvider};
use crate::error::{
    self, CliError, ErrorFacts, InterruptContext, Operation, Phase, Plane, SdkContext,
};
use crate::output::{Format, terminal_safe};
use crate::render::style::Style;
use crate::render::{pretty, table};
use crate::store::PROGRAM;
use crate::tenant::config::{self, ContentError, TenantConfig};
use crate::tenant::{ConfigLoadError, Selected, StoreView};
use scm::airs::management::TopicListAllOptions;
use scm::{ErrorKind, RequestOptions};
use serde::Serialize;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Warn,
    Fail,
    Skip,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

pub const EXECUTABLE: &str = "Executable";
pub const TENANT: &str = "Tenant";
pub const CONFIG_FILE: &str = "Config file";
pub const ENVIRONMENT: &str = "Environment";
pub const SCANNER_CREDENTIALS: &str = "Scanner credentials";
pub const MANAGEMENT_CREDENTIALS: &str = "Management credentials";
pub const SCANNER_API: &str = "Scanner API";
pub const MANAGEMENT_OAUTH: &str = "Management OAuth";
pub const GATEWAY_API: &str = "Gateway API";

const NOT_EVALUATED: &str = "not evaluated — fix the failed checks above first";
const VERIFY_MANAGEMENT: &str =
    "Verify mgmtClientId / mgmtClientSecret / mgmtTsgId belong to this tenant";

/// The probe timeout detail, derived from the probe budget (TS wording).
fn timeout_detail() -> String {
    format!(
        "timed out after {}ms — network unreachable or endpoint not responding",
        PROBE_BUDGET.as_millis()
    )
}

impl Check {
    fn new(name: &'static str, status: Status, detail: impl Into<String>) -> Self {
        Self {
            name,
            status,
            detail: detail.into(),
            hint: None,
        }
    }
    fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    fn not_evaluated(name: &'static str) -> Self {
        Self::new(name, Status::Skip, NOT_EVALUATED)
    }
    fn is_not_evaluated(&self) -> bool {
        self.status == Status::Skip && self.detail == NOT_EVALUATED
    }
}

/// `plural(n, noun)` from the TypeScript doctor.
pub fn plural(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

/// `✓ All checks passed[ (...)]` or `✗ N check(s) failed`, and whether it failed.
pub fn summary(checks: &[Check]) -> (bool, String) {
    let count = |status| checks.iter().filter(|c| c.status == status).count();
    let (fails, warns, skips) = (
        count(Status::Fail),
        count(Status::Warn),
        count(Status::Skip),
    );
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
        (false, "All checks passed".to_owned())
    } else {
        (false, format!("All checks passed ({})", parts.join(", ")))
    }
}

/// The names this program answers to on `PATH` (DESIGN §21.2): `airs`, the TypeScript
/// CLI's name and the 0.x/1.x compatibility name, i.e. `airs`, `airs-cli` or `airs-cli-rs`.
pub const PROGRAM_NAMES: [&str; 3] = ["airs", "airs-cli", "airs-cli-rs"];

/// The environment variable the npm launcher sets to its own real path (§21.16), so the
/// npm links to it count as this program.
pub const NPM_LAUNCHER_VAR: &str = "AIRS_NPM_LAUNCHER";

const EXECUTABLE_PASS: &str = "no other airs, airs-cli or airs-cli-rs earlier on PATH";
const EXECUTABLE_HINT: &str = "Remove the other installation or put this one first on PATH; install airs through one channel only (Homebrew, npm, cargo or a release tarball)";

/// What the Executable check knows about this process (§21.15).
pub struct RunningProgram {
    /// The running executable, canonical; `None` when it cannot be determined.
    pub exe: Option<PathBuf>,
    /// The npm launcher that started it (`AIRS_NPM_LAUNCHER`, canonical), if any.
    pub launcher: Option<PathBuf>,
}

impl RunningProgram {
    /// This process: `current_exe` and `AIRS_NPM_LAUNCHER` (when absolute), canonical.
    pub fn current() -> Self {
        let canonical = |path: PathBuf| std::fs::canonicalize(path).ok();
        Self {
            exe: std::env::current_exe().ok().and_then(canonical),
            launcher: std::env::var_os(NPM_LAUNCHER_VAR)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .and_then(canonical),
        }
    }

    /// Whether `file` (canonical) is this program: the executable itself, another of its
    /// names in the executable's own directory (`cargo install` and `cargo build` write
    /// one copy per name there), or the npm launcher that started it.
    fn is(&self, file: &Path) -> bool {
        let Some(exe) = &self.exe else { return false };
        if file == exe || self.launcher.as_deref() == Some(file) {
            return true;
        }
        file.parent() == exe.parent()
            && file
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| PROGRAM_NAMES.contains(&name))
    }
}

/// The file a shell runs for `name`: the first absolute `PATH` entry holding an
/// executable regular file (after symlinks) of that name, and its canonical path.
fn first_on_path(path: &OsStr, name: &str) -> Option<(PathBuf, PathBuf)> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(path)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| {
            std::fs::metadata(candidate)
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
        .and_then(|found| {
            let canonical = std::fs::canonicalize(&found).ok()?;
            Some((found, canonical))
        })
}

/// The Executable check (§21.15): warns when another `airs`, `airs-cli` or `airs-cli-rs`
/// earlier on `PATH` resolves to a different file than this program; never fails.
pub fn check_executable(path: Option<&OsStr>, program: &RunningProgram) -> Check {
    if program.exe.is_none() {
        return Check::new(
            EXECUTABLE,
            Status::Skip,
            "the running executable cannot be determined",
        );
    }
    let others: Vec<String> = PROGRAM_NAMES
        .iter()
        .filter_map(|name| {
            let (found, canonical) = first_on_path(path?, name)?;
            if program.is(&canonical) {
                return None;
            }
            Some(if found == canonical {
                format!("{name} on PATH is another program: {}", found.display())
            } else {
                format!(
                    "{name} on PATH is another program: {} ({})",
                    found.display(),
                    canonical.display()
                )
            })
        })
        .collect();
    if others.is_empty() {
        Check::new(EXECUTABLE, Status::Pass, EXECUTABLE_PASS)
    } else {
        Check::new(EXECUTABLE, Status::Warn, others.join("; ")).hint(EXECUTABLE_HINT)
    }
}

/// The store as doctor sees it: where `D` is, and `config.toml` when it was read.
pub enum StoreState<'a> {
    /// No store directory could be resolved (§3.1).
    Unresolved(&'a CliError),
    /// `D` resolved but `config.toml` (or `D`) could not be used (§3.2, §3.8).
    Failed(&'a Path, &'a CliError),
    /// `config.toml` was read.
    Read(&'a StoreView),
}

/// The Tenant check (§9).
pub fn check_tenant(
    store: StoreState<'_>,
    selection: Option<Result<Option<&Selected>, &CliError>>,
) -> Check {
    let message = CliError::message;
    let view = match store {
        StoreState::Unresolved(error) => {
            return Check::new(TENANT, Status::Fail, message(error))
                .hint("Set AIRS_CLI_CONFIG_DIR or XDG_CONFIG_HOME to an absolute path");
        }
        StoreState::Failed(index, error) => {
            return Check::new(TENANT, Status::Fail, message(error))
                .hint(format!("Fix or restore {}", index.display()));
        }
        StoreState::Read(view) => view,
    };
    let index_path = view.store.index_path();
    let index_path = index_path.display();
    match selection {
        Some(Ok(Some(selected))) => Check::new(
            TENANT,
            Status::Pass,
            format!(
                "{} (TSG {}) selected{} in {index_path}",
                selected.name,
                selected.tsg_id,
                if selected.by_flag { " by --tenant" } else { "" },
            ),
        ),
        Some(Err(error)) => Check::new(TENANT, Status::Fail, message(error)),
        _ if view.index.tenants.is_empty() => Check::new(
            TENANT,
            Status::Fail,
            format!("no tenants registered in {index_path}"),
        )
        .hint(format!(
            "Run '{PROGRAM} tenant create <name>' (prompts for TSG ID, client ID and secret), then '{PROGRAM} tenant switch <name>'"
        )),
        _ => Check::new(
            TENANT,
            Status::Fail,
            format!(
                "no tenant selected (registered: {})",
                view.index.names().join(", ")
            ),
        )
        .hint(format!("Run '{PROGRAM} tenant switch <name>'")),
    }
}

/// The Config file check and, unless it failed, the validated config. Uses the same
/// strict reader as every command ([`StoreView::load_checked`], D21).
pub fn check_config(view: &StoreView, selected: &Selected) -> (Check, Option<TenantConfig>) {
    let tenant = &selected.name;
    let path = view.tenant_path(tenant);
    let path = path.display();
    let fail = |detail: String| Check::new(CONFIG_FILE, Status::Fail, detail);
    let parsed = match view.load_checked(selected) {
        Ok(parsed) => parsed,
        Err(error) => {
            let check = match error {
                ConfigLoadError::NotFound => fail(format!("not found at {path}")).hint(format!(
                    "Restore the file, or delete the tenant with '{PROGRAM} tenant delete {tenant}' and create it again"
                )),
                ConfigLoadError::Unsafe(unsafe_path) => fail(format!(
                    "{} is not private: it must be a regular file (not a symlink) owned by the current user with no group or other permissions",
                    unsafe_path.display()
                ))
                .hint("Make it a regular file owned by you with mode 0600 (directories 0700), then re-run doctor"),
                ConfigLoadError::Unreadable => fail(format!(
                    "{path} cannot be read (not a regular file, larger than 1 MiB, or unreadable)"
                )),
                ConfigLoadError::Content(ContentError::InvalidToml) => {
                    fail(format!("{path} is not valid TOML")).hint("Fix the file, then re-run doctor")
                }
                ConfigLoadError::Content(ContentError::UnknownKeys(keys)) => {
                    let hint = format!(
                        "Remove them from the file; '{PROGRAM} tenant set {tenant} <key>' accepts only the supported keys"
                    );
                    fail(format!("{path} has unknown keys: {}", keys.join(", "))).hint(hint)
                }
                ConfigLoadError::Content(ContentError::InvalidValues(keys)) => {
                    // Not `tenant set`: it reads the whole file strictly first, so it
                    // refuses a file with an invalid value (D89).
                    let mut hint = format!(
                        "Correct them in the file, then re-run doctor (values are never printed here; '{PROGRAM} tenant set' refuses an invalid file)"
                    );
                    if keys.contains(&"defaultOutput") {
                        // D51: the dropped formats make the config invalid.
                        hint.push_str("; defaultOutput accepts only pretty, table or json (markdown, csv and yaml were dropped)");
                    }
                    fail(format!("{path} has invalid values for: {}", keys.join(", "))).hint(hint)
                }
                ConfigLoadError::TsgMismatch => fail(format!(
                    "{path} carries a different mgmtTsgId than the registration (TSG {})",
                    selected.tsg_id
                ))
                .hint(format!(
                    "Choose the TSG with '{PROGRAM} tenant set {tenant} mgmtTsgId <tsg>'; it updates both files"
                )),
            };
            return (check, None);
        }
    };
    (
        Check::new(
            CONFIG_FILE,
            Status::Pass,
            format!(
                "valid tenant config at {path} (TSG {} matches registration)",
                selected.tsg_id
            ),
        ),
        Some(parsed),
    )
}

pub fn check_environment(ignored: &[String], proxies: &[&str]) -> Check {
    let suffix = if proxies.is_empty() {
        String::new()
    } else {
        format!("; proxy variables in effect: {}", proxies.join(", "))
    };
    if ignored.is_empty() {
        Check::new(
            ENVIRONMENT,
            Status::Pass,
            format!("no configuration variables set{suffix}"),
        )
    } else {
        Check::new(
            ENVIRONMENT,
            Status::Warn,
            format!(
                "ignored {}: {}{suffix}",
                plural(ignored.len(), "variable"),
                ignored.join(", ")
            ),
        )
        .hint(format!("Configuration comes only from tenant files ('{PROGRAM} tenant set <name> <key>'); unset these"))
    }
}

pub fn check_scanner_credentials(config: Option<&TenantConfig>, tenant: &str) -> Check {
    let Some(config) = config else {
        return Check::not_evaluated(SCANNER_CREDENTIALS);
    };
    let keys = config.scanner_keys();
    if keys.is_empty() {
        if let Some(named) = config.scanners().filter(|named| !named.is_empty()) {
            return Check::new(
                SCANNER_CREDENTIALS,
                Status::Pass,
                format!(
                    "{} named scanner(s) configured; scan and bulk-scan require --scanner; bound copies do not follow flat credential rotation",
                    named.len()
                ),
            );
        }
        return Check::new(
            SCANNER_CREDENTIALS,
            Status::Skip,
            "not configured — runtime scan, bulk-scan and topics eval are unavailable",
        )
        .hint(config::setting_remedy(Some(tenant), &["airsApiKey"]));
    }
    let detail: Vec<String> = keys.iter().map(|key| format!("{key} (file)")).collect();
    let mut detail = detail.join(", ");
    if let Some(named) = config.scanners().filter(|named| !named.is_empty()) {
        detail.push_str(&format!(
            "; {} named scanner(s); bound copies do not follow flat credential rotation",
            named.len()
        ));
    }
    Check::new(SCANNER_CREDENTIALS, Status::Pass, detail)
}

pub fn check_management_credentials(
    config: Option<&TenantConfig>,
    tenant: &str,
    scanner_passed: bool,
) -> Check {
    let Some(config) = config else {
        return Check::not_evaluated(MANAGEMENT_CREDENTIALS);
    };
    let missing = config.missing_management();
    if missing.is_empty() {
        return Check::new(
            MANAGEMENT_CREDENTIALS,
            Status::Pass,
            "set: mgmtClientId (file), mgmtClientSecret (file), mgmtTsgId (file)",
        );
    }
    Check::new(
        MANAGEMENT_CREDENTIALS,
        if scanner_passed { Status::Warn } else { Status::Fail },
        format!(
            "missing: {} — management, red team, model security, gateway and supplychain commands are unavailable",
            missing.join(", ")
        ),
    )
    .hint(config::setting_remedy(Some(tenant), &missing))
}

/// One attempt per probe request (§9, D49): a preflight reports a 429/5xx at once
/// instead of retrying into its budget, and adds no load to a throttled endpoint.
fn probe_options() -> Result<RequestOptions, CliError> {
    RequestOptions::with_timeout(PROBE_BUDGET)
        .and_then(|options| options.with_retries(0))
        .map_err(|_| CliError::operational("AISEC_SDK_ERROR:Invalid probe budget"))
}

fn probe_context<'a>(invocation: &'a Invocation, plane: Plane, phase: Phase) -> SdkContext<'a> {
    SdkContext {
        plane,
        phase,
        operation: Operation::Read,
        timeout: PROBE_BUDGET,
        secrets: &invocation.secrets,
    }
}

/// The facts of a probe API failure. The SDK retries 429/5xx answers inside the probe
/// budget, so a budget that ends while it waits to retry is a `Deadline` although the
/// endpoint answered every attempt. The last HTTP error status received (from the
/// error, else from the client's transport, since a deadline that drops a page fetch
/// loses it) is what the endpoint said, so the probe is classified and described as
/// that HTTP error, not as a timeout.
fn probe_facts(error: &scm::Error, last: &LastStatus) -> ErrorFacts {
    let mut facts = ErrorFacts::of(error);
    if facts.kind == ErrorKind::Deadline && facts.status.is_none() {
        facts.status = last.get();
    }
    last_answer(facts)
}

fn last_answer(mut facts: ErrorFacts) -> ErrorFacts {
    if facts.kind == ErrorKind::Deadline && facts.status.is_some_and(|status| status >= 400) {
        facts.kind = ErrorKind::Http;
    } else if facts.kind == ErrorKind::Deadline {
        facts.status = None;
    }
    facts
}

/// Classify a scanner probe failure (DESIGN §9 Scanner API).
pub fn classify_scanner(facts: &ErrorFacts, message: &str) -> Result<Check, CliError> {
    Ok(match (facts.kind, facts.http_status()) {
        (ErrorKind::Cancelled, _) => {
            return Err(CliError::Interrupted(InterruptContext::NoDataWritten));
        }
        (ErrorKind::Deadline, _) => Check::new(SCANNER_API, Status::Fail, timeout_detail())
            .hint("Check network connectivity and the airsApiEndpoint setting"),
        (_, Some(status @ (401 | 403))) => Check::new(
            SCANNER_API,
            Status::Fail,
            format!("API key rejected (HTTP {status}): {message}"),
        )
        .hint("Verify airsApiKey belongs to this tenant and is not expired"),
        (_, Some(429)) => Check::new(
            SCANNER_API,
            Status::Warn,
            format!("endpoint reachable, but rate limited (HTTP 429): {message}"),
        )
        .hint("Retry later"),
        (_, Some(status)) if status >= 500 => Check::new(
            SCANNER_API,
            Status::Fail,
            format!("scanner API error (HTTP {status}): {message}"),
        )
        .hint("Retry later; check the AIRS service status"),
        (_, Some(status)) => Check::new(
            SCANNER_API,
            Status::Pass,
            format!("endpoint reachable, API key accepted (HTTP {status} on probe query)"),
        ),
        (ErrorKind::Transport | ErrorKind::Io, None) => Check::new(
            SCANNER_API,
            Status::Fail,
            format!("network unreachable: {message}"),
        )
        .hint("Check network connectivity, proxy settings, and DNS"),
        _ => Check::new(SCANNER_API, Status::Fail, message)
            .hint("Fix airsApiEndpoint in the tenant file"),
    })
}

/// Classify a management probe failure; `token` is true for the explicit token fetch.
pub fn classify_management(
    facts: &ErrorFacts,
    message: &str,
    token: bool,
) -> Result<Check, CliError> {
    let fail =
        |detail: String| Check::new(MANAGEMENT_OAUTH, Status::Fail, detail).hint(VERIFY_MANAGEMENT);
    Ok(match facts.kind {
        ErrorKind::Cancelled => {
            return Err(CliError::Interrupted(InterruptContext::NoDataWritten));
        }
        ErrorKind::Deadline => Check::new(MANAGEMENT_OAUTH, Status::Fail, timeout_detail())
            .hint("Check network connectivity and the mgmtTokenEndpoint / mgmtEndpoint settings"),
        ErrorKind::Authentication => match facts.status {
            // Token non-2xx: TS wording (DESIGN §9, ts-baseline-observed.md §2.2).
            Some(status) => fail(format!("management API error (HTTP {status}): {message}")),
            None => fail(format!("authentication failed: {message}")),
        },
        ErrorKind::Decode if token => fail(format!("authentication failed: {message}")),
        ErrorKind::Transport | ErrorKind::Io => fail(format!("network unreachable: {message}")),
        _ if token => fail(message.to_owned()),
        // The API accepted the token but its 2xx answer was unusable: an endpoint
        // problem, not a credential one, and not an "HTTP 200" error.
        ErrorKind::Decode | ErrorKind::Limit if facts.http_status().is_none() => Check::new(
            MANAGEMENT_OAUTH,
            Status::Fail,
            format!("management API error: {message}"),
        )
        .hint("Fix mgmtEndpoint in the tenant file"),
        _ => match facts.http_status() {
            Some(status) => fail(format!("management API error (HTTP {status}): {message}")),
            None => fail(format!("management API error: {message}")),
        },
    })
}

async fn probe_scanner(invocation: &Invocation, config: &TenantConfig) -> Result<Check, CliError> {
    let (scanner, last_status) = match clients::build_observed_scanner(
        config,
        &invocation.session,
        invocation.debug.as_ref(),
    ) {
        Ok(built) => built,
        Err(setup) => {
            let key = setup.key.unwrap_or("airsApiEndpoint");
            return Ok(Check::new(SCANNER_API, Status::Fail, setup.message())
                .hint(format!("Fix {key} in the tenant file")));
        }
    };
    let options = probe_options()?;
    let probe_id = uuid::Uuid::new_v4().to_string();
    match scanner.query_by_scan_ids(&[probe_id], Some(&options)).await {
        Ok(_) => Ok(Check::new(
            SCANNER_API,
            Status::Pass,
            "endpoint reachable, API key accepted",
        )),
        Err(failure) => {
            let facts = probe_facts(&failure, &last_status);
            let message = error::facts_message(
                &facts,
                &probe_context(invocation, Plane::Scanner, Phase::Api),
            );
            classify_scanner(&facts, &message)
        }
    }
}

/// The `Management OAuth` probe, and the provider holding its token when the token
/// request succeeded (the `Gateway API` probe shares it, choice 52).
async fn probe_management(
    invocation: &mut Invocation,
    config: &TenantConfig,
) -> Result<(Check, Option<Arc<RecordingProvider>>), CliError> {
    let clients = match ManagementClients::build(
        config,
        &invocation.session,
        &invocation.secrets,
        invocation.debug.as_ref(),
    ) {
        Ok(clients) => clients,
        Err(setup) => {
            return Ok((
                Check::new(MANAGEMENT_OAUTH, Status::Fail, setup.message()).hint(VERIFY_MANAGEMENT),
                None,
            ));
        }
    };
    let options = probe_options()?;
    // The provider registers every lease (this one and any refresh) in the secrets.
    match clients.provider.get_token(Some(&options)).await {
        Ok(_) => {}
        Err(failure) => {
            let message = error::sdk_message(
                &failure,
                &probe_context(invocation, Plane::Management, Phase::Token),
            );
            return Ok((
                classify_management(&ErrorFacts::of(&failure), &message, true)?,
                None,
            ));
        }
    }
    let topics = clients.management.topics();
    let check = match topics
        .list_all(TopicListAllOptions::default(), Some(&options))
        .await
    {
        Ok(collection) => {
            let count = plural(collection.items.len(), "custom topic");
            Check::new(
                MANAGEMENT_OAUTH,
                Status::Pass,
                if collection.is_complete() {
                    format!("OAuth token obtained, topics API answered ({count})")
                } else {
                    format!("OAuth token obtained, topics API answered (at least {count})")
                },
            )
        }
        Err(failure) => {
            let facts = probe_facts(failure.error(), &clients.last_status);
            let message = error::facts_message(
                &facts,
                &probe_context(invocation, Plane::Management, Phase::Api),
            );
            classify_management(&facts, &message, false)?
        }
    };
    Ok((check, Some(clients.provider.clone())))
}

/// The data-plane grant hint of the `Gateway API` check (the §19.3 text without
/// `⚠ 403: `, capitalised; D250).
pub const GATEWAY_GRANT_HINT: &str = "The service account is missing a workspace-scope grant (data plane, /ai_gw/v2). SCM Access Management edits the existing role row by default — use \"Add Role\" so the account ends up with both role rows, not one row moved.";

/// Classify a `Gateway API` probe failure (§19.16).
pub fn classify_gateway(facts: &ErrorFacts, message: &str) -> Result<Check, CliError> {
    let fail = |detail: String| Check::new(GATEWAY_API, Status::Fail, detail);
    Ok(match (facts.kind, facts.http_status()) {
        (ErrorKind::Cancelled, _) => {
            return Err(CliError::Interrupted(InterruptContext::NoDataWritten));
        }
        (ErrorKind::Deadline, None) => fail(timeout_detail())
            .hint("Check network connectivity and the gatewayDataEndpoint setting"),
        (_, Some(403)) => Check::new(
            GATEWAY_API,
            Status::Warn,
            format!("endpoint reachable, but access denied (HTTP 403): {message}"),
        )
        .hint(GATEWAY_GRANT_HINT),
        (_, Some(429)) => Check::new(
            GATEWAY_API,
            Status::Warn,
            format!("endpoint reachable, but rate limited (HTTP 429): {message}"),
        )
        .hint("Retry later"),
        (_, Some(status)) if status >= 500 => {
            fail(format!("gateway API error (HTTP {status}): {message}"))
                .hint("Retry later; check the AIRS service status")
        }
        (_, Some(status)) => fail(format!("gateway API error (HTTP {status}): {message}"))
            .hint("Verify the tenant credentials and network connectivity"),
        (ErrorKind::Transport | ErrorKind::Io, None) => {
            fail(format!("network unreachable: {message}"))
                .hint("Check network connectivity, proxy settings, and DNS")
        }
        _ => fail(format!("gateway API error: {message}"))
            .hint("Fix gatewayDataEndpoint in the tenant file"),
    })
}

/// The `Gateway API` probe (§19.16): `GET {gatewayDataEndpoint}/workspaces` with the
/// `Management OAuth` probe's token, one attempt within its own 5 s budget.
async fn probe_gateway(
    invocation: &Invocation,
    config: &TenantConfig,
    provider: Arc<RecordingProvider>,
) -> Result<Check, CliError> {
    let (gateway, last_status) = match clients::gateway_probe(
        config,
        provider,
        &invocation.session,
        invocation.debug.as_ref(),
    ) {
        Ok(built) => built,
        Err(setup) => {
            return Ok(Check::new(GATEWAY_API, Status::Fail, setup.message())
                .hint("Fix gatewayDataEndpoint in the tenant file"));
        }
    };
    let options = probe_options()?.without_response_defaults();
    match gateway
        .workspaces()
        .list(&Default::default(), Some(&options))
        .await
        .and_then(|answer| crate::json::page_value(&answer))
    {
        Ok(answer) => {
            let count = answer
                .get("data")
                .and_then(crate::json::OValue::as_array)
                .map_or(0, <[crate::json::OValue]>::len);
            Ok(Check::new(
                GATEWAY_API,
                Status::Pass,
                format!(
                    "endpoint reachable ({} in scope)",
                    plural(count, "workspace")
                ),
            ))
        }
        Err(failure) => {
            let facts = probe_facts(&failure, &last_status);
            let message = error::facts_message(
                &facts,
                &probe_context(invocation, Plane::Management, Phase::Api),
            );
            classify_gateway(&facts, &message)
        }
    }
}

/// Run every check in order: `Executable` first, then the eight of §9.
pub async fn checks(invocation: &mut Invocation) -> Result<Vec<Check>, CliError> {
    // Always evaluated: it reads only PATH and the running executable (§21.15).
    let executable = check_executable(
        std::env::var_os("PATH").as_deref(),
        &RunningProgram::current(),
    );
    let store = invocation.env.store();
    let view = store
        .as_ref()
        .ok()
        .map(|_| StoreView::open(&invocation.env));
    let selection = match &view {
        Some(Ok(view)) => Some(view.select(invocation.tenant_flag.as_deref())),
        _ => None,
    };
    let index_path = store.as_ref().ok().map(crate::store::Store::index_path);
    let state = match (&store, &view, &index_path) {
        (Err(error), _, _) => StoreState::Unresolved(error),
        (Ok(_), Some(Err(error)), Some(path)) => StoreState::Failed(path, error),
        (Ok(_), Some(Ok(view)), _) => StoreState::Read(view),
        _ => unreachable!("a resolved store is always read"),
    };
    let tenant = check_tenant(
        state,
        selection.as_ref().map(|s| s.as_ref().map(Option::as_ref)),
    );
    let store_failed = !matches!(view, Some(Ok(_)));
    if store_failed {
        let mut all = vec![executable, tenant];
        all.extend(
            [
                CONFIG_FILE,
                ENVIRONMENT,
                SCANNER_CREDENTIALS,
                MANAGEMENT_CREDENTIALS,
                SCANNER_API,
                MANAGEMENT_OAUTH,
                GATEWAY_API,
            ]
            .map(Check::not_evaluated),
        );
        return Ok(all);
    }
    let Some(Ok(view)) = view else {
        unreachable!("checked above")
    };
    let selected = selection.and_then(Result::ok).flatten();
    let (config_check, config) = match &selected {
        Some(selected) => check_config(&view, selected),
        None => (Check::not_evaluated(CONFIG_FILE), None),
    };
    if let Some(config) = &config {
        invocation.secrets.register_config(config);
    }
    let environment = check_environment(
        &crate::env::ignored_variables(std::env::vars_os()),
        &crate::env::proxy_variables(std::env::vars_os()),
    );
    let name = selected
        .as_ref()
        .map_or("<name>", |s| s.name.as_str())
        .to_owned();
    let scanner_credentials = check_scanner_credentials(config.as_ref(), &name);
    let management_credentials = check_management_credentials(
        config.as_ref(),
        &name,
        scanner_credentials.status == Status::Pass,
    );
    let scanner_api = match (&config, scanner_credentials.status) {
        (Some(config), Status::Pass) if config.scanner_keys().is_empty() => Check::new(
            SCANNER_API,
            Status::Skip,
            "skipped — named scanner credentials require explicit selection; no scanner was probed",
        ),
        (Some(config), Status::Pass) => probe_scanner(invocation, config).await?,
        _ if scanner_credentials.is_not_evaluated() => Check::not_evaluated(SCANNER_API),
        _ => Check::new(
            SCANNER_API,
            Status::Skip,
            "skipped — no scanner credentials configured",
        ),
    };
    let (management_oauth, provider) = match (&config, management_credentials.status) {
        (Some(config), Status::Pass) => probe_management(invocation, config).await?,
        _ if management_credentials.is_not_evaluated() => {
            (Check::not_evaluated(MANAGEMENT_OAUTH), None)
        }
        _ => (
            Check::new(
                MANAGEMENT_OAUTH,
                Status::Skip,
                "skipped — management credentials not configured",
            ),
            None,
        ),
    };
    // The gateway probe shares the Management OAuth token; without one (a token or
    // client failure) it is not evaluated (choice 52).
    let gateway_api = match (&config, management_credentials.status, provider) {
        (Some(config), Status::Pass, Some(provider)) => {
            probe_gateway(invocation, config, provider).await?
        }
        (_, Status::Pass, None) => Check::not_evaluated(GATEWAY_API),
        _ if management_credentials.is_not_evaluated() => Check::not_evaluated(GATEWAY_API),
        _ => Check::new(
            GATEWAY_API,
            Status::Skip,
            "skipped — management credentials not configured",
        ),
    };
    Ok(vec![
        executable,
        tenant,
        config_check,
        environment,
        scanner_credentials,
        management_credentials,
        scanner_api,
        management_oauth,
        gateway_api,
    ])
}

fn bullet_kind(status: Status) -> pretty::Bullet {
    match status {
        Status::Pass => pretty::Bullet::Pass,
        Status::Warn => pretty::Bullet::Warn,
        Status::Fail => pretty::Bullet::Fail,
        Status::Skip => pretty::Bullet::Skip,
    }
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Pass => "pass",
        Status::Warn => "warn",
        Status::Fail => "fail",
        Status::Skip => "skip",
    }
}

/// The `Name │ Status │ Detail │ Hint` table (§9, R§3.9).
fn report_table(checks: &[Check]) -> String {
    let rows: Vec<Vec<String>> = checks
        .iter()
        .map(|check| {
            vec![
                check.name.to_owned(),
                status_name(check.status).to_owned(),
                terminal_safe(&check.detail).into_owned(),
                check
                    .hint
                    .as_deref()
                    .map_or_else(String::new, |hint| terminal_safe(hint).into_owned()),
            ]
        })
        .collect();
    table::table(&["Name", "Status", "Detail", "Hint"], &rows)
}

/// Write the report; returns whether any check failed.
pub fn render(
    output: &mut crate::output::Output,
    checks: &[Check],
    format: Format,
) -> Result<bool, CliError> {
    let (failed, text) = summary(checks);
    match format {
        Format::Json => {
            output.json(checks)?;
            return Ok(failed);
        }
        // No header, no summary, nothing on stderr; `--quiet` changes nothing.
        Format::Table => {
            output.data_line(&report_table(checks));
            return Ok(failed);
        }
        Format::Pretty => {}
    }
    pretty::banner(output, "Doctor", Some("Prisma AIRS CLI preflight checks"));
    for check in checks {
        let line = format!("{} — {}", check.name, terminal_safe(&check.detail));
        pretty::bullet(output, &line, bullet_kind(check.status));
        if let Some(hint) = &check.hint {
            pretty::dim(output, &format!("    {}", terminal_safe(hint)));
        }
    }
    output.data("\n");
    if failed {
        // Keep the visual order: the report first, then the failure summary on stderr.
        let _ = output.commit();
        let summary = output.paint_err(Style::Red, &format!("✗ {text}"));
        output.write_stderr(&format!("  {summary}\n"));
    } else {
        pretty::success(output, &text);
    }
    output.data("\n");
    Ok(failed)
}

pub async fn run(invocation: &mut Invocation, format: Format) -> Result<(), CliError> {
    let mut checks = checks(invocation).await?;
    // Config and executable paths can coincide with an unused named secret.
    // Project only after configuration and OAuth discovery, before any renderer.
    for check in &mut checks {
        check.detail = terminal_safe(invocation.secrets.scrub(&check.detail)).into_owned();
        if let Some(hint) = &mut check.hint {
            *hint = terminal_safe(invocation.secrets.scrub(hint)).into_owned();
        }
    }
    if render(&mut invocation.output, &checks, format)? {
        let _ = invocation.output.commit();
        return Err(CliError::Reported(crate::error::EXIT_OPERATIONAL));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::testing::capture;
    use scm::RemoteOutcome;

    fn check(status: Status) -> Check {
        Check::new("x", status, "d")
    }

    #[test]
    fn plurals_and_summaries() {
        assert_eq!(plural(1, "check"), "1 check");
        assert_eq!(plural(0, "key"), "0 keys");
        assert_eq!(plural(2, "custom topic"), "2 custom topics");
        assert_eq!(
            summary(&[check(Status::Pass)]),
            (false, "All checks passed".into())
        );
        assert_eq!(
            summary(&[
                check(Status::Warn),
                check(Status::Skip),
                check(Status::Skip)
            ]),
            (false, "All checks passed (1 warning, 2 skipped)".into())
        );
        assert_eq!(
            summary(&[check(Status::Warn), check(Status::Warn)]),
            (false, "All checks passed (2 warnings)".into())
        );
        assert_eq!(
            summary(&vec![check(Status::Skip); 4]),
            (false, "All checks passed (4 skipped)".into())
        );
        assert_eq!(
            summary(&[check(Status::Fail)]),
            (true, "1 check failed".into())
        );
        assert_eq!(
            summary(&[
                check(Status::Fail),
                check(Status::Fail),
                check(Status::Warn)
            ]),
            (true, "2 checks failed".into())
        );
    }

    /// The Executable check (§21.15) over a private directory tree: `PATH` order, the
    /// executable bit, symlinks, the copies in the executable's own directory, the npm
    /// launcher, relative entries and an unknown executable.
    #[test]
    fn executable_check() {
        use std::os::unix::fs::symlink;
        let dir = crate::store::testdir::TestDir::new("doctor-executable");
        let root = dir.path();
        let exe = dir.write("install/bin/airs", b"x", 0o755);
        dir.write("install/bin/airs-cli", b"copy", 0o755);
        dir.write(&format!("install/bin/{}", PROGRAM_NAMES[2]), b"copy", 0o755);
        let other = dir.write("other/airs", b"y", 0o755);
        dir.write("noexec/airs", b"z", 0o644);
        dir.write("npm/lib/node_modules/pkg/bin/airs.js", b"js", 0o755);
        std::fs::create_dir_all(root.join("links")).unwrap();
        symlink(&exe, root.join("links/airs")).unwrap();
        symlink(
            root.join("npm/lib/node_modules/pkg/bin/airs.js"),
            root.join("links/airs-cli"),
        )
        .unwrap();
        let program = RunningProgram {
            exe: Some(std::fs::canonicalize(&exe).unwrap()),
            launcher: None,
        };
        let path = |dirs: &[&str]| {
            std::env::join_paths(dirs.iter().map(|d| {
                if d.starts_with('/') || d.is_empty() || *d == "." {
                    PathBuf::from(d)
                } else {
                    root.join(d)
                }
            }))
            .unwrap()
        };
        let pass = |check: Check| {
            assert_eq!(
                (check.name, check.status, check.detail.as_str(), check.hint),
                (EXECUTABLE, Status::Pass, EXECUTABLE_PASS, None)
            );
        };
        // Nothing on PATH, no PATH, its own directory, and a non-executable file first.
        pass(check_executable(Some(&path(&["/nonexistent"])), &program));
        pass(check_executable(None, &program));
        pass(check_executable(Some(&path(&["install/bin"])), &program));
        pass(check_executable(
            Some(&path(&["noexec", "install/bin"])),
            &program,
        ));
        // Another airs first: a warning naming it; the copies after it change nothing.
        let check = check_executable(Some(&path(&["other", "install/bin"])), &program);
        assert_eq!(check.status, Status::Warn);
        assert_eq!(
            check.detail,
            format!(
                "airs on PATH is another program: {}",
                std::fs::canonicalize(&other).unwrap().display()
            )
        );
        assert_eq!(check.hint.as_deref(), Some(EXECUTABLE_HINT));
        // A link to this executable is this program; a link to something else names both
        // paths, and a relative entry is ignored.
        let links = root.join("links");
        let check = check_executable(Some(&path(&[".", "links", "install/bin"])), &program);
        assert_eq!(check.status, Status::Warn);
        assert_eq!(
            check.detail,
            format!(
                "airs-cli on PATH is another program: {} ({})",
                links.join("airs-cli").display(),
                std::fs::canonicalize(root.join("npm/lib/node_modules/pkg/bin/airs.js"))
                    .unwrap()
                    .display()
            )
        );
        // Started by that npm launcher, the link is this program.
        let npm = RunningProgram {
            launcher: Some(
                std::fs::canonicalize(root.join("npm/lib/node_modules/pkg/bin/airs.js")).unwrap(),
            ),
            ..program
        };
        pass(check_executable(
            Some(&path(&["links", "install/bin"])),
            &npm,
        ));
        // Every shadowed name, in order, joined.
        let check = check_executable(
            Some(&path(&["other", "links"])),
            &RunningProgram {
                exe: Some(std::fs::canonicalize(root.join("noexec/airs")).unwrap()),
                launcher: None,
            },
        );
        assert_eq!(check.status, Status::Warn);
        assert_eq!(
            check.detail.matches("on PATH is another program").count(),
            2
        );
        assert!(check.detail.starts_with("airs on PATH"), "{}", check.detail);
        assert!(
            check.detail.contains("; airs-cli on PATH"),
            "{}",
            check.detail
        );
        // An unknown executable skips, whatever PATH holds.
        let unknown = check_executable(
            Some(&path(&["other"])),
            &RunningProgram {
                exe: None,
                launcher: None,
            },
        );
        assert_eq!(
            (unknown.status, unknown.detail.as_str()),
            (Status::Skip, "the running executable cannot be determined")
        );
    }

    #[test]
    fn environment_check() {
        assert_eq!(
            check_environment(&[], &[]).detail,
            "no configuration variables set"
        );
        let warn = check_environment(
            &[
                "PANW_MGMT_CLIENT_SECRET".into(),
                "PRISMA_AIRS_CONFIG_PATH".into(),
            ],
            &["NO_PROXY"],
        );
        assert_eq!(warn.status, Status::Warn);
        assert_eq!(
            warn.detail,
            "ignored 2 variables: PANW_MGMT_CLIENT_SECRET, PRISMA_AIRS_CONFIG_PATH; proxy variables in effect: NO_PROXY"
        );
        assert_eq!(
            check_environment(&["PANW_X".into()], &[]).detail,
            "ignored 1 variable: PANW_X"
        );
    }

    fn facts(kind: ErrorKind, status: Option<u16>) -> ErrorFacts {
        ErrorFacts {
            kind,
            status,
            error_type: None,
            problem: None,
            service_code: None,
            request_id: None,
            remote_outcome: RemoteOutcome::Rejected,
        }
    }

    #[test]
    fn scanner_probe_classification() {
        let c = classify_scanner(&facts(ErrorKind::Http, Some(401)), "m").unwrap();
        assert_eq!(
            (c.status, c.detail.as_str()),
            (Status::Fail, "API key rejected (HTTP 401): m")
        );
        let c = classify_scanner(&facts(ErrorKind::Limit, Some(403)), "m").unwrap();
        assert_eq!(c.status, Status::Fail);
        let c = classify_scanner(&facts(ErrorKind::Http, Some(404)), "m").unwrap();
        assert_eq!(
            (c.status, c.detail.as_str()),
            (
                Status::Pass,
                "endpoint reachable, API key accepted (HTTP 404 on probe query)"
            )
        );
        let c = classify_scanner(&facts(ErrorKind::Http, Some(429)), "m").unwrap();
        assert_eq!(
            (c.status, c.hint.as_deref()),
            (Status::Warn, Some("Retry later"))
        );
        let c = classify_scanner(&facts(ErrorKind::Http, Some(503)), "m").unwrap();
        assert_eq!(c.detail, "scanner API error (HTTP 503): m");
        let c = classify_scanner(&facts(ErrorKind::Deadline, None), "m").unwrap();
        assert_eq!(
            c.detail,
            "timed out after 5000ms — network unreachable or endpoint not responding"
        );
        let c = classify_scanner(&facts(ErrorKind::Transport, None), "m").unwrap();
        assert_eq!(c.detail, "network unreachable: m");
        let c = classify_scanner(&facts(ErrorKind::Decode, Some(200)), "m").unwrap();
        assert_eq!(
            c.hint.as_deref(),
            Some("Fix airsApiEndpoint in the tenant file")
        );
        assert!(classify_scanner(&facts(ErrorKind::Cancelled, None), "m").is_err());
    }

    #[test]
    fn management_probe_classification() {
        let c =
            classify_management(&facts(ErrorKind::Authentication, Some(401)), "m", true).unwrap();
        assert_eq!(c.detail, "management API error (HTTP 401): m");
        assert_eq!(c.hint.as_deref(), Some(VERIFY_MANAGEMENT));
        let c = classify_management(&facts(ErrorKind::Authentication, None), "m", true).unwrap();
        assert_eq!(c.detail, "authentication failed: m");
        let c = classify_management(&facts(ErrorKind::Decode, None), "m", true).unwrap();
        assert_eq!(c.detail, "authentication failed: m");
        let c = classify_management(&facts(ErrorKind::Transport, None), "m", true).unwrap();
        assert_eq!(c.detail, "network unreachable: m");
        let c = classify_management(&facts(ErrorKind::Http, Some(403)), "m", false).unwrap();
        assert_eq!(c.detail, "management API error (HTTP 403): m");
        let c = classify_management(&facts(ErrorKind::Deadline, None), "m", false).unwrap();
        assert_eq!(
            c.hint.as_deref(),
            Some("Check network connectivity and the mgmtTokenEndpoint / mgmtEndpoint settings")
        );
        let c = classify_management(&facts(ErrorKind::Pagination, None), "m", false).unwrap();
        assert_eq!(c.detail, "management API error: m");
        // A malformed or oversized 2xx body: an endpoint problem, no "HTTP 200".
        for kind in [ErrorKind::Decode, ErrorKind::Limit] {
            let c = classify_management(&facts(kind, Some(200)), "m", false).unwrap();
            assert_eq!(c.detail, "management API error: m");
            assert_eq!(
                c.hint.as_deref(),
                Some("Fix mgmtEndpoint in the tenant file")
            );
        }
        let c = classify_management(&facts(ErrorKind::Limit, Some(503)), "m", false).unwrap();
        assert_eq!(c.detail, "management API error (HTTP 503): m");
    }

    #[test]
    fn a_deadline_after_an_http_error_is_that_http_error() {
        let deadline = |status| last_answer(facts(ErrorKind::Deadline, status));
        let c = classify_scanner(&deadline(Some(429)), "m").unwrap();
        assert_eq!(
            (c.status, c.detail.as_str()),
            (
                Status::Warn,
                "endpoint reachable, but rate limited (HTTP 429): m"
            )
        );
        let c = classify_scanner(&deadline(Some(503)), "m").unwrap();
        assert_eq!(c.detail, "scanner API error (HTTP 503): m");
        let c = classify_scanner(&deadline(Some(200)), "m").unwrap();
        assert!(c.detail.starts_with("timed out after 5000ms"));
        let real = probe_facts(
            &scm::Error::new(ErrorKind::Deadline),
            &LastStatus::default(),
        );
        assert_eq!(real.kind, ErrorKind::Deadline, "no status: still a timeout");
    }

    fn store_view(dir: &crate::store::testdir::TestDir) -> StoreView {
        let env = crate::tenant::Environment {
            config_dir: Some(dir.path().as_os_str().to_owned()),
            ..crate::tenant::Environment::default()
        };
        StoreView::open(&env).unwrap()
    }

    fn dev() -> Selected {
        Selected {
            name: "dev".into(),
            tsg_id: "100".into(),
            by_flag: false,
        }
    }

    #[test]
    fn config_file_checks() {
        let dir = crate::store::testdir::TestDir::new("doctor-config");
        dir.write(
            "config.toml",
            b"version = 1\n\n[tenants.dev]\ntsg_id = \"100\"\n",
            0o600,
        );
        let view = store_view(&dir);
        let p = dir.path().join("tenants/dev.toml").display().to_string();
        let (c, _) = check_config(&view, &dev());
        assert_eq!(c.detail, format!("not found at {p}"));
        assert_eq!(
            c.hint.as_deref(),
            Some(
                "Restore the file, or delete the tenant with 'airs tenant delete dev' and create it again"
            )
        );
        let write = |bytes: &[u8], mode| dir.write("tenants/dev.toml", bytes, mode);
        write(b"mgmtTsgId = \"100\"\n", 0o600);
        let (c, cfg) = check_config(&view, &dev());
        assert_eq!(
            c.detail,
            format!("valid tenant config at {p} (TSG 100 matches registration)")
        );
        assert!(cfg.is_some());
        write(b"mgmtTsgId = \"100\"\n", 0o644);
        let (c, cfg) = check_config(&view, &dev());
        assert_eq!(
            c.detail,
            format!(
                "{p} is not private: it must be a regular file (not a symlink) owned by the current user with no group or other permissions"
            )
        );
        assert!(cfg.is_none());
        write(b"mgmtTsgId = ", 0o600);
        assert_eq!(
            check_config(&view, &dev()).0.detail,
            format!("{p} is not valid TOML")
        );
        write(b"mgmtTsgId = \"100\"\nzeta = 1\ncustom = 2\n", 0o600);
        let (c, _) = check_config(&view, &dev());
        assert_eq!(c.detail, format!("{p} has unknown keys: custom, zeta"));
        assert_eq!(
            c.hint.as_deref(),
            Some(
                "Remove them from the file; 'airs tenant set dev <key>' accepts only the supported keys"
            )
        );
        write(
            b"scanConcurrency = 99\nmgmtClientSecret = \"SUPER-SECRET\"\nmgmtTsgId = \"100\"\ndefaultOutput = \"csv\"\n",
            0o600,
        );
        let (c, cfg) = check_config(&view, &dev());
        assert_eq!(
            c.detail,
            format!("{p} has invalid values for: scanConcurrency, defaultOutput")
        );
        assert!(
            c.hint
                .as_deref()
                .unwrap()
                .ends_with("(markdown, csv and yaml were dropped)")
        );
        let around_path = format!("{c:?}").replace(&p, "<path>");
        assert!(!around_path.contains("SUPER-SECRET") && !around_path.contains("99"));
        assert!(cfg.is_none());
        write(b"mgmtTsgId = \"200\"\n", 0o600);
        let (c, _) = check_config(&view, &dev());
        assert_eq!(
            c.detail,
            format!("{p} carries a different mgmtTsgId than the registration (TSG 100)")
        );
        assert_eq!(
            c.hint.as_deref(),
            Some(
                "Choose the TSG with 'airs tenant set dev mgmtTsgId <tsg>'; it updates both files"
            )
        );
        // The random temporary path can itself contain "200".
        assert!(!c.detail.replace(&p, "<path>").contains("200"));
        let mut big = b"mgmtTsgId = \"100\"\n".to_vec();
        big.resize(crate::store::MAX_FILE_BYTES as usize + 1, b'\n');
        write(&big, 0o600);
        assert!(
            check_config(&view, &dev())
                .0
                .detail
                .ends_with("cannot be read (not a regular file, larger than 1 MiB, or unreadable)")
        );
    }

    #[test]
    fn credential_checks() {
        let cfg = TenantConfig::parse(b"airsApiKey = \"k\"\nmgmtTsgId = \"100\"\n").unwrap();
        let scanner = check_scanner_credentials(Some(&cfg), "dev");
        assert_eq!(
            (scanner.status, scanner.detail.as_str()),
            (Status::Pass, "airsApiKey (file)")
        );
        let mgmt = check_management_credentials(Some(&cfg), "dev", true);
        assert_eq!(mgmt.status, Status::Warn);
        assert_eq!(
            mgmt.detail,
            "missing: mgmtClientId, mgmtClientSecret — management, red team, model security, gateway and supplychain commands are unavailable"
        );
        assert_eq!(
            mgmt.hint.as_deref(),
            Some(
                "Run 'airs tenant set dev <key>' for mgmtClientId, mgmtClientSecret (secrets prompt hidden, or use --stdin)"
            )
        );
        assert_eq!(
            check_management_credentials(Some(&cfg), "dev", false).status,
            Status::Fail
        );
        let empty = TenantConfig::parse(b"mgmtTsgId = \"100\"\n").unwrap();
        let scanner = check_scanner_credentials(Some(&empty), "dev");
        assert_eq!(scanner.status, Status::Skip);
        assert!(check_scanner_credentials(None, "dev").is_not_evaluated());
        assert!(check_management_credentials(None, "dev", false).is_not_evaluated());
    }

    #[test]
    fn tenant_check_outcomes() {
        let dir = crate::store::testdir::TestDir::new("doctor-tenant");
        let view = store_view(&dir);
        let index = dir.path().join("config.toml");
        let c = check_tenant(StoreState::Read(&view), Some(Ok(None)));
        assert_eq!(
            c.detail,
            format!("no tenants registered in {}", index.display())
        );
        let error = CliError::operational("broken");
        let c = check_tenant(StoreState::Failed(&index, &error), None);
        assert_eq!(c.hint, Some(format!("Fix or restore {}", index.display())));
        let xdg = CliError::operational("XDG_CONFIG_HOME must be an absolute path");
        let c = check_tenant(StoreState::Unresolved(&xdg), None);
        assert_eq!(c.detail, "XDG_CONFIG_HOME must be an absolute path");
        assert_eq!(
            c.hint.as_deref(),
            Some("Set AIRS_CLI_CONFIG_DIR or XDG_CONFIG_HOME to an absolute path")
        );
        let selected = Selected {
            by_flag: true,
            ..dev()
        };
        let c = check_tenant(StoreState::Read(&view), Some(Ok(Some(&selected))));
        assert_eq!(
            c.detail,
            format!("dev (TSG 100) selected by --tenant in {}", index.display())
        );
    }

    #[test]
    fn pretty_report_streams() {
        let checks = vec![
            Check::new(TENANT, Status::Pass, "ok").hint("h"),
            Check::new(CONFIG_FILE, Status::Skip, NOT_EVALUATED),
        ];
        let (mut output, stdout, stderr) = capture(false);
        assert!(!render(&mut output, &checks, Format::Pretty).unwrap());
        output.commit().unwrap();
        assert_eq!(
            stdout.text(),
            "\n  Doctor\n  Prisma AIRS CLI preflight checks\n\n  ✓ Tenant — ok\n      h\n  ○ Config file — not evaluated — fix the failed checks above first\n\n  ✓ All checks passed (1 skipped)\n\n"
        );
        assert_eq!(stderr.text(), "");

        let failing = vec![Check::new(TENANT, Status::Fail, "bad").hint("h")];
        let (mut output, stdout, stderr) = capture(true);
        assert!(render(&mut output, &failing, Format::Pretty).unwrap());
        output.commit().unwrap();
        assert_eq!(stdout.text(), "  ✗ Tenant — bad\n\n\n");
        assert_eq!(stderr.text(), "  ✗ 1 check failed\n");

        let (mut output, stdout, _) = capture(false);
        render(&mut output, &failing, Format::Json).unwrap();
        output.commit().unwrap();
        assert_eq!(
            stdout.text(),
            "[\n  {\n    \"name\": \"Tenant\",\n    \"status\": \"fail\",\n    \"detail\": \"bad\",\n    \"hint\": \"h\"\n  }\n]\n"
        );
    }
}
