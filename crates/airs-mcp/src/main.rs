//! `airs-mcp`: the optional protected runtime MCP service (plan M2; contract §2, §4, §11).
//!
//! Logs are one JSON object per line and carry only fixed codes, claim *shapes*,
//! identifiers allowed by contract §2.5 and storage metadata.

use std::process::ExitCode;

use airs_mcp::run;

/// Write a line to stdout or stderr without panicking: a failed or closed stream
/// drops the line and the command keeps its intended exit status.
fn out(line: &str) {
    use std::io::Write;
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{line}").and_then(|()| o.flush());
}

fn err(line: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr().lock(), "{line}");
}

fn usage() -> ExitCode {
    err(
        "usage:\n  airs-mcp serve --config PATH\n  airs-mcp check-config --config PATH\n  \
         airs-mcp control-backup --config PATH --dir NEW_DIR\n  \
         airs-mcp control-restore --config PATH --set DIR\n  \
         airs-mcp control-activate-epoch --config PATH --epoch HEX8\n  \
         airs-mcp control-reconcile --config PATH\n  \
         airs-mcp migrate-archive --config PATH --source DIR",
    );
    ExitCode::from(2)
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let mut found = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == name {
            if found.is_some() {
                return None; // repeated scalar flags are refused
            }
            found = it.next().map(String::as_str);
        }
    }
    found
}

fn emit(value: serde_json::Value) {
    out(&value.to_string());
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = |name: &str| flag(&args, name).map(std::path::PathBuf::from);
    match args.first().map(String::as_str) {
        Some(
            command @ ("control-backup"
            | "control-restore"
            | "control-activate-epoch"
            | "control-reconcile"
            | "migrate-archive"),
        ) => {
            let Some(p) = path("--config") else {
                return usage();
            };
            let action = match command {
                "control-backup" => path("--dir").map(run::Maintenance::Backup),
                "control-restore" => path("--set").map(run::Maintenance::Restore),
                "control-reconcile" => Some(run::Maintenance::Reconcile),
                "migrate-archive" => path("--source").map(run::Maintenance::MigrateArchive),
                _ => flag(&args, "--epoch")
                    .filter(|e| e.len() == 8 && e.bytes().all(|b| b.is_ascii_hexdigit()))
                    .map(|e| run::Maintenance::ActivateEpoch(e.to_owned())),
            };
            let Some(action) = action else {
                return usage();
            };
            match run::maintenance(&p, action) {
                Ok(v) => {
                    emit(v);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    err(&e.to_string());
                    ExitCode::from(1)
                }
            }
        }
        Some("check-config") => {
            let Some(p) = path("--config") else {
                return usage();
            };
            match run::load_config(&p) {
                Ok(c) => {
                    emit(
                        serde_json::json!({"event": "config_ok", "revision": c.revision, "grants": c.grants.len()}),
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    err(&e.to_string());
                    ExitCode::from(1)
                }
            }
        }
        Some("serve") => {
            let Some(p) = path("--config") else {
                return usage();
            };
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            match rt.block_on(run::serve(p)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    err(&e.to_string());
                    ExitCode::from(1)
                }
            }
        }
        _ => usage(),
    }
}
