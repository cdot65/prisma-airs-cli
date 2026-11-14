//! Planning of the gateway writes of chunk 3 other than workspaces and scopes (DESIGN
//! §19.3, §19.10–§19.13): every flag is parsed and checked before the tenant (D232), one
//! planner per command group; the always-confirming writes refuse a non-interactive run
//! without `--force` here (D243, choice 124). Dispatch is `gateway_writes`.

use super::Plan;
use super::gateway::GwPlan;
use super::write_common::{self as wc};
use crate::cli::{
    GatewayCommand, GwAdminGuardrailsCommand, GwAdminMcpServersCommand, GwApiKeysCommand,
    GwAuthSettingsCommand, GwConfigsCommand, GwDeploymentsCommand, GwGuardrailsCommand,
    GwIntegrationCreateArgs, GwIntegrationModelsCommand, GwIntegrationSharedArgs,
    GwIntegrationWorkspacesCommand, GwIntegrationsCommand, GwKeyCreateArgs,
    GwMcpCapabilitiesCommand, GwMcpCommand, GwMcpIntegrationsCommand, GwMcpWorkspacesCommand,
    GwOrganisationArgs, GwOrganisationSelfCommand, GwOrganisationsCommand, GwPluginsCommand,
    GwProvidersCommand, GwSecretArgs, GwServiceKeysCommand, GwStructuredArgs, GwUserKeysCommand,
};
use crate::error::CliError;
use crate::gateway;
use crate::gateway::body::{self, Input};
use crate::gateway::inventory::KeyKind;
use crate::gateway::secrets::{self, SecretWrite};
use crate::gateway::writes::Write;
use crate::output::Format;
use serde_json::{Value, json};
use std::ffi::OsString;
use std::path::PathBuf;

/// The placeholder provider UUID of a body whose `--ai-provider` slug is resolved later.
pub(super) const PROVIDER_PLACEHOLDER: &str = "00000000-0000-4000-8000-000000000000";

/// Where an integration's provider credential comes from (§19.12).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeySource {
    /// `--key` (already a named field) or no key flag on `update`.
    Given,
    /// No key flag on `create`: the hidden prompt when interactive (§19.12).
    Missing,
    File(OsString),
    Stdin,
}

/// A validated gateway write (§19.10–§19.13).
#[derive(Clone, Debug, PartialEq)]
pub struct WritePlan {
    pub write: Write,
    pub input: Input,
    pub key: KeySource,
    /// An `--ai-provider` slug, resolved over the catalog after the token (G38).
    pub provider: Option<String>,
    /// An explicit `--organisation-id` / `--tsg-id` (compared with the tenant, D245).
    pub organisation: Option<String>,
    /// Whether the body carries `organisation_id` (defaulting to the tenant's TSG).
    pub org_body: bool,
    pub secret: Option<PathBuf>,
    pub force: bool,
    pub dry_run: bool,
    /// The inline-credential warnings (D247), printed before anything is read.
    pub warnings: Vec<String>,
    /// A create's `--workspace` (its unknown-outcome hint).
    pub workspace: Option<String>,
    pub format: Option<Format>,
}

impl WritePlan {
    fn new(write: Write, format: Option<Format>) -> Self {
        Self {
            write,
            input: Input::default(),
            key: KeySource::Given,
            provider: None,
            organisation: None,
            org_body: false,
            secret: None,
            force: false,
            dry_run: false,
            warnings: Vec::new(),
            workspace: None,
            format,
        }
    }

    /// The one-time credential write this is, if any (`rotating`: the body rotates).
    pub fn secret_write(&self, rotating: bool) -> Option<SecretWrite> {
        match &self.write {
            Write::KeyCreate(kind) => Some(SecretWrite::KeyCreate(*kind)),
            Write::KeyRotate(kind, id) => Some(SecretWrite::KeyRotate(*kind, id.clone())),
            Write::DeploymentCreate => Some(SecretWrite::DeploymentCreate),
            Write::DeploymentUpdate(id) if rotating => {
                Some(SecretWrite::DeploymentRotate(id.clone()))
            }
            _ => None,
        }
    }
}

fn uuid(field: &str, value: &str) -> Result<String, CliError> {
    gateway::check_uuid(field, value)?;
    Ok(value.to_owned())
}

fn opt_uuid(field: &str, value: Option<&String>) -> Result<Option<Value>, CliError> {
    value
        .map(|value| uuid(field, value).map(Value::String))
        .transpose()
}

fn text(value: Option<&String>) -> Option<Value> {
    value.map(|value| Value::String(value.clone()))
}

fn json_flag(flag: &str, value: Option<&String>) -> Result<Option<Value>, CliError> {
    value.map(|value| body::json(flag, value)).transpose()
}

fn bool_flag(flag: &str, value: Option<&String>) -> Result<Option<Value>, CliError> {
    value
        .map(|value| body::boolean(flag, value).map(Value::Bool))
        .transpose()
}

fn csv_flag(flag: &str, value: Option<&String>) -> Result<Option<Value>, CliError> {
    value
        .map(|value| body::csv(flag, value).map(|items| json!(items)))
        .transpose()
}

fn time_flag(flag: &str, value: Option<&String>) -> Result<Option<Value>, CliError> {
    value
        .map(|value| body::timestamp(flag, value).map(Value::String))
        .transpose()
}

/// The structured input options (`--file` read later; the assignments parsed now).
fn structured(args: &GwStructuredArgs) -> Result<Input, CliError> {
    Ok(Input {
        file: args.file.clone(),
        named: Vec::new(),
        sets: body::assignments(&args.set, &args.set_string)?,
    })
}

/// An explicit numeric TSG flag (`Invalid organisationId: <v>` / `Invalid tsgId: <v>`).
fn tsg(field: &str, value: Option<&String>) -> Result<Option<String>, CliError> {
    value
        .map(|value| gateway::check_tsg(field, value).map(|()| value.clone()))
        .transpose()
}

fn organisation(args: &GwOrganisationArgs) -> Result<Option<String>, CliError> {
    tsg("organisationId", args.organisation_id.as_ref())
}

/// The one-time credential destination (D237): `--show-secret` refused, `--secret-output`
/// required (when `required`), `""` and `-` refused.
fn destination(args: &GwSecretArgs, required: bool) -> Result<Option<PathBuf>, CliError> {
    if args.show_secret {
        return Err(CliError::usage(secrets::SHOW_SECRET));
    }
    match &args.secret_output {
        Some(value) => secrets::plan_destination(value).map(Some),
        None if required => Err(CliError::usage(secrets::NEEDS_DESTINATION)),
        None => Ok(None),
    }
}

/// The inline-credential warning of `--scim-token`, `--credential` and `--header` (D247).
fn inline_warning(flag: &str) -> String {
    format!(
        "⚠ Inline {flag} is visible in shell history and process listings; prefer a --file request body (mode 0600)"
    )
}

/// The inline `--key` warning (TS text).
const KEY_WARNING: &str = "⚠ Inline --key is visible in shell history and process listings; prefer --key-stdin, --key-file, or --secret-mappings";

/// The non-interactive refusal of a write that always confirms (planning).
fn gate(plan: &WritePlan) -> Result<(), CliError> {
    if let Some(confirm) = plan
        .write
        .confirm(&json!({}), plan.organisation.as_deref(), None)
    {
        wc::gate_on_terminals(plan.force, &confirm.action)?;
    }
    Ok(())
}

fn key_create(
    kind: KeyKind,
    args: &GwKeyCreateArgs,
    user: Option<&String>,
    format: Option<Format>,
    dry_run: bool,
) -> Result<WritePlan, CliError> {
    let mut plan = WritePlan::new(Write::KeyCreate(kind), format);
    plan.dry_run = dry_run;
    plan.secret = destination(&args.secret, !dry_run)?;
    let workspace = opt_uuid("workspaceId", args.workspace.as_ref())?;
    plan.workspace = args.workspace.clone();
    plan.organisation = organisation(&args.organisation)?;
    plan.org_body = true;
    let mut input = structured(&args.input)?;
    input.named(
        "alert_emails",
        csv_flag("--alert-emails", args.alert_emails.as_ref())?,
    );
    input.text("description", args.description.as_ref());
    input.named(
        "expires_at",
        time_flag("--expires-at", args.expires_at.as_ref())?,
    );
    input.text("name", args.name.as_ref());
    input.named("organisation_id", text(plan.organisation.as_ref()));
    input.named("scopes", csv_flag("--scopes", args.scopes.as_ref())?);
    input.text("type", args.kind.as_ref());
    input.named("workspace_id", workspace);
    input.named("user_id", opt_uuid("userId", user)?);
    plan.input = input;
    Ok(plan)
}

/// The integration flags shared by `create` and `update` (§19.12): the credential source
/// and its warning, `--base-url`, `--header` (warned), the JSON members.
fn integration_shared(
    plan: &mut WritePlan,
    args: &GwIntegrationSharedArgs,
    create: bool,
) -> Result<Vec<(&'static str, Option<Value>)>, CliError> {
    let sources = [args.key.is_some(), args.key_stdin, args.key_file.is_some()];
    if sources.iter().filter(|given| **given).count() > 1 {
        return Err(CliError::usage(
            "Use only one of --key, --key-stdin, or --key-file",
        ));
    }
    plan.key = match (&args.key, args.key_stdin, &args.key_file) {
        (Some(_), ..) => {
            plan.warnings.push(KEY_WARNING.to_owned());
            KeySource::Given
        }
        (None, true, _) => KeySource::Stdin,
        (None, false, Some(path)) => KeySource::File(path.clone()),
        (None, false, None) if create => KeySource::Missing,
        _ => KeySource::Given,
    };
    let configurations = json_flag("--configurations", args.configurations.as_ref())?;
    let secret_mappings = json_flag("--secret-mappings", args.secret_mappings.as_ref())?;
    let (auth_type, host) = match &args.base_url {
        Some(url) => {
            let parsed = url::Url::parse(url).map_err(|_| {
                CliError::usage(
                    "Invalid --base-url: AISEC_USER_REQUEST_PAYLOAD_ERROR:custom host must be an absolute http(s) URL including its API prefix",
                )
            })?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err(CliError::usage(
                    "Invalid --base-url: AISEC_USER_REQUEST_PAYLOAD_ERROR:custom host must use http or https",
                ));
            }
            let options = scm::airs::gateway::CustomHostConfigurationOptions::new(url.clone());
            scm::airs::gateway::custom_host_configuration(&options).map_err(|_| {
                CliError::usage(
                    "Invalid --base-url: AISEC_USER_REQUEST_PAYLOAD_ERROR:custom host must be an absolute http(s) URL including its API prefix",
                )
            })?;
            (Some(json!("apiKey")), Some(json!(url)))
        }
        None => (None, None),
    };
    let headers = if args.header.is_empty() {
        None
    } else {
        plan.warnings.push(inline_warning("--header"));
        Some(body::string_map("--header", &args.header)?)
    };
    Ok(vec![
        ("configurations", configurations),
        ("description", text(args.description.as_ref())),
        ("key", text(args.key.as_ref())),
        ("name", text(args.name.as_ref())),
        ("secret_mappings", secret_mappings),
        ("configurations.provider_auth_type", auth_type),
        ("configurations.custom_host", host),
        ("configurations.custom_headers", headers),
    ])
}

/// Plan a write command (`None` for the reads and the workspace and scope commands).
pub fn plan(command: &GatewayCommand, format: Option<Format>) -> Option<Result<Plan, CliError>> {
    let planned = plan_write(command, format)?;
    Some(planned.map(|plan| Plan::Gateway(Box::new(GwPlan::Write(Box::new(plan))))))
}

fn plan_write(
    command: &GatewayCommand,
    format: Option<Format>,
) -> Option<Result<WritePlan, CliError>> {
    use GatewayCommand as G;
    match command {
        G::Configs(group) => plan_configs(group.command.as_ref()?, format),
        G::Guardrails(group) => plan_guardrails(group.command.as_ref()?, format),
        G::Providers(group) => plan_providers(group.command.as_ref()?, format),
        G::ApiKeys(group) => plan_api_keys(group.command.as_ref()?, format),
        G::Deployments(group) => plan_deployments(group.command.as_ref()?, format),
        G::Integrations(group) => plan_integrations(group.command.as_ref()?, format),
        G::Mcp(group) => plan_mcp(group.command.as_ref()?, format),
        G::Organisations(group) => plan_organisations(group.command.as_ref()?, format),
        G::Plugins(group) => plan_plugins(group.command.as_ref()?, format),
        G::AdminGuardrails(group) => plan_admin_guardrails(group.command.as_ref()?, format),
        G::AuditLogs(_)
        | G::Scopes(_)
        | G::Telemetry(_)
        | G::Workspaces(_)
        | G::Inference(_)
        | G::Report(_) => None,
    }
}

/// A removal or other body-less write that confirms: the non-interactive refusal now.
fn removal(format: Option<Format>, write: Write, force: bool) -> Result<WritePlan, CliError> {
    let mut plan = WritePlan::new(write, format);
    plan.force = force;
    gate(&plan)?;
    Ok(plan)
}

/// `configs create|update|delete` (§19.10).
fn plan_configs(
    command: &GwConfigsCommand,
    format: Option<Format>,
) -> Option<Result<WritePlan, CliError>> {
    Some(match command {
        GwConfigsCommand::Create(args) => (|| {
            let mut plan = WritePlan::new(Write::ConfigCreate, format);
            let mut input = structured(&args.input)?;
            input.text("name", args.name.as_ref());
            input.named(
                "workspace_id",
                opt_uuid("workspaceId", args.workspace.as_ref())?,
            );
            plan.workspace = args.workspace.clone();
            plan.input = input;
            Ok(plan)
        })(),
        GwConfigsCommand::Update(args) => (|| {
            let mut plan = WritePlan::new(Write::ConfigUpdate(uuid("configId", &args.id)?), format);
            let mut input = structured(&args.input)?;
            input.text("name", args.name.as_ref());
            input.text("workspace_id", args.workspace.as_ref());
            input.text("status", args.status.as_ref());
            plan.input = input;
            Ok(plan)
        })(),
        GwConfigsCommand::Delete(args) => uuid("configId", &args.id)
            .and_then(|id| removal(format, Write::ConfigDelete(id), args.force)),
        _ => return None,
    })
}

/// `guardrails create|update|delete` (§19.10).
fn plan_guardrails(
    command: &GwGuardrailsCommand,
    format: Option<Format>,
) -> Option<Result<WritePlan, CliError>> {
    Some(match command {
        GwGuardrailsCommand::Create(args) => (|| {
            let mut plan = WritePlan::new(Write::GuardrailCreate, format);
            let mut input = structured(&args.input)?;
            input.named("actions", json_flag("--actions", args.actions.as_ref())?);
            input.named("checks", json_flag("--checks", args.checks.as_ref())?);
            input.text("name", args.name.as_ref());
            input.named(
                "workspace_id",
                opt_uuid("workspaceId", args.workspace.as_ref())?,
            );
            plan.workspace = args.workspace.clone();
            plan.input = input;
            Ok(plan)
        })(),
        GwGuardrailsCommand::Update(args) => (|| {
            let mut plan = WritePlan::new(
                Write::GuardrailUpdate(uuid("guardrailId", &args.id)?),
                format,
            );
            let mut input = structured(&args.input)?;
            input.named("actions", json_flag("--actions", args.actions.as_ref())?);
            input.named("checks", json_flag("--checks", args.checks.as_ref())?);
            input.text("name", args.name.as_ref());
            plan.input = input;
            Ok(plan)
        })(),
        GwGuardrailsCommand::Delete(args) => uuid("guardrailId", &args.id)
            .and_then(|id| removal(format, Write::GuardrailDelete(id), args.force)),
        _ => return None,
    })
}

/// `providers create|update|delete` (§19.10).
fn plan_providers(
    command: &GwProvidersCommand,
    format: Option<Format>,
) -> Option<Result<WritePlan, CliError>> {
    Some(match command {
        GwProvidersCommand::Create(args) => (|| {
            let mut plan = WritePlan::new(Write::ProviderCreate, format);
            let mut input = structured(&args.input)?;
            input.named(
                "ai_provider_id",
                opt_uuid("aiProviderId", args.ai_provider_id.as_ref())?,
            );
            input.named(
                "expires_at",
                time_flag("--expires-at", args.expires_at.as_ref())?,
            );
            input.named(
                "integration_id",
                opt_uuid("integrationId", args.integration_id.as_ref())?,
            );
            input.text("name", args.name.as_ref());
            input.text("note", args.note.as_ref());
            input.named(
                "rate_limits",
                json_flag("--rate-limit", args.rate_limit.as_ref())?,
            );
            input.text("slug", args.slug.as_ref());
            input.named(
                "usage_limits",
                json_flag("--usage-limit", args.usage_limit.as_ref())?,
            );
            input.named(
                "workspace_id",
                opt_uuid("workspaceId", args.workspace.as_ref())?,
            );
            plan.workspace = args.workspace.clone();
            plan.input = input;
            Ok(plan)
        })(),
        GwProvidersCommand::Update(args) => (|| {
            let mut plan =
                WritePlan::new(Write::ProviderUpdate(uuid("providerId", &args.id)?), format);
            plan.force = args.force;
            let mut input = structured(&args.input)?;
            input.named(
                "expires_at",
                time_flag("--expires-at", args.expires_at.as_ref())?,
            );
            input.text("name", args.name.as_ref());
            input.text("note", args.note.as_ref());
            input.named(
                "rate_limits",
                json_flag("--rate-limit", args.rate_limit.as_ref())?,
            );
            input.named(
                "reset_usage",
                bool_flag("--reset-usage", args.reset_usage.as_ref())?,
            );
            input.named(
                "usage_limits",
                json_flag("--usage-limit", args.usage_limit.as_ref())?,
            );
            plan.input = input;
            Ok(plan)
        })(),
        GwProvidersCommand::Delete(args) => uuid("providerId", &args.id)
            .and_then(|id| removal(format, Write::ProviderDelete(id), args.force)),
        _ => return None,
    })
}

/// `api-keys service|user create|update|rotate|delete` (§19.10, §19.11).
fn plan_api_keys(
    command: &GwApiKeysCommand,
    format: Option<Format>,
) -> Option<Result<WritePlan, CliError>> {
    let (kind, command) = match command {
        GwApiKeysCommand::Service(group) => (
            KeyKind::Service,
            match group.command.as_ref()? {
                GwServiceKeysCommand::Create(args) => {
                    return Some(key_create(
                        KeyKind::Service,
                        &args.key,
                        None,
                        format,
                        args.dry_run,
                    ));
                }
                GwServiceKeysCommand::Update(args) => KeyLeaf::Update(args),
                GwServiceKeysCommand::Rotate(args) => KeyLeaf::Rotate(args),
                GwServiceKeysCommand::Delete(args) => KeyLeaf::Delete(args),
                _ => return None,
            },
        ),
        GwApiKeysCommand::User(group) => (
            KeyKind::User,
            match group.command.as_ref()? {
                GwUserKeysCommand::Create(args) => {
                    return Some(key_create(
                        KeyKind::User,
                        &args.key,
                        args.user_id.as_ref(),
                        format,
                        false,
                    ));
                }
                GwUserKeysCommand::Update(args) => KeyLeaf::Update(args),
                GwUserKeysCommand::Rotate(args) => KeyLeaf::Rotate(args),
                GwUserKeysCommand::Delete(args) => KeyLeaf::Delete(args),
                _ => return None,
            },
        ),
    };
    Some(match command {
        KeyLeaf::Update(args) => (|| {
            let mut plan = WritePlan::new(Write::KeyUpdate(kind, uuid("keyId", &args.id)?), format);
            plan.force = args.force;
            let mut input = structured(&args.input)?;
            input.named(
                "alert_emails",
                csv_flag("--alert-emails", args.alert_emails.as_ref())?,
            );
            input.text("description", args.description.as_ref());
            input.named(
                "expires_at",
                time_flag("--expires-at", args.expires_at.as_ref())?,
            );
            input.text("name", args.name.as_ref());
            input.named(
                "reset_usage",
                bool_flag("--reset-usage", args.reset_usage.as_ref())?,
            );
            input.named("scopes", csv_flag("--scopes", args.scopes.as_ref())?);
            plan.input = input;
            Ok(plan)
        })(),
        KeyLeaf::Rotate(args) => (|| {
            let secret = destination(&args.secret, true)?;
            let mut plan = WritePlan::new(Write::KeyRotate(kind, uuid("keyId", &args.id)?), format);
            plan.secret = secret;
            plan.force = args.force;
            plan.input.named(
                "key_transition_period_ms",
                args.transition_ms
                    .as_ref()
                    .map(|ms| body::integer("--transition-ms", ms).map(Value::from))
                    .transpose()?,
            );
            gate(&plan)?;
            Ok(plan)
        })(),
        KeyLeaf::Delete(args) => uuid("keyId", &args.id)
            .and_then(|id| removal(format, Write::KeyDelete(kind, id), args.force)),
    })
}

/// `deployments create|update|archive` (§19.10, §19.11).
fn plan_deployments(
    command: &GwDeploymentsCommand,
    format: Option<Format>,
) -> Option<Result<WritePlan, CliError>> {
    Some(match command {
        GwDeploymentsCommand::Create(args) => (|| {
            let mut plan = WritePlan::new(Write::DeploymentCreate, format);
            plan.secret = destination(&args.secret, true)?;
            plan.organisation = organisation(&args.organisation)?;
            plan.org_body = true;
            let mut input = structured(&args.input)?;
            input.named(
                "auth_settings",
                json_flag("--auth-settings", args.auth_settings.as_ref())?,
            );
            input.named(
                "deployment_config",
                json_flag("--deployment-config", args.deployment_config.as_ref())?,
            );
            input.named(
                "is_default",
                bool_flag("--is-default", args.is_default.as_ref())?,
            );
            input.text("name", args.name.as_ref());
            input.named("organisation_id", text(plan.organisation.as_ref()));
            input.text("slug", args.slug.as_ref());
            input.text("type", args.kind.as_ref());
            plan.input = input;
            Ok(plan)
        })(),
        GwDeploymentsCommand::Update(args) => (|| {
            if args.secret.show_secret {
                return Err(CliError::usage(secrets::SHOW_SECRET));
            }
            let id = uuid("deploymentId", &args.id)?;
            let mut plan = WritePlan::new(Write::DeploymentUpdate(id), format);
            plan.force = args.force;
            plan.dry_run = args.dry_run;
            let rotate = bool_flag("--rotate-auth", args.rotate_auth.as_ref())?;
            plan.secret = destination(
                &args.secret,
                rotate == Some(Value::Bool(true)) && !plan.dry_run,
            )?;
            let mut input = structured(&args.input)?;
            input.named(
                "auth_settings",
                json_flag("--auth-settings", args.auth_settings.as_ref())?,
            );
            input.named(
                "deployment_config",
                json_flag("--deployment-config", args.deployment_config.as_ref())?,
            );
            input.named(
                "is_default",
                bool_flag("--is-default", args.is_default.as_ref())?,
            );
            input.text("name", args.name.as_ref());
            input.named(
                "override_existing",
                bool_flag("--override-existing", args.override_existing.as_ref())?,
            );
            input.named("rotate_auth", rotate);
            input.text("status", args.status.as_ref());
            input.text("type", args.kind.as_ref());
            plan.input = input;
            Ok(plan)
        })(),
        GwDeploymentsCommand::Archive(args) => (|| {
            let id = uuid("deploymentId", &args.id)?;
            let mut plan = WritePlan::new(Write::DeploymentArchive(id), format);
            plan.organisation = organisation(&args.organisation)?;
            plan.force = args.force;
            gate(&plan)?;
            Ok(plan)
        })(),
        _ => return None,
    })
}

/// `integrations create` (§19.12): `--ai-provider` (a UUID lowercased now, a slug resolved
/// after the token, G38) or `--ai-provider-id`, the shared flags, the organisation.
fn integration_create(
    args: &GwIntegrationCreateArgs,
    format: Option<Format>,
) -> Result<WritePlan, CliError> {
    let mut plan = WritePlan::new(Write::IntegrationCreate, format);
    if args.ai_provider.is_some() && args.ai_provider_id.is_some() {
        return Err(CliError::usage(
            "Use --ai-provider or --ai-provider-id, not both",
        ));
    }
    let mut provider_id = opt_uuid("aiProviderId", args.ai_provider_id.as_ref())?;
    if let Some(reference) = &args.ai_provider {
        let wanted = crate::tenant::config::js_trim(reference);
        if wanted.is_empty() {
            return Err(CliError::usage("--ai-provider cannot be empty"));
        }
        if crate::pagination::is_uuid(wanted) {
            provider_id = Some(Value::String(wanted.to_ascii_lowercase()));
        } else {
            plan.provider = Some(wanted.to_owned());
        }
    }
    plan.organisation = organisation(&args.organisation)?;
    plan.org_body = true;
    let shared = integration_shared(&mut plan, &args.shared, true)?;
    let mut input = structured(&args.input)?;
    input.named("ai_provider_id", provider_id);
    for (path, value) in shared {
        if path == "secret_mappings" {
            input.named("organisation_id", text(plan.organisation.as_ref()));
        }
        input.named(path, value);
    }
    input.text("slug", args.slug.as_ref());
    plan.input = input;
    Ok(plan)
}

/// `integrations create|update|delete`, `models set`, `workspaces set` (§19.12).
fn plan_integrations(
    command: &GwIntegrationsCommand,
    format: Option<Format>,
) -> Option<Result<WritePlan, CliError>> {
    Some(match command {
        GwIntegrationsCommand::Create(args) => integration_create(args, format),
        GwIntegrationsCommand::Update(args) => (|| {
            let id = uuid("integrationId", &args.id)?;
            let mut plan = WritePlan::new(Write::IntegrationUpdate(id), format);
            plan.force = args.force;
            let shared = integration_shared(&mut plan, &args.shared, false)?;
            let mut input = structured(&args.input)?;
            for (path, value) in shared {
                input.named(path, value);
            }
            plan.input = input;
            Ok(plan)
        })(),
        GwIntegrationsCommand::Delete(args) => (|| {
            let id = uuid("integrationId", &args.id)?;
            let mut plan = WritePlan::new(Write::IntegrationDelete(id), format);
            plan.organisation = organisation(&args.organisation)?;
            plan.force = args.force;
            gate(&plan)?;
            Ok(plan)
        })(),
        GwIntegrationsCommand::Models(group) => match group.command.as_ref()? {
            GwIntegrationModelsCommand::Set(args) => (|| {
                let id = uuid("integrationId", &args.id)?;
                let mut plan = WritePlan::new(Write::ModelsSet(id), format);
                plan.force = args.force;
                let mut input = structured(&args.input)?;
                input.named(
                    "allow_all_models",
                    bool_flag("--allow-all-models", args.allow_all_models.as_ref())?,
                );
                if !args.model.is_empty() {
                    input.named(
                        "models",
                        Some(body::model_bindings("--model", &args.model)?),
                    );
                }
                plan.input = input;
                gate(&plan)?;
                Ok(plan)
            })(),
            _ => return None,
        },
        GwIntegrationsCommand::Workspaces(group) => match group.command.as_ref()? {
            GwIntegrationWorkspacesCommand::Set(args) => (|| {
                let id = uuid("integrationId", &args.id)?;
                let mut plan = WritePlan::new(Write::IntegrationWorkspacesSet(id), format);
                plan.force = args.force;
                let mut input = structured(&args.input)?;
                input.named(
                    "create_default_provider",
                    bool_flag(
                        "--create-default-provider",
                        args.create_default_provider.as_ref(),
                    )?,
                );
                input.text("default_provider_slug", args.default_provider_slug.as_ref());
                input.named(
                    "global_workspace_access.enabled",
                    bool_flag("--global-access", args.global_access.as_ref())?,
                );
                input.named(
                    "override_existing_workspace_access",
                    Some(Value::Bool(!args.preserve_existing)),
                );
                if !args.workspace_binding.is_empty() {
                    input.named(
                        "workspaces",
                        Some(body::id_bindings(
                            "--workspace-binding",
                            &args.workspace_binding,
                        )?),
                    );
                }
                plan.input = input;
                gate(&plan)?;
                Ok(plan)
            })(),
            _ => return None,
        },
        _ => return None,
    })
}

/// `mcp integrations create|update|delete`, `capabilities set`, `workspaces set` (§19.12).
fn plan_mcp(command: &GwMcpCommand, format: Option<Format>) -> Option<Result<WritePlan, CliError>> {
    Some(match command {
        GwMcpCommand::Integrations(group) => match group.command.as_ref()? {
            GwMcpIntegrationsCommand::Create(args) => (|| {
                let mut plan = WritePlan::new(Write::McpCreate, format);
                plan.organisation = organisation(&args.organisation)?;
                plan.org_body = true;
                let mut input = structured(&args.input)?;
                input.text("auth_type", args.auth_type.as_ref());
                input.named(
                    "configurations",
                    json_flag("--configurations", args.configurations.as_ref())?,
                );
                input.text("description", args.description.as_ref());
                input.text("name", args.name.as_ref());
                input.named("organisation_id", text(plan.organisation.as_ref()));
                input.named(
                    "secret_mappings",
                    json_flag("--secret-mappings", args.secret_mappings.as_ref())?,
                );
                input.text("slug", args.slug.as_ref());
                input.text("transport", args.transport.as_ref());
                input.text("url", args.url.as_ref());
                plan.input = input;
                Ok(plan)
            })(),
            GwMcpIntegrationsCommand::Update(args) => (|| {
                let id = uuid("mcpIntegrationId", &args.id)?;
                let mut plan = WritePlan::new(Write::McpUpdate(id), format);
                plan.force = args.force;
                let mut input = structured(&args.input)?;
                input.text("auth_type", args.auth_type.as_ref());
                input.named(
                    "configurations",
                    json_flag("--configurations", args.configurations.as_ref())?,
                );
                input.text("description", args.description.as_ref());
                input.text("name", args.name.as_ref());
                input.named(
                    "secret_mappings",
                    json_flag("--secret-mappings", args.secret_mappings.as_ref())?,
                );
                input.text("transport", args.transport.as_ref());
                input.text("url", args.url.as_ref());
                plan.input = input;
                Ok(plan)
            })(),
            GwMcpIntegrationsCommand::Delete(args) => uuid("mcpIntegrationId", &args.id)
                .and_then(|id| removal(format, Write::McpDelete(id), args.force)),
            GwMcpIntegrationsCommand::Capabilities(group) => match group.command.as_ref()? {
                GwMcpCapabilitiesCommand::Set(args) => (|| {
                    let id = uuid("mcpIntegrationId", &args.id)?;
                    let mut plan = WritePlan::new(Write::CapabilitiesSet(id), format);
                    plan.force = args.force;
                    let mut input = structured(&args.input)?;
                    if !args.capability.is_empty() {
                        input.named(
                            "capabilities",
                            Some(body::capability_bindings("--capability", &args.capability)?),
                        );
                    }
                    plan.input = input;
                    gate(&plan)?;
                    Ok(plan)
                })(),
                _ => return None,
            },
            GwMcpIntegrationsCommand::Workspaces(group) => match group.command.as_ref()? {
                GwMcpWorkspacesCommand::Set(args) => (|| {
                    let id = uuid("mcpIntegrationId", &args.id)?;
                    let mut plan = WritePlan::new(Write::McpWorkspacesSet(id), format);
                    plan.force = args.force;
                    let mut input = structured(&args.input)?;
                    input.named(
                        "global_workspace_access.enabled",
                        bool_flag("--global-access", args.global_access.as_ref())?,
                    );
                    input.named(
                        "override_existing_workspace_access",
                        Some(Value::Bool(!args.preserve_existing)),
                    );
                    if !args.workspace_binding.is_empty() {
                        input.named(
                            "workspaces",
                            Some(body::id_bindings(
                                "--workspace-binding",
                                &args.workspace_binding,
                            )?),
                        );
                    }
                    plan.input = input;
                    gate(&plan)?;
                    Ok(plan)
                })(),
                _ => return None,
            },
            _ => return None,
        },
    })
}

/// `organisations self update`, `auth-settings update` (§19.12).
fn plan_organisations(
    command: &GwOrganisationsCommand,
    format: Option<Format>,
) -> Option<Result<WritePlan, CliError>> {
    Some(match command {
        GwOrganisationsCommand::SelfOrg(group) => match group.command.as_ref()? {
            GwOrganisationSelfCommand::Update(args) => (|| {
                let mut plan = WritePlan::new(Write::OrganisationUpdate, format);
                plan.force = args.force;
                let mut input = structured(&args.input)?;
                input.text("name", args.name.as_ref());
                plan.input = input;
                gate(&plan)?;
                Ok(plan)
            })(),
            _ => return None,
        },
        GwOrganisationsCommand::AuthSettings(group) => match group.command.as_ref()? {
            GwAuthSettingsCommand::Update(args) => (|| {
                let mut plan = WritePlan::new(Write::AuthSettingsUpdate, format);
                plan.organisation = tsg("tsgId", args.tsg.tsg_id.as_ref())?;
                plan.force = args.force;
                let mut input = structured(&args.input)?;
                input.named(
                    "auth_settings",
                    json_flag("--auth-settings", args.auth_settings.as_ref())?,
                );
                input.named("domains", csv_flag("--domains", args.domains.as_ref())?);
                if args.scim_token.is_some() {
                    plan.warnings.push(inline_warning("--scim-token"));
                }
                input.text("scim_token", args.scim_token.as_ref());
                plan.input = input;
                gate(&plan)?;
                Ok(plan)
            })(),
            _ => return None,
        },
        _ => return None,
    })
}

/// `plugins create` (§19.12).
fn plan_plugins(
    command: &GwPluginsCommand,
    format: Option<Format>,
) -> Option<Result<WritePlan, CliError>> {
    Some(match command {
        GwPluginsCommand::Create(args) => (|| {
            let mut plan = WritePlan::new(Write::PluginCreate, format);
            plan.organisation = organisation(&args.organisation)?;
            plan.org_body = true;
            let mut input = structured(&args.input)?;
            if !args.credential.is_empty() {
                plan.warnings.push(inline_warning("--credential"));
                input.named(
                    "credentials",
                    Some(body::string_map("--credential", &args.credential)?),
                );
            }
            input.named(
                "integration_id",
                opt_uuid("integrationId", args.integration_id.as_ref())?,
            );
            input.named("organisation_id", text(plan.organisation.as_ref()));
            plan.input = input;
            Ok(plan)
        })(),
        _ => return None,
    })
}

/// `admin-guardrails create|update|delete` and the `mcp-servers` writes (§19.13).
fn plan_admin_guardrails(
    command: &GwAdminGuardrailsCommand,
    format: Option<Format>,
) -> Option<Result<WritePlan, CliError>> {
    Some(match command {
        GwAdminGuardrailsCommand::Create(args) => (|| {
            let mut plan = WritePlan::new(Write::AdminGuardrailCreate, format);
            let mut input = structured(&args.input)?;
            input.text("name", args.name.as_ref());
            input.named("checks", json_flag("--checks", args.checks.as_ref())?);
            input.named("actions", json_flag("--actions", args.actions.as_ref())?);
            input.text("target", args.target.as_ref());
            input.named(
                "workspace_id",
                opt_uuid("workspaceId", args.workspace.as_ref())?,
            );
            input.named(
                "organisation_id",
                opt_uuid("organisationId", args.organisation.as_ref())?,
            );
            plan.input = input;
            Ok(plan)
        })(),
        GwAdminGuardrailsCommand::Update(args) => (|| {
            let id = uuid("guardrailId", &args.id)?;
            let mut plan = WritePlan::new(Write::AdminGuardrailUpdate(id), format);
            let mut input = structured(&args.input)?;
            input.text("name", args.name.as_ref());
            input.named("checks", json_flag("--checks", args.checks.as_ref())?);
            input.named("actions", json_flag("--actions", args.actions.as_ref())?);
            plan.input = input;
            Ok(plan)
        })(),
        GwAdminGuardrailsCommand::Delete(args) => uuid("guardrailId", &args.id)
            .and_then(|id| removal(format, Write::AdminGuardrailDelete(id), args.force)),
        GwAdminGuardrailsCommand::McpServers(group) => match group.command.as_ref()? {
            GwAdminMcpServersCommand::Sync(args) => (|| {
                let id = uuid("guardrailId", &args.id)?;
                let mut plan = WritePlan::new(Write::McpSync(id), format);
                plan.force = args.force;
                plan.input = structured(&args.input)?;
                gate(&plan)?;
                Ok(plan)
            })(),
            GwAdminMcpServersCommand::Upsert(args) => (|| {
                let id = uuid("guardrailId", &args.id)?;
                let server = uuid("mcpServerId", &args.server)?;
                let mut plan = WritePlan::new(Write::McpUpsert(id, server), format);
                plan.input = structured(&args.input)?;
                Ok(plan)
            })(),
            _ => return None,
        },
        _ => return None,
    })
}

enum KeyLeaf<'a> {
    Update(&'a crate::cli::GwKeyUpdateArgs),
    Rotate(&'a crate::cli::GwKeyRotateArgs),
    Delete(&'a crate::cli::GwIdForceArgs),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::FromArgMatches;

    const U: &str = "11111111-1111-4111-8111-000000000301";

    fn plan_of(args: &[&str]) -> Result<Plan, CliError> {
        let mut full = vec!["airs", "gateway"];
        full.extend_from_slice(args);
        let matches = crate::cli::command().try_get_matches_from(&full).unwrap();
        crate::commands::plan(&Cli::from_arg_matches(&matches).unwrap())
    }

    fn write_of(args: &[&str]) -> WritePlan {
        match plan_of(args) {
            Ok(Plan::Gateway(plan)) => match *plan {
                GwPlan::Write(plan) => *plan,
                _ => panic!("not a write: {args:?}"),
            },
            Ok(_) => panic!("not a gateway plan: {args:?}"),
            Err(error) => panic!("{args:?}: {error:?}"),
        }
    }

    fn usage(args: &[&str]) -> String {
        match plan_of(args) {
            Err(CliError::Usage(message)) => message,
            other => panic!("expected a usage error for {args:?}: {:?}", other.is_ok()),
        }
    }

    #[test]
    fn named_flags_map_to_typescript_paths() {
        let plan = write_of(&[
            "guardrails",
            "create",
            "--workspace",
            U,
            "--name",
            "g1",
            "--checks",
            r#"[{"id":"x"}]"#,
            "--actions",
            r#"{"deny":true}"#,
            "--set",
            "actions.async=false",
        ]);
        assert_eq!(plan.write, Write::GuardrailCreate);
        assert_eq!(
            body::build(&plan.input).unwrap(),
            json!({"workspace_id": U, "name": "g1", "checks": [{"id": "x"}], "actions": {"deny": true, "async": false}})
        );
        let plan = write_of(&[
            "integrations",
            "workspaces",
            "set",
            U,
            "--workspace-binding",
            "ws-a=true",
            "--global-access",
            "false",
            "--force",
        ]);
        assert_eq!(
            body::build(&plan.input).unwrap(),
            json!({"workspaces": [{"id": "ws-a", "enabled": true}], "global_workspace_access": {"enabled": false}, "override_existing_workspace_access": true})
        );
        let plan = write_of(&[
            "integrations",
            "create",
            "--ai-provider",
            "OpenAI",
            "--key",
            "FAKE-K",
            "--base-url",
            "http://127.0.0.1:9/v1",
            "--header",
            "x-a=1",
            "--name",
            "i",
        ]);
        assert_eq!(plan.provider.as_deref(), Some("OpenAI"));
        assert_eq!(plan.warnings.len(), 2);
        assert_eq!(
            body::build(&plan.input).unwrap(),
            json!({"key": "FAKE-K", "name": "i", "configurations": {"provider_auth_type": "apiKey", "custom_host": "http://127.0.0.1:9/v1", "custom_headers": {"x-a": "1"}}})
        );
    }

    /// D232, D233, D237, D245, D247: every check is a usage error during planning.
    #[test]
    fn checks_come_first() {
        let secret = "--secret-output";
        for (args, message) in [
            (
                &["configs", "update", "c-1", "--name", "n"][..],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid configId: c-1",
            ),
            (
                &["configs", "create", "--workspace", "w"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid workspaceId: w",
            ),
            (
                &["configs", "create", "--set", "FAKE-NOEQUALS"],
                "Invalid --set: Expected path=value",
            ),
            (
                &["api-keys", "service", "create", "--name", "k"],
                secrets::NEEDS_DESTINATION,
            ),
            (
                &[
                    "api-keys",
                    "user",
                    "create",
                    "--show-secret",
                    secret,
                    "k.json",
                ],
                secrets::SHOW_SECRET,
            ),
            (
                &["api-keys", "service", "create", secret, ""],
                "--secret-output needs a path",
            ),
            (
                &["api-keys", "service", "rotate", U, secret, "-", "--force"],
                "--secret-output - would print the credential; give a file path",
            ),
            (
                &[
                    "api-keys",
                    "service",
                    "create",
                    secret,
                    "k.json",
                    "--organisation-id",
                    "x1",
                ],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid organisationId: x1",
            ),
            (
                &[
                    "api-keys",
                    "service",
                    "rotate",
                    U,
                    secret,
                    "k.json",
                    "--transition-ms",
                    "1.5",
                    "--force",
                ],
                "Invalid --transition-ms: Expected an integer",
            ),
            (
                &[
                    "api-keys",
                    "user",
                    "update",
                    U,
                    "--expires-at",
                    "2027-01-01",
                ],
                "Invalid --expires-at: Expected an ISO-8601 timestamp",
            ),
            (
                &["deployments", "update", U, "--is-default", "yes"],
                "Invalid --is-default: Expected true or false",
            ),
            (
                &[
                    "deployments",
                    "update",
                    U,
                    "--rotate-auth",
                    "true",
                    "--force",
                ],
                secrets::NEEDS_DESTINATION,
            ),
            (
                &["deployments", "create", "--name", "d"],
                secrets::NEEDS_DESTINATION,
            ),
            (
                &["integrations", "create", "--key", "a", "--key-stdin"],
                "Use only one of --key, --key-stdin, or --key-file",
            ),
            (
                &[
                    "integrations",
                    "create",
                    "--ai-provider",
                    "x",
                    "--ai-provider-id",
                    U,
                ],
                "Use --ai-provider or --ai-provider-id, not both",
            ),
            (
                &[
                    "integrations",
                    "create",
                    "--base-url",
                    "ftp://h/v1",
                    "--key",
                    "k",
                ],
                "Invalid --base-url: AISEC_USER_REQUEST_PAYLOAD_ERROR:custom host must use http or https",
            ),
            (
                &[
                    "integrations",
                    "create",
                    "--base-url",
                    "not a url",
                    "--key",
                    "k",
                ],
                "Invalid --base-url: AISEC_USER_REQUEST_PAYLOAD_ERROR:custom host must be an absolute http(s) URL including its API prefix",
            ),
            (
                &[
                    "integrations",
                    "create",
                    "--header",
                    "x=FAKE-1",
                    "--header",
                    "x=FAKE-2",
                    "--key",
                    "k",
                ],
                "Invalid --header: Duplicate key: x",
            ),
            (
                &["plugins", "create", "--credential", "constructor=x"],
                "Invalid --credential: Unsafe key: constructor",
            ),
            (
                &[
                    "mcp",
                    "integrations",
                    "capabilities",
                    "set",
                    U,
                    "--capability",
                    "tool=true",
                    "--force",
                ],
                "Invalid --capability: Expected type:name=enabled",
            ),
            (
                &[
                    "integrations",
                    "models",
                    "set",
                    U,
                    "--model",
                    "gpt",
                    "--force",
                ],
                "Invalid --model: Expected slug=value",
            ),
            (
                &[
                    "organisations",
                    "auth-settings",
                    "update",
                    "--tsg-id",
                    "1x",
                    "--force",
                ],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid tsgId: 1x",
            ),
            (
                &["admin-guardrails", "create", "--organisation", "100"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid organisationId: 100",
            ),
            (
                &["admin-guardrails", "mcp-servers", "upsert", U, "s"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid mcpServerId: s",
            ),
            (
                &["providers", "create", "--integration-id", "i"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid integrationId: i",
            ),
            (
                &[
                    "api-keys",
                    "user",
                    "create",
                    secret,
                    "k.json",
                    "--user-id",
                    "u",
                ],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid userId: u",
            ),
        ] {
            assert_eq!(usage(args), message, "{args:?}");
        }
    }

    /// D243: the always-confirming writes refuse without `--force` off a terminal (the
    /// test process has no terminal).
    #[test]
    fn non_interactive_refusals() {
        for (args, action) in [
            (
                &["configs", "delete", U][..],
                "permanently delete config 11111111-1111-4111-8111-000000000301",
            ),
            (
                &["api-keys", "user", "delete", U],
                "revoke user API key 11111111-1111-4111-8111-000000000301",
            ),
            (
                &["deployments", "archive", U],
                "archive deployment 11111111-1111-4111-8111-000000000301",
            ),
            (
                &["organisations", "self", "update", "--name", "n"],
                "update the current organisation",
            ),
            (
                &["admin-guardrails", "mcp-servers", "sync", U],
                "replace all MCP mappings on admin guardrail 11111111-1111-4111-8111-000000000301",
            ),
        ] {
            assert_eq!(
                usage(args),
                format!("refusing to {action} without --force in non-interactive mode"),
                "{args:?}"
            );
        }
    }
}
