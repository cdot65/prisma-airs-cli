//! Command planning (all usage validation, before any tenant read) and dispatch.

pub mod bulk_scan;
pub mod dlp;
pub mod dlp_backup;
pub mod dlp_dictionaries;
pub mod dlp_filtering;
pub mod dlp_patterns;
pub mod dlp_profiles;
pub mod dlp_reads;
pub mod dlp_write;
pub mod doctor;
pub mod gateway;
pub mod gateway_inference;
pub mod gateway_inventory;
pub mod gateway_provision;
pub mod gateway_report;
pub mod gateway_telemetry;
pub mod gateway_workspaces;
pub mod gateway_write_plan;
pub mod gateway_writes;
pub mod mcp_client;
pub mod mcp_ops;
pub mod modelsec;
pub mod modelsec_groups;
pub mod modelsec_install;
pub mod modelsec_labels;
pub mod modelsec_reads;
pub mod modelsec_rules;
pub mod modelsec_scans;
pub mod modelsec_write;
pub mod profile_backup;
pub mod profile_cleanup;
pub mod profile_restore;
pub mod profile_write;
pub mod profiles;
pub mod redteam;
pub mod redteam_adapters;
pub mod redteam_backup;
pub mod redteam_custom;
pub mod redteam_infra;
pub mod redteam_report;
pub mod redteam_scan;
pub mod redteam_targets;
pub mod redteam_write;
pub mod report_command;
pub mod runtime_apps;
pub mod runtime_dashboard;
pub mod runtime_extras;
pub mod runtime_inventory;
pub mod runtime_keys;
pub mod runtime_report;
pub mod runtime_scan_logs;
pub mod runtime_write;
pub mod scan;
pub mod supplychain;
pub mod supplychain_report;
pub mod tenant;
pub mod tenant_scanners;
pub mod tenant_write;
pub mod topic_assign;
pub mod topic_eval;
pub mod topic_write;
pub mod topics;
pub mod write_common;

use crate::cli::{Cli, ProfilesCommand, RuntimeCommand, TenantCommand, TopCommand, TopicsCommand};
use crate::debug::{DebugLog, DebugPolicy};
use crate::error::{
    self, CliError, InterruptContext, Operation, Phase, Plane, SdkContext, SecretSet,
};
use crate::output::{self, Format, Output};
use crate::pagination::{self, GetParams, ListParams};
use crate::tenant::{self as registry, Environment, TenantContext};
use std::time::Duration;

/// Per-invocation state shared by every command.
pub struct Invocation {
    pub output: Output,
    /// Shared by every SDK client; cancelled by SIGINT/SIGTERM.
    pub session: scm::Session,
    pub env: Environment,
    /// The validated `--tenant` value.
    pub tenant_flag: Option<String>,
    /// Secrets seen this invocation (shared with the OAuth provider, which adds every
    /// lease); server text carrying one is never printed.
    pub secrets: SecretSet,
    pub payload_spans: crate::report::signal::privacy::PayloadSpans,
    /// The `--debug` API log, when this run writes one (DESIGN §2.9); every client's
    /// transport records into it.
    pub debug: Option<DebugLog>,
    /// Which streams are terminals (the prompt and confirmation gates, §3.10).
    pub terminals: crate::input::Terminals,
    /// Whether a tenant write passed its commit point (an interruption then reports
    /// `the change was saved`, §3.9).
    pub committed: bool,
    /// What an interruption reports when no SDK call said otherwise and `committed` is
    /// not set: a multi-step write after its first step (`topics revert`), or the scans
    /// sent so far (`topics eval`) (DESIGN §13.2). `None` keeps the plan's context.
    pub interrupt: Option<InterruptContext>,
    /// Normalized receipts of what this command received (scan/bulk-scan/resume-poll),
    /// retained on the MCP service only with `--record-history` (contract §7.2).
    pub retained: Vec<serde_json::Value>,
}

impl Invocation {
    /// What the shared collector reads from this invocation (session, secrets, payload
    /// spans and the CLI version).
    pub fn collect_context(&self) -> crate::report::signal::sink::CollectContext<'_> {
        crate::report::signal::sink::CollectContext {
            session: &self.session,
            secrets: &self.secrets,
            payload_spans: &self.payload_spans,
            version: crate::VERSION,
        }
    }

    pub fn new(output: Output, tenant_flag: Option<String>) -> Self {
        Self {
            output,
            session: scm::Session::new(),
            env: Environment::capture(),
            tenant_flag,
            secrets: SecretSet::default(),
            payload_spans: crate::report::signal::privacy::PayloadSpans::default(),
            debug: None,
            terminals: crate::input::Terminals::detect(),
            committed: false,
            interrupt: None,
            retained: Vec::new(),
        }
    }

    /// Open the debug log (after planning, before the tenant is read) and print its
    /// notice. A failure is an operational error and no request is sent.
    pub fn open_debug_log(&mut self) -> Result<(), CliError> {
        let dir =
            crate::debug::directory(self.env.xdg_state_home.as_deref(), self.env.home.as_deref())?;
        let log = crate::debug::open(
            &dir,
            self.secrets.clone(),
            crate::render::style::stderr_color(),
        )?;
        self.output.status(&format!(
            "Debug: API log → {}",
            output::terminal_safe(&log.path().display().to_string())
        ));
        self.debug = Some(log);
        Ok(())
    }

    /// Load the selected tenant (store, selection, tenant file, TSG pin) and register
    /// its secrets for output scrubbing.
    pub fn load_tenant(&mut self) -> Result<TenantContext, CliError> {
        let context = registry::load_selected(&self.env, self.tenant_flag.as_deref())?;
        self.secrets.register_config(&context.config);
        Ok(context)
    }

    /// Prepare a management read (milestone 2): load the tenant, settle the output
    /// format (`format` is the explicit `--output`; otherwise the tenant's
    /// `defaultOutput`, else `pretty`), check the management credentials, build the
    /// clients and fetch the OAuth token under `options` (from [`walk_options`];
    /// `None`: the SDK's own token budget).
    pub async fn connect_management(
        &mut self,
        format: Option<Format>,
        options: Option<&scm::RequestOptions>,
    ) -> Result<(scm::airs::management::ManagementClient, Format), CliError> {
        let context = self.load_tenant()?;
        let format = output::resolve_tenant_default(format, context.config.default_output());
        crate::tenant::config::require_management(&context.config, Some(context.name()))?;
        let clients = crate::clients::ManagementClients::build(
            &context.config,
            &self.session,
            &self.secrets,
            self.debug.as_ref(),
        )?;
        let budget = if options.is_some() {
            crate::clients::WALK_BUDGET
        } else {
            crate::clients::TOKEN_BUDGET
        };
        clients.authenticate(options, budget).await?;
        Ok((clients.management, format))
    }

    /// Load the tenant, check the management credentials and build the clients, without
    /// any request (a confirmation may follow before the token is fetched, §13.2).
    pub fn management_clients(&mut self) -> Result<crate::clients::ManagementClients, CliError> {
        let context = self.load_tenant()?;
        crate::tenant::config::require_management(&context.config, Some(context.name()))?;
        Ok(crate::clients::ManagementClients::build(
            &context.config,
            &self.session,
            &self.secrets,
            self.debug.as_ref(),
        )?)
    }

    /// Map an SDK error from a management API call made under `budget`.
    pub fn management_error(&self, error: &scm::Error, budget: Duration) -> CliError {
        error::from_sdk(
            error,
            &SdkContext {
                plane: Plane::Management,
                phase: Phase::Api,
                operation: Operation::Read,
                timeout: budget,
                secrets: &self.secrets,
            },
        )
    }
}

/// An interruption before any write of a runtime write command was sent reads
/// `nothing was changed` (§13.2), not the read commands' `no data was written`.
pub fn nothing_changed(error: CliError) -> CliError {
    match error {
        CliError::Interrupted(InterruptContext::NoDataWritten) => {
            CliError::Interrupted(InterruptContext::NothingChanged)
        }
        other => other,
    }
}

/// An M4 confirmation (§13.2): `? <message> (y/N) ` on stderr; `y` or `yes` confirm. A
/// signal that already arrived stops the command first (nothing was changed); one at
/// the prompt ends the process with `Cancelled; nothing was changed.`.
pub fn confirm(invocation: &mut Invocation, message: &str) -> Result<bool, CliError> {
    if let Some(signals) = invocation.output.signals() {
        signals.set_runtime_prompt();
    }
    if invocation.output.pending_signal().is_some() {
        return Err(CliError::Interrupted(InterruptContext::NothingChanged));
    }
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    Ok(crate::input::Prompter::new(&mut input, &mut invocation.output, true).confirm(message))
}

/// A hidden write prompt (§3.10): `? <label> [input is masked] ` on stderr with echo off
/// on a terminal stdin, asked until the answer is nonblank. As [`confirm`], a signal that
/// already arrived stops the command first (nothing was changed); EOF at the prompt ends
/// the process with `Cancelled; nothing was changed.` (exit 130).
pub fn prompt_hidden(invocation: &mut Invocation, label: &str) -> Result<String, CliError> {
    if let Some(signals) = invocation.output.signals() {
        signals.set_runtime_prompt();
    }
    if invocation.output.pending_signal().is_some() {
        return Err(CliError::Interrupted(InterruptContext::NothingChanged));
    }
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let terminal = invocation.terminals.stdin;
    let answer = crate::input::Prompter::new(&mut input, &mut invocation.output, terminal).ask(
        label,
        true,
        |answer| Ok(answer.to_owned()),
    );
    answer.map_err(|crate::input::Cancelled| {
        invocation.output.write_stderr("\n");
        invocation
            .output
            .status(crate::signals::CANCELLED_NOTHING_CHANGED);
        CliError::Reported(crate::error::EXIT_SIGINT)
    })
}

/// The deprecated-alias warning (TS text, DESIGN §13.1).
pub const FORMAT_DEPRECATED: &str =
    "⚠ --format is deprecated and will be removed in v4 — use --output";

/// The hidden `--format` value of the command, when it has one and it was given.
fn format_alias(cli: &Cli) -> Option<&str> {
    match &cli.command {
        Some(TopCommand::Runtime(crate::cli::RuntimeGroup {
            command:
                Some(RuntimeCommand::Topics(crate::cli::TopicsGroup {
                    command: Some(command),
                })),
        })) => match command {
            TopicsCommand::Create(args) => args.format.as_deref(),
            TopicsCommand::Apply(args) => args.format.as_deref(),
            TopicsCommand::Revert(args) => args.format.as_deref(),
            TopicsCommand::Eval(args) => args.format.as_deref(),
            _ => None,
        },
        _ => None,
    }
}

/// Whether the command line uses the deprecated `--format` alias.
pub fn uses_format_alias(cli: &Cli) -> bool {
    format_alias(cli).is_some()
}

pub use bulk_scan::INPUT_DEPRECATED;

/// The `runtime bulk-scan` arguments, when the command line runs it.
fn bulk_scan_args(cli: &Cli) -> Option<&crate::cli::BulkScanArgs> {
    match &cli.command {
        Some(TopCommand::Runtime(crate::cli::RuntimeGroup {
            command: Some(RuntimeCommand::BulkScan(args)),
        })) => Some(args),
        _ => None,
    }
}

/// Whether the command line uses the hidden `--input` alias of `runtime bulk-scan`.
pub fn uses_input_alias(cli: &Cli) -> bool {
    bulk_scan_args(cli).is_some_and(|args| args.input.is_some())
}

/// The `runtime dlp` group, when the command line runs it.
pub fn dlp_group(cli: &Cli) -> Option<&crate::cli::DlpGroup> {
    match &cli.command {
        Some(TopCommand::Runtime(crate::cli::RuntimeGroup {
            command: Some(RuntimeCommand::Dlp(group)),
        })) => Some(group),
        _ => None,
    }
}

/// Whether the command line runs one of the M4 runtime write commands (§13).
fn is_runtime_write(cli: &Cli) -> bool {
    match &cli.command {
        Some(TopCommand::Runtime(crate::cli::RuntimeGroup {
            command:
                Some(RuntimeCommand::Topics(crate::cli::TopicsGroup {
                    command: Some(command),
                })),
        })) => !matches!(command, TopicsCommand::List(_) | TopicsCommand::Get(_)),
        Some(TopCommand::Runtime(crate::cli::RuntimeGroup {
            command:
                Some(RuntimeCommand::Profiles(crate::cli::ProfilesGroup {
                    command: Some(command),
                })),
        })) => matches!(
            command,
            ProfilesCommand::Create(_)
                | ProfilesCommand::Update(_)
                | ProfilesCommand::Delete(_)
                | ProfilesCommand::Cleanup(_)
        ),
        _ => false,
    }
}

/// The format of an M4 command that renders data (§13.1, D94): `--format` is validated
/// like `--output`; an explicit `--output` wins, else `--format`, else `pretty`. The
/// tenant's `defaultOutput` is never consulted.
fn runtime_write_format(cli: &Cli) -> Result<Format, CliError> {
    let alias = output::validate_explicit(format_alias(cli))?;
    let explicit = output::validate_explicit(cli.global.output.as_deref())?;
    Ok(explicit.or(alias).unwrap_or(Format::Pretty))
}

/// One shared deadline for a whole walk, including its token request (DESIGN §5).
pub fn walk_options() -> Result<scm::RequestOptions, CliError> {
    scm::RequestOptions::with_timeout(crate::clients::WALK_BUDGET).map_err(|_| {
        CliError::operational("AISEC_SDK_ERROR:The request options could not be configured")
    })
}

/// [`walk_options`] without the SDK's response defaults: the one shared deadline of a
/// profile read (a page, a walk or a lookup) and of its token request (§15.25 "Response
/// defaults"). Only `SecurityProfileListResponse.ai_profiles: []` is reachable today, and
/// an absent list already reads as empty, so nothing printed changes; a profile record
/// never gains a key the server did not send.
pub fn profile_walk_options() -> Result<scm::RequestOptions, CliError> {
    walk_options().map(scm::RequestOptions::without_response_defaults)
}

/// A fully validated invocation.
pub enum Plan {
    /// A command group without a subcommand: its help on stderr, exit 2.
    GroupHelp(&'static [&'static str]),
    /// A direct scan command whose received results are then retained on the MCP
    /// connection (`--record-history CONNECTION`); the inner command's outcome stands.
    Recorded(Box<Plan>, String),
    TenantList(Format),
    TenantPath(Option<String>),
    TenantRead(Option<String>, Format),
    TenantGet {
        name: String,
        key: &'static str,
        format: Format,
    },
    Doctor(Format),
    Scan(scan::ScanInput, Format),
    ProfilesList(ListParams, Option<Format>),
    ProfilesGet(GetParams, Option<Format>),
    TopicsList(ListParams, Option<Format>),
    TopicsGet(GetParams, Option<Format>),
    /// `tenant create|set|unset|switch|delete` (§3.11).
    TenantWrite(tenant_write::WritePlan),
    TenantScanners(tenant_scanners::ScannerPlan),
    /// `runtime topics create` (§13.5).
    TopicsCreate(topic_write::CreatePlan),
    /// `runtime topics update` (§13.6).
    TopicsUpdate(topic_write::UpdatePlan),
    /// `runtime topics delete` (§13.7).
    TopicsDelete(topic_write::DeletePlan),
    /// `runtime topics apply` (§13.8).
    TopicsApply(topic_assign::ApplyPlan),
    /// `runtime topics revert` (§13.9).
    TopicsRevert(topic_assign::RevertPlan),
    /// `runtime topics eval` (§13.10).
    TopicsEval(topic_eval::EvalPlan),
    /// `runtime topics sample` (§13.11).
    TopicsSample(topic_eval::SamplePlan),
    /// `runtime profiles create` (§13.12).
    ProfilesCreate(Box<profile_write::CreatePlan>),
    /// `runtime profiles update` (§13.13).
    ProfilesUpdate(Box<profile_write::UpdatePlan>),
    /// `runtime profiles delete` (§13.14).
    ProfilesDelete(profile_write::DeletePlan),
    /// `runtime profiles cleanup` (§13.15).
    ProfilesCleanup(profile_cleanup::CleanupPlan),
    /// `runtime bulk-scan` (§14).
    BulkScan(Box<bulk_scan::BulkPlan>),
    /// `runtime resume-poll` (§14.13).
    ResumePoll(bulk_scan::ResumePlan),
    /// A `redteam` command (§15).
    RedTeam(Box<redteam::RtPlan>),
    /// A `runtime dlp` command (§16).
    Dlp(Box<dlp::DlpPlan>),
    /// A `model-security` command (§17).
    ModelSecurity(Box<modelsec::MsPlan>),
    /// An M6d runtime extras command (§18).
    RuntimeExtras(Box<runtime_extras::RxPlan>),
    /// A `gateway` command (§19).
    Gateway(Box<gateway::GwPlan>),
    /// A `supplychain` command (§20).
    Supplychain(Box<supplychain::ScPlan>),
    /// The optional runtime MCP client (contract §7).
    Mcp(Box<mcp_client::McpPlan>),
}

impl Plan {
    /// What an interrupted run reports when no SDK call said otherwise. Exhaustive on
    /// purpose: a new command (a write, M4) must decide what an interruption means.
    pub fn interrupt_context(&self) -> InterruptContext {
        match self {
            Self::Scan(..) => InterruptContext::ScanOutcomeUnknown,
            Self::Recorded(inner, _) => inner.interrupt_context(),
            Self::Mcp(plan) => plan.interrupt_context(),
            // `the change was saved` once the commit point passed (lib::execute).
            Self::TenantWrite(_) | Self::TenantScanners(_) => {
                InterruptContext::NoConfigurationChanged
            }
            // `the change was saved` once a write completed (lib::execute, §13.2).
            Self::TopicsCreate(_)
            | Self::TopicsUpdate(_)
            | Self::TopicsDelete(_)
            | Self::ProfilesCreate(_)
            | Self::ProfilesUpdate(_)
            | Self::ProfilesDelete(_)
            | Self::TopicsApply(_)
            | Self::TopicsRevert(_) => InterruptContext::NothingChanged,
            // Replaced by the deletions so far once the deletion loop starts (§13.15).
            Self::ProfilesCleanup(_) => InterruptContext::NothingChanged,
            // Replaced by the scans sent so far once the CSV is read (§13.10).
            Self::TopicsEval(_) => InterruptContext::ScanNotSubmitted,
            // `the change was saved` once the file is written.
            Self::TopicsSample(_) => InterruptContext::NoDataWritten,
            // Replaced by the job's own context once the job exists (§14.16).
            Self::BulkScan(_) => bulk_scan::nothing_submitted(),
            // Replaced by the job's own context once the job lock is held (§14.16).
            Self::ResumePoll(_) => bulk_scan::nothing_changed(),
            Self::RedTeam(plan) => plan.interrupt_context(),
            Self::Dlp(plan) => plan.interrupt_context(),
            Self::ModelSecurity(plan) => plan.interrupt_context(),
            Self::RuntimeExtras(plan) => plan.interrupt_context(),
            Self::Gateway(plan) => plan.interrupt_context(),
            Self::Supplychain(plan) => plan.interrupt_context(),
            Self::GroupHelp(_)
            | Self::TenantList(_)
            | Self::TenantPath(_)
            | Self::TenantRead(..)
            | Self::TenantGet { .. }
            | Self::Doctor(_)
            | Self::ProfilesList(..)
            | Self::ProfilesGet(..)
            | Self::TopicsList(..)
            | Self::TopicsGet(..) => InterruptContext::NoDataWritten,
        }
    }

    /// Whether `--debug` opens a log for this command (DESIGN §2.9, D63): only the
    /// commands that can send requests. Exhaustive on purpose: a new command decides.
    pub fn debug_policy(&self) -> DebugPolicy {
        match self {
            Self::RedTeam(plan) if plan.is_environment_report() => DebugPolicy::Refused,
            Self::Recorded(inner, _) => inner.debug_policy(),
            Self::Dlp(plan) => plan.debug_policy(),
            Self::ModelSecurity(plan) => plan.debug_policy(),
            Self::RuntimeExtras(plan) => plan.debug_policy(),
            Self::Gateway(plan) => plan.debug_policy(),
            Self::Supplychain(plan) => plan.debug_policy(),
            Self::Doctor(_)
            | Self::Scan(..)
            | Self::ProfilesList(..)
            | Self::ProfilesGet(..)
            | Self::TopicsList(..)
            | Self::TopicsGet(..)
            | Self::TopicsCreate(_)
            | Self::TopicsUpdate(_)
            | Self::TopicsDelete(_)
            | Self::ProfilesCreate(_)
            | Self::ProfilesUpdate(_)
            | Self::ProfilesDelete(_)
            | Self::TopicsApply(_)
            | Self::TopicsRevert(_)
            | Self::TopicsEval(_)
            | Self::ProfilesCleanup(_)
            | Self::BulkScan(_)
            | Self::ResumePoll(_)
            | Self::RedTeam(_) => DebugPolicy::Log,
            Self::TopicsSample(_)
            | Self::GroupHelp(_)
            | Self::TenantList(_)
            | Self::TenantPath(_)
            | Self::TenantRead(..)
            | Self::TenantGet { .. }
            | Self::TenantWrite(_) => DebugPolicy::None,
            Self::TenantScanners(_) => DebugPolicy::None,
            // MCP calls are not SDK traffic; the API log covers AIRS calls only.
            Self::Mcp(_) => DebugPolicy::None,
        }
    }
}

/// Validate every flag and value. Never reads the registry or the network. Every
/// command validates `--output` (DESIGN §2.4); management reads settle a missing one
/// from the tenant later.
pub fn plan(cli: &Cli) -> Result<Plan, CliError> {
    let explicit = cli.global.output.as_deref();
    if let Some(TopCommand::Tenant(crate::cli::TenantGroup {
        command: Some(TenantCommand::Scanners(group)),
    })) = &cli.command
    {
        let format = output::resolve(explicit)?;
        if cli.global.tenant.is_some() {
            return Err(CliError::usage(
                "--tenant cannot be used with 'tenant scanners'; name the tenant as an argument",
            ));
        }
        return tenant_scanners::plan(group, format);
    }
    // The write commands name their tenant positionally: `--output` is validated first,
    // then any `--tenant` is refused, then the command's own checks run (§3.11).
    if let Some(TopCommand::Tenant(crate::cli::TenantGroup {
        command: Some(command),
    })) = &cli.command
        && let Some(subcommand) = tenant_write::write_command_name(command)
    {
        output::validate_explicit(explicit)?;
        if cli.global.tenant.is_some() {
            return Err(CliError::usage(format!(
                "--tenant cannot be used with 'tenant {subcommand}'; name the tenant as an argument"
            )));
        }
        return Ok(Plan::TenantWrite(tenant_write::plan(command)?));
    }
    // The M4 commands check `--format` and `--output` before `--tenant` (§13.2).
    if is_runtime_write(cli) {
        runtime_write_format(cli)?;
    }
    // So do `runtime bulk-scan` and `runtime resume-poll` (§14.2 step 3).
    if bulk_scan_args(cli).is_some()
        || matches!(
            &cli.command,
            Some(TopCommand::Runtime(crate::cli::RuntimeGroup {
                command: Some(RuntimeCommand::ResumePoll(_)),
            }))
        )
    {
        output::validate_explicit(explicit)?;
    }
    // So does every red team command (§15.3 "Validation order"), every DLP command
    // (§16.3), every model-security command (§17.3), every gateway command (§19.3) and
    // every supplychain command (§20.3).
    if matches!(
        &cli.command,
        Some(
            TopCommand::Redteam(_)
                | TopCommand::ModelSecurity(_)
                | TopCommand::Gateway(_)
                | TopCommand::Supplychain(_)
        )
    ) || dlp_group(cli).is_some()
        || matches!(
            &cli.command,
            Some(TopCommand::Runtime(crate::cli::RuntimeGroup {
                command: Some(command),
            })) if runtime_extras::is_runtime_extras(command)
        )
    {
        output::validate_explicit(explicit)?;
    }
    registry::validate_tenant_flag(cli.global.tenant.as_deref())?;
    let Some(command) = &cli.command else {
        return Ok(Plan::GroupHelp(&[]));
    };
    Ok(match command {
        TopCommand::Tenant(group) => match &group.command {
            None => Plan::GroupHelp(&["tenant"]),
            Some(
                TenantCommand::Create(_)
                | TenantCommand::Set(_)
                | TenantCommand::Unset { .. }
                | TenantCommand::Switch { .. }
                | TenantCommand::Delete { .. },
            ) => unreachable!("write commands are planned above"),
            Some(TenantCommand::Scanners(_)) => unreachable!("scanner commands are planned above"),
            Some(TenantCommand::List) => Plan::TenantList(output::resolve(explicit)?),
            // The path is printed in every format; an invalid name is still rejected.
            Some(TenantCommand::Path { name }) => {
                output::validate_explicit(explicit)?;
                Plan::TenantPath(name.clone())
            }
            Some(TenantCommand::Read { name }) => {
                Plan::TenantRead(name.clone(), output::resolve(explicit)?)
            }
            Some(TenantCommand::Get { name, key }) => {
                // The key is validated before the format (D34 order).
                let key = crate::tenant::config::read_schema_key(key).ok_or_else(|| {
                    CliError::usage(format!(
                        "Unknown configuration key. Supported keys: {}",
                        crate::tenant::config::KEYS.join(", ")
                    ))
                })?;
                Plan::TenantGet {
                    name: name.clone(),
                    key,
                    format: output::resolve(explicit)?,
                }
            }
        },
        TopCommand::Doctor => Plan::Doctor(output::resolve(explicit)?),
        TopCommand::Runtime(group) => match &group.command {
            None => Plan::GroupHelp(&["runtime"]),
            Some(RuntimeCommand::Scan(args)) => {
                let format = output::resolve(explicit)?;
                recorded(
                    Plan::Scan(scan::validate_input(args)?, format),
                    &args.record_history,
                    args.scanner.is_some(),
                )?
            }
            Some(RuntimeCommand::BulkScan(args)) => {
                let format = output::resolve(explicit)?;
                recorded(
                    Plan::BulkScan(Box::new(bulk_scan::plan(args, format)?)),
                    &args.record_history,
                    args.scanner.is_some(),
                )?
            }
            Some(RuntimeCommand::ResumePoll(args)) => {
                let format = output::resolve(explicit)?;
                recorded(
                    Plan::ResumePoll(bulk_scan::plan_resume(args, format)?),
                    &args.record_history,
                    true,
                )?
            }
            Some(RuntimeCommand::Receipts(group)) => {
                mcp_client::plan_receipts(group, output::resolve(explicit)?)?
            }
            Some(RuntimeCommand::Report(args)) if args.mcp.is_some() => mcp_client::plan_report(
                args,
                args.mcp.as_deref().unwrap_or_default(),
                output::resolve(explicit)?,
            )?,
            Some(RuntimeCommand::Dlp(group)) => dlp::plan(group, explicit)?,
            Some(RuntimeCommand::History(group)) => {
                mcp_client::plan_history(group, output::resolve(explicit)?)?
            }
            Some(RuntimeCommand::Jobs(group)) => {
                mcp_client::plan_jobs(group, output::resolve(explicit)?)?
            }
            Some(RuntimeCommand::Schedules(group)) => {
                mcp_client::plan_schedules(group, output::resolve(explicit)?)?
            }
            Some(RuntimeCommand::Sync(args)) => {
                mcp_client::plan_sync(args, output::resolve(explicit)?)?
            }
            Some(
                command @ (RuntimeCommand::ApiKeys(_)
                | RuntimeCommand::CustomerApps(_)
                | RuntimeCommand::Dashboard(_)
                | RuntimeCommand::DeploymentProfiles(_)
                | RuntimeCommand::Report(_)
                | RuntimeCommand::ScanLogs(_)
                | RuntimeCommand::Sessions(_)),
            ) => runtime_extras::plan(command, explicit, cli.global.debug)?,
            Some(runtime_command @ RuntimeCommand::Profiles(profiles)) => match &profiles.command {
                None => Plan::GroupHelp(&["runtime", "profiles"]),
                Some(ProfilesCommand::Backup(_) | ProfilesCommand::Restore(_)) => {
                    runtime_extras::plan(runtime_command, explicit, cli.global.debug)?
                }
                Some(ProfilesCommand::List(args)) => {
                    let format = output::validate_explicit(explicit)?;
                    Plan::ProfilesList(
                        pagination::parse_list(&args.paging, args.all_versions)?,
                        format,
                    )
                }
                Some(ProfilesCommand::Get(args)) => {
                    let format = output::validate_explicit(explicit)?;
                    Plan::ProfilesGet(pagination::parse_get(&args.lookup)?, format)
                }
                // 7.3.0 resolves these two like reads: the tenant's `defaultOutput` applies
                // (§13.17).
                Some(ProfilesCommand::Create(args)) => {
                    let format = output::validate_explicit(explicit)?;
                    Plan::ProfilesCreate(Box::new(profile_write::plan_create(args, format)?))
                }
                Some(ProfilesCommand::Update(args)) => {
                    let format = output::validate_explicit(explicit)?;
                    Plan::ProfilesUpdate(Box::new(profile_write::plan_update(args, format)?))
                }
                Some(ProfilesCommand::Delete(args)) => {
                    output::validate_explicit(explicit)?;
                    Plan::ProfilesDelete(profile_write::plan_delete(
                        args,
                        crate::input::Terminals::detect().interactive(),
                    )?)
                }
                Some(ProfilesCommand::Cleanup(args)) => {
                    let format = runtime_write_format(cli)?;
                    Plan::ProfilesCleanup(profile_cleanup::plan_cleanup(
                        args,
                        format,
                        crate::input::Terminals::detect().interactive(),
                    )?)
                }
            },
            Some(RuntimeCommand::Topics(topics)) => match &topics.command {
                None => Plan::GroupHelp(&["runtime", "topics"]),
                Some(TopicsCommand::List(args)) => {
                    let format = output::validate_explicit(explicit)?;
                    Plan::TopicsList(
                        pagination::parse_list(&args.paging, args.all_versions)?,
                        format,
                    )
                }
                Some(TopicsCommand::Get(args)) => {
                    let format = output::validate_explicit(explicit)?;
                    Plan::TopicsGet(pagination::parse_get(&args.lookup)?, format)
                }
                Some(TopicsCommand::Create(args)) => {
                    let format = runtime_write_format(cli)?;
                    Plan::TopicsCreate(topic_write::plan_create(args, format)?)
                }
                Some(TopicsCommand::Update(args)) => {
                    let format = runtime_write_format(cli)?;
                    Plan::TopicsUpdate(topic_write::plan_update(args, format)?)
                }
                Some(TopicsCommand::Delete(args)) => {
                    output::validate_explicit(explicit)?;
                    Plan::TopicsDelete(topic_write::plan_delete(
                        args,
                        crate::input::Terminals::detect().interactive(),
                    )?)
                }
                Some(TopicsCommand::Apply(args)) => {
                    let format = runtime_write_format(cli)?;
                    Plan::TopicsApply(topic_assign::plan_apply(args, format)?)
                }
                Some(TopicsCommand::Revert(args)) => {
                    let format = runtime_write_format(cli)?;
                    Plan::TopicsRevert(topic_assign::plan_revert(
                        args,
                        format,
                        crate::input::Terminals::detect().interactive(),
                    )?)
                }
                Some(TopicsCommand::Eval(args)) => {
                    let format = runtime_write_format(cli)?;
                    Plan::TopicsEval(topic_eval::plan_eval(args, format)?)
                }
                Some(TopicsCommand::Sample(args)) => {
                    output::validate_explicit(explicit)?;
                    Plan::TopicsSample(topic_eval::plan_sample(args))
                }
            },
        },
        TopCommand::Redteam(group) => redteam::plan(group, explicit, cli.global.debug)?,
        TopCommand::ModelSecurity(group) => modelsec::plan(group, explicit)?,
        TopCommand::Gateway(group) => gateway::plan(group, explicit, cli.global.debug)?,
        TopCommand::Supplychain(group) => supplychain::plan(group, explicit, cli.global.debug)?,
        TopCommand::Mcp(group) => mcp_client::plan(group, output::resolve(explicit)?)?,
    })
}

/// `--record-history CONNECTION` (contract §7.2): a connection name, and a named
/// scanner whose association attributes the receipts (resume-poll reads the saved
/// one); validated before any tenant access.
fn recorded(plan: Plan, connection: &Option<String>, named: bool) -> Result<Plan, CliError> {
    let Some(name) = connection else {
        return Ok(plan);
    };
    if !airs_runtime_core::tenant::mcp::valid_name(name) {
        return Err(CliError::usage(
            "--record-history must name an MCP connection (1-63 of [a-z0-9-])",
        ));
    }
    if !named {
        return Err(CliError::usage(
            "--record-history needs a named scanner (--scanner NAME): receipts are attributed by its association",
        ));
    }
    Ok(Plan::Recorded(Box::new(plan), name.clone()))
}

/// Run a validated plan (everything except `GroupHelp`, which `lib` renders).
pub async fn dispatch(invocation: &mut Invocation, plan: Plan) -> Result<(), CliError> {
    match plan {
        Plan::GroupHelp(_) => Ok(()),
        Plan::Recorded(inner, connection) => {
            let outcome = Box::pin(dispatch(invocation, *inner)).await;
            mcp_ops::record_history(invocation, &connection).await;
            outcome
        }
        Plan::TenantList(format) => tenant::list(invocation, format),
        Plan::TenantPath(name) => tenant::path(invocation, name.as_deref()),
        Plan::TenantRead(name, format) => tenant::read(invocation, name.as_deref(), format),
        Plan::TenantGet { name, key, format } => tenant::get(invocation, &name, key, format),
        Plan::Doctor(format) => doctor::run(invocation, format).await,
        Plan::Scan(input, format) => scan::run(invocation, &input, format).await,
        Plan::ProfilesList(params, format) => profiles::list(invocation, &params, format).await,
        Plan::ProfilesGet(params, format) => profiles::get(invocation, &params, format).await,
        Plan::TopicsList(params, format) => topics::list(invocation, &params, format).await,
        Plan::TopicsGet(params, format) => topics::get(invocation, &params, format).await,
        Plan::TenantWrite(write) => tenant_write::run(invocation, write),
        Plan::TenantScanners(plan) => tenant_scanners::run(invocation, plan),
        Plan::Mcp(plan) => mcp_client::run(invocation, *plan).await,
        Plan::TopicsCreate(plan) => topic_write::create(invocation, &plan).await,
        Plan::TopicsUpdate(plan) => topic_write::update(invocation, &plan).await,
        Plan::TopicsDelete(plan) => topic_write::delete(invocation, &plan).await,
        Plan::TopicsApply(plan) => topic_assign::apply(invocation, &plan).await,
        Plan::TopicsRevert(plan) => topic_assign::revert(invocation, &plan).await,
        Plan::TopicsEval(plan) => topic_eval::eval(invocation, &plan).await,
        Plan::TopicsSample(plan) => topic_eval::sample(invocation, &plan),
        Plan::ProfilesCreate(plan) => profile_write::create(invocation, &plan).await,
        Plan::ProfilesUpdate(plan) => profile_write::update(invocation, &plan).await,
        Plan::ProfilesDelete(plan) => profile_write::delete(invocation, &plan).await,
        Plan::ProfilesCleanup(plan) => profile_cleanup::cleanup(invocation, &plan).await,
        Plan::BulkScan(plan) => bulk_scan::run(invocation, &plan).await,
        Plan::ResumePoll(plan) => bulk_scan::resume(invocation, &plan).await,
        Plan::RedTeam(plan) => redteam::dispatch(invocation, *plan).await,
        Plan::Dlp(plan) => dlp::dispatch(invocation, *plan).await,
        Plan::ModelSecurity(plan) => modelsec::dispatch(invocation, *plan).await,
        Plan::RuntimeExtras(plan) => runtime_extras::dispatch(invocation, *plan).await,
        Plan::Gateway(plan) => gateway::dispatch(invocation, *plan).await,
        Plan::Supplychain(plan) => supplychain::dispatch(invocation, *plan).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::FromArgMatches;

    fn plan_of(args: &[&str]) -> Result<Plan, CliError> {
        let matches = crate::cli::command().try_get_matches_from(args).unwrap();
        plan(&Cli::from_arg_matches(&matches).unwrap())
    }

    fn usage(args: &[&str]) -> String {
        match plan_of(args) {
            Err(CliError::Usage(message)) => message,
            Err(other) => panic!("expected usage error, got {other:?}"),
            Ok(_) => panic!("expected usage error"),
        }
    }

    #[test]
    fn usage_errors_need_no_tenant() {
        assert_eq!(
            usage(&["airs", "runtime", "profiles", "list", "--output", "xml"]),
            "Invalid output format 'xml'. Expected: pretty, table, json"
        );
        assert_eq!(
            usage(&["airs", "doctor", "--output", "yaml"]),
            "Invalid output format 'yaml'. Expected: pretty, table, json"
        );
        assert_eq!(
            usage(&["airs", "tenant", "path", "--output", "csv"]),
            "Invalid output format 'csv'. Expected: pretty, table, json"
        );
        assert_eq!(
            usage(&["airs", "runtime", "topics", "list", "--limit", "0"]),
            "--limit must be a positive integer"
        );
        assert_eq!(
            usage(&[
                "airs",
                "runtime",
                "profiles",
                "get",
                "x",
                "--revision",
                "abc"
            ]),
            "--revision must be a non-negative integer"
        );
        assert!(
            usage(&["airs", "tenant", "get", "dev", "nope"]).starts_with(
                "Unknown configuration key. Supported keys: airsApiKey, airsApiToken,"
            )
        );
        assert!(usage(&["airs", "--tenant", "-x", "doctor"]).starts_with("Tenant name must be"));
        assert_eq!(
            usage(&["airs", "runtime", "scan", "--profile", "p", ""]),
            "AISEC_USER_REQUEST_PAYLOAD_ERROR:At least one of prompt, response, codePrompt, codeResponse, or toolEvent must be provided"
        );
    }

    #[test]
    fn unknown_key_wins_over_output_and_path_ignores_a_valid_output() {
        assert!(
            usage(&["airs", "tenant", "get", "dev", "nope", "--output", "xml"])
                .starts_with("Unknown configuration key")
        );
        assert!(matches!(
            plan_of(&["airs", "tenant", "path", "--output", "table"]),
            Ok(Plan::TenantPath(None))
        ));
    }

    #[test]
    fn write_commands_validate_output_then_refuse_tenant() {
        assert_eq!(
            usage(&[
                "airs", "--tenant", "-x", "tenant", "switch", "dev", "--output", "xml"
            ]),
            "Invalid output format 'xml'. Expected: pretty, table, json"
        );
        for (sub, args) in [
            ("create", &["create", "dev"][..]),
            ("set", &["set", "dev", "defaultOutput", "json"]),
            ("unset", &["unset", "dev", "defaultOutput"]),
            ("switch", &["switch", "dev"]),
            ("delete", &["rm", "dev"]),
        ] {
            let mut full = vec!["airs", "--tenant", "-x", "tenant"];
            full.extend_from_slice(args);
            assert_eq!(
                usage(&full),
                format!(
                    "--tenant cannot be used with 'tenant {sub}'; name the tenant as an argument"
                )
            );
        }
        assert_eq!(
            usage(&["airs", "--tenant", "dev", "tenant", "create", "_bad"]),
            "--tenant cannot be used with 'tenant create'; name the tenant as an argument",
            "before the name check"
        );
        assert_eq!(
            usage(&["airs", "tenant", "create", "_bad", "--output", "table"]),
            "Tenant name must be 1–64 letters, digits, hyphens or underscores and start with a letter/digit"
        );
        assert!(matches!(
            plan_of(&["airs", "tenant", "switch", "dev", "--output", "json"]),
            Ok(Plan::TenantWrite(tenant_write::WritePlan::Switch { .. }))
        ));
        assert_eq!(
            plan_of(&["airs", "tenant", "switch", "dev"])
                .ok()
                .unwrap()
                .interrupt_context(),
            InterruptContext::NoConfigurationChanged
        );
    }

    #[test]
    fn runtime_writes_check_formats_before_the_tenant_flag_and_ignore_the_default() {
        let create = [
            "airs",
            "--tenant",
            "-bad",
            "runtime",
            "topics",
            "create",
            "--name",
            "n",
            "--description",
            "d",
            "--examples",
            "a",
            "b",
        ];
        let mut with_output = create.to_vec();
        with_output.extend(["--output", "xml"]);
        assert_eq!(
            usage(&with_output),
            "Invalid output format 'xml'. Expected: pretty, table, json"
        );
        let mut with_format = create.to_vec();
        with_format.extend(["--format", "csv"]);
        assert_eq!(
            usage(&with_format),
            "Invalid output format 'csv'. Expected: pretty, table, json"
        );
        assert!(usage(&create).starts_with("Tenant name must be"));
        let plain = &create[3..];
        let mut args = vec!["airs"];
        args.extend_from_slice(plain);
        args.extend(["--format", "json", "--output", "table"]);
        assert!(matches!(
            plan_of(&args),
            Ok(Plan::TopicsCreate(topic_write::CreatePlan {
                format: Format::Table,
                ..
            }))
        ));
        let mut args = vec!["airs"];
        args.extend_from_slice(plain);
        let plan = plan_of(&args).ok().unwrap();
        assert!(matches!(
            &plan,
            Plan::TopicsCreate(topic_write::CreatePlan {
                format: Format::Pretty,
                ..
            })
        ));
        assert_eq!(plan.interrupt_context(), InterruptContext::NothingChanged);
        assert_eq!(plan.debug_policy(), DebugPolicy::Log);
        assert_eq!(
            usage(&[
                "airs",
                "runtime",
                "profiles",
                "delete",
                "p",
                "--force",
                "--updated-by",
                "a@b",
                "--output",
                "yaml"
            ]),
            "Invalid output format 'yaml'. Expected: pretty, table, json"
        );
    }

    #[test]
    fn topic_assign_and_eval_plans_check_formats_first() {
        let apply = [
            "airs",
            "--tenant",
            "-bad",
            "runtime",
            "topics",
            "apply",
            "--profile",
            "p",
            "--name",
            "n",
            "--intent",
            "deny",
            "--format",
            "xml",
        ];
        assert_eq!(
            usage(&apply),
            "Invalid output format 'xml'. Expected: pretty, table, json"
        );
        assert!(usage(&apply[..12]).starts_with("Tenant name must be"));
        let mut eval = vec![
            "airs",
            "runtime",
            "topics",
            "eval",
            "--profile",
            "p",
            "--prompts",
            "x.csv",
            "--format",
            "json",
        ];
        let plan = plan_of(&eval).ok().unwrap();
        assert!(matches!(
            &plan,
            Plan::TopicsEval(topic_eval::EvalPlan {
                format: Format::Json,
                concurrency: 5,
                rate: None,
                ..
            })
        ));
        assert_eq!(plan.interrupt_context(), InterruptContext::ScanNotSubmitted);
        eval.extend(["--output", "table"]);
        assert!(matches!(
            plan_of(&eval),
            Ok(Plan::TopicsEval(topic_eval::EvalPlan {
                format: Format::Table,
                ..
            }))
        ));
        let sample = plan_of(&["airs", "runtime", "topics", "sample", "--output", "json"]);
        assert!(matches!(
            &sample,
            Ok(Plan::TopicsSample(topic_eval::SamplePlan {
                output_file: None
            }))
        ));
        assert_eq!(
            sample.ok().unwrap().interrupt_context(),
            InterruptContext::NoDataWritten
        );
    }

    #[test]
    fn groups_without_subcommands_render_help() {
        for (args, path) in [
            (&["airs"][..], &[][..]),
            (&["airs", "--quiet"], &[]),
            (&["airs", "runtime"], &["runtime"]),
            (&["airs", "runtime", "topics"], &["runtime", "topics"]),
            (&["airs", "tenant"], &["tenant"]),
        ] {
            match plan_of(args) {
                Ok(Plan::GroupHelp(found)) => assert_eq!(found, path),
                _ => panic!("expected group help for {args:?}"),
            }
        }
    }

    #[test]
    fn debug_policy_logs_only_commands_that_send_requests() {
        let policy = |args: &[&str]| plan_of(args).ok().unwrap().debug_policy();
        for args in [
            &["airs", "doctor"][..],
            &["airs", "runtime", "scan", "--profile", "p", "hi"],
            &["airs", "runtime", "profiles", "list"],
            &["airs", "runtime", "profiles", "get", "x"],
            &["airs", "runtime", "topics", "ls"],
            &["airs", "runtime", "topics", "get", "x"],
            &[
                "airs",
                "runtime",
                "topics",
                "apply",
                "--profile",
                "p",
                "--name",
                "n",
            ],
            &[
                "airs",
                "runtime",
                "topics",
                "revert",
                "--profile",
                "p",
                "--name",
                "n",
                "--force",
            ],
            &[
                "airs",
                "runtime",
                "topics",
                "eval",
                "--profile",
                "p",
                "--prompts",
                "p.csv",
            ],
        ] {
            assert_eq!(policy(args), DebugPolicy::Log, "{args:?}");
        }
        for args in [
            &["airs", "tenant", "list"][..],
            &["airs", "tenant", "switch", "dev"],
            &["airs", "tenant", "delete", "dev"],
            &["airs", "tenant", "create", "dev"],
            &["airs", "tenant", "path"],
            &["airs", "tenant", "read"],
            &["airs", "tenant", "get", "dev", "mgmtTsgId"],
            &["airs", "runtime"],
            &["airs", "runtime", "topics", "sample"],
        ] {
            assert_eq!(policy(args), DebugPolicy::None, "{args:?}");
        }
    }

    #[test]
    fn valid_plans() {
        assert!(matches!(
            plan_of(&[
                "airs", "runtime", "profiles", "ls", "--output", "json", "--all"
            ]),
            Ok(Plan::ProfilesList(
                ListParams { all: true, .. },
                Some(Format::Json)
            ))
        ));
        assert!(matches!(
            plan_of(&["airs", "tenant", "get", "dev", "mgmtTsgId"]),
            Ok(Plan::TenantGet {
                key: "mgmtTsgId",
                format: Format::Pretty,
                ..
            })
        ));
        assert!(matches!(
            plan_of(&["airs", "--output", "json", "doctor"]),
            Ok(Plan::Doctor(Format::Json))
        ));
        assert!(matches!(
            plan_of(&["airs", "tenant", "list"]),
            Ok(Plan::TenantList(Format::Pretty))
        ));
        assert!(matches!(
            plan_of(&["airs", "runtime", "topics", "get", "x"]),
            Ok(Plan::TopicsGet(_, None))
        ));
        assert!(matches!(
            plan_of(&[
                "airs",
                "runtime",
                "scan",
                "--profile",
                "p",
                "hi",
                "--output",
                "table"
            ]),
            Ok(Plan::Scan(_, Format::Table))
        ));
    }

    /// The call-site half of §15.25 point 4, which no output can show while no profile
    /// default is reachable: in the profile commands no profile read or write passes
    /// `None` or the defaulted `walk_options()` (deletes keep `None`, their answers unused;
    /// `topics apply|revert` keep `walk_options()` for the topic walk only). A line-based
    /// heuristic over the four profile command files: a call whose `None` rustfmt moved to
    /// its own line, or another module (backup, restore and report build their options
    /// through `runtime::options`), is left to review.
    #[test]
    fn profile_call_sites_use_the_no_default_options() {
        const CALLS: [&str; 6] = [
            ".list(",
            ".list_all(",
            ".get(",
            ".get_by_name(",
            ".create(",
            ".update(",
        ];
        let bare = |text: &str| {
            text.matches("walk_options()").count() - text.matches("profile_walk_options()").count()
        };
        for (file, text) in [
            ("profiles.rs", include_str!("profiles.rs")),
            ("profile_write.rs", include_str!("profile_write.rs")),
            ("topic_assign.rs", include_str!("topic_assign.rs")),
            ("profile_cleanup.rs", include_str!("profile_cleanup.rs")),
        ] {
            for line in text.lines() {
                let call = CALLS.iter().any(|call| line.contains(call));
                assert!(!(call && line.contains("None)")), "{file}: {line}");
            }
            let expected = match file {
                // The topic walks of apply and revert.
                "topic_assign.rs" => 2,
                // The token of the run; each listing builds its own no-default deadline.
                "profile_cleanup.rs" => 1,
                _ => 0,
            };
            assert_eq!(bare(text), expected, "{file}: a defaulted walk_options()");
        }
        assert_eq!(
            include_str!("topic_assign.rs")
                .matches("profile_walk_options()?")
                .count(),
            2,
            "the profile reads of apply and revert"
        );
    }

    /// scm-sdk 0.2.4 (DESIGN §15.25 "Response defaults"): every profile read and every
    /// profile write runs without the SDK's response defaults;
    /// topic walks keep them (G27, TypeScript's `examples: []`). The SDK half: the option
    /// is what keeps an absent `ai_profiles` absent, and no default is reachable from a
    /// profile record, so a directional record reads the same in both modes.
    #[test]
    fn profile_reads_and_writes_skip_the_response_defaults() {
        use scm::airs::management::models::{SecurityProfile, SecurityProfileListResponse};
        use scm::validation::Mode;
        assert!(!profile_walk_options().unwrap().response_defaults());
        assert!(walk_options().unwrap().response_defaults());
        assert!(
            !crate::runtime::request_options(crate::clients::SINGLE_REQUEST_BUDGET)
                .unwrap()
                .response_defaults()
        );
        let empty = serde_json::json!({});
        let defaulted = SecurityProfileListResponse::parse(empty.clone(), Mode::Response).unwrap();
        let sent =
            SecurityProfileListResponse::parse(empty, Mode::ResponseWithoutDefaults).unwrap();
        assert!(!defaulted.ai_profiles.is_unset());
        assert!(sent.ai_profiles.is_unset());
        let mut record = airs_test_harness::fixtures::runtime::directional_profile();
        record["policy"]["ai-security-profiles"][0]["content-type-configurations"]["audio"] =
            serde_json::json!({"future": null});
        record["policy"]["ai-security-profiles"][0]["model-configuration"]["enable-full-conversation-inspection"] =
            serde_json::Value::Null;
        record["extra_top_level"] = serde_json::json!("kept");
        for mode in [Mode::Response, Mode::ResponseWithoutDefaults] {
            let parsed = SecurityProfile::parse(record.clone(), mode).unwrap();
            assert_eq!(serde_json::to_value(&parsed).unwrap(), record, "{mode:?}");
        }
    }
}
