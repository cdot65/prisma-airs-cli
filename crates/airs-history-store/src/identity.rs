//! Public endpoint/TSG namespace only; no OAuth or scanner credential discovery.
use super::failure;
use airs_runtime_core::{clients, error::CliError, tenant::TenantContext};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cloud {
    pub tsg: String,
    pub endpoints: [String; 3],
    pub environment_id: String,
    pub cloud_id: String,
}

impl Cloud {
    pub fn from_tenant(tenant: &TenantContext) -> Result<Self, CliError> {
        let endpoints = airs_runtime_core::cloud::tenant_endpoints(tenant)?;
        Self::new(&tenant.selected.tsg_id, endpoints)
    }

    /// The cloud identity of an explicitly configured service (contract §11 `[airs]`): the
    /// management, dashboard and token endpoints, admitted and canonicalized exactly as
    /// [`Cloud::from_tenant`] does, so the same endpoints name the same archive.
    pub fn from_parts(tsg: &str, endpoints: [String; 3]) -> Result<Self, CliError> {
        let keys = ["mgmtEndpoint", "mgmtDashboardEndpoint", "mgmtTokenEndpoint"];
        let mut canonical: [String; 3] = Default::default();
        for (i, key) in keys.iter().enumerate() {
            canonical[i] = clients::admit(key, Some(&endpoints[i]), &endpoints[i])?
                .url()
                .as_str()
                .trim_end_matches('/')
                .to_owned();
        }
        Self::new(tsg, canonical)
    }

    fn new(tsg: &str, endpoints: [String; 3]) -> Result<Self, CliError> {
        let (environment_id, cloud_id) = airs_runtime_core::cloud::ids(tsg, &endpoints)?;
        Ok(Self {
            tsg: tsg.to_owned(),
            endpoints,
            environment_id,
            cloud_id,
        })
    }

    pub fn descriptor(&self) -> Result<String, CliError> {
        serde_json::to_string(&self.endpoints).map_err(|_| failure("invalid cloud descriptor"))
    }
    pub fn check_public(
        &self,
        secrets: &airs_runtime_core::error::SecretSet,
    ) -> Result<(), CliError> {
        for (key, value) in ["mgmtEndpoint", "mgmtDashboardEndpoint", "mgmtTokenEndpoint"]
            .iter()
            .zip(&self.endpoints)
        {
            let canonical = clients::admit(key, Some(value), value)?
                .url()
                .as_str()
                .trim_end_matches('/')
                .to_owned();
            if canonical != *value {
                return Err(failure("cloud descriptor is not canonical"));
            }
        }
        if Self::new(&self.tsg, self.endpoints.clone())? != *self {
            return Err(failure("cloud descriptor identity is invalid"));
        }
        if self
            .endpoints
            .iter()
            .any(|endpoint| secrets.carried_by(endpoint))
        {
            Err(failure(
                "the cloud descriptor contains credential material and cannot be archived",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use airs_runtime_core::tenant::{Selected, config::TenantConfig};
    fn tenant(text: &str, name: &str, tsg: &str) -> TenantContext {
        TenantContext {
            selected: Selected {
                name: name.into(),
                tsg_id: tsg.into(),
                by_flag: false,
            },
            config: TenantConfig::parse(text.as_bytes()).unwrap(),
        }
    }
    #[test]
    fn explicit_parts_name_the_same_cloud_as_an_explicit_tenant() {
        let text = "mgmtEndpoint='https://m.example.test/aisec/'\nmgmtDashboardEndpoint='https://m.example.test/aisec'\nmgmtTokenEndpoint='https://auth.example.test/oauth2/access_token'";
        let from_tenant = Cloud::from_tenant(&tenant(text, "one", "123")).unwrap();
        let from_parts = Cloud::from_parts(
            "123",
            [
                "https://m.example.test/aisec/".into(),
                "https://m.example.test/aisec".into(),
                "https://auth.example.test/oauth2/access_token".into(),
            ],
        )
        .unwrap();
        assert_eq!(from_parts, from_tenant);
        assert!(Cloud::from_parts("12x", from_parts.endpoints.clone()).is_err());
        assert!(
            Cloud::from_parts(
                "123",
                [
                    "http://m.example.test/aisec".into(),
                    from_parts.endpoints[1].clone(),
                    from_parts.endpoints[2].clone(),
                ],
            )
            .is_err(),
            "plain http off loopback is refused"
        );
    }

    #[test]
    fn aliases_and_rotations_share_only_the_exact_public_cloud() {
        let _isolation = crate::test_isolation();
        let a = Cloud::from_tenant(&tenant("airsApiKey='key-a'", "one", "123")).unwrap();
        let b = Cloud::from_tenant(&tenant("airsApiKey='key-b'", "two", "123")).unwrap();
        assert_eq!(a, b);
        assert_ne!(
            a.cloud_id,
            Cloud::from_tenant(&tenant("", "two", "124"))
                .unwrap()
                .cloud_id
        );
        assert!(!a.descriptor().unwrap().contains("key-"));
        assert!(
            Cloud::from_tenant(&tenant(
                "mgmtEndpoint='https://example.invalid/aisec'",
                "two",
                "123"
            ))
            .is_err()
        );
        let text = "mgmtEndpoint='https://example.invalid/aisec/'\nmgmtDashboardEndpoint='https://example.invalid/aisec/'\nmgmtTokenEndpoint='https://example.invalid/token/'";
        let custom = Cloud::from_tenant(&tenant(text, "two", "123")).unwrap();
        assert_ne!(a.cloud_id, custom.cloud_id);
        assert_eq!(
            custom,
            Cloud::from_tenant(&tenant(&text.replace("/'", "'"), "other", "123")).unwrap()
        );
        for tsg in ["", "12x", "１２", "123456789012345678901234567890123"] {
            assert!(Cloud::from_tenant(&tenant("", "two", tsg)).is_err());
        }
    }

    #[test]
    fn persisted_descriptors_reject_forged_identity_and_credential_paths() {
        let _isolation = crate::test_isolation();
        let mut cloud = Cloud::from_tenant(&tenant("", "one", "123")).unwrap();
        let secrets = airs_runtime_core::error::SecretSet::default();
        cloud.check_public(&secrets).unwrap();
        cloud.cloud_id = "0".repeat(64);
        assert!(cloud.check_public(&secrets).is_err());
        let cloud = Cloud::new(
            "123",
            [
                "https://example.invalid/synthetic-private-credential".into(),
                "https://example.invalid/dashboard".into(),
                "https://example.invalid/token".into(),
            ],
        )
        .unwrap();
        let secrets = airs_runtime_core::error::SecretSet::default();
        secrets.add("synthetic-private-credential");
        assert!(cloud.check_public(&secrets).is_err());
    }
}
