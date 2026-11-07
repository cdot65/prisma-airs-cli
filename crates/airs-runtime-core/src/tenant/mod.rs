//! The tenant credential model: validated tenant files and scanner bindings (DESIGN §3,
//! §17). Selection, locking and store writes stay in the CLI.
pub mod config;
pub mod mcp;
pub mod scanners;

use config::TenantConfig;

/// A selected tenant and how it was chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selected {
    pub name: String,
    /// The pinned TSG from `config.toml`.
    pub tsg_id: String,
    pub by_flag: bool,
}

/// Everything an API command needs about the selected tenant.
#[derive(Clone, Debug)]
pub struct TenantContext {
    pub selected: Selected,
    pub config: TenantConfig,
}

impl TenantContext {
    pub fn name(&self) -> &str {
        &self.selected.name
    }
}
