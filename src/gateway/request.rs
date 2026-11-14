//! Pure preparation of gateway write bodies. Execution and preview consume the
//! same typed request; this module owns no files, terminals, clients or tokens.

use super::writes::{Typed, Write};
use crate::error::CliError;
use serde_json::Value;

/// Preflight validates before tenant loading; binding validates the actual
/// selected organisation before a request can be used for execution or preview.
pub enum Context<'a> {
    Preflight {
        explicit_organisation: Option<&'a str>,
    },
    Tenant {
        selected: &'a str,
        explicit_organisation: Option<&'a str>,
    },
}

/// A preparation refusal is classified where it originates, without parsing
/// user-facing text. The command adapter owns logging and presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    Validation,
    TenantBinding,
}

pub struct PreparationError {
    kind: FailureKind,
    error: CliError,
}

impl PreparationError {
    pub fn kind(&self) -> FailureKind {
        self.kind
    }
    pub fn into_cli_error(self) -> CliError {
        self.error
    }
    fn tenant(error: CliError) -> Self {
        Self {
            kind: FailureKind::TenantBinding,
            error,
        }
    }
}

impl std::fmt::Debug for PreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreparationError([REDACTED])")
    }
}

/// The body and its validated typed representation stay together. There is no
/// serialization or ordinary Debug implementation for this credential-bearing data.
pub struct PreparedRequest {
    body: Value,
    typed: Typed,
}

impl PreparedRequest {
    pub fn prepare(
        write: &Write,
        mut body: Value,
        organisation_in_body: bool,
        context: Context<'_>,
    ) -> Result<Self, PreparationError> {
        let (organisation, tsg) = match context {
            Context::Preflight {
                explicit_organisation,
            } => ("0", explicit_organisation),
            Context::Tenant {
                selected,
                explicit_organisation,
            } => {
                if selected.is_empty() {
                    return Err(PreparationError::tenant(CliError::operational(
                        "Selected tenant is missing mgmtTsgId; configure it before using gateway commands",
                    )));
                }
                super::check_tsg("tsgId", selected).map_err(PreparationError::tenant)?;
                if let Some(explicit) = explicit_organisation
                    && explicit != selected
                {
                    let flag = if *write == Write::AuthSettingsUpdate {
                        "--tsg-id"
                    } else {
                        "--organisation-id"
                    };
                    return Err(PreparationError::tenant(super::foreign_tsg(
                        flag,
                        &super::shown(explicit),
                    )));
                }
                if organisation_in_body
                    && let Some(org) = body.get("organisation_id").and_then(Value::as_str)
                    && org != selected
                {
                    return Err(PreparationError::tenant(super::foreign_tsg(
                        "--organisation-id",
                        &super::shown(org),
                    )));
                }
                (explicit_organisation.unwrap_or(selected), Some(selected))
            }
        };
        if organisation_in_body
            && let Some(map) = body.as_object_mut()
            && !map.contains_key("organisation_id")
        {
            map.insert(
                "organisation_id".into(),
                Value::String(organisation.to_owned()),
            );
        }
        let typed = Typed::parse(write, &body, tsg).map_err(|error| PreparationError {
            kind: FailureKind::Validation,
            error,
        })?;
        Ok(Self { body, typed })
    }

    pub fn body(&self) -> &Value {
        &self.body
    }

    pub fn typed(&self) -> &Typed {
        &self.typed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::inventory::KeyKind;
    use serde_json::json;

    fn key_body() -> Value {
        json!({"name": "synthetic", "type": "workspace-service", "scopes": ["completions.write"], "workspace_id": "11111111-1111-4111-8111-000000000001"})
    }

    #[test]
    fn preflight_and_binding_share_validation_but_resolve_different_context() {
        let write = Write::KeyCreate(KeyKind::Service);
        let preflight = PreparedRequest::prepare(
            &write,
            key_body(),
            true,
            Context::Preflight {
                explicit_organisation: None,
            },
        )
        .unwrap();
        assert_eq!(preflight.body()["organisation_id"], json!("0"));
        let bound = PreparedRequest::prepare(
            &write,
            key_body(),
            true,
            Context::Tenant {
                selected: "100",
                explicit_organisation: None,
            },
        )
        .unwrap();
        assert_eq!(bound.body()["organisation_id"], json!("100"));
        assert_eq!(
            bound.typed().credential_wire(&write).unwrap()["organisation_id"],
            json!("100")
        );
    }

    #[test]
    fn body_and_flag_organisations_are_checked_against_the_selected_tenant() {
        let write = Write::KeyCreate(KeyKind::Service);
        let mut foreign_body = key_body();
        foreign_body["organisation_id"] = json!("200");
        assert!(
            PreparedRequest::prepare(
                &write,
                foreign_body,
                true,
                Context::Tenant {
                    selected: "100",
                    explicit_organisation: None
                }
            )
            .is_err()
        );
        assert!(
            PreparedRequest::prepare(
                &write,
                key_body(),
                true,
                Context::Tenant {
                    selected: "100",
                    explicit_organisation: Some("200")
                }
            )
            .is_err()
        );
    }

    #[test]
    fn binding_preserves_an_explicit_matching_organisation() {
        let write = Write::KeyCreate(KeyKind::Service);
        let mut body = key_body();
        body["organisation_id"] = json!("100");
        let prepared = PreparedRequest::prepare(
            &write,
            body.clone(),
            true,
            Context::Tenant {
                selected: "100",
                explicit_organisation: Some("100"),
            },
        )
        .unwrap();
        assert_eq!(prepared.body(), &body);
    }

    #[test]
    fn a_deployment_body_is_preserved_and_uses_the_real_schema() {
        let write = Write::DeploymentUpdate("11111111-1111-4111-8111-000000000001".into());
        let context = || Context::Tenant {
            selected: "100",
            explicit_organisation: None,
        };
        let body = json!({"auth_settings": {"allow_all_workspaces": true}});
        let prepared = PreparedRequest::prepare(&write, body.clone(), false, context()).unwrap();
        assert_eq!(prepared.body(), &body);
        assert!(
            PreparedRequest::prepare(
                &write,
                json!({"auth_settings": {"allow_all_workspaces": 1}}),
                false,
                context()
            )
            .is_err()
        );
    }
    #[test]
    fn preparation_refusals_are_classified_without_inspecting_messages() {
        let write = Write::KeyCreate(KeyKind::Service);
        let mut body = key_body();
        body["organisation_id"] = json!("200");
        let error = PreparedRequest::prepare(
            &write,
            body,
            true,
            Context::Tenant {
                selected: "100",
                explicit_organisation: None,
            },
        )
        .err()
        .unwrap();
        assert_eq!(error.kind(), FailureKind::TenantBinding);
        let error = PreparedRequest::prepare(
            &write,
            json!({}),
            true,
            Context::Preflight {
                explicit_organisation: None,
            },
        )
        .err()
        .unwrap();
        assert_eq!(error.kind(), FailureKind::Validation);
        assert_eq!(format!("{error:?}"), "PreparationError([REDACTED])");
    }
    #[test]
    fn selected_tenant_format_is_a_shared_binding_check() {
        let write = Write::DeploymentUpdate("11111111-1111-4111-8111-000000000001".into());
        for selected in ["", "not-numeric", "123456789012345678901234567890123"] {
            let error = PreparedRequest::prepare(
                &write,
                json!({"auth_settings": {"allow_all_workspaces": true}}),
                false,
                Context::Tenant {
                    selected,
                    explicit_organisation: None,
                },
            )
            .err()
            .unwrap();
            assert_eq!(error.kind(), FailureKind::TenantBinding);
        }
    }
    #[test]
    fn a_missing_selected_tsg_names_the_required_configuration() {
        let write = Write::DeploymentUpdate("11111111-1111-4111-8111-000000000001".into());
        let error = PreparedRequest::prepare(
            &write,
            json!({"auth_settings": {"allow_all_workspaces": true}}),
            false,
            Context::Tenant {
                selected: "",
                explicit_organisation: None,
            },
        )
        .err()
        .unwrap();
        assert_eq!(error.kind(), FailureKind::TenantBinding);
        match error.into_cli_error() {
            CliError::Operational(failure) => assert_eq!(
                failure.message,
                "Selected tenant is missing mgmtTsgId; configure it before using gateway commands"
            ),
            other => panic!("unexpected refusal: {other:?}"),
        }
    }
}
