//! Strict spike configuration. Contains no secrets: every value is a public
//! identifier, URL or path. Unknown keys are refused.

use std::path::PathBuf;

use serde::Deserialize;

/// Hard bound on the configuration file (plan: bounded projected config).
pub const MAX_CONFIG_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Public, non-secret configuration revision, reported in `service_info`.
    pub revision: String,
    pub server: Server,
    pub upstream: Upstream,
    pub assertion: Assertion,
    /// Optional per-route secret header (an alternative tried in M0.6; unused live,
    /// because the gateway forwards no custom headers on `oauth_auto`). When absent,
    /// `assertion.audiences` must be non-empty. The deployed binding is the upstream
    /// token (contract §2.2 resolution 3): `upstream.audiences` + `allowed_client_ids`.
    #[serde(default)]
    pub route_binding: Option<RouteBinding>,
    #[serde(default)]
    pub admin: Admin,
    #[serde(default)]
    pub grants: Vec<Grant>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub listen: String,
    /// Exact `Host` authorities accepted (DNS-rebinding protection).
    pub allowed_hosts: Vec<String>,
    /// RFC 9728 resource identifier advertised in protected-resource metadata.
    pub resource: String,
    /// Authorization servers advertised in protected-resource metadata.
    pub authorization_servers: Vec<String>,
    /// Scopes advertised in protected-resource metadata.
    pub scopes_supported: Vec<String>,
    /// Private, UID-owned 0700 data subtree (contract §9.3).
    pub data_dir: PathBuf,
    #[serde(default = "default_max_echo")]
    pub max_echo_bytes: usize,
    #[serde(default = "default_max_chunk")]
    pub max_chunk_bytes: usize,
    #[serde(default = "default_max_body")]
    pub max_request_body_bytes: usize,
    /// MCP session handling (contract §2.3). `stateless` (the default, and M1's
    /// mode) issues no `Mcp-Session-Id`: every request stands on its own tokens.
    /// `bound` keeps rmcp sessions, each bound to the principal that created it.
    #[serde(default)]
    pub sessions: SessionMode,
    /// Concurrent MCP sessions (bound + being created); ≤ the compiled 10,000.
    /// Applies only to `sessions = "bound"`.
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,
    /// Loopback tests only: hold each session-creating request this long after
    /// its capacity reservation, so concurrency tests genuinely overlap.
    #[serde(default)]
    pub test_creation_delay_ms: u64,
    /// Tests only: allow `http://127.0.0.1` JWKS origins. Production refuses it.
    #[serde(default)]
    pub allow_loopback_http_jwks: bool,
    /// Backend TLS (the gateway connects over HTTPS, as for Bonbon). Required
    /// unless `listen` is a loopback address.
    pub tls: Option<Tls>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    /// No MCP sessions; the gateway cannot multiplex agents onto one (M0.6 7.6).
    #[default]
    Stateless,
    /// Legacy rmcp sessions bound to their creating principal.
    Bound,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tls {
    pub cert_file: PathBuf,
    pub key_file: PathBuf,
}

fn default_max_echo() -> usize {
    256 * 1024
}
fn default_max_chunk() -> usize {
    64 * 1024
}
fn default_max_sessions() -> usize {
    crate::auth::MAX_SESSIONS
}
fn default_max_body() -> usize {
    256 * 1024
}

/// The Entra access token presented by the gateway in `Authorization: Bearer`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Upstream {
    pub issuer: String,
    pub audiences: Vec<String>,
    pub tenant_id: String,
    pub jwks_url: String,
    pub algorithms: Vec<String>,
    /// Each accepted token must carry at least one of these in `scp` or `roles`.
    pub accepted_permissions: Vec<String>,
    /// Allowed gateway client IDs, compared with `azp` (v2) or `appid` (v1).
    pub allowed_client_ids: Vec<String>,
    /// Allowed integration-owner object IDs (`oid`). An owner constraint only.
    pub owner_object_ids: Vec<String>,
}

/// The gateway-signed identity assertion naming the caller and agent.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assertion {
    pub header: String,
    pub issuer: String,
    /// Accepted assertion audiences. The gateway signs the organisation ID here for every
    /// integration, so this is NOT a per-backend binding; the binding is the upstream
    /// token's API `aud` plus dedicated `azp` (contract §2.2, plan revision 7 amendment 1).
    pub audiences: Vec<String>,
    pub algorithms: Vec<String>,
    /// Either a pinned JWKS URL or a PEM public key file (exactly one).
    pub jwks_url: Option<String>,
    pub public_key_pem_file: Option<PathBuf>,
    pub max_lifetime_seconds: u64,
    pub workspace_ids: Vec<String>,
    pub organization_ids: Vec<String>,
    #[serde(default)]
    pub claims: ClaimNames,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimNames {
    pub subject: String,
    pub client_id: String,
    pub workspace: String,
    pub organization: String,
    /// Optional backend-binding claim checked in addition to `aud` when set.
    pub backend_binding: Option<String>,
}

impl Default for ClaimNames {
    fn default() -> Self {
        Self {
            subject: "sub".into(),
            client_id: "client_id".into(),
            workspace: "workspace_id".into(),
            organization: "organisation_id".into(),
            backend_binding: None,
        }
    }
}

/// A per-integration secret header that only the gateway's route to this backend
/// adds (`configurations.custom_headers`). It binds the request, together with the
/// backend-specific upstream `aud`, when the assertion itself carries no audience.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteBinding {
    /// Lowercase header name, distinct from `authorization` and the assertion header.
    pub header: String,
    /// File holding the secret (at least 32 bytes after trimming); read once at start.
    pub secret_file: PathBuf,
}

/// Administrative fresh-proof option (contract §3.6). Absent or `disabled`
/// means no administrative mutation is ever admitted.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admin {
    #[serde(default)]
    pub mode: AdminMode,
    /// Claim in the identity assertion that must equal the server-issued nonce.
    pub proof_nonce_claim: Option<String>,
    /// Maximum age of the proof's authentication time.
    pub proof_max_age_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdminMode {
    #[default]
    Disabled,
    FreshProof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Reader,
    Operator,
    Administrator,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub subject: String,
    pub client_id: String,
    pub owner_object_id: String,
    pub permission: Permission,
    pub tools: Vec<String>,
}

#[derive(Debug)]
pub enum ConfigError {
    Unreadable,
    TooLarge,
    Invalid(&'static str),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable => f.write_str("configuration is not a readable regular file"),
            Self::TooLarge => f.write_str("configuration exceeds 64 KiB"),
            // Fixed reasons only: never echo parser text that may contain input.
            Self::Invalid(why) => write!(f, "invalid configuration: {why}"),
        }
    }
}

pub fn parse(text: &str) -> Result<Config, ConfigError> {
    let config: Config = toml::from_str(text).map_err(|_| ConfigError::Invalid("schema"))?;
    validate(&config)?;
    Ok(config)
}

fn nonempty(list: &[String], why: &'static str) -> Result<(), ConfigError> {
    if list.is_empty() || list.iter().any(|s| s.trim().is_empty()) {
        return Err(ConfigError::Invalid(why));
    }
    Ok(())
}

fn check_algorithms(list: &[String]) -> Result<(), ConfigError> {
    nonempty(list, "algorithms")?;
    for alg in list {
        // Asymmetric only: a shared HMAC secret would put signing power in config.
        if !matches!(
            alg.as_str(),
            "RS256" | "RS384" | "RS512" | "PS256" | "ES256" | "ES384" | "EdDSA"
        ) {
            return Err(ConfigError::Invalid("algorithm not allowed"));
        }
    }
    Ok(())
}

fn validate(c: &Config) -> Result<(), ConfigError> {
    nonempty(&c.server.allowed_hosts, "server.allowed_hosts")?;
    nonempty(
        &c.server.authorization_servers,
        "server.authorization_servers",
    )?;
    nonempty(&c.upstream.audiences, "upstream.audiences")?;
    nonempty(
        &c.upstream.accepted_permissions,
        "upstream.accepted_permissions",
    )?;
    nonempty(
        &c.upstream.allowed_client_ids,
        "upstream.allowed_client_ids",
    )?;
    nonempty(&c.upstream.owner_object_ids, "upstream.owner_object_ids")?;
    match &c.route_binding {
        // Resolution 1: the signed assertion `aud` is the binding.
        None => nonempty(&c.assertion.audiences, "assertion.audiences")?,
        Some(rb) => {
            let h = rb.header.as_str();
            if h.is_empty()
                || h != h.to_ascii_lowercase()
                || !h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                || h == "authorization"
                || h == c.assertion.header.to_ascii_lowercase()
            {
                return Err(ConfigError::Invalid("route_binding.header"));
            }
        }
    }
    if c.assertion.claims.backend_binding.is_some() && c.assertion.audiences.is_empty() {
        return Err(ConfigError::Invalid(
            "assertion.claims.backend_binding needs assertion.audiences",
        ));
    }
    nonempty(&c.assertion.workspace_ids, "assertion.workspace_ids")?;
    nonempty(&c.assertion.organization_ids, "assertion.organization_ids")?;
    check_algorithms(&c.upstream.algorithms)?;
    if c.upstream
        .accepted_permissions
        .iter()
        .any(|p| crate::auth::scope_tier(p).is_none())
    {
        return Err(ConfigError::Invalid(
            "upstream.accepted_permissions must be frozen AirsHistory scopes",
        ));
    }
    check_algorithms(&c.assertion.algorithms)?;
    if c.assertion.jwks_url.is_some() == c.assertion.public_key_pem_file.is_some() {
        return Err(ConfigError::Invalid(
            "assertion needs exactly one key source",
        ));
    }
    if !(1..=300).contains(&c.assertion.max_lifetime_seconds) {
        return Err(ConfigError::Invalid("assertion.max_lifetime_seconds"));
    }
    let addr: std::net::SocketAddr = c
        .server
        .listen
        .parse()
        .map_err(|_| ConfigError::Invalid("server.listen"))?;
    if c.server.tls.is_none() && !addr.ip().is_loopback() {
        return Err(ConfigError::Invalid("server.tls is required off loopback"));
    }
    if c.server.test_creation_delay_ms > 0 && !addr.ip().is_loopback() {
        return Err(ConfigError::Invalid(
            "test_creation_delay_ms is a loopback test setting",
        ));
    }
    if c.server.allow_loopback_http_jwks && !addr.ip().is_loopback() {
        return Err(ConfigError::Invalid(
            "allow_loopback_http_jwks is a loopback test setting",
        ));
    }
    if !c.server.data_dir.is_absolute() {
        return Err(ConfigError::Invalid("server.data_dir must be absolute"));
    }
    if !(1..=crate::auth::MAX_SESSIONS).contains(&c.server.max_sessions) {
        return Err(ConfigError::Invalid("server.max_sessions"));
    }
    if c.server.max_chunk_bytes > 64 * 1024 || c.server.max_echo_bytes > 1024 * 1024 {
        return Err(ConfigError::Invalid(
            "response bound above compiled ceiling",
        ));
    }
    // Configured strings that diagnostics print (the revision) or use as log keys and
    // claim lookups (the claim names): 1-128 characters without control characters, so
    // no configuration can put terminal controls into a log line (review 10 minor 1).
    let names = &c.assertion.claims;
    for (field, value) in [
        ("revision", Some(c.revision.as_str())),
        ("assertion.claims.subject", Some(names.subject.as_str())),
        ("assertion.claims.client_id", Some(names.client_id.as_str())),
        ("assertion.claims.workspace", Some(names.workspace.as_str())),
        (
            "assertion.claims.organization",
            Some(names.organization.as_str()),
        ),
        (
            "assertion.claims.backend_binding",
            names.backend_binding.as_deref(),
        ),
        (
            "admin.proof_nonce_claim",
            c.admin.proof_nonce_claim.as_deref(),
        ),
    ] {
        if value.is_some_and(|v| !diagnostic_text(v)) {
            return Err(ConfigError::Invalid(field));
        }
    }
    if c.admin.mode == AdminMode::FreshProof
        && (c.admin.proof_nonce_claim.is_none() || c.admin.proof_max_age_seconds.is_none())
    {
        return Err(ConfigError::Invalid(
            "fresh_proof needs nonce claim and max age",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for g in &c.grants {
        if !seen.insert((g.subject.clone(), g.client_id.clone())) {
            return Err(ConfigError::Invalid("duplicate grant"));
        }
        if g.tools.is_empty() {
            return Err(ConfigError::Invalid("grant without tools"));
        }
        for t in &g.tools {
            match crate::tools::required_tier(t) {
                None => return Err(ConfigError::Invalid("grant names an unknown tool")),
                Some(tier) if tier > g.permission => {
                    return Err(ConfigError::Invalid("grant lists a tool above its tier"));
                }
                Some(_) => {}
            }
        }
    }
    Ok(())
}

/// A configured diagnostic string: 1-128 characters, none of them a control character.
fn diagnostic_text(value: &str) -> bool {
    let count = value.chars().count();
    (1..=128).contains(&count) && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = include_str!("../../../deploy/m0.6/spike-config.toml");

    #[test]
    fn grants_list_only_known_tools_at_or_below_their_tier() {
        let reader = r#"tools = ["service_info", "echo", "text_page", "artifact_chunk"]"#;
        assert!(TEMPLATE.contains(reader), "template Reader grant changed");
        for (tools, reason) in [
            (
                r#"tools = ["service_info", "echo", "text_page", "artifact_chunk", "operator_noop"]"#,
                "grant lists a tool above its tier",
            ),
            (
                r#"tools = ["service_info", "no_such_tool"]"#,
                "grant names an unknown tool",
            ),
        ] {
            let text = TEMPLATE.replacen(reader, tools, 1);
            match parse(&text) {
                Err(ConfigError::Invalid(r)) => assert_eq!(r, reason),
                other => panic!("{tools}: {other:?}"),
            }
        }
    }

    /// Review 10 minor 1: a configured revision or claim name with a control character
    /// (C0, DEL or C1), or one longer than 128 characters, is refused, so none can reach a
    /// log line through `listening` or a claim-name key.
    #[test]
    fn configured_diagnostic_strings_refuse_control_characters() {
        for (from, to, field) in [
            (
                "revision = \"m06-1\"",
                "revision = \"m06\\u0085x\"",
                "revision",
            ),
            (
                "revision = \"m06-1\"",
                "revision = \"m06\\u001b[2J\"",
                "revision",
            ),
            ("revision = \"m06-1\"", "revision = \"\"", "revision"),
            (
                "subject = \"sub\"",
                "subject = \"s\\u007fb\"",
                "assertion.claims.subject",
            ),
            (
                "client_id = \"client_id\"",
                "client_id = \"c\\u009bid\"",
                "assertion.claims.client_id",
            ),
        ] {
            assert!(TEMPLATE.contains(from), "template changed: {from}");
            match parse(&TEMPLATE.replacen(from, to, 1)) {
                Err(ConfigError::Invalid(reason)) => assert_eq!(reason, field, "{to}"),
                other => panic!("{to}: {other:?}"),
            }
        }
        let long = format!("revision = \"{}\"", "r".repeat(129));
        assert!(matches!(
            parse(&TEMPLATE.replacen("revision = \"m06-1\"", &long, 1)),
            Err(ConfigError::Invalid("revision"))
        ));
        assert!(diagnostic_text(&"r".repeat(128)) && diagnostic_text("m06-8"));
    }

    #[test]
    fn the_deployment_template_parses_and_test_hooks_are_refused_off_loopback() {
        let c = parse(TEMPLATE).expect("deploy/m0.6/spike-config.toml must stay valid");
        assert_eq!(c.admin.mode, AdminMode::Disabled);
        assert_eq!(c.server.max_sessions, crate::auth::MAX_SESSIONS);
        for hook in [
            "test_creation_delay_ms = 10",
            "allow_loopback_http_jwks = true",
            "max_sessions = 0",
            "max_sessions = 10001",
        ] {
            let text = TEMPLATE.replacen(
                "max_chunk_bytes = 65536",
                &format!("max_chunk_bytes = 65536\n{hook}"),
                1,
            );
            assert!(
                parse(&text).is_err(),
                "{hook} accepted on a non-loopback listener"
            );
        }
    }
}
