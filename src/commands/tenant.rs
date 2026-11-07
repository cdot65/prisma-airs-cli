//! The tenant read commands on the TOML store: `tenant list|ls`, `path`, `read`, `get`
//! (DESIGN §3.6).

use super::Invocation;
use crate::error::CliError;
use crate::output::{Format, terminal_safe};
use crate::render::{pretty, table};
use crate::store::{PROGRAM, shown};
use crate::tenant::StoreView;
use crate::tenant::config::{ConfigValue, Shown};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TenantRow<'a> {
    name: &'a str,
    config_path: &'a str,
    tsg_id: &'a str,
    active: bool,
}

#[derive(Serialize)]
struct SettingRow<'a> {
    key: &'a str,
    value: &'a Shown,
}

fn store_of(invocation: &Invocation) -> Result<StoreView, CliError> {
    StoreView::open(&invocation.env)
}

/// The `tenant list` table columns (R§3.5).
const LIST_COLUMNS: [&str; 4] = ["Tenant", "Selected", "TSG ID", "Config file"];

/// `tenant list`: `config.toml` rows only; tenant files are never opened. `pretty` is
/// the table (TS parity); an empty store prints `No tenants found` in pretty and an
/// empty table line in table (§3.6).
pub fn list(invocation: &mut Invocation, format: Format) -> Result<(), CliError> {
    let view = store_of(invocation)?;
    let output = &mut invocation.output;
    let index = &view.index;
    if index.tenants.is_empty() && format == Format::Pretty {
        pretty::empty_list(output, "tenants");
        output.status(&format!(
            "Run '{PROGRAM} tenant create <name>' to register one{}.",
            view.import_suffix()
        ));
        return Ok(());
    }
    if index.active.is_none() && !index.tenants.is_empty() {
        output.status(&format!(
            "No tenant selected; run '{PROGRAM} tenant switch <name>'."
        ));
    }
    let paths: Vec<String> = index
        .tenants
        .keys()
        .map(|name| view.tenant_path(name).display().to_string())
        .collect();
    let rows: Vec<TenantRow<'_>> = index
        .tenants
        .iter()
        .zip(&paths)
        .map(|((name, tsg_id), config_path)| TenantRow {
            name,
            config_path,
            tsg_id,
            active: index.active.as_deref() == Some(name.as_str()),
        })
        .collect();
    if format == Format::Json {
        return output.json(&rows);
    }
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            vec![
                terminal_safe(row.name).into_owned(),
                row.active.to_string(),
                terminal_safe(row.tsg_id).into_owned(),
                terminal_safe(row.config_path).into_owned(),
            ]
        })
        .collect();
    // The table line is printed even when empty (TS logs the empty render).
    output.data_line(&table::table(&LIST_COLUMNS, &cells));
    Ok(())
}

/// A shown value as a table cell: strings terminal-safe, numbers in JS form.
fn shown_cell(value: &Shown) -> String {
    match value {
        Shown::Value(ConfigValue::Text(text)) => terminal_safe(text).into_owned(),
        Shown::Value(ConfigValue::Integer(number)) => {
            crate::render::js_integer(i128::from(*number))
        }
        Shown::Redacted(text) => (*text).to_owned(),
        Shown::Scanners(_) => terminal_safe(&value.display()).into_owned(),
    }
}

/// `tenant path [name]`: `D/tenants/<name>.toml`; the file is not opened.
pub fn path(invocation: &mut Invocation, name: Option<&str>) -> Result<(), CliError> {
    let view = store_of(invocation)?;
    let selected = view.resolve(name, invocation.tenant_flag.as_deref())?;
    // The store path is environment text: no control character reaches the terminal (D52).
    invocation
        .output
        .data_line(&shown(&view.tenant_path(&selected.name)));
    Ok(())
}

/// `tenant read [name]`: the schema-ordered effective config, secrets redacted.
pub fn read(
    invocation: &mut Invocation,
    name: Option<&str>,
    format: Format,
) -> Result<(), CliError> {
    let view = store_of(invocation)?;
    let selected = view.resolve(name, invocation.tenant_flag.as_deref())?;
    let config = view.load_config(&selected)?;
    invocation.secrets.register_config(&config);
    // `config.toml` text is file text: no control character reaches stderr (D52).
    let status = format!(
        "Tenant {} (TSG {}): {}",
        terminal_safe(&selected.name),
        terminal_safe(&selected.tsg_id),
        shown(&view.tenant_path(&selected.name))
    );
    invocation.output.status(if config.scanners().is_some() {
        invocation.secrets.scrub(&status)
    } else {
        &status
    });
    let rows = config.read_rows();
    let mut public_scanners = serde_json::to_value(config.shown("scanners"))
        .map_err(|_| CliError::operational("Scanner metadata cannot be safely displayed"))?;
    if config.scanners().is_some() {
        crate::tenant::scanners::safe_public_value(&mut public_scanners, &invocation.secrets)?;
    }
    if format == Format::Json {
        if config.scanners().is_some() {
            let mut rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|(key, value)| {
                    let public = if *key == "scanners" {
                        public_scanners.clone()
                    } else {
                        serde_json::to_value(value).map_err(|_| {
                            CliError::operational("Scanner metadata cannot be safely displayed")
                        })?
                    };
                    Ok(serde_json::json!({"key":key,"value":public}))
                })
                .collect::<Result<_, CliError>>()?;
            for row in &mut rows {
                crate::tenant::scanners::safe_public_value(row, &invocation.secrets)?;
            }
            return invocation.output.json(&rows);
        }
        let rows: Vec<SettingRow<'_>> = rows
            .iter()
            .map(|(key, value)| SettingRow { key, value })
            .collect();
        return invocation.output.json(&rows);
    }
    // `pretty` is the table (TS parity, R§3.6).
    let cells = rows
        .iter()
        .map(|(key, value)| {
            (
                (*key).to_owned(),
                if *key == "scanners" {
                    public_scanners.to_string()
                } else if config.scanners().is_some() {
                    invocation.secrets.scrub(&shown_cell(value)).to_owned()
                } else {
                    shown_cell(value)
                },
            )
        })
        .collect();
    invocation.output.data_line(&table::key_value_table(cells));
    Ok(())
}

/// `tenant get <name> <key>`: one effective setting, redacted.
pub fn get(
    invocation: &mut Invocation,
    name: &str,
    key: &'static str,
    format: Format,
) -> Result<(), CliError> {
    let view = store_of(invocation)?;
    // The positional name only: `--tenant` never stands in for it (TS `resolveEntry`).
    let selected = view.resolve(Some(name), None)?;
    let config = view.load_config(&selected)?;
    invocation.secrets.register_config(&config);
    if key == "scanners" || config.scanners().is_some() {
        let mut public = serde_json::to_value(config.shown(key))
            .map_err(|_| CliError::operational("Scanner metadata cannot be safely displayed"))?;
        crate::tenant::scanners::safe_public_value(&mut public, &invocation.secrets)?;
        let cell = public
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| public.to_string());
        return match format {
            Format::Json => invocation
                .output
                .json(&[serde_json::json!({"key":key,"value":public})]),
            Format::Pretty => {
                invocation.output.data_line(&cell);
                Ok(())
            }
            Format::Table => {
                invocation
                    .output
                    .data_line(&table::key_value_table(vec![(key.into(), cell)]));
                Ok(())
            }
        };
    }
    let value = config.shown(key);
    match format {
        Format::Json => invocation.output.json(&[SettingRow { key, value: &value }]),
        Format::Table => {
            let cells = vec![(key.to_owned(), shown_cell(&value))];
            invocation.output.data_line(&table::key_value_table(cells));
            Ok(())
        }
        // The bare value, never coloured (`String(value)`).
        Format::Pretty => {
            invocation
                .output
                .data_line(&terminal_safe(&value.display()));
            Ok(())
        }
    }
}
