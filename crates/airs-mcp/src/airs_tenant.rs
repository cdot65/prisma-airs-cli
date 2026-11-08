//! The service's AIRS tenant (contract §11 `[airs]`): management and scanner
//! credentials read from their projected secret files, validated with the CLI's own
//! tenant and scanner validators (`TenantConfig::parse`), and registered in the job's
//! `SecretSet`. The secret bytes never enter `Config`, logs or identities.

use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use airs_runtime_core::error::SecretSet;
use airs_runtime_core::store::toml_string;
use airs_runtime_core::tenant::config::TenantConfig;
use airs_runtime_core::tenant::{Selected, TenantContext};

use crate::config::Config;

/// The largest secret file accepted (contract §11).
pub const MAX_SECRET_BYTES: u64 = 16 * 1024;

/// A fixed refusal: names the setting, never a path's contents or parser text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantError(pub &'static str);

impl std::fmt::Display for TenantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AIRS credentials refused: {}", self.0)
    }
}

/// One secret: a regular file (a projected Secret is a symlink into `..data`; its
/// target must be regular) of at most 16 KiB, UTF-8, trimmed, non-empty, printable.
fn read_secret(path: &Path, what: &'static str) -> Result<String, TenantError> {
    // Opened non-blocking (a FIFO or device cannot stall the worker), following the
    // projected Secret's symlink; only then is the opened target required to be a
    // regular file within the byte bound.
    let mut f: std::fs::File = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| TenantError(what))?
    .into();
    let m = f.metadata().map_err(|_| TenantError(what))?;
    if !m.is_file() || m.size() > MAX_SECRET_BYTES {
        return Err(TenantError(what));
    }
    let mut bytes = Vec::new();
    f.by_ref()
        .take(MAX_SECRET_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| TenantError(what))?;
    let text = String::from_utf8(bytes).map_err(|_| TenantError(what))?;
    let text = text.trim();
    if text.is_empty()
        || text.len() as u64 > MAX_SECRET_BYTES
        || !text.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(TenantError(what));
    }
    Ok(text.to_owned())
}

/// Build the tenant a collection runs against, registering every secret in `secrets`.
pub fn tenant(config: &Config, secrets: &SecretSet) -> Result<TenantContext, TenantError> {
    let a = &config.airs;
    let client_id = read_secret(&a.management_client_id_file, "management client ID file")?;
    let client_secret = read_secret(
        &a.management_client_secret_file,
        "management client secret file",
    )?;
    secrets.add(&client_id);
    secrets.add(&client_secret);
    let mut text = String::new();
    for (key, value) in [
        ("mgmtClientId", client_id.as_str()),
        ("mgmtClientSecret", client_secret.as_str()),
        ("mgmtTsgId", a.tsg.as_str()),
        ("mgmtEndpoint", a.management_endpoint.as_str()),
        ("mgmtDashboardEndpoint", a.dashboard_endpoint.as_str()),
        ("mgmtTokenEndpoint", a.token_endpoint.as_str()),
    ] {
        text.push_str(&format!("{key} = {}\n", toml_string(value)));
    }
    for s in &a.scanners {
        let (field, path) = match (&s.api_key_file, &s.api_token_file) {
            (Some(p), None) => ("apiKey", p),
            (None, Some(p)) => ("apiToken", p),
            _ => return Err(TenantError("scanner secret file")),
        };
        let secret = read_secret(path, "scanner secret file")?;
        secrets.add(&secret);
        text.push_str(&format!(
            "\n[scanners.{}]\napplicationId = {}\nendpoint = {}\nnumRetries = {}\nrevision = {}\n{field} = {}\n",
            toml_string(&s.name),
            toml_string(&s.application_id),
            toml_string(&s.endpoint),
            s.num_retries,
            toml_string(&s.revision),
            toml_string(&secret),
        ));
    }
    let parsed =
        TenantConfig::parse(text.as_bytes()).map_err(|_| TenantError("credential validation"));
    // The document held secrets: drop it before returning either result.
    drop(text);
    Ok(TenantContext {
        selected: Selected {
            name: config.service.tenant.clone(),
            tsg_id: a.tsg.clone(),
            by_flag: false,
        },
        config: parsed?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M4 review 1 minor: a FIFO as a secret file is refused at once (no blocking open).
    #[test]
    fn a_fifo_secret_file_is_refused_without_blocking() {
        let dir = std::env::temp_dir().join(format!("airs-tenant-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let fifo = dir.join("secret");
        // POSIX mkfifo(1): rustix has no mknodat/mkfifoat on macOS.
        use std::os::unix::fs::FileTypeExt;
        let made = std::process::Command::new("mkfifo")
            .args(["-m", "600"])
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(made.success(), "mkfifo failed");
        assert!(std::fs::metadata(&fifo).unwrap().file_type().is_fifo());
        let started = std::time::Instant::now();
        assert!(read_secret(&fifo, "secret file").is_err());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "no blocking open"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
