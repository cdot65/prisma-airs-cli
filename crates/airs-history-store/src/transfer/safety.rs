//! Logical transfer accepts normalized allowlisted models, never arbitrary SDK extensions.
use super::{
    codec::{Row, Table},
    rows::Spec,
};
use crate::{failure, repository};
use airs_runtime_core::{
    error::{CliError, SecretSet},
    signal::{
        evidence::{Application, Metric, Profile},
        privacy::{self, PayloadSpans},
    },
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileFields {
    profile: Profile,
    association: String,
    profile_key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationFields {
    application: Application,
    association: String,
    application_key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MetricFields {
    metric: Metric,
    association: String,
    population_key: String,
}

fn invalid() -> CliError {
    failure("logical transfer contains unsupported or unsafe normalized metadata")
}
fn model<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T, CliError> {
    serde_json::from_value(v.clone()).map_err(|_| invalid())
}
fn metadata_keys(v: &Value, depth: usize) -> Result<(), CliError> {
    if depth > 32 {
        return Err(invalid());
    }
    match v {
        Value::Object(map) => {
            for (key, value) in map {
                if ![
                    "origin",
                    "original_collection_id",
                    "original_digest",
                    "projected_digest",
                    "legacy_tenant",
                    "window",
                    "selected_app_ids",
                    "evidence_schema_version",
                    "rules_version",
                    "redaction_version",
                    "sdk_version",
                    "scope",
                    "provider_completeness",
                    "absolute_bounds",
                    "coverage",
                    "actual_query",
                    "rolling_window_anchor",
                    "time_interval",
                    "time_unit",
                    "absolute_selector",
                    "semantics",
                    "interval",
                    "unit",
                    "start_ms",
                    "end_ms",
                    "example_limit",
                    "example_selection",
                    "example_policy",
                    "progress",
                    "run_state",
                    "management_admission_notes",
                    "backlog_reserved_candidates",
                    "event_budget_policy",
                    "retained_full_events_this_run",
                    "partial_receipts_this_run",
                    "unconfirmed_management_reads",
                    "unconfirmed_read_semantics",
                    "revisited_work_attempts",
                    "incomplete_revisited_results",
                    "retained_work",
                    "source",
                    "state",
                    "work_descriptors",
                    "scanner_reference",
                    "scanner_references",
                    "scanner_endpoint",
                    "name",
                    "revision",
                    "applicationId",
                    "endpoint",
                    "historical_access",
                    "reservation",
                    "ineligible_configuration",
                    "eligibility_checked_without_send",
                    "observed_configuration",
                    "no_remote_request",
                    "identity_withheld",
                    "first_query_since_ms",
                    "first_query_until_ms",
                    "first_run",
                    "last_query_since_ms",
                    "last_query_until_ms",
                    "query_since_ms",
                    "query_until_ms",
                    "ended_ms",
                    "requested_run",
                    "scope_kind",
                    "scope_identity_supported",
                    "coverage_semantics",
                    "required",
                    "records",
                    "pages",
                    "population",
                    "status",
                    "started_ms",
                    "notes",
                    "requested_since_ms",
                    "requested_until_ms",
                    "actual_rolling_query",
                    "operation",
                    "query",
                    "actual_send",
                    "application_id",
                    "application_name",
                    "session_id",
                    "app_id",
                    "app_name",
                    "identity",
                    "scan_id",
                    "sub_request",
                    "report_id",
                    "projection",
                    "positive_metadata",
                    "positive_hint_event_sources",
                    "report_observed",
                    "report_event_sources",
                    "report_ids",
                    "origins",
                    "comparison_subsumed",
                    "subsumed_hint_event_sources",
                    "source_attribution",
                    "logical_imported",
                    "original_kind",
                    "transfer_digest",
                    "pruned",
                    "before_ms",
                    "examples_only",
                    "affected_records",
                    // Service and native sync run provenance (M5 review 1 finding 2):
                    // typed by `service_provenance` below.
                    "credential_revision",
                    "fact_counts",
                    "new",
                    "duplicate",
                    "first_accepted",
                    "incomplete_required_sources",
                    "selected_report_ids",
                    "cached_examples",
                    "token_attempts",
                    "run_id",
                ]
                .contains(&key.as_str())
                {
                    return Err(invalid());
                }
                metadata_keys(value, depth + 1)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                metadata_keys(value, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
/// The strict shapes of the service-written run provenance (M5 review 1 finding 2):
/// the credential revision a service run is bound to (a UUID v4), its write-time fact
/// counts (exactly `{new, duplicate}`, non-negative integers), a receipt run's
/// first-acceptance record (unique receipt digests) and the progress counters.
fn service_provenance(value: &Value) -> Result<(), CliError> {
    let Some(map) = value.as_object() else {
        return Ok(());
    };
    let hex64 = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if let Some(r) = map.get("credential_revision")
        && !r.as_str().is_some_and(airs_runtime_core::dto::is_uuid_v4)
    {
        return Err(invalid());
    }
    if let Some(c) = map.get("fact_counts") {
        let ok = c.as_object().is_some_and(|c| {
            c.len() == 2
                && ["new", "duplicate"]
                    .iter()
                    .all(|k| c.get(*k).is_some_and(Value::is_u64))
        });
        if !ok {
            return Err(invalid());
        }
    }
    if let Some(f) = map.get("first_accepted") {
        let ok = f.as_array().is_some_and(|ids| {
            let mut seen = std::collections::BTreeSet::new();
            ids.iter()
                .all(|id| id.as_str().is_some_and(|s| hex64(s) && seen.insert(s)))
        });
        if !ok {
            return Err(invalid());
        }
    }
    if let Some(p) = map.get("progress").and_then(Value::as_object) {
        for key in ["selected_report_ids", "cached_examples", "token_attempts"] {
            if p.get(key).is_some_and(|v| !v.is_u64()) {
                return Err(invalid());
            }
        }
    }
    Ok(())
}
fn public_endpoint(value: &Value) -> Result<(), CliError> {
    let endpoint = value.as_str().ok_or_else(invalid)?;
    let admitted = airs_runtime_core::clients::admit("airsApiEndpoint", Some(endpoint), endpoint)?;
    if admitted.url().as_str().trim_end_matches('/') != endpoint {
        return Err(invalid());
    }
    Ok(())
}
/// Preserve admitted public endpoint namespaces (including loopback IP literals),
/// then apply the same current content policy used for ingestion to other data.
fn project(value: &Value, secrets: &SecretSet) -> Result<Value, CliError> {
    fn hide(
        value: &mut Value,
        saved: &mut Vec<(String, Value)>,
        prefix: String,
    ) -> Result<(), CliError> {
        match value {
            Value::Object(map) => {
                let names = map.keys().cloned().collect::<Vec<_>>();
                for name in names {
                    let path = format!("{prefix}/{}", name.replace('~', "~0").replace('/', "~1"));
                    if ["scanner_endpoint", "endpoint"].contains(&name.as_str())
                        && map[&name] != Value::Null
                    {
                        public_endpoint(&map[&name])?;
                        saved.push((path, map[&name].clone()));
                        map.insert(name, Value::Null);
                    } else {
                        hide(map.get_mut(&name).ok_or_else(invalid)?, saved, path)?;
                    }
                }
            }
            Value::Array(array) => {
                for (index, v) in array.iter_mut().enumerate() {
                    hide(v, saved, format!("{prefix}/{index}"))?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn secret_free(value: &Value, secrets: &SecretSet) -> bool {
        match value {
            Value::String(v) => !secrets.carried_by(v),
            Value::Array(a) => a.iter().all(|v| secret_free(v, secrets)),
            Value::Object(m) => m
                .iter()
                .all(|(k, v)| !secrets.carried_by(k) && secret_free(v, secrets)),
            _ => true,
        }
    }
    if !secret_free(value, secrets) {
        return Err(invalid());
    }
    let mut projected = value.clone();
    let mut endpoints = vec![];
    hide(&mut projected, &mut endpoints, String::new())?;
    privacy::document(&mut projected, secrets, &PayloadSpans::default());
    for (path, value) in endpoints {
        *projected.pointer_mut(&path).ok_or_else(invalid)? = value;
    }
    Ok(projected)
}

pub fn validate(spec: &Spec, row: &Row, secrets: &SecretSet) -> Result<(), CliError> {
    spec.values(row)?;
    for (column, value) in spec.columns.iter().zip(&row.values) {
        let Some(text) = value.as_str() else { continue };
        if secrets.carried_by(text) {
            return Err(invalid());
        }
        if column.name.ends_with("_json") {
            let strict: airs_runtime_core::signal::evidence::Strict =
                serde_json::from_str(text).map_err(|_| invalid())?;
            let value = strict.0;
            match (row.table, column.name.as_str()) {
                (Table::EventFacts | Table::ReceiptFacts, "fields_json") => {
                    repository::validate_transfer_fact(&value, row.table == Table::EventFacts)?
                }
                (Table::ProfileObservations, "fields_json") => {
                    let fields: ProfileFields = model(&value)?;
                    if !["exact_name", "unlinked_privacy"].contains(&fields.association.as_str())
                        || fields.profile_key.is_empty()
                        || fields.profile.name.is_empty()
                    {
                        return Err(invalid());
                    }
                }
                (Table::ApplicationObservations, "fields_json") => {
                    let fields: ApplicationFields = model(&value)?;
                    if !["exact_identity", "unlinked_privacy"]
                        .contains(&fields.association.as_str())
                        || fields.application_key.is_empty()
                    {
                        return Err(invalid());
                    }
                    let _ = fields.application;
                }
                (Table::AggregateObservations, "fields_json") => {
                    let fields: MetricFields = model(&value)?;
                    if !["exact_label", "unlinked_privacy"].contains(&fields.association.as_str())
                        || fields.population_key.is_empty()
                        || fields
                            .metric
                            .value
                            .is_some_and(|v| !v.is_finite() || v < 0.0)
                    {
                        return Err(invalid());
                    }
                }
                (Table::Examples, "excerpts_json") => {
                    let _: Vec<String> = model(&value)?;
                }
                (Table::CollectionRuns, "provenance_json") => {
                    metadata_keys(&value, 0)?;
                    service_provenance(&value)?;
                }
                _ => metadata_keys(&value, 0)?,
            }
            if project(&value, secrets)? != value {
                return Err(failure(
                    "logical transfer requires additional privacy projection; refusing rather than corrupting immutable identities",
                ));
            }
        } else {
            let value = serde_json::json!({column.name.clone():text});
            if project(&value, secrets)? != value {
                return Err(invalid());
            }
        }
    }
    Ok(())
}
