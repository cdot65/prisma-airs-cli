//! Dispatch of the gateway writes of chunk 3 other than workspaces and scopes (DESIGN
//! §19.3, §19.10–§19.13; planning is `gateway_write_plan`). [`run`] sequences the phases
//! in the §19.3 order, each a named step here:
//! 1. [`prepare`] (before the tenant): the warnings, the `--secret-output` path check, the
//!    credential source (`--key-file`, `--key-stdin`, the hidden prompt), the body, its
//!    refinements and schema, the non-interactive refusal;
//! 2. [`connect`]: the tenant, the client and the organisation comparison (D245);
//! 3. [`confirmed`]: the prompt (D243);
//! 4. [`authorise`]: the `--secret-output` reservation (§19.11), the token and the
//!    `--ai-provider` catalog read (G38);
//! 5. the one attempt (D228): [`send_typed`] through the SDK, or [`send_credential`], the
//!    G36 raw write whose answer goes to the reserved file.

use super::Invocation;
use super::gateway_write_plan::{KeySource, PROVIDER_PLACEHOLDER, WritePlan};
use super::write_common::{self as wc};
use crate::clients::{GatewayClients, SINGLE_REQUEST_BUDGET};
use crate::debug::{GatewayPreparationCheckpoint as Checkpoint, GatewayPreparationStage as Stage};
use crate::error::{CliError, InterruptContext};
use crate::files::Reservation;
use crate::gateway::body::{self, Input};
use crate::gateway::request::{Context as RequestContext, FailureKind, PreparedRequest};
use crate::gateway::secrets::{self, CredentialRequest, SecretWrite};
use crate::gateway::writes::{Confirm, Write};
use crate::gateway::{self, GwPlane, views, withhold};
use crate::json::OValue;
use crate::output::{Format, terminal_safe};
use crate::writes::OutcomeLines;
use serde_json::{Value, json};
use std::ffi::OsString;

// ---------------------------------------------------------------------------
// The credential inputs (§19.12)
// ---------------------------------------------------------------------------

/// The `--key-stdin` refusal (TS text with the program name).
const KEY_STDIN: &str = "--key-stdin reads one credential from piped stdin, e.g. 'airs gateway integrations create ... --key-stdin < provider.key'; in a terminal, omit every key flag to be prompted";

/// `--key-file <path>`: 64 KiB, UTF-8, one trailing newline removed; exactly one nonempty
/// credential (TS texts, exit 2).
fn read_key_file(path: &OsString) -> Result<String, CliError> {
    let shown = terminal_safe(&crate::files::shown(path)).into_owned();
    let text = crate::files::read_text(path, crate::input::INPUT_LIMIT as u64)
        .map_err(|_| CliError::usage(format!("Cannot read --key-file {shown}")))?;
    let value = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(&text);
    if crate::tenant::config::js_trim(value).is_empty() || value.contains(['\r', '\n', '\0']) {
        return Err(CliError::usage(
            "--key-file must contain exactly one nonempty credential",
        ));
    }
    Ok(value.to_owned())
}

/// `--key-stdin`: piped stdin only (§3.10 rules).
fn read_key_stdin(invocation: &Invocation) -> Result<String, CliError> {
    if invocation.terminals.stdin {
        return Err(CliError::usage(KEY_STDIN));
    }
    crate::input::read_piped_stdin(&invocation.output).map_err(|_| CliError::usage(KEY_STDIN))
}

/// The hidden `? Provider API key: [input is masked] ` prompt (before the tenant).
fn prompt_key(invocation: &mut Invocation) -> Result<String, CliError> {
    super::prompt_hidden(invocation, "Provider API key:")
}

/// A provider credential is required (TS text).
const CREDENTIAL_REQUIRED: &str = "A provider credential is required: run in a terminal to be prompted, or pass --key-file, --key-stdin (piped), --secret-mappings, or --key (the gateway rejects a credential-less integration with a generic AB01)";

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

fn not_sent() -> InterruptContext {
    InterruptContext::NothingChanged
}

/// A write after phase 1 ([`prepare`]): everything checked that needs no tenant.
struct Prepared {
    /// The built body (its `organisation_id` default not yet filled in).
    body: Value,
    /// The body sets `rotate_auth: true` (a `deployments update` that rotates).
    rotating: bool,
    /// The confirmation the write asks for, if any (D243).
    confirm: Option<Confirm>,
    /// The `--secret-output` path as the prompt and messages show it.
    secret_shown: Option<String>,
}

/// A write after phase 2 ([`connect`]): the client, the format and the organisation.
struct Connected {
    clients: GatewayClients,
    format: Format,
    /// The explicit organisation, else the tenant's TSG.
    org: String,
    request: PreparedRequest,
}

/// Run a planned write (§19.10–§19.13): the phases of the module documentation, in order.
pub async fn run(invocation: &mut Invocation, plan: &WritePlan) -> Result<(), CliError> {
    let prepared = prepare(invocation, plan)?;
    if plan.dry_run {
        return preview(invocation, plan, prepared);
    }
    let mut connected = connect(invocation, plan, prepared.body.clone())?;
    if !confirmed(invocation, plan, &prepared, &connected)? {
        return Ok(());
    }
    let secret_write = plan.secret_write(prepared.rotating);
    let reservation = match (&secret_write, &plan.secret) {
        (Some(_), Some(path)) => Some(Reservation::reserve_new(path)?),
        _ => None,
    };
    authorise(invocation, plan, &mut connected).await?;
    match (secret_write, reservation, &plan.secret) {
        (Some(write), Some(reservation), Some(path)) => {
            send_credential(
                invocation,
                &connected,
                &write,
                &CredentialRequest::new(&plan.write, connected.request.typed())?,
                (reservation, path),
            )
            .await
        }
        _ => send_typed(invocation, plan, &connected).await,
    }
}

/// All preparation checkpoints share this adapter so classification and log
/// cardinality cannot drift between execution and preview.
fn prepare_request(
    invocation: &Invocation,
    plan: &WritePlan,
    body: Value,
    context: RequestContext<'_>,
    checkpoint: Checkpoint,
) -> Result<PreparedRequest, CliError> {
    PreparedRequest::prepare(&plan.write, body, plan.org_body, context).map_err(|error| {
        if let Some(log) = &invocation.debug {
            let stage = match error.kind() {
                FailureKind::Validation => Stage::RequestValidation,
                FailureKind::TenantBinding => Stage::TenantBinding,
            };
            log.gateway_preparation_rejected(plan.write.diagnostic_operation(), stage, checkpoint);
        }
        error.into_cli_error()
    })
}

/// Phase 1, before the tenant (§19.3, choice 124): the inline-credential warnings, the
/// `--secret-output` path check, the credential source, the body with its refinements and
/// schema (a placeholder organisation), and the non-interactive refusal.
fn prepare(invocation: &mut Invocation, plan: &WritePlan) -> Result<Prepared, CliError> {
    for warning in &plan.warnings {
        invocation.output.warning(warning);
    }
    if let Some(path) = &plan.secret {
        secrets::check_destination(path)?;
    }
    let input = credential_input(invocation, plan)?;
    let mut body = if plan.write.has_body() {
        body::build(&input)?
    } else {
        json!({})
    };
    crate::gateway::writes::register_body(&body, plan.write.policy(), &invocation.secrets);
    if plan.write == Write::IntegrationCreate {
        integration_create_body(plan, &mut body)?;
    }
    let rotating = body.get("rotate_auth") == Some(&Value::Bool(true));
    if let Write::DeploymentUpdate(_) = plan.write {
        match (rotating, plan.secret.is_some()) {
            (true, false) if !plan.dry_run => {
                return Err(CliError::usage(secrets::NEEDS_DESTINATION));
            }
            (false, true) => return Err(CliError::usage(secrets::ONLY_ROTATE)),
            _ => {}
        }
    }
    prepare_request(
        invocation,
        plan,
        body.clone(),
        RequestContext::Preflight {
            explicit_organisation: plan.organisation.as_deref(),
        },
        Checkpoint::Preflight,
    )?;
    let secret_shown = plan.secret.as_deref().map(secrets::shown_path);
    let confirm = plan
        .write
        .confirm(&body, plan.organisation.as_deref(), secret_shown.as_deref());
    if let Some(confirm) = &confirm
        && !plan.dry_run
    {
        wc::gate_on_terminals(plan.force, &confirm.action)?;
    }
    Ok(Prepared {
        body,
        rotating,
        confirm,
        secret_shown,
    })
}

fn preview(
    invocation: &mut Invocation,
    plan: &WritePlan,
    prepared: Prepared,
) -> Result<(), CliError> {
    let (_context, format, tsg) = invocation.gateway_local_context(plan.format)?;
    let request = prepare_request(
        invocation,
        plan,
        prepared.body,
        RequestContext::Tenant {
            selected: &tsg,
            explicit_organisation: plan.organisation.as_deref(),
        },
        Checkpoint::TenantBinding,
    )?;
    let view = crate::gateway::preview::document(
        &plan.write,
        &request,
        &tsg,
        prepared.confirm.is_some(),
        plan.secret_write(prepared.rotating).is_some(),
        plan.secret.is_some(),
        &invocation.secrets,
    )?;
    views::render_detail(&mut invocation.output, &view, format)
}

/// The structured input with the provider credential of `--key-file`, `--key-stdin` or
/// the hidden prompt (§19.12), the credential registered in the `SecretSet` before the
/// body is built.
fn credential_input(invocation: &mut Invocation, plan: &WritePlan) -> Result<Input, CliError> {
    let mut input = plan.input.clone();
    match &plan.key {
        KeySource::File(path) => input
            .named
            .push(("key", Value::String(read_key_file(path)?))),
        KeySource::Stdin => input
            .named
            .push(("key", Value::String(read_key_stdin(invocation)?))),
        KeySource::Missing
            if invocation.terminals.interactive()
                && input.file.is_none()
                && !input
                    .named
                    .iter()
                    .any(|(path, _)| *path == "secret_mappings") =>
        {
            let key = prompt_key(invocation)?;
            input.named.push(("key", Value::String(key)));
        }
        KeySource::Given | KeySource::Missing => {}
    }
    if let Some(Value::String(key)) = input
        .named
        .iter()
        .find(|(path, _)| *path == "key")
        .map(|(_, value)| value)
    {
        invocation.secrets.add(key);
    }
    Ok(input)
}

/// `integrations create`: a credential is required (TS text, exit 2), and a slug
/// `--ai-provider` holds the placeholder UUID until the catalog read (G38).
fn integration_create_body(plan: &WritePlan, body: &mut Value) -> Result<(), CliError> {
    let key = body
        .get("key")
        .and_then(Value::as_str)
        .is_some_and(|key| !key.is_empty());
    let mappings = body
        .get("secret_mappings")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    if !key && !mappings {
        return Err(CliError::usage(CREDENTIAL_REQUIRED));
    }
    if plan.provider.is_some()
        && let Some(map) = body.as_object_mut()
    {
        map.entry("ai_provider_id")
            .or_insert_with(|| Value::String(PROVIDER_PLACEHOLDER.to_owned()));
    }
    Ok(())
}

/// Phase 2: the tenant and the client (an explicit organisation compared with the
/// tenant's, D245), a body naming a foreign organisation refused, the organisation filled
/// in and the body validated again. Nothing has been sent.
fn connect(
    invocation: &mut Invocation,
    plan: &WritePlan,
    body: Value,
) -> Result<Connected, CliError> {
    let (context, format, tenant_tsg) = invocation
        .gateway_local_context(plan.format)
        .map_err(super::nothing_changed)?;
    crate::tenant::config::require_management(&context.config, Some(context.name()))
        .map_err(super::nothing_changed)?;
    let request = prepare_request(
        invocation,
        plan,
        body,
        RequestContext::Tenant {
            selected: &tenant_tsg,
            explicit_organisation: plan.organisation.as_deref(),
        },
        Checkpoint::TenantBinding,
    )?;
    let clients = GatewayClients::build(
        &context.config,
        &invocation.session,
        &invocation.secrets,
        invocation.debug.as_ref(),
    )
    .map_err(|error| super::nothing_changed(error.into()))?;
    let org = plan.organisation.clone().unwrap_or(tenant_tsg);
    Ok(Connected {
        clients,
        format,
        org,
        request,
    })
}

/// Phase 3: the prompt of a confirming write without `--force` (D243), worded with the
/// organisation now known; before the token (the write reads nothing first).
fn confirmed(
    invocation: &mut Invocation,
    plan: &WritePlan,
    prepared: &Prepared,
    connected: &Connected,
) -> Result<bool, CliError> {
    let Some(confirm) = &prepared.confirm else {
        return Ok(true);
    };
    if plan.force {
        return Ok(true);
    }
    let message = plan
        .write
        .confirm(
            connected.request.body(),
            Some(&connected.org),
            prepared.secret_shown.as_deref(),
        )
        .map_or_else(|| confirm.prompt.clone(), |confirm| confirm.prompt);
    wc::confirm(invocation, &message)
}

/// Phase 4 (after the `--secret-output` reservation): the token, then a slug
/// `--ai-provider` resolved over the catalog (one read) and the body validated with it.
async fn authorise(
    invocation: &mut Invocation,
    plan: &WritePlan,
    connected: &mut Connected,
) -> Result<(), CliError> {
    connected
        .clients
        .authenticate(None, crate::clients::TOKEN_BUDGET)
        .await
        .map_err(super::nothing_changed)?;
    let Some(slug) = &plan.provider else {
        return Ok(());
    };
    let id = provider_id(invocation, &connected.clients, slug).await?;
    let mut body = connected.request.body().clone();
    if body.get("ai_provider_id").and_then(Value::as_str) == Some(PROVIDER_PLACEHOLDER)
        && let Some(map) = body.as_object_mut()
    {
        map.insert("ai_provider_id".into(), Value::String(id));
    }
    connected.request = prepare_request(
        invocation,
        plan,
        body,
        RequestContext::Tenant {
            selected: &connected.org,
            explicit_organisation: plan.organisation.as_deref(),
        },
        Checkpoint::ProviderResolution,
    )?;
    Ok(())
}

/// G38: the catalog read (nothing changed on a failure) and the slug matched over it.
async fn provider_id(
    invocation: &mut Invocation,
    clients: &GatewayClients,
    slug: &str,
) -> Result<String, CliError> {
    let options = gateway::options(SINGLE_REQUEST_BUDGET)?;
    let catalog = match clients.gateway.integrations().catalog(Some(&options)).await {
        Ok(catalog) => catalog,
        Err(error) => {
            return Err(super::nothing_changed(
                invocation.gateway_error(&error, GwPlane::Admin),
            ));
        }
    };
    gateway::provider::resolve(&crate::json::model_value(&catalog)?, slug)
}

/// Phase 5, a typed write: one attempt through the SDK (D228), the `✓` line, and the
/// answer withheld and printed (a removal prints nothing more).
async fn send_typed(
    invocation: &mut Invocation,
    plan: &WritePlan,
    connected: &Connected,
) -> Result<(), CliError> {
    let words = plan.write.words(&connected.org, plan.workspace.as_deref());
    let gateway = &connected.clients.gateway;
    let mut answer = attempt(
        invocation,
        plan.write.plane(),
        &words,
        &not_sent(),
        &InterruptContext::ChangeMayBeApplied,
        async || {
            let options = gateway::sdk_options(SINGLE_REQUEST_BUDGET)?;
            connected
                .request
                .typed()
                .send(gateway, &plan.write, &connected.org, &options)
                .await
        },
    )
    .await?;
    invocation.committed = true;
    let answer_id = answer
        .get("id")
        .and_then(OValue::as_str)
        .map(|id| terminal_safe(invocation.secrets.scrub(id)).into_owned());
    invocation.output.success(&words.ack(answer_id.as_deref()));
    if plan.write.is_removal() {
        return Ok(());
    }
    withhold::apply(&mut answer, plan.write.policy(), &invocation.secrets);
    views::render_detail(&mut invocation.output, &answer, connected.format)
}

/// One write (D228) with the grant hint before the §6 error (D234); the failure is
/// described by the shared rule ([`crate::writes::describe`]) and its interruption mapped
/// to `not_sent` / `sent`.
pub async fn attempt<T>(
    invocation: &mut Invocation,
    plane: GwPlane,
    lines: impl OutcomeLines,
    not_sent: &InterruptContext,
    sent: &InterruptContext,
    call: impl AsyncFnMut() -> scm::Result<T>,
) -> Result<T, CliError> {
    match crate::writes::attempt(call).await {
        Ok(value) => Ok(value),
        Err(error) => {
            if let Some(hint) = gateway::grant_hint(&error, plane) {
                invocation.output.warning(&hint);
            }
            Err(crate::writes::in_contexts(
                crate::writes::describe(&error, lines, &invocation.secrets),
                not_sent,
                sent,
            ))
        }
    }
}

/// Phase 5, a one-time credential write (§19.11): the G36 raw request sent once, the
/// whole answer saved through the reservation, then the `✓` lines and `{saved_to}`.
async fn send_credential(
    invocation: &mut Invocation,
    connected: &Connected,
    write: &SecretWrite,
    request: &CredentialRequest,
    (reservation, path): (Reservation, &std::path::Path),
) -> Result<(), CliError> {
    let gateway = &connected.clients.gateway;
    let answer: Value = attempt(
        invocation,
        request.plane(),
        write,
        &not_sent(),
        &InterruptContext::ChangeMayBeApplied,
        async || {
            let options = gateway::sdk_options(SINGLE_REQUEST_BUDGET)?;
            request.send(gateway, &options).await
        },
    )
    .await?;
    // From the 2xx answer on, an interruption never reports "nothing was changed".
    invocation.interrupt = Some(secrets::not_saved_context(write));
    let mut interrupt = None;
    let saved = secrets::save_answer(
        answer,
        reservation,
        path,
        write,
        &invocation.secrets,
        &mut interrupt,
    );
    if interrupt.is_some() {
        invocation.interrupt = interrupt;
    }
    let saved = saved?;
    let warning = secrets::durability_warning(saved.published, path);
    if let Err(error) = saved.rest {
        if let Some(warning) = &warning {
            invocation.output.warning(warning);
        }
        return Err(error);
    }
    invocation
        .output
        .success(&write.success_line(saved.answer_id.as_deref()));
    if let Some(warning) = &warning {
        invocation.output.warning(warning);
    }
    invocation.output.success(&format!(
        "Wrote the one-time credential to {} (mode 0600); it is shown only once.",
        secrets::shown_path(path)
    ));
    let record = OValue::Object(vec![(
        "saved_to".to_owned(),
        OValue::String(crate::files::shown(path.as_os_str())),
    )]);
    views::render_detail(&mut invocation.output, &record, connected.format)
}
