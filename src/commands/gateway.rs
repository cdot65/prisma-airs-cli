//! `gateway` planning and dispatch (DESIGN §19). Planning runs every argument check of
//! §19.3 before the tenant is read (D232); dispatch connects the gateway client from the
//! tenant's data, admin and IAM endpoints (`clients::GatewayClients`) and runs the
//! command.
//!
//! Chunk 1 (§19.1) plans the workspace and scope reads and every telemetry read; chunk 2
//! the inventory and admin-guardrail reads (`gateway_inventory`); chunk 3 the writes
//! (`gateway_writes`, `gateway_provision`); chunk 4 inference (`gateway_inference`) and
//! the report (`gateway_report`).

use super::{Invocation, Plan};
use crate::cli::{
    GatewayCommand, GatewayGroup, GwCacheCommand, GwChartFilterArgs, GwFeedbackCommand,
    GwLogsCommand, GwScopesCommand, GwTelemetryCommand, GwWindowArgs, GwWorkspacesCommand,
};
use crate::clients::{GatewayClients, SINGLE_REQUEST_BUDGET, TOKEN_BUDGET};
use crate::error::{CliError, InterruptContext};
use crate::gateway::resolve::ListStatus;
use crate::gateway::window::{self, Dimension, FilterFlags, Filters, LogsPaging, Window};
use crate::gateway::{self, GwPlane};
use crate::output::{self, Format};
use scm::airs::gateway::{AIGatewayClient, AIGatewayGroupColumn};

/// A window-only telemetry chart (`AIGatewayWindowOptions`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chart {
    Errors,
    Users,
    ErrorTrends,
    ErrorCategoryTrends,
    GroupedErrors,
    RescuedRetries,
    UserTrends,
    CacheSummary,
    CacheTrend,
    FeedbackDistribution,
    FeedbackModels,
    FeedbackTrend,
    FeedbackWeighted,
}

/// A filtered telemetry chart (`AIGatewayChartOptions`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilteredChart {
    Latency,
    Requests,
    Tokens,
}

/// One telemetry read other than `cost` (§19.8).
#[derive(Clone, Debug, PartialEq)]
pub enum TelemetryRead {
    Chart(Chart),
    Filtered(FilteredChart, Filters),
    FilterBoundaries,
    GroupBy {
        dimension: Dimension,
        columns: Option<Vec<AIGatewayGroupColumn>>,
        filters: Filters,
    },
    Logs(LogsPaging),
}

/// `workspaces list` (§19.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceList {
    /// One list: the plane and an explicit `--status`.
    One {
        plane: GwPlane,
        status: Option<ListStatus>,
    },
    /// `--all`: admin active, then admin archived (choice 15).
    All,
}

/// A validated `gateway` invocation; `format` is the explicit `--output`.
#[derive(Clone, Debug, PartialEq)]
pub enum GwPlan {
    WorkspacesList {
        list: WorkspaceList,
        format: Option<Format>,
    },
    WorkspacesGet {
        reference: String,
        plane: GwPlane,
        format: Option<Format>,
    },
    ScopesList {
        format: Option<Format>,
    },
    ScopesGet {
        name: String,
        format: Option<Format>,
    },
    Cost {
        workspace: String,
        days: u64,
        window: Window,
        filters: Filters,
        format: Option<Format>,
    },
    Telemetry {
        read: TelemetryRead,
        workspace: String,
        window: Window,
        format: Option<Format>,
    },
    /// An inventory or admin-guardrail read (§19.9, §19.13; chunk 2).
    Inventory {
        read: crate::gateway::inventory::InventoryRead,
        format: Option<Format>,
    },
    /// A write of §19.10–§19.13 (chunk 3).
    Write(Box<super::gateway_write_plan::WritePlan>),
    /// `workspaces create` (§19.6).
    WorkspaceCreate(Box<super::gateway_provision::ProvisionPlan>),
    /// `workspaces update <ref>` (§19.6).
    WorkspaceUpdate(Box<super::gateway_provision::UpdatePlan>),
    /// `workspaces archive <ref>`, hidden `delete <ref>` (§19.6).
    WorkspaceArchive {
        reference: String,
        force: bool,
        deprecated: bool,
    },
    /// `scopes create` (§19.7).
    ScopeCreate {
        name: String,
        description: Option<String>,
        format: Option<Format>,
    },
    /// `scopes bind <name> --workspace <ref>` (§19.7).
    ScopeBind {
        name: String,
        workspace: String,
        format: Option<Format>,
    },
    /// `scopes delete|rm <name>` (§19.7).
    ScopeDelete {
        name: String,
        force: bool,
    },
    /// `inference chat|responses|embeddings` (§19.14; chunk 4).
    Inference(Box<super::gateway_inference::InferencePlan>),
    /// `report|dashboard` (§19.15; chunk 4).
    Report(Box<super::gateway_report::GatewayReportPlan>),
}

impl GwPlan {
    /// What an interruption reports (§19.3 "Interruption"): the reads write no data; a
    /// write changed nothing until it is sent (its own contexts take over then).
    pub fn interrupt_context(&self) -> InterruptContext {
        match self {
            Self::Write(_)
            | Self::WorkspaceCreate(_)
            | Self::WorkspaceUpdate(_)
            | Self::WorkspaceArchive { .. }
            | Self::ScopeCreate { .. }
            | Self::ScopeBind { .. }
            | Self::ScopeDelete { .. } => InterruptContext::NothingChanged,
            Self::Inference(_) => {
                InterruptContext::Context(super::gateway_inference::NOTHING_SENT.to_owned())
            }
            Self::Report(_) => {
                InterruptContext::Context(super::report_command::NO_REPORT.to_owned())
            }
            _ => InterruptContext::NoDataWritten,
        }
    }

    /// The `--debug` policy (§19.3 "Debug log"): every command that sends requests logs
    /// status and headers only (D64); the report refuses `--debug` during planning (D156).
    pub fn debug_policy(&self) -> crate::debug::DebugPolicy {
        match self {
            Self::Report(_) => crate::debug::DebugPolicy::Refused,
            _ => crate::debug::DebugPolicy::Log,
        }
    }
}

/// `--plane data|admin` (TS text), `data` by default.
fn plane(value: Option<&String>) -> Result<Option<GwPlane>, CliError> {
    match value.map(String::as_str) {
        None => Ok(None),
        Some("data") => Ok(Some(GwPlane::Data)),
        Some("admin") => Ok(Some(GwPlane::Admin)),
        Some(other) => Err(CliError::usage(format!(
            "Invalid --plane '{}'. Valid planes: data, admin",
            gateway::shown(other)
        ))),
    }
}

/// `--status active|archived` (TS text).
fn status(value: Option<&String>) -> Result<Option<ListStatus>, CliError> {
    match value.map(String::as_str) {
        None => Ok(None),
        Some("active") => Ok(Some(ListStatus::Active)),
        Some("archived") => Ok(Some(ListStatus::Archived)),
        Some(other) => Err(CliError::usage(format!(
            "Invalid --status '{}'. Valid statuses: active, archived",
            gateway::shown(other)
        ))),
    }
}

fn filter_flags(args: &GwChartFilterArgs) -> FilterFlags<'_> {
    FilterFlags {
        trace_id: args.trace_id.as_deref(),
        metadata: args.metadata.as_deref(),
        status_codes: args.status_codes.as_deref(),
        api_key_ids: args.api_key_ids.as_deref(),
        ai_org_models: args.ai_org_models.as_deref(),
        total_units_min: args.total_units_min.as_deref(),
        total_units_max: args.total_units_max.as_deref(),
        cost_min: args.cost_min.as_deref(),
        cost_max: args.cost_max.as_deref(),
    }
}

/// The window of a leaf, fixed at the time of the check (§19.8).
fn leaf_window(args: &GwWindowArgs) -> Result<Window, CliError> {
    window::window(
        args.days.as_deref(),
        args.start.as_deref(),
        args.end.as_deref(),
        window::now_ms(),
    )
}

/// A telemetry plan: the leaf's own checks have run; `--workspace` is checked last.
fn telemetry(
    read: TelemetryRead,
    window: Window,
    args: &GwWindowArgs,
    format: Option<Format>,
) -> Result<GwPlan, CliError> {
    gateway::check_resolved_ref("--workspace", &args.workspace)?;
    Ok(GwPlan::Telemetry {
        read,
        workspace: args.workspace.clone(),
        window,
        format,
    })
}

/// A window-only chart leaf.
fn chart(chart: Chart, args: &GwWindowArgs, format: Option<Format>) -> Result<GwPlan, CliError> {
    let window = leaf_window(args)?;
    telemetry(TelemetryRead::Chart(chart), window, args, format)
}

/// Plan a `gateway` command line; `--output` and `--tenant` were validated already
/// (§19.3 "Validation order").
pub fn plan(group: &GatewayGroup, explicit: Option<&str>, debug: bool) -> Result<Plan, CliError> {
    let format = output::validate_explicit(explicit)?;
    let Some(command) = &group.command else {
        return Ok(Plan::GroupHelp(&["gateway"]));
    };
    match command {
        GatewayCommand::Report(args) => {
            let plan = super::gateway_report::plan(args, format, debug, window::now_ms())?;
            return Ok(Plan::Gateway(Box::new(GwPlan::Report(Box::new(plan)))));
        }
        GatewayCommand::Inference(group) => {
            let Some(command) = &group.command else {
                return Ok(Plan::GroupHelp(&["gateway", "inference"]));
            };
            let plan = super::gateway_inference::plan(command, format)?;
            return Ok(Plan::Gateway(Box::new(GwPlan::Inference(Box::new(plan)))));
        }
        _ => {}
    }
    if let Some(planned) = super::gateway_write_plan::plan(command, format) {
        return planned;
    }
    if let Some(planned) = super::gateway_inventory::plan(command, format) {
        return planned;
    }
    let plan = match command {
        GatewayCommand::Workspaces(group) => match &group.command {
            None => return Ok(Plan::GroupHelp(&["gateway", "workspaces"])),
            Some(GwWorkspacesCommand::List(args)) => {
                let plane = plane(args.plane.as_ref())?;
                let status = status(args.status.as_ref())?;
                if args.all && (plane.is_some() || status.is_some()) {
                    return Err(CliError::usage(
                        "--all already merges admin-plane active + archived; drop --plane/--status",
                    ));
                }
                let list = if args.all {
                    WorkspaceList::All
                } else {
                    WorkspaceList::One {
                        plane: plane.unwrap_or(GwPlane::Data),
                        status,
                    }
                };
                GwPlan::WorkspacesList { list, format }
            }
            Some(GwWorkspacesCommand::Get(args)) => {
                let plane = plane(args.plane.as_ref())?.unwrap_or(GwPlane::Data);
                gateway::check_workspace_ref(&args.reference)?;
                GwPlan::WorkspacesGet {
                    reference: args.reference.clone(),
                    plane,
                    format,
                }
            }
            Some(GwWorkspacesCommand::Create(args)) => plan_workspace_create(args, format)?,
            Some(GwWorkspacesCommand::Update(args)) => plan_workspace_update(args, format)?,
            Some(GwWorkspacesCommand::Archive(args)) | Some(GwWorkspacesCommand::Delete(args)) => {
                gateway::check_resolved_ref("<ref>", &args.reference)?;
                super::write_common::gate_on_terminals(
                    args.force,
                    &format!("archive workspace {}", args.reference),
                )?;
                GwPlan::WorkspaceArchive {
                    reference: args.reference.clone(),
                    force: args.force,
                    deprecated: matches!(group.command, Some(GwWorkspacesCommand::Delete(_))),
                }
            }
        },
        GatewayCommand::Scopes(group) => match &group.command {
            None => return Ok(Plan::GroupHelp(&["gateway", "scopes"])),
            Some(GwScopesCommand::List) => GwPlan::ScopesList { format },
            Some(GwScopesCommand::Get(args)) => {
                gateway::check_scope_name(&args.name)?;
                GwPlan::ScopesGet {
                    name: args.name.clone(),
                    format,
                }
            }
            Some(GwScopesCommand::Create(args)) => {
                gateway::check_scope_name(&args.name)?;
                GwPlan::ScopeCreate {
                    name: args.name.clone(),
                    description: args.description.clone(),
                    format,
                }
            }
            Some(GwScopesCommand::Bind(args)) => {
                gateway::check_scope_name(&args.name)?;
                gateway::check_resolved_ref("--workspace", &args.workspace)?;
                GwPlan::ScopeBind {
                    name: args.name.clone(),
                    workspace: args.workspace.clone(),
                    format,
                }
            }
            Some(GwScopesCommand::Delete(args)) => {
                gateway::check_scope_name(&args.name)?;
                super::write_common::gate_on_terminals(
                    args.force,
                    &format!("delete IAM scope {}", args.name),
                )?;
                GwPlan::ScopeDelete {
                    name: args.name.clone(),
                    force: args.force,
                }
            }
        },
        GatewayCommand::Telemetry(group) => match &group.command {
            None => return Ok(Plan::GroupHelp(&["gateway", "telemetry"])),
            Some(command) => return plan_telemetry(command, format),
        },
        _ => unreachable!("the inventory commands are planned above"),
    };
    Ok(Plan::Gateway(Box::new(plan)))
}

/// `workspaces create` (§19.6): the checks in order, the body, its schema.
fn plan_workspace_create(
    args: &crate::cli::GwWorkspaceCreateArgs,
    format: Option<Format>,
) -> Result<GwPlan, CliError> {
    use crate::gateway::provision;
    if args.existing_scope && args.scope_name.is_none() {
        return Err(CliError::usage("--existing-scope requires --scope-name"));
    }
    if let Some(scope) = &args.scope_name {
        gateway::check_scope_name(scope)?;
    }
    let fields = &args.fields;
    let name = args.name.clone();
    let mut body = provision::workspace_fields(
        &[
            ("name", Some(&name)),
            ("description", args.description.as_ref()),
            ("icon", args.icon.as_ref()),
        ],
        args.users.as_deref(),
        provision::JsonFlags {
            defaults: fields.defaults.as_deref(),
            metadata: fields.metadata.as_deref(),
            usage_limits: fields.usage_limits.as_deref(),
            rate_limits: fields.rate_limits.as_deref(),
        },
    )?;
    let mut checked = body.clone();
    checked.insert(
        "scope_name".into(),
        serde_json::Value::String(
            args.scope_name
                .clone()
                .unwrap_or_else(|| "ws_placeholder_aaaaaa".to_owned()),
        ),
    );
    provision::check_create(&serde_json::Value::Object(checked))?;
    body.remove("scope_name");
    let warnings = args
        .scope_name
        .as_ref()
        .filter(|scope| provision::scope_name_looks_unrelated(&name, scope))
        .map(|scope| provision::unrelated_warning(scope, &name))
        .into_iter()
        .collect();
    Ok(GwPlan::WorkspaceCreate(Box::new(
        super::gateway_provision::ProvisionPlan {
            name,
            scope: args.scope_name.clone(),
            existing: args.existing_scope,
            description: args.description.clone(),
            body,
            warnings,
            format,
        },
    )))
}

/// `workspaces update <ref>` (§19.6).
fn plan_workspace_update(
    args: &crate::cli::GwWorkspaceUpdateArgs,
    format: Option<Format>,
) -> Result<GwPlan, CliError> {
    use crate::gateway::provision;
    gateway::check_resolved_ref("<ref>", &args.reference)?;
    let fields = &args.fields;
    let body = provision::workspace_fields(
        &[
            ("name", args.name.as_ref()),
            ("description", args.description.as_ref()),
            ("icon", args.icon.as_ref()),
        ],
        None,
        provision::JsonFlags {
            defaults: fields.defaults.as_deref(),
            metadata: fields.metadata.as_deref(),
            usage_limits: fields.usage_limits.as_deref(),
            rate_limits: fields.rate_limits.as_deref(),
        },
    )?;
    if body.is_empty() {
        return Err(CliError::usage(provision::UPDATE_NEEDS_A_FIELD));
    }
    provision::check_update(&serde_json::Value::Object(body.clone()), &args.reference)?;
    if !provision::replaced_blocks(&body).is_empty() {
        super::write_common::gate_on_terminals(
            args.force,
            &format!("replace the settings of workspace {}", args.reference),
        )?;
    }
    Ok(GwPlan::WorkspaceUpdate(Box::new(
        super::gateway_provision::UpdatePlan {
            reference: args.reference.clone(),
            body,
            force: args.force,
            format,
        },
    )))
}

fn plan_telemetry(command: &GwTelemetryCommand, format: Option<Format>) -> Result<Plan, CliError> {
    use GwTelemetryCommand as T;
    let plan = match command {
        T::Cost(args) => {
            let (days, window) = window::days_window(args.days.as_deref(), window::now_ms())?;
            let filters = window::filters(&filter_flags(&args.filters))?;
            gateway::check_resolved_ref("--workspace", &args.workspace)?;
            GwPlan::Cost {
                workspace: args.workspace.clone(),
                days,
                window,
                filters,
                format,
            }
        }
        T::Errors(args) => chart(Chart::Errors, args, format)?,
        T::Users(args) => chart(Chart::Users, args, format)?,
        T::ErrorTrends(args) => chart(Chart::ErrorTrends, args, format)?,
        T::ErrorCategoryTrends(args) => chart(Chart::ErrorCategoryTrends, args, format)?,
        T::GroupedErrors(args) => chart(Chart::GroupedErrors, args, format)?,
        T::RescuedRetries(args) => chart(Chart::RescuedRetries, args, format)?,
        T::UserTrends(args) => chart(Chart::UserTrends, args, format)?,
        T::Cache(group) => match &group.command {
            None => return Ok(Plan::GroupHelp(&["gateway", "telemetry", "cache"])),
            Some(GwCacheCommand::Summary(args)) => chart(Chart::CacheSummary, args, format)?,
            Some(GwCacheCommand::Trend(args)) => chart(Chart::CacheTrend, args, format)?,
        },
        T::Feedback(group) => match &group.command {
            None => return Ok(Plan::GroupHelp(&["gateway", "telemetry", "feedback"])),
            Some(GwFeedbackCommand::Distribution(args)) => {
                chart(Chart::FeedbackDistribution, args, format)?
            }
            Some(GwFeedbackCommand::Models(args)) => chart(Chart::FeedbackModels, args, format)?,
            Some(GwFeedbackCommand::Trend(args)) => chart(Chart::FeedbackTrend, args, format)?,
            Some(GwFeedbackCommand::Weighted(args)) => {
                chart(Chart::FeedbackWeighted, args, format)?
            }
        },
        T::FilterBoundaries(args) => {
            let window = leaf_window(args)?;
            telemetry(TelemetryRead::FilterBoundaries, window, args, format)?
        }
        T::Latency(args) | T::Requests(args) | T::Tokens(args) => {
            let which = match command {
                T::Latency(_) => FilteredChart::Latency,
                T::Requests(_) => FilteredChart::Requests,
                _ => FilteredChart::Tokens,
            };
            let window = leaf_window(&args.window)?;
            let filters = window::filters(&filter_flags(&args.filters))?;
            telemetry(
                TelemetryRead::Filtered(which, filters),
                window,
                &args.window,
                format,
            )?
        }
        T::GroupBy(args) => {
            let dimension = window::dimension(&args.dimension)?;
            let window = leaf_window(&args.window)?;
            let filters = window::filters(&filter_flags(&args.filters))?;
            let columns = window::columns(dimension, args.columns.as_deref())?;
            telemetry(
                TelemetryRead::GroupBy {
                    dimension,
                    columns,
                    filters,
                },
                window,
                &args.window,
                format,
            )?
        }
        T::Logs(group) => match &group.command {
            None => return Ok(Plan::GroupHelp(&["gateway", "telemetry", "logs"])),
            Some(GwLogsCommand::List(args)) => {
                let window = leaf_window(&args.window)?;
                let paging = window::logs_paging(
                    args.page_size.as_deref(),
                    args.current_page.as_deref(),
                    args.status_code.as_deref(),
                    args.trace_id.as_deref(),
                )?;
                telemetry(TelemetryRead::Logs(paging), window, &args.window, format)?
            }
        },
    };
    Ok(Plan::Gateway(Box::new(plan)))
}

/// Run a gateway plan.
pub async fn dispatch(invocation: &mut Invocation, plan: GwPlan) -> Result<(), CliError> {
    use super::{gateway_telemetry as tel, gateway_workspaces as ws};
    match plan {
        GwPlan::WorkspacesList { list, format } => ws::list(invocation, list, format).await,
        GwPlan::WorkspacesGet {
            reference,
            plane,
            format,
        } => ws::get(invocation, &reference, plane, format).await,
        GwPlan::ScopesList { format } => ws::scopes_list(invocation, format).await,
        GwPlan::ScopesGet { name, format } => ws::scopes_get(invocation, &name, format).await,
        GwPlan::Cost {
            workspace,
            days,
            window,
            filters,
            format,
        } => tel::cost(invocation, &workspace, days, &window, &filters, format).await,
        GwPlan::Telemetry {
            read,
            workspace,
            window,
            format,
        } => tel::read(invocation, &read, &workspace, &window, format).await,
        GwPlan::Inventory { read, format } => {
            super::gateway_inventory::read(invocation, &read, format).await
        }
        GwPlan::Write(plan) => super::gateway_writes::run(invocation, &plan).await,
        GwPlan::WorkspaceCreate(plan) => super::gateway_provision::create(invocation, &plan).await,
        GwPlan::WorkspaceUpdate(plan) => super::gateway_provision::update(invocation, &plan).await,
        GwPlan::WorkspaceArchive {
            reference,
            force,
            deprecated,
        } => super::gateway_provision::archive(invocation, &reference, force, deprecated).await,
        GwPlan::ScopeCreate {
            name,
            description,
            format,
        } => {
            super::gateway_provision::scope_create(
                invocation,
                &name,
                description.as_deref(),
                format,
            )
            .await
        }
        GwPlan::ScopeBind {
            name,
            workspace,
            format,
        } => super::gateway_provision::scope_bind(invocation, &name, &workspace, format).await,
        GwPlan::ScopeDelete { name, force } => {
            super::gateway_provision::scope_delete(invocation, &name, force).await
        }
        GwPlan::Inference(plan) => super::gateway_inference::run(invocation, &plan).await,
        GwPlan::Report(plan) => super::gateway_report::run(invocation, &plan).await,
    }
}

impl Invocation {
    /// Load the tenant, settle the format (the explicit `--output`, else the tenant's
    /// `defaultOutput`, else `pretty`; choice 5), check the management credentials
    /// (§3.5), build the gateway client (§19.3) and fetch the OAuth token under the SDK's
    /// own token budget, so a token failure is the §6 `AISEC_OAUTH_ERROR` message.
    pub async fn connect_gateway(
        &mut self,
        explicit: Option<Format>,
    ) -> Result<(AIGatewayClient, Format), CliError> {
        let (gateway, format, _) = self.connect_gateway_tsg(explicit, None).await?;
        Ok((gateway, format))
    }

    /// [`Invocation::connect_gateway`] with the TSG comparison of §19.3 between the
    /// credential check and the client: an explicit `--tsg-id`/`--organisation-id` (`flag`,
    /// `value`) other than the tenant's `mgmtTsgId` is refused (D245, exit 1, no request).
    /// Also returns the tenant's `mgmtTsgId`.
    pub async fn connect_gateway_tsg(
        &mut self,
        explicit: Option<Format>,
        tsg: Option<(&str, &str)>,
    ) -> Result<(AIGatewayClient, Format, String), CliError> {
        let (clients, format, tenant_tsg) = self.gateway_clients(explicit, tsg)?;
        clients.authenticate(None, TOKEN_BUDGET).await?;
        Ok((clients.gateway, format, tenant_tsg))
    }

    /// [`Invocation::connect_gateway_tsg`] without the token request (a write prompts
    /// first, §19.3 "Confirmations").
    pub fn gateway_clients(
        &mut self,
        explicit: Option<Format>,
        tsg: Option<(&str, &str)>,
    ) -> Result<(GatewayClients, Format, String), CliError> {
        let (context, format, tenant_tsg) = self.gateway_local_context(explicit)?;
        crate::tenant::config::require_management(&context.config, Some(context.name()))?;
        if let Some((flag, value)) = tsg
            && value != tenant_tsg
        {
            return Err(gateway::foreign_tsg(flag, value));
        }
        let clients = GatewayClients::build(
            &context.config,
            &self.session,
            &self.secrets,
            self.debug.as_ref(),
        )?;
        Ok((clients, format, tenant_tsg))
    }

    /// Resolve local selection and presentation without constructing clients or
    /// requiring credentials. Both execution and offline preview use this path.
    pub fn gateway_local_context(
        &mut self,
        explicit: Option<Format>,
    ) -> Result<(crate::tenant::TenantContext, Format, String), CliError> {
        let context = self.load_tenant()?;
        let format = output::resolve_tenant_default(explicit, context.config.default_output());
        let tsg = context
            .config
            .effective_str("mgmtTsgId")
            .unwrap_or_default()
            .to_owned();
        Ok((context, format, tsg))
    }

    /// The failure of a gateway request on `plane`: the 403 grant hint on stderr first
    /// (G42, D234), then the §6 error.
    pub fn gateway_error(&mut self, error: &scm::Error, plane: GwPlane) -> CliError {
        if let Some(hint) = gateway::grant_hint(error, plane) {
            self.output.warning(&hint);
        }
        self.management_error(error, SINGLE_REQUEST_BUDGET)
    }
}

/// The G39 warning of a list whose answer says it is incomplete (D248; the shared rule,
/// [`gateway::incomplete_total`]).
pub fn incomplete_warning(
    rows: usize,
    total: Option<&crate::json::OValue>,
    has_more: bool,
    label: &str,
) -> Option<String> {
    let total_text = match gateway::incomplete_total(rows, total, has_more)? {
        gateway::Incomplete::Of(total) => total.to_string(),
        gateway::Incomplete::MoreThanRows => format!("more than {rows}"),
    };
    Some(format!(
        "⚠ The service returned {rows} of {total_text} {label}; the list is incomplete (the API offers no continuation)"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::FromArgMatches;

    fn plan_of(args: &[&str]) -> Result<Plan, CliError> {
        let mut full = vec!["airs", "gateway"];
        full.extend_from_slice(args);
        let matches = crate::cli::command().try_get_matches_from(&full).unwrap();
        crate::commands::plan(&Cli::from_arg_matches(&matches).unwrap())
    }

    fn gw_plan(args: &[&str]) -> GwPlan {
        match plan_of(args) {
            Ok(Plan::Gateway(plan)) => *plan,
            Ok(_) => panic!("not a gateway plan: {args:?}"),
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

    #[test]
    fn workspace_and_scope_plans() {
        assert_eq!(
            gw_plan(&["workspaces", "ls"]),
            GwPlan::WorkspacesList {
                list: WorkspaceList::One {
                    plane: GwPlane::Data,
                    status: None
                },
                format: None
            }
        );
        assert_eq!(
            gw_plan(&[
                "workspace",
                "list",
                "--plane",
                "admin",
                "--status",
                "archived"
            ]),
            GwPlan::WorkspacesList {
                list: WorkspaceList::One {
                    plane: GwPlane::Admin,
                    status: Some(ListStatus::Archived)
                },
                format: None
            }
        );
        assert_eq!(
            gw_plan(&["workspaces", "list", "--all", "--output", "json"]),
            GwPlan::WorkspacesList {
                list: WorkspaceList::All,
                format: Some(Format::Json)
            }
        );
        let plan = gw_plan(&["scopes", "get", "ws_production_bx7qw0"]);
        assert_eq!(plan.interrupt_context(), InterruptContext::NoDataWritten);
        assert_eq!(plan.debug_policy(), crate::debug::DebugPolicy::Log);
        for (args, message) in [
            (
                &["workspaces", "list", "--plane", "bogus"][..],
                "Invalid --plane 'bogus'. Valid planes: data, admin",
            ),
            (
                &["workspaces", "list", "--status", "bogus"],
                "Invalid --status 'bogus'. Valid statuses: active, archived",
            ),
            (
                &["workspaces", "list", "--all", "--plane", "admin"],
                "--all already merges admin-plane active + archived; drop --plane/--status",
            ),
            (
                &["workspaces", "get", "Main A"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid workspaceRef: Main A (expected a workspace UUID or slug)",
            ),
            (
                &["workspaces", "get", "ws-x", "--plane", "x"],
                "Invalid --plane 'x'. Valid planes: data, admin",
            ),
            (
                &["scopes", "get", "bad name"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid name: bad name (expected an IAM scope name such as ws_production_bx7qw0)",
            ),
            (
                &["workspaces", "list", "--output", "yaml"],
                "Invalid output format 'yaml'. Expected: pretty, table, json",
            ),
        ] {
            assert_eq!(usage(args), message, "{args:?}");
        }
    }

    #[test]
    fn telemetry_plans_check_windows_filters_and_flags_first() {
        let plan = gw_plan(&["telemetry", "errors", "--workspace", "Main A"]);
        assert!(matches!(
            plan,
            GwPlan::Telemetry {
                read: TelemetryRead::Chart(Chart::Errors),
                ref workspace,
                ..
            } if workspace == "Main A"
        ));
        assert!(matches!(
            gw_plan(&["telemetry", "cost", "--workspace", "w", "--days", "30"]),
            GwPlan::Cost { days: 30, .. }
        ));
        assert!(matches!(
            gw_plan(&["telemetry", "group-by", "users", "--workspace", "w"]),
            GwPlan::Telemetry {
                read: TelemetryRead::GroupBy {
                    dimension: Dimension::Users,
                    columns: None,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            gw_plan(&[
                "telemetry",
                "logs",
                "ls",
                "--workspace",
                "w",
                "--page-size",
                "5"
            ]),
            GwPlan::Telemetry {
                read: TelemetryRead::Logs(LogsPaging { page_size: 5, .. }),
                ..
            }
        ));
        for (args, message) in [
            (
                &["telemetry", "errors", "--workspace", " "][..],
                "--workspace cannot be empty",
            ),
            (
                &["telemetry", "errors", "--workspace", "w", "--days", "0"],
                "Invalid --days: Expected a positive integer",
            ),
            (
                &[
                    "telemetry",
                    "requests",
                    "--workspace",
                    "w",
                    "--cost-min",
                    "NaN",
                ],
                window::FILTER_SYNTAX,
            ),
            (
                &[
                    "telemetry",
                    "cost",
                    "--workspace",
                    "w",
                    "--cost-min",
                    "1",
                    "--cost-max",
                    "0",
                ],
                window::FILTER_SEMANTICS,
            ),
            (
                &["telemetry", "group-by", "status", "--workspace", "w"],
                "Invalid grouping dimension: choose from ai_service, model, api_key, provider, status_code, users",
            ),
            (
                &[
                    "telemetry",
                    "group-by",
                    "users",
                    "--workspace",
                    "w",
                    "--columns",
                    "cost",
                ],
                "User grouping does not support --columns",
            ),
            (
                &[
                    "telemetry",
                    "logs",
                    "list",
                    "--workspace",
                    "w",
                    "--current-page",
                    "-1",
                ],
                "--current-page must be a nonnegative safe integer",
            ),
            (
                &[
                    "telemetry",
                    "users",
                    "--workspace",
                    "w",
                    "--days",
                    "1",
                    "--start",
                    "2026-10-01T00:00:00Z",
                ],
                "Use --days or --start/--end, not both",
            ),
        ] {
            assert_eq!(usage(args), message, "{args:?}");
        }
    }

    #[test]
    fn groups_without_a_subcommand_render_help() {
        for (args, path) in [
            (&[][..], &["gateway"][..]),
            (&["workspaces"], &["gateway", "workspaces"]),
            (&["workspace"], &["gateway", "workspaces"]),
            (&["scopes"], &["gateway", "scopes"]),
            (&["telemetry"], &["gateway", "telemetry"]),
            (&["telemetry", "cache"], &["gateway", "telemetry", "cache"]),
            (
                &["telemetry", "feedback"],
                &["gateway", "telemetry", "feedback"],
            ),
            (&["telemetry", "logs"], &["gateway", "telemetry", "logs"]),
        ] {
            match plan_of(args) {
                Ok(Plan::GroupHelp(found)) => assert_eq!(found, path),
                _ => panic!("expected group help for {args:?}"),
            }
        }
    }

    #[test]
    fn incomplete_lists_are_named() {
        let total = |n: i64| crate::json::OValue::Number(n.into());
        assert_eq!(
            incomplete_warning(2, Some(&total(2)), false, "workspaces"),
            None
        );
        assert_eq!(
            incomplete_warning(2, Some(&total(5)), false, "workspaces").unwrap(),
            "⚠ The service returned 2 of 5 workspaces; the list is incomplete (the API offers no continuation)"
        );
        assert_eq!(
            incomplete_warning(2, Some(&total(2)), true, "IAM scopes").unwrap(),
            "⚠ The service returned 2 of more than 2 IAM scopes; the list is incomplete (the API offers no continuation)"
        );
        assert_eq!(
            incomplete_warning(2, Some(&crate::json::parse("2.5").unwrap()), false, "x"),
            None
        );
    }
}
