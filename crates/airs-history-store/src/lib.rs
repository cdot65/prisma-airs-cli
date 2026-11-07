//! `airs-history-store`: the server-owned runtime history archive (plan M1.2;
//! RUNTIME_MCP_CONTRACT §1, §9). It owns the private SQLite archive per public cloud
//! identity, its admission, reconciliation, retry work, receipts, sync state, transfer,
//! compaction and recovery. Only the `airs-mcp` service links it; the `airs` CLI never
//! does (its package graph contains no SQLite).
#![forbid(unsafe_code)]
mod database;
pub mod identity;
pub mod inspect;
mod paths;
pub mod query;
pub mod receipts;
pub mod report;
pub mod repository;
pub mod run_summary;
pub mod sync;
// Unfinished B4 transfer work, preserved for server-side completion (M5 wires it). Not
// reachable from any service operation yet.
#[cfg(test)]
pub(crate) mod test_support;
#[allow(dead_code)]
pub mod transfer;
mod work;
pub use database::{SCHEMA_V1, compact, now_ms, recover, recovery_identity};
pub use paths::{root, validate_override};

use airs_runtime_core::error::CliError;

/// Never include SQLite/server/file text or paths in archive failures.
pub fn failure(reason: &str) -> CliError {
    CliError::operational(format!("Runtime history: {reason}"))
}

/// Independent archive fixtures share a test filesystem. Isolate their setup and
/// cleanup from each other so the real 250ms retention policy measures each case's
/// intentional contention, rather than unrelated synthetic archive saturation.
/// Child processes have their own gate; tested process/file locks stay concurrent.
#[cfg(test)]
pub(crate) fn test_isolation() -> std::sync::MutexGuard<'static, ()> {
    static GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    GATE.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
