//! Built-in route behaviour: credential checks, datasets and pagination.
//!
//! Everything here is synchronous and runs under the server state lock.

use std::borrow::Borrow;
use std::collections::{BTreeMap, HashMap, VecDeque};

use serde_json::{Map, Value, json};

use super::plan::Planned;
use super::record::{AuthIdentity, BasicEncoding, RecordedRequest, Route};
use crate::encoding::{base64_decode, form_decode, payload_hash, percent_decode_plus};
use crate::fixtures;

/// Prefix of every issued access token; the tenant TSG follows.
pub const TOKEN_PREFIX: &str = "mock-token-";

/// Credentials of one tenant known to the server.
#[derive(Clone)]
pub(crate) struct TenantCreds {
    pub tsg: String,
    pub client_id: String,
    pub client_secret: String,
    pub api_key: Option<String>,
    pub api_token: Option<String>,
}

/// Mutable server state, shared by the connection tasks and the test thread.
#[derive(Default)]
pub(crate) struct State {
    pub tenants: BTreeMap<String, TenantCreds>,
    pub profiles: HashMap<String, Vec<Value>>,
    pub topics: HashMap<String, Vec<Value>>,
    pub zero_next_offset_on_last_page: bool,
    /// `PUT …/uuid/{id}` appends a new record instead of replacing (see
    /// [`crate::FixtureServer::set_put_appends_new_record`]).
    pub put_appends: bool,
    /// Applied to the stored record (not the answer) of each `POST /v1/mgmt/profile` and
    /// `PUT …/profile/uuid/{id}` (see [`crate::FixtureServer::set_profile_write_edit`]).
    pub profile_write_edit: Option<fn(&mut Value)>,
    /// Records created by the write routes so far (their ids are
    /// `fixtures::uuid(900 + n)`).
    pub records_created: u64,
    pub rotating_tokens: bool,
    pub tokens_issued: u64,
    pub scan_rules: Vec<(String, Planned)>,
    /// Async scan behaviour per prompt marker (M5).
    pub async_rules: Vec<(String, AsyncRule)>,
    /// Receipts issued by the async route, in issue order.
    pub receipts: Vec<AsyncReceipt>,
    /// Emit result rows in reverse item order.
    pub async_reversed: bool,
    /// Held requests started and released so far.
    pub holds_started: u64,
    pub holds_released: u64,
    pub once: HashMap<Route, VecDeque<Planned>>,
    pub defaults: HashMap<Route, Planned>,
    pub log: Vec<RecordedRequest>,
    /// The red team dataset (DESIGN §10 M6a).
    pub redteam: super::redteam::RedTeamData,
    /// The DLP dataset (DESIGN §10 M6b).
    pub dlp: super::dlp::DlpData,
    /// The model-security dataset (DESIGN §10 M6c).
    pub modelsec: super::modelsec::ModelSecData,
    /// The runtime extras dataset (DESIGN §10 M6d).
    pub runtime: super::runtime::RuntimeData,
    /// The gateway dataset (DESIGN §10 M6e).
    pub gateway: super::gateway::GatewayData,
    /// The supplychain dataset (DESIGN §10 M6f).
    pub supplychain: super::supplychain::SupplychainData,
    /// The bytes `GET /uv-release/<file>` serves (the fake uv archive, M6c).
    pub uv_archive: Option<Vec<u8>>,
    pub disconnects: usize,
    pub connections: u64,
}

impl State {
    fn tenant_by_api_key(&self, key: &str) -> Option<&TenantCreds> {
        self.tenants
            .values()
            .find(|tenant| tenant.api_key.as_deref() == Some(key))
    }

    fn tenant_by_api_token(&self, token: &str) -> Option<&TenantCreds> {
        self.tenants
            .values()
            .find(|tenant| tenant.api_token.as_deref() == Some(token))
    }

    /// TSG of a registered tenant when `token` is `mock-token-<tsg>` (or a rotating
    /// `mock-token-<tsg>.r<n>`).
    fn management_token_tsg(&self, token: &str) -> Option<String> {
        token
            .strip_prefix(TOKEN_PREFIX)
            .map(|rest| rest.split_once(".r").map_or(rest, |(tsg, _)| tsg))
            .filter(|tsg| self.tenants.contains_key(*tsg))
            .map(str::to_owned)
    }

    /// The response for a request: a one-shot override, else the route default, else
    /// the built-in behaviour.
    pub fn plan(&mut self, request: &RecordedRequest) -> Planned {
        let planned = match self
            .once
            .get_mut(&request.route)
            .and_then(VecDeque::pop_front)
        {
            Some(planned) => planned,
            None => match self.defaults.get(&request.route) {
                Some(planned) => planned.clone(),
                None => return self.builtin(request),
            },
        };
        if planned.builtin {
            let mut answer = self.builtin(request);
            answer.hold = planned.hold;
            answer.delay = planned.delay;
            answer.disconnect = planned.disconnect;
            return answer;
        }
        planned
    }

    fn builtin(&mut self, request: &RecordedRequest) -> Planned {
        match request.route {
            Route::Token => self.token(request),
            Route::Profiles | Route::Topics => self.management(request),
            Route::TopicCreate
            | Route::TopicUpdate
            | Route::TopicDelete
            | Route::TopicForceDelete
            | Route::ProfileCreate
            | Route::ProfileUpdate
            | Route::ProfileDelete
            | Route::ProfileForceDelete => self.write(request),
            Route::EnvTrap => Planned::empty_json(404),
            Route::ScanSync => scanner_rejection(request).unwrap_or_else(|| self.scan(request)),
            Route::ScanResults => {
                scanner_rejection(request).unwrap_or_else(|| self.async_results(request))
            }
            Route::ScanAsync => {
                scanner_rejection(request).unwrap_or_else(|| self.async_submit(request))
            }
            Route::ScanReports => {
                scanner_rejection(request).unwrap_or_else(|| self.async_reports(request))
            }
            Route::RedTeam(rt) => super::redteam::answer(&mut self.redteam, rt, request),
            Route::Dlp(dl) => super::dlp::answer(&mut self.dlp, dl, request),
            Route::ModelSecurity(ms) => super::modelsec::answer(&mut self.modelsec, ms, request),
            Route::Runtime(rx) => super::runtime::answer(&mut self.runtime, rx, request),
            Route::Gateway(gw) => super::gateway::answer(&mut self.gateway, gw, request),
            Route::Supplychain(sc) => {
                super::supplychain::answer(&mut self.supplychain, sc, request)
            }
            Route::UvRelease => match &self.uv_archive {
                Some(bytes) => Planned::raw(200, "application/gzip", bytes.clone()),
                None => Planned::empty_json(404),
            },
            Route::Unknown => Planned::empty_json(404),
        }
    }

    fn token(&mut self, request: &RecordedRequest) -> Planned {
        let form = request.form();
        let grant = form
            .iter()
            .find(|(name, _)| name == "grant_type")
            .map(|(_, value)| value.as_str());
        if grant != Some("client_credentials") {
            return Planned::json(400, &json!({"error": "unsupported_grant_type"}));
        }
        match &request.auth {
            AuthIdentity::Basic {
                tsg: Some(tsg),
                valid: true,
                ..
            }
            | AuthIdentity::Form {
                tsg: Some(tsg),
                valid: true,
                ..
            } => {
                self.tokens_issued += 1;
                let token = if self.rotating_tokens {
                    format!("{TOKEN_PREFIX}{tsg}.r{}", self.tokens_issued)
                } else {
                    format!("{TOKEN_PREFIX}{tsg}")
                };
                Planned::json(
                    200,
                    &json!({"access_token": token, "token_type": "Bearer", "expires_in": 3600}),
                )
            }
            _ => Planned::empty_json(401),
        }
    }

    fn management(&self, request: &RecordedRequest) -> Planned {
        if request.header("x-pan-token").is_some() {
            return Planned::empty_json(403);
        }
        let AuthIdentity::Bearer { tsg: Some(tsg) } = &request.auth else {
            return Planned::empty_json(401);
        };
        if request.path_tsg.as_deref() != Some(tsg.as_str()) {
            return Planned::empty_json(403);
        }
        let (Some(offset), Some(limit)) = (
            numeric_query(request, "offset", 0),
            numeric_query(request, "limit", 100),
        ) else {
            return Planned::empty_json(400);
        };
        // Borrowed rows; only the page that is served is cloned.
        fn all(rows: Option<&Vec<Value>>) -> Vec<&Value> {
            rows.map(|rows| rows.iter().collect()).unwrap_or_default()
        }
        let (rows, key) = if request.route == Route::Profiles {
            let rows = self.profiles.get(tsg);
            let rows = if request.query_value("latest") == Some("true") {
                latest_by(rows.map(Vec::as_slice).unwrap_or_default(), "profile_name")
            } else {
                all(rows)
            };
            (rows, "ai_profiles")
        } else {
            (all(self.topics.get(tsg)), "custom_topics")
        };
        Planned::json(
            200,
            &page(
                &rows,
                key,
                offset,
                limit,
                self.zero_next_offset_on_last_page,
            ),
        )
    }

    /// The stateful M4 write routes (DESIGN §10 M4): the tenant is the bearer's TSG.
    fn write(&mut self, request: &RecordedRequest) -> Planned {
        if request.header("x-pan-token").is_some() {
            return Planned::empty_json(403);
        }
        let AuthIdentity::Bearer { tsg: Some(tsg) } = &request.auth else {
            return Planned::empty_json(401);
        };
        let tsg = tsg.clone();
        let path = request
            .path
            .strip_prefix(request.base)
            .unwrap_or(&request.path);
        let id = path
            .rsplit('/')
            .find(|segment| !segment.is_empty() && *segment != "force")
            .unwrap_or_default()
            .to_owned();
        let force = matches!(
            request.route,
            Route::TopicForceDelete | Route::ProfileForceDelete
        );
        if force
            && request
                .query_value("updated_by")
                .is_none_or(|value| value.trim().is_empty())
        {
            return Planned::json(400, &json!({"message": "updated_by is required"}));
        }
        let body = request.body_json();
        match request.route {
            Route::TopicCreate | Route::ProfileCreate => {
                let Some(Value::Object(body)) = body else {
                    return Planned::empty_json(400);
                };
                let record = self.new_record(request.route == Route::TopicCreate, &body, 1);
                let mut stored = record.clone();
                if request.route == Route::ProfileCreate
                    && let Some(edit) = self.profile_write_edit
                {
                    edit(&mut stored);
                }
                self.rows_mut(request.route, &tsg).push(stored);
                Planned::json(200, &record)
            }
            Route::TopicUpdate | Route::ProfileUpdate => {
                let Some(Value::Object(body)) = body else {
                    return Planned::empty_json(400);
                };
                let topic = request.route == Route::TopicUpdate;
                let id_key = if topic { "topic_id" } else { "profile_id" };
                let appends = self.put_appends;
                let rows = self.rows_mut(request.route, &tsg);
                let Some(index) = rows
                    .iter()
                    .position(|row| row.get(id_key).and_then(Value::as_str) == Some(id.as_str()))
                else {
                    return Planned::json(404, &json!({"message": "not found"}));
                };
                let revision = rows[index]
                    .get("revision")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    + 1;
                if appends {
                    let mut merged = rows[index].clone();
                    overlay(&mut merged, &body);
                    let Value::Object(merged) = merged else {
                        return Planned::empty_json(500);
                    };
                    let record = self.new_record(topic, &merged, revision);
                    self.rows_mut(request.route, &tsg).push(record.clone());
                    Planned::json(200, &record)
                } else {
                    let edit = if topic { None } else { self.profile_write_edit };
                    let rows = self.rows_mut(request.route, &tsg);
                    let row = &mut rows[index];
                    overlay(row, &body);
                    row[id_key] = Value::String(id);
                    row["revision"] = json!(revision);
                    let answer = row.clone();
                    if let Some(edit) = edit {
                        edit(row);
                    }
                    Planned::json(200, &answer)
                }
            }
            Route::TopicDelete | Route::TopicForceDelete => {
                let rows = self.topics.entry(tsg.clone()).or_default();
                if !rows
                    .iter()
                    .any(|row| row.get("topic_id").and_then(Value::as_str) == Some(id.as_str()))
                {
                    return Planned::json(404, &json!({"message": "Topic not found"}));
                }
                let profiles = self.profiles.entry(tsg.clone()).or_default();
                if force {
                    for profile in profiles.iter_mut() {
                        strip_topic_refs(profile, &id);
                    }
                } else {
                    let referencing: Vec<Value> = profiles
                        .iter()
                        .filter(|profile| references_topic(profile, &id))
                        .map(|profile| {
                            json!({
                                "profile_id": profile.get("profile_id").cloned().unwrap_or(Value::Null),
                                "profile_name": profile.get("profile_name").cloned().unwrap_or(Value::Null),
                                "revision": profile.get("revision").cloned().unwrap_or(json!(0)),
                            })
                        })
                        .collect();
                    if !referencing.is_empty() {
                        return Planned::json(
                            409,
                            &json!({"message": "Topic is referenced by security profiles", "payload": referencing}),
                        );
                    }
                }
                self.topics
                    .entry(tsg)
                    .or_default()
                    .retain(|row| row.get("topic_id").and_then(Value::as_str) != Some(id.as_str()));
                Planned::json(200, &json!({"message": format!("Topic {id} deleted")}))
            }
            Route::ProfileDelete | Route::ProfileForceDelete => {
                let rows = self.profiles.entry(tsg).or_default();
                let before = rows.len();
                rows.retain(|row| {
                    row.get("profile_id").and_then(Value::as_str) != Some(id.as_str())
                });
                if rows.len() == before {
                    return Planned::json(404, &json!({"message": "Profile not found"}));
                }
                Planned::json(200, &json!({"message": format!("Profile {id} deleted")}))
            }
            _ => Planned::empty_json(404),
        }
    }

    fn rows_mut(&mut self, route: Route, tsg: &str) -> &mut Vec<Value> {
        let map = if matches!(
            route,
            Route::TopicCreate | Route::TopicUpdate | Route::TopicDelete | Route::TopicForceDelete
        ) {
            &mut self.topics
        } else {
            &mut self.profiles
        };
        map.entry(tsg.to_owned()).or_default()
    }

    /// A record created by a write route: the body's fields, a fresh id, `revision`, and
    /// the fixture's audit fields.
    fn new_record(&mut self, topic: bool, body: &Map<String, Value>, revision: u64) -> Value {
        self.records_created += 1;
        let id = fixtures::uuid(900 + self.records_created);
        let mut record = Value::Object(Map::new());
        if topic {
            record["topic_id"] = Value::String(id);
            record["topic_name"] = body.get("topic_name").cloned().unwrap_or(json!(""));
            record["revision"] = json!(revision);
            record["active"] = body.get("active").cloned().unwrap_or(json!(true));
            record["description"] = body.get("description").cloned().unwrap_or(json!(""));
            record["examples"] = body.get("examples").cloned().unwrap_or(json!([]));
            record["created_by"] = json!("creator@example.com");
            record["last_modified_ts"] = json!("2026-02-01T00:00:00Z");
            record["created_ts"] = json!("2026-01-01T00:00:00Z");
        } else {
            record["profile_id"] = Value::String(id);
            record["profile_name"] = body.get("profile_name").cloned().unwrap_or(json!(""));
            record["revision"] = json!(revision);
            record["active"] = body.get("active").cloned().unwrap_or(json!(true));
            record["created_by"] = json!("creator@example.com");
            record["updated_by"] = json!("updater@example.com");
            record["last_modified_ts"] = json!("2026-02-01T00:00:00Z");
            if let Some(policy) = body.get("policy") {
                record["policy"] = policy.clone();
            }
            // 8.0 (DESIGN §13.17): the service answers with the request's DLP tenant.
            if let Some(tenant) = body.get("dlp_tenant_id") {
                record["dlp_tenant_id"] = tenant.clone();
            }
        }
        record
    }

    fn scan(&self, request: &RecordedRequest) -> Planned {
        let Some(body) = request.body_json() else {
            return Planned::empty_json(400);
        };
        let texts = scan_texts(&body);
        if let Some((_, planned)) = self
            .scan_rules
            .iter()
            .find(|(marker, _)| texts.iter().any(|text| text.contains(marker.as_str())))
        {
            return planned.clone();
        }
        let profile = body
            .pointer("/ai_profile/profile_name")
            .and_then(Value::as_str)
            .unwrap_or("fixture-profile");
        Planned::json(200, &fixtures::scan_allow(profile))
    }

    /// `POST /v1/scan/async/request`: a receipt whose ids come from a per-server
    /// counter ([`fixtures::async_scan_id`]), so both CLIs get the same ids on fresh
    /// servers.
    fn async_submit(&mut self, request: &RecordedRequest) -> Planned {
        let Some(Value::Array(objects)) = request.body_json() else {
            return Planned::empty_json(400);
        };
        let mut items = Vec::new();
        for object in &objects {
            let (Some(req_id), Some(prompt)) = (
                object.get("req_id"),
                object
                    .pointer("/scan_req/contents/0/prompt")
                    .and_then(Value::as_str),
            ) else {
                return Planned::empty_json(400);
            };
            items.push((req_id.clone(), prompt.to_owned()));
        }
        let n = self.receipts.len() as u64 + 1;
        let receipt = AsyncReceipt {
            scan_id: fixtures::async_scan_id(n),
            report_id: fixtures::async_report_id(n),
            items,
            polls: 0,
        };
        let body = json!({
            "received": "2026-10-17T00:00:00Z",
            "scan_id": receipt.scan_id,
            "report_id": receipt.report_id,
        });
        self.receipts.push(receipt);
        Planned::json(200, &body)
    }

    /// The rule for `prompt` (first marker contained in it), else a plain allow.
    fn async_rule(&self, prompt: &str) -> AsyncRule {
        self.async_rules
            .iter()
            .find(|(marker, _)| prompt.contains(marker.as_str()))
            .map(|(_, rule)| rule.clone())
            .unwrap_or_default()
    }

    /// `GET /v1/scan/results?scan_ids=…`: one row per item of each known receipt.
    fn async_results(&mut self, request: &RecordedRequest) -> Planned {
        let ids: Vec<String> = request
            .query_value("scan_ids")
            .unwrap_or_default()
            .split(',')
            .map(str::to_owned)
            .collect();
        let mut rows = Vec::new();
        for id in ids {
            let Some(position) = self.receipts.iter().position(|r| r.scan_id == id) else {
                continue;
            };
            self.receipts[position].polls += 1;
            let receipt = self.receipts[position].clone();
            let mut receipt_rows: Vec<Value> = receipt
                .items
                .iter()
                .map(|(req_id, prompt)| {
                    let rule = self.async_rule(prompt);
                    if receipt.polls <= rule.pending {
                        return json!({"req_id": req_id, "status": "pending", "scan_id": receipt.scan_id});
                    }
                    match &rule.row {
                        AsyncRow::Verdict(verdict) => {
                            let mut result = verdict.clone();
                            result["scan_id"] = json!(receipt.scan_id);
                            result["report_id"] = json!(receipt.report_id);
                            json!({"req_id": req_id, "status": "complete", "scan_id": receipt.scan_id, "result": result})
                        }
                        AsyncRow::Failed => {
                            json!({"req_id": req_id, "status": "failed", "scan_id": receipt.scan_id})
                        }
                    }
                })
                .collect();
            if self.async_reversed {
                receipt_rows.reverse();
            }
            rows.extend(receipt_rows);
        }
        Planned::json(200, &Value::Array(rows))
    }

    /// `GET /v1/scan/reports?report_ids=…`: a detection-free report per item.
    fn async_reports(&self, request: &RecordedRequest) -> Planned {
        let ids: Vec<&str> = request
            .query_value("report_ids")
            .unwrap_or_default()
            .split(',')
            .collect();
        let reports: Vec<Value> = self
            .receipts
            .iter()
            .filter(|receipt| ids.contains(&receipt.report_id.as_str()))
            .flat_map(|receipt| {
                receipt.items.iter().map(move |(req_id, _)| {
                    json!({"report_id": receipt.report_id, "scan_id": receipt.scan_id, "req_id": req_id, "detection_results": []})
                })
            })
            .collect();
        Planned::json(200, &Value::Array(reports))
    }

    /// Classifies the client identity of a request (plus, for a Basic token request,
    /// which encoding of the credential halves matched).
    pub fn identify(
        &self,
        route: Route,
        headers: &[(String, String)],
        body: &[u8],
    ) -> (AuthIdentity, Option<BasicEncoding>) {
        let header = |name: &str| {
            headers
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        let authorization = header("authorization");
        if route == Route::Token {
            return self.identify_token_client(authorization, body);
        }
        (
            self.identify_api_client(
                authorization,
                header("x-pan-token"),
                header("x-payload-hash"),
                body,
            ),
            None,
        )
    }

    fn identify_api_client(
        &self,
        authorization: Option<&str>,
        api_key: Option<&str>,
        payload_hash_header: Option<&str>,
        body: &[u8],
    ) -> AuthIdentity {
        let bearer = authorization.and_then(|value| value.strip_prefix("Bearer "));
        if let Some(tsg) = bearer.and_then(|token| self.management_token_tsg(token)) {
            return AuthIdentity::Bearer { tsg: Some(tsg) };
        }
        let token_id = bearer
            .and_then(|token| self.tenant_by_api_token(token))
            .map(|tenant| tenant.tsg.clone());
        let key = api_key.map(|key| {
            let key_id = self.tenant_by_api_key(key).map(|tenant| tenant.tsg.clone());
            let hash_valid = match payload_hash_header {
                Some(hash) => hash == payload_hash(key, body),
                None => body.is_empty(),
            };
            (key_id, hash_valid)
        });
        match (key, bearer, token_id) {
            (Some((key_id, hash_valid)), Some(_), token_id) => AuthIdentity::ApiKeyAndToken {
                key_id,
                hash_valid,
                token_id,
            },
            (Some((key_id, hash_valid)), None, _) => AuthIdentity::ApiKey { key_id, hash_valid },
            (None, Some(_), Some(token_id)) => AuthIdentity::ApiToken { token_id },
            (None, Some(_), None) => AuthIdentity::Bearer { tsg: None },
            (None, None, _) if authorization.is_some() => AuthIdentity::Other,
            (None, None, _) => AuthIdentity::None,
        }
    }

    fn identify_token_client(
        &self,
        authorization: Option<&str>,
        body: &[u8],
    ) -> (AuthIdentity, Option<BasicEncoding>) {
        let form = form_decode(&String::from_utf8_lossy(body));
        let field = |name: &str| {
            form.iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        let tsg = field("scope")
            .and_then(|scope| scope.strip_prefix("tsg_id:"))
            .map(str::to_owned);
        let tenant = tsg.as_deref().and_then(|tsg| self.tenants.get(tsg));
        let matches = |id: &str, secret: &str| {
            tenant.is_some_and(|tenant| tenant.client_id == id && tenant.client_secret == secret)
        };
        if let Some(value) = authorization {
            let Some((id, secret)) = value.strip_prefix("Basic ").and_then(decode_basic) else {
                return (AuthIdentity::Other, None);
            };
            // TS sends the raw pair; RFC 6749 §2.3.1 (the Rust SDK) form-encodes each half.
            let (decoded_id, decoded_secret) =
                (percent_decode_plus(&id), percent_decode_plus(&secret));
            let raw_ok = matches(&id, &secret);
            let encoded_ok = matches(&decoded_id, &decoded_secret);
            let encoding = if decoded_id == id && decoded_secret == secret {
                BasicEncoding::Indistinguishable
            } else if raw_ok {
                BasicEncoding::Raw
            } else if encoded_ok {
                BasicEncoding::FormUrlEncoded
            } else {
                BasicEncoding::Unmatched
            };
            let client_id = if !raw_ok && encoded_ok {
                decoded_id
            } else {
                id
            };
            let identity = AuthIdentity::Basic {
                client_id,
                tsg,
                valid: raw_ok || encoded_ok,
            };
            return (identity, Some(encoding));
        }
        let identity = match field("client_id") {
            Some(id) => AuthIdentity::Form {
                client_id: id.to_owned(),
                valid: field("client_secret").is_some_and(|secret| matches(id, secret)),
                tsg,
            },
            None => AuthIdentity::None,
        };
        (identity, None)
    }
}

/// What the async routes answer for the prompts that contain a marker
/// ([`crate::FixtureServer::add_async_rule`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsyncRule {
    /// Polls answered `pending` before the row below.
    pub pending: u32,
    pub row: AsyncRow,
}

/// The terminal row of an async scan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AsyncRow {
    /// `complete` with this verdict as `result` (its `scan_id` and `report_id` are set
    /// to the receipt's).
    Verdict(Value),
    /// `failed`.
    Failed,
}

impl Default for AsyncRule {
    fn default() -> Self {
        Self::verdict(fixtures::scan_allow("fixture-profile"))
    }
}

impl AsyncRule {
    pub fn verdict(verdict: Value) -> Self {
        Self {
            pending: 0,
            row: AsyncRow::Verdict(verdict),
        }
    }

    pub fn failed() -> Self {
        Self {
            pending: 0,
            row: AsyncRow::Failed,
        }
    }

    /// Answer `pending` for the first `polls` polls.
    pub fn after_pending(mut self, polls: u32) -> Self {
        self.pending = polls;
        self
    }
}

/// One receipt the async route issued.
#[derive(Clone, Debug, PartialEq)]
pub struct AsyncReceipt {
    pub scan_id: String,
    pub report_id: String,
    /// `(req_id, prompt)` per submitted object, in body order.
    pub items: Vec<(Value, String)>,
    /// Result polls of this receipt so far.
    pub polls: u32,
}

/// Copies every field of `body` onto `row` (the PUT semantics of the fixture service:
/// fields replaced from the body).
fn overlay(row: &mut Value, body: &Map<String, Value>) {
    if let Value::Object(row) = row {
        for (key, value) in body {
            row.insert(key.clone(), value.clone());
        }
    }
}

/// The `topic-guardrails` topic refs of every `ai-security-profiles` element of a profile.
fn topic_refs_mut(profile: &mut Value) -> Vec<&mut Value> {
    let mut lists = Vec::new();
    let Some(Value::Array(elements)) = profile.pointer_mut("/policy/ai-security-profiles") else {
        return lists;
    };
    for element in elements {
        let Some(Value::Array(items)) =
            element.pointer_mut("/model-configuration/model-protection")
        else {
            continue;
        };
        for item in items {
            if item.get("name").and_then(Value::as_str) != Some("topic-guardrails") {
                continue;
            }
            if let Some(Value::Array(groups)) = item.get_mut("topic-list") {
                for group in groups {
                    if let Some(topics) = group.get_mut("topic") {
                        lists.push(topics);
                    }
                }
            }
        }
    }
    lists
}

/// Whether a profile record references topic `id` in its topic guardrails.
fn references_topic(profile: &Value, id: &str) -> bool {
    let mut copy = profile.clone();
    topic_refs_mut(&mut copy).into_iter().any(|refs| {
        refs.as_array().is_some_and(|refs| {
            refs.iter()
                .any(|r| r.get("topic_id").and_then(Value::as_str) == Some(id))
        })
    })
}

/// Removes every ref of topic `id` from a profile record (the force delete).
fn strip_topic_refs(profile: &mut Value, id: &str) {
    for refs in topic_refs_mut(profile) {
        if let Value::Array(refs) = refs {
            refs.retain(|r| r.get("topic_id").and_then(Value::as_str) != Some(id));
        }
    }
}

/// `401 {}` unless the request carries a valid scanner credential (a registered key with
/// a valid payload hash, or a registered scanner token); management bearers are refused.
fn scanner_rejection(request: &RecordedRequest) -> Option<Planned> {
    let accepted = match &request.auth {
        AuthIdentity::ApiKey {
            key_id: Some(_),
            hash_valid: true,
        }
        | AuthIdentity::ApiToken { .. } => true,
        AuthIdentity::ApiKeyAndToken {
            key_id,
            hash_valid,
            token_id,
        } => key_id.is_some() && *hash_valid && token_id.is_some(),
        _ => false,
    };
    (!accepted).then(|| Planned::empty_json(401))
}

fn decode_basic(encoded: &str) -> Option<(String, String)> {
    let decoded = String::from_utf8(base64_decode(encoded)?).ok()?;
    let (id, secret) = decoded.split_once(':')?;
    Some((id.to_owned(), secret.to_owned()))
}

/// A non-negative integer query value, `default` when absent, `None` when malformed.
fn numeric_query(request: &RecordedRequest, name: &str, default: usize) -> Option<usize> {
    match request.query_value(name) {
        None => Some(default),
        Some(value) => value.parse().ok(),
    }
}

/// Highest `revision` per `name_key`, keeping the first-seen position of each name and
/// the first row on ties.
fn latest_by<'a>(rows: &'a [Value], name_key: &str) -> Vec<&'a Value> {
    let revision = |value: &Value| value.get("revision").and_then(Value::as_f64).unwrap_or(0.0);
    // Linear in the rows (large datasets are paged with `latest=true` once per request).
    // Keyed by the name's JSON text, which tells apart exactly the values `==` does.
    let mut position: HashMap<Option<String>, usize> = HashMap::new();
    let mut latest: Vec<&Value> = Vec::new();
    for row in rows {
        let name = row.get(name_key).map(Value::to_string);
        match position.get(&name) {
            Some(&index) if revision(row) > revision(latest[index]) => latest[index] = row,
            Some(_) => {}
            None => {
                position.insert(name, latest.len());
                latest.push(row);
            }
        }
    }
    latest
}

/// One page: `{<key>: rows[offset..offset+limit], next_offset?}`. `next_offset` is set
/// only when rows remain, or to `0` on the last page with the quirk enabled.
fn page<T: Borrow<Value>>(
    rows: &[T],
    key: &str,
    offset: usize,
    limit: usize,
    zero_on_last: bool,
) -> Value {
    let start = offset.min(rows.len());
    let end = offset.saturating_add(limit).min(rows.len());
    let mut body = Map::new();
    let served = rows[start..end].iter().map(|row| row.borrow().clone());
    body.insert(key.to_owned(), Value::Array(served.collect()));
    if end < rows.len() {
        body.insert("next_offset".into(), json!(end));
    } else if zero_on_last {
        body.insert("next_offset".into(), json!(0));
    }
    Value::Object(body)
}

/// Every `prompt`/`response` string in a scan request's `contents`.
fn scan_texts(body: &Value) -> Vec<&str> {
    body.get("contents")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|content| ["prompt", "response"].map(|key| content.get(key)))
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_keeps_first_seen_order_and_first_tie() {
        let rows = vec![
            json!({"profile_name": "a", "revision": 1, "id": 1}),
            json!({"profile_name": "b", "revision": 1, "id": 2}),
            json!({"profile_name": "a", "revision": 3, "id": 3}),
            json!({"profile_name": "b", "revision": 1, "id": 4}),
        ];
        let ids: Vec<i64> = latest_by(&rows, "profile_name")
            .iter()
            .filter_map(|row| row["id"].as_i64())
            .collect();
        assert_eq!(ids, vec![3, 2]);
    }

    #[test]
    fn pages_set_next_offset_only_when_rows_remain() {
        let rows: Vec<Value> = (0..5).map(|n| json!(n)).collect();
        assert_eq!(
            page(&rows, "k", 0, 2, false),
            json!({"k": [0, 1], "next_offset": 2})
        );
        assert_eq!(page(&rows, "k", 4, 2, false), json!({"k": [4]}));
        assert_eq!(
            page(&rows, "k", 4, 2, true),
            json!({"k": [4], "next_offset": 0})
        );
        assert_eq!(page(&rows, "k", 9, 2, false), json!({"k": []}));
    }
}
