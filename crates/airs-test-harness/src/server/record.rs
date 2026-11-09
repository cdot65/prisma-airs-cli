//! Recorded requests, route classification and client identity.

use std::fmt;

use serde_json::Value;

use super::dlp::Dl;
use super::gateway::Gw;
use super::modelsec::Ms;
use super::redteam::Rt;
use super::runtime::Rx;
use super::supplychain::Sc;
use crate::encoding::form_decode;

/// The fixture route a request was classified as (method and path).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Route {
    /// `POST /oauth/token`.
    Token,
    /// `GET /aisec/v1/mgmt/profiles/tsg/{tsg}`.
    Profiles,
    /// `GET /aisec/v1/mgmt/topics/tsg/{tsg}`.
    Topics,
    /// `POST /scan/v1/scan/sync/request`.
    ScanSync,
    /// `GET /scan/v1/scan/results`.
    ScanResults,
    /// `POST /scan/v1/scan/async/request` (M5 bulk submissions).
    ScanAsync,
    /// `GET /scan/v1/scan/reports` (the M5 report fallback).
    ScanReports,
    /// `POST /aisec/v1/mgmt/topic` (M4 write).
    TopicCreate,
    /// `PUT /aisec/v1/mgmt/topic/uuid/{id}`.
    TopicUpdate,
    /// `DELETE /aisec/v1/mgmt/topic/{id}`.
    TopicDelete,
    /// `DELETE /aisec/v1/mgmt/topic/{id}/force?updated_by=…`.
    TopicForceDelete,
    /// `POST /aisec/v1/mgmt/profile`.
    ProfileCreate,
    /// `PUT /aisec/v1/mgmt/profile/uuid/{id}`.
    ProfileUpdate,
    /// `DELETE /aisec/v1/mgmt/profile/{id}`.
    ProfileDelete,
    /// `DELETE /aisec/v1/mgmt/profile/{id}/force?updated_by=…`.
    ProfileForceDelete,
    /// Anything under `/env-trap` (the TypeScript endpoint-variable trap, DESIGN §10
    /// M4); answered with `404 {}`.
    EnvTrap,
    /// A red team route under `/redteam/data`, `/redteam/mgmt` or `/redteam/broker`
    /// (DESIGN §10 M6a).
    RedTeam(Rt),
    /// A DLP route under `/dlp/v2/api/` (DESIGN §10 M6b).
    Dlp(Dl),
    /// A model-security route under `/aims/mgmt/v1/` or `/aims/data/v1/` (DESIGN §10
    /// M6c).
    ModelSecurity(Ms),
    /// A runtime extras route under `/aisec/v1/mgmt/` (API keys, customer apps, deployment
    /// profiles) or `/dashboard/v1/mgmt/` (dashboard and sessions; DESIGN §10 M6d).
    Runtime(Rx),
    /// A gateway route under `/ai_gw/v2`, `/ai_gw/admin/v2` or `/iam` (DESIGN §10 M6e),
    /// the data-plane workspace list the TS doctor probes included.
    Gateway(Gw),
    /// A supplychain route under `/agentguard/data` or `/agentguard/mgmt` (DESIGN §10
    /// M6f).
    Supplychain(Sc),
    /// `GET /uv-release/<file>` (no tenant prefix, no credential): the fake uv archive of
    /// the `--fetch-uv` tests (DESIGN §10 M6c).
    UvRelease,
    /// Anything else; answered with `404 {}`.
    Unknown,
}

impl Route {
    /// Every route outside the nested families ([`Route::RedTeam`], [`Route::Dlp`],
    /// [`Route::ModelSecurity`], [`Route::Runtime`], [`Route::Gateway`],
    /// [`Route::Supplychain`]), in declaration order.
    pub const ALL: [Route; 18] = [
        Route::Token,
        Route::Profiles,
        Route::Topics,
        Route::ScanSync,
        Route::ScanResults,
        Route::ScanAsync,
        Route::ScanReports,
        Route::TopicCreate,
        Route::TopicUpdate,
        Route::TopicDelete,
        Route::TopicForceDelete,
        Route::ProfileCreate,
        Route::ProfileUpdate,
        Route::ProfileDelete,
        Route::ProfileForceDelete,
        Route::EnvTrap,
        Route::UvRelease,
        Route::Unknown,
    ];

    /// A short stable label (`token`, `profiles`, ...).
    pub fn label(self) -> &'static str {
        match self {
            Route::Token => "token",
            Route::Profiles => "profiles",
            Route::Topics => "topics",
            Route::ScanSync => "scan-sync",
            Route::ScanResults => "scan-results",
            Route::ScanAsync => "scan-async",
            Route::ScanReports => "scan-reports",
            Route::TopicCreate => "topic-create",
            Route::TopicUpdate => "topic-update",
            Route::TopicDelete => "topic-delete",
            Route::TopicForceDelete => "topic-force-delete",
            Route::ProfileCreate => "profile-create",
            Route::ProfileUpdate => "profile-update",
            Route::ProfileDelete => "profile-delete",
            Route::ProfileForceDelete => "profile-force-delete",
            Route::EnvTrap => "env-trap",
            Route::RedTeam(rt) => rt.label(),
            Route::Dlp(dl) => dl.label(),
            Route::ModelSecurity(ms) => ms.label(),
            Route::Runtime(rx) => rx.label(),
            Route::Gateway(gw) => gw.label(),
            Route::Supplychain(sc) => sc.label(),
            Route::UvRelease => "uv-release",
            Route::Unknown => "unknown",
        }
    }

    /// Classifies a request; returns the route and the `{tsg}` path segment when the
    /// route carries one. Anything under `/env-trap` is
    /// [`Route::EnvTrap`].
    pub fn classify(method: &str, path: &str) -> (Route, Option<String>) {
        if path == paths::ENV_TRAP || path.starts_with(&format!("{}/", paths::ENV_TRAP)) {
            return (Route::EnvTrap, None);
        }
        if let Some(rt) = Rt::classify(method, path) {
            return (Route::RedTeam(rt), None);
        }
        if let Some(dl) = Dl::classify(method, path) {
            return (Route::Dlp(dl), None);
        }
        if let Some(ms) = Ms::classify(method, path) {
            return (Route::ModelSecurity(ms), None);
        }
        if let Some((rx, tsg)) = Rx::classify(method, path) {
            return (Route::Runtime(rx), tsg);
        }
        if let Some(gw) = Gw::classify(method, path) {
            return (Route::Gateway(gw), None);
        }
        if let Some(sc) = Sc::classify(method, path) {
            return (Route::Supplychain(sc), None);
        }
        if method == "GET"
            && path
                .strip_prefix(paths::UV_RELEASE_PREFIX)
                .is_some_and(|file| !file.is_empty() && !file.contains('/'))
        {
            return (Route::UvRelease, None);
        }
        let tsg_route = |prefix: &str| {
            path.strip_prefix(prefix)
                .filter(|tsg| !tsg.is_empty() && !tsg.contains('/'))
                .map(str::to_owned)
        };
        let id_route = |prefix: &str, suffix: &str| {
            path.strip_prefix(prefix)
                .and_then(|rest| rest.strip_suffix(suffix))
                .is_some_and(|id| !id.is_empty() && !id.contains('/'))
        };
        let route = match method {
            "POST" if path == paths::TOKEN => Route::Token,
            "POST" if path == paths::SCAN_SYNC => Route::ScanSync,
            "POST" if path == paths::SCAN_ASYNC => Route::ScanAsync,
            "GET" if path == paths::SCAN_REPORTS => Route::ScanReports,
            "POST" if path == paths::TOPIC => Route::TopicCreate,
            "POST" if path == paths::PROFILE => Route::ProfileCreate,
            "GET" if path == paths::SCAN_RESULTS => Route::ScanResults,
            "GET" => {
                if let Some(tsg) = tsg_route(paths::PROFILES_PREFIX) {
                    return (Route::Profiles, Some(tsg));
                } else if let Some(tsg) = tsg_route(paths::TOPICS_PREFIX) {
                    return (Route::Topics, Some(tsg));
                }
                Route::Unknown
            }
            "PUT" if id_route(paths::TOPIC_UUID_PREFIX, "") => Route::TopicUpdate,
            "PUT" if id_route(paths::PROFILE_UUID_PREFIX, "") => Route::ProfileUpdate,
            "DELETE" if id_route(paths::TOPIC_PREFIX, "/force") => Route::TopicForceDelete,
            "DELETE" if id_route(paths::TOPIC_PREFIX, "") => Route::TopicDelete,
            "DELETE" if id_route(paths::PROFILE_PREFIX, "/force") => Route::ProfileForceDelete,
            "DELETE" if id_route(paths::PROFILE_PREFIX, "") => Route::ProfileDelete,
            _ => Route::Unknown,
        };
        (route, None)
    }

    /// The base a request path was served under: `""` (the tenant base) or `/env-trap` (the trap).
    pub fn base_of(path: &str) -> &'static str {
        if path == paths::ENV_TRAP || path.starts_with(&format!("{}/", paths::ENV_TRAP)) {
            paths::ENV_TRAP
        } else {
            ""
        }
    }

    /// Whether this is a write route: the M4 management writes and every red team
    /// write (`Rt::is_write`).
    pub fn is_write(self) -> bool {
        if let Route::RedTeam(rt) = self {
            return rt.is_write();
        }
        if let Route::Dlp(dl) = self {
            return dl.is_write();
        }
        if let Route::ModelSecurity(ms) = self {
            return ms.is_write();
        }
        if let Route::Runtime(rx) = self {
            return rx.is_write();
        }
        if let Route::Gateway(gw) = self {
            return gw.is_write();
        }
        matches!(
            self,
            Route::TopicCreate
                | Route::TopicUpdate
                | Route::TopicDelete
                | Route::TopicForceDelete
                | Route::ProfileCreate
                | Route::ProfileUpdate
                | Route::ProfileDelete
                | Route::ProfileForceDelete
        )
    }
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Fixture paths and the endpoint prefixes the fixture tenant config uses.
pub mod paths {
    /// Token endpoint path (`mgmtTokenEndpoint = <base>/oauth/token`).
    pub const TOKEN: &str = "/oauth/token";
    /// Management API prefix (`mgmtEndpoint = <base>/aisec`).
    pub const MGMT_PREFIX: &str = "/aisec";
    /// Scanner API prefix (`airsApiEndpoint = <base>/scan`).
    pub const SCAN_PREFIX: &str = "/scan";
    /// AI Gateway data plane prefix (`aiGwDataEndpoint = <base>/ai_gw/v2`).
    pub const AIGW_DATA_PREFIX: &str = "/ai_gw/v2";
    /// Profiles route prefix; the TSG follows.
    pub const PROFILES_PREFIX: &str = "/aisec/v1/mgmt/profiles/tsg/";
    /// Topics route prefix; the TSG follows.
    pub const TOPICS_PREFIX: &str = "/aisec/v1/mgmt/topics/tsg/";
    /// Sync scan path.
    pub const SCAN_SYNC: &str = "/scan/v1/scan/sync/request";
    /// Scan results path.
    pub const SCAN_RESULTS: &str = "/scan/v1/scan/results";
    /// Async scan submission path.
    pub const SCAN_ASYNC: &str = "/scan/v1/scan/async/request";
    /// Threat report path (the bulk-scan report fallback).
    pub const SCAN_REPORTS: &str = "/scan/v1/scan/reports";
    /// AI Gateway workspaces list path.
    pub const AIGW_WORKSPACES: &str = "/ai_gw/v2/workspaces";
    /// Topic create path (`POST`).
    pub const TOPIC: &str = "/aisec/v1/mgmt/topic";
    /// Topic delete prefix (`DELETE <prefix><id>[/force]`).
    pub const TOPIC_PREFIX: &str = "/aisec/v1/mgmt/topic/";
    /// Topic update prefix (`PUT <prefix><id>`).
    pub const TOPIC_UUID_PREFIX: &str = "/aisec/v1/mgmt/topic/uuid/";
    /// Profile create path (`POST`).
    pub const PROFILE: &str = "/aisec/v1/mgmt/profile";
    /// Profile delete prefix (`DELETE <prefix><id>[/force]`).
    pub const PROFILE_PREFIX: &str = "/aisec/v1/mgmt/profile/";
    /// Profile update prefix (`PUT <prefix><id>`).
    pub const PROFILE_UUID_PREFIX: &str = "/aisec/v1/mgmt/profile/uuid/";
    /// A trap that records anything sent to it: tests point ignored variables here.
    pub const ENV_TRAP: &str = "/env-trap";
    /// The fake uv release (`GET /uv-release/uv-<target>.tar.gz`).
    pub const UV_RELEASE_PREFIX: &str = "/uv-release/";
}

/// Who a request authenticated as, derived from its headers (and, for the token route,
/// its form body). Holds identifiers only, never secret values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthIdentity {
    /// No credential header.
    None,
    /// Token request with HTTP Basic client authentication.
    Basic {
        /// Decoded client id.
        client_id: String,
        /// TSG from `scope=tsg_id:<tsg>`.
        tsg: Option<String>,
        /// Credentials matched the registered tenant for that TSG.
        valid: bool,
    },
    /// Token request with `client_id`/`client_secret` form fields.
    Form {
        /// Form `client_id`.
        client_id: String,
        /// TSG from `scope=tsg_id:<tsg>`.
        tsg: Option<String>,
        /// Credentials matched the registered tenant for that TSG.
        valid: bool,
    },
    /// `Authorization: Bearer` that is not a registered scanner token: `tsg` is set when
    /// it is `mock-token-<tsg>` for a registered tenant.
    Bearer {
        /// Tenant the management token was issued for.
        tsg: Option<String>,
    },
    /// `x-pan-token` scanner key.
    ApiKey {
        /// TSG of the tenant that owns the key; `None` for an unregistered key.
        key_id: Option<String>,
        /// `x-payload-hash` is the HMAC of the exact body (or absent with no body).
        hash_valid: bool,
    },
    /// `Authorization: Bearer <airsApiToken>` scanner token.
    ApiToken {
        /// TSG of the tenant that owns the token.
        token_id: String,
    },
    /// Both scanner credentials (key and token) were sent.
    ApiKeyAndToken {
        /// TSG of the tenant that owns the key; `None` for an unregistered key.
        key_id: Option<String>,
        /// `x-payload-hash` validity, as for [`AuthIdentity::ApiKey`].
        hash_valid: bool,
        /// TSG of the tenant that owns the token; `None` for an unregistered token.
        token_id: Option<String>,
    },
    /// An `Authorization` header with another scheme, or a token request whose Basic
    /// header could not be decoded.
    Other,
}

/// How the halves of an HTTP Basic token-request credential were encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BasicEncoding {
    /// Both halves are unreserved text, so raw and form-encoded forms are identical.
    Indistinguishable,
    /// `base64(id:secret)` without encoding (the TS SDK).
    Raw,
    /// RFC 6749 §2.3.1 form-encoded halves (the Rust SDK).
    FormUrlEncoded,
    /// Neither form matched the registered credentials.
    Unmatched,
}

/// One request received by the fixture server.
#[derive(Clone)]
pub struct RecordedRequest {
    /// Arrival order, starting at 0.
    pub seq: usize,
    /// Connection the request arrived on, starting at 0 (keep-alive reuse is visible).
    pub connection: u64,
    /// Upper-case method.
    pub method: String,
    /// Path without the query.
    pub path: String,
    /// Raw query string (without `?`), when present.
    pub raw_query: Option<String>,
    /// Decoded query pairs in wire order.
    pub query: Vec<(String, String)>,
    /// Headers with lowercase names.
    pub headers: Vec<(String, String)>,
    /// Raw body bytes.
    pub body: Vec<u8>,
    /// Classified route.
    pub route: Route,
    /// The base it was served under ([`Route::base_of`]): `""` or `/env-trap`.
    pub base: &'static str,
    /// `{tsg}` path segment for management routes.
    pub path_tsg: Option<String>,
    /// Classified client identity.
    pub auth: AuthIdentity,
    /// For a Basic token request: which encoding of the credential halves matched.
    pub basic_encoding: Option<BasicEncoding>,
}

impl RecordedRequest {
    /// First value of a header (name compared case-insensitively).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// First value of a query parameter.
    pub fn query_value(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Body parsed as JSON, when it is JSON.
    pub fn body_json(&self) -> Option<Value> {
        serde_json::from_slice(&self.body).ok()
    }

    /// Body as (lossy) UTF-8 text.
    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Body decoded as `application/x-www-form-urlencoded` pairs.
    pub fn form(&self) -> Vec<(String, String)> {
        form_decode(&self.body_text())
    }

    /// `METHOD /path?query` (query included) for compact assertions.
    pub fn line(&self) -> String {
        match &self.raw_query {
            Some(query) => format!("{} {}?{}", self.method, self.path, query),
            None => format!("{} {}", self.method, self.path),
        }
    }
}

/// Headers whose values are credentials; Debug output prints them as `<redacted>`.
const SENSITIVE_HEADERS: &[&str] = &["authorization", "x-pan-token", "x-payload-hash", "cookie"];

impl fmt::Debug for RecordedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let headers: Vec<String> = self
            .headers
            .iter()
            .map(|(name, value)| {
                if SENSITIVE_HEADERS.contains(&name.as_str()) {
                    format!("{name}: <redacted>")
                } else {
                    format!("{name}: {value}")
                }
            })
            .collect();
        // Token bodies may carry a form client_secret; show only their field names.
        let body = if self.route == Route::Token {
            let names: Vec<String> = self.form().into_iter().map(|(name, _)| name).collect();
            format!("<form fields {names:?}>")
        } else {
            self.body_text()
        };
        f.debug_struct("RecordedRequest")
            .field("seq", &self.seq)
            .field("connection", &self.connection)
            .field("line", &self.line())
            .field("route", &self.route)
            .field("base", &self.base)
            .field("auth", &self.auth)
            .field("basic_encoding", &self.basic_encoding)
            .field("headers", &headers)
            .field("body", &body)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_routes() {
        assert_eq!(
            Route::classify("POST", "/oauth/token"),
            (Route::Token, None)
        );
        assert_eq!(
            Route::classify("GET", "/aisec/v1/mgmt/profiles/tsg/100"),
            (Route::Profiles, Some("100".into()))
        );
        assert_eq!(
            Route::classify("GET", "/aisec/v1/mgmt/topics/tsg/7"),
            (Route::Topics, Some("7".into()))
        );
        assert_eq!(
            Route::classify("GET", "/aisec/v1/mgmt/topics/tsg/7/x"),
            (Route::Unknown, None)
        );
        assert_eq!(
            Route::classify("GET", "/oauth/token"),
            (Route::Unknown, None)
        );
        assert_eq!(
            Route::classify("POST", "/scan/v1/scan/sync/request"),
            (Route::ScanSync, None)
        );
        assert_eq!(
            Route::classify("GET", "/scan/v1/scan/results"),
            (Route::ScanResults, None)
        );
        assert_eq!(
            Route::classify("POST", "/scan/v1/scan/async/request"),
            (Route::ScanAsync, None)
        );
        assert_eq!(
            Route::classify("GET", "/scan/v1/scan/reports"),
            (Route::ScanReports, None)
        );
        assert_eq!(
            Route::classify("GET", "/ai_gw/v2/workspaces"),
            (Route::Gateway(Gw::DataWorkspaces), None)
        );
        let id = "00000000-0000-4000-8000-000000000011";
        for (method, path, route) in [
            (
                "POST",
                "/aisec/v1/mgmt/topic".to_owned(),
                Route::TopicCreate,
            ),
            (
                "PUT",
                format!("/aisec/v1/mgmt/topic/uuid/{id}"),
                Route::TopicUpdate,
            ),
            (
                "DELETE",
                format!("/aisec/v1/mgmt/topic/{id}"),
                Route::TopicDelete,
            ),
            (
                "DELETE",
                format!("/aisec/v1/mgmt/topic/{id}/force"),
                Route::TopicForceDelete,
            ),
            (
                "POST",
                "/aisec/v1/mgmt/profile".to_owned(),
                Route::ProfileCreate,
            ),
            (
                "PUT",
                format!("/aisec/v1/mgmt/profile/uuid/{id}"),
                Route::ProfileUpdate,
            ),
            (
                "DELETE",
                format!("/aisec/v1/mgmt/profile/{id}"),
                Route::ProfileDelete,
            ),
            (
                "DELETE",
                format!("/aisec/v1/mgmt/profile/{id}/force"),
                Route::ProfileForceDelete,
            ),
            (
                "PUT",
                "/aisec/v1/mgmt/topic/uuid/".to_owned(),
                Route::Unknown,
            ),
            (
                "DELETE",
                format!("/aisec/v1/mgmt/topic/{id}/x"),
                Route::Unknown,
            ),
        ] {
            assert_eq!(
                Route::classify(method, &path),
                (route, None),
                "{method} {path}"
            );
            assert_eq!(Route::base_of(&path), "");
        }
        assert_eq!(
            Route::classify("GET", "/env-trap/v1/mgmt/topics/tsg/7"),
            (Route::EnvTrap, None)
        );
        assert_eq!(Route::base_of("/env-trap/v1/x"), "/env-trap");
    }
}
