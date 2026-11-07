//! M6d planning and dispatch (DESIGN §18): every §18.3 argument check runs here, before
//! the tenant is read (D220); dispatch connects the management client with the planes the
//! command needs (`ManagementClients::build_for`, §18.3) and runs it.
//!
//! Step 1 (§18.1) plans the reads: `api-keys list`, `customer-apps list|get|consumption`,
//! `deployment-profiles list`, the `scan-logs query` stub and every `dashboard` and
//! `sessions` read. Step 2 plans the writes (`api-keys create|regenerate|delete`,
//! `customer-apps update|delete`: their arguments and the D81 gate) and `runtime report`.
//! Step 3 adds `profiles backup|restore`: their plans come from `profile_backup::plan`
//! and `profile_restore::plan` (§18.15, §18.16) and are dispatched here like the others.

use super::report_command::ReportDestination;
use super::runtime_apps::UpdatePlan;
use super::runtime_keys::RegeneratePlan;
use super::runtime_report::RuntimeReportPlan;
use super::{Invocation, Plan};
use crate::cli::{
    ApiKeysCommand, CustomerAppsCommand, DashboardCommand, DashboardPageArgs, DashboardWindowArgs,
    DashboardWindowDaysArgs, DeploymentProfilesCommand, ProfilesCommand, RuntimeCommand,
    ScanIdentityArgs, SessionIdentityArgs, SessionsCommand,
};
use crate::clients::{ManagementClients, Planes};
use crate::error::{CliError, InterruptContext};
use crate::output::{self, Format};
use crate::pagination::{self, ListParams};
use crate::runtime::keys;
use crate::runtime::{self, APPLICATION_UNITS, DETAIL_UNITS, TIME_UNITS};
use crate::tenant::TenantContext;
use std::ffi::OsString;
use std::path::PathBuf;

/// A time-range dashboard read (TS `window`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeRead {
    TopApplications,
    ViolationsTrend,
    AppsList,
    SessionsChart,
}

/// One dashboard or sessions read (§18.12, §18.13), validated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DashRead {
    Applications {
        interval: u64,
        unit: String,
        limit: u64,
        offset: u64,
    },
    Application {
        violations: bool,
        app_id: String,
        app_name: String,
        interval: u64,
    },
    TimeRange {
        read: TimeRead,
        interval: u64,
        unit: String,
    },
    Session {
        identity: SessionIdentity,
        limit: u64,
        offset: u64,
    },
    Transaction {
        identity: SessionIdentity,
        scan_id: String,
        sub_request: u64,
    },
}

/// The session identity and its window (TS `identity`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionIdentity {
    pub session_id: String,
    pub app_id: String,
    pub app_name: String,
    pub interval: u64,
    pub unit: String,
}

/// `sessions list|ls` (§18.13).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionsList {
    pub interval: u64,
    pub unit: String,
    pub limit: u64,
    pub offset: u64,
    pub all: bool,
    pub max: u64,
}

/// A validated M6d invocation; `format` is the explicit `--output`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RxPlan {
    KeysList {
        params: ListParams,
        format: Option<Format>,
    },
    AppsList {
        params: ListParams,
        format: Option<Format>,
    },
    AppsGet {
        name: String,
        format: Option<Format>,
    },
    Consumption {
        app: Option<String>,
        interval: u64,
        format: Option<Format>,
    },
    DeploymentProfiles {
        unactivated: bool,
        format: Option<Format>,
    },
    /// The TypeScript stub (§18.11): reads the tenant selection, sends nothing.
    ScanLogs(Box<super::runtime_scan_logs::ScanLogsPlan>),
    Dashboard {
        read: DashRead,
        format: Option<Format>,
    },
    SessionsList {
        list: SessionsList,
        format: Option<Format>,
    },
    ScanContent {
        scan_id: String,
        sub_request: u64,
        /// `--output-file` as given; `None`: `--show-content`.
        file: Option<PathBuf>,
        format: Option<Format>,
    },
    KeysCreate {
        config: OsString,
        key_file: PathBuf,
        format: Option<Format>,
    },
    KeysRegenerate(Box<RegeneratePlan>),
    KeysDelete {
        name: String,
        updated_by: String,
        force: bool,
    },
    AppsUpdate(Box<UpdatePlan>),
    AppsDelete {
        name: String,
        updated_by: String,
        force: bool,
    },
    Report(Box<RuntimeReportPlan>),
    /// `runtime profiles backup [profile]` (§18.15).
    ProfilesBackup(Box<super::profile_backup::BackupPlan>),
    /// `runtime profiles restore <file>` (§18.16).
    ProfilesRestore(Box<super::profile_restore::RestoreRun>),
}

impl RxPlan {
    /// What an interruption reports (§18.3 "Interruption"): the reads write no data
    /// (`scan-content --output-file` names its file once it is published).
    pub fn interrupt_context(&self) -> InterruptContext {
        match self {
            Self::KeysCreate { .. }
            | Self::KeysRegenerate(_)
            | Self::KeysDelete { .. }
            | Self::AppsUpdate(_)
            | Self::AppsDelete { .. } => InterruptContext::NothingChanged,
            Self::Report(_) => {
                InterruptContext::Context(super::report_command::NO_REPORT.to_owned())
            }
            Self::ProfilesBackup(_) => {
                InterruptContext::Context(super::profile_backup::NO_BACKUP_WRITTEN.to_owned())
            }
            Self::ProfilesRestore(_) => InterruptContext::NothingChanged,
            _ => InterruptContext::NoDataWritten,
        }
    }

    /// The `--debug` policy (§18.3 "Debug log"): every command that sends requests logs
    /// status and headers only; the `scan-logs` stub sends nothing; the report refuses
    /// `--debug` during planning.
    pub fn debug_policy(&self) -> crate::debug::DebugPolicy {
        match self {
            Self::ScanLogs(_) => crate::debug::DebugPolicy::Refused,
            Self::Report(_) | Self::ProfilesBackup(_) | Self::ProfilesRestore(_) => {
                crate::debug::DebugPolicy::Refused
            }
            _ => crate::debug::DebugPolicy::Log,
        }
    }
}

/// Whether the command line runs an M6d command (they check `--output` before the
/// tenant flag, §18.3 "Validation order").
pub fn is_runtime_extras(command: &RuntimeCommand) -> bool {
    matches!(
        command,
        RuntimeCommand::ApiKeys(_)
            | RuntimeCommand::CustomerApps(_)
            | RuntimeCommand::Dashboard(_)
            | RuntimeCommand::DeploymentProfiles(_)
            | RuntimeCommand::Report(_)
            | RuntimeCommand::ScanLogs(_)
            | RuntimeCommand::Sessions(_)
            | RuntimeCommand::Profiles(crate::cli::ProfilesGroup {
                command: Some(ProfilesCommand::Backup(_) | ProfilesCommand::Restore(_)),
            })
    )
}

/// The checks of `api-keys regenerate` (§18.5 "Arguments", in order): the id segment
/// (G29), `--interval`, `--unit`, `--updated-by`, `--key-file`, the body schema, the D81
/// gate.
fn plan_regenerate(
    args: &crate::cli::ApiKeyRegenerateArgs,
    format: Option<Format>,
) -> Result<RegeneratePlan, CliError> {
    crate::segment::check_segment("API key id", &args.api_key_id)?;
    let interval = runtime::rotation_interval(&args.interval)?;
    if crate::tenant::config::js_trim(&args.unit).is_empty() {
        return Err(CliError::usage("--unit must not be empty"));
    }
    runtime::check_text("--unit", &args.unit)?;
    let updated_by = args
        .updated_by
        .as_deref()
        .map(runtime::updated_by)
        .transpose()?;
    let key_file = keys::plan_key_file(&args.key_file)?;
    let mut body = serde_json::json!({
        "rotation_time_interval": interval,
        "rotation_time_unit": args.unit,
    });
    if let Some(updated_by) = updated_by {
        body["updated_by"] = serde_json::Value::String(updated_by);
    }
    scm::airs::management::models::ApiKeyRegenerateRequest::parse(
        body.clone(),
        scm::validation::Mode::Request,
    )
    .map_err(|_| {
        CliError::usage(
            crate::writes::invalid_body(
                "POST",
                &format!("/v1/mgmt/apikey/regenerate/{}", args.api_key_id),
            )
            .message(),
        )
    })?;
    super::runtime_write::gate(
        args.force,
        &format!("regenerate API key {}", args.api_key_id),
    )?;
    Ok(RegeneratePlan {
        id: args.api_key_id.clone(),
        body,
        key_file,
        force: args.force,
        format,
    })
}

/// The checks of `runtime report` (§18.14, in TS order).
fn plan_report(
    args: &crate::cli::RuntimeReportArgs,
    explicit: Option<Format>,
    debug: bool,
) -> Result<RuntimeReportPlan, CliError> {
    if debug {
        return Err(CliError::usage(
            "Disable --debug for environment reports to avoid persisting sensitive traffic content",
        ));
    }
    if explicit.is_some() {
        return Err(CliError::usage(
            "The environment report is written as HTML; omit --output",
        ));
    }
    if args.from_evidence.is_some()
        && (args.max_pages.is_some()
            || args.max_events.is_some()
            || args.max_reports.is_some()
            || args.interval.is_some()
            || args.unit.is_some()
            || !args.app_ids.is_empty()
            || args.budget.timeout.is_some()
            || args.budget.concurrency.is_some()
            || args.budget.max_requests.is_some())
    {
        return Err(CliError::usage(
            "--from-evidence cannot be combined with live collection options",
        ));
    }
    let max_pages = args.max_pages.as_deref().unwrap_or("40");
    let pages = (!max_pages.is_empty()
        && !max_pages.starts_with('0')
        && max_pages.bytes().all(|byte| byte.is_ascii_digit()))
    .then(|| max_pages.parse::<u32>().ok())
    .flatten()
    .filter(|pages| (1..=100).contains(pages))
    .ok_or_else(|| CliError::usage("--max-pages must be an integer from 1 to 100"))?;
    let title = args
        .title
        .clone()
        .unwrap_or_else(|| "AIRS Signal Brief".to_owned());
    if crate::tenant::config::js_trim(&title).is_empty() || title.encode_utf16().count() > 240 {
        return Err(CliError::usage("--title must contain 1–240 characters"));
    }
    let destination = match &args.output_file {
        Some(path) if crate::tenant::config::js_trim(&path.to_string_lossy()).is_empty() => {
            return Err(CliError::usage("--output-file cannot be empty"));
        }
        Some(path) if path == "-" => ReportDestination::Stdout,
        Some(path) => ReportDestination::File(PathBuf::from(path)),
        None => ReportDestination::File(PathBuf::from(super::runtime_report::default_name(
            std::time::SystemTime::now(),
        ))),
    };
    Ok(RuntimeReportPlan {
        destination,
        title,
        strict: args.strict,
        from_evidence: args.from_evidence.clone(),
        review_file: args.review_file.clone(),
        bundle_dir: args.bundle_dir.clone(),
        collection: {
            use crate::runtime::scan_logs::{self, CollectionOptions};
            let unit = args.unit.clone().unwrap_or_else(|| "hours".into());
            if !["hour", "hours", "day", "days"].contains(&unit.as_str()) {
                return Err(CliError::usage("--unit must be hour, hours, day or days"));
            }
            if args.app_ids.len() > 1000
                || args.app_ids.iter().any(|id| {
                    id.trim().is_empty() || id.len() > 4096 || id.chars().any(char::is_control)
                })
            {
                return Err(CliError::usage(
                    "Supply at most 1000 nonempty --app-id values without controls",
                ));
            }
            if args.bundle_dir.as_ref().is_some_and(|p| {
                p.as_os_str().is_empty()
                    || crate::tenant::config::js_trim(&p.to_string_lossy()).is_empty()
            }) {
                return Err(CliError::usage("--bundle-dir cannot be empty"));
            }
            CollectionOptions {
                limits: crate::runtime::evidence_limits(&args.budget)?,
                interval: scan_logs::number(args.interval.as_deref(), 24, 1, 87600, "--interval")?,
                unit,
                app_ids: args.app_ids.clone(),
                max_pages: u64::from(pages),
                max_events: scan_logs::number(
                    args.max_events.as_deref(),
                    10000,
                    1,
                    10000,
                    "--max-events",
                )?,
                max_reports: scan_logs::number(
                    args.max_reports.as_deref(),
                    5,
                    0,
                    10,
                    "--max-reports",
                )?,
            }
        },
    })
}

fn or<'a>(value: Option<&'a String>, default: &'a str) -> &'a str {
    value.map_or(default, String::as_str)
}

/// A command's `--interval` and `--unit` (with their defaults) once the unit vocabulary
/// is checked: TypeScript checks the unit while parsing (`rd.ts:52-70`), so this is the
/// command's first check, and the only place the vocabulary is checked.
struct Window<'a> {
    interval: &'a str,
    unit: &'a str,
}

impl<'a> Window<'a> {
    fn new(
        interval: Option<&'a String>,
        unit: Option<&'a String>,
        defaults: (&'a str, &'a str),
        units: &[&str],
    ) -> Result<Self, CliError> {
        runtime::check_unit(unit.map(String::as_str), units)?;
        Ok(Self {
            interval: or(interval, defaults.0),
            unit: or(unit, defaults.1),
        })
    }

    /// TS `window` (`rd.ts:28-33`).
    fn parse(&self) -> Result<(u64, String), CliError> {
        let interval = runtime::dash_integer(self.interval, "--interval", 1)?;
        let unit = runtime::dash_text(self.unit, "--unit")?;
        Ok((interval, unit))
    }
}

/// The unit vocabulary, then TS `window`.
fn window(
    interval: Option<&String>,
    unit: Option<&String>,
    defaults: (&str, &str),
    units: &[&str],
) -> Result<(u64, String), CliError> {
    Window::new(interval, unit, defaults, units)?.parse()
}

fn page(page: &DashboardPageArgs) -> Result<(u64, u64), CliError> {
    Ok((
        runtime::dash_integer(or(page.limit.as_ref(), "25"), "--limit", 1)?,
        runtime::dash_integer(or(page.offset.as_ref(), "0"), "--offset", 0)?,
    ))
}

fn required(value: Option<&String>, flag: &str) -> Result<String, CliError> {
    runtime::dash_text(or(value, ""), flag)
}

/// TS `identity` (`rd.ts:35-42`): the unit vocabulary first (parse time), then the three
/// names, then the window.
fn identity(args: &SessionIdentityArgs) -> Result<SessionIdentity, CliError> {
    let window = Window::new(
        args.window.interval.as_ref(),
        args.window.unit.as_ref(),
        ("30", "days"),
        TIME_UNITS,
    )?;
    let session_id = required(args.session_id.as_ref(), "--session-id")?;
    let app_id = required(args.app_id.as_ref(), "--app-id")?;
    let app_name = required(args.app_name.as_ref(), "--app-name")?;
    let (interval, unit) = window.parse()?;
    Ok(SessionIdentity {
        session_id,
        app_id,
        app_name,
        interval,
        unit,
    })
}

fn scan(args: &ScanIdentityArgs) -> Result<(String, u64), CliError> {
    Ok((
        required(args.scan_id.as_ref(), "--scan-id")?,
        runtime::dash_integer(
            or(args.scan_sub_req_id.as_ref(), ""),
            "--scan-sub-req-id",
            0,
        )?,
    ))
}

fn time_read(
    read: TimeRead,
    args_interval: Option<&String>,
    args_unit: Option<&String>,
    defaults: (&str, &str),
    format: Option<Format>,
) -> Result<RxPlan, CliError> {
    let (interval, unit) = window(args_interval, args_unit, defaults, TIME_UNITS)?;
    Ok(RxPlan::Dashboard {
        read: DashRead::TimeRange {
            read,
            interval,
            unit,
        },
        format,
    })
}

fn day_window(args: &DashboardWindowArgs) -> (Option<&String>, Option<&String>) {
    (args.interval.as_ref(), args.unit.as_ref())
}

fn days_window(args: &DashboardWindowDaysArgs) -> (Option<&String>, Option<&String>) {
    (args.interval.as_ref(), args.unit.as_ref())
}

/// Plan an M6d command line; `--output` and `--tenant` were validated already.
pub fn plan(
    command: &RuntimeCommand,
    explicit: Option<&str>,
    debug: bool,
) -> Result<Plan, CliError> {
    let format = output::validate_explicit(explicit)?;
    let plan = match command {
        RuntimeCommand::Report(args) => RxPlan::Report(Box::new(plan_report(args, format, debug)?)),
        RuntimeCommand::Profiles(group) => match &group.command {
            Some(ProfilesCommand::Backup(args)) => {
                RxPlan::ProfilesBackup(Box::new(super::profile_backup::plan(args, format, debug)?))
            }
            Some(ProfilesCommand::Restore(args)) => RxPlan::ProfilesRestore(Box::new(
                super::profile_restore::plan(args, format, debug)?,
            )),
            _ => unreachable!("planned by commands::plan"),
        },
        RuntimeCommand::ApiKeys(group) => match &group.command {
            None => return Ok(Plan::GroupHelp(&["runtime", "api-keys"])),
            Some(ApiKeysCommand::List(args)) => RxPlan::KeysList {
                params: pagination::parse_list(&args.paging, false)?,
                format,
            },
            Some(ApiKeysCommand::Create(args)) => RxPlan::KeysCreate {
                config: args.config.clone(),
                key_file: keys::plan_key_file(&args.key_file)?,
                format,
            },
            Some(ApiKeysCommand::Regenerate(args)) => {
                RxPlan::KeysRegenerate(Box::new(plan_regenerate(args, format)?))
            }
            Some(ApiKeysCommand::Delete(args)) => {
                crate::segment::check_segment("API key name", &args.api_key_name)?;
                let updated_by = runtime::updated_by(&args.updated_by)?;
                super::runtime_write::gate(
                    args.force,
                    &format!("delete API key {}", args.api_key_name),
                )?;
                RxPlan::KeysDelete {
                    name: args.api_key_name.clone(),
                    updated_by,
                    force: args.force,
                }
            }
        },
        RuntimeCommand::CustomerApps(group) => match &group.command {
            None => return Ok(Plan::GroupHelp(&["runtime", "customer-apps"])),
            Some(CustomerAppsCommand::List(args)) => RxPlan::AppsList {
                params: pagination::parse_list(&args.paging, false)?,
                format,
            },
            Some(CustomerAppsCommand::Get(args)) => RxPlan::AppsGet {
                name: runtime::non_blank(
                    &args.app_name,
                    "Customer app name must not be empty",
                    "appName",
                )?,
                format,
            },
            Some(CustomerAppsCommand::Update(args)) => {
                let id =
                    runtime::non_blank(&args.app_id, "Customer app id must not be empty", "appId")?;
                super::runtime_write::gate(args.force, &format!("replace customer app {id}"))?;
                RxPlan::AppsUpdate(Box::new(UpdatePlan {
                    id,
                    config: args.config.clone(),
                    force: args.force,
                    format,
                }))
            }
            Some(CustomerAppsCommand::Delete(args)) => {
                let name = runtime::non_blank(
                    &args.app_name,
                    "Customer app name must not be empty",
                    "appName",
                )?;
                let updated_by = runtime::updated_by(&args.updated_by)?;
                super::runtime_write::gate(args.force, &format!("delete customer app {name}"))?;
                RxPlan::AppsDelete {
                    name,
                    updated_by,
                    force: args.force,
                }
            }
            Some(CustomerAppsCommand::Consumption(args)) => {
                let interval = runtime::time_interval(args.time_interval.as_deref())?;
                let app = args
                    .app_name
                    .as_deref()
                    .map(|name| {
                        runtime::non_blank(
                            name,
                            "appName must not be empty; omit it to report every dashboard application",
                            "appName",
                        )
                    })
                    .transpose()?;
                RxPlan::Consumption {
                    app,
                    interval,
                    format,
                }
            }
        },
        RuntimeCommand::DeploymentProfiles(group) => match &group.command {
            None => return Ok(Plan::GroupHelp(&["runtime", "deployment-profiles"])),
            Some(DeploymentProfilesCommand::List(args)) => RxPlan::DeploymentProfiles {
                unactivated: args.unactivated,
                format,
            },
        },
        RuntimeCommand::ScanLogs(group) => match &group.command {
            None => return Ok(Plan::GroupHelp(&["runtime", "scan-logs"])),
            Some(command) => RxPlan::ScanLogs(Box::new(super::runtime_scan_logs::plan(
                command, format, debug,
            )?)),
        },
        RuntimeCommand::Dashboard(group) => match &group.command {
            None => return Ok(Plan::GroupHelp(&["runtime", "dashboard"])),
            Some(DashboardCommand::Applications(args)) => {
                // The vocabulary check leaves only TS's interval test (`rd.ts:166`).
                let (interval, unit) = window(
                    args.interval.as_ref(),
                    args.unit.as_ref(),
                    ("1", "day"),
                    APPLICATION_UNITS,
                )?;
                if ![1, 7, 30, 60].contains(&interval) {
                    return Err(CliError::usage(
                        "Application overview supports interval 1/7/30/60 and unit day/days/hour",
                    ));
                }
                let (limit, offset) = page(&args.page)?;
                RxPlan::Dashboard {
                    read: DashRead::Applications {
                        interval,
                        unit,
                        limit,
                        offset,
                    },
                    format,
                }
            }
            Some(
                DashboardCommand::Application(args) | DashboardCommand::ApplicationViolations(args),
            ) => {
                let violations = matches!(
                    group.command,
                    Some(DashboardCommand::ApplicationViolations(_))
                );
                // The vocabulary is `days` alone, so TS's unit test (`rd.ts:195`) cannot
                // fail after it; the unit is not sent (`time_unit=days`, §18.12).
                let window = Window::new(
                    args.interval.as_ref(),
                    args.unit.as_ref(),
                    ("30", "days"),
                    DETAIL_UNITS,
                )?;
                let interval = runtime::dash_integer(window.interval, "--interval", 1)?;
                if ![7, 30, 60].contains(&interval) {
                    return Err(CliError::usage(
                        "Application detail requires 7, 30, or 60 days",
                    ));
                }
                RxPlan::Dashboard {
                    read: DashRead::Application {
                        violations,
                        app_id: required(args.app_id.as_ref(), "--app-id")?,
                        app_name: required(args.app_name.as_ref(), "--app-name")?,
                        interval,
                    },
                    format,
                }
            }
            Some(DashboardCommand::TopApplications(args)) => {
                let (interval, unit) = day_window(args);
                time_read(
                    TimeRead::TopApplications,
                    interval,
                    unit,
                    ("1", "day"),
                    format,
                )?
            }
            Some(DashboardCommand::ViolationsTrend(args)) => {
                let (interval, unit) = day_window(args);
                time_read(
                    TimeRead::ViolationsTrend,
                    interval,
                    unit,
                    ("1", "day"),
                    format,
                )?
            }
            Some(DashboardCommand::AppsList(args)) => {
                let (interval, unit) = days_window(args);
                time_read(TimeRead::AppsList, interval, unit, ("30", "days"), format)?
            }
        },
        RuntimeCommand::Sessions(group) => match &group.command {
            None => return Ok(Plan::GroupHelp(&["runtime", "sessions"])),
            Some(SessionsCommand::Chart(args)) => {
                let (interval, unit) = day_window(args);
                time_read(
                    TimeRead::SessionsChart,
                    interval,
                    unit,
                    ("1", "day"),
                    format,
                )?
            }
            Some(SessionsCommand::List(args)) => {
                let (interval, unit) = window(
                    args.window.interval.as_ref(),
                    args.window.unit.as_ref(),
                    ("1", "day"),
                    TIME_UNITS,
                )?;
                let (limit, offset) = page(&args.page)?;
                let max = runtime::dash_integer(or(args.max.as_ref(), "10000"), "--max", 0)?;
                RxPlan::SessionsList {
                    list: SessionsList {
                        interval,
                        unit,
                        limit,
                        offset,
                        all: args.all,
                        max,
                    },
                    format,
                }
            }
            Some(SessionsCommand::Get(args)) => {
                let identity = identity(&args.identity)?;
                let (limit, offset) = page(&args.page)?;
                RxPlan::Dashboard {
                    read: DashRead::Session {
                        identity,
                        limit,
                        offset,
                    },
                    format,
                }
            }
            Some(SessionsCommand::Transaction(args)) => {
                let identity = identity(&args.identity)?;
                let (scan_id, sub_request) = scan(&args.scan)?;
                RxPlan::Dashboard {
                    read: DashRead::Transaction {
                        identity,
                        scan_id,
                        sub_request,
                    },
                    format,
                }
            }
            Some(SessionsCommand::ScanContent(args)) => {
                let (scan_id, sub_request) = scan(&args.scan)?;
                // TS: `Boolean(showContent) === Boolean(outputFile)`, so `--output-file ''`
                // counts as absent.
                let file = args.output_file.as_ref().filter(|path| !path.is_empty());
                if args.show_content == file.is_some() {
                    return Err(CliError::usage(
                        "Scan content requires exactly one of --show-content or --output-file <new-path>",
                    ));
                }
                if let Some(path) = file
                    && path
                        .to_str()
                        .is_some_and(|text| crate::tenant::config::js_trim(text).is_empty())
                {
                    return Err(CliError::usage("--output-file is required"));
                }
                RxPlan::ScanContent {
                    scan_id,
                    sub_request,
                    file: file.map(PathBuf::from),
                    format,
                }
            }
        },
        _ => unreachable!("planned by commands::plan"),
    };
    Ok(Plan::RuntimeExtras(Box::new(plan)))
}

/// Run an M6d plan.
pub async fn dispatch(invocation: &mut Invocation, plan: RxPlan) -> Result<(), CliError> {
    use super::{runtime_apps as apps, runtime_dashboard as dash, runtime_inventory as inv};
    match plan {
        RxPlan::KeysList { params, format } => {
            super::runtime_keys::list(invocation, &params, format).await
        }
        RxPlan::AppsList { params, format } => apps::list(invocation, &params, format).await,
        RxPlan::AppsGet { name, format } => apps::get(invocation, &name, format).await,
        RxPlan::Consumption {
            app,
            interval,
            format,
        } => apps::consumption(invocation, app.as_deref(), interval, format).await,
        RxPlan::DeploymentProfiles {
            unactivated,
            format,
        } => inv::deployment_profiles(invocation, unactivated, format).await,
        RxPlan::ScanLogs(plan) => super::runtime_scan_logs::run(invocation, &plan).await,
        RxPlan::Dashboard { read, format } => dash::read(invocation, &read, format).await,
        RxPlan::SessionsList { list, format } => {
            dash::sessions_list(invocation, &list, format).await
        }
        RxPlan::ScanContent {
            scan_id,
            sub_request,
            file,
            format,
        } => dash::scan_content(invocation, &scan_id, sub_request, file.as_deref(), format).await,
        RxPlan::KeysCreate {
            config,
            key_file,
            format,
        } => super::runtime_keys::create(invocation, &config, &key_file, format).await,
        RxPlan::KeysRegenerate(plan) => super::runtime_keys::regenerate(invocation, &plan).await,
        RxPlan::KeysDelete {
            name,
            updated_by,
            force,
        } => super::runtime_keys::delete(invocation, &name, &updated_by, force).await,
        RxPlan::AppsUpdate(plan) => apps::update(invocation, &plan).await,
        RxPlan::AppsDelete {
            name,
            updated_by,
            force,
        } => apps::delete(invocation, &name, &updated_by, force).await,
        RxPlan::Report(plan) => super::runtime_report::run(invocation, &plan).await,
        RxPlan::ProfilesBackup(plan) => super::profile_backup::run(invocation, &plan).await,
        RxPlan::ProfilesRestore(plan) => super::profile_restore::run(invocation, &plan).await,
    }
}

/// The selected tenant of an M6d command once its management credentials are checked
/// (§3.5), before any client is built: what the transfer commands' tenant checks need
/// (§18.16 "Tenant checks ... before the client and any request").
pub struct RuntimeTenant {
    context: TenantContext,
    tsg: String,
}

impl RuntimeTenant {
    /// The tenant's store name.
    pub fn name(&self) -> &str {
        self.context.name()
    }

    /// The tenant's `mgmtTsgId`, trimmed (§3.7).
    pub fn tsg(&self) -> &str {
        &self.tsg
    }
}

/// A connected M6d client: the management clients (with the planes the command needs)
/// and the tenant's `defaultOutput`.
pub struct Connected {
    pub clients: ManagementClients,
    pub default_output: Option<String>,
}

impl Connected {
    /// The format of a read: the explicit `--output`, else the tenant's `defaultOutput`
    /// (choice 18), else `pretty`.
    pub fn format(&self, explicit: Option<Format>) -> Format {
        output::resolve_tenant_default(explicit, self.default_output.as_deref())
    }
}

impl Invocation {
    /// Load the tenant and check the management credentials (§3.5), without building a
    /// client or sending a request.
    pub fn runtime_tenant(&mut self) -> Result<RuntimeTenant, CliError> {
        let context = self.load_tenant()?;
        crate::tenant::config::require_management(&context.config, Some(context.name()))?;
        let tsg = crate::tenant::config::js_trim(
            context
                .config
                .effective_str("mgmtTsgId")
                .unwrap_or_default(),
        )
        .to_owned();
        Ok(RuntimeTenant { context, tsg })
    }

    /// Build the management client of `tenant` with `planes` (§18.3; each endpoint
    /// admitted by §5), without any request: the caller fetches the token.
    pub fn connect_tenant(
        &self,
        tenant: &RuntimeTenant,
        planes: Planes,
    ) -> Result<Connected, CliError> {
        let config = &tenant.context.config;
        let clients = ManagementClients::build_for(
            config,
            &self.session,
            &self.secrets,
            self.debug.as_ref(),
            planes,
        )?;
        Ok(Connected {
            clients,
            default_output: config.default_output().map(str::to_owned),
        })
    }

    /// [`Invocation::runtime_tenant`] then [`Invocation::connect_tenant`]: the commands
    /// with no check between the tenant and the client.
    pub fn connect_runtime(&mut self, planes: Planes) -> Result<Connected, CliError> {
        let tenant = self.runtime_tenant()?;
        self.connect_tenant(&tenant, planes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::FromArgMatches;

    fn plan_of(args: &[&str]) -> Result<Plan, CliError> {
        let mut full = vec!["airs", "runtime"];
        full.extend_from_slice(args);
        let matches = crate::cli::command().try_get_matches_from(&full).unwrap();
        crate::commands::plan(&Cli::from_arg_matches(&matches).unwrap())
    }

    fn rx(args: &[&str]) -> RxPlan {
        match plan_of(args) {
            Ok(Plan::RuntimeExtras(plan)) => *plan,
            Ok(_) => panic!("not an M6d plan: {args:?}"),
            Err(error) => panic!("{args:?}: {error:?}"),
        }
    }

    fn usage(args: &[&str]) -> String {
        match plan_of(args) {
            Err(CliError::Usage(message)) => message,
            Err(other) => panic!("expected a usage error, got {other:?}"),
            Ok(_) => panic!("expected a usage error: {args:?}"),
        }
    }

    /// §18.3 "Validation order": `--output`, then `--tenant`, then the arguments (D220).
    #[test]
    fn output_and_tenant_flag_come_before_the_arguments() {
        let parse = |args: &[&str]| {
            let matches = crate::cli::command().try_get_matches_from(args).unwrap();
            crate::commands::plan(&Cli::from_arg_matches(&matches).unwrap())
        };
        for dead in ["yaml", "markdown", "csv"] {
            assert!(matches!(
                parse(&["airs", "--tenant", "-bad", "runtime", "sessions", "chart", "--unit", "week", "--output", dead]),
                Err(CliError::Usage(message)) if message == format!("Invalid output format '{dead}'. Expected: pretty, table, json")
            ));
        }
        assert!(matches!(
            parse(&["airs", "--tenant", "-bad", "runtime", "api-keys", "list", "--limit", "0"]),
            Err(CliError::Usage(message)) if message.starts_with("Tenant name must be")
        ));
    }

    /// Every §18.3 check, with the TS texts (TR§3.1, TX§3.1, D220).
    #[test]
    fn argument_checks() {
        for (args, message) in [
            (
                &["api-keys", "list", "--limit", "0"][..],
                "--limit must be a positive integer",
            ),
            (
                &["customer-apps", "ls", "--all", "--offset", "0"],
                "--all cannot be combined with --offset",
            ),
            (
                &["customer-apps", "get", " "],
                "Customer app name must not be empty",
            ),
            (
                &["customer-apps", "get", "a\u{1b}"],
                "appName must be at most 4096 bytes and contain no control characters",
            ),
            (
                &["customer-apps", "consumption", ""],
                "appName must not be empty; omit it to report every dashboard application",
            ),
            (
                &["customer-apps", "consumption", "--time-interval", "7.5"],
                "--time-interval must be 7, 30, or 60 (the API rejects other values)",
            ),
            (
                &["sessions", "list", "--unit", "week"],
                "Unsupported --unit. Supported units: hour, hours, day, days. For one week, use --interval 7 --unit days.",
            ),
            (
                &["sessions", "list", "--limit", "0"],
                "--limit must be a safe integer >= 1",
            ),
            (
                &["sessions", "list", "--offset", "-1"],
                "--offset must be a safe integer >= 0",
            ),
            (
                &["sessions", "list", "--max", "NaN"],
                "--max must be a safe integer >= 0",
            ),
            (
                &["sessions", "chart", "--interval", "1oops"],
                "--interval must be a safe integer >= 1",
            ),
            (
                &["dashboard", "applications", "--unit", "hours"],
                "Unsupported --unit. Supported units: day, days, hour. For one week, use --interval 7 --unit days.",
            ),
            (
                &["dashboard", "applications", "--interval", "2"],
                "Application overview supports interval 1/7/30/60 and unit day/days/hour",
            ),
            (
                &[
                    "dashboard",
                    "application",
                    "--app-id",
                    "a",
                    "--app-name",
                    "n",
                    "--interval",
                    "1",
                ],
                "Application detail requires 7, 30, or 60 days",
            ),
            (
                &[
                    "dashboard",
                    "application",
                    "--app-id",
                    "a",
                    "--app-name",
                    "n",
                    "--unit",
                    "day",
                ],
                "Unsupported --unit. Supported units: days. For one week, use --interval 7 --unit days.",
            ),
            (
                &[
                    "dashboard",
                    "application-violations",
                    "--app-id",
                    " ",
                    "--app-name",
                    "n",
                ],
                "--app-id is required",
            ),
            (
                &[
                    "sessions",
                    "get",
                    "--session-id",
                    "s",
                    "--app-id",
                    "a",
                    "--app-name",
                    "\t",
                ],
                "--app-name is required",
            ),
            // TS order: the unit vocabulary (parse time), the names, then the window.
            (
                &[
                    "sessions",
                    "get",
                    "--session-id",
                    " ",
                    "--app-id",
                    "a",
                    "--app-name",
                    "n",
                    "--unit",
                    "week",
                ],
                "Unsupported --unit. Supported units: hour, hours, day, days. For one week, use --interval 7 --unit days.",
            ),
            (
                &[
                    "sessions",
                    "get",
                    "--session-id",
                    " ",
                    "--app-id",
                    "a",
                    "--app-name",
                    "n",
                    "--interval",
                    "0",
                ],
                "--session-id is required",
            ),
            (
                &[
                    "sessions",
                    "transaction",
                    "--session-id",
                    "s",
                    "--app-id",
                    "a",
                    "--app-name",
                    "n",
                    "--scan-id",
                    "x",
                    "--scan-sub-req-id",
                    "-1",
                ],
                "--scan-sub-req-id must be a safe integer >= 0",
            ),
            (
                &[
                    "sessions",
                    "scan-content",
                    "--scan-id",
                    "x",
                    "--scan-sub-req-id",
                    "0",
                ],
                "Scan content requires exactly one of --show-content or --output-file <new-path>",
            ),
            (
                &[
                    "sessions",
                    "scan-content",
                    "--scan-id",
                    "x",
                    "--scan-sub-req-id",
                    "0",
                    "--show-content",
                    "--output-file",
                    "f",
                ],
                "Scan content requires exactly one of --show-content or --output-file <new-path>",
            ),
            (
                &[
                    "sessions",
                    "scan-content",
                    "--scan-id",
                    "x",
                    "--scan-sub-req-id",
                    "0",
                    "--output-file",
                    " ",
                ],
                "--output-file is required",
            ),
        ] {
            assert_eq!(usage(args), message, "{args:?}");
        }
    }

    /// Step 2 (§18.5, §18.6, §18.8, §18.14): the write plans change nothing when
    /// interrupted before a write; the report refuses `--debug` and names its context.
    #[test]
    fn write_and_report_plans() {
        let plan = rx(&[
            "api-keys",
            "regenerate",
            "a/b",
            "--interval",
            "030",
            "--unit",
            "days",
            "--key-file",
            "k.key",
            "--force",
        ]);
        let RxPlan::KeysRegenerate(regenerate) = &plan else {
            panic!("{plan:?}");
        };
        assert_eq!(regenerate.id, "a/b");
        assert_eq!(
            regenerate.body,
            serde_json::json!({"rotation_time_interval": 30, "rotation_time_unit": "days"})
        );
        assert_eq!(plan.interrupt_context(), InterruptContext::NothingChanged);
        assert_eq!(plan.debug_policy(), crate::debug::DebugPolicy::Log);
        let report = rx(&["report", "--max-pages", "5", "--output-file", "-"]);
        assert_eq!(report.debug_policy(), crate::debug::DebugPolicy::Refused);
        assert_eq!(
            report.interrupt_context(),
            InterruptContext::Context("no report was written".into())
        );
        for (args, message) in [
            (
                &[
                    "api-keys",
                    "create",
                    "--config",
                    "c.json",
                    "--key-file",
                    "-",
                ][..],
                "--key-file - would print the key; give a file path",
            ),
            (
                &[
                    "api-keys",
                    "delete",
                    "%2e%2e",
                    "--updated-by",
                    "u",
                    "--force",
                ],
                "Invalid API key name: %2e%2e",
            ),
            (
                &[
                    "customer-apps",
                    "update",
                    "\t",
                    "--config",
                    "c.json",
                    "--force",
                ],
                "Customer app id must not be empty",
            ),
            (
                &["report", "--max-pages", "100", "--title", " "],
                "--title must contain 1–240 characters",
            ),
        ] {
            assert_eq!(usage(args), message, "{args:?}");
        }
    }

    #[test]
    fn plans_and_defaults() {
        assert_eq!(
            rx(&["dashboard", "applications"]),
            RxPlan::Dashboard {
                read: DashRead::Applications {
                    interval: 1,
                    unit: "day".into(),
                    limit: 25,
                    offset: 0
                },
                format: None
            }
        );
        assert_eq!(
            rx(&[
                "dashboard",
                "apps-list",
                "--interval",
                "01",
                "--output",
                "table"
            ]),
            RxPlan::Dashboard {
                read: DashRead::TimeRange {
                    read: TimeRead::AppsList,
                    interval: 1,
                    unit: "days".into()
                },
                format: Some(Format::Table)
            }
        );
        assert_eq!(
            rx(&["customer-apps", "consumption"]),
            RxPlan::Consumption {
                app: None,
                interval: 30,
                format: None
            }
        );
        let plan = rx(&["scan-logs", "query", "--interval", "24", "--unit", "hours"]);
        assert!(matches!(plan, RxPlan::ScanLogs(_)));
        assert_eq!(plan.debug_policy(), crate::debug::DebugPolicy::Refused);
        let list = rx(&["sessions", "ls", "--all", "--offset", "2"]);
        assert_eq!(list.debug_policy(), crate::debug::DebugPolicy::Log);
        assert_eq!(list.interrupt_context(), InterruptContext::NoDataWritten);
        assert!(matches!(
            plan_of(&["sessions"]),
            Ok(Plan::GroupHelp(["runtime", "sessions"]))
        ));
    }
}
