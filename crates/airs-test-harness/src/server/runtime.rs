//! The runtime extras fixture routes (DESIGN §10 M6d): API keys, customer apps and
//! deployment profiles under the management base `/aisec` (no `x-tsg-id`; the TSG in the
//! path), and the dashboard and sessions reads under the dashboard base `/dashboard`
//! (`x-tsg-id: <tsg>` required), over a per-server dataset ([`RuntimeData`]).
//!
//! The offset-cursor lists answer `{<rows>, next_offset}` pages (knobs: a repeated row, a
//! repeated cursor, an empty page with a fresh cursor, a row carrying an `api_key`, a
//! customer-app record with a nested `api_key`). The dashboard lists are skip/limit pages
//! with `pagination {limit, skip, total_items}` (knobs: a page cap, a reported total other
//! than the count, no total, a service that ignores the offset). `POST
//! /aisec/v1/mgmt/scanlogs` is a trap: the CLIs never send it (§18.11).
//!
//! The write routes (step 2): `POST <mgmt>/apikey` (the required fields checked; a new
//! `api_key_id` and `api_key: FAKE-APIKEY-SECRET-<tsg>-<n>`), `POST
//! <mgmt>/apikey/regenerate/{id}` (a new id and key, the old key revoked), `DELETE
//! <mgmt>/apikey/delete/{name}?updated_by=`, `PUT <mgmt>/customerapp?customer_app_id=`
//! (replaces the record) and `DELETE <mgmt>/customerapp?app_name=&updated_by=` (removes
//! the app and its keys); knobs: an answer without `api_key`, without `auth_code`, the
//! delete answer shapes.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::plan::Planned;
use super::record::{AuthIdentity, RecordedRequest};
use crate::fixtures::runtime as fx;

/// The management prefix of the runtime routes.
pub const MGMT: &str = "/aisec/v1/mgmt";
/// The dashboard prefix (`mgmtDashboardEndpoint = <base>/dashboard`).
pub const DASH: &str = "/dashboard/v1/mgmt";

/// A runtime extras fixture route (`Route::Runtime`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rx {
    /// `GET <mgmt>/apikeys/tsg/{tsg}`.
    ApiKeys,
    /// `GET <mgmt>/customerapp/tsg/{tsg}`.
    CustomerApps,
    /// `GET <mgmt>/customerapp?app_name=`.
    CustomerApp,
    /// `GET <mgmt>/deploymentprofiles`.
    DeploymentProfiles,
    /// `POST <mgmt>/scanlogs` (a trap: never sent).
    ScanLogs,
    /// `GET <dash>/dashboard/v2/apps/applicationsoverview`.
    AppsOverview,
    /// `GET <dash>/dashboard/v2/apps/application`.
    Application,
    /// `GET <dash>/dashboard/v2/apps/applicationviolationbreakdown`.
    ApplicationViolations,
    /// `GET <dash>/dashboard/v2/apps/topapplicationsviolations`.
    TopApplications,
    /// `GET <dash>/dashboard/v2/apps/applicationsviolationstrend`.
    ViolationsTrend,
    /// `GET <dash>/dashboard/v2/apps/appslist`.
    AppsList,
    /// `GET <dash>/dashboard/v2/sessions/sessionschart`.
    SessionsChart,
    /// `GET <dash>/dashboard/v2/sessions/sessionsoverview`.
    SessionsOverview,
    /// `GET <dash>/dashboard/v2/sessions/session`.
    Session,
    /// `GET <dash>/dashboard/v2/sessions/sessiontransaction`.
    SessionTransaction,
    /// `GET <dash>/reports/scancontent`.
    ScanContent,
    /// `POST <mgmt>/apikey`.
    ApiKeyCreate,
    /// `POST <mgmt>/apikey/regenerate/{id}`.
    ApiKeyRegenerate,
    /// `DELETE <mgmt>/apikey/delete/{name}`.
    ApiKeyDelete,
    /// `PUT <mgmt>/customerapp?customer_app_id=`.
    CustomerAppUpdate,
    /// `DELETE <mgmt>/customerapp?app_name=&updated_by=`.
    CustomerAppDelete,
}

impl Rx {
    /// A short stable label (`rx-api-keys`, ...).
    pub fn label(self) -> &'static str {
        match self {
            Rx::ApiKeys => "rx-api-keys",
            Rx::CustomerApps => "rx-customer-apps",
            Rx::CustomerApp => "rx-customer-app",
            Rx::DeploymentProfiles => "rx-deployment-profiles",
            Rx::ScanLogs => "rx-scan-logs",
            Rx::AppsOverview => "rx-apps-overview",
            Rx::Application => "rx-application",
            Rx::ApplicationViolations => "rx-application-violations",
            Rx::TopApplications => "rx-top-applications",
            Rx::ViolationsTrend => "rx-violations-trend",
            Rx::AppsList => "rx-apps-list",
            Rx::SessionsChart => "rx-sessions-chart",
            Rx::SessionsOverview => "rx-sessions-overview",
            Rx::Session => "rx-session",
            Rx::SessionTransaction => "rx-session-transaction",
            Rx::ScanContent => "rx-scan-content",
            Rx::ApiKeyCreate => "rx-api-key-create",
            Rx::ApiKeyRegenerate => "rx-api-key-regenerate",
            Rx::ApiKeyDelete => "rx-api-key-delete",
            Rx::CustomerAppUpdate => "rx-customer-app-update",
            Rx::CustomerAppDelete => "rx-customer-app-delete",
        }
    }

    /// Whether the route is a write (the scan-logs trap is a POST).
    pub fn is_write(self) -> bool {
        matches!(
            self,
            Rx::ScanLogs
                | Rx::ApiKeyCreate
                | Rx::ApiKeyRegenerate
                | Rx::ApiKeyDelete
                | Rx::CustomerAppUpdate
                | Rx::CustomerAppDelete
        )
    }

    /// Whether the route is on the dashboard base.
    pub fn is_dashboard(self) -> bool {
        !matches!(
            self,
            Rx::ApiKeys
                | Rx::CustomerApps
                | Rx::CustomerApp
                | Rx::DeploymentProfiles
                | Rx::ScanLogs
                | Rx::ApiKeyCreate
                | Rx::ApiKeyRegenerate
                | Rx::ApiKeyDelete
                | Rx::CustomerAppUpdate
                | Rx::CustomerAppDelete
        )
    }

    /// Classifies a request; returns the route and the `{tsg}` path segment when the route
    /// carries one.
    pub fn classify(method: &str, path: &str) -> Option<(Rx, Option<String>)> {
        if let Some(rest) = path.strip_prefix(MGMT) {
            let segments: Vec<&str> = rest.strip_prefix('/')?.split('/').collect();
            return match (method, segments.as_slice()) {
                ("GET", ["apikeys", "tsg", tsg]) if !tsg.is_empty() => {
                    Some((Rx::ApiKeys, Some((*tsg).to_owned())))
                }
                ("GET", ["customerapp", "tsg", tsg]) if !tsg.is_empty() => {
                    Some((Rx::CustomerApps, Some((*tsg).to_owned())))
                }
                ("GET", ["customerapp"]) => Some((Rx::CustomerApp, None)),
                ("GET", ["deploymentprofiles"]) => Some((Rx::DeploymentProfiles, None)),
                ("POST", ["scanlogs"]) => Some((Rx::ScanLogs, None)),
                ("POST", ["apikey"]) => Some((Rx::ApiKeyCreate, None)),
                ("POST", ["apikey", "regenerate", id]) if !id.is_empty() => {
                    Some((Rx::ApiKeyRegenerate, None))
                }
                ("DELETE", ["apikey", "delete", name]) if !name.is_empty() => {
                    Some((Rx::ApiKeyDelete, None))
                }
                ("PUT", ["customerapp"]) => Some((Rx::CustomerAppUpdate, None)),
                ("DELETE", ["customerapp"]) => Some((Rx::CustomerAppDelete, None)),
                _ => None,
            };
        }
        let rest = path.strip_prefix(DASH)?;
        if method != "GET" {
            return None;
        }
        let route = match rest {
            "/dashboard/v2/apps/applicationsoverview" => Rx::AppsOverview,
            "/dashboard/v2/apps/application" => Rx::Application,
            "/dashboard/v2/apps/applicationviolationbreakdown" => Rx::ApplicationViolations,
            "/dashboard/v2/apps/topapplicationsviolations" => Rx::TopApplications,
            "/dashboard/v2/apps/applicationsviolationstrend" => Rx::ViolationsTrend,
            "/dashboard/v2/apps/appslist" => Rx::AppsList,
            "/dashboard/v2/sessions/sessionschart" => Rx::SessionsChart,
            "/dashboard/v2/sessions/sessionsoverview" => Rx::SessionsOverview,
            "/dashboard/v2/sessions/session" => Rx::Session,
            "/dashboard/v2/sessions/sessiontransaction" => Rx::SessionTransaction,
            "/reports/scancontent" => Rx::ScanContent,
            _ => return None,
        };
        Some((route, None))
    }
}

/// The runtime extras dataset of one fixture server. Tests seed it with
/// [`RuntimeData::seeded`] (`FixtureServer::seed_runtime`) and edit its public fields
/// (`FixtureServer::runtime`).
#[derive(Clone, Debug, Default)]
pub struct RuntimeData {
    pub api_keys: Vec<Value>,
    pub customer_apps: Vec<Value>,
    /// The record `GET customerapp?app_name=` answers, per name (default: the list row of
    /// that name).
    pub customer_app_records: BTreeMap<String, Value>,
    pub deployment_profiles: Vec<Value>,
    /// Served only with `?unactivated=true` (after the activated ones).
    pub unactivated_profiles: Vec<Value>,
    /// Every offset page serves the first page's rows (a service ignoring the offset).
    pub repeat_rows: bool,
    /// `next_offset` is always `limit` (a repeated cursor).
    pub repeat_cursor: bool,
    /// The first page is empty with a fresh `next_offset` (TS follows it, D46).
    pub empty_first_page: bool,
    /// The last page says `next_offset: 0` instead of omitting it (D12).
    pub zero_next_offset: bool,
    /// The dashboard overview items.
    pub apps: Vec<Value>,
    /// `apps/application` answers by `<appid>|<appname>` (default: [`fx::application`] of
    /// the query).
    pub applications: BTreeMap<String, Value>,
    /// `applicationviolationbreakdown` answers by `<appid>|<appname>` (default:
    /// [`fx::breakdown`]).
    pub breakdowns: BTreeMap<String, Value>,
    /// The sessions of `sessionsoverview`.
    pub sessions: Vec<Value>,
    /// Exact app ID/name/session response overrides, independent of arrival order.
    pub session_records: BTreeMap<(String, String, String), Value>,
    /// The largest dashboard page served (a larger `limit` is capped).
    pub page_cap: Option<usize>,
    /// Report this `total_items` instead of the count.
    pub total_override: Option<u64>,
    /// Omit `pagination.total_items`.
    pub omit_totals: bool,
    /// Serve the first dashboard page for every offset.
    pub ignore_offset: bool,
    /// API keys issued by the create and regenerate routes so far (their ids are
    /// `fx::uuid(900 + n)`, their values `FAKE-APIKEY-SECRET-<tsg>-<n>`).
    pub keys_issued: u64,
    /// The key answers omit `api_key` (a key the CLI cannot save).
    pub omit_key_value: bool,
    /// The key answers omit `auth_code` (a 2xx that fails the response schema, G26).
    pub omit_auth_code: bool,
    /// The `DELETE apikey/delete` answer (default `{"message": "api key <name> deleted"}`).
    pub key_delete_answer: Option<Value>,
    /// The `DELETE customerapp` answer (default `{"message": "customer app and associated
    /// keys successfully deleted"}`).
    pub app_delete_answer: Option<Value>,
}

impl RuntimeData {
    /// The standard dataset (`fixtures::runtime`).
    pub fn seeded() -> Self {
        Self {
            api_keys: fx::api_keys(),
            customer_apps: fx::customer_apps(),
            deployment_profiles: vec![
                fx::deployment_profile(1, "active"),
                fx::deployment_profile(2, "activated"),
            ],
            unactivated_profiles: vec![fx::deployment_profile(3, "unactivated")],
            apps: fx::overview_items(),
            sessions: fx::sessions(),
            ..Self::default()
        }
    }
}

fn number(request: &RecordedRequest, name: &str, default: usize) -> Option<usize> {
    match request.query_value(name) {
        None => Some(default),
        Some(value) => value.parse().ok(),
    }
}

/// One offset-cursor page of `rows` under `key`: `rows[offset..offset + limit]` and a
/// `next_offset` while rows remain, with the dataset's knobs.
fn cursor_page(
    data: &RuntimeData,
    request: &RecordedRequest,
    rows: &[Value],
    key: &str,
) -> Planned {
    let (Some(offset), Some(limit)) = (number(request, "offset", 0), number(request, "limit", 100))
    else {
        return Planned::json(400, &json!({"message": "invalid paging"}));
    };
    if limit == 0 {
        return Planned::json(400, &json!({"message": "invalid paging"}));
    }
    let mut body = Map::new();
    let page = |start: usize| {
        let start = start.min(rows.len());
        let end = start.saturating_add(limit).min(rows.len());
        (rows[start..end].to_vec(), end)
    };
    let (served, next) = if data.empty_first_page {
        if offset == 0 {
            (Vec::new(), Some(limit))
        } else {
            let (served, end) = page(offset - limit.min(offset));
            (served, (end < rows.len()).then_some(end + limit))
        }
    } else if data.repeat_rows {
        // The first page for every offset; the cursor still advances.
        let (served, _) = page(0);
        let next = offset.saturating_add(limit);
        (served, (next < rows.len()).then_some(next))
    } else {
        let (served, end) = page(offset);
        (served, (end < rows.len()).then_some(end))
    };
    let next = if data.repeat_cursor && !rows.is_empty() {
        Some(limit)
    } else {
        next
    };
    body.insert(key.to_owned(), Value::Array(served));
    if let Some(next) = next {
        body.insert("next_offset".to_owned(), json!(next));
    } else if data.zero_next_offset {
        body.insert("next_offset".to_owned(), json!(0));
    }
    Planned::json(200, &Value::Object(body))
}

/// One skip/limit dashboard page of `rows` under `key` (`pagination {limit, skip,
/// total_items}`), with the dataset's knobs.
fn dash_page(data: &RuntimeData, request: &RecordedRequest, rows: &[Value], key: &str) -> Planned {
    let (Some(offset), Some(limit)) = (number(request, "offset", 0), number(request, "limit", 25))
    else {
        return Planned::json(400, &json!({"message": "invalid paging"}));
    };
    let served_limit = data.page_cap.map_or(limit, |cap| limit.min(cap));
    let skip = if data.ignore_offset { 0 } else { offset };
    let start = skip.min(rows.len());
    let end = skip.saturating_add(served_limit).min(rows.len());
    let mut pagination = Map::new();
    pagination.insert("limit".to_owned(), json!(limit));
    pagination.insert("skip".to_owned(), json!(offset));
    if !data.omit_totals {
        pagination.insert(
            "total_items".to_owned(),
            json!(data.total_override.unwrap_or(rows.len() as u64)),
        );
    }
    let mut body = Map::new();
    body.insert(key.to_owned(), Value::Array(rows[start..end].to_vec()));
    body.insert("pagination".to_owned(), Value::Object(pagination));
    Planned::json(200, &Value::Object(body))
}

fn string_field(body: &Map<String, Value>, key: &str) -> bool {
    body.get(key).is_some_and(Value::is_string)
}

/// The last path segment, percent-decoded.
fn last_segment(request: &RecordedRequest) -> String {
    crate::encoding::percent_decode(request.path.rsplit('/').next().unwrap_or_default())
}

/// A freshly issued key record (`ApiKeySchema`), registered in the dataset.
fn issue_key(data: &mut RuntimeData, tsg: &str, name: &str, cust_app: &str) -> Value {
    data.keys_issued += 1;
    let n = data.keys_issued;
    let mut key = json!({
        "api_key_id": fx::uuid(900 + n),
        "api_key_last8": format!("NEW8-{n:03}"),
        "api_key_name": name,
        "auth_code": fx::auth_code(900 + n),
        "tsg_id": tsg,
        "expiration": "2027-06-01T00:00:00Z",
        "revoked": false,
        "cust_app": cust_app,
        "created_by": "admin@example.com",
        "creation_ts": fx::AT,
    });
    data.api_keys.push(key.clone());
    if !data.omit_key_value {
        key["api_key"] = json!(fx::key_secret(tsg, &n.to_string()));
    }
    if data.omit_auth_code {
        key.as_object_mut().map(|object| object.remove("auth_code"));
    }
    key
}

/// The stateful write routes (step 2).
fn write(data: &mut RuntimeData, route: Rx, request: &RecordedRequest, tsg: &str) -> Planned {
    let body = request.body_json();
    match route {
        Rx::ApiKeyCreate => {
            let Some(Value::Object(body)) = body else {
                return Planned::json(400, &json!({"message": "invalid body"}));
            };
            let valid = [
                "auth_code",
                "cust_app",
                "created_by",
                "api_key_name",
                "rotation_time_unit",
            ]
            .iter()
            .all(|key| string_field(&body, key))
                && body.get("revoked").is_some_and(Value::is_boolean)
                && body
                    .get("rotation_time_interval")
                    .is_some_and(Value::is_number);
            if !valid {
                return Planned::json(400, &json!({"message": "missing required fields"}));
            }
            let name = body["api_key_name"].as_str().unwrap_or_default().to_owned();
            let app = body["cust_app"].as_str().unwrap_or_default().to_owned();
            Planned::json(200, &issue_key(data, tsg, &name, &app))
        }
        Rx::ApiKeyRegenerate => {
            let id = last_segment(request);
            let Some(Value::Object(body)) = body else {
                return Planned::json(400, &json!({"message": "invalid body"}));
            };
            if !body
                .get("rotation_time_interval")
                .is_some_and(Value::is_number)
                || !string_field(&body, "rotation_time_unit")
            {
                return Planned::json(400, &json!({"message": "invalid rotation"}));
            }
            let Some(old) = data
                .api_keys
                .iter_mut()
                .find(|key| key.get("api_key_id").and_then(Value::as_str) == Some(id.as_str()))
            else {
                return Planned::json(404, &json!({"message": "api key not found"}));
            };
            old["revoked"] = json!(true);
            let name = old["api_key_name"].as_str().unwrap_or_default().to_owned();
            let app = old["cust_app"].as_str().unwrap_or_default().to_owned();
            Planned::json(200, &issue_key(data, tsg, &name, &app))
        }
        Rx::ApiKeyDelete => {
            let name = last_segment(request);
            if request
                .query_value("updated_by")
                .is_none_or(|value| value.trim().is_empty())
            {
                return Planned::json(400, &json!({"message": "updated_by is required"}));
            }
            let before = data.api_keys.len();
            data.api_keys.retain(|key| {
                key.get("api_key_name").and_then(Value::as_str) != Some(name.as_str())
            });
            if data.api_keys.len() == before {
                return Planned::json(404, &json!({"message": "api key not found"}));
            }
            let answer = data
                .key_delete_answer
                .clone()
                .unwrap_or_else(|| json!({"message": format!("api key {name} deleted")}));
            Planned::json(200, &answer)
        }
        Rx::CustomerAppUpdate => {
            let id = request
                .query_value("customer_app_id")
                .unwrap_or_default()
                .to_owned();
            let Some(Value::Object(body)) = body else {
                return Planned::json(400, &json!({"message": "invalid body"}));
            };
            if !["app_name", "cloud_provider", "environment"]
                .iter()
                .all(|key| string_field(&body, key))
            {
                return Planned::json(400, &json!({"message": "missing required fields"}));
            }
            let Some(row) = data
                .customer_apps
                .iter_mut()
                .find(|app| app.get("customer_appId").and_then(Value::as_str) == Some(id.as_str()))
            else {
                return Planned::json(404, &json!({"message": "customer app not found"}));
            };
            let keys = row.get("api_keys_dp_info").cloned();
            let mut record = Map::new();
            record.insert("customer_appId".to_owned(), json!(id));
            record.insert("tsg_id".to_owned(), json!(tsg));
            for (key, value) in body {
                if key != "customer_appId" && key != "tsg_id" {
                    record.insert(key, value);
                }
            }
            let answer = Value::Object(record.clone());
            if let Some(keys) = keys {
                record.insert("api_keys_dp_info".to_owned(), keys);
            }
            *row = Value::Object(record);
            Planned::json(200, &answer)
        }
        Rx::CustomerAppDelete => {
            let name = request
                .query_value("app_name")
                .unwrap_or_default()
                .to_owned();
            if request
                .query_value("updated_by")
                .is_none_or(|value| value.trim().is_empty())
            {
                return Planned::json(400, &json!({"message": "updated_by is required"}));
            }
            let before = data.customer_apps.len();
            data.customer_apps
                .retain(|app| app.get("app_name").and_then(Value::as_str) != Some(name.as_str()));
            if data.customer_apps.len() == before {
                return Planned::json(404, &json!({"message": "customer app not found"}));
            }
            data.api_keys
                .retain(|key| key.get("cust_app").and_then(Value::as_str) != Some(name.as_str()));
            let answer = data.app_delete_answer.clone().unwrap_or_else(
                || json!({"message": "customer app and associated keys successfully deleted"}),
            );
            Planned::json(200, &answer)
        }
        _ => Planned::empty_json(404),
    }
}

/// The answer of a runtime extras route.
pub(crate) fn answer(data: &mut RuntimeData, route: Rx, request: &RecordedRequest) -> Planned {
    if request.header("x-pan-token").is_some() {
        return Planned::empty_json(403);
    }
    let AuthIdentity::Bearer { tsg: Some(tsg) } = &request.auth else {
        return Planned::empty_json(401);
    };
    if route.is_dashboard() {
        if request.header("x-tsg-id") != Some(tsg.as_str()) {
            return Planned::json(
                403,
                &json!({"message": "x-tsg-id does not match the token"}),
            );
        }
    } else if request.header("x-tsg-id").is_some() {
        return Planned::json(400, &json!({"message": "unexpected x-tsg-id"}));
    }
    if let Some(path_tsg) = &request.path_tsg
        && path_tsg != tsg
    {
        return Planned::empty_json(403);
    }
    let pair = || {
        format!(
            "{}|{}",
            request.query_value("appid").unwrap_or_default(),
            request.query_value("appname").unwrap_or_default()
        )
    };
    if route.is_write() && route != Rx::ScanLogs {
        let tsg = tsg.clone();
        return write(data, route, request, &tsg);
    }
    match route {
        Rx::ApiKeys => cursor_page(data, request, &data.api_keys, "api_keys"),
        Rx::CustomerApps => cursor_page(data, request, &data.customer_apps, "customer_apps"),
        Rx::CustomerApp => {
            let name = request.query_value("app_name").unwrap_or_default();
            data.customer_app_records
                .get(name)
                .cloned()
                .or_else(|| {
                    data.customer_apps
                        .iter()
                        .find(|row| row.get("app_name").and_then(Value::as_str) == Some(name))
                        .cloned()
                })
                .map_or_else(
                    || Planned::json(404, &json!({"message": "customer app not found"})),
                    |row| Planned::json(200, &row),
                )
        }
        Rx::DeploymentProfiles => {
            let mut rows = data.deployment_profiles.clone();
            if request.query_value("unactivated") == Some("true") {
                rows.extend(data.unactivated_profiles.iter().cloned());
            }
            Planned::json(
                200,
                &json!({"deployment_profiles": rows, "status": "success"}),
            )
        }
        Rx::ScanLogs => Planned::json(500, &json!({"message": "scan-logs trap reached"})),
        Rx::AppsOverview => dash_page(data, request, &data.apps, "items"),
        Rx::Application => {
            let key = pair();
            let answer = data.applications.get(&key).cloned().unwrap_or_else(|| {
                let (id, name) = key.split_once('|').unwrap_or_default();
                fx::application(id, name)
            });
            Planned::json(200, &answer)
        }
        Rx::ApplicationViolations => data.breakdowns.get(&pair()).cloned().map_or_else(
            || Planned::json(200, &fx::breakdown()),
            |answer| Planned::json(200, &answer),
        ),
        Rx::TopApplications => Planned::json(200, &fx::top_applications()),
        Rx::ViolationsTrend => Planned::json(200, &fx::violations_trend()),
        Rx::AppsList => Planned::json(200, &fx::apps_list()),
        Rx::SessionsChart => Planned::json(200, &fx::sessions_chart()),
        Rx::SessionsOverview => dash_page(data, request, &data.sessions, "items"),
        Rx::Session => {
            let key = (
                request.query_value("app_id").unwrap_or_default().to_owned(),
                request
                    .query_value("app_name")
                    .unwrap_or_default()
                    .to_owned(),
                request
                    .query_value("session_id")
                    .unwrap_or_default()
                    .to_owned(),
            );
            let value = data
                .session_records
                .get(&key)
                .cloned()
                .unwrap_or_else(|| fx::session(&key.2));
            Planned::json(200, &value)
        }
        Rx::SessionTransaction => Planned::json(
            200,
            &fx::transaction(
                request.query_value("session_id").unwrap_or_default(),
                request.query_value("scan_id").unwrap_or_default(),
                request
                    .query_value("scan_sub_req_id")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0),
            ),
        ),
        Rx::ApiKeyCreate
        | Rx::ApiKeyRegenerate
        | Rx::ApiKeyDelete
        | Rx::CustomerAppUpdate
        | Rx::CustomerAppDelete => Planned::empty_json(404),
        Rx::ScanContent => Planned::json(
            200,
            &fx::scan_content(
                tsg,
                request.query_value("scan_id").unwrap_or_default(),
                request
                    .query_value("scan_sub_req_id")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0),
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_the_runtime_routes() {
        for (method, path, route) in [
            ("GET", "/aisec/v1/mgmt/apikeys/tsg/100", Some(Rx::ApiKeys)),
            (
                "GET",
                "/aisec/v1/mgmt/customerapp/tsg/100",
                Some(Rx::CustomerApps),
            ),
            ("GET", "/aisec/v1/mgmt/customerapp", Some(Rx::CustomerApp)),
            (
                "GET",
                "/aisec/v1/mgmt/deploymentprofiles",
                Some(Rx::DeploymentProfiles),
            ),
            ("POST", "/aisec/v1/mgmt/scanlogs", Some(Rx::ScanLogs)),
            ("GET", "/aisec/v1/mgmt/apikeys/tsg/", None),
            ("POST", "/aisec/v1/mgmt/apikey", Some(Rx::ApiKeyCreate)),
            (
                "POST",
                "/aisec/v1/mgmt/apikey/regenerate/a%2Fb",
                Some(Rx::ApiKeyRegenerate),
            ),
            ("POST", "/aisec/v1/mgmt/apikey/", None),
            (
                "DELETE",
                "/aisec/v1/mgmt/apikey/delete/key-1",
                Some(Rx::ApiKeyDelete),
            ),
            ("DELETE", "/aisec/v1/mgmt/apikey/delete/", None),
            (
                "PUT",
                "/aisec/v1/mgmt/customerapp",
                Some(Rx::CustomerAppUpdate),
            ),
            (
                "DELETE",
                "/aisec/v1/mgmt/customerapp",
                Some(Rx::CustomerAppDelete),
            ),
            ("GET", "/aisec/v1/mgmt/profiles/tsg/100", None),
            (
                "GET",
                "/dashboard/v1/mgmt/dashboard/v2/apps/applicationsoverview",
                Some(Rx::AppsOverview),
            ),
            (
                "GET",
                "/dashboard/v1/mgmt/dashboard/v2/sessions/sessiontransaction",
                Some(Rx::SessionTransaction),
            ),
            (
                "GET",
                "/dashboard/v1/mgmt/reports/scancontent",
                Some(Rx::ScanContent),
            ),
            ("POST", "/dashboard/v1/mgmt/reports/scancontent", None),
            ("GET", "/aisec/v1/mgmt/dashboard/v2/apps/appslist", None),
        ] {
            assert_eq!(
                Rx::classify(method, path).map(|(route, _)| route),
                route,
                "{method} {path}"
            );
        }
    }
}
