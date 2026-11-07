//! The archive's public cloud identity (contract §11 `[airs]`, plan "History transfer"):
//! the TSG and the canonical management, dashboard and token endpoints, hashed into
//! `environment_id` and `cloud_id`. Shared so the optional CLI client can name the
//! archive a service holds (`receipts_ingest`'s `cloud_id`) without the history store.
use crate::{clients, error::CliError, tenant::TenantContext};
use sha2::{Digest, Sha256};

fn failure(reason: &str) -> CliError {
    CliError::operational(format!("Runtime history: {reason}"))
}

/// The tenant's endpoints, admitted and canonicalized (defaults for a public tenant;
/// a custom environment must name all three).
pub fn tenant_endpoints(tenant: &TenantContext) -> Result<[String; 3], CliError> {
    let config = &tenant.config;
    let keys = ["mgmtEndpoint", "mgmtDashboardEndpoint", "mgmtTokenEndpoint"];
    let defaults = [
        clients::DEFAULT_MGMT_ENDPOINT,
        clients::DEFAULT_MGMT_ENDPOINT,
        clients::DEFAULT_TOKEN_ENDPOINT,
    ];
    let mut endpoints: [String; 3] = Default::default();
    let mut custom = false;
    for (i, key) in keys.iter().enumerate() {
        endpoints[i] = clients::admit(key, config.effective_str(key), defaults[i])?
            .url()
            .as_str()
            .trim_end_matches('/')
            .to_owned();
        custom |= endpoints[i] != defaults[i];
    }
    if custom && keys.iter().any(|key| config.effective_str(key).is_none()) {
        return Err(failure(
            "custom environments require explicit management, dashboard and token endpoints",
        ));
    }
    Ok(endpoints)
}

/// `(environment_id, cloud_id)` of a TSG and its canonical endpoints.
pub fn ids(tsg: &str, endpoints: &[String; 3]) -> Result<(String, String), CliError> {
    if tsg.is_empty() || tsg.len() > 32 || !tsg.bytes().all(|b| b.is_ascii_digit()) {
        return Err(failure("a pinned TSG of 1–32 ASCII digits is required"));
    }
    let tuple = serde_json::to_vec(endpoints).map_err(|_| failure("invalid cloud descriptor"))?;
    let environment_id = format!("env1:{:x}", Sha256::digest(tuple));
    let key = serde_json::to_vec(&(1, tsg, &environment_id))
        .map_err(|_| failure("invalid cloud descriptor"))?;
    Ok((environment_id, format!("{:x}", Sha256::digest(key))))
}

/// The cloud identity of the selected tenant: `(tsg, environment_id, cloud_id)`.
pub fn of_tenant(tenant: &TenantContext) -> Result<(String, String, String), CliError> {
    let endpoints = tenant_endpoints(tenant)?;
    let (env, cloud) = ids(&tenant.selected.tsg_id, &endpoints)?;
    Ok((tenant.selected.tsg_id.clone(), env, cloud))
}
