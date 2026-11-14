//! The gateway writes of chunk 3 other than workspaces and scopes (DESIGN §19.3 "Writes
//! and outcomes", "Confirmations", §19.10, §19.13): what each sends, on which plane, its
//! outcome words and hints, its acknowledgement, its prompt, the withholding policy of
//! its answer, the request schema of its body and the one SDK call that sends it.
//!
//! A body is validated with its SDK request schema before the tenant ([`Typed::parse`],
//! the D116 usage text) and sent with the typed call ([`Typed::send`]) under one attempt
//! (D228): the SDK never re-sends a write; `writes::attempt` replays a plain 401 once.

use super::GwPlane;
use super::body;
use super::inventory::KeyKind;
use super::withhold::{self, Policy};
use crate::error::{CliError, SecretSet};
use crate::json::{OValue, page_value};
use crate::writes::OutcomeLines;
use scm::ErrorKind;
use scm::airs::gateway::AIGatewayClient;
use scm::airs::gateway::models::*;
use serde_json::Value;

/// One gateway write of §19.10 and §19.13 (the secret writes of §19.11 included).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Write {
    ConfigCreate,
    ConfigUpdate(String),
    ConfigDelete(String),
    GuardrailCreate,
    GuardrailUpdate(String),
    GuardrailDelete(String),
    ProviderCreate,
    ProviderUpdate(String),
    ProviderDelete(String),
    KeyCreate(KeyKind),
    KeyUpdate(KeyKind, String),
    KeyRotate(KeyKind, String),
    KeyDelete(KeyKind, String),
    DeploymentCreate,
    DeploymentUpdate(String),
    DeploymentArchive(String),
    IntegrationCreate,
    IntegrationUpdate(String),
    IntegrationDelete(String),
    ModelsSet(String),
    IntegrationWorkspacesSet(String),
    McpCreate,
    McpUpdate(String),
    McpDelete(String),
    CapabilitiesSet(String),
    McpWorkspacesSet(String),
    OrganisationUpdate,
    AuthSettingsUpdate,
    PluginCreate,
    AdminGuardrailCreate,
    AdminGuardrailUpdate(String),
    AdminGuardrailDelete(String),
    McpSync(String),
    McpUpsert(String, String),
}

impl Write {
    /// Static log identity. Resource IDs, routes and user input never enter it.
    pub fn diagnostic_operation(&self) -> &'static str {
        match self {
            Self::ConfigCreate => "gateway.configs.create",
            Self::ConfigUpdate(_) => "gateway.configs.update",
            Self::ConfigDelete(_) => "gateway.configs.delete",
            Self::GuardrailCreate => "gateway.guardrails.create",
            Self::GuardrailUpdate(_) => "gateway.guardrails.update",
            Self::GuardrailDelete(_) => "gateway.guardrails.delete",
            Self::ProviderCreate => "gateway.virtual_keys.create",
            Self::ProviderUpdate(_) => "gateway.virtual_keys.update",
            Self::ProviderDelete(_) => "gateway.virtual_keys.delete",
            Self::DeploymentCreate => "gateway.deployments.create",
            Self::DeploymentUpdate(_) => "gateway.deployments.update",
            Self::IntegrationCreate => "gateway.integrations.create",
            Self::IntegrationUpdate(_) => "gateway.integrations.update",
            Self::IntegrationDelete(_) => "gateway.integrations.delete",
            Self::McpCreate => "gateway.mcp_servers.create",
            Self::McpUpdate(_) => "gateway.mcp_servers.update",
            Self::McpDelete(_) => "gateway.mcp_servers.delete",
            Self::AdminGuardrailCreate => "gateway.admin_guardrails.create",
            Self::AdminGuardrailUpdate(_) => "gateway.admin_guardrails.update",
            Self::AdminGuardrailDelete(_) => "gateway.admin_guardrails.delete",
            Self::DeploymentArchive(_) => "gateway.deployments.archive",
            Self::KeyCreate(KeyKind::Service) => "gateway.api_keys.service.create",
            Self::KeyCreate(KeyKind::User) => "gateway.api_keys.user.create",
            Self::KeyUpdate(_, _) => "gateway.api_keys.update",
            Self::KeyRotate(_, _) => "gateway.api_keys.rotate",
            Self::KeyDelete(_, _) => "gateway.api_keys.delete",
            Self::ModelsSet(_) => "gateway.integrations.models.set",
            Self::IntegrationWorkspacesSet(_) => "gateway.integrations.workspaces.set",
            Self::CapabilitiesSet(_) => "gateway.mcp_servers.capabilities.set",
            Self::McpWorkspacesSet(_) => "gateway.mcp_servers.workspaces.set",
            Self::OrganisationUpdate => "gateway.organisations.update",
            Self::AuthSettingsUpdate => "gateway.organisations.auth_settings.update",
            Self::PluginCreate => "gateway.plugins.create",
            Self::McpSync(_) => "gateway.mcp_servers.sync",
            Self::McpUpsert(_, _) => "gateway.mcp_servers.upsert",
        }
    }
}

/// What a write does, for its outcome lines and acknowledgement (§19.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Words {
    /// The thing without its id (`config`, `service API key`, `model bindings of
    /// integration <id>`).
    pub noun: String,
    /// The id the command names (appended to the thing and the acknowledgement).
    pub id: Option<String>,
    /// `created`, `updated`, `deleted`, `revoked`, `replaced`, ….
    pub done: &'static str,
    /// Whether a create's acknowledgement names the answer's `id`.
    pub answer_id: bool,
    /// The hint after an unknown or unreadable outcome ([`OutcomeLines::hint`]): private,
    /// so the only `Words` outside this module are [`Write::words`]'s, which always carry
    /// it (choice 143).
    hint: Option<String>,
}

impl Words {
    fn new(noun: impl Into<String>, id: Option<&str>, done: &'static str) -> Self {
        Self {
            noun: noun.into(),
            id: id.map(str::to_owned),
            done,
            answer_id: false,
            hint: None,
        }
    }

    fn created(noun: &str) -> Self {
        Self {
            answer_id: true,
            ..Self::new(noun, None, "created")
        }
    }

    /// `<noun>[ <id>]`.
    pub fn thing(&self) -> String {
        match &self.id {
            Some(id) => format!("{} {}", self.noun, id),
            None => self.noun.clone(),
        }
    }

    /// `<Noun> <done>[: <id>]` (the stderr `✓` line; `answer` is the answer's `id`).
    pub fn ack(&self, answer: Option<&str>) -> String {
        let mut noun = self.noun.clone();
        if let Some(first) = noun.get(..1) {
            let upper = first.to_ascii_uppercase();
            noun.replace_range(..1, &upper);
        }
        let id = self.id.as_deref().or(answer.filter(|_| self.answer_id));
        match id {
            Some(id) => format!("{noun} {}: {id}", self.done),
            None => format!("{noun} {}", self.done),
        }
    }
}

impl OutcomeLines for Words {
    /// `Outcome unknown: the <thing> may have been <done>.`
    fn unknown_line(&self) -> String {
        format!(
            "Outcome unknown: the {} may have been {}.",
            self.thing(),
            self.done
        )
    }

    /// `Outcome: the <thing> was <done>, but the response could not be read.`
    fn unreadable_line(&self) -> String {
        format!(
            "Outcome: the {} was {}, but the response could not be read.",
            self.thing(),
            self.done
        )
    }

    /// The write's hint ([`Write::words`]).
    fn hint(&self) -> Option<String> {
        self.hint.clone()
    }
}

/// A confirmation: the prompt (asked after the tenant) and the non-interactive refusal
/// action (`refusing to <action> without --force in non-interactive mode`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Confirm {
    pub prompt: String,
    pub action: String,
}

impl Confirm {
    /// A TypeScript prompt whose refusal is the prompt without `?`, its first letter
    /// lowercased (TS `runConfirmedWrite`), with `extra` appended to the prompt only.
    fn ts(prompt: String, extra: &str) -> Self {
        let mut action = prompt.trim_end_matches('?').to_owned();
        if let Some(first) = action.get(..1) {
            let lower = first.to_ascii_lowercase();
            action.replace_range(..1, &lower);
        }
        Self {
            prompt: format!("{prompt}{extra}"),
            action,
        }
    }
}

/// `service` or `user`.
pub fn kind_name(kind: KeyKind) -> &'static str {
    match kind {
        KeyKind::Service => "service",
        KeyKind::User => "user",
    }
}

/// Members of an integration update body that carry a provider credential (§19.3).
const INTEGRATION_SECRETS: [&str; 4] = [
    "azure_entra_client_secret",
    "aws_secret_access_key",
    "vertex_service_account_json",
    "custom_headers",
];

/// Members of an MCP integration update's `configurations` that carry a credential.
const MCP_SECRETS: [&str; 4] = [
    "custom_headers",
    "client_secret",
    "oauth_client_secret",
    "oauth_metadata",
];

fn has(body: &Value, key: &str) -> bool {
    body.get(key).is_some()
}

fn configurations_have(body: &Value, keys: &[&str]) -> bool {
    body.get("configurations")
        .and_then(Value::as_object)
        .is_some_and(|map| keys.iter().any(|key| map.contains_key(*key)))
}

impl Write {
    /// The plane of the write (the grant hint, §19.3).
    pub fn plane(&self) -> GwPlane {
        match self {
            Self::ConfigCreate
            | Self::ConfigUpdate(_)
            | Self::ConfigDelete(_)
            | Self::GuardrailCreate
            | Self::GuardrailUpdate(_)
            | Self::GuardrailDelete(_)
            | Self::ProviderCreate
            | Self::ProviderUpdate(_)
            | Self::ProviderDelete(_)
            | Self::KeyCreate(_)
            | Self::KeyUpdate(..)
            | Self::KeyRotate(..)
            | Self::KeyDelete(..) => GwPlane::Data,
            _ => GwPlane::Admin,
        }
    }

    /// `(METHOD, path)` relative to the plane (the D116 text; the G36 raw writes).
    pub fn route(&self, tsg: Option<&str>) -> (&'static str, String) {
        let kind = |kind: &KeyKind| kind_name(*kind);
        match self {
            Self::ConfigCreate => ("POST", "/configs".into()),
            Self::ConfigUpdate(id) => ("PUT", format!("/configs/{id}")),
            Self::ConfigDelete(id) => ("DELETE", format!("/configs/{id}")),
            Self::GuardrailCreate => ("POST", "/guardrails".into()),
            Self::GuardrailUpdate(id) => ("PUT", format!("/guardrails/{id}")),
            Self::GuardrailDelete(id) => ("DELETE", format!("/guardrails/{id}")),
            Self::ProviderCreate => ("POST", "/providers".into()),
            Self::ProviderUpdate(id) => ("PUT", format!("/providers/{id}")),
            Self::ProviderDelete(id) => ("DELETE", format!("/providers/{id}")),
            Self::KeyCreate(k) => ("POST", format!("/api-keys/{}", kind(k))),
            Self::KeyUpdate(k, id) => ("PUT", format!("/api-keys/{}/{id}", kind(k))),
            Self::KeyRotate(k, id) => ("POST", format!("/api-keys/{}/{id}/rotate", kind(k))),
            Self::KeyDelete(k, id) => ("DELETE", format!("/api-keys/{}/{id}", kind(k))),
            Self::DeploymentCreate => ("POST", "/deployments".into()),
            Self::DeploymentUpdate(id) => ("PUT", format!("/deployments/{id}")),
            Self::DeploymentArchive(id) => ("DELETE", format!("/deployments/{id}")),
            Self::IntegrationCreate => ("POST", "/integrations".into()),
            Self::IntegrationUpdate(id) => ("PUT", format!("/integrations/{id}")),
            Self::IntegrationDelete(id) => ("DELETE", format!("/integrations/{id}")),
            Self::ModelsSet(id) => ("PUT", format!("/integrations/{id}/models")),
            Self::IntegrationWorkspacesSet(id) => ("PUT", format!("/integrations/{id}/workspaces")),
            Self::McpCreate => ("POST", "/mcp-integrations".into()),
            Self::McpUpdate(id) => ("PUT", format!("/mcp-integrations/{id}")),
            Self::McpDelete(id) => ("DELETE", format!("/mcp-integrations/{id}")),
            Self::CapabilitiesSet(id) => ("PUT", format!("/mcp-integrations/{id}/capabilities")),
            Self::McpWorkspacesSet(id) => ("PUT", format!("/mcp-integrations/{id}/workspaces")),
            Self::OrganisationUpdate => ("PUT", "/organisations/self".into()),
            Self::AuthSettingsUpdate => (
                "PUT",
                format!("/organisations/{}/auth-settings", tsg.unwrap_or("<tsg>")),
            ),
            Self::PluginCreate => ("POST", "/plugins".into()),
            Self::AdminGuardrailCreate => ("POST", "/guardrails".into()),
            Self::AdminGuardrailUpdate(id) => ("PUT", format!("/guardrails/{id}")),
            Self::AdminGuardrailDelete(id) => ("DELETE", format!("/guardrails/{id}")),
            Self::McpSync(id) => ("PUT", format!("/guardrails/{id}/mcp-servers")),
            Self::McpUpsert(id, server) => {
                ("PUT", format!("/guardrails/{id}/mcp-servers/{server}"))
            }
        }
    }

    /// Whether the write removes, archives or revokes (stdout stays empty, D244).
    pub fn is_removal(&self) -> bool {
        matches!(
            self,
            Self::ConfigDelete(_)
                | Self::GuardrailDelete(_)
                | Self::ProviderDelete(_)
                | Self::KeyDelete(..)
                | Self::DeploymentArchive(_)
                | Self::IntegrationDelete(_)
                | Self::McpDelete(_)
                | Self::AdminGuardrailDelete(_)
        )
    }

    /// Whether the write takes a structured body.
    pub fn has_body(&self) -> bool {
        !self.is_removal()
    }

    /// The outcome words and hint (§19.3 table; `tsg` the organisation of `auth-settings
    /// update`, `workspace` the create's workspace UUID when the body names one).
    pub fn words(&self, tsg: &str, workspace: Option<&str>) -> Words {
        Words {
            hint: Some(self.outcome_hint(workspace)),
            ..self.outcome_words(tsg)
        }
    }

    /// [`Self::words`] without the hint.
    fn outcome_words(&self, tsg: &str) -> Words {
        fn id(id: &str) -> Option<&str> {
            Some(id)
        }
        match self {
            Self::ConfigCreate => Words::created("config"),
            Self::ConfigUpdate(i) => Words::new("config", id(i), "updated"),
            Self::ConfigDelete(i) => Words::new("config", id(i), "deleted"),
            Self::GuardrailCreate => Words::created("guardrail"),
            Self::GuardrailUpdate(i) => Words::new("guardrail", id(i), "updated"),
            Self::GuardrailDelete(i) => Words::new("guardrail", id(i), "deleted"),
            Self::ProviderCreate => Words::created("provider"),
            Self::ProviderUpdate(i) => Words::new("provider", id(i), "updated"),
            Self::ProviderDelete(i) => Words::new("provider", id(i), "deleted"),
            Self::KeyCreate(k) => Words::created(&format!("{} API key", kind_name(*k))),
            Self::KeyUpdate(k, i) => {
                Words::new(format!("{} API key", kind_name(*k)), id(i), "updated")
            }
            Self::KeyRotate(k, i) => {
                Words::new(format!("{} API key", kind_name(*k)), id(i), "rotated")
            }
            Self::KeyDelete(k, i) => {
                Words::new(format!("{} API key", kind_name(*k)), id(i), "revoked")
            }
            Self::DeploymentCreate => Words {
                answer_id: true,
                ..Words::new("deployment", None, "registered")
            },
            Self::DeploymentUpdate(i) => Words::new("deployment", id(i), "updated"),
            Self::DeploymentArchive(i) => Words::new("deployment", id(i), "archived"),
            Self::IntegrationCreate => Words::created("integration"),
            Self::IntegrationUpdate(i) => Words::new("integration", id(i), "updated"),
            Self::IntegrationDelete(i) => Words::new("integration", id(i), "deleted"),
            Self::ModelsSet(i) => Words::new(
                format!("model bindings of integration {i}"),
                None,
                "replaced",
            ),
            Self::IntegrationWorkspacesSet(i) => Words::new(
                format!("workspace bindings of integration {i}"),
                None,
                "replaced",
            ),
            Self::McpCreate => Words::created("MCP integration"),
            Self::McpUpdate(i) => Words::new("MCP integration", id(i), "updated"),
            Self::McpDelete(i) => Words::new("MCP integration", id(i), "deleted"),
            Self::CapabilitiesSet(i) => {
                Words::new(format!("MCP capabilities of {i}"), None, "replaced")
            }
            Self::McpWorkspacesSet(i) => {
                Words::new(format!("MCP workspace access of {i}"), None, "replaced")
            }
            Self::OrganisationUpdate => Words::new("current organisation", None, "updated"),
            Self::AuthSettingsUpdate => Words::new(
                format!("authentication settings of organisation {tsg}"),
                None,
                "replaced",
            ),
            Self::PluginCreate => Words::created("plugin"),
            Self::AdminGuardrailCreate => Words::created("admin guardrail"),
            Self::AdminGuardrailUpdate(i) => Words::new("admin guardrail", id(i), "updated"),
            Self::AdminGuardrailDelete(i) => Words::new("admin guardrail", id(i), "deleted"),
            Self::McpSync(i) => Words::new(
                format!("MCP mappings of admin guardrail {i}"),
                None,
                "replaced",
            ),
            Self::McpUpsert(i, server) => Words::new(
                format!("MCP mapping {server} of admin guardrail {i}"),
                None,
                "set",
            ),
        }
    }

    /// The hint after an unknown or unreadable outcome (§19.3 table), carried by
    /// [`Self::words`]; `workspace` the create's workspace UUID, when the body names one.
    fn outcome_hint(&self, workspace: Option<&str>) -> String {
        let p = crate::store::PROGRAM;
        let ws = workspace.unwrap_or("<uuid>");
        let get = |group: &str, id: &str| format!("Check it with '{p} gateway {group} get {id}'.");
        let list =
            |group: &str| format!("Check '{p} gateway {group} list' before creating it again.");
        match self {
            Self::ConfigCreate => format!(
                "Check '{p} gateway configs list --workspace {ws}' before creating it again."
            ),
            Self::GuardrailCreate => format!(
                "Check '{p} gateway guardrails list --workspace {ws}' before creating it again."
            ),
            Self::ProviderCreate => format!(
                "Check '{p} gateway providers list --workspace {ws}' before creating it again."
            ),
            Self::ConfigUpdate(i) | Self::ConfigDelete(i) => get("configs", i),
            Self::GuardrailUpdate(i) | Self::GuardrailDelete(i) => get("guardrails", i),
            Self::ProviderUpdate(i) | Self::ProviderDelete(i) => get("providers", i),
            Self::KeyUpdate(k, i) | Self::KeyDelete(k, i) => {
                get(&format!("api-keys {}", kind_name(*k)), i)
            }
            Self::KeyCreate(k) => format!(
                "Check '{p} gateway api-keys {} list --workspace {ws}' before creating another key; revoke a key whose credential was not saved.",
                kind_name(*k)
            ),
            Self::KeyRotate(k, i) => format!(
                "The old key stops working after its transition period; check '{p} gateway api-keys {} get {i}' and rotate it again to obtain a usable credential.",
                kind_name(*k)
            ),
            Self::DeploymentCreate => format!(
                "Check '{p} gateway deployments list'; a registration whose credentials were not saved needs '{p} gateway deployments update <id> --rotate-auth true --secret-output <new file>'."
            ),
            Self::DeploymentUpdate(i) | Self::DeploymentArchive(i) => get("deployments", i),
            Self::IntegrationCreate => list("integrations"),
            Self::McpCreate => list("mcp integrations"),
            Self::AdminGuardrailCreate => list("admin-guardrails"),
            Self::PluginCreate => list("plugins"),
            Self::IntegrationUpdate(i) | Self::IntegrationDelete(i) => get("integrations", i),
            Self::McpUpdate(i) | Self::McpDelete(i) => get("mcp integrations", i),
            Self::AdminGuardrailUpdate(i) | Self::AdminGuardrailDelete(i) => {
                get("admin-guardrails", i)
            }
            Self::ModelsSet(i) => {
                format!("Check them with '{p} gateway integrations models list {i}'.")
            }
            Self::IntegrationWorkspacesSet(i) => {
                format!("Check them with '{p} gateway integrations workspaces list {i}'.")
            }
            Self::CapabilitiesSet(i) => {
                format!("Check them with '{p} gateway mcp integrations capabilities list {i}'.")
            }
            Self::McpWorkspacesSet(i) => {
                format!("Check them with '{p} gateway mcp integrations workspaces list {i}'.")
            }
            Self::McpSync(i) | Self::McpUpsert(i, _) => {
                format!("Check them with '{p} gateway admin-guardrails mcp-servers list {i}'.")
            }
            Self::OrganisationUpdate => {
                format!("Check it with '{p} gateway organisations self get'.")
            }
            Self::AuthSettingsUpdate => {
                format!("Check it with '{p} gateway organisations auth-settings get'.")
            }
        }
    }

    /// The confirmation of the write for `body` (§19.3 table; `None`: no prompt).
    /// `tsg` the organisation of `auth-settings update` (`None` before the tenant without
    /// `--tsg-id`); `secret` the `--secret-output` path as shown.
    ///
    /// A workspace-binding replacement warns that unnamed bindings are removed unless the
    /// body that is sent says `override_existing_workspace_access: false` (the
    /// `--preserve-existing` flag, which a later `--set` or `--file` member overrides).
    pub fn confirm(
        &self,
        body: &Value,
        tsg: Option<&str>,
        secret: Option<&str>,
    ) -> Option<Confirm> {
        let removed = if body.get("override_existing_workspace_access") == Some(&Value::Bool(false))
        {
            ""
        } else {
            " Bindings not named are removed."
        };
        let reset = body.get("reset_usage") == Some(&Value::Bool(true));
        // Choice 178: an update whose body archives the deployment (`--status archived`, or the
        // same value through `--set` or `--file`) confirms as `deployments archive` does.
        let archives = body.get("status").and_then(Value::as_str) == Some("archived");
        // A deployment update that also replaces its authentication settings says so, whether it
        // rotates the authentication or archives the deployment.
        let settings = if has(body, "auth_settings") {
            " Its authentication settings are replaced too."
        } else {
            ""
        };
        let custom = |prompt: String, action: String| Confirm { prompt, action };
        Some(match self {
            Self::ConfigDelete(i) => Confirm::ts(format!("Permanently delete config {i}?"), ""),
            Self::GuardrailDelete(i) => {
                Confirm::ts(format!("Permanently delete guardrail {i}?"), "")
            }
            Self::ProviderDelete(i) => Confirm::ts(format!("Permanently delete provider {i}?"), ""),
            Self::IntegrationDelete(i) => {
                Confirm::ts(format!("Permanently delete integration {i}?"), "")
            }
            Self::McpDelete(i) => {
                Confirm::ts(format!("Permanently delete MCP integration {i}?"), "")
            }
            Self::AdminGuardrailDelete(i) => {
                Confirm::ts(format!("Permanently delete admin guardrail {i}?"), "")
            }
            Self::KeyDelete(k, i) => {
                Confirm::ts(format!("Revoke {} API key {i}?", kind_name(*k)), "")
            }
            Self::KeyRotate(k, i) => Confirm::ts(
                format!("Rotate {} API key {i}?", kind_name(*k)),
                &format!(
                    " The new key is written to {}.",
                    secret.unwrap_or("--secret-output")
                ),
            ),
            Self::DeploymentArchive(i) => Confirm::ts(format!("Archive deployment {i}?"), ""),
            Self::ModelsSet(i) => Confirm::ts(format!("Replace model bindings on {i}?"), ""),
            Self::IntegrationWorkspacesSet(i) => {
                Confirm::ts(format!("Replace workspace bindings on {i}?"), removed)
            }
            Self::CapabilitiesSet(i) => {
                Confirm::ts(format!("Replace enabled MCP capabilities on {i}?"), "")
            }
            Self::McpWorkspacesSet(i) => {
                Confirm::ts(format!("Replace MCP workspace access on {i}?"), removed)
            }
            Self::McpSync(i) => Confirm::ts(
                format!("Replace all MCP mappings on admin guardrail {i}?"),
                "",
            ),
            Self::KeyUpdate(k, i) if reset => custom(
                format!(
                    "Reset the accumulated usage of {} API key {i}?",
                    kind_name(*k)
                ),
                format!("reset the usage of {} API key {i}", kind_name(*k)),
            ),
            Self::ProviderUpdate(i) if reset => custom(
                format!("Reset the accumulated usage of provider {i}?"),
                format!("reset the usage of provider {i}"),
            ),
            Self::DeploymentUpdate(i) if body.get("rotate_auth") == Some(&Value::Bool(true)) => {
                let archived = if archives {
                    " The deployment is archived too."
                } else {
                    ""
                };
                custom(
                    format!(
                        "Rotate the authentication of deployment {i}? Its current credentials stop working; the new ones are written to {}.{settings}{archived}",
                        secret.unwrap_or("--secret-output")
                    ),
                    format!("rotate the authentication of deployment {i}"),
                )
            }
            Self::DeploymentUpdate(i) if archives => custom(
                format!("Archive deployment {i}? (status archived){settings}"),
                format!("archive deployment {i}"),
            ),
            Self::IntegrationUpdate(i)
                if has(body, "key")
                    || has(body, "secret_mappings")
                    || configurations_have(body, &INTEGRATION_SECRETS) =>
            {
                custom(
                    format!(
                        "Replace the provider credential of integration {i}? Calls through it use the new credential at once."
                    ),
                    format!("replace the provider credential of integration {i}"),
                )
            }
            Self::DeploymentUpdate(i) if has(body, "auth_settings") => custom(
                format!(
                    "Replace the authentication settings of deployment {i}? Clients of the deployment use them at once."
                ),
                format!("replace the authentication settings of deployment {i}"),
            ),
            Self::McpUpdate(i)
                if has(body, "auth_type")
                    || has(body, "secret_mappings")
                    || configurations_have(body, &MCP_SECRETS) =>
            {
                custom(
                    format!(
                        "Replace the authentication settings of MCP integration {i}? Calls through it use them at once."
                    ),
                    format!("replace the authentication settings of MCP integration {i}"),
                )
            }
            Self::OrganisationUpdate => custom(
                "Update the current organisation? The change applies to the whole tenant.".into(),
                "update the current organisation".into(),
            ),
            Self::AuthSettingsUpdate => {
                let organisation = tsg.map_or_else(
                    || "the selected tenant's organisation".to_owned(),
                    |tsg| format!("organisation {tsg}"),
                );
                custom(
                    format!(
                        "Replace the authentication settings of {organisation}? Single sign-on and SCIM provisioning use them at once."
                    ),
                    format!("replace the authentication settings of {organisation}"),
                )
            }
            _ => return None,
        })
    }

    /// The withholding policy of the answer and of the request body (§19.3).
    pub fn policy(&self) -> &'static Policy {
        match self {
            Self::ProviderCreate | Self::ProviderUpdate(_) => &withhold::PROVIDERS,
            Self::KeyCreate(_) | Self::KeyUpdate(..) | Self::KeyRotate(..) => &withhold::API_KEYS,
            Self::DeploymentCreate | Self::DeploymentUpdate(_) => &withhold::DEPLOYMENTS,
            Self::IntegrationCreate | Self::IntegrationUpdate(_) => &withhold::INTEGRATIONS,
            Self::McpCreate | Self::McpUpdate(_) => &withhold::MCP,
            Self::AuthSettingsUpdate => &withhold::AUTH_SETTINGS,
            Self::PluginCreate => &withhold::PLUGINS,
            _ => &withhold::PLAIN,
        }
    }

    /// The CLI refinements of the TS request schemas (G49, §19.3), before the schema.
    pub fn refine(&self, body: &Value) -> Result<(), CliError> {
        match self {
            Self::ConfigUpdate(_)
            | Self::GuardrailUpdate(_)
            | Self::ProviderUpdate(_)
            | Self::KeyUpdate(..)
            | Self::DeploymentUpdate(_)
            | Self::IntegrationUpdate(_)
            | Self::IntegrationWorkspacesSet(_)
            | Self::McpUpdate(_)
            | Self::McpWorkspacesSet(_)
            | Self::OrganisationUpdate
            | Self::AuthSettingsUpdate
            | Self::AdminGuardrailUpdate(_) => body::require_fields(body),
            Self::GuardrailCreate => {
                if body.get("target").and_then(Value::as_str) == Some("mcp_tools") {
                    return Ok(());
                }
                let mut parts = Vec::new();
                if !has(body, "checks") {
                    parts.push("checks: LLM guardrails require checks");
                }
                if !has(body, "actions") {
                    parts.push("actions: LLM guardrails require actions");
                }
                if parts.is_empty() {
                    Ok(())
                } else {
                    Err(CliError::usage(format!(
                        "{}: {}",
                        body::INVALID_REQUEST,
                        parts.join("; ")
                    )))
                }
            }
            Self::KeyRotate(..) => match body.get("key_transition_period_ms") {
                Some(Value::Number(ms)) if ms.as_f64().is_some_and(|ms| ms < 1_800_000.0) => {
                    Err(body::refinement(
                        "key_transition_period_ms",
                        "Number must be greater than or equal to 1800000",
                    ))
                }
                _ => Ok(()),
            },
            Self::ModelsSet(_) => non_empty_array(body, "models"),
            Self::CapabilitiesSet(_) => non_empty_array(body, "capabilities"),
            Self::PluginCreate => match body.get("credentials").and_then(Value::as_object) {
                Some(map) if !map.is_empty() => Ok(()),
                _ => Err(body::refinement(
                    "credentials",
                    "Provide at least one field",
                )),
            },
            _ => Ok(()),
        }
    }
}

/// TS's zod texts for a required non-empty array member: `<member>: Required` when the
/// body has no such member (the flag omitted), `<member>: Array must contain at least 1
/// element(s)` for an explicit `[]`, `<member>: Expected array, received <type>` otherwise.
fn non_empty_array(body: &Value, member: &str) -> Result<(), CliError> {
    let message = match body.get(member) {
        None => "Required".to_owned(),
        Some(Value::Array(items)) if !items.is_empty() => return Ok(()),
        Some(Value::Array(_)) => "Array must contain at least 1 element(s)".to_owned(),
        Some(other) => format!(
            "Expected array, received {}",
            match other {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                _ => "object",
            }
        ),
    };
    Err(body::refinement(member, &message))
}

/// A body validated with the write's SDK request schema (`Mode::Request`).
#[derive(Clone, Debug)]
pub enum Typed {
    None,
    ConfigCreate(GatewayConfigCreateRequest),
    ConfigUpdate(GatewayConfigUpdateRequest),
    GuardrailCreate(GatewayGuardrailCreateRequest),
    GuardrailUpdate(GatewayGuardrailUpdateRequest),
    ProviderCreate(GatewayProviderCreateRequest),
    ProviderUpdate(GatewayProviderUpdateRequest),
    ServiceKeyCreate(GatewayServiceApiKeyCreateRequest),
    UserKeyCreate(GatewayUserApiKeyCreateRequest),
    KeyUpdate(GatewayApiKeyUpdateRequest),
    KeyRotate(GatewayApiKeyRotateRequest),
    DeploymentCreate(GatewayDeploymentCreateRequest),
    DeploymentUpdate(GatewayDeploymentUpdateRequest),
    IntegrationCreate(GatewayIntegrationCreateRequest),
    IntegrationUpdate(GatewayIntegrationUpdateRequest),
    ModelsSet(GatewayIntegrationModelsBulkUpdateRequest),
    IntegrationWorkspacesSet(GatewayIntegrationWorkspacesBulkUpdateRequest),
    McpCreate(McpIntegrationCreateRequest),
    McpUpdate(McpIntegrationUpdateRequest),
    CapabilitiesSet(McpIntegrationCapabilitiesBulkUpdateRequest),
    McpWorkspacesSet(McpIntegrationWorkspacesBulkUpdateRequest),
    OrganisationUpdate(GatewayOrganisationUpdateRequest),
    AuthSettingsUpdate(GatewayOrganisationAuthSettingsUpdateRequest),
    PluginCreate(GatewayPluginCreateRequest),
    AdminGuardrailCreate(GatewayAdminGuardrailCreateRequest),
    AdminGuardrailUpdate(GatewayGuardrailUpdateRequest),
    McpSync(GatewayAdminGuardrailMcpSyncRequest),
    McpUpsert(GatewayAdminGuardrailMcpMappingRequest),
}

impl Typed {
    /// The refinements, then the SDK request schema of `write` (the D116 usage text).
    pub fn parse(write: &Write, body: &Value, tsg: Option<&str>) -> Result<Self, CliError> {
        write.refine(body)?;
        let (method, path) = write.route(tsg);
        macro_rules! v {
            ($parse:expr $(,)?) => {
                body::validate(body, method, &path, $parse)
            };
        }
        Ok(match write {
            Write::ConfigCreate => Self::ConfigCreate(v!(GatewayConfigCreateRequest::parse)?),
            Write::ConfigUpdate(_) => Self::ConfigUpdate(v!(GatewayConfigUpdateRequest::parse)?),
            Write::GuardrailCreate => {
                Self::GuardrailCreate(v!(GatewayGuardrailCreateRequest::parse)?)
            }
            Write::GuardrailUpdate(_) => {
                Self::GuardrailUpdate(v!(GatewayGuardrailUpdateRequest::parse)?)
            }
            Write::ProviderCreate => Self::ProviderCreate(v!(GatewayProviderCreateRequest::parse)?),
            Write::ProviderUpdate(_) => {
                Self::ProviderUpdate(v!(GatewayProviderUpdateRequest::parse)?)
            }
            Write::KeyCreate(KeyKind::Service) => {
                Self::ServiceKeyCreate(v!(GatewayServiceApiKeyCreateRequest::parse)?)
            }
            Write::KeyCreate(KeyKind::User) => {
                Self::UserKeyCreate(v!(GatewayUserApiKeyCreateRequest::parse)?)
            }
            Write::KeyUpdate(..) => Self::KeyUpdate(v!(GatewayApiKeyUpdateRequest::parse)?),
            Write::KeyRotate(..) => Self::KeyRotate(v!(GatewayApiKeyRotateRequest::parse)?),
            Write::DeploymentCreate => {
                Self::DeploymentCreate(v!(GatewayDeploymentCreateRequest::parse)?)
            }
            Write::DeploymentUpdate(_) => {
                Self::DeploymentUpdate(v!(GatewayDeploymentUpdateRequest::parse)?)
            }
            Write::IntegrationCreate => {
                Self::IntegrationCreate(v!(GatewayIntegrationCreateRequest::parse)?)
            }
            Write::IntegrationUpdate(_) => {
                Self::IntegrationUpdate(v!(GatewayIntegrationUpdateRequest::parse)?)
            }
            Write::ModelsSet(_) => {
                Self::ModelsSet(v!(GatewayIntegrationModelsBulkUpdateRequest::parse)?)
            }
            Write::IntegrationWorkspacesSet(_) => Self::IntegrationWorkspacesSet(v!(
                GatewayIntegrationWorkspacesBulkUpdateRequest::parse,
            )?),
            Write::McpCreate => Self::McpCreate(v!(McpIntegrationCreateRequest::parse)?),
            Write::McpUpdate(_) => Self::McpUpdate(v!(McpIntegrationUpdateRequest::parse)?),
            Write::CapabilitiesSet(_) => {
                Self::CapabilitiesSet(v!(McpIntegrationCapabilitiesBulkUpdateRequest::parse)?)
            }
            Write::McpWorkspacesSet(_) => {
                Self::McpWorkspacesSet(v!(McpIntegrationWorkspacesBulkUpdateRequest::parse)?)
            }
            Write::OrganisationUpdate => {
                Self::OrganisationUpdate(v!(GatewayOrganisationUpdateRequest::parse)?)
            }
            Write::AuthSettingsUpdate => {
                Self::AuthSettingsUpdate(v!(GatewayOrganisationAuthSettingsUpdateRequest::parse,)?)
            }
            Write::PluginCreate => Self::PluginCreate(v!(GatewayPluginCreateRequest::parse)?),
            Write::AdminGuardrailCreate => {
                Self::AdminGuardrailCreate(v!(GatewayAdminGuardrailCreateRequest::parse)?)
            }
            Write::AdminGuardrailUpdate(_) => {
                Self::AdminGuardrailUpdate(v!(GatewayGuardrailUpdateRequest::parse)?)
            }
            Write::McpSync(_) => Self::McpSync(v!(GatewayAdminGuardrailMcpSyncRequest::parse)?),
            Write::McpUpsert(..) => {
                Self::McpUpsert(v!(GatewayAdminGuardrailMcpMappingRequest::parse)?)
            }
            _ => Self::None,
        })
    }

    /// The wire JSON of a one-time credential write's body (G36, §19.11: sent raw so the
    /// credential is read from the answer before the rest of it is validated). Only the
    /// credential pairs of `write` and its validated body have one; any other pair is the
    /// D116 invalid-body error, so a raw write can never send a `null` or foreign body.
    pub fn credential_wire(&self, write: &Write) -> Result<Value, CliError> {
        let (method, route) = write.route(None);
        let wire = match (write, self) {
            (Write::KeyCreate(KeyKind::Service), Self::ServiceKeyCreate(b)) => {
                serde_json::to_value(b)
            }
            (Write::KeyCreate(KeyKind::User), Self::UserKeyCreate(b)) => serde_json::to_value(b),
            (Write::KeyRotate(..), Self::KeyRotate(b)) => serde_json::to_value(b),
            (Write::DeploymentCreate, Self::DeploymentCreate(b)) => serde_json::to_value(b),
            (Write::DeploymentUpdate(_), Self::DeploymentUpdate(b)) => serde_json::to_value(b),
            _ => return Err(crate::writes::invalid_body(method, &route)),
        };
        wire.map_err(|_| crate::writes::invalid_body(method, &route))
    }

    /// The one typed SDK call of `write` (never re-sent by the SDK); the answer as an
    /// ordered document (`Null` for a removal). `tsg` is the organisation of the paths
    /// and queries that carry one.
    pub async fn send(
        &self,
        gateway: &AIGatewayClient,
        write: &Write,
        tsg: &str,
        options: &scm::RequestOptions,
    ) -> scm::Result<OValue> {
        let o = Some(options);
        let null = |_: ()| OValue::Null;
        match (write, self) {
            (Write::ConfigCreate, Self::ConfigCreate(b)) => {
                page_value(&gateway.configs().create(b, o).await?)
            }
            (Write::ConfigUpdate(id), Self::ConfigUpdate(b)) => {
                page_value(&gateway.configs().update(id, b, o).await?)
            }
            (Write::ConfigDelete(id), _) => gateway.configs().delete(id, o).await.map(null),
            (Write::GuardrailCreate, Self::GuardrailCreate(b)) => {
                page_value(&gateway.guardrails().create(b, o).await?)
            }
            (Write::GuardrailUpdate(id), Self::GuardrailUpdate(b)) => {
                page_value(&gateway.guardrails().update(id, b, o).await?)
            }
            (Write::GuardrailDelete(id), _) => gateway.guardrails().delete(id, o).await.map(null),
            (Write::ProviderCreate, Self::ProviderCreate(b)) => {
                page_value(&gateway.providers().create(b, o).await?)
            }
            (Write::ProviderUpdate(id), Self::ProviderUpdate(b)) => {
                page_value(&gateway.providers().update(id, b, o).await?)
            }
            (Write::ProviderDelete(id), _) => gateway.providers().delete(id, o).await.map(null),
            (Write::KeyUpdate(KeyKind::Service, id), Self::KeyUpdate(b)) => {
                page_value(&gateway.api_keys().update_service(id, b, o).await?)
            }
            (Write::KeyUpdate(KeyKind::User, id), Self::KeyUpdate(b)) => {
                page_value(&gateway.api_keys().update_user(id, b, o).await?)
            }
            (Write::KeyDelete(KeyKind::Service, id), _) => {
                gateway.api_keys().delete_service(id, o).await.map(null)
            }
            (Write::KeyDelete(KeyKind::User, id), _) => {
                gateway.api_keys().delete_user(id, o).await.map(null)
            }
            (Write::DeploymentUpdate(id), Self::DeploymentUpdate(b)) => {
                page_value(&gateway.deployments().update(id, b, o).await?)
            }
            (Write::DeploymentArchive(id), _) => {
                gateway.deployments().delete(id, tsg, o).await.map(null)
            }
            (Write::IntegrationCreate, Self::IntegrationCreate(b)) => {
                page_value(&gateway.integrations().create(b, o).await?)
            }
            (Write::IntegrationUpdate(id), Self::IntegrationUpdate(b)) => {
                page_value(&gateway.integrations().update(id, b, o).await?)
            }
            (Write::IntegrationDelete(id), _) => {
                gateway.integrations().delete(id, tsg, o).await.map(null)
            }
            (Write::ModelsSet(id), Self::ModelsSet(b)) => {
                page_value(&gateway.integrations().set_models(id, b, o).await?)
            }
            (Write::IntegrationWorkspacesSet(id), Self::IntegrationWorkspacesSet(b)) => {
                page_value(&gateway.integrations().set_workspaces(id, b, o).await?)
            }
            (Write::McpCreate, Self::McpCreate(b)) => {
                page_value(&gateway.mcp_integrations().create(b, o).await?)
            }
            (Write::McpUpdate(id), Self::McpUpdate(b)) => {
                page_value(&gateway.mcp_integrations().update(id, b, o).await?)
            }
            (Write::McpDelete(id), _) => gateway.mcp_integrations().delete(id, o).await.map(null),
            (Write::CapabilitiesSet(id), Self::CapabilitiesSet(b)) => page_value(
                &gateway
                    .mcp_integrations()
                    .set_capabilities(id, b, o)
                    .await?,
            ),
            (Write::McpWorkspacesSet(id), Self::McpWorkspacesSet(b)) => {
                page_value(&gateway.mcp_integrations().set_workspaces(id, b, o).await?)
            }
            (Write::OrganisationUpdate, Self::OrganisationUpdate(b)) => {
                page_value(&gateway.organisations().update_self(b, o).await?)
            }
            (Write::AuthSettingsUpdate, Self::AuthSettingsUpdate(b)) => page_value(
                &gateway
                    .organisations()
                    .update_auth_settings(tsg, b, o)
                    .await?,
            ),
            (Write::PluginCreate, Self::PluginCreate(b)) => {
                page_value(&gateway.plugins().create(b, o).await?)
            }
            (Write::AdminGuardrailCreate, Self::AdminGuardrailCreate(b)) => {
                page_value(&gateway.admin_guardrails().create(b, o).await?)
            }
            (Write::AdminGuardrailUpdate(id), Self::AdminGuardrailUpdate(b)) => {
                page_value(&gateway.admin_guardrails().update(id, b, o).await?)
            }
            (Write::AdminGuardrailDelete(id), _) => {
                gateway.admin_guardrails().delete(id, o).await.map(null)
            }
            (Write::McpSync(id), Self::McpSync(b)) => page_value(
                &gateway
                    .admin_guardrails()
                    .sync_mcp_servers(id, b, o)
                    .await?,
            ),
            (Write::McpUpsert(id, server), Self::McpUpsert(b)) => page_value(
                &gateway
                    .admin_guardrails()
                    .upsert_mcp_server(id, server, b, o)
                    .await?,
            ),
            // The secret writes are raw (G36, `gateway::secrets`); a mismatch is a bug.
            _ => Err(scm::Error::new(ErrorKind::Configuration)),
        }
    }
}

/// Register every credential of a request `body` (the write's paths, the credential-name
/// rule, URL secrets) in the `SecretSet` before anything is printed (§19.12, G43).
pub fn register_body(body: &Value, policy: &Policy, secrets: &SecretSet) {
    if let Ok(mut doc) = crate::json::to_ordered(body) {
        withhold::apply(&mut doc, policy, secrets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn words_acks_and_lines() {
        let words = Write::ConfigUpdate("c-1".into()).words("100", None);
        assert_eq!(
            words.unknown_line(),
            "Outcome unknown: the config c-1 may have been updated."
        );
        assert_eq!(words.ack(Some("other")), "Config updated: c-1");
        let created = Write::KeyCreate(KeyKind::Service).words("100", None);
        assert_eq!(created.ack(Some("k-1")), "Service API key created: k-1");
        assert_eq!(created.ack(None), "Service API key created");
        assert_eq!(
            Write::ModelsSet("i-1".into())
                .words("100", None)
                .ack(Some("x")),
            "Model bindings of integration i-1 replaced"
        );
        assert_eq!(
            Write::AuthSettingsUpdate
                .words("100", None)
                .unreadable_line(),
            "Outcome: the authentication settings of organisation 100 was replaced, but the response could not be read."
        );
        assert_eq!(
            Write::McpCreate.words("1", None).ack(Some("m-1")),
            "MCP integration created: m-1"
        );
        // Choice 143: the words carry the write's hint, so `describe` is never handed a
        // write's lines without it.
        assert_eq!(
            words.hint().as_deref(),
            Some("Check it with 'airs gateway configs get c-1'.")
        );
        assert_eq!(
            Write::ConfigCreate
                .words("100", Some("ws-1"))
                .hint()
                .as_deref(),
            Some("Check 'airs gateway configs list --workspace ws-1' before creating it again.")
        );
        assert_eq!(
            Write::ProviderCreate.words("100", None).hint().as_deref(),
            Some(
                "Check 'airs gateway providers list --workspace <uuid>' before creating it again."
            )
        );
    }

    /// D243, choices 174 and 178: a deployment update that replaces its `auth_settings` and an
    /// MCP update that sets `auth_type` confirm, worded as the organisation auth-settings
    /// prompt; a deployment update that archives it confirms as the archive does; a rotation
    /// that also replaces the settings or archives says so; other updates do not confirm.
    #[test]
    fn d243_writes_that_replace_authentication_confirm() {
        let deployment = Write::DeploymentUpdate("d".into());
        let settings = deployment
            .confirm(&json!({"auth_settings": {"type": "none"}}), None, None)
            .unwrap();
        assert_eq!(
            settings.prompt,
            "Replace the authentication settings of deployment d? Clients of the deployment use them at once."
        );
        assert_eq!(
            settings.action,
            "replace the authentication settings of deployment d"
        );
        let both = deployment
            .confirm(
                &json!({"rotate_auth": true, "auth_settings": {}}),
                None,
                Some("k.json"),
            )
            .unwrap();
        assert!(
            both.prompt
                .ends_with("written to k.json. Its authentication settings are replaced too."),
            "{}",
            both.prompt
        );
        assert!(
            deployment
                .confirm(&json!({"name": "n"}), None, None)
                .is_none()
        );
        // Choice 178: an update that archives the deployment confirms as the archive does;
        // `status: active` does not.
        let archive = deployment
            .confirm(&json!({"status": "archived", "name": "n"}), None, None)
            .unwrap();
        assert_eq!(archive.prompt, "Archive deployment d? (status archived)");
        assert_eq!(archive.action, "archive deployment d");
        let archive_settings = deployment
            .confirm(
                &json!({"status": "archived", "auth_settings": {}}),
                None,
                None,
            )
            .unwrap();
        assert_eq!(
            archive_settings.prompt,
            "Archive deployment d? (status archived) Its authentication settings are replaced too."
        );
        let rotate_archive = deployment
            .confirm(
                &json!({"rotate_auth": true, "status": "archived"}),
                None,
                Some("k.json"),
            )
            .unwrap();
        assert!(
            rotate_archive
                .prompt
                .ends_with("written to k.json. The deployment is archived too."),
            "{}",
            rotate_archive.prompt
        );
        assert!(
            deployment
                .confirm(&json!({"status": "active"}), None, None)
                .is_none()
        );
        let mcp = Write::McpUpdate("m".into());
        for body in [
            json!({"auth_type": "none"}),
            json!({"auth_type": "headers"}),
            json!({"secret_mappings": []}),
            json!({"configurations": {"client_secret": "x"}}),
        ] {
            let confirm = mcp.confirm(&body, None, None).unwrap();
            assert_eq!(
                confirm.prompt,
                "Replace the authentication settings of MCP integration m? Calls through it use them at once.",
                "{body}"
            );
            assert_eq!(
                confirm.action,
                "replace the authentication settings of MCP integration m"
            );
        }
        assert!(
            mcp.confirm(&json!({"description": "d"}), None, None)
                .is_none()
        );
    }

    #[test]
    fn confirmations_follow_the_body() {
        let none = json!({});
        let delete = Write::KeyDelete(KeyKind::User, "k".into())
            .confirm(&none, None, None)
            .unwrap();
        assert_eq!(delete.prompt, "Revoke user API key k?");
        assert_eq!(delete.action, "revoke user API key k");
        let set = Write::IntegrationWorkspacesSet("i".into())
            .confirm(&none, None, None)
            .unwrap();
        assert_eq!(
            set.prompt,
            "Replace workspace bindings on i? Bindings not named are removed."
        );
        assert_eq!(set.action, "replace workspace bindings on i");
        // The removal warning follows the body that is sent, not --preserve-existing: a
        // `--set override_existing_workspace_access=true` after it removes bindings.
        for write in [
            Write::IntegrationWorkspacesSet("i".into()),
            Write::McpWorkspacesSet("i".into()),
        ] {
            let preserved = write
                .confirm(
                    &json!({"override_existing_workspace_access": false}),
                    None,
                    None,
                )
                .unwrap();
            assert!(
                !preserved.prompt.contains("removed"),
                "{}",
                preserved.prompt
            );
            for body in [
                json!({"override_existing_workspace_access": true}),
                json!({"workspaces": []}),
            ] {
                let replaced = write.confirm(&body, None, None).unwrap();
                assert!(
                    replaced
                        .prompt
                        .ends_with(" Bindings not named are removed."),
                    "{body}: {}",
                    replaced.prompt
                );
            }
        }
        assert!(
            Write::ProviderUpdate("p".into())
                .confirm(&json!({"name": "n"}), None, None)
                .is_none()
        );
        assert_eq!(
            Write::ProviderUpdate("p".into())
                .confirm(&json!({"reset_usage": true}), None, None)
                .unwrap()
                .action,
            "reset the usage of provider p"
        );
        assert!(
            Write::IntegrationUpdate("i".into())
                .confirm(&json!({"configurations": {"custom_host": "h"}}), None, None)
                .is_none()
        );
        assert!(
            Write::IntegrationUpdate("i".into())
                .confirm(
                    &json!({"configurations": {"custom_headers": {}}}),
                    None,
                    None
                )
                .is_some()
        );
        assert_eq!(
            Write::AuthSettingsUpdate
                .confirm(&none, None, None)
                .unwrap()
                .action,
            "replace the authentication settings of the selected tenant's organisation"
        );
        assert!(Write::ConfigCreate.confirm(&none, None, None).is_none());
        assert!(
            Write::McpUpsert("g".into(), "s".into())
                .confirm(&none, None, None)
                .is_none()
        );
    }

    /// G49, settled on scm-sdk 0.2.2: the SDK request schemas (`Mode::Request`) enforce
    /// every TypeScript refinement the CLI checks first, and are strict. Each pair is a body
    /// the schema accepts and the same body breaking one refinement, so the refusal is the
    /// refinement's; the CLI's own check only chooses TypeScript's text (§19 G49).
    #[test]
    fn g49_the_sdk_request_schemas_enforce_the_refinements() {
        use scm::validation::Mode;
        let id = "11111111-1111-4111-8111-000000000001";
        macro_rules! pair {
            ($model:ty, $valid:expr, $broken:expr) => {{
                let (valid, broken): (Value, Value) = ($valid, $broken);
                let name = stringify!($model);
                assert!(
                    <$model>::parse(valid.clone(), Mode::Request).is_ok(),
                    "{name} accepts {valid}"
                );
                assert!(
                    <$model>::parse(broken.clone(), Mode::Request).is_err(),
                    "{name} refuses {broken}"
                );
            }};
        }
        pair!(GatewayConfigUpdateRequest, json!({"name": "x"}), json!({}));
        pair!(
            GatewayConfigUpdateRequest,
            json!({"name": "x"}),
            json!({"name": "x", "unknown": 1})
        );
        let guardrail = json!({"workspace_id": id, "name": "g", "checks": [{"id": "prompt-injection"}], "actions": {"deny": true}});
        let mut no_checks = guardrail.clone();
        no_checks.as_object_mut().unwrap().remove("checks");
        let mut no_actions = guardrail.clone();
        no_actions.as_object_mut().unwrap().remove("actions");
        pair!(GatewayGuardrailCreateRequest, guardrail.clone(), no_checks);
        pair!(GatewayGuardrailCreateRequest, guardrail, no_actions);
        assert!(
            GatewayGuardrailCreateRequest::parse(
                json!({"workspace_id": id, "name": "g", "target": "mcp_tools"}),
                Mode::Request
            )
            .is_ok(),
            "an MCP guardrail needs neither"
        );
        pair!(
            GatewayApiKeyRotateRequest,
            json!({"key_transition_period_ms": 1_800_000}),
            json!({"key_transition_period_ms": 1_799_999})
        );
        let models =
            json!({"models": [{"slug": "gpt-4o", "enabled": true}], "allow_all_models": false});
        pair!(
            GatewayIntegrationModelsBulkUpdateRequest,
            models.clone(),
            json!({"models": [], "allow_all_models": false})
        );
        pair!(
            GatewayIntegrationModelsBulkUpdateRequest,
            models,
            json!({"allow_all_models": false})
        );
        pair!(
            McpIntegrationCapabilitiesBulkUpdateRequest,
            json!({"capabilities": [{"name": "search", "type": "tool", "enabled": true}]}),
            json!({"capabilities": []})
        );
        pair!(
            GatewayPluginCreateRequest,
            json!({"organisation_id": "100", "integration_id": id, "credentials": {"a": "b"}}),
            json!({"organisation_id": "100", "integration_id": id, "credentials": {}})
        );
        // Every update schema refuses an empty body (the "at least one field" refinement;
        // each of their members is optional).
        for refused in [
            GatewayGuardrailUpdateRequest::parse(json!({}), Mode::Request).is_err(),
            GatewayProviderUpdateRequest::parse(json!({}), Mode::Request).is_err(),
            GatewayApiKeyUpdateRequest::parse(json!({}), Mode::Request).is_err(),
            GatewayDeploymentUpdateRequest::parse(json!({}), Mode::Request).is_err(),
            GatewayIntegrationUpdateRequest::parse(json!({}), Mode::Request).is_err(),
            McpIntegrationUpdateRequest::parse(json!({}), Mode::Request).is_err(),
            GatewayOrganisationUpdateRequest::parse(json!({}), Mode::Request).is_err(),
            GatewayOrganisationAuthSettingsUpdateRequest::parse(json!({}), Mode::Request).is_err(),
            GatewayIntegrationWorkspacesBulkUpdateRequest::parse(json!({}), Mode::Request).is_err(),
            McpIntegrationWorkspacesBulkUpdateRequest::parse(json!({}), Mode::Request).is_err(),
        ] {
            assert!(refused);
        }
    }

    /// G49: the refinements are the CLI's, with TypeScript's texts.
    #[test]
    fn refinements() {
        let text = |write: Write, body: Value| match write.refine(&body) {
            Err(CliError::Usage(message)) => message,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            text(Write::ConfigUpdate("c".into()), json!({})),
            "Invalid AI Gateway request: <root>: Provide at least one field"
        );
        assert_eq!(
            text(Write::GuardrailCreate, json!({"name": "g"})),
            "Invalid AI Gateway request: checks: LLM guardrails require checks; actions: LLM guardrails require actions"
        );
        assert!(
            Write::GuardrailCreate
                .refine(&json!({"target": "mcp_tools"}))
                .is_ok()
        );
        assert_eq!(
            text(Write::GuardrailCreate, json!({"checks": []})),
            "Invalid AI Gateway request: actions: LLM guardrails require actions"
        );
        assert_eq!(
            text(
                Write::KeyRotate(KeyKind::Service, "k".into()),
                json!({"key_transition_period_ms": 1000})
            ),
            "Invalid AI Gateway request: key_transition_period_ms: Number must be greater than or equal to 1800000"
        );
        assert_eq!(
            text(Write::ModelsSet("i".into()), json!({"models": []})),
            "Invalid AI Gateway request: models: Array must contain at least 1 element(s)"
        );
        assert_eq!(
            text(Write::ModelsSet("i".into()), json!({})),
            "Invalid AI Gateway request: models: Required",
            "the flag omitted (zod's text for a missing member)"
        );
        assert_eq!(
            text(Write::CapabilitiesSet("i".into()), json!({})),
            "Invalid AI Gateway request: capabilities: Required"
        );
        assert_eq!(
            text(Write::PluginCreate, json!({"credentials": {}})),
            "Invalid AI Gateway request: credentials: Provide at least one field"
        );
        assert!(
            Write::KeyRotate(KeyKind::User, "k".into())
                .refine(&json!({}))
                .is_ok()
        );
    }
}
