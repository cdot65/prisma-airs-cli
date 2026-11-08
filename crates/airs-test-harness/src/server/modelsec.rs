//! The model-security fixture routes (DESIGN §10 M6c): the management plane under
//! `/aims/mgmt/v1/` (security groups with their rule instances, security rules) and the
//! data plane under `/aims/data/v1/` (scans with their evaluations, violations, files and
//! labels, models and their versions), bearer-checked like management, over a per-server
//! dataset ([`ModelSecData`]).
//!
//! Lists are skip/limit pages answering `{pagination: {total_items}, <rows>}` (scans add
//! `count_capped` when [`ModelSecData::count_capped`] is set). Knobs: a reported total
//! other than the count, no total, a service that ignores `skip` (repeated rows), a page
//! size cap, the capped count. `/v1/scans/label-keys…` is classified before
//! `/v1/scans/<uuid>`. The step-2 write routes (`server/modelsec_writes.rs`) change the
//! dataset, so a later read sees the change; any other method or path under `/aims` is an
//! unknown route.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::plan::Planned;
use super::record::{AuthIdentity, RecordedRequest};
use super::redteam::percent_decode;
use crate::fixtures::modelsec as fx;

/// The management-plane prefix (`modelSecMgmtEndpoint = <base>/aims/mgmt`).
pub const MGMT_PREFIX: &str = "/aims/mgmt/v1";
/// The data-plane prefix (`modelSecDataEndpoint = <base>/aims/data`).
pub const DATA_PREFIX: &str = "/aims/data/v1";

/// A model-security fixture route (`Route::ModelSecurity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ms {
    /// `GET <mgmt>/security-groups`.
    GroupList,
    /// `GET <mgmt>/security-groups/{uuid}`.
    GroupGet,
    /// `GET <mgmt>/security-groups/{g}/rule-instances`.
    RuleInstanceList,
    /// `GET <mgmt>/security-groups/{g}/rule-instances/{i}`.
    RuleInstanceGet,
    /// `GET <mgmt>/security-rules`.
    RuleList,
    /// `GET <mgmt>/security-rules/{uuid}`.
    RuleGet,
    /// `GET <data>/scans`.
    ScanList,
    /// `GET <data>/scans/{uuid}`.
    ScanGet,
    /// `GET <data>/scans/{s}/evaluations`.
    Evaluations,
    /// `GET <data>/evaluations/{uuid}`.
    EvaluationGet,
    /// `GET <data>/scans/{s}/rule-violations`.
    Violations,
    /// `GET <data>/violations/{uuid}`.
    ViolationGet,
    /// `GET <data>/scans/{s}/files`.
    ScanFiles,
    /// `GET <data>/scans/label-keys`.
    LabelKeys,
    /// `GET <data>/scans/label-keys/{key}/values`.
    LabelValues,
    /// `GET <data>/models`.
    ModelList,
    /// `GET <data>/models/{uuid}`.
    ModelGet,
    /// `GET <data>/models/{m}/model-versions`.
    VersionList,
    /// `GET <data>/model-versions/{uuid}`.
    VersionGet,
    /// `GET <data>/model-versions/{v}/files`.
    VersionFiles,
    /// `POST <mgmt>/security-groups`.
    GroupCreate,
    /// `PUT <mgmt>/security-groups/{uuid}`.
    GroupUpdate,
    /// `DELETE <mgmt>/security-groups/{uuid}`.
    GroupDelete,
    /// `PUT <mgmt>/security-groups/{g}/rule-instances/{i}`.
    RuleInstanceUpdate,
    /// `POST <data>/scans` (billed in the real service).
    ScanCreate,
    /// `POST <data>/scans/{s}/labels`.
    LabelsAdd,
    /// `PUT <data>/scans/{s}/labels`.
    LabelsSet,
    /// `DELETE <data>/scans/{s}/labels?keys=…`.
    LabelsDelete,
    /// `GET <mgmt>/pypi/authenticate` (the credential-bearing index URL, §17.12).
    PypiAuth,
}

impl Ms {
    /// A short stable label (`ms-groups-list`, ...).
    pub fn label(self) -> &'static str {
        match self {
            Ms::GroupList => "ms-groups-list",
            Ms::GroupGet => "ms-groups-get",
            Ms::RuleInstanceList => "ms-rule-instances-list",
            Ms::RuleInstanceGet => "ms-rule-instances-get",
            Ms::RuleList => "ms-rules-list",
            Ms::RuleGet => "ms-rules-get",
            Ms::ScanList => "ms-scans-list",
            Ms::ScanGet => "ms-scans-get",
            Ms::Evaluations => "ms-scans-evaluations",
            Ms::EvaluationGet => "ms-evaluations-get",
            Ms::Violations => "ms-scans-violations",
            Ms::ViolationGet => "ms-violations-get",
            Ms::ScanFiles => "ms-scans-files",
            Ms::LabelKeys => "ms-label-keys",
            Ms::LabelValues => "ms-label-values",
            Ms::ModelList => "ms-models-list",
            Ms::ModelGet => "ms-models-get",
            Ms::VersionList => "ms-model-versions-list",
            Ms::VersionGet => "ms-model-versions-get",
            Ms::VersionFiles => "ms-model-version-files",
            Ms::GroupCreate => "ms-groups-create",
            Ms::GroupUpdate => "ms-groups-update",
            Ms::GroupDelete => "ms-groups-delete",
            Ms::RuleInstanceUpdate => "ms-rule-instances-update",
            Ms::ScanCreate => "ms-scans-create",
            Ms::LabelsAdd => "ms-labels-add",
            Ms::LabelsSet => "ms-labels-set",
            Ms::LabelsDelete => "ms-labels-delete",
            Ms::PypiAuth => "ms-pypi-auth",
        }
    }

    /// Whether the route is a write (every non-GET request is one, D179).
    pub fn is_write(self) -> bool {
        matches!(
            self,
            Ms::GroupCreate
                | Ms::GroupUpdate
                | Ms::GroupDelete
                | Ms::RuleInstanceUpdate
                | Ms::ScanCreate
                | Ms::LabelsAdd
                | Ms::LabelsSet
                | Ms::LabelsDelete
        )
    }

    /// The write route of `method` on a read route's path, if the service has one.
    fn write_of(method: &str, read: Ms, labels: bool) -> Option<Ms> {
        match (method, read, labels) {
            ("POST", Ms::GroupList, _) => Some(Ms::GroupCreate),
            ("PUT", Ms::GroupGet, _) => Some(Ms::GroupUpdate),
            ("DELETE", Ms::GroupGet, _) => Some(Ms::GroupDelete),
            ("PUT", Ms::RuleInstanceGet, _) => Some(Ms::RuleInstanceUpdate),
            ("POST", Ms::ScanList, _) => Some(Ms::ScanCreate),
            ("POST", _, true) => Some(Ms::LabelsAdd),
            ("PUT", _, true) => Some(Ms::LabelsSet),
            ("DELETE", _, true) => Some(Ms::LabelsDelete),
            _ => None,
        }
    }

    /// Classifies a request path under `/aims/mgmt/v1/` or `/aims/data/v1/`; `None` for
    /// anything else.
    pub fn classify(method: &str, path: &str) -> Option<Ms> {
        if method != "GET" {
            if !matches!(method, "POST" | "PUT" | "DELETE") {
                return None;
            }
            // `…/scans/<s>/labels` exists only for writes.
            let labels = path.strip_prefix(DATA_PREFIX).is_some_and(|rest| {
                let segments: Vec<&str> = rest.split('/').collect();
                matches!(segments.as_slice(), ["", "scans", id, "labels"] if !id.is_empty() && *id != "label-keys")
            });
            let read = if labels {
                Some(Ms::ScanGet)
            } else {
                Ms::classify("GET", path)
            }?;
            return Ms::write_of(method, read, labels);
        }
        let item = |segment: &&str| !segment.is_empty();
        if let Some(rest) = path.strip_prefix(MGMT_PREFIX) {
            let segments: Vec<&str> = rest.strip_prefix('/')?.split('/').collect();
            return match segments.as_slice() {
                ["security-groups"] => Some(Ms::GroupList),
                ["security-groups", id] if item(id) => Some(Ms::GroupGet),
                ["security-groups", id, "rule-instances"] if item(id) => Some(Ms::RuleInstanceList),
                ["security-groups", id, "rule-instances", instance]
                    if item(id) && item(instance) =>
                {
                    Some(Ms::RuleInstanceGet)
                }
                ["pypi", "authenticate"] => Some(Ms::PypiAuth),
                ["security-rules"] => Some(Ms::RuleList),
                ["security-rules", id] if item(id) => Some(Ms::RuleGet),
                _ => None,
            };
        }
        let rest = path.strip_prefix(DATA_PREFIX)?;
        let segments: Vec<&str> = rest.strip_prefix('/')?.split('/').collect();
        match segments.as_slice() {
            // The label routes first: `label-keys` is not a scan id.
            ["scans", "label-keys"] => Some(Ms::LabelKeys),
            ["scans", "label-keys", key, "values"] if item(key) => Some(Ms::LabelValues),
            ["scans"] => Some(Ms::ScanList),
            ["scans", id] if item(id) => Some(Ms::ScanGet),
            ["scans", id, "evaluations"] if item(id) => Some(Ms::Evaluations),
            ["scans", id, "rule-violations"] if item(id) => Some(Ms::Violations),
            ["scans", id, "files"] if item(id) => Some(Ms::ScanFiles),
            ["evaluations", id] if item(id) => Some(Ms::EvaluationGet),
            ["violations", id] if item(id) => Some(Ms::ViolationGet),
            ["models"] => Some(Ms::ModelList),
            ["models", id] if item(id) => Some(Ms::ModelGet),
            ["models", id, "model-versions"] if item(id) => Some(Ms::VersionList),
            ["model-versions", id] if item(id) => Some(Ms::VersionGet),
            ["model-versions", id, "files"] if item(id) => Some(Ms::VersionFiles),
            _ => None,
        }
    }
}

/// The model-security dataset of one fixture server. Tests seed it with
/// [`ModelSecData::seeded`] (through `FixtureServer::seed_modelsec`) and edit its public
/// fields (`FixtureServer::modelsec`).
#[derive(Clone, Debug, Default)]
pub struct ModelSecData {
    pub groups: Vec<Value>,
    pub rules: Vec<Value>,
    /// Rule instances; each names its `security_group_uuid`.
    pub rule_instances: Vec<Value>,
    pub scans: Vec<Value>,
    /// Evaluations; each names its `scan_uuid`.
    pub evaluations: Vec<Value>,
    /// Violations; each names its `scan_uuid`.
    pub violations: Vec<Value>,
    /// Files; each names its `scan_uuid` and `model_version_uuid`.
    pub files: Vec<Value>,
    pub models: Vec<Value>,
    /// Versions; each names its `model_uuid`.
    pub versions: Vec<Value>,
    pub label_keys: Vec<String>,
    /// Label values per key.
    pub label_values: BTreeMap<String, Vec<String>>,
    /// Report this `pagination.total_items` on every list page instead of the count.
    pub total_override: Option<u64>,
    /// Omit `pagination.total_items` from every list page.
    pub omit_totals: bool,
    /// Serve the first page for every `skip` (a service that ignores paging).
    pub ignore_skip: bool,
    /// The largest page the service serves (a larger `limit` is capped).
    pub page_cap: Option<usize>,
    /// Scan list pages say `count_capped: true` and report this total (a cap, D182).
    pub count_capped: Option<u64>,
    /// What an accepted group `DELETE` does (D186).
    pub group_delete: GroupDeleteMode,
    /// Groups created so far (the next one is `U(101 + n)`).
    pub groups_created: u64,
    /// Scans created so far (the next one is `U(451 + n)`).
    pub scans_created: u64,
    /// The `url` of `GET /pypi/authenticate` instead of [`fx::pypi_url`] of the caller's
    /// TSG (an http URL, no userinfo, a query, …; the §17.12 knobs).
    pub pypi_url: Option<String>,
    /// The `created_at`/`updated_at` of the records the write routes create (default the
    /// fixture's fixed [`fx::AT`]; `now`: the time of the create, as the live self-tests
    /// use).
    pub created_at: Option<String>,
    /// The group list leaves out tombstoned groups (the live self-tests' service).
    pub list_hides_tombstones: bool,
    /// [`GroupDeleteMode::Lagging`]: the reads each deleted group still answers.
    pub delete_lag: BTreeMap<String, u32>,
    /// A scan the write route created takes this `eval_outcome` at its first read (the
    /// live scan flow's bounded wait ends); `None` keeps it `PENDING`.
    pub created_scans_finish_as: Option<String>,
    /// A created group gets one rule instance `U(351 + n)` of the first rule (as the
    /// service creates instances in a new group; the live write flow's knob).
    pub instances_for_created_groups: bool,
}

/// What the fixture does with an accepted group `DELETE` (§10 M6c: the service deletes
/// asynchronously, so a re-read may still find the group).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroupDeleteMode {
    /// The group is gone: a later `get` answers 404.
    #[default]
    Hard,
    /// The group is still served, with `is_tombstone: true`.
    Tombstone,
    /// The group is still served unchanged (`state` as before).
    StillActive,
    /// The group is still served (unchanged) to the next `n` reads of it, then gone (a
    /// soft delete the service finishes later).
    Lagging(u32),
}

impl ModelSecData {
    /// The timestamp of a record created now ([`ModelSecData::created_at`]).
    pub(super) fn stamp(&self) -> Option<String> {
        match self.created_at.as_deref() {
            Some("now") => Some(utc_now()),
            other => other.map(str::to_owned),
        }
    }

    /// The standard dataset (`fixtures::modelsec`): two groups, two rules, two rule
    /// instances in group `U(1)`, two scans with the evaluations, violations and files of
    /// the first, two models with two versions of the first, two label keys.
    pub fn seeded() -> Self {
        Self {
            groups: fx::groups(),
            rules: fx::rules(),
            rule_instances: fx::rule_instances(),
            scans: fx::scans(),
            evaluations: fx::evaluations(),
            violations: fx::violations(),
            files: fx::files(),
            models: fx::models(),
            versions: fx::versions(),
            label_keys: vec!["env".to_owned(), "team".to_owned()],
            label_values: BTreeMap::from([
                (
                    "env".to_owned(),
                    vec!["prod".to_owned(), "staging".to_owned()],
                ),
                ("team".to_owned(), vec!["ml".to_owned()]),
            ]),
            ..Self::default()
        }
    }
}

/// The percent-decoded path segments after a plane prefix.
fn segments(path: &str) -> Vec<String> {
    path.split('/').map(percent_decode).collect()
}

pub(super) fn not_found(what: &str) -> Planned {
    Planned::json(404, &json!({"message": format!("{what} not found")}))
}

pub(super) fn find<'a>(rows: &'a [Value], id: &str) -> Option<&'a Value> {
    rows.iter()
        .find(|row| row.get("uuid").and_then(Value::as_str) == Some(id))
}

fn with_field<'a>(rows: &'a [Value], field: &str, value: &str) -> Vec<&'a Value> {
    rows.iter()
        .filter(|row| row.get(field).and_then(Value::as_str) == Some(value))
        .collect()
}

/// The answer of a model-security route (credentials already checked by the caller).
pub(crate) fn answer(data: &mut ModelSecData, route: Ms, request: &RecordedRequest) -> Planned {
    if request.header("x-pan-token").is_some() {
        return Planned::empty_json(403);
    }
    let AuthIdentity::Bearer { tsg: Some(tsg) } = &request.auth else {
        return Planned::empty_json(401);
    };
    if route == Ms::PypiAuth {
        let url = data.pypi_url.clone().unwrap_or_else(|| fx::pypi_url(tsg));
        return Planned::json(200, &json!({"url": url, "expires_at": fx::PYPI_EXPIRES}));
    }
    let parts = segments(&request.path);
    // `/aims/<plane>/v1/<a>/<b>/…`: parts[0] is empty.
    let part = |index: usize| parts.get(index + 4).map(String::as_str).unwrap_or_default();
    if route.is_write() {
        return super::modelsec_writes::answer(data, route, request, &|index| {
            part(index).to_owned()
        });
    }
    if route == Ms::ScanGet
        && let Some(outcome) = data.created_scans_finish_as.clone()
    {
        let id = part(1).to_owned();
        let created = (1..=data.scans_created).any(|n| fx::uuid(450 + n) == id);
        if let Some(scan) = data
            .scans
            .iter_mut()
            .find(|row| created && row.get("uuid").and_then(Value::as_str) == Some(id.as_str()))
        {
            scan["eval_outcome"] = json!(outcome);
        }
    }
    if route == Ms::GroupGet {
        let id = part(1).to_owned();
        if let Some(left) = data.delete_lag.get_mut(&id) {
            if *left == 0 {
                data.delete_lag.remove(&id);
                data.groups
                    .retain(|row| row.get("uuid").and_then(Value::as_str) != Some(id.as_str()));
            } else {
                *left -= 1;
            }
        }
    }
    match route {
        Ms::GroupList => page(
            data,
            request,
            "security_groups",
            data.groups
                .iter()
                .filter(|row| {
                    !(data.list_hides_tombstones
                        && row.get("is_tombstone").and_then(Value::as_bool) == Some(true))
                })
                .collect(),
            20,
        ),
        Ms::GroupGet => find(&data.groups, part(1)).map_or_else(
            || not_found("security group"),
            |row| Planned::json(200, row),
        ),
        Ms::RuleInstanceList => {
            if find(&data.groups, part(1)).is_none() {
                return not_found("security group");
            }
            let mut rows = with_field(&data.rule_instances, "security_group_uuid", part(1));
            if let Some(rule) = request.query_value("security_rule_uuid") {
                rows.retain(|row| {
                    row.get("security_rule_uuid").and_then(Value::as_str) == Some(rule)
                });
            }
            if let Some(state) = request.query_value("state") {
                rows.retain(|row| row.get("state").and_then(Value::as_str) == Some(state));
            }
            page(data, request, "rule_instances", rows, 20)
        }
        Ms::RuleInstanceGet => with_field(&data.rule_instances, "security_group_uuid", part(1))
            .into_iter()
            .find(|row| row.get("uuid").and_then(Value::as_str) == Some(part(3)))
            .map_or_else(|| not_found("rule instance"), |row| Planned::json(200, row)),
        Ms::RuleList => {
            let mut rows: Vec<&Value> = data.rules.iter().collect();
            if let Some(source) = request.query_value("source_type") {
                rows.retain(|row| {
                    row.get("compatible_sources")
                        .and_then(Value::as_array)
                        .is_some_and(|sources| sources.iter().any(|s| s.as_str() == Some(source)))
                });
            }
            page(data, request, "rules", rows, 20)
        }
        Ms::RuleGet => find(&data.rules, part(1))
            .map_or_else(|| not_found("security rule"), |row| Planned::json(200, row)),
        Ms::ScanList => {
            let mut rows: Vec<&Value> = data.scans.iter().collect();
            if let Some(outcome) = request.query_value("eval_outcomes") {
                rows.retain(|row| row.get("eval_outcome").and_then(Value::as_str) == Some(outcome));
            }
            let mut answer = page(data, request, "scans", rows, 20);
            if let Some(cap) = data.count_capped
                && answer.status == 200
                && let Ok(mut body) = serde_json::from_slice::<Value>(&answer.body)
            {
                body["count_capped"] = json!(true);
                body["pagination"] = json!({"total_items": cap});
                answer = Planned::json(200, &body);
            }
            answer
        }
        Ms::ScanGet => find(&data.scans, part(1))
            .map_or_else(|| not_found("scan"), |row| Planned::json(200, row)),
        Ms::Evaluations => page(
            data,
            request,
            "evaluations",
            with_field(&data.evaluations, "scan_uuid", part(1)),
            20,
        ),
        Ms::EvaluationGet => find(&data.evaluations, part(1))
            .map_or_else(|| not_found("evaluation"), |row| Planned::json(200, row)),
        Ms::Violations => page(
            data,
            request,
            "violations",
            with_field(&data.violations, "scan_uuid", part(1)),
            20,
        ),
        Ms::ViolationGet => find(&data.violations, part(1))
            .map_or_else(|| not_found("violation"), |row| Planned::json(200, row)),
        Ms::ScanFiles => {
            let mut rows = with_field(&data.files, "scan_uuid", part(1));
            if let Some(kind) = request.query_value("type") {
                rows.retain(|row| row.get("type").and_then(Value::as_str) == Some(kind));
            }
            if let Some(result) = request.query_value("result") {
                rows.retain(|row| row.get("result").and_then(Value::as_str) == Some(result));
            }
            page(data, request, "files", rows, 20)
        }
        Ms::LabelKeys => {
            let keys: Vec<Value> = data.label_keys.iter().map(|key| json!(key)).collect();
            page(data, request, "keys", keys.iter().collect(), 20)
        }
        Ms::LabelValues => {
            let values: Vec<Value> = data
                .label_values
                .get(part(2))
                .map(|values| values.iter().map(|value| json!(value)).collect())
                .unwrap_or_default();
            page(data, request, "values", values.iter().collect(), 20)
        }
        Ms::ModelList => page(data, request, "models", data.models.iter().collect(), 50),
        Ms::ModelGet => find(&data.models, part(1))
            .map_or_else(|| not_found("model"), |row| Planned::json(200, row)),
        Ms::VersionList => page(
            data,
            request,
            "model_versions",
            with_field(&data.versions, "model_uuid", part(1)),
            50,
        ),
        Ms::VersionGet => find(&data.versions, part(1))
            .map_or_else(|| not_found("model version"), |row| Planned::json(200, row)),
        Ms::VersionFiles => page(
            data,
            request,
            "files",
            with_field(&data.files, "model_version_uuid", part(1)),
            50,
        ),
        // Answered by `modelsec_writes::answer` above.
        Ms::GroupCreate
        | Ms::GroupUpdate
        | Ms::GroupDelete
        | Ms::RuleInstanceUpdate
        | Ms::ScanCreate
        | Ms::LabelsAdd
        | Ms::LabelsSet
        | Ms::LabelsDelete
        // Answered above.
        | Ms::PypiAuth => Planned::empty_json(404),
    }
}

/// One page of `rows` under `key`: `skip` (default 0) and `limit` (default
/// `default_limit`; `0` or malformed is a 422), with the dataset's knobs.
fn page(
    data: &ModelSecData,
    request: &RecordedRequest,
    key: &str,
    rows: Vec<&Value>,
    default_limit: usize,
) -> Planned {
    let number = |name: &str, default: usize| match request.query_value(name) {
        None => Some(default),
        Some(value) => value.parse::<usize>().ok(),
    };
    let (Some(skip), Some(limit)) = (number("skip", 0), number("limit", default_limit)) else {
        return Planned::json(422, &json!({"detail": [{"msg": "invalid paging"}]}));
    };
    if limit == 0 {
        return Planned::json(422, &json!({"detail": [{"msg": "limit must be positive"}]}));
    }
    let limit = data.page_cap.map_or(limit, |cap| limit.min(cap));
    let skip = if data.ignore_skip { 0 } else { skip };
    let total = data.total_override.unwrap_or(rows.len() as u64);
    let start = skip.min(rows.len());
    let end = skip.saturating_add(limit).min(rows.len());
    let shown: Vec<Value> = rows[start..end].iter().map(|row| (*row).clone()).collect();
    let pagination = if data.omit_totals {
        json!({})
    } else {
        json!({"total_items": total})
    };
    let mut body = serde_json::Map::new();
    body.insert("pagination".to_owned(), pagination);
    body.insert(key.to_owned(), Value::Array(shown));
    Planned::json(200, &Value::Object(body))
}

/// The current UTC time as `YYYY-MM-DDTHH:MM:SSZ` (Howard Hinnant's civil_from_days).
fn utc_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let rest = seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest / 60 % 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_the_read_and_write_routes() {
        let u = fx::uuid(1);
        for (method, path, route) in [
            (
                "GET",
                "/aims/mgmt/v1/security-groups".to_owned(),
                Some(Ms::GroupList),
            ),
            (
                "GET",
                format!("/aims/mgmt/v1/security-groups/{u}"),
                Some(Ms::GroupGet),
            ),
            (
                "GET",
                format!("/aims/mgmt/v1/security-groups/{u}/rule-instances"),
                Some(Ms::RuleInstanceList),
            ),
            (
                "GET",
                format!("/aims/mgmt/v1/security-groups/{u}/rule-instances/{u}"),
                Some(Ms::RuleInstanceGet),
            ),
            (
                "GET",
                "/aims/mgmt/v1/security-rules".to_owned(),
                Some(Ms::RuleList),
            ),
            (
                "GET",
                format!("/aims/mgmt/v1/security-rules/{u}"),
                Some(Ms::RuleGet),
            ),
            ("GET", "/aims/data/v1/scans".to_owned(), Some(Ms::ScanList)),
            (
                "GET",
                "/aims/data/v1/scans/label-keys".to_owned(),
                Some(Ms::LabelKeys),
            ),
            (
                "GET",
                "/aims/data/v1/scans/label-keys/a%2Fb/values".to_owned(),
                Some(Ms::LabelValues),
            ),
            ("GET", format!("/aims/data/v1/scans/{u}"), Some(Ms::ScanGet)),
            (
                "GET",
                format!("/aims/data/v1/scans/{u}/evaluations"),
                Some(Ms::Evaluations),
            ),
            (
                "GET",
                format!("/aims/data/v1/scans/{u}/rule-violations"),
                Some(Ms::Violations),
            ),
            (
                "GET",
                format!("/aims/data/v1/scans/{u}/files"),
                Some(Ms::ScanFiles),
            ),
            (
                "GET",
                format!("/aims/data/v1/evaluations/{u}"),
                Some(Ms::EvaluationGet),
            ),
            (
                "GET",
                format!("/aims/data/v1/violations/{u}"),
                Some(Ms::ViolationGet),
            ),
            (
                "GET",
                "/aims/data/v1/models".to_owned(),
                Some(Ms::ModelList),
            ),
            (
                "GET",
                format!("/aims/data/v1/models/{u}"),
                Some(Ms::ModelGet),
            ),
            (
                "GET",
                format!("/aims/data/v1/models/{u}/model-versions"),
                Some(Ms::VersionList),
            ),
            (
                "GET",
                format!("/aims/data/v1/model-versions/{u}"),
                Some(Ms::VersionGet),
            ),
            (
                "GET",
                format!("/aims/data/v1/model-versions/{u}/files"),
                Some(Ms::VersionFiles),
            ),
            // TS sends `labels values ..` here (MS§2.8): not a route.
            (
                "GET",
                "/aims/data/v1/scans/values".to_owned(),
                Some(Ms::ScanGet),
            ),
            (
                "GET",
                "/aims/data/v1/scans/label-keys//values".to_owned(),
                None,
            ),
            ("GET", "/aims/data/v1/scans/".to_owned(), None),
            ("PATCH", "/aims/mgmt/v1/security-groups".to_owned(), None),
            (
                "POST",
                "/aims/mgmt/v1/security-groups".to_owned(),
                Some(Ms::GroupCreate),
            ),
            (
                "PUT",
                format!("/aims/mgmt/v1/security-groups/{u}"),
                Some(Ms::GroupUpdate),
            ),
            (
                "DELETE",
                format!("/aims/mgmt/v1/security-groups/{u}"),
                Some(Ms::GroupDelete),
            ),
            ("DELETE", "/aims/mgmt/v1/security-groups".to_owned(), None),
            (
                "PUT",
                format!("/aims/mgmt/v1/security-groups/{u}/rule-instances/{u}"),
                Some(Ms::RuleInstanceUpdate),
            ),
            (
                "POST",
                "/aims/data/v1/scans".to_owned(),
                Some(Ms::ScanCreate),
            ),
            (
                "POST",
                format!("/aims/data/v1/scans/{u}/labels"),
                Some(Ms::LabelsAdd),
            ),
            (
                "PUT",
                format!("/aims/data/v1/scans/{u}/labels"),
                Some(Ms::LabelsSet),
            ),
            (
                "DELETE",
                format!("/aims/data/v1/scans/{u}/labels"),
                Some(Ms::LabelsDelete),
            ),
            ("GET", format!("/aims/data/v1/scans/{u}/labels"), None),
            ("PUT", format!("/aims/data/v1/scans/{u}"), None),
            (
                "POST",
                "/aims/data/v1/scans/label-keys/labels".to_owned(),
                None,
            ),
            ("GET", "/aims/mgmt/v1/other".to_owned(), None),
            ("GET", "/aims/other/v1/scans".to_owned(), None),
        ] {
            assert_eq!(Ms::classify(method, &path), route, "{method} {path}");
        }
    }
}
