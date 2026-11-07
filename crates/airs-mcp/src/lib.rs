//! `airs-mcp`: the optional protected runtime MCP service (plan M2–M8). The library half
//! lets loopback tests drive the real server.
pub mod airs_tenant;
pub mod auth;
pub mod authority_store;
pub mod catalogue;
pub mod collection;
pub mod config;
pub mod control;
pub mod daily_store;
pub mod ingest_store;
pub mod jobs;
pub mod lifecycle;
pub mod metrics;
pub mod migrate;
pub mod report_html;
pub mod reports;
pub mod run;
pub mod schedule_store;
pub mod storage;
pub mod tools;
pub mod worker;

/// The M5 exit gate's shared adversarial transfer cases (test-only; the same file the
/// store driver's suite runs).
#[cfg(test)]
#[path = "../../airs-history-store/tests/support/transfer_cases.rs"]
pub(crate) mod transfer_cases;
