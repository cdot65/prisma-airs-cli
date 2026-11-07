//! Local named scanner management; all configuration writes use Transaction.
use super::{Invocation, tenant_write};
use crate::{
    cli::{ScannersCommand, ScannersGroup},
    error::CliError,
    output::Format,
    store::{self, Store},
    tenant::{
        Selected, StoreView,
        config::TenantConfig,
        scanners::{self, ScannerCredential},
    },
};
use serde_json::{Value, json};
use std::os::unix::fs::MetadataExt;
use std::{
    io::Read,
    path::{Path, PathBuf},
};

pub struct ScannerPlan {
    tenant: String,
    action: Action,
    format: Format,
}
enum Action {
    List,
    Snapshots,
    Set {
        name: String,
        file: Option<PathBuf>,
        stdin: bool,
        replace: bool,
    },
    Bind {
        name: String,
        application: String,
        kind: Option<String>,
        replace: bool,
    },
    Remove {
        name: String,
    },
    Restore {
        file: PathBuf,
        expect_tsg: String,
    },
    DeleteSnapshot {
        file: PathBuf,
    },
}

pub fn plan(group: &ScannersGroup, format: Format) -> Result<super::Plan, CliError> {
    let Some(command) = &group.command else {
        return Ok(super::Plan::GroupHelp(&["tenant", "scanners"]));
    };
    let (tenant, action) = match command {
        ScannersCommand::List(a) => (&a.tenant, Action::List),
        ScannersCommand::Snapshots(a) => (&a.tenant, Action::Snapshots),
        ScannersCommand::Set(a) => {
            scanners::validate_name(&a.name)?;
            if a.stdin == a.file.is_some() {
                return Err(CliError::usage("Supply --file or --stdin"));
            }
            if a.file.as_ref().is_some_and(|p| p.as_os_str().is_empty()) {
                return Err(CliError::usage("--file cannot be empty"));
            }
            (
                &a.tenant,
                Action::Set {
                    name: a.name.clone(),
                    file: a.file.clone(),
                    stdin: a.stdin,
                    replace: a.replace,
                },
            )
        }
        ScannersCommand::BindDefault(a) => {
            scanners::validate_name(&a.name)?;
            if !scanners::valid_application(&a.application_id) {
                return Err(CliError::usage(
                    "--app-id must be nonblank, at most 4096 bytes and contain no controls",
                ));
            }
            if a.kind
                .as_deref()
                .is_some_and(|kind| !["key", "token"].contains(&kind))
            {
                return Err(CliError::usage("--use must be key or token"));
            }
            (
                &a.tenant,
                Action::Bind {
                    name: a.name.clone(),
                    application: a.application_id.clone(),
                    kind: a.kind.clone(),
                    replace: a.replace,
                },
            )
        }
        ScannersCommand::Remove(a) => {
            scanners::validate_name(&a.name)?;
            if !a.force {
                return Err(CliError::usage("--force is required"));
            }
            (
                &a.tenant,
                Action::Remove {
                    name: a.name.clone(),
                },
            )
        }
        ScannersCommand::Restore(a) => {
            if !a.force {
                return Err(CliError::usage("--force is required"));
            }
            crate::tenant::config::write_rule(&a.tenant, "mgmtTsgId", &a.expect_tsg)
                .map_err(CliError::usage)?;
            if a.file.as_os_str().is_empty() {
                return Err(CliError::usage("--file cannot be empty"));
            }
            (
                &a.tenant,
                Action::Restore {
                    file: a.file.clone(),
                    expect_tsg: a.expect_tsg.clone(),
                },
            )
        }
        ScannersCommand::DeleteSnapshot(a) => {
            if !a.force {
                return Err(CliError::usage("--force is required"));
            }
            if a.file.as_os_str().is_empty() {
                return Err(CliError::usage("--file cannot be empty"));
            }
            (
                &a.tenant,
                Action::DeleteSnapshot {
                    file: a.file.clone(),
                },
            )
        }
    };
    if !store::valid_tenant_name(tenant) {
        return Err(CliError::usage(crate::tenant::NAME_RULE));
    }
    Ok(super::Plan::TenantScanners(ScannerPlan {
        tenant: tenant.clone(),
        action,
        format,
    }))
}

fn snapshot_error() -> CliError {
    CliError::operational("Scanner snapshot does not match the selected tenant")
}

fn snapshot_name(name: &str, tenant: &str) -> bool {
    ["pre-scanners", "pre-restore"].iter().any(|kind| {
        name.strip_prefix(&format!(".{tenant}.{kind}."))
            .and_then(|rest| rest.strip_suffix(".toml"))
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok_and(|v| v.to_string() == id))
    })
}

fn snapshot_path(store: &Store, tenant: &str, file: &Path) -> Result<PathBuf, CliError> {
    let name = file
        .file_name()
        .and_then(|v| v.to_str())
        .filter(|name| snapshot_name(name, tenant))
        .ok_or_else(snapshot_error)?;
    let parent = file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let supplied = parent.canonicalize().map_err(|_| snapshot_error())?;
    let expected = store
        .tenants_dir()
        .canonicalize()
        .map_err(|_| snapshot_error())?;
    if supplied != expected {
        return Err(snapshot_error());
    }
    Ok(store.tenants_dir().join(name))
}

fn view(store: &Store) -> Result<StoreView, CliError> {
    Ok(StoreView {
        store: store.clone(),
        index: store.read_index()?,
        ts_registry: None,
    })
}
fn selected(view: &StoreView, name: &str) -> Result<Selected, CliError> {
    Ok(Selected {
        name: name.into(),
        tsg_id: view
            .index
            .tsg(name)
            .ok_or_else(|| CliError::operational("Tenant not found"))?
            .into(),
        by_flag: false,
    })
}

fn emit(inv: &mut Invocation, mut value: Value, format: Format) -> Result<(), CliError> {
    scanners::safe_public_value(&mut value, &inv.secrets)?;
    match format {
        Format::Json => inv.output.json(&value),
        Format::Pretty | Format::Table => {
            let text = |v: &Value| match v {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                _ => v.to_string(),
            };
            if let Some(map) = value.as_object() {
                let rows: Vec<Vec<String>> = map
                    .iter()
                    .map(|(name, v)| {
                        vec![
                            name.clone(),
                            text(&v["applicationId"]),
                            text(&v["endpoint"]),
                            text(&v["numRetries"]),
                            text(&v["revision"]),
                            text(&v["hasApiKey"]),
                            text(&v["hasApiToken"]),
                        ]
                    })
                    .collect();
                inv.output.data_line(&crate::render::table::table(
                    &[
                        "Scanner",
                        "Application",
                        "Endpoint",
                        "Retries",
                        "Revision",
                        "Key",
                        "Token",
                    ],
                    &rows,
                ));
            } else if let Some(items) = value.as_array() {
                let rows: Vec<Vec<String>> = items
                    .iter()
                    .map(|v| vec![text(&v["path"]), text(&v["bytes"])])
                    .collect();
                inv.output
                    .data_line(&crate::render::table::table(&["Snapshot", "Bytes"], &rows));
            }
            Ok(())
        }
    }
}

pub fn run(inv: &mut Invocation, plan: ScannerPlan) -> Result<(), CliError> {
    inv.secrets.guard_local_output();
    // Consume bounded input before a tenant/store read and before acquiring locks.
    let input = if let Action::Set { file, stdin, .. } = &plan.action {
        let bytes = if *stdin {
            if inv.terminals.stdin {
                return Err(CliError::usage(crate::input::NEEDS_PIPE));
            }
            let mut bytes = Vec::new();
            std::io::stdin()
                .lock()
                .take(store::MAX_FILE_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| CliError::operational("Cannot read private scanner input"))?;
            bytes
        } else {
            store::read_checked(
                file.as_ref()
                    .ok_or_else(|| CliError::usage("Supply --file or --stdin"))?,
            )
            .map_err(|_| CliError::operational("Cannot read private scanner input"))?
        };
        let scanner = ScannerCredential::input(&bytes)?;
        for secret in scanner.secrets() {
            inv.secrets.add(secret);
        }
        Some(scanner)
    } else {
        None
    };
    let store = inv.env.store()?;
    let initial = view(&store)?;
    let registration = selected(&initial, &plan.tenant)?;
    let config = initial.load_config(&registration)?;
    inv.secrets.register_config(&config);
    match &plan.action {
        Action::List => {
            let data =
                serde_json::to_value(config.shown("scanners")).map_err(|_| snapshot_error())?;
            return emit(inv, data, plan.format);
        }
        Action::Snapshots => {
            let mut snapshots = Vec::new();
            for entry in std::fs::read_dir(store.tenants_dir()).map_err(|_| snapshot_error())? {
                let entry = entry.map_err(|_| snapshot_error())?;
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                if !snapshot_name(&name, &plan.tenant) {
                    continue;
                }
                let file = store::open_checked(&entry.path(), rustix::fs::OFlags::RDONLY)
                    .map_err(|_| snapshot_error())?;
                let meta = file.metadata().map_err(|_| snapshot_error())?;
                snapshots.push(json!({"path":entry.path().to_string_lossy(),"bytes":meta.len()}));
            }
            snapshots.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
            return emit(inv, json!(snapshots), plan.format);
        }
        _ => {}
    }
    // Match other tenant writers: ordinary state/input failures must not even
    // create or rewrite config.lock. Repeat these checks after acquiring it.
    match &plan.action {
        Action::Set { name, replace, .. } => {
            let scanner = input
                .clone()
                .ok_or_else(|| CliError::usage(scanners::INPUT_ERROR))?;
            config.clone().set_scanner(name, scanner, *replace)?;
        }
        Action::Bind {
            name,
            application,
            kind,
            replace,
        } => {
            let scanner = ScannerCredential::from_flat(&config, application, kind.as_deref())?;
            config.clone().set_scanner(name, scanner, *replace)?;
        }
        Action::Remove { name } => {
            if !config.scanners().is_some_and(|map| map.contains_key(name)) {
                return Err(CliError::operational("Selected scanner is not configured"));
            }
        }
        Action::Restore { file, expect_tsg } => {
            if expect_tsg != &registration.tsg_id {
                return Err(snapshot_error());
            }
            let path = snapshot_path(&store, &plan.tenant, file)?;
            let bytes = store::read_checked(&path).map_err(|_| snapshot_error())?;
            let restored = TenantConfig::parse(&bytes).map_err(|_| snapshot_error())?;
            inv.secrets.register_config(&restored);
            if restored.effective_str("mgmtTsgId") != Some(registration.tsg_id.as_str()) {
                return Err(snapshot_error());
            }
        }
        Action::DeleteSnapshot { file } => {
            let path = snapshot_path(&store, &plan.tenant, file)?;
            store::open_checked(&path, rustix::fs::OFlags::RDONLY).map_err(|_| snapshot_error())?;
        }
        Action::List | Action::Snapshots => unreachable!(),
    }
    let mut tx = tenant_write::begin(inv, &store, true)?;
    let mut snapshot = None;
    let result = (|| -> Result<(), CliError> {
        let current = view(&store)?;
        let registration = selected(&current, &plan.tenant)?;
        let (mut config, previous) = current.load_for_write(&registration, true)?;
        inv.secrets.register_config(&config);
        match &plan.action {
            Action::Set { name, replace, .. } | Action::Bind { name, replace, .. } => {
                let scanner = match &plan.action {
                    Action::Bind {
                        application, kind, ..
                    } => ScannerCredential::from_flat(&config, application, kind.as_deref())?,
                    _ => input
                        .clone()
                        .ok_or_else(|| CliError::usage(scanners::INPUT_ERROR))?,
                };
                let upgrading = config.scanners().is_none();
                config.set_scanner(name, scanner, *replace)?;
                if upgrading {
                    inv.output.status("Named scanner settings require a new CLI; saving a private rollback snapshot before the format upgrade.");
                    snapshot =
                        Some(tx.scanner_snapshot(&plan.tenant, "pre-scanners", &previous)?);
                }
                tx.write(
                    &store.tenant_path(&plan.tenant),
                    config.to_toml().as_bytes(),
                    Some(previous),
                )?;
            }
            Action::Remove { name } => {
                if !config.remove_scanner(name) {
                    return Err(CliError::operational("Selected scanner is not configured"));
                }
                tx.write(
                    &store.tenant_path(&plan.tenant),
                    config.to_toml().as_bytes(),
                    Some(previous),
                )?;
            }
            Action::Restore { file, expect_tsg } => {
                if expect_tsg != &registration.tsg_id {
                    return Err(snapshot_error());
                }
                let path = snapshot_path(&store, &plan.tenant, file)?;
                let bytes = store::read_checked(&path).map_err(|_| snapshot_error())?;
                let restored = TenantConfig::parse(&bytes).map_err(|_| snapshot_error())?;
                inv.secrets.register_config(&restored);
                if restored.effective_str("mgmtTsgId") != Some(registration.tsg_id.as_str()) {
                    return Err(snapshot_error());
                }
                snapshot = Some(tx.scanner_snapshot(&plan.tenant, "pre-restore", &previous)?);
                tx.write(&store.tenant_path(&plan.tenant), &bytes, Some(previous))?;
            }
            Action::DeleteSnapshot { file } => {
                let path = snapshot_path(&store, &plan.tenant, file)?;
                let opened = store::open_checked(&path, rustix::fs::OFlags::RDONLY)
                    .map_err(|_| snapshot_error())?;
                let original = opened.metadata().map_err(|_| snapshot_error())?;
                let named = std::fs::symlink_metadata(&path).map_err(|_| snapshot_error())?;
                if !named.is_file()
                    || named.dev() != original.dev()
                    || named.ino() != original.ino()
                {
                    return Err(snapshot_error());
                }
                tx.remove(&path).map_err(|_| snapshot_error())?;
            }
            Action::List | Action::Snapshots => unreachable!(),
        }
        Ok(())
    })();
    if let Err(error) = result {
        let rolled_back = tx.rollback();
        tenant_write::finish(inv, tx);
        return Err(if rolled_back {
            error
        } else {
            CliError::operational(
                "Scanner change failed; verify the tenant configuration and rollback snapshots",
            )
        });
    }
    inv.committed = true;
    tenant_write::finish(inv, tx);
    inv.output.success("Scanner configuration change saved");
    if let Some(path) = snapshot {
        inv.output.status(&format!(
            "Private rollback snapshot: {}",
            inv.secrets.scrub(&store::shown(&path))
        ));
    }
    Ok(())
}
