//! Operation-specific, sanitized views of prepared write requests.
//! The output is descriptive and cannot be used to execute a request.

use super::request::PreparedRequest;
use super::writes::Write;
use crate::error::{CliError, SecretSet};
use crate::json::{self, OValue};
use serde_json::{Map, Value, json};

const WITHHELD: &str = "[withheld]";
// Display policy only, not a claim about the service's accepted vocabulary.
// Unrecognized strings stay private even when the local schema accepts them.
const DISPLAY_TYPE_LITERALS: &[&str] = &[
    "production",
    "non_production",
    "workspace-service",
    "service",
];
const FIELDS: &[&str] = &[
    "name",
    "description",
    "type",
    "status",
    "scopes",
    "organisation_id",
    "workspace_id",
    "expires_at",
    "alert_emails",
    "rate_limits",
    "usage_limits",
    "defaults",
    "rotation_policy",
    "auth_settings",
    "deployment_config",
    "is_default",
    "override_existing",
    "rotate_auth",
    "tags",
];

fn withheld(redacted: &mut bool) -> Value {
    *redacted = true;
    Value::String(WITHHELD.into())
}

fn auth_settings(value: &Value, redacted: &mut bool) -> Value {
    let Some(settings) = value.as_object() else {
        return withheld(redacted);
    };
    let mut result = Map::new();
    for (key, value) in settings {
        let projected = match key.as_str() {
            "allow_all_workspaces" | "is_playground_proxy_allowed" if value.is_boolean() => {
                value.clone()
            }
            "is_dataservice_hosted"
                if value.is_boolean() || matches!(value.as_i64(), Some(0 | 1)) =>
            {
                value.clone()
            }
            "workspaces_allowed" | "remove_workspaces_allowed" => {
                let Some(items) = value.as_array() else {
                    return withheld(redacted);
                };
                Value::Array(
                    items
                        .iter()
                        .map(|item| {
                            if item.as_str().is_some_and(|slug| {
                                slug.starts_with("ws-")
                                    && slug.len() <= 128
                                    && slug.bytes().all(|b| {
                                        b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
                                    })
                            }) {
                                item.clone()
                            } else {
                                withheld(redacted)
                            }
                        })
                        .collect(),
                )
            }
            "allow_all_workspaces"
            | "is_dataservice_hosted"
            | "is_playground_proxy_allowed"
            | "gateway_base_url"
            | "mcp_gateway_base_url"
            | "jwt_subs_allowed"
            | "jwt_sub_workspace_mapping"
            | "remove_subs_allowed" => withheld(redacted),
            _ => {
                *redacted = true;
                continue;
            }
        };
        result.insert(key.clone(), projected);
    }
    Value::Object(result)
}

fn body_view(body: &Value, redacted: &mut bool) -> Value {
    let mut result = Map::new();
    if let Some(body) = body.as_object() {
        for (key, value) in body {
            if !FIELDS.contains(&key.as_str()) {
                *redacted = true;
                continue;
            }
            let projected = match key.as_str() {
                "organisation_id"
                    if value.as_str().is_some_and(|text| {
                        !text.is_empty()
                            && text.len() <= 128
                            && text.bytes().all(|b| b.is_ascii_digit())
                    }) =>
                {
                    value.clone()
                }
                "workspace_id"
                    if value.as_str().is_some_and(|text| {
                        text.len() == 36 && uuid::Uuid::parse_str(text).is_ok()
                    }) =>
                {
                    value.clone()
                }
                "expires_at"
                    if value.is_null()
                        || value.as_str().is_some_and(|text| {
                            text.len() <= 64 && crate::modelsec::datetime_millis(text).is_some()
                        }) =>
                {
                    value.clone()
                }
                "is_default" | "override_existing" | "rotate_auth" if value.is_boolean() => {
                    value.clone()
                }
                "type"
                    if value
                        .as_str()
                        .is_some_and(|v| DISPLAY_TYPE_LITERALS.contains(&v)) =>
                {
                    value.clone()
                }
                "status"
                    if value
                        .as_str()
                        .is_some_and(|v| matches!(v, "active" | "archived")) =>
                {
                    value.clone()
                }
                "scopes" => match value.as_array() {
                    Some(scopes) => Value::Array(
                        scopes
                            .iter()
                            .map(|scope| {
                                if scope.as_str().is_some_and(|v| {
                                    matches!(
                                        v,
                                        "agents.invoke"
                                            | "completions.write"
                                            | "logs.write"
                                            | "mcp.invoke"
                                            | "prompts.render"
                                    )
                                }) {
                                    scope.clone()
                                } else {
                                    withheld(redacted)
                                }
                            })
                            .collect(),
                    ),
                    None => withheld(redacted),
                },
                "auth_settings" => auth_settings(value, redacted),
                _ => withheld(redacted),
            };
            result.insert(key.clone(), projected);
        }
    }
    Value::Object(result)
}

/// Resource-relative SDK paths must see the resource root, before it is nested
/// in the preview envelope. General secret scrubbing also runs on the envelope.
fn sanitized_body(
    wire: &Value,
    policy: &super::withhold::Policy,
    secrets: &SecretSet,
) -> Result<(OValue, bool), CliError> {
    let mut redacted = false;
    let mut body = json::model_value(&body_view(wire, &mut redacted))?;
    let before = body.clone();
    super::withhold::apply(&mut body, policy, secrets);
    redacted |= before != body;
    Ok((body, redacted))
}

pub fn document(
    write: &Write,
    request: &PreparedRequest,
    tsg: &str,
    confirmation: bool,
    secret_output: bool,
    destination_supplied: bool,
    secrets: &SecretSet,
) -> Result<OValue, CliError> {
    // These are also the typed serializers execution uses for the two supported
    // operations. Project after normalization, never before validation.
    let wire = request.typed().credential_wire(write)?;
    let (body, redacted) = sanitized_body(&wire, write.policy(), secrets)?;
    let (method, route) = write.route(Some(tsg));
    let mut view = json::model_value(&json!({
        "schema_version": 1,
        "mode": "dry-run",
        "locally_validated": true,
        "service_acceptance_checked": false,
        "request_sent": false,
        "redacted": redacted,
        "request": {"method": method, "route": route, "plane": write.plane().name(), "body": body},
        "execution_requirements": {
            "confirmation_required": confirmation,
            "secret_output_required": secret_output,
            "secret_output_supplied": destination_supplied,
        },
    }))?;
    crate::secrets::withhold(&mut view, secrets);
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbitrary_keys_and_values_are_not_copied_to_the_projection() {
        let mut redacted = false;
        let projected = body_view(
            &json!({
                "name": "SYNTHETIC-PRIVATE-NAME",
                "type": "SYNTHETIC-PRIVATE-TYPE",
                "defaults": {"SYNTHETIC-PRIVATE-KEY": "value"},
                "SYNTHETIC-PRIVATE-UNKNOWN": "value",
                "scopes": ["completions.write", "SYNTHETIC-PRIVATE-SCOPE"],
                "auth_settings": {
                    "gateway_base_url": "https://user:SYNTHETIC-PRIVATE-PASSWORD@gw.example.test",
                    "allow_all_workspaces": true,
                    "jwt_sub_workspace_mapping": {"SYNTHETIC-PRIVATE-SUBJECT": "value"}
                }
            }),
            &mut redacted,
        );
        assert!(redacted);
        assert!(!projected.to_string().contains("SYNTHETIC-PRIVATE"));
        assert_eq!(
            projected["auth_settings"]["allow_all_workspaces"],
            json!(true)
        );
        assert_eq!(projected["scopes"][0], json!("completions.write"));
    }
    #[test]
    fn resource_paths_apply_before_nesting_and_mark_the_preview_redacted() {
        let policy = super::super::withhold::Policy {
            operations: &[],
            paths: &[&["workspace_id"]],
        };
        let (body, redacted) = sanitized_body(
            &json!({"workspace_id": "11111111-1111-4111-8111-000000000001"}),
            &policy,
            &SecretSet::default(),
        )
        .unwrap();
        assert!(redacted);
        assert_ne!(
            serde_json::to_value(body).unwrap()["workspace_id"],
            json!("11111111-1111-4111-8111-000000000001")
        );
    }

    #[test]
    fn output_guards_withhold_unrecognized_scalar_shapes_without_revalidating() {
        let mut redacted = false;
        let body = body_view(
            &json!({
                "organisation_id": "SYNTHETIC-PRIVATE", "workspace_id": "SYNTHETIC-PRIVATE",
                "expires_at": "SYNTHETIC-PRIVATE", "rotate_auth": "SYNTHETIC-PRIVATE", "scopes": "SYNTHETIC-PRIVATE",
                "auth_settings": {"is_dataservice_hosted": "SYNTHETIC-PRIVATE", "allow_all_workspaces": 1}
            }),
            &mut redacted,
        );
        assert!(redacted);
        assert_eq!(body["scopes"], json!(WITHHELD));
        assert!(!body.to_string().contains("SYNTHETIC-PRIVATE"));
        assert_eq!(
            body["auth_settings"]["allow_all_workspaces"],
            json!(WITHHELD)
        );
    }
}
