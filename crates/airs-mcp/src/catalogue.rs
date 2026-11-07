//! The frozen tool catalogue (contract §4.3), read from core's `schemas/tools.v1.json`.
//! Tiers, authority classes, mutation flags, result limits and parameter schemas all
//! come from that one file; the server never restates them.

use std::sync::OnceLock;

use serde_json::{Map, Value};

use crate::config::Permission;

/// One catalogue entry.
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub tier: Permission,
    /// `grant`, `direct_proof`, `direct_proof_or_approval` or `upload_session` (§3.5).
    pub authority: String,
    pub mutating: bool,
    pub idempotency_key: bool,
    /// Result bound in bytes for the complete serialized `CallToolResult` (§5).
    pub result_limit: usize,
    /// The frozen parameter schema with every `$ref` inlined (a self-contained MCP
    /// `inputSchema`).
    pub input_schema: Map<String, Value>,
    /// The frozen result schema, `$ref`s inlined (tests validate results against it).
    pub result_schema: Value,
}

/// The tools this service version implements. Every other catalogue tool is refused by
/// name (`invalid_request`), never listed, and still checked against grants first.
pub const IMPLEMENTED: &[&str] = &[
    "service_info",
    "history_status",
    "history_coverage",
    "jobs_list",
    "jobs_get",
    "jobs_cancel",
    "collection_start",
    "receipts_ingest",
    "schedules_list",
    "schedules_set",
    "schedules_disable",
    "lifecycle_preview",
    "lifecycle_execute",
    "admin_confirm",
    "transfer_upload_begin",
    "transfer_upload_chunk",
    "transfer_upload_finalize",
    "transfer_upload_abort",
    "export_read",
    "reports_create",
    "reports_list",
    "reports_get",
    "artifacts_read",
    "artifacts_download",
    "assessments_submit",
    "enrichment_start",
    "history_query",
];

fn tier(name: &str) -> Option<Permission> {
    match name {
        "reader" => Some(Permission::Reader),
        "operator" => Some(Permission::Operator),
        "administrator" => Some(Permission::Administrator),
        _ => None,
    }
}

fn inline(value: &Value, defs: &Map<String, Value>, depth: usize) -> Value {
    assert!(depth < 32, "frozen schema $ref nesting is bounded");
    match value {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref") {
                let name = r.strip_prefix("#/$defs/").expect("local $ref");
                return inline(&defs[name], defs, depth + 1);
            }
            Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), inline(v, defs, depth + 1)))
                    .collect(),
            )
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|v| inline(v, defs, depth + 1)).collect())
        }
        other => other.clone(),
    }
}

fn load() -> Vec<Tool> {
    let doc: Value =
        serde_json::from_str(airs_runtime_core::dto::TOOLS_V1).expect("frozen catalogue parses");
    let defs = doc["$defs"].as_object().expect("$defs").clone();
    let limits = doc["result_limits"].as_object().expect("result_limits");
    let mut tools: Vec<Tool> = doc["tools"]
        .as_object()
        .expect("tools")
        .iter()
        .map(|(name, t)| {
            let limit = t["result_limit"].as_str().expect("result_limit");
            Tool {
                name: name.clone(),
                tier: tier(t["tier"].as_str().expect("tier")).expect("known tier"),
                authority: t["authority"].as_str().expect("authority").to_owned(),
                mutating: t["mutating"].as_bool().expect("mutating"),
                idempotency_key: t["idempotency_key"].as_bool().expect("idempotency_key"),
                result_limit: limits[limit].as_u64().expect("limit bytes") as usize,
                input_schema: inline(&t["params"], &defs, 0)
                    .as_object()
                    .expect("object params")
                    .clone(),
                result_schema: inline(&t["result"], &defs, 0),
            }
        })
        .collect();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    tools
}

/// Every catalogue tool, sorted by name.
pub fn all() -> &'static [Tool] {
    static TOOLS: OnceLock<Vec<Tool>> = OnceLock::new();
    TOOLS.get_or_init(load)
}

pub fn get(name: &str) -> Option<&'static Tool> {
    all().iter().find(|t| t.name == name)
}

/// The tier a catalogue tool requires, or `None` for an unknown name.
pub fn required_tier(name: &str) -> Option<Permission> {
    get(name).map(|t| t.tier)
}

pub fn implemented(name: &str) -> bool {
    IMPLEMENTED.contains(&name)
}

/// A tool an Administrator grant may list (contract §11): read-only tools and tools
/// whose authority is proof-bound (`direct_proof`, `direct_proof_or_approval`,
/// `upload_session`); never a mutating `grant` tool (§3.5).
pub fn administrator_may_list(tool: &Tool) -> bool {
    !tool.mutating || tool.authority != "grant"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_is_the_frozen_contract() {
        let names: Vec<&str> = all().iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names.len(), 27);
        for name in IMPLEMENTED {
            let tool = get(name).expect("implemented tools are catalogued");
            let admin = [
                "lifecycle_preview",
                "lifecycle_execute",
                "admin_confirm",
                "transfer_upload_begin",
                "transfer_upload_chunk",
                "transfer_upload_finalize",
                "transfer_upload_abort",
                "export_read",
            ];
            if admin.contains(name) {
                // M5: the administrative tools keep their frozen authority classes.
                assert_eq!(tool.tier, Permission::Administrator, "{name}");
                continue;
            }
            assert_eq!(
                tool.authority, "grant",
                "{name}: only grant authority outside §3"
            );
            let reader = [
                "service_info",
                "history_status",
                "history_coverage",
                "jobs_list",
                "jobs_get",
                "schedules_list",
                "reports_list",
                "reports_get",
                "artifacts_read",
                "artifacts_download",
                "history_query",
            ];
            let expected = if reader.contains(name) {
                Permission::Reader
            } else {
                Permission::Operator
            };
            assert_eq!(tool.tier, expected, "{name}");
            let keyed = [
                "collection_start",
                "receipts_ingest",
                "schedules_set",
                "schedules_disable",
                "reports_create",
                "assessments_submit",
                "enrichment_start",
            ];
            assert_eq!(tool.idempotency_key, keyed.contains(name), "{name}");
        }
        assert_eq!(
            required_tier("collection_start"),
            Some(Permission::Operator)
        );
        assert_eq!(
            required_tier("lifecycle_execute"),
            Some(Permission::Administrator)
        );
        assert_eq!(required_tier("echo"), None);
        let text =
            serde_json::to_string(&all().iter().map(|t| &t.input_schema).collect::<Vec<_>>())
                .unwrap();
        assert!(!text.contains("$ref"), "input schemas are self-contained");
        assert!(administrator_may_list(get("lifecycle_execute").unwrap()));
        assert!(administrator_may_list(get("history_status").unwrap()));
        assert!(!administrator_may_list(get("collection_start").unwrap()));
    }
}
