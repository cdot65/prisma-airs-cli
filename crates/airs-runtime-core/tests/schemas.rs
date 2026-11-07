//! M0.1 frozen tool schemas: structural invariants plus strict validation of
//! one example request/result per tool and generated negative mutations.

use serde_json::{Map, Value, json};

const TOOLS: &str = include_str!("../schemas/tools.v1.json");
const EXAMPLES: &str = include_str!("../schemas/examples.v1.json");

/// The frozen catalogue (contract §4.3).
const CATALOGUE: &[(&str, &str)] = &[
    ("service_info", "reader"),
    ("history_status", "reader"),
    ("history_coverage", "reader"),
    ("history_query", "reader"),
    ("reports_list", "reader"),
    ("reports_get", "reader"),
    ("artifacts_read", "reader"),
    ("artifacts_download", "reader"),
    ("jobs_list", "reader"),
    ("jobs_get", "reader"),
    ("schedules_list", "reader"),
    ("collection_start", "operator"),
    ("reports_create", "operator"),
    ("enrichment_start", "operator"),
    ("jobs_cancel", "operator"),
    ("schedules_set", "operator"),
    ("schedules_disable", "operator"),
    ("receipts_ingest", "operator"),
    ("assessments_submit", "operator"),
    ("lifecycle_preview", "administrator"),
    ("lifecycle_execute", "administrator"),
    ("admin_confirm", "administrator"),
    ("transfer_upload_begin", "administrator"),
    ("transfer_upload_chunk", "administrator"),
    ("transfer_upload_finalize", "administrator"),
    ("transfer_upload_abort", "administrator"),
    ("export_read", "administrator"),
];

fn doc() -> Value {
    serde_json::from_str(TOOLS).unwrap()
}

/// A strict validator for the JSON Schema subset the catalogue uses.
fn valid(root: &Value, schema: &Value, v: &Value) -> bool {
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        let name = r.strip_prefix("#/$defs/").expect("local ref");
        return valid(root, &root["$defs"][name], v);
    }
    if let Some(any) = schema.get("anyOf").and_then(Value::as_array) {
        return any.iter().any(|s| valid(root, s, v));
    }
    if let Some(c) = schema.get("const") {
        return c == v;
    }
    if let Some(e) = schema.get("enum").and_then(Value::as_array) {
        return e.contains(v);
    }
    let types: Vec<&str> = match schema.get("type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(ts)) => ts.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    };
    let type_ok = types.is_empty()
        || types.iter().any(|t| match *t {
            "string" => v.is_string(),
            "integer" => v.is_i64() || v.is_u64(),
            "boolean" => v.is_boolean(),
            "object" => v.is_object(),
            "array" => v.is_array(),
            "null" => v.is_null(),
            _ => false,
        });
    if !type_ok {
        return false;
    }
    match v {
        Value::String(s) => {
            let n = s.chars().count() as u64;
            if schema
                .get("maxLength")
                .and_then(Value::as_u64)
                .is_some_and(|m| n > m)
                || schema
                    .get("minLength")
                    .and_then(Value::as_u64)
                    .is_some_and(|m| n < m)
            {
                return false;
            }
            if let Some(p) = schema.get("pattern").and_then(Value::as_str) {
                return regex::Regex::new(p).unwrap().is_match(s);
            }
            true
        }
        Value::Number(n) => {
            let x = n.as_i64().map(|i| i as f64).or(n.as_f64()).unwrap();
            !(schema
                .get("minimum")
                .and_then(Value::as_f64)
                .is_some_and(|m| x < m)
                || schema
                    .get("maximum")
                    .and_then(Value::as_f64)
                    .is_some_and(|m| x > m))
        }
        Value::Array(items) => {
            let n = items.len() as u64;
            if schema
                .get("maxItems")
                .and_then(Value::as_u64)
                .is_some_and(|m| n > m)
                || schema
                    .get("minItems")
                    .and_then(Value::as_u64)
                    .is_some_and(|m| n < m)
            {
                return false;
            }
            schema
                .get("items")
                .is_none_or(|s| items.iter().all(|i| valid(root, s, i)))
        }
        Value::Object(map) => {
            let props = schema.get("properties").and_then(Value::as_object);
            if let Some(req) = schema.get("required").and_then(Value::as_array)
                && req
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|k| !map.contains_key(k))
            {
                return false;
            }
            for (k, val) in map {
                // The schema governing this member: its property, else the
                // additionalProperties schema; `false` forbids it outright.
                let governing = match props.and_then(|p| p.get(k)) {
                    Some(s) => Some(s),
                    None => match schema.get("additionalProperties") {
                        Some(Value::Bool(false)) => return false,
                        Some(s @ Value::Object(_)) => Some(s),
                        _ => None,
                    },
                };
                if governing.is_some_and(|s| !valid(root, s, val)) {
                    return false;
                }
            }
            true
        }
        _ => true,
    }
}

fn walk_objects(v: &Value, path: &str, out: &mut Vec<String>) {
    if let Value::Object(m) = v {
        // Every object schema, with or without `properties`, must be closed.
        if m.get("type") == Some(&json!("object")) {
            match m.get("additionalProperties") {
                Some(Value::Bool(false)) | Some(Value::Object(_)) => {}
                _ => out.push(format!("{path}: open object")),
            }
            let empty = Map::new();
            let props = m
                .get("properties")
                .and_then(Value::as_object)
                .unwrap_or(&empty);
            for r in m
                .get("required")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if !props.contains_key(r.as_str().unwrap()) {
                    out.push(format!("{path}: required {r} not a property"));
                }
            }
        }
        if let Some(r) = m.get("$ref").and_then(Value::as_str) {
            let name = r.trim_start_matches("#/$defs/");
            if doc()["$defs"].get(name).is_none() {
                out.push(format!("{path}: dangling {r}"));
            }
        }
        for (k, child) in m {
            walk_objects(child, &format!("{path}/{k}"), out);
        }
    } else if let Value::Array(a) = v {
        for (i, child) in a.iter().enumerate() {
            walk_objects(child, &format!("{path}/{i}"), out);
        }
    }
}

#[test]
fn catalogue_matches_the_contract_and_admin_mutations_need_proof() {
    let d = doc();
    let tools = d["tools"].as_object().unwrap();
    assert_eq!(tools.len(), CATALOGUE.len());
    for (name, tier) in CATALOGUE {
        let t = &tools[*name];
        assert_eq!(t["tier"], *tier, "{name}");
        let authority = t["authority"].as_str().unwrap();
        assert!(
            d["authority_rules"].get(authority).is_some(),
            "{name}: unknown authority {authority}"
        );
        if *tier == "administrator" && t["mutating"] == true {
            assert!(
                matches!(
                    authority,
                    "direct_proof" | "direct_proof_or_approval" | "upload_session"
                ),
                "{name}: admin mutation with ordinary grant authority"
            );
        } else {
            assert_eq!(authority, "grant", "{name}");
        }
        let req = t["params"]["required"].as_array().unwrap();
        assert_eq!(
            t["idempotency_key"] == true,
            req.contains(&json!("idempotency_key")),
            "{name}"
        );
        let limit = t["result_limit"].as_str().unwrap();
        // Only byte-transfer tools may use the 256 KiB CLI limit.
        assert_eq!(
            limit == "cli",
            matches!(*name, "artifacts_download" | "export_read"),
            "{name}"
        );
    }
}

#[test]
fn every_object_schema_is_closed_and_every_ref_resolves() {
    let mut problems = Vec::new();
    walk_objects(&doc(), "", &mut problems);
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn every_example_request_and_result_validates() {
    let d = doc();
    let ex: Value = serde_json::from_str(EXAMPLES).unwrap();
    let ex = ex["examples"].as_object().unwrap();
    assert_eq!(ex.len(), CATALOGUE.len());
    for (name, _) in CATALOGUE {
        let t = &d["tools"][*name];
        assert!(
            valid(&d, &t["params"], &ex[*name]["request"]),
            "{name} request"
        );
        assert!(
            valid(&d, &t["result"], &ex[*name]["result"]),
            "{name} result"
        );
    }
}

#[test]
fn unknown_fields_versions_and_missing_keys_are_refused_for_every_tool() {
    let d = doc();
    let ex: Value = serde_json::from_str(EXAMPLES).unwrap();
    for (name, _) in CATALOGUE {
        let schema = &d["tools"][*name]["params"];
        let good = ex["examples"][*name]["request"].clone();
        let mutate = |f: &dyn Fn(&mut Map<String, Value>)| {
            let mut v = good.clone();
            f(v.as_object_mut().unwrap());
            v
        };
        assert!(
            !valid(
                &d,
                schema,
                &mutate(&|m| {
                    m.insert("sql".into(), json!("drop"));
                })
            ),
            "{name}: top-level extra"
        );
        assert!(
            !valid(
                &d,
                schema,
                &mutate(&|m| {
                    m["params"]
                        .as_object_mut()
                        .unwrap()
                        .insert("path".into(), json!("/etc"));
                })
            ),
            "{name}: param extra"
        );
        assert!(
            !valid(
                &d,
                schema,
                &mutate(&|m| {
                    m.insert("schema_version".into(), json!(2));
                })
            ),
            "{name}: version"
        );
        assert!(
            !valid(
                &d,
                schema,
                &mutate(&|m| {
                    m.insert("tenant".into(), json!("Bad Tenant!"));
                })
            ),
            "{name}: tenant"
        );
        if d["tools"][*name]["idempotency_key"] == true {
            assert!(
                !valid(
                    &d,
                    schema,
                    &mutate(&|m| {
                        m.remove("idempotency_key");
                    })
                ),
                "{name}: missing key"
            );
        }
    }
    // Ranges and receipt batches are bounded.
    let ing = &d["tools"]["receipts_ingest"]["params"];
    let mut big = ex["examples"]["receipts_ingest"]["request"].clone();
    let r = big["params"]["records"][0].clone();
    big["params"]["records"] = Value::Array(vec![r; 201]);
    assert!(!valid(&d, ing, &big), "201 receipts must be refused");
    let dl = &d["tools"]["artifacts_download"]["params"];
    let mut over = ex["examples"]["artifacts_download"]["request"].clone();
    over["params"]["length"] = json!(65537);
    assert!(!valid(&d, dl, &over), "chunk above 64 KiB must be refused");
}

/// The compiled decoder must accept exactly what the frozen schema accepts for
/// the envelope: every example round-trips, and envelope mutations are refused.
#[test]
fn compiled_envelope_decoder_agrees_with_the_frozen_schema() {
    use airs_runtime_core::dto::{ErrorCode, Request, check_epoch, parse_tool_request};
    let d = doc();
    let ex: Value = serde_json::from_str(EXAMPLES).unwrap();
    for (name, _) in CATALOGUE {
        let idem = d["tools"][*name]["idempotency_key"] == true;
        let good = ex["examples"][*name]["request"].clone();
        let raw = good.to_string();
        let req: Request<Value> =
            parse_tool_request(&raw, idem).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(
            serde_json::to_value(&req).unwrap(),
            good,
            "{name}: lossless round trip"
        );
        let mutate = |f: &dyn Fn(&mut Map<String, Value>)| {
            let mut v = good.clone();
            f(v.as_object_mut().unwrap());
            v.to_string()
        };
        let refused = |raw: String| parse_tool_request::<Value>(&raw, idem).err();
        assert_eq!(
            refused(mutate(&|m| {
                m.insert("sql".into(), json!(1));
            })),
            Some(ErrorCode::InvalidRequest),
            "{name}"
        );
        assert_eq!(
            refused(mutate(&|m| {
                m.insert("schema_version".into(), json!(2));
            })),
            Some(ErrorCode::UnsupportedVersion),
            "{name}"
        );
        assert_eq!(
            refused(mutate(&|m| {
                m.insert("tenant".into(), json!("Bad Tenant!"));
            })),
            Some(ErrorCode::InvalidRequest),
            "{name}"
        );
        if idem {
            assert_eq!(
                refused(mutate(&|m| {
                    m.remove("expected_epoch");
                })),
                Some(ErrorCode::InvalidRequest),
                "{name}: missing epoch"
            );
            assert_eq!(
                refused(mutate(&|m| {
                    m.remove("idempotency_key");
                })),
                Some(ErrorCode::InvalidRequest),
                "{name}: missing key"
            );
            assert_eq!(
                refused(mutate(&|m| {
                    m.insert("expected_epoch".into(), json!("ZZZZZZZZ"));
                })),
                Some(ErrorCode::InvalidRequest),
                "{name}: bad epoch"
            );
            assert_eq!(check_epoch(&req, "0000000a"), Ok(()));
            assert_eq!(
                check_epoch(&req, "0000000b"),
                Err(ErrorCode::ControlRestored),
                "{name}: other epoch"
            );
        } else {
            for k in ["idempotency_key", "expected_epoch"] {
                let null = mutate(&|m| {
                    m.insert(k.into(), Value::Null);
                });
                assert_eq!(
                    refused(null.clone()),
                    Some(ErrorCode::InvalidRequest),
                    "{name}: explicit null {k}"
                );
                let parsed: Value = serde_json::from_str(&null).unwrap();
                assert!(
                    !valid(&d, &d["tools"][*name]["params"], &parsed),
                    "{name}: schema null {k}"
                );
            }
            let with_key = mutate(&|m| {
                m.insert("idempotency_key".into(), json!("0123456789abcdef0123"));
                m.insert("expected_epoch".into(), json!("0000000a"));
            });
            assert_eq!(
                refused(with_key),
                Some(ErrorCode::InvalidRequest),
                "{name}: key on non-idempotent tool"
            );
        }
    }
}

#[test]
fn job_results_are_closed_variants_with_success_partial_and_failure_examples() {
    let d = doc();
    let ex: Value = serde_json::from_str(EXAMPLES).unwrap();
    let job = json!({"$ref": "#/$defs/job"});
    for (label, j) in ex["job_examples"].as_object().unwrap() {
        assert!(valid(&d, &job, j), "{label}");
        let mut forged = j.clone();
        forged["result"]["arbitrary"] = json!({"nested": true});
        assert!(
            !valid(&d, &job, &forged),
            "{label}: arbitrary result field accepted"
        );
    }
    // A collection result can never claim Reports API sends (zero by contract).
    let mut sends = ex["job_examples"]["succeeded_collection"].clone();
    sends["result"]["sends"]["reports"] = json!(1);
    assert!(!valid(&d, &job, &sends));
    for (tool, list) in ex["extra_requests"].as_object().unwrap() {
        for r in list.as_array().unwrap() {
            assert!(
                valid(&d, &d["tools"][tool.as_str()]["params"], r),
                "{tool} extra request"
            );
        }
    }
}

/// Responses too: every error envelope the schema accepts decodes through the
/// compiled type and back unchanged; drift in either direction is refused.
#[test]
fn compiled_error_envelope_agrees_with_the_frozen_schema() {
    use airs_runtime_core::dto::{ErrorCode, parse_error};
    let d = doc();
    let err = json!({"$ref": "#/$defs/error"});
    let codes: Vec<String> = d["$defs"]["error"]["properties"]["code"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    let statuses = d["$defs"]["error"]["properties"]["credential_status"]["enum"]
        .as_array()
        .unwrap()
        .clone();
    let mut samples = Vec::new();
    for (i, code) in codes.iter().enumerate() {
        let mut e = json!({"schema_version": 1, "code": code, "reason": "fixed reason"});
        if i % 2 == 0 {
            e["retry_after_seconds"] = json!(60);
        }
        if i % 3 == 0 {
            e["job_id"] = json!("j1_0000000a_00000000-0000-4000-8000-000000000001");
        }
        e["credential_status"] = statuses[i % statuses.len()].clone();
        samples.push(e);
    }
    let ex: Value = serde_json::from_str(EXAMPLES).unwrap();
    samples.push(ex["job_examples"]["failed_collection"]["result"]["error"].clone());
    for e in &samples {
        assert!(valid(&d, &err, e), "schema rejects {e}");
        let decoded =
            parse_error(&e.to_string()).unwrap_or_else(|c| panic!("decoder rejects {e}: {c:?}"));
        assert_eq!(serde_json::to_value(&decoded).unwrap(), *e, "lossless");
    }
    // The compiled code set is exactly the schema's.
    let all = [
        ErrorCode::InvalidRequest,
        ErrorCode::UnsupportedVersion,
        ErrorCode::Unauthenticated,
        ErrorCode::Forbidden,
        ErrorCode::NotFound,
        ErrorCode::Conflict,
        ErrorCode::IdempotencyConflict,
        ErrorCode::ControlRestored,
        ErrorCode::Overloaded,
        ErrorCode::QuotaExhausted,
        ErrorCode::StorageUnavailable,
        ErrorCode::MaintenanceRequired,
        ErrorCode::LowSpace,
        ErrorCode::CredentialBlocked,
        ErrorCode::UpstreamUnavailable,
        ErrorCode::Canceled,
        ErrorCode::ResultTooLarge,
        ErrorCode::Internal,
    ];
    let mut compiled: Vec<String> = all
        .iter()
        .map(|c| {
            serde_json::to_value(c)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    let mut frozen = codes.clone();
    compiled.sort();
    frozen.sort();
    assert_eq!(compiled, frozen);
    let mut extra = samples[0].clone();
    extra["detail"] = json!("raw sdk text");
    assert!(!valid(&d, &err, &extra) && parse_error(&extra.to_string()).is_err());
    for (field, value) in [
        ("job_id", json!("not-a-job-id")),
        (
            "job_id",
            json!("j1_0000000A_00000000-0000-4000-8000-000000000001"),
        ),
        ("job_id", Value::Null),
        ("job_id", json!(format!("j1_0000000a_{}", "-".repeat(36)))),
        (
            "job_id",
            json!("j1_0000000a_00000000-0000-1000-8000-000000000001"),
        ),
        (
            "job_id",
            json!("j1_0000000a_00000000-0000-4000-c000-000000000001"),
        ),
        (
            "job_id",
            json!("j1_0000000a_000000000000-4000-8000-0000-00000001"),
        ),
        ("credential_status", Value::Null),
        ("retry_after_seconds", Value::Null),
        ("retry_after_seconds", json!(-1)),
    ] {
        let mut m = samples[0].clone();
        m[field] = value.clone();
        assert!(!valid(&d, &err, &m), "schema accepts {field}={value}");
        assert!(
            parse_error(&m.to_string()).is_err(),
            "decoder accepts {field}={value}"
        );
    }
    let mut bad = samples[0].clone();
    bad["credential_status"] = json!("expired");
    assert!(!valid(&d, &err, &bad) && parse_error(&bad.to_string()).is_err());
}

#[test]
fn every_server_issued_identifier_in_results_is_constrained() {
    let d = doc();
    let ex: Value = serde_json::from_str(EXAMPLES).unwrap();
    // (tool, JSON pointer to an identifier inside its example result)
    let cases = [
        ("schedules_set", "/schedule_id"),
        ("schedules_disable", "/schedule_id"),
        ("schedules_list", "/schedules/0/schedule_id"),
        ("jobs_get", "/job/job_id"),
        ("reports_get", "/manifest/version_id"),
        ("transfer_upload_begin", "/upload_id"),
        ("lifecycle_preview", "/preview_id"),
        ("admin_confirm", "/approval_id"),
        ("assessments_submit", "/assessment_id"),
    ];
    for (tool, ptr) in cases {
        let schema = &d["tools"][tool]["result"];
        let good = ex["examples"][tool]["result"].clone();
        assert!(valid(&d, schema, &good), "{tool}");
        for bad in [
            "not-a-schedule-id",
            "s1_------------------------------------",
            "x1_00000000-0000-4000-8000-000000000001",
        ] {
            let mut m = good.clone();
            *m.pointer_mut(ptr).unwrap_or_else(|| panic!("{tool}{ptr}")) = json!(bad);
            assert!(!valid(&d, schema, &m), "{tool}{ptr} accepted {bad}");
        }
    }
}

/// Every job-submitting tool's example result names its own operation.
#[test]
fn submission_examples_name_their_own_operation() {
    let d = doc();
    let ex: Value = serde_json::from_str(EXAMPLES).unwrap();
    let mut submitting = 0;
    for (name, _) in CATALOGUE {
        let result = &d["tools"][*name]["result"];
        if result.get("$ref") == Some(&json!("#/$defs/accepted")) {
            submitting += 1;
            assert_eq!(
                ex["examples"][*name]["result"]["job"]["operation"],
                json!(*name),
                "{name}"
            );
        }
    }
    assert_eq!(
        submitting, 4,
        "collection_start, reports_create, enrichment_start, lifecycle_execute"
    );
}
