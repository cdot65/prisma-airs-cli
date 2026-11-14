//! `airs-runtime-core`: the code the standalone `airs` CLI and the optional `airs-mcp`
//! service share (RUNTIME_MCP_IMPLEMENTATION_PLAN "Workspace boundary"; contract §1).
//!
//! It owns failures and the secret set, ordered JSON, terminal-safe text, the tenant
//! credential model and its validators, SDK client construction with URL admission,
//! the read budget, the scan verdict, privacy projection and withholding, the Signal
//! Brief evidence model, analysis and renderers, and the shared live collector with
//! its [`signal::sink::HistorySink`] seam. It also holds the contract-level service code
//! frozen in M0: the versioned tool DTOs (`dto`, `schemas/`), the scheduling rules
//! (`schedule`, contract §8) and the administrative-authority state machine
//! (`authority`, contract §3). It owns no SQLite, no listener and no CLI
//! I/O (`Invocation`, output streams, tenant selection or store writes).
#![forbid(unsafe_code)]

pub mod authority;
pub mod clients;
pub mod cloud;
pub mod debug;
pub mod dto;
pub mod error;
pub mod files;
pub mod json;
pub mod limits;
pub mod output_format;
pub mod profile_policy;
pub mod read_budget;
pub mod scan_logs;
pub mod scan_verdict;
pub mod schedule;
pub mod secrets;
pub mod signal;
pub mod store;
pub mod style;
pub mod tenant;
pub mod text;
pub mod time;
pub mod withhold;
pub mod withhold_rules;

#[cfg(test)]
pub(crate) mod test_support;

/// The version every collected evidence document records (lockstep with `airs-cli`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The SCM SDK version this core is compiled against (the exact `=0.2.6` pin).
pub const SDK_VERSION: &str = "0.2.6";

#[cfg(test)]
mod package_tests {
    /// The published package carries the repository's own license and notice, byte for
    /// byte (review M1-1 minor: the first package had neither).
    #[test]
    fn the_package_notices_are_the_repository_notices() {
        let crate_dir = env!("CARGO_MANIFEST_DIR");
        for name in ["LICENSE", "NOTICE"] {
            let ours = std::fs::read(format!("{crate_dir}/{name}")).unwrap();
            let root = std::fs::read(format!("{crate_dir}/../../{name}")).unwrap();
            assert_eq!(ours, root, "{name} differs from the repository's");
        }
        let manifest = std::fs::read_to_string(format!("{crate_dir}/Cargo.toml")).unwrap();
        assert!(manifest.contains(r#""/LICENSE", "/NOTICE""#));
    }
}
