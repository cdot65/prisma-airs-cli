//! Two-token verification (contract §3.2–§3.4). Both tokens are verified on every
//! HTTP request, before the MCP layer sees it. Keys come only from pinned
//! configuration; nothing in a token or tool input selects a key origin.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use serde_json::{Map, Value};
use tokio::sync::Mutex;

use crate::config::{Assertion, Config, Grant, Permission, Upstream};

/// Bounded JWKS body (a real Entra key set is a few KiB).
const MAX_JWKS_BYTES: usize = 64 * 1024;
const JWKS_TTL: Duration = Duration::from_secs(600);
/// Unknown `kid` may force at most one refetch per this interval (rotation, not DoS).
const JWKS_MIN_REFETCH: Duration = Duration::from_secs(60);
const LEEWAY_SECONDS: u64 = 60;

/// Safe denial reasons. The HTTP layer maps every one to 401/403 with a fixed body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denial {
    MissingToken,
    MissingAssertion,
    MalformedToken,
    ForbiddenHeader,
    UnknownKey,
    KeySourceUnavailable,
    InvalidSignatureOrClaims,
    WrongTenant,
    WrongClient,
    MissingPermission,
    WrongOwner,
    WrongWorkspace,
    WrongOrganization,
    MissingIdentity,
    AssertionLifetime,
    BackendBinding,
    RouteBinding,
    NoGrant,
}

impl Denial {
    pub fn code(self) -> &'static str {
        match self {
            Self::MissingToken
            | Self::MissingAssertion
            | Self::MalformedToken
            | Self::ForbiddenHeader
            | Self::UnknownKey
            | Self::InvalidSignatureOrClaims
            | Self::AssertionLifetime => "unauthenticated",
            Self::KeySourceUnavailable => "upstream_unavailable",
            _ => "forbidden",
        }
    }
    pub fn reason(self) -> &'static str {
        match self {
            Self::MissingToken => "missing bearer token",
            Self::MissingAssertion => "missing identity assertion",
            Self::MalformedToken => "malformed token",
            Self::ForbiddenHeader => "token header names a key location",
            Self::UnknownKey => "token signed by an unknown key",
            Self::KeySourceUnavailable => "key source unavailable",
            Self::InvalidSignatureOrClaims => "token signature or registered claims rejected",
            Self::WrongTenant => "token tenant not accepted",
            Self::WrongClient => "gateway client not accepted",
            Self::MissingPermission => "token lacks an accepted permission",
            Self::WrongOwner => "integration owner not accepted",
            Self::WrongWorkspace => "workspace not accepted",
            Self::WrongOrganization => "organization not accepted",
            Self::MissingIdentity => "caller or agent identity missing",
            Self::AssertionLifetime => "identity assertion lifetime not accepted",
            Self::BackendBinding => "identity assertion is not bound to this backend",
            Self::RouteBinding => "request did not arrive through this backend's gateway route",
            Self::NoGrant => "caller and agent have no grant",
        }
    }
    pub fn http_status(self) -> u16 {
        match self.code() {
            "unauthenticated" => 401,
            "upstream_unavailable" => 503,
            _ => 403,
        }
    }
}

/// Where verification keys come from: a pinned JWKS URL or a fixed PEM key.
pub enum KeySource {
    Jwks(JwksCache),
    Pem(Box<DecodingKey>),
}

pub struct JwksCache {
    url: String,
    client: reqwest::Client,
    state: Mutex<JwksState>,
    ttl: Duration,
    min_refetch: Duration,
}

#[derive(Default)]
struct JwksState {
    keys: Option<(Instant, JwkSet)>,
    last_fetch: Option<Instant>,
}

impl JwksCache {
    pub fn new(url: &str, allow_loopback_http: bool) -> Result<Self, &'static str> {
        let parsed = url::Url::parse(url).map_err(|_| "jwks url")?;
        let loopback = matches!(
            parsed.host_str(),
            Some("127.0.0.1") | Some("[::1]") | Some("localhost")
        );
        match parsed.scheme() {
            "https" => {}
            "http" if allow_loopback_http && loopback => {}
            _ => return Err("jwks url must be https"),
        }
        if parsed.query().is_some() || parsed.fragment().is_some() || !parsed.username().is_empty()
        {
            return Err("jwks url must not carry query, fragment or credentials");
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .no_proxy()
            .build()
            .map_err(|_| "http client")?;
        Ok(Self {
            url: url.to_owned(),
            client,
            state: Mutex::new(JwksState::default()),
            ttl: JWKS_TTL,
            min_refetch: JWKS_MIN_REFETCH,
        })
    }

    async fn fetch(&self) -> Result<JwkSet, Denial> {
        let mut response = self
            .client
            .get(&self.url)
            .send()
            .await
            .map_err(|_| Denial::KeySourceUnavailable)?;
        if !response.status().is_success() {
            return Err(Denial::KeySourceUnavailable);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| Denial::KeySourceUnavailable)?
        {
            if body.len() + chunk.len() > MAX_JWKS_BYTES {
                return Err(Denial::KeySourceUnavailable);
            }
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&body).map_err(|_| Denial::KeySourceUnavailable)
    }

    /// Test hook: shorten the cache lifetime and refetch spacing.
    pub fn with_timing(mut self, ttl: Duration, min_refetch: Duration) -> Self {
        self.ttl = ttl;
        self.min_refetch = min_refetch;
        self
    }

    /// Resolve a key. Fetches are spaced by `min_refetch` regardless of cache
    /// freshness or outcome, so an outage or a stream of unknown `kid`s cannot
    /// cause more than one (5 s-bounded) fetch per window. A stale set keeps
    /// serving while refreshes fail.
    pub async fn key(&self, kid: &str) -> Result<DecodingKey, Denial> {
        let mut state = self.state.lock().await;
        let lookup = |set: &JwkSet| {
            set.find(kid)
                .and_then(|jwk| DecodingKey::from_jwk(jwk).ok())
        };
        let fresh = state
            .keys
            .as_ref()
            .is_some_and(|(at, _)| at.elapsed() < self.ttl);
        if fresh && let Some(key) = state.keys.as_ref().and_then(|(_, set)| lookup(set)) {
            return Ok(key);
        }
        let may_fetch = state
            .last_fetch
            .is_none_or(|t| t.elapsed() >= self.min_refetch);
        if may_fetch {
            state.last_fetch = Some(Instant::now());
            match self.fetch().await {
                Ok(set) => state.keys = Some((Instant::now(), set)),
                Err(e) if state.keys.is_none() => return Err(e),
                Err(_) => {}
            }
        } else if state.keys.is_none() {
            return Err(Denial::KeySourceUnavailable);
        }
        state
            .keys
            .as_ref()
            .and_then(|(_, set)| lookup(set))
            .ok_or(Denial::UnknownKey)
    }
}

/// The validated principal tuple (contract §3.4). Values are public identifiers.
#[derive(Debug, Clone)]
pub struct Principal {
    pub tenant_id: String,
    pub owner_object_id: String,
    pub subject: String,
    pub client_id: String,
    pub workspace: String,
    pub organization: String,
    pub grant: Grant,
    /// Tier carried by the upstream token's delegated scopes.
    pub token_tier: Permission,
    /// Allowlisted claim *shapes* (names → JSON types), never values.
    pub upstream_shape: BTreeMap<String, &'static str>,
    pub assertion_shape: BTreeMap<String, &'static str>,
    /// The raw assertion claims, kept in memory for the admin proof check only.
    pub assertion_claims: Map<String, Value>,
}

/// Session ID → owning principal key, plus reservations for requests that may
/// create a session. Capacity = `bound.len() + pending`, checked and reserved
/// atomically under one lock before the transport is reached.
#[derive(Debug, Default)]
pub struct SessionTable {
    pub bound: std::collections::HashMap<String, String>,
    pub pending: usize,
}

/// Upper bound on concurrently bound MCP sessions (fail closed beyond it).
pub const MAX_SESSIONS: usize = 10_000;

pub struct Verifier {
    pub config: Arc<Config>,
    /// Bound MCP sessions plus in-flight creation reservations.
    pub sessions: std::sync::Mutex<SessionTable>,
    upstream_keys: KeySource,
    assertion_keys: KeySource,
    /// SHA-256 of the route-binding secret; the secret itself is not retained.
    route_secret_digest: Option<[u8; 32]>,
}

/// Minimum route-binding secret length after trimming (256 bits of base64url is 43).
const MIN_ROUTE_SECRET: usize = 32;
const MAX_ROUTE_SECRET: usize = 1024;

fn digest(bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).into()
}

/// Constant-time comparison of two digests.
fn digest_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

const SHAPE_ALLOWLIST: &[&str] = &[
    "iss",
    "aud",
    "exp",
    "nbf",
    "iat",
    "ver",
    "tid",
    "oid",
    "azp",
    "appid",
    "azpacr",
    "scp",
    "roles",
    "idtyp",
    "sub",
    "client_id",
    "workspace_id",
    "organisation_id",
    "organization_id",
    "scope",
    "jti",
    "auth_time",
    "acr",
    "amr",
    "nonce",
    "sid",
    // Candidate caller-identity claim names (M0.6 discovery; names only, never values).
    "user_id",
    "email",
    "preferred_username",
    "name",
    "upn",
    "user",
];

fn json_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Public identifier values that diagnostics may log (`assertion_registered`,
/// `ungranted`): a string of at most 128 characters, otherwise only its kind.
/// Every control character (C0, DEL, C1) becomes U+FFFD, as the CLI's
/// `output::terminal_safe` does, because JSON escapes C0 but not DEL or C1.
/// Never applied to tokens or secrets, which are not logged at all.
pub fn bounded(v: Option<&Value>) -> Value {
    match v {
        None => Value::Null,
        Some(Value::String(s)) if s.chars().count() <= 128 => Value::String(
            s.chars()
                .map(|c| if c.is_control() { '\u{fffd}' } else { c })
                .collect(),
        ),
        Some(Value::String(s)) => Value::String(format!("<omitted: {} chars>", s.chars().count())),
        Some(other) => Value::String(format!("<{}>", json_type(other))),
    }
}

fn shape(claims: &Map<String, Value>, extra: &[&str]) -> BTreeMap<String, &'static str> {
    let mut out = BTreeMap::new();
    let mut others = 0usize;
    for (k, v) in claims {
        if SHAPE_ALLOWLIST.contains(&k.as_str()) || extra.contains(&k.as_str()) {
            out.insert(k.clone(), json_type(v));
        } else {
            others += 1;
        }
    }
    if others > 0 {
        out.insert(format!("<{others} other claims>"), "omitted");
    }
    out
}

fn algorithms(names: &[String]) -> Vec<Algorithm> {
    names.iter().filter_map(|n| n.parse().ok()).collect()
}

fn str_claim<'a>(claims: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    claims
        .get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// The frozen scope → tier mapping (contract §2.3).
pub fn scope_tier(scope: &str) -> Option<Permission> {
    match scope {
        "AirsHistory.Read" => Some(Permission::Reader),
        "AirsHistory.Operate" => Some(Permission::Operator),
        "AirsHistory.Admin" => Some(Permission::Administrator),
        _ => None,
    }
}

/// Highest tier among the token's accepted delegated scopes.
fn token_tier(claims: &Map<String, Value>, accepted: &[String]) -> Option<Permission> {
    permissions(claims)
        .iter()
        .filter(|p| accepted.contains(p))
        .filter_map(|p| scope_tier(p))
        .max()
}

fn permissions(claims: &Map<String, Value>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(scp) = claims.get("scp").and_then(Value::as_str) {
        out.extend(scp.split(' ').filter(|s| !s.is_empty()).map(str::to_owned));
    }
    out
}

impl Verifier {
    pub fn new(config: Arc<Config>) -> Result<Self, &'static str> {
        let loopback = config.server.allow_loopback_http_jwks;
        let upstream_keys = KeySource::Jwks(JwksCache::new(&config.upstream.jwks_url, loopback)?);
        let assertion_keys = match (
            &config.assertion.jwks_url,
            &config.assertion.public_key_pem_file,
        ) {
            (Some(url), None) => KeySource::Jwks(JwksCache::new(url, loopback)?),
            (None, Some(path)) => {
                let pem = std::fs::read(path).map_err(|_| "assertion public key unreadable")?;
                let key = DecodingKey::from_rsa_pem(&pem)
                    .or_else(|_| DecodingKey::from_ec_pem(&pem))
                    .or_else(|_| DecodingKey::from_ed_pem(&pem))
                    .map_err(|_| "assertion public key invalid")?;
                KeySource::Pem(Box::new(key))
            }
            _ => return Err("assertion key source"),
        };
        let route_secret_digest = match &config.route_binding {
            None => None,
            Some(rb) => {
                let raw = std::fs::read(&rb.secret_file)
                    .map_err(|_| "route binding secret unreadable")?;
                let text = std::str::from_utf8(&raw)
                    .map_err(|_| "route binding secret invalid")?
                    .trim();
                if !(MIN_ROUTE_SECRET..=MAX_ROUTE_SECRET).contains(&text.len())
                    || !text.bytes().all(|b| b.is_ascii_graphic())
                {
                    return Err("route binding secret invalid");
                }
                Some(digest(text.as_bytes()))
            }
        };
        Ok(Self {
            config,
            sessions: Default::default(),
            upstream_keys,
            assertion_keys,
            route_secret_digest,
        })
    }

    /// Verify the signature (pinned key, allowlisted algorithm) and `exp`/`nbf`.
    /// Issuer and audience are then checked explicitly so each denial is labeled.
    async fn decode_signed(
        &self,
        token: &str,
        keys: &KeySource,
        allowed: &[String],
        required: &[&str],
    ) -> Result<Map<String, Value>, Denial> {
        let header = decode_header(token).map_err(|_| Denial::MalformedToken)?;
        // Never let a token choose where its key comes from.
        if header.jku.is_some() || header.x5u.is_some() || header.jwk.is_some() {
            return Err(Denial::ForbiddenHeader);
        }
        let algs = algorithms(allowed);
        if !algs.contains(&header.alg) {
            return Err(Denial::InvalidSignatureOrClaims);
        }
        let key = match keys {
            KeySource::Jwks(cache) => {
                let kid = header.kid.as_deref().ok_or(Denial::UnknownKey)?;
                cache.key(kid).await?
            }
            KeySource::Pem(key) => (**key).clone(),
        };
        let mut validation = Validation::new(header.alg);
        validation.algorithms = algs;
        validation.leeway = LEEWAY_SECONDS;
        validation.validate_nbf = true;
        validation.validate_aud = false;
        validation.set_required_spec_claims(required);
        decode::<Map<String, Value>>(token, &key, &validation)
            .map(|d| d.claims)
            .map_err(|_| Denial::InvalidSignatureOrClaims)
    }

    /// Verify both tokens and resolve the grant. Called on every HTTP request.
    pub async fn verify(
        &self,
        bearer: Option<&str>,
        assertion: Option<&str>,
        route_binding: Option<&str>,
    ) -> Result<Principal, Rejection> {
        let mut seen = Rejection {
            denial: Denial::MissingToken,
            upstream_shape: None,
            assertion_shape: None,
            ungranted: None,
            assertion_registered: None,
        };
        match self
            .verify_inner(bearer, assertion, route_binding, &mut seen)
            .await
        {
            Ok(p) => Ok(p),
            Err(denial) => Err(Rejection { denial, ..seen }),
        }
    }

    async fn verify_inner(
        &self,
        bearer: Option<&str>,
        assertion: Option<&str>,
        route_binding: Option<&str>,
        seen: &mut Rejection,
    ) -> Result<Principal, Denial> {
        let c = &self.config;
        let bearer = bearer.ok_or(Denial::MissingToken)?;
        let assertion = assertion.ok_or(Denial::MissingAssertion)?;
        let names = &c.assertion.claims;
        let extra: Vec<&str> = [
            names.subject.as_str(),
            names.client_id.as_str(),
            names.workspace.as_str(),
            names.organization.as_str(),
        ]
        .into_iter()
        .chain(names.backend_binding.as_deref())
        .chain(c.admin.proof_nonce_claim.as_deref())
        .collect();

        let up = self
            .decode_signed(
                bearer,
                &self.upstream_keys,
                &c.upstream.algorithms,
                &["exp", "nbf"],
            )
            .await?;
        // Signature verified: shapes may be logged for M0.6 receipts even if denied.
        seen.upstream_shape = Some(shape(&up, &[]));
        check_upstream(&up, &c.upstream)?;
        let ia = self
            .decode_signed(
                assertion,
                &self.assertion_keys,
                &c.assertion.algorithms,
                &["exp", "iat"],
            )
            .await?;
        seen.assertion_shape = Some(shape(&ia, &extra));
        seen.assertion_registered = Some(Box::new(serde_json::json!({
            "iss": bounded(ia.get("iss")),
            "aud": bounded(ia.get("aud")),
            "workspace": bounded(ia.get(&names.workspace)),
            "organization": bounded(ia.get(&names.organization)),
        })));
        // Resolution 3 (contract §2.2): the gateway's per-integration route secret.
        if let Some(expected) = &self.route_secret_digest {
            match route_binding {
                Some(v) if digest_eq(&digest(v.trim().as_bytes()), expected) => {}
                _ => return Err(Denial::RouteBinding),
            }
        }
        check_assertion(&ia, &c.assertion)?;

        let subject = str_claim(&ia, &names.subject)
            .ok_or(Denial::MissingIdentity)?
            .to_owned();
        let client_id = str_claim(&ia, &names.client_id)
            .ok_or(Denial::MissingIdentity)?
            .to_owned();
        let workspace = str_claim(&ia, &names.workspace)
            .ok_or(Denial::WrongWorkspace)?
            .to_owned();
        if !c.assertion.workspace_ids.contains(&workspace) {
            return Err(Denial::WrongWorkspace);
        }
        let organization = str_claim(&ia, &names.organization)
            .ok_or(Denial::WrongOrganization)?
            .to_owned();
        if !c.assertion.organization_ids.contains(&organization) {
            return Err(Denial::WrongOrganization);
        }
        let tenant_id = str_claim(&up, "tid").ok_or(Denial::WrongTenant)?.to_owned();
        let owner = str_claim(&up, "oid").ok_or(Denial::WrongOwner)?.to_owned();
        // Grants are keyed by (assertion sub, agent client_id); the upstream oid is
        // only the integration-owner constraint recorded with the grant. Unknown
        // agents have no grant and are denied.
        let grant = c.grants.iter().find(|g| {
            g.subject == subject && g.client_id == client_id && g.owner_object_id == owner
        });
        let Some(grant) = grant else {
            seen.ungranted = Some(Box::new((subject, client_id)));
            return Err(Denial::NoGrant);
        };
        Ok(Principal {
            tenant_id,
            owner_object_id: owner,
            subject,
            client_id,
            workspace,
            organization,
            grant: grant.clone(),
            token_tier: token_tier(&up, &c.upstream.accepted_permissions)
                .ok_or(Denial::MissingPermission)?,
            upstream_shape: shape(&up, &[]),
            assertion_shape: shape(&ia, &extra),
            assertion_claims: ia,
        })
    }
}

impl Principal {
    /// The identity an MCP session is bound to: the full validated tuple. A
    /// refreshed token for the same identity yields the same key.
    pub fn session_key(&self) -> String {
        [
            &self.tenant_id,
            &self.owner_object_id,
            &self.subject,
            &self.client_id,
            &self.workspace,
            &self.organization,
        ]
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("\u{0}")
    }

    /// Effective tier = token scope tier ∩ grant tier (contract §2.3).
    pub fn effective_tier(&self) -> Permission {
        self.token_tier.min(self.grant.permission)
    }
}

/// A denial plus the claim shapes of whichever tokens passed signature checks.
#[derive(Debug, Clone)]
pub struct Rejection {
    pub denial: Denial,
    pub upstream_shape: Option<BTreeMap<String, &'static str>>,
    pub assertion_shape: Option<BTreeMap<String, &'static str>>,
    /// Set only for `NoGrant`, after both tokens fully verified: the public
    /// `(sub, client_id)` identifiers an operator needs to write the grant (M0.6).
    pub ungranted: Option<Box<(String, String)>>,
    /// The assertion's registered `iss`/`aud` values, recorded only after its
    /// signature verified. Public configuration identifiers, never credentials (M0.6).
    pub assertion_registered: Option<Box<Value>>,
}

fn check_upstream(claims: &Map<String, Value>, cfg: &Upstream) -> Result<(), Denial> {
    if str_claim(claims, "iss") != Some(cfg.issuer.as_str()) || !aud_matches(claims, &cfg.audiences)
    {
        return Err(Denial::InvalidSignatureOrClaims);
    }
    // Delegated access only: an app-only token (no `scp`, or `idtyp=app`) is refused.
    if str_claim(claims, "scp").is_none() || str_claim(claims, "idtyp") == Some("app") {
        return Err(Denial::MissingPermission);
    }
    if str_claim(claims, "tid") != Some(cfg.tenant_id.as_str()) {
        return Err(Denial::WrongTenant);
    }
    let client = str_claim(claims, "azp").or_else(|| str_claim(claims, "appid"));
    if !client.is_some_and(|id| cfg.allowed_client_ids.iter().any(|a| a == id)) {
        return Err(Denial::WrongClient);
    }
    if !permissions(claims)
        .iter()
        .any(|p| cfg.accepted_permissions.contains(p))
    {
        return Err(Denial::MissingPermission);
    }
    match str_claim(claims, "oid") {
        Some(oid) if cfg.owner_object_ids.iter().any(|o| o == oid) => Ok(()),
        _ => Err(Denial::WrongOwner),
    }
}

fn check_assertion(claims: &Map<String, Value>, cfg: &Assertion) -> Result<(), Denial> {
    if str_claim(claims, "iss") != Some(cfg.issuer.as_str()) {
        return Err(Denial::InvalidSignatureOrClaims);
    }
    // The assertion audience check. Under contract §2.2 resolution 1 this would be the
    // per-backend binding; the measured gateway signs the org ID, so the deployed binding
    // is the upstream token (resolution 3). Config validation permits an empty list only
    // under a route binding.
    if !cfg.audiences.is_empty() && !aud_matches(claims, &cfg.audiences) {
        return Err(Denial::BackendBinding);
    }
    let exp = claims
        .get("exp")
        .and_then(Value::as_u64)
        .ok_or(Denial::AssertionLifetime)?;
    let iat = claims
        .get("iat")
        .and_then(Value::as_u64)
        .ok_or(Denial::AssertionLifetime)?;
    if exp <= iat || exp - iat > cfg.max_lifetime_seconds {
        return Err(Denial::AssertionLifetime);
    }
    if let Some(name) = &cfg.claims.backend_binding {
        match str_claim(claims, name) {
            Some(v) if cfg.audiences.iter().any(|a| a == v) => {}
            _ => return Err(Denial::BackendBinding),
        }
    }
    Ok(())
}

fn aud_matches(claims: &Map<String, Value>, accepted: &[String]) -> bool {
    match claims.get("aud") {
        Some(Value::String(a)) => accepted.iter().any(|c| c == a),
        Some(Value::Array(list)) => {
            !list.is_empty()
                && list.iter().all(Value::is_string)
                && list
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|a| accepted.iter().any(|c| c == a))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    const JWKS: &str = include_str!("../tests/fixtures/entra-jwks.json");

    /// Loopback JWKS that counts fetches and can be switched to failing.
    async fn source(failing: bool) -> (String, Arc<AtomicUsize>, Arc<AtomicBool>) {
        let fetches = Arc::new(AtomicUsize::new(0));
        let fail = Arc::new(AtomicBool::new(failing));
        let (f, d) = (fetches.clone(), fail.clone());
        let app = axum::Router::new().route(
            "/keys",
            axum::routing::get(move || {
                f.fetch_add(1, Ordering::SeqCst);
                let down = d.load(Ordering::SeqCst);
                async move {
                    if down {
                        (axum::http::StatusCode::SERVICE_UNAVAILABLE, String::new())
                    } else {
                        (axum::http::StatusCode::OK, JWKS.to_owned())
                    }
                }
            }),
        );
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/keys", l.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        (url, fetches, fail)
    }

    #[tokio::test]
    async fn initial_outage_is_throttled_and_fails_closed() {
        let (url, fetches, _) = source(true).await;
        let cache = JwksCache::new(&url, true)
            .unwrap()
            .with_timing(Duration::from_secs(600), Duration::from_secs(60));
        for _ in 0..10 {
            assert_eq!(
                cache.key("entra-1").await.err(),
                Some(Denial::KeySourceUnavailable)
            );
        }
        assert_eq!(
            fetches.load(Ordering::SeqCst),
            1,
            "outage must not cause a fetch per request"
        );
    }

    #[tokio::test]
    async fn expired_cache_keeps_serving_stale_keys_while_the_source_is_down() {
        let (url, fetches, fail) = source(false).await;
        // TTL zero: every lookup is "stale"; refetch spacing 60 s.
        let cache = JwksCache::new(&url, true)
            .unwrap()
            .with_timing(Duration::ZERO, Duration::from_secs(60));
        assert!(cache.key("entra-1").await.is_ok());
        fail.store(true, Ordering::SeqCst);
        for _ in 0..10 {
            assert!(
                cache.key("entra-1").await.is_ok(),
                "stale set must keep serving"
            );
            assert_eq!(cache.key("unknown").await.err(), Some(Denial::UnknownKey));
        }
        assert_eq!(
            fetches.load(Ordering::SeqCst),
            1,
            "expired cache must respect refetch spacing"
        );
    }

    #[tokio::test]
    async fn spacing_elapsed_allows_exactly_one_more_fetch() {
        let (url, fetches, fail) = source(false).await;
        let cache = JwksCache::new(&url, true)
            .unwrap()
            .with_timing(Duration::ZERO, Duration::from_millis(200));
        assert!(cache.key("entra-1").await.is_ok());
        fail.store(true, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(250)).await;
        for _ in 0..5 {
            assert!(cache.key("entra-1").await.is_ok());
        }
        assert_eq!(fetches.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn non_loopback_http_and_url_parts_are_refused() {
        assert!(JwksCache::new("http://idp.example/keys", true).is_err());
        assert!(JwksCache::new("http://127.0.0.1/keys", false).is_err());
        assert!(JwksCache::new("https://u:p@idp.example/keys", false).is_err());
        assert!(JwksCache::new("https://idp.example/keys?x=1", false).is_err());
        assert!(JwksCache::new("https://idp.example/keys", false).is_ok());
    }
}

#[cfg(test)]
mod bounded_tests {
    use super::bounded;
    use serde_json::json;

    #[test]
    fn diagnostic_identifier_values_are_bounded() {
        assert_eq!(bounded(Some(&json!("org-1"))), json!("org-1"));
        assert_eq!(
            bounded(Some(&json!("x".repeat(128)))),
            json!("x".repeat(128))
        );
        assert_eq!(
            bounded(Some(&json!("x".repeat(129)))),
            json!("<omitted: 129 chars>")
        );
        assert_eq!(bounded(Some(&json!(["a", "b"]))), json!("<array>"));
        assert_eq!(bounded(Some(&json!({"k": "v"}))), json!("<object>"));
        assert_eq!(bounded(None), json!(null));
    }

    #[test]
    fn diagnostic_values_never_carry_terminal_control_characters() {
        // C0 (ESC, BEL, CR, LF, TAB), DEL and C1 (CSI U+009B, NEL U+0085) all become
        // U+FFFD, and nothing survives into the serialized log line either.
        let hostile = "a\u{1b}[2Jb\u{7}c\rd\ne\tf\u{7f}g\u{9b}31mh\u{85}i";
        let out = bounded(Some(&json!(hostile)));
        assert_eq!(
            out,
            json!(
                "a\u{fffd}[2Jb\u{fffd}c\u{fffd}d\u{fffd}e\u{fffd}f\u{fffd}g\u{fffd}31mh\u{fffd}i"
            )
        );
        let line = serde_json::to_string(&json!({"ungranted": out})).unwrap();
        assert!(!line.chars().any(char::is_control), "{line:?}");
        // The length bound counts the original characters.
        let long = "\u{9b}".repeat(129);
        assert_eq!(bounded(Some(&json!(long))), json!("<omitted: 129 chars>"));
    }
}
