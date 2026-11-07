//! `airs-mcp-spike`: the M0.6 disposable protected echo/info backend.
//!
//! It holds no AIRS credentials and no customer history. Logs are one JSON object
//! per line and carry only fixed codes, claim *shapes* and storage metadata.

use std::process::ExitCode;
use std::sync::Arc;

use airs_mcp_spike::{run, storage};

fn usage() -> ExitCode {
    eprintln!(
        "usage:\n  airs-mcp-spike serve --config PATH\n  airs-mcp-spike check-config --config PATH\n  \
         airs-mcp-spike storage-probe --data-dir DIR [--hold-seconds N]\n  \
         airs-mcp-spike drill seed --data-dir DIR --rows N --marker TEXT\n  \
         airs-mcp-spike drill backup --data-dir DIR --marker TEXT   (writes DIR/backups/backup-<id>/)\n  \
         airs-mcp-spike drill verify --set DIR --marker TEXT"
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
    println!("{value}");
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = |name: &str| flag(&args, name).map(std::path::PathBuf::from);
    match args.first().map(String::as_str) {
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
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            }
        }
        Some("serve") => {
            let Some(p) = path("--config") else {
                return usage();
            };
            let config = match run::load_config(&p) {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(1);
                }
            };
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            match rt.block_on(run::serve(config)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            }
        }
        Some("storage-probe") => {
            let Some(d) = path("--data-dir") else {
                return usage();
            };
            let hold: u64 = flag(&args, "--hold-seconds")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let admitted = match storage::admit(&d) {
                Ok(a) => a,
                Err(e) => {
                    emit(serde_json::json!({"event": "storage_refused", "reason": e}));
                    return ExitCode::from(1);
                }
            };
            match storage::acquire(&admitted) {
                Ok(owner) => {
                    emit(
                        serde_json::json!({"event": "storage_owned", "admitted": admitted, "owner": owner.record}),
                    );
                    std::thread::sleep(std::time::Duration::from_secs(hold));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    emit(serde_json::json!({"event": "storage_refused", "reason": e}));
                    ExitCode::from(1)
                }
            }
        }
        Some("drill") => {
            let marker = flag(&args, "--marker").unwrap_or_default();
            if marker.len() < 8 {
                return usage();
            }
            let result = match args.get(1).map(String::as_str) {
                Some("seed") => {
                    let (Some(d), Some(rows)) = (
                        path("--data-dir"),
                        flag(&args, "--rows").and_then(|r| r.parse().ok()),
                    ) else {
                        return usage();
                    };
                    storage::admit(&d)
                        .and_then(|a| storage::drill_seed(&a, rows, marker))
                        .map(|_| serde_json::json!({"event": "drill_seeded", "rows": rows}))
                }
                Some("backup") => {
                    let Some(d) = path("--data-dir") else {
                        return usage();
                    };
                    storage::admit(&d).and_then(|a| {
                        let out = storage::private_child(&a, "backups")?;
                        storage::drill_backup(&a, &out, marker)
                            .map(|set| serde_json::json!({"event": "drill_backup", "set": set.file_name().map(|n| n.to_string_lossy().into_owned())}))
                    })
                }
                Some("verify") => {
                    let Some(s) = path("--set") else {
                        return usage();
                    };
                    storage::drill_verify(&s, marker)
                        .map(|m| serde_json::json!({"event": "drill_verified", "manifest": m}))
                }
                _ => return usage(),
            };
            match result {
                Ok(v) => {
                    emit(v);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    emit(serde_json::json!({"event": "drill_failed", "reason": e}));
                    ExitCode::from(1)
                }
            }
        }
        _ => usage(),
    }
}
