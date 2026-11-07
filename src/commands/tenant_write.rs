//! The tenant write commands on the TOML store: `tenant create`, `set`, `unset`,
//! `switch` and `delete|rm` (DESIGN §3.9–§3.11).
//!
//! Order of work for every command: the argument checks of [`plan`] (usage errors,
//! before anything is read), then the terminal and stdin checks, the store location,
//! an unlocked read with the state checks, the prompts, and finally one locked
//! [`Transaction`] that re-reads and re-checks the state before writing. No lock is
//! held while waiting for the user or for stdin. stdout stays empty: outcomes are
//! stderr lines (D74).

use super::Invocation;
use crate::cli::{CreateArgs, SetArgs, TenantCommand};
use crate::error::{CliError, EXIT_SIGINT, InterruptContext};
use crate::input::{self, Cancelled, Prompter};
use crate::output::Output;
use crate::signals::{CANCELLED, Phase};
use crate::store::atomic::{self, Transaction};
use crate::store::lock::Waiter;
use crate::store::{Index, MAX_TENANTS, PROGRAM, Store, shown, valid_tenant_name};
use crate::tenant::config::{self, ConfigValue, TenantConfig, schema_key, write_rule};
use crate::tenant::{NAME_RULE, Selected, StoreView};

/// A validated write command (everything [`plan`] could check without reading).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WritePlan {
    Create {
        name: String,
        tsg: Option<String>,
        client_id: Option<String>,
        secret_stdin: bool,
    },
    Set {
        name: String,
        key: &'static str,
        /// The validated argv value; `None` for `--stdin` or a prompt.
        value: Option<ConfigValue>,
        stdin: bool,
    },
    Unset {
        name: String,
        key: &'static str,
    },
    Switch {
        name: String,
    },
    Delete {
        name: String,
        force: bool,
    },
}

/// The canonical subcommand name of a write command, or `None` for the read commands.
pub fn write_command_name(command: &TenantCommand) -> Option<&'static str> {
    match command {
        TenantCommand::Create(_) => Some("create"),
        TenantCommand::Set(_) => Some("set"),
        TenantCommand::Unset { .. } => Some("unset"),
        TenantCommand::Switch { .. } => Some("switch"),
        TenantCommand::Delete { .. } => Some("delete"),
        TenantCommand::Import(_)
        | TenantCommand::Scanners(_)
        | TenantCommand::List
        | TenantCommand::Path { .. }
        | TenantCommand::Read { .. }
        | TenantCommand::Get { .. } => None,
    }
}

fn unknown_key() -> CliError {
    CliError::usage(format!(
        "Unknown configuration key. Supported keys: {}",
        config::KEYS.join(", ")
    ))
}

fn known_key(key: &str) -> Result<&'static str, CliError> {
    if key == "scanners" {
        return Err(CliError::usage(
            "Use 'airs tenant scanners' to change named scanner settings",
        ));
    }
    schema_key(key).ok_or_else(unknown_key)
}

/// The argument checks of each command, in the §3.11 order (usage errors, exit 2).
pub fn plan(command: &TenantCommand) -> Result<WritePlan, CliError> {
    match command {
        TenantCommand::Create(CreateArgs {
            name,
            tsg_id,
            client_id,
            client_secret_stdin,
        }) => {
            if !valid_tenant_name(name) {
                return Err(CliError::usage(NAME_RULE));
            }
            let checked = |key: &'static str, value: &Option<String>| {
                value
                    .as_deref()
                    .map(|value| {
                        write_rule(name, key, value)
                            .map(|_| value.to_owned())
                            .map_err(CliError::usage)
                    })
                    .transpose()
            };
            Ok(WritePlan::Create {
                name: name.clone(),
                tsg: checked("mgmtTsgId", tsg_id)?,
                client_id: checked("mgmtClientId", client_id)?,
                secret_stdin: *client_secret_stdin,
            })
        }
        TenantCommand::Set(SetArgs {
            name,
            key,
            value,
            stdin,
        }) => {
            let key = known_key(key)?;
            if value.is_some() && *stdin {
                return Err(CliError::usage("Use a value or --stdin, not both"));
            }
            if value.is_some() && config::is_secret_key(key) {
                return Err(CliError::usage(
                    "Do not pass credentials as arguments. Omit the value for a hidden prompt, or use --stdin.",
                ));
            }
            let value = value
                .as_deref()
                .map(|value| write_rule(name, key, value).map_err(CliError::usage))
                .transpose()?;
            Ok(WritePlan::Set {
                name: name.clone(),
                key,
                value,
                stdin: *stdin,
            })
        }
        TenantCommand::Unset { name, key } => {
            let key = known_key(key)?;
            if config::is_credential_key(key) {
                return Err(CliError::usage(format!(
                    "{key} is a credential and cannot be cleared; set a new value instead"
                )));
            }
            Ok(WritePlan::Unset {
                name: name.clone(),
                key,
            })
        }
        TenantCommand::Switch { name } => Ok(WritePlan::Switch { name: name.clone() }),
        TenantCommand::Delete { name, force } => Ok(WritePlan::Delete {
            name: name.clone(),
            force: *force,
        }),
        TenantCommand::Import(_)
        | TenantCommand::Scanners(_)
        | TenantCommand::List
        | TenantCommand::Path { .. }
        | TenantCommand::Read { .. }
        | TenantCommand::Get { .. } => {
            unreachable!("planning routes only the write commands here")
        }
    }
}

/// Run a planned write command.
pub fn run(invocation: &mut Invocation, plan: WritePlan) -> Result<(), CliError> {
    match plan {
        WritePlan::Create {
            name,
            tsg,
            client_id,
            secret_stdin,
        } => create(invocation, &name, tsg, client_id, secret_stdin),
        WritePlan::Set {
            name,
            key,
            value,
            stdin,
        } => set(invocation, &name, key, value, stdin),
        WritePlan::Unset { name, key } => unset(invocation, &name, key),
        WritePlan::Switch { name } => switch(invocation, &name),
        WritePlan::Delete { name, force } => delete(invocation, &name, force),
    }
}

// -- Shared steps -----------------------------------------------------------------------

/// Prints the waiting status once (quiet-suppressed) and stops on a signal.
struct StatusWaiter<'a> {
    output: &'a mut Output,
    secrets: crate::error::SecretSet,
}

impl Waiter for StatusWaiter<'_> {
    fn waiting(&mut self, holder: &str) {
        self.output.status(self.secrets.scrub(&format!(
            "Waiting for another {PROGRAM} change to finish ({holder})"
        )));
    }
    fn interrupted(&mut self) -> Option<CliError> {
        self.output
            .pending_signal()
            .map(|_| CliError::Interrupted(InterruptContext::NoConfigurationChanged))
    }
}

/// §3.9 steps 2–4 with the wait marked for the signal thread. A signal that already
/// arrived (during a prompt's aftermath) stops before anything is created.
pub(super) fn begin(
    invocation: &mut Invocation,
    store: &Store,
    tenant_files: bool,
) -> Result<Transaction, CliError> {
    stop_if_signalled(invocation)?;
    let signals = invocation.output.signals();
    let _waiting = signals.as_deref().map(|state| state.enter(Phase::Waiting));
    let mut waiter = StatusWaiter {
        output: &mut invocation.output,
        secrets: invocation.secrets.clone(),
    };
    Transaction::begin(store, tenant_files, &mut waiter)
}

/// Release the lock and print the directory-sync warnings (never quiet-suppressed).
pub(super) fn finish(invocation: &mut Invocation, transaction: Transaction) {
    for warning in transaction.commit() {
        invocation
            .output
            .warning(invocation.secrets.scrub(&warning));
    }
}

/// The store as a reader sees it, for the state checks (the import suffix is not
/// needed: write commands never print the "nothing registered" messages).
fn view(store: &Store) -> Result<StoreView, CliError> {
    Ok(StoreView {
        store: store.clone(),
        index: store.read_index()?,
        ts_registry: None,
    })
}

fn registered(view: &StoreView, name: &str) -> Option<Selected> {
    view.index.tsg(name).map(|tsg| Selected {
        name: name.to_owned(),
        tsg_id: tsg.to_owned(),
        by_flag: false,
    })
}

fn not_found() -> CliError {
    CliError::operational("Tenant not found")
}

fn not_found_register() -> CliError {
    CliError::operational("Tenant not found; register a named tenant first")
}

/// A cancelled prompt: end the prompt line, print the TypeScript status
/// (quiet-suppressed) and exit 130 with nothing written.
fn cancelled(invocation: &mut Invocation) -> CliError {
    invocation.output.write_stderr("\n");
    invocation.output.status(CANCELLED);
    CliError::Reported(EXIT_SIGINT)
}

/// A signal that arrived outside a prompt (between two prompts) stops the command
/// before the next one: nothing has been written.
fn stop_if_signalled(invocation: &Invocation) -> Result<(), CliError> {
    match invocation.output.pending_signal() {
        Some(_) => Err(CliError::Interrupted(
            InterruptContext::NoConfigurationChanged,
        )),
        None => Ok(()),
    }
}

/// Prompt for `key` of tenant `name` until its write rules accept the answer.
fn ask(
    invocation: &mut Invocation,
    message: &str,
    name: &str,
    key: &'static str,
) -> Result<ConfigValue, CliError> {
    stop_if_signalled(invocation)?;
    let hidden = config::is_secret_key(key);
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let answer =
        Prompter::new(&mut input, &mut invocation.output, true)
            .ask(message, hidden, |value| write_rule(name, key, value));
    match answer {
        Ok(value) => Ok(value),
        Err(Cancelled) => Err(cancelled(invocation)),
    }
}

/// Read piped stdin and apply `key`'s write rules (usage errors).
fn stdin_value(
    invocation: &Invocation,
    name: &str,
    key: &'static str,
) -> Result<ConfigValue, CliError> {
    let raw = input::read_piped_stdin(&invocation.output)?;
    write_rule(name, key, &raw).map_err(CliError::usage)
}

fn text(value: ConfigValue) -> String {
    match value {
        ConfigValue::Text(text) => text,
        ConfigValue::Integer(number) => number.to_string(),
    }
}

// -- create -----------------------------------------------------------------------------

/// The state checks of `create` (§3.11 step 7), unlocked and again under the lock.
fn check_new(index: &Index, name: &str) -> Result<(), CliError> {
    if index.tsg(name).is_some() {
        return Err(CliError::operational("Tenant name already exists"));
    }
    if let Some(other) = index.case_conflict(name) {
        return Err(CliError::operational(format!(
            "Tenant name conflicts with existing tenant {other}; names may not differ only in case"
        )));
    }
    if index.tenants.len() >= MAX_TENANTS {
        return Err(CliError::operational(format!(
            "Cannot create tenant {name}: at most 1000 tenants can be registered"
        )));
    }
    Ok(())
}

fn create(
    invocation: &mut Invocation,
    name: &str,
    tsg: Option<String>,
    client_id: Option<String>,
    secret_stdin: bool,
) -> Result<(), CliError> {
    let terminals = invocation.terminals;
    if secret_stdin && terminals.stdin {
        return Err(CliError::usage(input::NEEDS_PIPE));
    }
    let complete = tsg.is_some() && client_id.is_some() && secret_stdin;
    if !complete && !terminals.interactive() {
        return Err(CliError::usage(input::INTERACTIVE_SETUP));
    }
    let secret = if secret_stdin {
        Some(stdin_value(invocation, name, "mgmtClientSecret")?)
    } else {
        None
    };
    let store = invocation.env.store()?;
    check_new(&store.read_index()?, name)?;
    let tsg = match tsg {
        Some(tsg) => tsg,
        None => text(ask(
            invocation,
            "Tenant service group ID (mgmtTsgId):",
            name,
            "mgmtTsgId",
        )?),
    };
    let client_id = match client_id {
        Some(id) => id,
        None => text(ask(
            invocation,
            "OAuth client ID (mgmtClientId):",
            name,
            "mgmtClientId",
        )?),
    };
    let secret = match secret {
        Some(secret) => secret,
        None => ask(
            invocation,
            "OAuth client secret (mgmtClientSecret):",
            name,
            "mgmtClientSecret",
        )?,
    };
    let config = TenantConfig::from_values([
        ("mgmtClientId", ConfigValue::Text(client_id)),
        ("mgmtClientSecret", secret),
        ("mgmtTsgId", ConfigValue::Text(tsg.clone())),
    ]);
    invocation.secrets.register_config(&config);
    let mut transaction = begin(invocation, &store, true)?;
    create_locked(&mut transaction, &store, name, &config, &tsg)?;
    invocation.committed = true;
    finish(invocation, transaction);
    invocation.output.success(&format!(
        "Registered {name} (TSG {tsg}); private config created. Use {PROGRAM} tenant switch {name}."
    ));
    Ok(())
}

/// Under the lock: re-check, write `tenants/<name>.toml` (an orphan of that name is
/// replaced), then `config.toml` (the commit point). A failed `config.toml` write
/// unlinks the new tenant file.
fn create_locked(
    transaction: &mut Transaction,
    store: &Store,
    name: &str,
    config: &TenantConfig,
    tsg: &str,
) -> Result<(), CliError> {
    let mut index = store.read_index()?;
    check_new(&index, name)?;
    index.tenants.insert(name.to_owned(), tsg.to_owned());
    let index_bytes = index.to_toml();
    let index_path = store.index_path();
    // The commit point's refusals come first, so they write nothing.
    atomic::check_size(&index_path, index_bytes.as_bytes())?;
    atomic::check_replaceable(&index_path)?;
    transaction.write(&store.tenant_path(name), config.to_toml().as_bytes(), None)?;
    if let Err(error) = transaction.write(&index_path, index_bytes.as_bytes(), None) {
        transaction.rollback();
        return Err(error);
    }
    Ok(())
}

// -- set --------------------------------------------------------------------------------

fn set(
    invocation: &mut Invocation,
    name: &str,
    key: &'static str,
    value: Option<ConfigValue>,
    stdin: bool,
) -> Result<(), CliError> {
    let terminals = invocation.terminals;
    if stdin && terminals.stdin {
        return Err(CliError::usage(input::NEEDS_PIPE));
    }
    if value.is_none() && !stdin && !terminals.interactive() {
        return Err(CliError::usage(input::INTERACTIVE_SETUP));
    }
    let value = match (value, stdin) {
        (Some(value), _) => Some(value),
        (None, true) => Some(stdin_value(invocation, name, key)?),
        (None, false) => None,
    };
    let store = invocation.env.store()?;
    let unlocked = view(&store)?;
    let selected = registered(&unlocked, name).ok_or_else(not_found_register)?;
    unlocked.load_for_write(&selected, key != "mgmtTsgId")?;
    let value = match value {
        Some(value) => value,
        None => ask(invocation, &format!("{key}:"), name, key)?,
    };
    if let ConfigValue::Text(text) = &value
        && config::is_secret_key(key)
    {
        invocation.secrets.add(text);
    }
    let mut transaction = begin(invocation, &store, false)?;
    set_locked(&mut transaction, &store, name, key, value)?;
    invocation.committed = true;
    finish(invocation, transaction);
    invocation.output.success(&format!(
        "Updated {key} for tenant {name}; selection unchanged."
    ));
    Ok(())
}

/// Under the lock: re-check, set the key and write the tenant file (always). For
/// `mgmtTsgId` also write `config.toml`'s pin (the commit point, D83); when that fails
/// the tenant file gets its previous bytes back.
fn set_locked(
    transaction: &mut Transaction,
    store: &Store,
    name: &str,
    key: &'static str,
    value: ConfigValue,
) -> Result<(), CliError> {
    // Every key but mgmtTsgId is checked against the pin; mgmtTsgId changes it.
    let pin = key != "mgmtTsgId";
    let locked = view(store)?;
    let selected = registered(&locked, name).ok_or_else(not_found_register)?;
    let (mut config, previous) = locked.load_for_write(&selected, pin)?;
    config.set(key, value.clone());
    let tenant_path = store.tenant_path(name);
    let tenant_bytes = config.to_toml();
    if pin {
        return transaction.write(&tenant_path, tenant_bytes.as_bytes(), Some(previous));
    }
    let mut index = locked.index;
    index.tenants.insert(name.to_owned(), text(value));
    let index_bytes = index.to_toml();
    let index_path = store.index_path();
    atomic::check_size(&index_path, index_bytes.as_bytes())?;
    atomic::check_replaceable(&index_path)?;
    transaction.write(&tenant_path, tenant_bytes.as_bytes(), Some(previous))?;
    if transaction
        .write(&index_path, index_bytes.as_bytes(), None)
        .is_err()
    {
        let restored = transaction.rollback();
        return Err(CliError::operational(format!(
            "Cannot save {}; {}",
            shown(&index_path),
            if restored {
                "it was not changed"
            } else {
                "run the command again to finish the change"
            }
        )));
    }
    Ok(())
}

// -- unset ------------------------------------------------------------------------------

fn unset(invocation: &mut Invocation, name: &str, key: &'static str) -> Result<(), CliError> {
    let store = invocation.env.store()?;
    registered(&view(&store)?, name).ok_or_else(not_found_register)?;
    let mut transaction = begin(invocation, &store, false)?;
    let removed = unset_locked(&mut transaction, &store, name, key)?;
    invocation.committed = removed;
    finish(invocation, transaction);
    if removed {
        invocation.output.success(&format!(
            "Removed {key} from tenant {name}; selection unchanged."
        ));
    } else {
        invocation.output.info(&format!(
            "{key} is not set for tenant {name} — nothing to do"
        ));
    }
    Ok(())
}

/// Under the lock: read the pinned tenant file; remove `key` and rewrite the file when
/// it is present (whether it was).
fn unset_locked(
    transaction: &mut Transaction,
    store: &Store,
    name: &str,
    key: &str,
) -> Result<bool, CliError> {
    let locked = view(store)?;
    let selected = registered(&locked, name).ok_or_else(not_found_register)?;
    let (mut config, previous) = locked.load_for_write(&selected, true)?;
    if !config.unset(key) {
        return Ok(false);
    }
    transaction.write(
        &store.tenant_path(name),
        config.to_toml().as_bytes(),
        Some(previous),
    )?;
    Ok(true)
}

// -- switch -----------------------------------------------------------------------------

fn switch(invocation: &mut Invocation, name: &str) -> Result<(), CliError> {
    let store = invocation.env.store()?;
    registered(&view(&store)?, name).ok_or_else(not_found)?;
    let mut transaction = begin(invocation, &store, false)?;
    let tsg = switch_locked(&mut transaction, &store, name)?;
    invocation.committed = true;
    finish(invocation, transaction);
    invocation
        .output
        .success(&format!("Selected {name} (TSG {tsg})"));
    Ok(())
}

/// Under the lock: re-check, read the pinned tenant file (no credential check: a file
/// with only `mgmtTsgId` can be selected), set `active` and write `config.toml`
/// (always). The pinned TSG.
fn switch_locked(
    transaction: &mut Transaction,
    store: &Store,
    name: &str,
) -> Result<String, CliError> {
    let locked = view(store)?;
    let selected = registered(&locked, name).ok_or_else(not_found)?;
    locked.load_for_write(&selected, true)?;
    let mut index = locked.index;
    index.active = Some(name.to_owned());
    transaction.write(&store.index_path(), index.to_toml().as_bytes(), None)?;
    Ok(selected.tsg_id)
}

// -- delete -----------------------------------------------------------------------------

fn delete(invocation: &mut Invocation, name: &str, force: bool) -> Result<(), CliError> {
    if !force && !invocation.terminals.interactive() {
        return Err(CliError::usage(format!(
            "refusing to delete tenant {name} without --force in non-interactive mode"
        )));
    }
    let store = invocation.env.store()?;
    let unlocked = view(&store)?;
    registered(&unlocked, name).ok_or_else(not_found)?;
    if !force {
        let selected = unlocked.index.active.as_deref() == Some(name);
        let message = format!(
            "Delete tenant {name}?{} Its config file will be deleted.",
            if selected {
                " It is the selected tenant; no tenant will be selected afterwards."
            } else {
                ""
            }
        );
        stop_if_signalled(invocation)?;
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        if !Prompter::new(&mut input, &mut invocation.output, true).confirm(&message) {
            invocation.output.info("Aborted");
            return Ok(());
        }
    }
    let mut transaction = begin(invocation, &store, false)?;
    let deleted = delete_locked(&mut transaction, &store, name)?;
    invocation.committed = true;
    finish(invocation, transaction);
    if !deleted.file_removed {
        return Err(CliError::operational(format!(
            "Deleted tenant {name}, but {} could not be removed; remove it by hand",
            shown(&store.tenant_path(name))
        )));
    }
    invocation.output.success(&format!(
        "Deleted {name} and its config file{}",
        if deleted.was_selected {
            "; no tenant is selected"
        } else {
            ""
        }
    ));
    Ok(())
}

/// What a delete did once `config.toml` was written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Deleted {
    /// The locked state had the tenant selected (the selection is now cleared).
    was_selected: bool,
    /// The tenant file is gone (or was never there).
    file_removed: bool,
}

/// Under the lock: re-check, remove the entry (clearing `active` when it names the
/// tenant), write `config.toml` (the commit point), then unlink the tenant file (a
/// missing file is fine; a symlink is removed, not its target). The tenant file is
/// never read, so a missing, corrupt or mismatched one can be deleted.
fn delete_locked(
    transaction: &mut Transaction,
    store: &Store,
    name: &str,
) -> Result<Deleted, CliError> {
    let locked = view(store)?;
    registered(&locked, name).ok_or_else(not_found)?;
    let mut index = locked.index;
    let was_selected = index.active.as_deref() == Some(name);
    index.tenants.remove(name);
    if was_selected {
        index.active = None;
    }
    transaction.write(&store.index_path(), index.to_toml().as_bytes(), None)?;
    let file_removed = match transaction.remove(&store.tenant_path(name)) {
        Ok(()) => true,
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    };
    Ok(Deleted {
        was_selected,
        file_removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::FromArgMatches;

    fn planned(args: &[&str]) -> Result<WritePlan, CliError> {
        let mut full = vec!["airs", "tenant"];
        full.extend_from_slice(args);
        let matches = crate::cli::command().try_get_matches_from(&full).unwrap();
        let cli = Cli::from_arg_matches(&matches).unwrap();
        match cli.command {
            Some(crate::cli::TopCommand::Tenant(group)) => plan(&group.command.unwrap()),
            _ => unreachable!(),
        }
    }

    fn usage(args: &[&str]) -> String {
        match planned(args) {
            Err(CliError::Usage(message)) => message,
            other => panic!("expected a usage error for {args:?}, got {other:?}"),
        }
    }

    #[test]
    fn create_checks_the_name_then_the_flag_values() {
        for name in ["a/b", "", "_x", "é", &"a".repeat(65), "space name"] {
            assert_eq!(usage(&["create", "--", name]), NAME_RULE, "{name:?}");
        }
        assert_eq!(
            usage(&["create", "_x", "--tsg-id", "abc"]),
            NAME_RULE,
            "the name first"
        );
        assert_eq!(
            usage(&["create", "dev", "--tsg-id", ""]),
            config::BLANK_CREDENTIAL
        );
        assert_eq!(
            usage(&["create", "dev", "--tsg-id", " 100"]),
            "Invalid mgmtTsgId: the Rust SDK requires 1-32 ASCII digits"
        );
        assert_eq!(
            usage(&["create", "dev", "--tsg-id", "100", "--client-id", " "]),
            config::BLANK_CREDENTIAL
        );
        assert_eq!(
            usage(&["create", "dev", "--client-id", "a\u{1b}[31m"]),
            config::INVALID_VALUE
        );
        assert_eq!(
            planned(&[
                "create",
                "dev",
                "--tsg-id",
                "100",
                "--client-id",
                " c ",
                "--client-secret-stdin"
            ]),
            Ok(WritePlan::Create {
                name: "dev".into(),
                tsg: Some("100".into()),
                client_id: Some(" c ".into()),
                secret_stdin: true,
            })
        );
        assert_eq!(
            planned(&["create", &"a".repeat(64)]).map(|_| ()),
            Ok(()),
            "64 characters are allowed"
        );
    }

    #[test]
    fn set_checks_in_typescript_order() {
        let unknown = usage(&["set", "dev", "aiGwDataEndpoint", "x"]);
        assert!(unknown.starts_with("Unknown configuration key. Supported keys: airsApiKey,"));
        assert!(unknown.ends_with("scanConcurrency, defaultOutput"));
        for key in [
            "__proto__",
            "constructor",
            "dataDir",
            "redTeamTokenEndpoint",
            "typesafeApiKey",
        ] {
            assert_eq!(usage(&["set", "dev", key, "x"]), unknown, "{key}");
        }
        assert_eq!(
            usage(&["set", "dev", "airsApiKey", "X", "--stdin"]),
            "Use a value or --stdin, not both",
            "before the secret check"
        );
        for key in [
            "airsApiKey",
            "airsApiToken",
            "mgmtClientSecret",
            "gatewayInferenceApiKey",
        ] {
            for value in ["FAKE-SECRET", ""] {
                assert_eq!(
                    usage(&["set", "dev", key, value]),
                    "Do not pass credentials as arguments. Omit the value for a hidden prompt, or use --stdin.",
                    "{key} {value:?}"
                );
            }
        }
        assert_eq!(
            usage(&["set", "dev", "scanConcurrency", "-1"]),
            config::INVALID_VALUE
        );
        assert_eq!(
            usage(&["set", "dev", "defaultOutput", "yaml"]),
            config::INVALID_VALUE
        );
        assert_eq!(
            usage(&["set", "dev", "mgmtEndpoint", ""]),
            "Empty values are not stored; use 'airs tenant unset dev mgmtEndpoint' to remove a setting"
        );
        assert_eq!(
            planned(&["set", "dev", "scanConcurrency", "07"]),
            Ok(WritePlan::Set {
                name: "dev".into(),
                key: "scanConcurrency",
                value: Some(ConfigValue::Integer(7)),
                stdin: false,
            })
        );
        assert_eq!(
            planned(&["set", "dev", "airsApiKey", "--stdin"]),
            Ok(WritePlan::Set {
                name: "dev".into(),
                key: "airsApiKey",
                value: None,
                stdin: true,
            })
        );
    }

    #[test]
    fn unset_refuses_unknown_keys_and_credentials() {
        assert!(usage(&["unset", "dev", "custom"]).starts_with("Unknown configuration key"));
        for key in ["mgmtTsgId", "mgmtClientId", "mgmtClientSecret"] {
            assert_eq!(
                usage(&["unset", "nope", key]),
                format!("{key} is a credential and cannot be cleared; set a new value instead")
            );
        }
        assert_eq!(
            planned(&["unset", "dev", "airsApiKey"]),
            Ok(WritePlan::Unset {
                name: "dev".into(),
                key: "airsApiKey"
            })
        );
        assert_eq!(
            planned(&["switch", "a/b"]),
            Ok(WritePlan::Switch { name: "a/b".into() }),
            "switch looks names up"
        );
        assert_eq!(
            planned(&["rm", "dev"]),
            Ok(WritePlan::Delete {
                name: "dev".into(),
                force: false
            })
        );
    }

    use crate::store::atomic::{Step, fail_writes};
    use crate::store::lock::Silent;
    use crate::store::testdir::TestDir;

    const DEV: &[u8] = b"mgmtClientId = \"c\"\nmgmtClientSecret = \"s\"\nmgmtTsgId = \"100\"\n";
    const INDEX: &[u8] = b"version = 1\nactive = \"dev\"\n\n[tenants.dev]\ntsg_id = \"100\"\n";

    fn store_with_dev(dir: &TestDir) -> Store {
        dir.write("D/config.toml", INDEX, 0o600);
        dir.write("D/tenants/dev.toml", DEV, 0o600);
        Store::at(dir.path().join("D"))
    }

    fn read(store: &Store, path: &std::path::Path) -> Vec<u8> {
        let _ = store;
        std::fs::read(path).unwrap()
    }

    #[test]
    fn a_failed_index_write_unlinks_the_new_tenant_file() {
        let dir = TestDir::new("write-create-rollback");
        let store = store_with_dev(&dir);
        let config = TenantConfig::from_values([("mgmtTsgId", ConfigValue::Text("200".into()))]);
        let mut tx = Transaction::begin(&store, true, &mut Silent).unwrap();
        // The tenant file (write 1) succeeds, config.toml (write 2) fails at the rename.
        fail_writes(Some(Step::Rename), &[2]);
        let error = create_locked(&mut tx, &store, "prod", &config, "200").unwrap_err();
        fail_writes(None, &[]);
        assert_eq!(
            error.message(),
            format!(
                "Cannot save {}; it was not changed",
                store.index_path().display()
            )
        );
        assert!(
            !store.tenant_path("prod").exists(),
            "the new file was unlinked"
        );
        assert_eq!(read(&store, &store.index_path()), INDEX);
        // Success: both files, the commit point last.
        create_locked(&mut tx, &store, "prod", &config, "200").unwrap();
        assert_eq!(
            read(&store, &store.tenant_path("prod")),
            b"mgmtTsgId = \"200\"\n"
        );
        assert_eq!(
            read(&store, &store.index_path()),
            b"version = 1\nactive = \"dev\"\n\n[tenants.dev]\ntsg_id = \"100\"\n\n[tenants.prod]\ntsg_id = \"200\"\n"
        );
        drop(tx.commit());
    }

    #[test]
    fn a_failed_pin_write_restores_the_tenant_file_or_says_to_run_again() {
        let dir = TestDir::new("write-set-rollback");
        let store = store_with_dev(&dir);
        let tsg = || ConfigValue::Text("300".into());
        let mut tx = Transaction::begin(&store, false, &mut Silent).unwrap();
        fail_writes(Some(Step::Rename), &[2]);
        let error = set_locked(&mut tx, &store, "dev", "mgmtTsgId", tsg()).unwrap_err();
        fail_writes(None, &[]);
        assert_eq!(
            error.message(),
            format!(
                "Cannot save {}; it was not changed",
                store.index_path().display()
            )
        );
        assert_eq!(read(&store, &store.tenant_path("dev")), DEV, "restored");
        assert_eq!(read(&store, &store.index_path()), INDEX);
        // The restore (write 3) fails too.
        fail_writes(Some(Step::Rename), &[2, 3]);
        let error = set_locked(&mut tx, &store, "dev", "mgmtTsgId", tsg()).unwrap_err();
        fail_writes(None, &[]);
        assert_eq!(
            error.message(),
            format!(
                "Cannot save {}; run the command again to finish the change",
                store.index_path().display()
            )
        );
        assert_eq!(
            read(&store, &store.index_path()),
            INDEX,
            "the pin is unchanged"
        );
        // Running it again finishes the change, and repairs the mismatch it left.
        set_locked(&mut tx, &store, "dev", "mgmtTsgId", tsg()).unwrap();
        assert_eq!(
            read(&store, &store.tenant_path("dev")),
            b"mgmtClientId = \"c\"\nmgmtClientSecret = \"s\"\nmgmtTsgId = \"300\"\n"
        );
        assert!(
            String::from_utf8(read(&store, &store.index_path()))
                .unwrap()
                .contains("tsg_id = \"300\"")
        );
        drop(tx.commit());
    }

    #[test]
    fn locked_steps_recheck_the_state() {
        let dir = TestDir::new("write-locked");
        let store = store_with_dev(&dir);
        let mut tx = Transaction::begin(&store, false, &mut Silent).unwrap();
        assert_eq!(
            set_locked(
                &mut tx,
                &store,
                "ghost",
                "defaultOutput",
                ConfigValue::Text("json".into())
            )
            .unwrap_err()
            .message(),
            "Tenant not found; register a named tenant first"
        );
        assert_eq!(
            switch_locked(&mut tx, &store, "ghost")
                .unwrap_err()
                .message(),
            "Tenant not found"
        );
        assert_eq!(switch_locked(&mut tx, &store, "dev").unwrap(), "100");
        assert!(!unset_locked(&mut tx, &store, "dev", "defaultOutput").unwrap());
        set_locked(
            &mut tx,
            &store,
            "dev",
            "defaultOutput",
            ConfigValue::Text("json".into()),
        )
        .unwrap();
        assert!(unset_locked(&mut tx, &store, "dev", "defaultOutput").unwrap());
        assert_eq!(
            read(&store, &store.tenant_path("dev")),
            DEV,
            "back to the canonical bytes"
        );
        // A pin mismatch blocks every key but mgmtTsgId.
        dir.write("D/tenants/dev.toml", b"mgmtTsgId = \"999\"\n", 0o600);
        let pin =
            "Tenant config TSG differs from its registered identity; register it under a new name";
        assert_eq!(
            switch_locked(&mut tx, &store, "dev").unwrap_err().message(),
            pin
        );
        assert_eq!(
            unset_locked(&mut tx, &store, "dev", "airsApiKey")
                .unwrap_err()
                .message(),
            pin
        );
        let deleted = delete_locked(&mut tx, &store, "dev").unwrap();
        assert_eq!(
            deleted,
            Deleted {
                was_selected: true,
                file_removed: true
            }
        );
        assert_eq!(read(&store, &store.index_path()), b"version = 1\n");
        assert!(!store.tenant_path("dev").exists());
        drop(tx.commit());
    }

    #[test]
    fn new_tenant_state_checks() {
        let mut index = Index::default();
        index.tenants.insert("dev".into(), "100".into());
        assert_eq!(
            check_new(&index, "dev").unwrap_err().message(),
            "Tenant name already exists"
        );
        assert_eq!(
            check_new(&index, "DEV").unwrap_err().message(),
            "Tenant name conflicts with existing tenant dev; names may not differ only in case"
        );
        assert!(check_new(&index, "prod").is_ok());
        for n in 0..999 {
            index.tenants.insert(format!("t{n}"), "1".into());
        }
        assert_eq!(index.tenants.len(), 1000);
        assert_eq!(
            check_new(&index, "prod").unwrap_err().message(),
            "Cannot create tenant prod: at most 1000 tenants can be registered"
        );
    }
}
