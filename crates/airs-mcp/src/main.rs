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
         airs-mcp control-backup-set --config PATH [--dir DIR] [--keep N]\n  \
         airs-mcp control-backup-verify (--set DIR | --latest-in DIR)\n  \
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

/// Whether `args` (after the command) are exactly `--flag value` pairs of `allowed`
/// flags, each at most once, with a value that is not itself a flag. A maintenance
/// command with anything else (a missing value, a repeated or unknown flag) is a usage
/// error before any storage is admitted (M8 review 3).
fn strict_flags(args: &[String], allowed: &[&str]) -> bool {
    let rest = &args[1.min(args.len())..];
    if !rest.len().is_multiple_of(2) {
        return false;
    }
    let mut seen: Vec<&str> = Vec::new();
    for pair in rest.chunks(2) {
        let (name, value) = (pair[0].as_str(), pair[1].as_str());
        if !allowed.contains(&name) || seen.contains(&name) || value.starts_with("--") {
            return false;
        }
        seen.push(name);
    }
    true
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
            | "control-backup-set"
            | "control-restore"
            | "control-activate-epoch"
            | "control-reconcile"
            | "migrate-archive"),
        ) => {
            let allowed: &[&str] = match command {
                "control-backup" => &["--config", "--dir"],
                "control-backup-set" => &["--config", "--dir", "--keep"],
                "control-restore" => &["--config", "--set"],
                "control-activate-epoch" => &["--config", "--epoch"],
                "control-reconcile" => &["--config"],
                _ => &["--config", "--source"],
            };
            if !strict_flags(&args, allowed) {
                return usage();
            }
            let Some(p) = path("--config") else {
                return usage();
            };
            let action = match command {
                "control-backup" => path("--dir").map(run::Maintenance::Backup),
                "control-backup-set" => {
                    let keep = match flag(&args, "--keep") {
                        None => None,
                        Some(k) => match k.parse::<usize>() {
                            Ok(n) if n >= 1 => Some(n),
                            _ => return usage(),
                        },
                    };
                    Some(run::Maintenance::BackupSet(path("--dir"), keep))
                }
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
        Some("control-backup-verify") => {
            if !strict_flags(&args, &["--set", "--latest-in"]) {
                return usage();
            }
            // Read-only: verifies a set wherever it is (also a restored off-volume copy).
            // `--set DIR` names one set; `--latest-in DIR` verifies the newest complete set
            // there (the backup Job cannot pass the new set's ID: no shell).
            let set = match (path("--set"), path("--latest-in")) {
                (Some(set), None) => set,
                (None, Some(dir)) => match airs_mcp::backup_set::latest_complete(&dir) {
                    Ok(set) => set,
                    Err(e) => {
                        err(&e.to_string());
                        return ExitCode::from(1);
                    }
                },
                _ => return usage(),
            };
            match airs_mcp::backup_set::verify(&set) {
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
