//! Strict service configuration (contract §11). It holds no secrets: every value is a
//! public identifier, URL, bound or path, and secrets are referenced by file. Unknown
//! keys are refused at every level; errors are fixed reasons, never parser text.

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
    #[serde(default)]
    pub admin: Admin,
    pub service: Service,
    pub airs: Airs,
    #[serde(default)]
    pub receipt_associations: Vec<ReceiptAssociation>,
    #[serde(default)]
    pub limits: Limits,
    #[serde(default)]
    pub grants: Vec<Grant>,
}

/// The public service identity (contract §11 `[service]`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    /// Public service tenant ID: every tool request's `tenant` must equal it (§4.2).
    pub tenant: String,
    /// The reporting timezone. Only the compiled `America/Chicago` is accepted (§8).
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// Maintenance mode: every tool but `service_info` answers `maintenance_required`.
    #[serde(default)]
    pub maintenance: bool,
}

fn default_timezone() -> String {
    airs_runtime_core::schedule::REPORT_TZ.into()
}

/// The one AIRS tenant (TSG) the service collects at launch (§11 `[airs]`). Secrets
/// are files projected from the cluster's secret store, never inline values.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Airs {
    pub tsg: String,
    pub management_endpoint: String,
    pub dashboard_endpoint: String,
    pub token_endpoint: String,
    pub management_client_id_file: PathBuf,
    pub management_client_secret_file: PathBuf,
    /// Credential revision (§10): changes only when the credential does.
    pub management_revision: String,
    #[serde(default)]
    pub scanners: Vec<Scanner>,
}

/// An application-bound scanner credential (credential contract §2), secret by file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scanner {
    pub name: String,
    #[serde(rename = "applicationId")]
    pub application_id: String,
    pub endpoint: String,
    #[serde(rename = "numRetries", default)]
    pub num_retries: u8,
    pub revision: String,
    pub api_key_file: Option<PathBuf>,
    pub api_token_file: Option<PathBuf>,
}

/// `(caller sub, agent client_id, declared scanner)` → the configured scanner whose
/// binding a client-asserted receipt is recorded under (§11).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptAssociation {
    pub subject: String,
    pub client_id: String,
    pub declared_scanner: String,
    pub scanner: String,
}

/// Optional overrides, each within the frozen bound of contract §6.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    #[serde(default = "d_interval")]
    pub collection_interval_minutes: u64,
    #[serde(default = "d_tick_seconds")]
    pub tick_seconds: u64,
    #[serde(default = "d_tick_sends")]
    pub tick_sends: u64,
    #[serde(default = "d_tick_pages")]
    pub tick_pages_per_source: u64,
    #[serde(default = "d_reports_sends")]
    pub reports_daily_sends: u64,
    #[serde(default = "d_reports_ids")]
    pub reports_daily_ids: u64,
    #[serde(default = "d_manual_hour")]
    pub manual_reports_per_principal_hour: u64,
    #[serde(default = "d_manual_day")]
    pub manual_reports_per_tenant_day: u64,
    #[serde(default = "d_queue_tenant")]
    pub queue_per_tenant: u64,
    #[serde(default = "d_queue_principal")]
    pub queue_per_principal: u64,
    #[serde(default = "d_staging")]
    pub staging_quota_bytes: u64,
}

fn d_interval() -> u64 {
    15
}
fn d_tick_seconds() -> u64 {
    120
}
fn d_tick_sends() -> u64 {
    200
}
fn d_tick_pages() -> u64 {
    40
}
fn d_reports_sends() -> u64 {
    2
}
fn d_reports_ids() -> u64 {
    10
}
fn d_manual_hour() -> u64 {
    4
}
fn d_manual_day() -> u64 {
    24
}
fn d_queue_tenant() -> u64 {
    64
}
fn d_queue_principal() -> u64 {
    8
}
fn d_staging() -> u64 {
    1 << 30
}

impl Default for Limits {
    fn default() -> Self {
        toml::from_str("").expect("every limit has a default")
    }
}

impl Limits {
    /// Each value and its frozen bound (contract §6). Defaults lie inside every bound;
    /// overrides may lower a compiled default where §6 gives no higher ceiling.
    fn bounds(&self) -> [(&'static str, u64, std::ops::RangeInclusive<u64>); 11] {
        [
            (
                "limits.collection_interval_minutes",
                self.collection_interval_minutes,
                5..=60,
            ),
            ("limits.tick_seconds", self.tick_seconds, 1..=600),
            ("limits.tick_sends", self.tick_sends, 1..=1000),
            (
                "limits.tick_pages_per_source",
                self.tick_pages_per_source,
                1..=200,
            ),
            (
                "limits.reports_daily_sends",
                self.reports_daily_sends,
                0..=10,
            ),
            ("limits.reports_daily_ids", self.reports_daily_ids, 0..=50),
            (
                "limits.manual_reports_per_principal_hour",
                self.manual_reports_per_principal_hour,
                0..=4,
            ),
            (
                "limits.manual_reports_per_tenant_day",
                self.manual_reports_per_tenant_day,
                0..=48,
            ),
            ("limits.queue_per_tenant", self.queue_per_tenant, 1..=64),
            (
                "limits.queue_per_principal",
                self.queue_per_principal,
                1..=8,
            ),
            (
                "limits.staging_quota_bytes",
                self.staging_quota_bytes,
                (1 << 20)..=(16 << 30),
            ),
        ]
    }
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
    #[serde(default = "default_max_body")]
    pub max_request_body_bytes: usize,
    /// Private, unauthenticated health listener (`/livez`, `/readyz`): fixed bodies, no
    /// tenant data. For cluster probes; never routed through the gateway.
    #[serde(default)]
    pub health_listen: Option<String>,
    /// Tests only: allow `http://127.0.0.1` JWKS origins. Production refuses it.
    #[serde(default)]
    pub allow_loopback_http_jwks: bool,
    /// Backend TLS (the gateway connects over HTTPS, as for Bonbon). Required
    /// unless `listen` is a loopback address.
    pub tls: Option<Tls>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tls {
    pub cert_file: PathBuf,
    pub key_file: PathBuf,
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
    /// Public label recorded in audit lines (contract §2.5); defaults to the tier name.
    #[serde(default)]
    pub label: Option<String>,
}

impl Grant {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(match self.permission {
            Permission::Reader => "reader",
            Permission::Operator => "operator",
            Permission::Administrator => "administrator",
        })
    }
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
    // The assertion audience is checked on every request. The deployed binding is the
    // upstream token (contract §2.2 resolution 3: `upstream.audiences` plus the
    // dedicated `allowed_client_ids`); the measured assertion `aud` is organisation-wide.
    nonempty(&c.assertion.audiences, "assertion.audiences")?;
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
    if let Some(health) = &c.server.health_listen {
        let health: std::net::SocketAddr = health
            .parse()
            .map_err(|_| ConfigError::Invalid("server.health_listen"))?;
        if health.port() == addr.port() && health.port() != 0 {
            return Err(ConfigError::Invalid("server.health_listen"));
        }
    }
    if c.server.allow_loopback_http_jwks && !addr.ip().is_loopback() {
        return Err(ConfigError::Invalid(
            "allow_loopback_http_jwks is a loopback test setting",
        ));
    }
    if !c.server.data_dir.is_absolute() {
        return Err(ConfigError::Invalid("server.data_dir must be absolute"));
    }
    if !(1024..=256 * 1024).contains(&c.server.max_request_body_bytes) {
        return Err(ConfigError::Invalid("server.max_request_body_bytes"));
    }
    validate_service(c)?;
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
        for name in &g.tools {
            let Some(tool) = crate::catalogue::get(name) else {
                return Err(ConfigError::Invalid("grant names an unknown tool"));
            };
            if tool.tier > g.permission {
                return Err(ConfigError::Invalid("grant lists a tool above its tier"));
            }
            if g.permission == Permission::Administrator
                && !crate::catalogue::administrator_may_list(tool)
            {
                return Err(ConfigError::Invalid(
                    "an administrator grant lists a mutating grant tool",
                ));
            }
        }
        for (field, value) in [
            ("grants.subject", &g.subject),
            ("grants.client_id", &g.client_id),
            ("grants.owner_object_id", &g.owner_object_id),
            ("grants.label", g.label.as_ref().unwrap_or(&g.client_id)),
        ] {
            if !diagnostic_text(value) {
                return Err(ConfigError::Invalid(field));
            }
        }
    }
    Ok(())
}

/// `[service]`, `[airs]`, scanners, receipt associations and `[limits]` (contract §11).
fn validate_service(c: &Config) -> Result<(), ConfigError> {
    let tenant = c.service.tenant.as_bytes();
    if tenant.is_empty()
        || tenant.len() > 63
        || !(tenant[0].is_ascii_lowercase() || tenant[0].is_ascii_digit())
        || !tenant
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
    {
        return Err(ConfigError::Invalid("service.tenant"));
    }
    if c.service.timezone != airs_runtime_core::schedule::REPORT_TZ {
        return Err(ConfigError::Invalid("service.timezone"));
    }
    let a = &c.airs;
    if a.tsg.is_empty() || a.tsg.len() > 32 || !a.tsg.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ConfigError::Invalid("airs.tsg"));
    }
    for (field, value) in [
        ("airs.management_endpoint", &a.management_endpoint),
        ("airs.dashboard_endpoint", &a.dashboard_endpoint),
        ("airs.token_endpoint", &a.token_endpoint),
    ] {
        admitted_endpoint(value).ok_or(ConfigError::Invalid(field))?;
    }
    for (field, path) in [
        (
            "airs.management_client_id_file",
            &a.management_client_id_file,
        ),
        (
            "airs.management_client_secret_file",
            &a.management_client_secret_file,
        ),
    ] {
        if !path.is_absolute() {
            return Err(ConfigError::Invalid(field));
        }
    }
    if !airs_runtime_core::dto::is_uuid_v4(&a.management_revision) {
        return Err(ConfigError::Invalid("airs.management_revision"));
    }
    let mut names = std::collections::BTreeSet::new();
    let mut applications = std::collections::BTreeSet::new();
    for s in &a.scanners {
        if s.name.is_empty()
            || s.name.len() > 64
            || !s
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !names.insert(s.name.to_ascii_lowercase())
        {
            return Err(ConfigError::Invalid("airs.scanners.name"));
        }
        if !airs_runtime_core::tenant::scanners::valid_application(&s.application_id)
            || !applications.insert(s.application_id.clone())
        {
            return Err(ConfigError::Invalid("airs.scanners.applicationId"));
        }
        admitted_endpoint(&s.endpoint).ok_or(ConfigError::Invalid("airs.scanners.endpoint"))?;
        if s.num_retries > 5 {
            return Err(ConfigError::Invalid("airs.scanners.numRetries"));
        }
        if !airs_runtime_core::dto::is_uuid_v4(&s.revision) {
            return Err(ConfigError::Invalid("airs.scanners.revision"));
        }
        match (&s.api_key_file, &s.api_token_file) {
            (Some(p), None) | (None, Some(p)) if p.is_absolute() => {}
            _ => {
                return Err(ConfigError::Invalid(
                    "airs.scanners needs exactly one absolute secret file",
                ));
            }
        }
    }
    for r in &c.receipt_associations {
        if !diagnostic_text(&r.subject)
            || !diagnostic_text(&r.client_id)
            || !a.scanners.iter().any(|s| s.name == r.declared_scanner)
            || !a.scanners.iter().any(|s| s.name == r.scanner)
        {
            return Err(ConfigError::Invalid("receipt_associations"));
        }
    }
    for (field, value, bound) in c.limits.bounds() {
        if !bound.contains(&value) {
            return Err(ConfigError::Invalid(field));
        }
    }
    Ok(())
}

/// D48 endpoint admission (DESIGN §5): https anywhere, http only for loopback hosts,
/// no credentials, query or fragment.
fn admitted_endpoint(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    airs_runtime_core::clients::admit("endpoint", Some(value), value)
        .ok()
        .map(|e| e.url().as_str().trim_end_matches('/').to_owned())
}

/// A configured diagnostic string: 1-128 characters, none of them a control character.
fn diagnostic_text(value: &str) -> bool {
    let count = value.chars().count();
    (1..=128).contains(&count) && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = include_str!("../config/example.toml");

    fn refused(text: &str) -> &'static str {
        match parse(text) {
            Err(ConfigError::Invalid(reason)) => reason,
            other => panic!("accepted or wrong error: {other:?}"),
        }
    }

    fn edit(from: &str, to: &str) -> String {
        assert!(TEMPLATE.contains(from), "template changed: {from}");
        TEMPLATE.replacen(from, to, 1)
    }

    #[test]
    fn the_example_parses_with_every_section_and_default() {
        let c = parse(TEMPLATE).expect("config/example.toml must stay valid");
        assert_eq!(c.admin.mode, AdminMode::Disabled);
        assert_eq!(c.service.tenant, "airs-tsg-1234567890");
        assert_eq!(c.service.timezone, "America/Chicago");
        assert!(!c.service.maintenance);
        assert_eq!(c.airs.scanners.len(), 1);
        assert_eq!(c.limits.collection_interval_minutes, 15);
        assert_eq!(c.limits.reports_daily_sends, 2);
        assert_eq!(c.limits.staging_quota_bytes, 1 << 30);
        assert_eq!(c.grants[0].label(), "claude-code");
        assert_eq!(c.server.health_listen.as_deref(), Some("0.0.0.0:8081"));
        let default_label = edit("label = \"claude-code\"\n", "");
        assert_eq!(parse(&default_label).unwrap().grants[0].label(), "reader");
    }

    /// Contract §11: every grant tool is catalogued at or below the grant's tier, and an
    /// Administrator grant never lists a mutating `grant`-authority tool (§3.5).
    #[test]
    fn grants_follow_the_frozen_catalogue() {
        let reader = r#"tools = ["service_info", "history_status", "history_coverage"]"#;
        for (tools, reason) in [
            (
                r#"tools = ["service_info", "collection_start"]"#,
                "grant lists a tool above its tier",
            ),
            (
                r#"tools = ["service_info", "echo"]"#,
                "grant names an unknown tool",
            ),
            (r#"tools = []"#, "grant without tools"),
        ] {
            assert_eq!(refused(&edit(reader, tools)), reason, "{tools}");
        }
        let admin = edit(
            "permission = \"operator\"",
            "permission = \"administrator\"",
        );
        assert_eq!(
            refused(&admin),
            "an administrator grant lists a mutating grant tool",
            "collection_start and receipts_ingest are grant-authority mutations"
        );
        let admin_ok = admin.replace(
            r#"tools = ["service_info", "history_status", "history_coverage", "collection_start", "receipts_ingest"]"#,
            r#"tools = ["service_info", "history_status", "lifecycle_preview", "lifecycle_execute", "admin_confirm"]"#,
        );
        assert!(parse(&admin_ok).is_ok());
        let duplicate = format!(
            "{TEMPLATE}\n[[grants]]\nsubject = \"PLACEHOLDER_CALLER_SUB\"\nclient_id = \"PLACEHOLDER_CLAUDE_CODE_CLIENT_ID\"\nowner_object_id = \"o\"\npermission = \"reader\"\ntools = [\"service_info\"]\n"
        );
        assert_eq!(refused(&duplicate), "duplicate grant");
    }

    /// `[service]`, `[airs]`, scanners, receipt associations and `[limits]` (§11, §6).
    #[test]
    fn service_airs_and_limits_are_validated() {
        for (from, to, reason) in [
            (
                "tenant = \"airs-tsg-1234567890\"",
                "tenant = \"Tenant\"",
                "service.tenant",
            ),
            (
                "timezone = \"America/Chicago\"",
                "timezone = \"UTC\"",
                "service.timezone",
            ),
            ("tsg = \"1234567890\"", "tsg = \"12x\"", "airs.tsg"),
            (
                "management_endpoint = \"https://api.apps.paloaltonetworks.com/aisec\"",
                "management_endpoint = \"http://api.apps.paloaltonetworks.com/aisec\"",
                "airs.management_endpoint",
            ),
            (
                "token_endpoint = \"https://auth.apps.paloaltonetworks.com/oauth2/access_token\"",
                "token_endpoint = \"https://user:pw@auth.example.test/token\"",
                "airs.token_endpoint",
            ),
            (
                "management_client_id_file = \"/run/airs-mcp/secrets/mgmt-client-id\"",
                "management_client_id_file = \"secrets/mgmt-client-id\"",
                "airs.management_client_id_file",
            ),
            (
                "management_revision = \"00000000-0000-4000-8000-000000000005\"",
                "management_revision = \"rev-1\"",
                "airs.management_revision",
            ),
            (
                "numRetries = 0",
                "numRetries = 6",
                "airs.scanners.numRetries",
            ),
            (
                "api_key_file = \"/run/airs-mcp/secrets/scanner-monitor\"",
                "api_key_file = \"/a\"\napi_token_file = \"/b\"",
                "airs.scanners needs exactly one absolute secret file",
            ),
            (
                "declared_scanner = \"monitor\"",
                "declared_scanner = \"other\"",
                "receipt_associations",
            ),
            (
                "collection_interval_minutes = 15",
                "collection_interval_minutes = 4",
                "limits.collection_interval_minutes",
            ),
            (
                "collection_interval_minutes = 15",
                "reports_daily_sends = 11",
                "limits.reports_daily_sends",
            ),
            (
                "collection_interval_minutes = 15",
                "manual_reports_per_tenant_day = 49",
                "limits.manual_reports_per_tenant_day",
            ),
            (
                "collection_interval_minutes = 15",
                "staging_quota_bytes = 17179869185",
                "limits.staging_quota_bytes",
            ),
            (
                "health_listen = \"0.0.0.0:8081\"",
                "health_listen = \"0.0.0.0:8443\"",
                "server.health_listen",
            ),
        ] {
            assert_eq!(refused(&edit(from, to)), reason, "{to}");
        }
        assert_eq!(
            refused(&edit("[limits]", "[limits]\nunknown = 1")),
            "schema",
            "unknown keys are refused"
        );
    }

    /// Review M0-10 minor 1: configured strings that reach logs or claim lookups refuse
    /// control characters (C0, DEL, C1) and lengths beyond 128.
    #[test]
    fn configured_diagnostic_strings_refuse_control_characters() {
        for (from, to, field) in [
            (
                "revision = \"example-1\"",
                "revision = \"ex\\u0085x\"",
                "revision",
            ),
            (
                "revision = \"example-1\"",
                "revision = \"ex\\u001b[2J\"",
                "revision",
            ),
            ("revision = \"example-1\"", "revision = \"\"", "revision"),
            (
                "subject = \"email\"",
                "subject = \"e\\u007fm\"",
                "assertion.claims.subject",
            ),
            (
                "label = \"claude-code\"",
                "label = \"c\\u009bc\"",
                "grants.label",
            ),
        ] {
            assert_eq!(refused(&edit(from, to)), field, "{to}");
        }
        let long = format!("revision = \"{}\"", "r".repeat(129));
        assert_eq!(
            refused(&edit("revision = \"example-1\"", &long)),
            "revision"
        );
        assert!(diagnostic_text(&"r".repeat(128)) && diagnostic_text("m06-8"));
    }

    #[test]
    fn loopback_test_hooks_and_transport_bounds_are_refused_in_production() {
        assert_eq!(
            refused(&edit(
                "data_dir = \"/data/airs\"",
                "data_dir = \"/data/airs\"\nallow_loopback_http_jwks = true"
            )),
            "allow_loopback_http_jwks is a loopback test setting"
        );
        assert_eq!(
            refused(&edit(
                "data_dir = \"/data/airs\"",
                "data_dir = \"/data/airs\"\nmax_request_body_bytes = 300000"
            )),
            "server.max_request_body_bytes"
        );
        assert_eq!(
            refused(&edit(
                "[server.tls]\ncert_file = \"/etc/airs-mcp/tls/tls.crt\"\nkey_file = \"/etc/airs-mcp/tls/tls.key\"\n",
                ""
            )),
            "server.tls is required off loopback"
        );
        assert_eq!(
            refused(&edit("data_dir = \"/data/airs\"", "data_dir = \"data\"")),
            "server.data_dir must be absolute"
        );
    }
}
