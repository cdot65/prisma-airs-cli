//! The optional runtime MCP client (RUNTIME_MCP_CONTRACT §7; DESIGN §22.8, D350–D354).
//!
//! - `airs mcp connections set|list|remove`: the tenant file's strict `mcp` namespace,
//!   written through the tenant store's transaction; the first write saves a private
//!   rollback snapshot (older binaries refuse the table).
//! - `airs mcp login|logout|status`: PKCE sign-in (the only interactive command on an ordinary connection; admin commands start their own fresh sign-in), the
//!   private session cache, and a local status view.
//! - `airs runtime history|jobs|schedules|sync --mcp CONNECTION`: typed, bounded tool
//!   calls. Without `--mcp` these are usage errors naming the optional service; there
//!   is no fallback to live collection or local state.
//!
//! The CLI never opens SQLite and makes no MCP call without an explicit selection.

use super::{Invocation, mcp_ops, tenant_write};
use crate::{
    cli::{
        McpCommand, McpConnectionsCommand, McpGroup, McpHistoryCommand, McpHistoryGroup,
        McpJobsCommand, McpJobsGroup, McpLifecycleArgs, McpReceiptsCommand, McpReceiptsGroup,
        McpSchedulesCommand, McpSchedulesGroup, McpSyncArgs, RuntimeReportArgs,
    },
    error::{CliError, InterruptContext},
    output::Format,
    store,
};
use airs_mcp_client::{Binding, Called, FileStore, McpConnection, Trust, receipts};
use airs_runtime_core::tenant::mcp::{self as conn, Cache, Role};
use serde_json::{Map, Value, json};

/// The usage error for a service command without `--mcp` (contract §7.2).
pub const NEEDS_MCP: &str = "This command uses the optional airs-mcp service: select a connection with --mcp CONNECTION (see 'airs mcp connections')";

pub struct McpPlan {
    action: Action,
    format: Format,
}

enum Action {
    List,
    Set(Box<McpConnectionInput>),
    Remove(String),
    Login {
        connection: String,
        open: bool,
        port: u16,
    },
    Logout(String),
    Status(Option<String>),
    /// One read tool call: (connection, tool, params).
    Read(String, &'static str, Value),
    /// One keyed mutation that returns its result (schedules, cancellation).
    Keyed(String, &'static str, Value),
    Sync {
        connection: String,
        params: Value,
        wait: bool,
        timeout: u64,
    },
    Report(Box<mcp_ops::ReportJob>),
    Download {
        connection: String,
        version: String,
        files: Vec<String>,
        dir: std::path::PathBuf,
    },
    Lifecycle(Box<mcp_ops::LifecycleJob>),
    Confirm(Box<mcp_ops::ConfirmJob>),
    Export {
        connection: String,
        file: std::path::PathBuf,
        sign_in: mcp_ops::SignIn,
    },
    Import(Box<mcp_ops::ImportJob>),
    Upload {
        connection: String,
        path: std::path::PathBuf,
    },
}

/// How one call authenticates: the connection's private session, or a token held in
/// memory for this command only (a fresh administrative sign-in, or a refresh).
pub enum Auth {
    Session,
    Token(String),
}

struct McpConnectionInput {
    name: String,
    connection: McpConnection,
    replace: bool,
}

pub(super) fn need(select: &crate::cli::McpSelect) -> Result<String, CliError> {
    let name = select
        .mcp
        .clone()
        .ok_or_else(|| CliError::usage(NEEDS_MCP))?;
    if !conn::valid_name(&name) {
        return Err(CliError::usage(
            "--mcp must name a connection (1-63 of [a-z0-9-])",
        ));
    }
    Ok(name)
}

fn utc(text: &str, flag: &str) -> Result<i64, CliError> {
    airs_runtime_core::dto::utc_ms(text)
        .ok_or_else(|| CliError::usage(format!("{flag} must be an RFC 3339 UTC instant (…Z)")))
}

/// The longest report range (contract §7.2).
const REPORT_RANGE_MS: i64 = 90 * 86_400_000;

/// An explicit range: both instants, in order, checked before any tenant access (M7
/// review 1 finding 7).
fn range(since: &Option<String>, until: &Option<String>) -> Result<Option<Value>, CliError> {
    match (since, until) {
        (None, None) => Ok(None),
        (Some(s), Some(u)) => {
            let (from, to) = (utc(s, "--since")?, utc(u, "--until")?);
            if from >= to {
                return Err(CliError::usage("--since must be before --until"));
            }
            Ok(Some(json!({"since": s, "until": u})))
        }
        _ => Err(CliError::usage("--since and --until go together")),
    }
}

/// `airs mcp …`.
pub fn plan(group: &McpGroup, format: Format) -> Result<super::Plan, CliError> {
    let Some(command) = &group.command else {
        return Ok(super::Plan::GroupHelp(&["mcp"]));
    };
    let action = match command {
        McpCommand::Connections(g) => match &g.command {
            None => return Ok(super::Plan::GroupHelp(&["mcp", "connections"])),
            Some(McpConnectionsCommand::List) => Action::List,
            Some(McpConnectionsCommand::Remove(a)) => {
                if !conn::valid_name(&a.connection) {
                    return Err(CliError::usage(
                        "Connection names use tenant-name syntax: 1-63 of [a-z0-9-]",
                    ));
                }
                Action::Remove(a.connection.clone())
            }
            Some(McpConnectionsCommand::Set(a)) => {
                let role = match a.role.as_str() {
                    "ordinary" => Role::Ordinary,
                    "admin" => Role::Admin,
                    _ => return Err(CliError::usage("--role must be ordinary or admin")),
                };
                let mut origins = Vec::new();
                for o in &a.approved_origin {
                    let canonical = conn::url_origin(o.trim_end_matches('/'))
                        .filter(|c| *c == o.trim_end_matches('/').to_ascii_lowercase())
                        .ok_or_else(|| CliError::usage("--approved-origin must be an origin (scheme://host[:port]), HTTPS or loopback HTTP"))?;
                    if !origins.contains(&canonical) {
                        origins.push(canonical);
                    }
                }
                let connection = McpConnection {
                    url: a.url.trim_end_matches('/').to_owned(),
                    client_id: a.client_id.clone(),
                    service_tenant: a.service_tenant.clone(),
                    expected_tsg: a.expected_tsg.clone(),
                    discovery_origins: origins,
                    role,
                    cache: if role == Role::Admin {
                        Cache::None
                    } else {
                        Cache::Private
                    },
                    revision: uuid::Uuid::new_v4().to_string(),
                };
                if !conn::valid_name(&a.connection) {
                    return Err(CliError::usage(
                        "Connection names use tenant-name syntax: 1-63 of [a-z0-9-]",
                    ));
                }
                connection.validate().map_err(|reason| {
                    CliError::usage(format!("Invalid MCP connection: {reason}"))
                })?;
                Action::Set(Box::new(McpConnectionInput {
                    name: a.connection.clone(),
                    connection,
                    replace: a.replace,
                }))
            }
        },
        McpCommand::Login(a) => Action::Login {
            connection: a.connection.clone(),
            open: a.open,
            port: a.port,
        },
        McpCommand::Logout(a) => Action::Logout(a.connection.clone()),
        McpCommand::Status(a) => Action::Status(a.connection.clone()),
        McpCommand::Confirm(a) => {
            if !conn::valid_name(&a.connection) {
                return Err(CliError::usage(
                    "--connection must name a connection (1-63 of [a-z0-9-])",
                ));
            }
            if !airs_mcp_client::previews::valid_preview_id(&a.preview_id) {
                return Err(CliError::usage("PREVIEW_ID must be a preview ID (p1_…)"));
            }
            if a.digest.as_deref().is_some_and(|d| !hex(d, 64)) {
                return Err(CliError::usage("--digest must be 64 lowercase hex digits"));
            }
            if a.proof_nonce.as_deref().is_some_and(|n| !hex(n, 32)) {
                return Err(CliError::usage(
                    "--proof-nonce must be 32 lowercase hex digits",
                ));
            }
            if a.digest.is_some() != a.proof_nonce.is_some() {
                return Err(CliError::usage("--digest and --proof-nonce go together"));
            }
            Action::Confirm(Box::new(mcp_ops::ConfirmJob {
                preview_id: a.preview_id.clone(),
                connection: a.connection.clone(),
                digest: a.digest.clone(),
                nonce: a.proof_nonce.clone(),
                sign_in: mcp_ops::SignIn {
                    open: a.open,
                    port: a.port,
                },
            }))
        }
    };
    Ok(super::Plan::Mcp(Box::new(McpPlan { action, format })))
}

/// `airs runtime history …`.
pub fn plan_history(group: &McpHistoryGroup, format: Format) -> Result<super::Plan, CliError> {
    let Some(command) = &group.command else {
        return Ok(super::Plan::GroupHelp(&["runtime", "history"]));
    };
    let action = match command {
        McpHistoryCommand::Status(s) => Action::Read(need(s)?, "history_status", json!({})),
        McpHistoryCommand::Coverage(a) => {
            let name = need(&a.select)?;
            let r = match range(&a.since, &a.until)? {
                Some(r) => r,
                None => {
                    let now = airs_runtime_core::time::now_ms();
                    json!({"since": airs_runtime_core::time::iso(now - 86_400_000), "until": airs_runtime_core::time::iso(now)})
                }
            };
            let mut p = json!({"range": r});
            if !a.app_id.is_empty() {
                p["app_ids"] = json!(a.app_id);
            }
            Action::Read(name, "history_coverage", p)
        }
        McpHistoryCommand::Query(a) => {
            let name = need(&a.select)?;
            let r = range(&Some(a.since.clone()), &Some(a.until.clone()))?;
            let mut p = json!({"range": r});
            if !a.app_id.is_empty() {
                p["app_ids"] = json!(a.app_id);
            }
            if !a.group_by.is_empty() {
                const KEYS: [&str; 6] = [
                    "day",
                    "application",
                    "detector",
                    "verdict",
                    "action",
                    "profile",
                ];
                if a.group_by.len() > 3 || a.group_by.iter().any(|g| !KEYS.contains(&g.as_str())) {
                    return Err(CliError::usage(
                        "--group-by takes at most three of day, application, detector, verdict, action, profile",
                    ));
                }
                p["group_by"] = json!(a.group_by);
            }
            if let Some(max) = a.max {
                if !(1..=100).contains(&max) {
                    return Err(CliError::usage("--max must be 1-100"));
                }
                p["page"] = json!({"limit": max});
            }
            Action::Read(name, "history_query", p)
        }
        McpHistoryCommand::Reports(a) => {
            let name = need(&a.select)?;
            let mut p = json!({});
            if let Some(r) = range(&a.since, &a.until)? {
                p["range"] = r;
            }
            if let Some(kind) = &a.kind {
                if !["daily", "revision", "manual", "catch_up"].contains(&kind.as_str()) {
                    return Err(CliError::usage(
                        "--kind must be daily, revision, manual or catch_up",
                    ));
                }
                p["kind"] = json!(kind);
            }
            if let Some(max) = a.max {
                if !(1..=50).contains(&max) {
                    return Err(CliError::usage("--max must be 1-50"));
                }
                p["page"] = json!({"limit": max});
            }
            Action::Read(name, "reports_list", p)
        }
        McpHistoryCommand::Download(a) => {
            let name = need(&a.select)?;
            if !id_of(&a.version, "v1_") {
                return Err(CliError::usage(
                    "--version-id must be a report version ID (v1_…)",
                ));
            }
            const NAMES: [&str; 3] = ["report.html", "evidence.json", "review.md"];
            let mut files: Vec<String> = Vec::new();
            for f in &a.file {
                if !NAMES.contains(&f.as_str()) {
                    return Err(CliError::usage(
                        "--file must be report.html, evidence.json or review.md",
                    ));
                }
                if !files.contains(f) {
                    files.push(f.clone());
                }
            }
            if files.is_empty() {
                files = NAMES.map(String::from).to_vec();
            }
            Action::Download {
                connection: name,
                version: a.version.clone(),
                files,
                dir: a.output_dir.clone(),
            }
        }
        McpHistoryCommand::Import(a) => {
            let name = need(&a.select)?;
            if a.expect_tsg.is_empty()
                || a.expect_tsg.len() > 32
                || !a.expect_tsg.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(CliError::usage("--expect-tsg must be 1-32 digits"));
            }
            if !a
                .expect_environment
                .strip_prefix("env1:")
                .is_some_and(|h| hex(h, 64))
            {
                return Err(CliError::usage(
                    "--expect-environment must be an environment ID (env1:<64 hex>)",
                ));
            }
            Action::Import(Box::new(mcp_ops::ImportJob {
                connection: name,
                file: a.file.clone(),
                expect_tsg: a.expect_tsg.clone(),
                expect_environment: a.expect_environment.clone(),
                sign_in: sign_in(&a.sign_in),
            }))
        }
        McpHistoryCommand::Export(a) => Action::Export {
            connection: need(&a.select)?,
            file: a.file.clone(),
            sign_in: sign_in(&a.sign_in),
        },
        McpHistoryCommand::Prune(a) => {
            if !a.delete_artifacts.is_empty() {
                return Err(CliError::usage(
                    "--delete-artifacts is not offered online in this version",
                ));
            }
            let mut p = json!({"action": "prune"});
            if a.lifecycle.execute.is_none() {
                if !a.examples_only {
                    return Err(CliError::usage(
                        "This version prunes examples only: pass --examples-only",
                    ));
                }
                let before = a
                    .before
                    .as_deref()
                    .ok_or_else(|| CliError::usage("--before is required"))?;
                utc(before, "--before")?;
                p["before"] = json!(before);
                p["examples_only"] = json!(true);
                if !a.app_id.is_empty() {
                    p["app_ids"] = json!(a.app_id);
                }
            }
            lifecycle(&a.lifecycle, p)?
        }
        McpHistoryCommand::Compact(a) => lifecycle(a, json!({"action": "compact"}))?,
        McpHistoryCommand::Backup(a) => lifecycle(a, json!({"action": "backup"}))?,
        McpHistoryCommand::Recover(a) => lifecycle(a, json!({"action": "recover"}))?,
        McpHistoryCommand::Restore(a) => {
            let mut p = json!({"action": "restore"});
            if a.lifecycle.execute.is_none() {
                let backup = a
                    .backup
                    .as_deref()
                    .ok_or_else(|| CliError::usage("--backup is required"))?;
                if backup.is_empty() || backup.len() > 64 {
                    return Err(CliError::usage("--backup must be a backup ID"));
                }
                p["backup_id"] = json!(backup);
            }
            lifecycle(&a.lifecycle, p)?
        }
    };
    Ok(super::Plan::Mcp(Box::new(McpPlan { action, format })))
}

fn hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A server identifier `<prefix><8 hex>-…` (a lowercase UUIDv4 suffix, contract §4.2).
fn id_of(text: &str, prefix: &str) -> bool {
    text.strip_prefix(prefix)
        .and_then(|u| uuid::Uuid::parse_str(u).ok().filter(|p| p.to_string() == u))
        .is_some_and(|u| u.get_version_num() == 4)
}

fn sign_in(a: &crate::cli::McpSignInArgs) -> mcp_ops::SignIn {
    mcp_ops::SignIn {
        open: a.open,
        port: a.port,
    }
}

/// A lifecycle preview, or with `--execute` its execution (admin connection, or an
/// approval on the previewing connection, decided once the connection is read).
fn lifecycle(a: &McpLifecycleArgs, params: Value) -> Result<Action, CliError> {
    let name = need(&a.select)?;
    if let Some(id) = &a.execute
        && !airs_mcp_client::previews::valid_preview_id(id)
    {
        return Err(CliError::usage("--execute must name a preview ID (p1_…)"));
    }
    if let Some(approval) = &a.approval
        && !id_of(approval, "q1_")
    {
        return Err(CliError::usage("--approval must be an approval ID (q1_…)"));
    }
    if a.no_wait && a.execute.is_none() {
        return Err(CliError::usage("--no-wait applies to --execute"));
    }
    Ok(Action::Lifecycle(Box::new(mcp_ops::LifecycleJob {
        connection: name,
        params,
        execute: a.execute.clone(),
        approval: a.approval.clone(),
        wait: !a.no_wait,
        sign_in: sign_in(&a.sign_in),
    })))
}

/// `airs runtime receipts …`.
pub fn plan_receipts(group: &McpReceiptsGroup, format: Format) -> Result<super::Plan, CliError> {
    let Some(command) = &group.command else {
        return Ok(super::Plan::GroupHelp(&["runtime", "receipts"]));
    };
    let action = match command {
        McpReceiptsCommand::Upload(a) => Action::Upload {
            connection: need(&a.select)?,
            path: a.from_bulk_state.clone(),
        },
    };
    Ok(super::Plan::Mcp(Box::new(McpPlan { action, format })))
}

/// `airs runtime report --mcp …` (contract §7.2): `--from-evidence` and the live
/// collector's budget options conflict with `--mcp` before any tenant or network access.
pub fn plan_report(
    a: &RuntimeReportArgs,
    name: &str,
    format: Format,
) -> Result<super::Plan, CliError> {
    if a.from_evidence.is_some() || a.review_file.is_some() {
        return Err(CliError::usage(
            "--from-evidence renders a saved snapshot locally and cannot be used with --mcp",
        ));
    }
    if a.max_events.is_some()
        || a.max_reports.is_some()
        || a.max_pages.is_some()
        || a.budget.concurrency.is_some()
        || a.budget.max_requests.is_some()
    {
        return Err(CliError::usage(
            "--mcp renders from the service archive: the live collection budget options do not apply",
        ));
    }
    if !conn::valid_name(name) {
        return Err(CliError::usage(
            "--mcp must name a connection (1-63 of [a-z0-9-])",
        ));
    }
    let r = match range(&a.since, &a.until)? {
        Some(r) => {
            if a.interval.is_some() || a.unit.is_some() {
                return Err(CliError::usage(
                    "--since/--until and --interval/--unit are alternatives",
                ));
            }
            let span = |k: &str| r[k].as_str().and_then(airs_runtime_core::dto::utc_ms);
            if span("until")
                .zip(span("since"))
                .is_none_or(|(u, s)| u - s > REPORT_RANGE_MS)
            {
                return Err(CliError::usage("The report range is at most 90 days"));
            }
            r
        }
        None => {
            let n: i64 = match &a.interval {
                None => 24,
                Some(t) => t
                    .parse()
                    .ok()
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| CliError::usage("--interval must be a positive integer"))?,
            };
            let hour = match a.unit.as_deref() {
                None | Some("hour" | "hours") => 3_600_000,
                Some("day" | "days") => 86_400_000,
                Some(_) => return Err(CliError::usage("--unit must be hours or days")),
            };
            let Some(span) = n.checked_mul(hour).filter(|s| *s <= REPORT_RANGE_MS) else {
                return Err(CliError::usage("The report range is at most 90 days"));
            };
            let until = airs_runtime_core::time::now_ms() / 1000 * 1000;
            json!({"since": airs_runtime_core::time::iso(until - span), "until": airs_runtime_core::time::iso(until)})
        }
    };
    let mut params = json!({"range": r});
    if !a.app_ids.is_empty() {
        params["app_ids"] = json!(a.app_ids);
    }
    if let Some(title) = &a.title {
        if title.chars().count() > 200 {
            return Err(CliError::usage("--title is at most 200 characters"));
        }
        params["title"] = json!(title);
    }
    // `--timeout` is the wait for the service's report (the live collector's budget
    // does not apply).
    let timeout = match &a.budget.timeout {
        None => 600,
        Some(t) => t.parse::<u64>().unwrap_or(0),
    };
    if !(1..=3600).contains(&timeout) {
        return Err(CliError::usage("--timeout must be 1-3600"));
    }
    let dest = match (&a.bundle_dir, &a.output_file) {
        (Some(dir), _) => mcp_ops::ReportDest::Bundle(dir.clone()),
        (None, Some(file)) if file == "-" => {
            return Err(CliError::usage(
                "--output-file - is not offered with --mcp; name a new file or use --bundle-dir",
            ));
        }
        (None, Some(file)) => mcp_ops::ReportDest::File(std::path::PathBuf::from(file)),
        (None, None) => mcp_ops::ReportDest::None,
    };
    if a.no_wait && !matches!(dest, mcp_ops::ReportDest::None) {
        return Err(CliError::usage(
            "--no-wait cannot write files; download later with 'airs runtime history download'",
        ));
    }
    Ok(super::Plan::Mcp(Box::new(McpPlan {
        action: Action::Report(Box::new(mcp_ops::ReportJob {
            connection: name.to_owned(),
            params,
            wait: !a.no_wait,
            timeout,
            strict: a.strict,
            dest,
        })),
        format,
    })))
}

/// `airs runtime jobs …`.
pub fn plan_jobs(group: &McpJobsGroup, format: Format) -> Result<super::Plan, CliError> {
    let Some(command) = &group.command else {
        return Ok(super::Plan::GroupHelp(&["runtime", "jobs"]));
    };
    let action = match command {
        McpJobsCommand::List(a) => {
            let name = need(&a.select)?;
            let mut p = json!({});
            if !a.state.is_empty() {
                p["states"] = json!(a.state);
            }
            if let Some(max) = a.max {
                if !(1..=50).contains(&max) {
                    return Err(CliError::usage("--max must be 1-50"));
                }
                p["page"] = json!({"limit": max});
            }
            Action::Read(name, "jobs_list", p)
        }
        McpJobsCommand::Get(a) => {
            Action::Read(need(&a.select)?, "jobs_get", json!({"job_id": a.job_id}))
        }
        McpJobsCommand::Cancel(a) => {
            Action::Read(need(&a.select)?, "jobs_cancel", json!({"job_id": a.job_id}))
        }
    };
    Ok(super::Plan::Mcp(Box::new(McpPlan { action, format })))
}

/// `airs runtime schedules …`.
pub fn plan_schedules(group: &McpSchedulesGroup, format: Format) -> Result<super::Plan, CliError> {
    let Some(command) = &group.command else {
        return Ok(super::Plan::GroupHelp(&["runtime", "schedules"]));
    };
    let action = match command {
        McpSchedulesCommand::List(s) => Action::Read(need(s)?, "schedules_list", json!({})),
        McpSchedulesCommand::Set(a) => {
            let name = need(&a.select)?;
            if !["collection", "daily_report", "daily_enrichment"].contains(&a.kind.as_str()) {
                return Err(CliError::usage(
                    "--kind must be collection, daily_report or daily_enrichment",
                ));
            }
            let enabled = match a.enabled.as_str() {
                "true" => true,
                "false" => false,
                _ => return Err(CliError::usage("--enabled must be true or false")),
            };
            utc(&a.effective_at, "--effective-at")?;
            let mut p = json!({"kind": a.kind, "enabled": enabled, "effective_at": a.effective_at});
            if let Some(i) = a.interval_minutes {
                if !(5..=60).contains(&i) {
                    return Err(CliError::usage("--interval-minutes must be 5-60"));
                }
                p["interval_minutes"] = json!(i);
            }
            Action::Keyed(name, "schedules_set", p)
        }
        McpSchedulesCommand::Disable(a) => {
            let name = need(&a.select)?;
            utc(&a.effective_at, "--effective-at")?;
            Action::Keyed(
                name,
                "schedules_disable",
                json!({"schedule_id": a.schedule_id, "effective_at": a.effective_at}),
            )
        }
    };
    Ok(super::Plan::Mcp(Box::new(McpPlan { action, format })))
}

/// `airs runtime sync --mcp …`.
pub fn plan_sync(a: &McpSyncArgs, format: Format) -> Result<super::Plan, CliError> {
    let name = need(&a.select)?;
    let r = range(&a.since, &a.until)?;
    if a.backfill != r.is_some() {
        return Err(CliError::usage(
            "--backfill needs --since and --until, and a range needs --backfill",
        ));
    }
    if !(1..=3600).contains(&a.timeout) {
        return Err(CliError::usage("--timeout must be 1-3600"));
    }
    let mut p = json!({"mode": if a.backfill { "backfill" } else { "recent" }});
    if let Some(r) = r {
        p["range"] = r;
    }
    if !a.app_id.is_empty() {
        p["app_ids"] = json!(a.app_id);
    }
    Ok(super::Plan::Mcp(Box::new(McpPlan {
        action: Action::Sync {
            connection: name,
            params: p,
            wait: !a.no_wait,
            timeout: a.timeout,
        },
        format,
    })))
}

impl McpPlan {
    /// What an interruption reports when the command did not set its own context.
    pub fn interrupt_context(&self) -> InterruptContext {
        match self.action {
            Action::Set(_) | Action::Remove(_) => InterruptContext::NoConfigurationChanged,
            _ => InterruptContext::NoDataWritten,
        }
    }
}

pub(super) fn emit(inv: &mut Invocation, value: &Value, format: Format) -> Result<(), CliError> {
    let text = serde_json::to_string(value).unwrap_or_default();
    if inv.secrets.carried_by(&text) {
        return Err(CliError::operational(
            "The service result cannot be safely displayed",
        ));
    }
    match format {
        Format::Json => inv.output.json(value),
        Format::Pretty | Format::Table => {
            let pretty = serde_json::to_string_pretty(value).unwrap_or_default();
            inv.output.data_line(&pretty);
            Ok(())
        }
    }
}

/// The selected tenant's ordinary connection `name` (admin connections keep no
/// session: they serve only the administrative commands, each with a fresh sign-in).
pub(super) fn connection(inv: &mut Invocation, name: &str) -> Result<McpConnection, CliError> {
    let c = connection_any(inv, name)?;
    if c.role == Role::Admin {
        return Err(CliError::operational(
            "Admin connections keep no session: they serve only administrative commands (lifecycle --execute, history import/export, mcp confirm), each with a fresh sign-in",
        ));
    }
    Ok(c)
}

/// The selected tenant's connection `name`, of either role.
pub(super) fn connection_any(inv: &mut Invocation, name: &str) -> Result<McpConnection, CliError> {
    let context = inv.load_tenant()?;
    Ok(conn::select(context.config.mcp(), name)?.clone())
}

pub(super) fn trust(c: &McpConnection) -> Result<Trust, CliError> {
    let origins: Vec<&str> = c.discovery_origins.iter().map(String::as_str).collect();
    Trust::new(&c.url, &origins)
        .map_err(|e| CliError::operational(format!("MCP connection refused: {e}")))
}

pub(super) fn store_for(c: &McpConnection, t: &Trust) -> Result<FileStore, CliError> {
    FileStore::new(&Binding::of(t, c))
        .map_err(|e| CliError::operational(format!("MCP session cache: {e}")))
}

pub(super) fn refused(r: airs_mcp_client::Refusal) -> CliError {
    let mut message = format!(
        "The MCP service refused the request: {} ({})",
        screened(&r.reason),
        screened(&r.code.to_string())
    );
    if let Some(s) = r.retry_after_seconds {
        message.push_str(&format!("; retry after {s} seconds"));
    }
    CliError::operational(message)
}

async fn call(
    c: &McpConnection,
    tool: &str,
    params: Value,
    key: Option<(&str, &str)>,
) -> Result<Value, CliError> {
    match outcome(c, &Auth::Session, tool, params, key).await? {
        Called::Ok(v) => Ok(v),
        Called::Refused(r) => Err(refused(r)),
    }
}

/// One call, keeping a refusal distinct from a transport/authentication failure.
pub(super) async fn outcome(
    c: &McpConnection,
    auth: &Auth,
    tool: &str,
    params: Value,
    key: Option<(&str, &str)>,
) -> Result<Called, CliError> {
    let t = trust(c)?;
    let token = match auth {
        Auth::Token(token) => token.clone(),
        Auth::Session => {
            let store = store_for(c, &t)?;
            airs_mcp_client::access_token(&t, &store)
                .await
                .map_err(|e| CliError::operational(screened(&e)))?
        }
    };
    protect(&token);
    airs_mcp_client::call(
        &t.resource,
        token,
        tool,
        airs_mcp_client::envelope(&c.service_tenant, params, key),
    )
    .await
    .map_err(|e| CliError::operational(screened(&e)))
}

/// The service's public tenant and recovery epoch, checked against the connection.
pub(super) async fn epoch(c: &McpConnection, auth: &Auth) -> Result<String, CliError> {
    let info = match outcome(c, auth, "service_info", json!({}), None).await? {
        Called::Ok(v) => v,
        Called::Refused(r) => return Err(refused(r)),
    };
    if info["tenant"]
        .as_str()
        .is_some_and(|t| t != c.service_tenant)
    {
        return Err(CliError::operational(
            "The MCP service serves another tenant than the connection names",
        ));
    }
    info["epoch"]
        .as_str()
        .filter(|e| e.len() == 8)
        .map(str::to_owned)
        .ok_or_else(|| CliError::operational("The MCP service reported no recovery epoch"))
}

/// This invocation's secret set, shared with the token sources below so that every MCP
/// credential the command obtains (a cached or refreshed session token, a fresh
/// administrative token) is registered before any output or failure can carry it
/// (M7 review 1 finding 1). Clones of a [`SecretSet`] share one set.
static GUARD: std::sync::Mutex<Option<airs_runtime_core::secrets::SecretSet>> =
    std::sync::Mutex::new(None);

pub(super) fn bind_guard(inv: &Invocation) {
    // Every MCP invocation filters its final failure through the set, whether or not
    // its tenant names scanner credentials (M7 review 2 finding 1).
    inv.secrets.guard_local_output();
    *GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(inv.secrets.clone());
}

/// A service job ID: `j1_`, the 8-hex recovery epoch, `_` and a lowercase UUID. Nothing
/// else the service returns is stored in a receipt or interpolated into a diagnostic
/// (M7 review 2 finding 1).
pub(super) fn valid_job_id(id: &str) -> bool {
    let hex = |s: &str| {
        s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    let Some(rest) = id.strip_prefix("j1_") else {
        return false;
    };
    let Some((epoch, uuid)) = rest.split_once('_') else {
        return false;
    };
    epoch.len() == 8
        && hex(epoch)
        && uuid.len() == 36
        && uuid.char_indices().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == '-'
            } else {
                c.is_ascii_digit() || ('a'..='f').contains(&c)
            }
        })
}

/// Test support: bind `set` as the invocation's secret set.
#[cfg(test)]
pub(super) fn bind_secrets_for_tests(set: airs_runtime_core::secrets::SecretSet) {
    *GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(set);
}

/// Register an MCP credential with the invocation's secret set.
pub(super) fn protect(token: &str) {
    if let Some(set) = GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        set.add(token);
    }
}

/// Whether `text` carries a registered secret, exactly or with its case changed (the CLI
/// lowercases scan IDs, M7 review 3 finding 1).
pub(super) fn carries_secret(text: &str) -> bool {
    GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .is_some_and(|set| set.carried_by(text) || set.appears_in_ignoring_case(text))
}

/// The byte length of the longest registered secret form (the overlap a streaming
/// screen keeps between chunks, M7 review 7).
pub(super) fn longest_secret() -> usize {
    GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map_or(0, |set| {
            set.snapshot().iter().map(String::len).max().unwrap_or(0)
        })
}

/// Whether `path` explicitly does not exist (`NotFound`), as opposed to being
/// unreadable through a permission or other error.
fn absent(path: &std::path::Path) -> bool {
    matches!(std::fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
}

/// How many secret forms are registered (a transfer re-screens its file when this
/// grew while it ran, M7 review 8).
pub(super) fn registry_len() -> usize {
    GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map_or(0, |set| set.snapshot().len())
}

/// `text` with any registered secret withheld (server-supplied text the CLI prints).
pub(super) fn screened(text: &str) -> String {
    match GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        Some(set) => set.scrub(text).to_owned(),
        None => text.to_owned(),
    }
}

pub(super) fn new_key() -> String {
    format!("cli-{}", uuid::Uuid::new_v4().simple())
}

pub async fn run(inv: &mut Invocation, plan: McpPlan) -> Result<(), CliError> {
    bind_guard(inv);
    let format = plan.format;
    match plan.action {
        Action::List => {
            let context = inv.load_tenant()?;
            let list: Map<String, Value> = context
                .config
                .mcp()
                .map(|m| m.iter().map(|(k, v)| (k.clone(), v.public())).collect())
                .unwrap_or_default();
            emit(inv, &Value::Object(list), format)
        }
        Action::Set(input) => write(inv, Some(*input), None),
        Action::Remove(name) => write(inv, None, Some(name)),
        Action::Login {
            connection: name,
            open,
            port,
        } => {
            let c = connection(inv, &name)?;
            let t = trust(&c)?;
            let store = store_for(&c, &t)?;
            airs_mcp_client::login(&t, &store, &c.client_id, vec![], port, open)
                .await
                .map_err(|e| CliError::operational(format!("Sign-in failed: {e}")))?;
            inv.output
                .success("Signed in; the session is saved privately");
            Ok(())
        }
        Action::Logout(name) => {
            let c = connection(inv, &name)?;
            let t = trust(&c)?;
            let removed = store_for(&c, &t)?.remove().map_err(CliError::operational)?;
            inv.output.success(if removed {
                "Signed out; the session was removed"
            } else {
                "No session to remove"
            });
            Ok(())
        }
        Action::Status(only) => {
            let context = inv.load_tenant()?;
            let root = airs_mcp_client::state_root().map_err(CliError::operational)?;
            let mut out = Map::new();
            for (name, c) in context.config.mcp().into_iter().flatten() {
                if only.as_ref().is_some_and(|o| o != name) {
                    continue;
                }
                let session = trust(c)
                    .ok()
                    .and_then(|t| FileStore::at(&root, &Binding::of(&t, c)).ok())
                    .is_some_and(|s| s.present());
                // A partial inventory is never shown as complete: an unreadable receipt
                // directory or receipt fails the command, and nothing is removed (M7
                // review 7 finding 2).
                let incomplete = || {
                    CliError::operational(format!(
                        "The operation receipts of connection {name} cannot all be read; nothing was removed. Check {}",
                        inv.secrets
                            .scrub(&crate::store::shown(&root.join("receipts").join(name)))
                    ))
                };
                let (list, unreadable) =
                    match receipts::Receipts::open(&root, name).and_then(|r| r.inventory()) {
                        Ok(found) => found,
                        // Only a receipt store that is explicitly absent is empty; a permission
                        // or other error on it or an ancestor is an incomplete inventory (M7
                        // review 8 finding 2).
                        Err(_) if absent(&root.join("receipts").join(name)) => (Vec::new(), 0),
                        Err(_) => return Err(incomplete()),
                    };
                if unreadable > 0 {
                    return Err(incomplete());
                }
                let unresolved = list
                    .iter()
                    .map(|r| json!({"operation": r.shown_operation(), "idempotencyKey": r.shown_key(), "jobId": r.trusted_job_id(), "legacy": r.format != receipts::FORMAT, "current": r.connection_revision == c.revision}))
                    .collect::<Vec<_>>();
                out.insert(name.clone(), json!({"url": c.url, "role": c.role.name(), "session": session, "unresolved": unresolved}));
            }
            if let Some(o) = &only
                && !out.contains_key(o)
            {
                return Err(CliError::operational(
                    "No such MCP connection in the selected tenant",
                ));
            }
            emit(inv, &Value::Object(out), format)
        }
        Action::Read(name, tool, params) => {
            let c = connection(inv, &name)?;
            let v = call(&c, tool, params, None).await?;
            emit(inv, &v, format)
        }
        Action::Keyed(name, tool, params) => {
            // A durable receipt and the one authentication recovery, replaying the
            // original key, as for every keyed submission (M7 review 1 finding 3).
            let c = connection(inv, &name)?;
            let admitted = mcp_ops::submit(&c, &name, &Auth::Session, tool, params).await?;
            emit(inv, &admitted.accepted, format)?;
            admitted.deliver(inv)
        }
        Action::Sync {
            connection: name,
            params,
            wait,
            timeout,
        } => sync(inv, &name, params, wait, timeout, format).await,
        Action::Report(job) => mcp_ops::report(inv, *job, format).await,
        Action::Download {
            connection: name,
            version,
            files,
            dir,
        } => {
            let c = connection(inv, &name)?;
            let written = mcp_ops::download(
                inv,
                &c,
                &Auth::Session,
                &version,
                &files,
                &dir,
                "--output-dir",
            )
            .await?;
            let shown: Vec<String> = written
                .iter()
                .map(|p| inv.secrets.scrub(&store::shown(p)).to_owned())
                .collect();
            emit(inv, &json!({"version_id": version, "files": shown}), format)
        }
        Action::Lifecycle(job) => mcp_ops::lifecycle(inv, *job, format).await,
        Action::Confirm(job) => mcp_ops::confirm(inv, *job, format).await,
        Action::Export {
            connection: name,
            file,
            sign_in,
        } => mcp_ops::export(inv, &name, &file, sign_in, format).await,
        Action::Import(job) => mcp_ops::import(inv, *job, format).await,
        Action::Upload {
            connection: name,
            path,
        } => mcp_ops::upload(inv, &name, &path, format).await,
    }
}

/// `runtime sync --mcp`: a durable operation receipt before the first send, the
/// submission, then (unless `--no-wait`) a wait that an interruption ends with the job
/// ID printed and the job left running.
async fn sync(
    inv: &mut Invocation,
    name: &str,
    params: Value,
    wait: bool,
    timeout: u64,
    format: Format,
) -> Result<(), CliError> {
    let c = connection(inv, name)?;
    let admitted = mcp_ops::submit(&c, name, &Auth::Session, "collection_start", params).await?;
    if !wait {
        return emit(inv, &admitted.accepted, format);
    }
    let id = admitted.job_id.clone();
    let job = mcp_ops::wait(inv, &c, name, &Auth::Session, &id, timeout, "collection").await?;
    emit(inv, &job, format)?;
    admitted.deliver(inv)?;
    match job["job"]["state"].as_str() {
        Some(state @ ("failed" | "canceled")) => Err(CliError::operational(format!(
            "The collection job ended {state}"
        ))),
        _ => Ok(()),
    }
}

/// `connections set|remove`: one tenant transaction; the first `mcp` write saves a
/// private rollback snapshot (older binaries refuse the table).
fn write(
    inv: &mut Invocation,
    set: Option<McpConnectionInput>,
    remove: Option<String>,
) -> Result<(), CliError> {
    inv.secrets.guard_local_output();
    let context = inv.load_tenant()?;
    let tenant = context.selected.name.clone();
    let store = inv.env.store()?;
    let mut tx = tenant_write::begin(inv, &store, true)?;
    let mut snapshot = None;
    let result = (|| -> Result<(), CliError> {
        let current = crate::tenant::StoreView {
            store: store.clone(),
            index: store.read_index()?,
            ts_registry: None,
        };
        let registration = crate::tenant::Selected {
            name: tenant.clone(),
            tsg_id: current
                .index
                .tsg(&tenant)
                .ok_or_else(|| CliError::operational("Tenant not found"))?
                .into(),
            by_flag: false,
        };
        let (mut config, previous) = current.load_for_write(&registration, true)?;
        if let Some(input) = &set {
            let upgrading = config.mcp().is_none();
            config.set_mcp(&input.name, input.connection.clone(), input.replace)?;
            if upgrading {
                snapshot = Some(tx.scanner_snapshot(&tenant, "pre-mcp", &previous)?);
            }
        }
        if let Some(name) = &remove
            && !config.remove_mcp(name)
        {
            return Err(CliError::operational(
                "No such MCP connection in the selected tenant",
            ));
        }
        tx.write(
            &store.tenant_path(&tenant),
            config.to_toml().as_bytes(),
            Some(previous),
        )
    })();
    if let Err(error) = result {
        let rolled_back = tx.rollback();
        tenant_write::finish(inv, tx);
        return Err(if rolled_back {
            error
        } else {
            CliError::operational(
                "Connection change failed; verify the tenant configuration and rollback snapshots",
            )
        });
    }
    inv.committed = true;
    tenant_write::finish(inv, tx);
    inv.output.success("MCP connection change saved");
    if let Some(path) = snapshot {
        inv.output.status(&format!(
            "Private rollback snapshot: {}",
            inv.secrets.scrub(&store::shown(&path))
        ));
    }
    Ok(())
}
