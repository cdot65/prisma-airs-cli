//! Tenant selection from the TOML store, `--tenant`, name resolution, the import
//! suffix and the shared tenant-file loader (DESIGN §3.4, §3.6, §3.7).
//!
//! Readers never write, create, lock, chmod or rename anything. The TypeScript
//! registry is not read here: only [`ts::registry_exists`] looks at its path, for the
//! import suffix.

pub use airs_runtime_core::tenant::config;
pub mod import;
pub use airs_runtime_core::tenant::scanners;
pub use airs_runtime_core::tenant::{Selected, TenantContext};
pub mod ts;

use crate::error::CliError;
use crate::store::{
    self, Index, PROGRAM, Store, TenantFileRead, shown, unsafe_dir_error, unsafe_file_error,
};
use config::{ContentError, TenantConfig};
use std::ffi::OsString;
use std::path::PathBuf;

pub use crate::store::valid_tenant_name;

/// The variables the store and the TypeScript registry locations depend on, captured
/// once per invocation. They choose where files are, never what they contain (§4).
#[derive(Clone, Debug, Default)]
pub struct Environment {
    /// `AIRS_CLI_CONFIG_DIR` (non-empty).
    pub config_dir: Option<OsString>,
    /// `XDG_CONFIG_HOME` (non-empty).
    pub xdg_config_home: Option<OsString>,
    /// `PRISMA_AIRS_TENANTS_PATH` (non-empty; the TypeScript registry only).
    pub tenants_path: Option<OsString>,
    /// `XDG_STATE_HOME` (non-empty; the debug log and the TypeScript registry).
    pub xdg_state_home: Option<OsString>,
    /// `HOME` as set (even empty), else the password database.
    pub home: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
}

impl Environment {
    pub fn capture() -> Self {
        let non_empty = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        Self {
            config_dir: non_empty("AIRS_CLI_CONFIG_DIR"),
            xdg_config_home: non_empty("XDG_CONFIG_HOME"),
            tenants_path: non_empty("PRISMA_AIRS_TENANTS_PATH"),
            xdg_state_home: non_empty("XDG_STATE_HOME"),
            // `HOME` exactly as set, even empty; only an unset `HOME` falls back to the
            // password database.
            home: std::env::var_os("HOME")
                .map(PathBuf::from)
                .or_else(std::env::home_dir),
            cwd: std::env::current_dir().ok(),
        }
    }

    /// The store directory `D` (§3.1).
    pub fn store(&self) -> Result<Store, CliError> {
        Store::locate(
            self.config_dir.as_deref(),
            self.xdg_config_home.as_deref(),
            self.home.as_deref(),
        )
    }
}

/// The tenant-name rule's message (TypeScript text, U+2013 in `1–64`).
pub const NAME_RULE: &str = "Tenant name must be 1–64 letters, digits, hyphens or underscores and start with a letter/digit";

/// Usage check for `--tenant` (before the store is read).
pub fn validate_tenant_flag(name: Option<&str>) -> Result<(), CliError> {
    match name {
        Some(name) if !valid_tenant_name(name) => Err(CliError::usage(NAME_RULE)),
        _ => Ok(()),
    }
}

/// Why a tenant file could not be used, in check order (§3.3, §3.7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigLoadError {
    /// `D/tenants` or the file is missing.
    NotFound,
    /// The §3.8 checks refused this directory or file.
    Unsafe(PathBuf),
    /// Larger than 1 MiB, or unreadable.
    Unreadable,
    /// The content failed §3.3 step 2.
    Content(ContentError),
    /// `mgmtTsgId` differs from the pin (or is missing).
    TsgMismatch,
}

/// The store as one read command sees it: `D`, `config.toml` and, when `config.toml`
/// does not exist, the TypeScript registry that the import suffix names.
#[derive(Clone, Debug)]
pub struct StoreView {
    pub store: Store,
    pub index: Index,
    /// The existing TypeScript registry, only when `config.toml` does not exist.
    pub ts_registry: Option<PathBuf>,
}

impl StoreView {
    /// Locate `D` (§3.1) and read `config.toml` (§3.2).
    pub fn open(env: &Environment) -> Result<Self, CliError> {
        let store = env.store()?;
        let index = store.read_index()?;
        let ts_registry = if index.exists {
            None
        } else {
            ts::registry_exists(env)
        };
        Ok(Self {
            store,
            index,
            ts_registry,
        })
    }

    /// The import suffix (§3.4, D90), when it applies.
    pub fn import_suffix(&self) -> String {
        match &self.ts_registry {
            Some(path) => format!(
                "; a TypeScript tenant registry exists at {}: run '{PROGRAM} tenant import' to copy it",
                shown(path)
            ),
            None => String::new(),
        }
    }

    fn selected(&self, name: &str, by_flag: bool) -> Option<Selected> {
        self.index.tsg(name).map(|tsg| Selected {
            name: name.to_owned(),
            tsg_id: tsg.to_owned(),
            by_flag,
        })
    }

    /// `--tenant` selects without changing the store; otherwise `active`.
    pub fn select(&self, flag: Option<&str>) -> Result<Option<Selected>, CliError> {
        if let Some(name) = flag {
            return self
                .selected(name, true)
                .map(Some)
                .ok_or_else(|| self.tenant_not_found(name));
        }
        Ok(self
            .index
            .active
            .as_deref()
            .and_then(|name| self.selected(name, false)))
    }

    /// `Tenant not found: <name> (registered: …)` (§3.4).
    pub fn tenant_not_found(&self, name: &str) -> CliError {
        CliError::operational(format!(
            "Tenant not found: {name} {}",
            self.index.registered_suffix()
        ))
    }

    /// The error for API commands and doctor when nothing is selected (§3.4).
    pub fn no_tenant_selected(&self) -> CliError {
        if self.index.tenants.is_empty() {
            CliError::operational(format!(
                "No tenant selected. Run '{PROGRAM} tenant create <name>' and then '{PROGRAM} tenant switch <name>'{}",
                self.import_suffix()
            ))
        } else {
            CliError::operational(format!(
                "No tenant selected. Run '{PROGRAM} tenant switch <name>' (registered: {})",
                self.index.names().join(", ")
            ))
        }
    }

    /// Name resolution for `tenant read|path|get` (§3.6): the positional name, else
    /// `--tenant`, else `active`. An empty positional name selects nothing (the TS falsy
    /// check), and neither `--tenant` nor `active` stands in for it.
    pub fn resolve(&self, name: Option<&str>, flag: Option<&str>) -> Result<Selected, CliError> {
        match name {
            Some("") => {}
            Some(name) => {
                return self
                    .selected(name, false)
                    .ok_or_else(|| CliError::operational("Tenant not found"));
            }
            None => {
                if let Some(selected) = self.select(flag)? {
                    return Ok(selected);
                }
            }
        }
        if self.index.tenants.is_empty() {
            Err(CliError::operational(format!(
                "No tenant selected; run '{PROGRAM} tenant create <name>' first{}",
                self.import_suffix()
            )))
        } else {
            Err(CliError::operational(format!(
                "No tenant selected; pass a name or run '{PROGRAM} tenant switch <name>' (registered: {})",
                self.index.names().join(", ")
            )))
        }
    }

    /// `D/tenants/<name>.toml`.
    pub fn tenant_path(&self, name: &str) -> PathBuf {
        self.store.tenant_path(name)
    }

    /// Read, validate and pin a tenant file: the one strict reader that commands and
    /// doctor share (D21). Commands collapse the failure ([`StoreView::load_config`]);
    /// doctor reports each case.
    pub fn load_checked(&self, selected: &Selected) -> Result<TenantConfig, ConfigLoadError> {
        self.load_raw(selected, true).map(|(config, _)| config)
    }

    /// [`StoreView::load_checked`] returning the file's bytes too; `pin: false` skips the
    /// TSG pin (`tenant set <name> mgmtTsgId` changes it, D83).
    fn load_raw(
        &self,
        selected: &Selected,
        pin: bool,
    ) -> Result<(TenantConfig, Vec<u8>), ConfigLoadError> {
        let bytes = self
            .store
            .read_tenant_bytes(&selected.name)
            .map_err(|error| match error {
                TenantFileRead::NotFound => ConfigLoadError::NotFound,
                TenantFileRead::Unsafe(path) => ConfigLoadError::Unsafe(path),
                TenantFileRead::Unreadable => ConfigLoadError::Unreadable,
            })?;
        let config = TenantConfig::parse(&bytes).map_err(ConfigLoadError::Content)?;
        if pin && config.effective_str("mgmtTsgId") != Some(selected.tsg_id.as_str()) {
            return Err(ConfigLoadError::TsgMismatch);
        }
        Ok((config, bytes))
    }

    /// [`StoreView::load_checked`] with the command messages, which echo nothing from
    /// the file.
    pub fn load_config(&self, selected: &Selected) -> Result<TenantConfig, CliError> {
        self.load_checked(selected)
            .map_err(|error| self.load_error(selected, error))
    }

    /// The writers' read of a tenant file under the lock (§3.11): the config and its
    /// current bytes (kept for rollback), with the command messages.
    pub fn load_for_write(
        &self,
        selected: &Selected,
        pin: bool,
    ) -> Result<(TenantConfig, Vec<u8>), CliError> {
        self.load_raw(selected, pin)
            .map_err(|error| self.load_error(selected, error))
    }

    fn load_error(&self, selected: &Selected, error: ConfigLoadError) -> CliError {
        match error {
            ConfigLoadError::TsgMismatch => CliError::operational(
                "Tenant config TSG differs from its registered identity; register it under a new name",
            ),
            ConfigLoadError::Unsafe(path) if path == self.store.tenants_dir() => {
                unsafe_dir_error(&path)
            }
            ConfigLoadError::Unsafe(path) => unsafe_file_error(&path),
            ConfigLoadError::NotFound
            | ConfigLoadError::Unreadable
            | ConfigLoadError::Content(_) => {
                store::tenant_file_error(&self.tenant_path(&selected.name))
            }
        }
    }
}

/// Resolve, read and validate the selected tenant (store, selection, file, pin).
pub fn load_selected(env: &Environment, flag: Option<&str>) -> Result<TenantContext, CliError> {
    let view = StoreView::open(env)?;
    let selected = view
        .select(flag)?
        .ok_or_else(|| view.no_tenant_selected())?;
    let config = view.load_config(&selected)?;
    Ok(TenantContext { selected, config })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testdir::TestDir;

    fn env(dir: &TestDir) -> Environment {
        Environment {
            config_dir: Some(dir.path().join("D").into_os_string()),
            tenants_path: Some(dir.path().join("ts/tenants.json").into_os_string()),
            home: Some(PathBuf::from("/nonexistent-home")),
            cwd: Some(dir.path().to_path_buf()),
            ..Environment::default()
        }
    }

    const INDEX: &[u8] =
        b"version = 1\n\n[tenants.bad]\ntsg_id = \"100\"\n\n[tenants.dev]\ntsg_id = \"100\"\n";

    #[test]
    fn tenant_flag_names_are_usage_checked() {
        assert!(validate_tenant_flag(Some("-bad")).is_err());
        assert!(validate_tenant_flag(Some("ok")).is_ok());
        assert!(validate_tenant_flag(None).is_ok());
    }

    #[test]
    fn selection_and_messages() {
        let dir = TestDir::new("tenant-select");
        dir.write("D/config.toml", INDEX, 0o600);
        dir.write(
            "D/tenants/dev.toml",
            b"mgmtTsgId = \"100\"\nmgmtClientId = \"c\"\nmgmtClientSecret = \"s\"\n",
            0o600,
        );
        dir.write("D/tenants/bad.toml", b"mgmtTsgId = \"200\"\n", 0o600);
        let view = StoreView::open(&env(&dir)).unwrap();
        assert_eq!(view.import_suffix(), "");
        assert_eq!(
            view.no_tenant_selected().message(),
            "No tenant selected. Run 'airs tenant switch <name>' (registered: bad, dev)"
        );
        assert_eq!(view.select(None).unwrap(), None);
        let selected = view.select(Some("dev")).unwrap().unwrap();
        assert!(selected.by_flag);
        assert_eq!(selected.tsg_id, "100");
        assert_eq!(
            view.select(Some("ghost")).unwrap_err().message(),
            "Tenant not found: ghost (registered: bad, dev)"
        );
        assert!(view.load_config(&selected).is_ok());
        let bad = view.resolve(Some("bad"), None).unwrap();
        assert_eq!(
            view.load_config(&bad).unwrap_err().message(),
            "Tenant config TSG differs from its registered identity; register it under a new name"
        );
        assert_eq!(
            view.resolve(Some("Dev"), None).unwrap_err().message(),
            "Tenant not found",
            "exact lookups"
        );
        assert_eq!(
            view.resolve(Some(""), Some("dev")).unwrap_err().message(),
            "No tenant selected; pass a name or run 'airs tenant switch <name>' (registered: bad, dev)"
        );
        assert_eq!(view.resolve(None, Some("dev")).unwrap().name, "dev");
        let context = load_selected(&env(&dir), Some("dev")).unwrap();
        assert_eq!(context.name(), "dev");
        assert!(load_selected(&env(&dir), None).is_err());
    }

    #[test]
    fn an_empty_store_and_the_import_suffix() {
        let dir = TestDir::new("tenant-empty");
        let view = StoreView::open(&env(&dir)).unwrap();
        assert!(!view.index.exists);
        assert_eq!(
            view.no_tenant_selected().message(),
            "No tenant selected. Run 'airs tenant create <name>' and then 'airs tenant switch <name>'"
        );
        assert_eq!(
            view.select(Some("ghost")).unwrap_err().message(),
            "Tenant not found: ghost (no tenants registered)"
        );
        let registry = dir.write("ts/tenants.json", b"{}", 0o600);
        let view = StoreView::open(&env(&dir)).unwrap();
        let suffix = format!(
            "; a TypeScript tenant registry exists at {}: run 'airs tenant import' to copy it",
            registry.display()
        );
        assert_eq!(
            view.no_tenant_selected().message(),
            format!(
                "No tenant selected. Run 'airs tenant create <name>' and then 'airs tenant switch <name>'{suffix}"
            )
        );
        assert_eq!(
            view.resolve(None, None).unwrap_err().message(),
            format!("No tenant selected; run 'airs tenant create <name>' first{suffix}")
        );
        // An existing (even empty) config.toml turns the suffix off.
        dir.write("D/config.toml", b"version = 1\n", 0o600);
        let view = StoreView::open(&env(&dir)).unwrap();
        assert_eq!(view.import_suffix(), "");
    }

    #[test]
    fn tenant_file_failures_collapse_for_commands() {
        let dir = TestDir::new("tenant-files");
        dir.write("D/config.toml", INDEX, 0o600);
        let view = StoreView::open(&env(&dir)).unwrap();
        let dev = view.resolve(Some("dev"), None).unwrap();
        let path = dir.path().join("D/tenants/dev.toml");
        let generic = format!("Cannot read a valid tenant config at {}", path.display());
        assert_eq!(view.load_checked(&dev), Err(ConfigLoadError::NotFound));
        assert_eq!(view.load_config(&dev).unwrap_err().message(), generic);
        for bytes in [
            &b"mgmtTsgId = \"100\"\naiGwDataEndpoint = \"x\"\n"[..],
            b"mgmtTsgId = \"100\"\ndefaultOutput = \"markdown\"\n",
            b"mgmtTsgId = \"100\"\nmgmtClientSecret = \"SUPER-SECRET\"\nnope",
        ] {
            dir.write("D/tenants/dev.toml", bytes, 0o600);
            let message = view.load_config(&dev).unwrap_err().message();
            assert_eq!(message, generic);
            assert!(!message.contains("SUPER-SECRET"));
        }
        dir.write("D/tenants/dev.toml", b"mgmtTsgId = \"100\"\n", 0o640);
        assert!(
            view.load_config(&dev)
                .unwrap_err()
                .message()
                .starts_with(&format!(
                    "Refusing to use {}: it must be a regular file",
                    path.display()
                ))
        );
        dir.write("D/tenants/dev.toml", b"mgmtTsgId = \"100\"\n", 0o600);
        assert!(view.load_config(&dev).is_ok());
        dir.write("D/tenants/dev.toml", b"", 0o600);
        assert_eq!(
            view.load_checked(&dev),
            Err(ConfigLoadError::TsgMismatch),
            "a file without mgmtTsgId mismatches"
        );
    }
}
