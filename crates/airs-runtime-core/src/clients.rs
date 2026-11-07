//! Endpoint admission and SDK client construction (DESIGN §5).
//!
//! Every configured URL is admitted with `Endpoint::loopback` (https anywhere, http
//! only for loopback hosts). Credentials come only from the tenant file and are
//! checked locally before any request, so configuration mistakes cost zero requests.
//!
//! A client with a plain-http (loopback) endpoint uses [`DirectTransport`], which never
//! goes through a proxy: http is admitted only because the traffic stays on the host,
//! and a proxy would carry the cleartext credentials off it.

use crate::debug::{DebugLog, DebugTransport};
use crate::error::{self, CliError, Operation, Phase, Plane, SdkContext, SecretSet};
use crate::tenant::config::{TenantConfig, js_trim};
use futures_util::StreamExt;
use futures_util::future::BoxFuture;
use scm::airs::management::ManagementClient;
use scm::airs::scan::Scanner;
use scm::auth::{AuthContext, Authentication, OAuthProvider, TokenLease, TokenProvider};
use scm::config::DestinationPolicy;
use scm::transport::{ReqwestTransport, Transport, TransportRequest, TransportResponse};
use scm::{Client, Endpoint, ErrorKind, Limits, OriginPolicy, RequestOptions, Secret, Session};
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

pub const DEFAULT_TOKEN_ENDPOINT: &str =
    "https://auth.apps.paloaltonetworks.com/oauth2/access_token";
pub const DEFAULT_MGMT_ENDPOINT: &str = "https://api.apps.paloaltonetworks.com/aisec";
pub const DEFAULT_SCAN_ENDPOINT: &str = "https://service.api.aisecurity.paloaltonetworks.com";
/// The red team planes' defaults (the SDK constants, TS `SDK:91-93`; DESIGN §15.3).
pub const DEFAULT_RED_TEAM_DATA_ENDPOINT: &str =
    "https://api.apps.paloaltonetworks.com/ai-red-teaming/data-plane";
pub const DEFAULT_RED_TEAM_MGMT_ENDPOINT: &str =
    "https://api.apps.paloaltonetworks.com/ai-red-teaming/mgmt-plane";
pub const DEFAULT_RED_TEAM_BROKER_ENDPOINT: &str =
    "https://api.apps.paloaltonetworks.com/ai-red-teaming/data-plane/network-broker";

/// The SDK default budget for one request without explicit `RequestOptions`.
pub const SINGLE_REQUEST_BUDGET: Duration = Duration::from_secs(60);
/// The SDK's fixed budget for an OAuth refresh when no options are passed.
pub const TOKEN_BUDGET: Duration = Duration::from_secs(30);
/// Full walks (`--all`, lookups, latest-topic listings).
pub const WALK_BUDGET: Duration = Duration::from_secs(300);
/// The whole of one doctor probe (token + request).
pub const PROBE_BUDGET: Duration = Duration::from_secs(5);

/// TS SDK default attempt count for 502/503/504 on management reads.
const MANAGEMENT_RETRIES: u32 = 5;
/// The TypeScript SDK's scanner credential limit (UTF-16 code units).
const MAX_SCANNER_CREDENTIAL: usize = 2048;

/// A client could not be configured; `key` names the tenant setting at fault.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupError {
    pub key: Option<&'static str>,
    pub error: CliError,
}

impl SetupError {
    fn config(key: &'static str, message: impl Into<String>) -> Self {
        Self {
            key: Some(key),
            error: CliError::operational(message),
        }
    }
    fn sdk(message: &str) -> Self {
        Self {
            key: None,
            error: CliError::operational(message),
        }
    }
    /// The failure message (for doctor details).
    pub fn message(&self) -> String {
        self.error.message()
    }
}

impl From<SetupError> for CliError {
    fn from(value: SetupError) -> Self {
        value.error
    }
}

/// The admission failure for `key` (the value is never echoed).
fn invalid_endpoint(key: &'static str) -> SetupError {
    SetupError::config(
        key,
        format!(
            "Invalid {key}: endpoints must use https (http is allowed only for loopback hosts) and must not contain credentials, a query or a fragment"
        ),
    )
}

/// Admit a configured URL (or the default when unset).
pub fn admit(
    key: &'static str,
    configured: Option<&str>,
    default: &str,
) -> Result<Endpoint, SetupError> {
    Endpoint::loopback(configured.unwrap_or(default)).map_err(|_| invalid_endpoint(key))
}

/// The status of the last HTTP response a client received (0 until one arrives).
///
/// A walk or probe that ends at its deadline while the SDK waits to retry a 429/5xx
/// surfaces as `Deadline`, and a deadline that drops a page fetch loses the answer's
/// status; doctor reads this to report what the endpoint actually said (§9).
#[derive(Clone, Debug, Default)]
pub struct LastStatus(Arc<ClientObservations>);

type OperationResponse = (Option<u16>, Option<u64>);

/// Only a usable future delay is returned. Zero/past/invalid values fall back to
/// the history service's conservative default. Durations round up and cap at24h.
pub(crate) fn retry_after_seconds(value: Option<&str>, now: std::time::SystemTime) -> Option<u64> {
    let value = value?.trim();
    if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
        let seconds = value.parse::<u64>().unwrap_or(u64::MAX);
        return (seconds > 0).then_some(seconds.min(86400));
    }
    let delay = httpdate::parse_http_date(value)
        .ok()?
        .duration_since(now)
        .ok()?;
    let seconds = delay
        .as_secs()
        .saturating_add(u64::from(delay.subsec_nanos() > 0));
    (seconds > 0).then_some(seconds.min(86400))
}

#[derive(Debug, Default)]
struct ClientObservations {
    status: AtomicU16,
    attempts: std::sync::Mutex<std::collections::BTreeMap<String, u64>>,
    responses: std::sync::Mutex<std::collections::BTreeMap<String, OperationResponse>>,
}

impl LastStatus {
    /// Latest operation send; cleared before entering its transport. History's
    /// scanner calls are serial per client, so a failed send cannot inherit status.
    pub fn response(&self, operation: &str) -> (Option<u16>, Option<u64>) {
        self.0
            .responses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(operation)
            .copied()
            .unwrap_or_default()
    }
    pub fn get(&self) -> Option<u16> {
        match self.0.status.load(Ordering::SeqCst) {
            0 => None,
            status => Some(status),
        }
    }

    /// Actual sends admitted to this client's transport, grouped by fixed SDK operation.
    pub fn attempts(&self, operation: &str) -> u64 {
        self.0
            .attempts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(operation)
            .copied()
            .unwrap_or(0)
    }
}

/// Records each response status in a [`LastStatus`]; otherwise the inner transport.
struct ObservedTransport {
    inner: Arc<dyn Transport>,
    last: LastStatus,
}

impl Transport for ObservedTransport {
    fn send<'a>(
        &'a self,
        request: TransportRequest,
    ) -> BoxFuture<'a, scm::Result<TransportResponse>> {
        Box::pin(async move {
            let operation = request.operation().to_owned();
            self.last
                .0
                .responses
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(operation.clone(), (None, None));
            *self
                .last
                .0
                .attempts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .entry(request.operation().to_owned())
                .or_default() += 1;
            let response = self.inner.send(request).await?;
            let retry_after = retry_after_seconds(
                response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok()),
                std::time::SystemTime::now(),
            );
            self.last
                .0
                .responses
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(operation, (Some(response.status().as_u16()), retry_after));
            self.last
                .0
                .status
                .store(response.status().as_u16(), Ordering::SeqCst);
            Ok(response)
        })
    }
}

/// The SDK's `ReqwestTransport` (which honours proxy settings), or [`DirectTransport`]
/// when any of the client's endpoints is plain http; wrapped by the debug transport
/// (a pass-through without `debug`, DESIGN §2.9) and observed by `last`.
fn transport(
    endpoints: &[&Endpoint],
    last: &LastStatus,
    debug: Option<&DebugLog>,
) -> Result<Arc<dyn Transport>, SetupError> {
    let failed = || SetupError::sdk("AISEC_SDK_ERROR:The HTTP transport could not be initialised");
    let inner: Arc<dyn Transport> = if endpoints
        .iter()
        .any(|endpoint| endpoint.url().scheme() == "http")
    {
        Arc::new(DirectTransport::new().ok_or_else(failed)?)
    } else {
        Arc::new(ReqwestTransport::new().map_err(|_| failed())?)
    };
    Ok(Arc::new(ObservedTransport {
        inner: DebugTransport::wrap(inner, debug),
        last: last.clone(),
    }))
}

/// The SDK's own user agent (`PAN-AIRS/<scm-sdk version>-rust-sdk`).
const SDK_USER_AGENT: &str = scm::constants::USER_AGENT;

/// `ReqwestTransport` with the SDK's hardening (no redirects, no retries, bounded
/// headers, gzip only) that never uses a proxy: environment and system proxies are
/// ignored. Used only for admitted plain-http (loopback) endpoints.
struct DirectTransport {
    client: reqwest::Client,
}

impl DirectTransport {
    fn new() -> Option<Self> {
        reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(32)
            .pool_idle_timeout(Duration::from_secs(90))
            .http1_max_headers(128)
            .http2_max_header_list_size(16 * 1024)
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .gzip(true)
            .referer(false)
            .user_agent(SDK_USER_AGENT)
            .build()
            .ok()
            .map(|client| Self { client })
    }
}

impl Transport for DirectTransport {
    fn send<'a>(
        &'a self,
        request: TransportRequest,
    ) -> BoxFuture<'a, scm::Result<TransportResponse>> {
        Box::pin(async move {
            let response = self
                .client
                .request(request.method().clone(), request.url().clone())
                .headers(request.headers().clone())
                .body(request.body().clone())
                .send()
                .await
                .map_err(|_| scm::Error::new(ErrorKind::Transport))?;
            let status = response.status();
            let headers = response.headers().clone();
            let stream = response
                .bytes_stream()
                .map(|chunk| chunk.map_err(|_| scm::Error::new(ErrorKind::Transport)));
            Ok(TransportResponse::new(status, headers, Box::pin(stream)))
        })
    }
}

/// The OAuth provider, recording every lease it hands out in the invocation's
/// [`SecretSet`]. The SDK fetches leases on its own (a 401/403 refresh-and-replay, an
/// expiry during a walk); each one is registered before any error is mapped, so a
/// server that echoes the bearer in a request ID or error code never gets it printed.
pub struct RecordingProvider {
    inner: Arc<OAuthProvider>,
    secrets: SecretSet,
    /// The invocation's API log: token records are held until the lease is registered.
    debug: Option<DebugLog>,
}

impl RecordingProvider {
    /// Fetch or reuse a lease under `options` (the SDK's 30 s refresh budget without).
    pub async fn get_token(&self, options: Option<&RequestOptions>) -> scm::Result<TokenLease> {
        let options = match options {
            Some(options) => options.clone(),
            None => RequestOptions::with_timeout(TOKEN_BUDGET)?,
        };
        self.token(&options).await
    }
}

impl TokenProvider for RecordingProvider {
    fn token<'a>(&'a self, options: &'a RequestOptions) -> BoxFuture<'a, scm::Result<TokenLease>> {
        Box::pin(async move {
            // The token response's record waits until its lease is in the set, so a
            // token echoed in that response's own headers is masked (DESIGN §2.9).
            // Dropping this future drops the hold, which withholds the headers.
            let hold = self.debug.as_ref().map(DebugLog::hold_tokens);
            let result = self.inner.token(options).await;
            if let Ok(lease) = &result {
                self.secrets.add(lease.secret().expose());
            }
            if let Some(hold) = hold {
                hold.release();
            }
            result
        })
    }
    fn context(&self) -> &AuthContext {
        self.inner.context()
    }
    fn invalidate(&self, generation: u64) -> bool {
        self.inner.invalidate(generation)
    }
}

/// The management client and the application-owned OAuth provider behind it.
pub struct ManagementClients {
    pub provider: Arc<RecordingProvider>,
    pub management: ManagementClient,
    /// The DLP client on the admitted `dlpEndpoint` (`Planes { dlp: true }` only; the
    /// same OAuth identity and the 32 MiB response bound, G31).
    pub dlp: Option<scm::dlp::DlpClient>,
    /// The last response status either of them received.
    pub last_status: LastStatus,
}

/// The extra API planes a management client may reach (DESIGN §18.3): each admitted into
/// the one destination policy of the invocation's OAuth identity only for the commands
/// that need it, so no other command's policy widens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Planes {
    /// `mgmtDashboardEndpoint` (else the TypeScript default, which is the management
    /// default URL): the dashboard and sessions routes, sent with `x-tsg-id` by the SDK's
    /// dashboard client (G30).
    pub dashboard: bool,
    /// `dlpEndpoint` (else the SDK default), with the 32 MiB DLP response bound (G31).
    pub dlp: bool,
}

impl ManagementClients {
    /// Build from the effective config. Credentials must already be checked present.
    /// Every OAuth lease is registered in `secrets` (shared with the invocation); every
    /// attempt of the client and its provider is recorded in `debug`.
    pub fn build(
        config: &TenantConfig,
        session: &Session,
        secrets: &SecretSet,
        debug: Option<&DebugLog>,
    ) -> Result<Self, SetupError> {
        Self::build_for(config, session, secrets, debug, Planes::default())
    }

    /// [`ManagementClients::build`] with the extra `planes` admitted (one OAuth identity,
    /// one token per invocation, DESIGN §18.3).
    pub fn build_for(
        config: &TenantConfig,
        session: &Session,
        secrets: &SecretSet,
        debug: Option<&DebugLog>,
        planes: Planes,
    ) -> Result<Self, SetupError> {
        Self::build_for_bounded(config, session, secrets, debug, planes, None)
    }

    pub fn build_for_bounded(
        config: &TenantConfig,
        session: &Session,
        secrets: &SecretSet,
        debug: Option<&DebugLog>,
        planes: Planes,
        budget: Option<&Arc<crate::read_budget::ReadBudget>>,
    ) -> Result<Self, SetupError> {
        let api = admit(
            "mgmtEndpoint",
            config.effective_str("mgmtEndpoint"),
            DEFAULT_MGMT_ENDPOINT,
        )?;
        let issuer = admit(
            "mgmtTokenEndpoint",
            config.effective_str("mgmtTokenEndpoint"),
            DEFAULT_TOKEN_ENDPOINT,
        )?;
        let dashboard = planes
            .dashboard
            .then(|| {
                admit(
                    "mgmtDashboardEndpoint",
                    config.effective_str("mgmtDashboardEndpoint"),
                    DEFAULT_MGMT_ENDPOINT,
                )
            })
            .transpose()?;
        let dlp = planes
            .dlp
            .then(|| {
                admit(
                    "dlpEndpoint",
                    config.effective_str("dlpEndpoint"),
                    scm::constants::DEFAULT_DLP_ENDPOINT,
                )
            })
            .transpose()?;
        let mut apis = vec![api.clone()];
        apis.extend(dashboard.iter().cloned());
        apis.extend(dlp.iter().cloned());
        let last_status = LastStatus::default();
        let OAuthParts {
            provider,
            transport,
            policy,
        } = oauth_parts_bounded(
            config,
            &issuer,
            &apis,
            session,
            secrets,
            debug,
            (&last_status, budget),
        )?;
        let failed =
            || SetupError::sdk("AISEC_SDK_ERROR:The management client could not be configured");
        let mut limits = Limits::default();
        limits.max_retries = MANAGEMENT_RETRIES;
        if planes.dlp {
            limits.max_response_bytes = DLP_MAX_RESPONSE_BYTES;
        }
        let client = Client::builder(api)
            .transport(transport)
            .destination_policy(policy)
            .authentication(Authentication::oauth(
                provider.clone() as Arc<dyn TokenProvider>,
                false,
            ))
            .session(session.clone())
            .limits(limits)
            .build()
            .map_err(|_| failed())?;
        let mut management = ManagementClient::new(client).map_err(|_| failed())?;
        if let Some(dashboard) = dashboard {
            management = management
                .with_dashboard_endpoint(dashboard)
                .map_err(|_| failed())?;
        }
        let dlp = dlp
            .map(|endpoint| management.dlp(endpoint))
            .transpose()
            .map_err(|_| failed())?;
        Ok(Self {
            provider,
            management,
            dlp,
            last_status,
        })
    }

    /// Fetch the OAuth token explicitly so token failures are reported as such.
    /// `budget` is the duration `options` was created with (30 s without options).
    pub async fn authenticate(
        &self,
        options: Option<&RequestOptions>,
        budget: Duration,
    ) -> Result<(), CliError> {
        match self.provider.get_token(options).await {
            Ok(_) => Ok(()),
            Err(failure) => Err(error::from_sdk(
                &failure,
                &SdkContext {
                    plane: Plane::Management,
                    phase: Phase::Token,
                    operation: Operation::Read,
                    timeout: budget,
                    secrets: &self.provider.secrets,
                },
            )),
        }
    }
}

/// The OAuth identity of a management-plane client: the lease-recording provider, the
/// transport (direct when any endpoint is plain http) and one destination policy
/// admitting `apis` and the issuer.
struct OAuthParts {
    provider: Arc<RecordingProvider>,
    transport: Arc<dyn Transport>,
    policy: Arc<dyn DestinationPolicy>,
}

/// Validate `mgmtTsgId`, `mgmtClientId` and `mgmtClientSecret` (§5 messages) and build
/// the [`OAuthParts`] for `apis` (the API bases the client may reach).
fn oauth_parts(
    config: &TenantConfig,
    issuer: &Endpoint,
    apis: &[Endpoint],
    session: &Session,
    secrets: &SecretSet,
    debug: Option<&DebugLog>,
    last_status: &LastStatus,
) -> Result<OAuthParts, SetupError> {
    oauth_parts_bounded(
        config,
        issuer,
        apis,
        session,
        secrets,
        debug,
        (last_status, None),
    )
}
fn oauth_parts_bounded(
    config: &TenantConfig,
    issuer: &Endpoint,
    apis: &[Endpoint],
    session: &Session,
    secrets: &SecretSet,
    debug: Option<&DebugLog>,
    observed: (&LastStatus, Option<&Arc<crate::read_budget::ReadBudget>>),
) -> Result<OAuthParts, SetupError> {
    let (last_status, budget) = observed;
    let tsg = config.effective_str("mgmtTsgId").unwrap_or_default();
    if tsg.is_empty() || tsg.len() > 32 || !tsg.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SetupError::config(
            "mgmtTsgId",
            "Invalid mgmtTsgId: the Rust SDK requires 1-32 ASCII digits",
        ));
    }
    let client_id = config.effective_str("mgmtClientId").unwrap_or_default();
    let context = AuthContext::new(issuer.clone(), client_id, 0, tsg).map_err(|_| {
        SetupError::config(
            "mgmtClientId",
            "Invalid mgmtClientId: the Rust SDK requires at most 1024 bytes without control characters",
        )
    })?;
    let invalid_secret = || {
        SetupError::config(
            "mgmtClientSecret",
            "Invalid mgmtClientSecret: the Rust SDK requires at most 16 KiB without control characters",
        )
    };
    let secret = Secret::new(config.effective_str("mgmtClientSecret").unwrap_or_default())
        .map_err(|_| invalid_secret())?;
    let mut endpoints: Vec<&Endpoint> = apis.iter().collect();
    endpoints.push(issuer);
    let transport = transport(&endpoints, last_status, debug)?;
    let transport = budget.map_or_else(|| transport.clone(), |b| b.wrap(transport.clone()));
    let mut admitted: Vec<Endpoint> = apis.to_vec();
    admitted.push(issuer.clone());
    let policy: Arc<dyn DestinationPolicy> = Arc::new(OriginPolicy::new(admitted));
    let provider = Arc::new(RecordingProvider {
        inner: Arc::new(
            OAuthProvider::new(
                context,
                secret,
                transport.clone(),
                policy.clone(),
                session.clone(),
            )
            .map_err(|_| invalid_secret())?,
        ),
        secrets: secrets.clone(),
        debug: debug.cloned(),
    });
    Ok(OAuthParts {
        provider,
        transport,
        policy,
    })
}

/// The red team client (the management OAuth identity across the data, management and
/// network broker planes, DESIGN §15.3) and the provider behind it.
pub struct RedTeamClients {
    pub provider: Arc<RecordingProvider>,
    pub red_team: scm::airs::red_team::RedTeamClient,
}

impl RedTeamClients {
    /// Build from the effective config (credentials already checked present): the three
    /// red team endpoints and the issuer admitted into one policy, `max_retries = 5`.
    pub fn build(
        config: &TenantConfig,
        session: &Session,
        secrets: &SecretSet,
        debug: Option<&DebugLog>,
    ) -> Result<Self, SetupError> {
        let data = admit(
            "redTeamDataEndpoint",
            config.effective_str("redTeamDataEndpoint"),
            DEFAULT_RED_TEAM_DATA_ENDPOINT,
        )?;
        let management = admit(
            "redTeamMgmtEndpoint",
            config.effective_str("redTeamMgmtEndpoint"),
            DEFAULT_RED_TEAM_MGMT_ENDPOINT,
        )?;
        let broker = admit(
            "redTeamNetworkBrokerEndpoint",
            config.effective_str("redTeamNetworkBrokerEndpoint"),
            DEFAULT_RED_TEAM_BROKER_ENDPOINT,
        )?;
        let issuer = admit(
            "mgmtTokenEndpoint",
            config.effective_str("mgmtTokenEndpoint"),
            DEFAULT_TOKEN_ENDPOINT,
        )?;
        let OAuthParts {
            provider,
            transport,
            policy,
        } = oauth_parts(
            config,
            &issuer,
            &[data.clone(), management.clone(), broker.clone()],
            session,
            secrets,
            debug,
            &LastStatus::default(),
        )?;
        let failed =
            || SetupError::sdk("AISEC_SDK_ERROR:The red team client could not be configured");
        let mut limits = Limits::default();
        limits.max_retries = MANAGEMENT_RETRIES;
        let client = Client::builder(data)
            .transport(transport)
            .destination_policy(policy)
            .authentication(Authentication::oauth(
                provider.clone() as Arc<dyn TokenProvider>,
                false,
            ))
            .session(session.clone())
            .limits(limits)
            .build()
            .map_err(|_| failed())?;
        let red_team = scm::airs::red_team::RedTeamClient::new(client, management, broker)
            .map_err(|_| failed())?;
        Ok(Self { provider, red_team })
    }

    /// Fetch the OAuth token explicitly so token failures are reported as such
    /// (`budget`: the duration `options` was created with, 30 s without options).
    pub async fn authenticate(
        &self,
        options: Option<&RequestOptions>,
        budget: Duration,
    ) -> Result<(), CliError> {
        match self.provider.get_token(options).await {
            Ok(_) => Ok(()),
            Err(failure) => Err(error::from_sdk(
                &failure,
                &SdkContext {
                    plane: Plane::Management,
                    phase: Phase::Token,
                    operation: Operation::Read,
                    timeout: budget,
                    secrets: &self.provider.secrets,
                },
            )),
        }
    }
}

/// The response bound of the DLP client (G13, DESIGN §16.3): a keyword-bearing
/// dictionary page can exceed the SDK's default 8 MiB (validation then allows 400,000
/// nodes).
pub const DLP_MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

/// The DLP client (the management OAuth identity on the tenant's `dlpEndpoint`, DESIGN
/// §16.3) and the provider behind it.
pub struct DlpClients {
    pub provider: Arc<RecordingProvider>,
    pub dlp: scm::dlp::DlpClient,
}

impl DlpClients {
    /// Build from the effective config (credentials already checked present): the
    /// tenant's `dlpEndpoint` (else the SDK default) and the issuer admitted into one
    /// policy, `max_retries = 5` for the replay-safe reads, a 32 MiB response bound and
    /// the default 8 MiB request bound. `mgmtEndpoint` is not used.
    pub fn build(
        config: &TenantConfig,
        session: &Session,
        secrets: &SecretSet,
        debug: Option<&DebugLog>,
    ) -> Result<Self, SetupError> {
        let base = admit(
            "dlpEndpoint",
            config.effective_str("dlpEndpoint"),
            scm::constants::DEFAULT_DLP_ENDPOINT,
        )?;
        let issuer = admit(
            "mgmtTokenEndpoint",
            config.effective_str("mgmtTokenEndpoint"),
            DEFAULT_TOKEN_ENDPOINT,
        )?;
        let OAuthParts {
            provider,
            transport,
            policy,
        } = oauth_parts(
            config,
            &issuer,
            std::slice::from_ref(&base),
            session,
            secrets,
            debug,
            &LastStatus::default(),
        )?;
        let failed = || SetupError::sdk("AISEC_SDK_ERROR:The DLP client could not be configured");
        let mut limits = Limits::default();
        limits.max_retries = MANAGEMENT_RETRIES;
        limits.max_response_bytes = DLP_MAX_RESPONSE_BYTES;
        let client = Client::builder(base)
            .transport(transport)
            .destination_policy(policy)
            .authentication(Authentication::oauth(
                provider.clone() as Arc<dyn TokenProvider>,
                false,
            ))
            .session(session.clone())
            .limits(limits)
            .build()
            .map_err(|_| failed())?;
        let dlp = scm::dlp::DlpClient::new(client).map_err(|_| failed())?;
        Ok(Self { provider, dlp })
    }

    /// Fetch the OAuth token explicitly so token failures are reported as such
    /// (`budget`: the duration `options` was created with, 30 s without options).
    pub async fn authenticate(
        &self,
        options: Option<&RequestOptions>,
        budget: Duration,
    ) -> Result<(), CliError> {
        match self.provider.get_token(options).await {
            Ok(_) => Ok(()),
            Err(failure) => Err(error::from_sdk(
                &failure,
                &SdkContext {
                    plane: Plane::Management,
                    phase: Phase::Token,
                    operation: Operation::Read,
                    timeout: budget,
                    secrets: &self.provider.secrets,
                },
            )),
        }
    }
}

/// The model-security client (the management OAuth identity on the tenant's
/// `modelSecDataEndpoint` and `modelSecMgmtEndpoint`, DESIGN §17.3) and the provider
/// behind it.
pub struct ModelSecurityClients {
    pub provider: Arc<RecordingProvider>,
    pub model_security: scm::airs::model_security::ModelSecurityClient,
}

impl ModelSecurityClients {
    /// Build from the effective config (credentials already checked present): the data
    /// and management bases (else the SDK defaults) and the issuer admitted into one
    /// policy, `max_retries = 5` for the replay-safe reads, the default 8 MiB response
    /// bound. `mgmtEndpoint` is not used; no TSG header is sent (TS parity).
    pub fn build(
        config: &TenantConfig,
        session: &Session,
        secrets: &SecretSet,
        debug: Option<&DebugLog>,
    ) -> Result<Self, SetupError> {
        let data = admit(
            "modelSecDataEndpoint",
            config.effective_str("modelSecDataEndpoint"),
            scm::constants::DEFAULT_MODEL_SEC_DATA_ENDPOINT,
        )?;
        let management = admit(
            "modelSecMgmtEndpoint",
            config.effective_str("modelSecMgmtEndpoint"),
            scm::constants::DEFAULT_MODEL_SEC_MGMT_ENDPOINT,
        )?;
        let issuer = admit(
            "mgmtTokenEndpoint",
            config.effective_str("mgmtTokenEndpoint"),
            DEFAULT_TOKEN_ENDPOINT,
        )?;
        let OAuthParts {
            provider,
            transport,
            policy,
        } = oauth_parts(
            config,
            &issuer,
            &[data.clone(), management.clone()],
            session,
            secrets,
            debug,
            &LastStatus::default(),
        )?;
        let failed =
            || SetupError::sdk("AISEC_SDK_ERROR:The model security client could not be configured");
        let mut limits = Limits::default();
        limits.max_retries = MANAGEMENT_RETRIES;
        let client = Client::builder(data.clone())
            .transport(transport)
            .destination_policy(policy)
            .authentication(Authentication::oauth(
                provider.clone() as Arc<dyn TokenProvider>,
                false,
            ))
            .session(session.clone())
            .limits(limits)
            .build()
            .map_err(|_| failed())?;
        let model_security =
            scm::airs::model_security::ModelSecurityClient::new(client, data, management)
                .map_err(|_| failed())?;
        Ok(Self {
            provider,
            model_security,
        })
    }

    /// Fetch the OAuth token explicitly so token failures are reported as such
    /// (`budget`: the duration `options` was created with, 30 s without options).
    pub async fn authenticate(
        &self,
        options: Option<&RequestOptions>,
        budget: Duration,
    ) -> Result<(), CliError> {
        match self.provider.get_token(options).await {
            Ok(_) => Ok(()),
            Err(failure) => Err(error::from_sdk(
                &failure,
                &SdkContext {
                    plane: Plane::Management,
                    phase: Phase::Token,
                    operation: Operation::Read,
                    timeout: budget,
                    secrets: &self.provider.secrets,
                },
            )),
        }
    }
}

/// The gateway management client (DESIGN §19.3): the data, admin and IAM planes on one
/// management OAuth identity with the `x-tsg-id` header (TS parity), and the provider
/// behind it.
pub struct GatewayClients {
    pub provider: Arc<RecordingProvider>,
    pub gateway: scm::airs::gateway::AIGatewayClient,
}

impl GatewayClients {
    /// Build from the effective config (credentials already checked present): the
    /// tenant's `gatewayDataEndpoint`, `gatewayAdminEndpoint` and `iamEndpoint` (else the
    /// SDK defaults) and the issuer admitted into one policy, `max_retries = 5` for the
    /// replay-safe reads, the default 8 MiB response bound. `mgmtEndpoint` is not used.
    pub fn build(
        config: &TenantConfig,
        session: &Session,
        secrets: &SecretSet,
        debug: Option<&DebugLog>,
    ) -> Result<Self, SetupError> {
        let data = admit(
            "gatewayDataEndpoint",
            config.effective_str("gatewayDataEndpoint"),
            scm::constants::DEFAULT_AI_GW_DATA_ENDPOINT,
        )?;
        let admin = admit(
            "gatewayAdminEndpoint",
            config.effective_str("gatewayAdminEndpoint"),
            scm::constants::DEFAULT_AI_GW_ADMIN_ENDPOINT,
        )?;
        let iam = admit(
            "iamEndpoint",
            config.effective_str("iamEndpoint"),
            scm::constants::DEFAULT_IAM_ENDPOINT,
        )?;
        let issuer = admit(
            "mgmtTokenEndpoint",
            config.effective_str("mgmtTokenEndpoint"),
            DEFAULT_TOKEN_ENDPOINT,
        )?;
        let OAuthParts {
            provider,
            transport,
            policy,
        } = oauth_parts(
            config,
            &issuer,
            &[data.clone(), admin.clone(), iam.clone()],
            session,
            secrets,
            debug,
            &LastStatus::default(),
        )?;
        let failed =
            || SetupError::sdk("AISEC_SDK_ERROR:The gateway client could not be configured");
        let mut limits = Limits::default();
        limits.max_retries = MANAGEMENT_RETRIES;
        let client = Client::builder(data)
            .transport(transport)
            .destination_policy(policy)
            .authentication(Authentication::oauth(
                provider.clone() as Arc<dyn TokenProvider>,
                false,
            ))
            .session(session.clone())
            .limits(limits)
            .build()
            .map_err(|_| failed())?;
        let gateway =
            scm::airs::gateway::AIGatewayClient::new(client, admin, iam).map_err(|_| failed())?;
        Ok(Self { provider, gateway })
    }

    /// Fetch the OAuth token explicitly so token failures are reported as such
    /// (`budget`: the duration `options` was created with, 30 s without options).
    pub async fn authenticate(
        &self,
        options: Option<&RequestOptions>,
        budget: Duration,
    ) -> Result<(), CliError> {
        match self.provider.get_token(options).await {
            Ok(_) => Ok(()),
            Err(failure) => Err(error::from_sdk(
                &failure,
                &SdkContext {
                    plane: Plane::Management,
                    phase: Phase::Token,
                    operation: Operation::Read,
                    timeout: budget,
                    secrets: &self.provider.secrets,
                },
            )),
        }
    }
}

/// The response bound of every supplychain answer (DESIGN §20.3, G66, D320): the unpaged
/// finding list carries scanned code and can exceed the SDK's 8 MiB default.
pub const SUPPLYCHAIN_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

/// The JSON value and validation-work bounds of every supplychain answer (DESIGN §20.3,
/// G66, D320): the SDK's maxima. The engine would scale them from the 32 MiB bound to
/// 400,000 values (`ValidationLimits::for_bytes`), which refuses an ordinary finding list
/// of about 15 MB; the SDK's decoder caps a document at 1,000,000 values whatever is set.
pub const SUPPLYCHAIN_MAX_VALUES: usize = 1_000_000;
pub const SUPPLYCHAIN_MAX_WORK: usize = 16_000_000;

/// The validation bounds of [`SupplychainClients`]: the 32 MiB input bound with
/// [`SUPPLYCHAIN_MAX_VALUES`] and [`SUPPLYCHAIN_MAX_WORK`].
pub fn supplychain_validation_limits() -> scm::Result<scm::validation::ValidationLimits> {
    let mut limits = scm::validation::ValidationLimits::for_bytes(SUPPLYCHAIN_RESPONSE_BYTES)?;
    limits.max_nodes = SUPPLYCHAIN_MAX_VALUES;
    limits.max_work = SUPPLYCHAIN_MAX_WORK;
    Ok(limits)
}

/// The supplychain client (DESIGN §20.3): the AgentGuard data and management planes on one
/// management OAuth identity with the `x-tsg-id` header (TS parity), and the provider
/// behind it.
pub struct SupplychainClients {
    pub provider: Arc<RecordingProvider>,
    pub agentguard: scm::airs::agentguard::AgentGuardClient,
    /// The validation bounds the client was built with ([`supplychain_validation_limits`],
    /// computed once per command): the raw scan read checks its answers against them
    /// (G66, `supplychain::scans::within_bounds`).
    pub validation: scm::validation::ValidationLimits,
}

impl SupplychainClients {
    /// Build from the effective config (credentials already checked present): the
    /// tenant's `supplychainDataEndpoint` and `supplychainMgmtEndpoint` (else the SDK
    /// defaults) and the issuer admitted into one policy, the 32 MiB response bound (G66).
    /// Retries are set per call (`with_retries(0)`, §20.3), never here.
    pub fn build(
        config: &TenantConfig,
        session: &Session,
        secrets: &SecretSet,
        debug: Option<&DebugLog>,
    ) -> Result<Self, SetupError> {
        let data = admit(
            "supplychainDataEndpoint",
            config.effective_str("supplychainDataEndpoint"),
            scm::constants::AGENT_GUARD_DATA_ENDPOINT,
        )?;
        let management = admit(
            "supplychainMgmtEndpoint",
            config.effective_str("supplychainMgmtEndpoint"),
            scm::constants::AGENT_GUARD_MGMT_ENDPOINT,
        )?;
        let issuer = admit(
            "mgmtTokenEndpoint",
            config.effective_str("mgmtTokenEndpoint"),
            DEFAULT_TOKEN_ENDPOINT,
        )?;
        let OAuthParts {
            provider,
            transport,
            policy,
        } = oauth_parts(
            config,
            &issuer,
            &[data.clone(), management.clone()],
            session,
            secrets,
            debug,
            &LastStatus::default(),
        )?;
        let failed =
            || SetupError::sdk("AISEC_SDK_ERROR:The supplychain client could not be configured");
        let mut limits = Limits::default();
        limits.max_response_bytes = SUPPLYCHAIN_RESPONSE_BYTES;
        let validation = supplychain_validation_limits().map_err(|_| failed())?;
        let client = Client::builder(data.clone())
            .validation_limits(validation.clone())
            .transport(transport)
            .destination_policy(policy)
            .authentication(Authentication::oauth(
                provider.clone() as Arc<dyn TokenProvider>,
                false,
            ))
            .session(session.clone())
            .limits(limits)
            .build()
            .map_err(|_| failed())?;
        let agentguard = scm::airs::agentguard::AgentGuardClient::new(client, data, management)
            .map_err(|_| failed())?;
        Ok(Self {
            provider,
            agentguard,
            validation,
        })
    }

    /// Fetch the OAuth token before the first read (DESIGN §20.3), under the read's
    /// options; the SDK error is returned whole, so the command prints TypeScript's
    /// request failure line with the token answer's status (choice 4).
    pub async fn authenticate(&self, options: Option<&RequestOptions>) -> scm::Result<()> {
        self.provider.get_token(options).await.map(|_| ())
    }
}

/// The doctor's `Gateway API` probe client (DESIGN §19.16, choice 52): the tenant's
/// `gatewayDataEndpoint` (else the SDK default) on the `Management OAuth` probe's
/// provider, so both probes share one token; `x-tsg-id` is forced by the gateway client.
/// Only the data base is admitted (the probe reads nothing else; the admin and IAM bases
/// of the client are the data base and are never used). Also returns the status of the
/// last response it received.
pub fn gateway_probe(
    config: &TenantConfig,
    provider: Arc<RecordingProvider>,
    session: &Session,
    debug: Option<&DebugLog>,
) -> Result<(scm::airs::gateway::AIGatewayClient, LastStatus), SetupError> {
    let data = admit(
        "gatewayDataEndpoint",
        config.effective_str("gatewayDataEndpoint"),
        scm::constants::DEFAULT_AI_GW_DATA_ENDPOINT,
    )?;
    let last_status = LastStatus::default();
    let transport = transport(&[&data], &last_status, debug)?;
    let failed = || SetupError::sdk("AISEC_SDK_ERROR:The gateway client could not be configured");
    let client = Client::builder(data.clone())
        .transport(transport)
        .destination_policy(Arc::new(OriginPolicy::new([data.clone()])))
        .authentication(Authentication::oauth(
            provider as Arc<dyn TokenProvider>,
            false,
        ))
        .session(session.clone())
        .build()
        .map_err(|_| failed())?;
    let gateway = scm::airs::gateway::AIGatewayClient::new(client, data.clone(), data)
        .map_err(|_| failed())?;
    Ok((gateway, last_status))
}

/// The gateway runtime client of `gateway inference` (DESIGN §19.14, D240, G48): the
/// runtime-key plane only (no OAuth, no `x-tsg-id`), on `--endpoint` or the tenant's
/// `gatewayInferenceEndpoint`, with the tenant's `gatewayInferenceApiKey` sent as
/// `x-portkey-api-key`. No environment variable is read.
pub struct GatewayRuntime {
    pub runtime: scm::airs::gateway::RuntimeClient,
}

impl GatewayRuntime {
    /// Build from the effective config; `endpoint` is the admitted `--endpoint`. The key
    /// joins `secrets` before anything can print it.
    pub fn build(
        config: &TenantConfig,
        tenant: &str,
        endpoint: Option<&Endpoint>,
        session: &Session,
        secrets: &SecretSet,
        debug: Option<&DebugLog>,
    ) -> Result<Self, CliError> {
        let endpoint = match endpoint {
            Some(endpoint) => endpoint.clone(),
            None => match config
                .effective_str("gatewayInferenceEndpoint")
                .filter(|value| !js_trim(value).is_empty())
            {
                Some(configured) => admit("gatewayInferenceEndpoint", Some(configured), "")?,
                None => {
                    return Err(CliError::operational(format!(
                        "gatewayInferenceEndpoint is not configured; set it with '{} tenant set {tenant} gatewayInferenceEndpoint <url>' or pass --endpoint",
                        crate::store::PROGRAM
                    )));
                }
            },
        };
        let key = config
            .effective_str("gatewayInferenceApiKey")
            .filter(|value| !js_trim(value).is_empty())
            .ok_or_else(|| {
                CliError::operational(format!(
                    "gatewayInferenceApiKey is not configured; set it with '{} tenant set {tenant} gatewayInferenceApiKey' (secrets prompt hidden, or use --stdin)",
                    crate::store::PROGRAM
                ))
            })?;
        secrets.add(key);
        let invalid_key = || {
            CliError::operational(
                "Invalid gatewayInferenceApiKey: it must be a nonempty header value on one line",
            )
        };
        if key.contains(['\r', '\n']) {
            return Err(invalid_key());
        }
        let authentication = Secret::new(key)
            .and_then(Authentication::runtime_key)
            .map_err(|_| invalid_key())?;
        let transport = transport(&[&endpoint], &LastStatus::default(), debug)?;
        let runtime = runtime_client(endpoint, transport, authentication, session)?;
        Ok(Self { runtime })
    }
}

/// The construction failure of the gateway runtime client (DESIGN §19.14, §19.18).
pub const GATEWAY_RUNTIME_FAILED: &str =
    "AISEC_SDK_ERROR:The gateway runtime client could not be configured";

/// The runtime client of `endpoint` (only that origin admitted) with `authentication`; any
/// SDK construction failure (the client builder, or an authentication that is not a
/// runtime key) is [`GATEWAY_RUNTIME_FAILED`], exit 1.
fn runtime_client(
    endpoint: Endpoint,
    transport: Arc<dyn Transport>,
    authentication: Authentication,
    session: &Session,
) -> Result<scm::airs::gateway::RuntimeClient, CliError> {
    let failed = |_| CliError::operational(GATEWAY_RUNTIME_FAILED);
    let client = Client::builder(endpoint.clone())
        .transport(transport)
        .destination_policy(Arc::new(OriginPolicy::new([endpoint])))
        .authentication(authentication)
        .session(session.clone())
        .build()
        .map_err(failed)?;
    scm::airs::gateway::RuntimeClient::new(client).map_err(failed)
}

/// A trimmed scanner credential, checked against the TypeScript and SDK limits.
fn scanner_credential(
    config: &TenantConfig,
    key: &'static str,
    label: &str,
) -> Result<Option<Secret>, SetupError> {
    let Some(value) = config
        .effective_str(key)
        .map(js_trim)
        .filter(|v| !v.is_empty())
    else {
        return Ok(None);
    };
    if value.encode_utf16().nth(MAX_SCANNER_CREDENTIAL).is_some() {
        return Err(SetupError::config(
            key,
            format!(
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:{label} exceeds max length of {MAX_SCANNER_CREDENTIAL}"
            ),
        ));
    }
    if !value.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(SetupError::config(
            key,
            format!("Invalid {key}: the Rust SDK requires printable ASCII without spaces"),
        ));
    }
    Secret::new(value)
        .map(Some)
        .map_err(|_| SetupError::config(key, format!("Invalid {key}")))
}

/// Build the scanner through `scm::configuration::init` (TS `init()` semantics); every
/// attempt is recorded in `debug`.
pub fn build_scanner(
    config: &TenantConfig,
    session: &Session,
    debug: Option<&DebugLog>,
) -> Result<Scanner, SetupError> {
    build_observed_scanner(config, session, debug).map(|(scanner, _)| scanner)
}

/// [`build_scanner`], plus the status of the last response it receives.
pub fn build_observed_scanner(
    config: &TenantConfig,
    session: &Session,
    debug: Option<&DebugLog>,
) -> Result<(Scanner, LastStatus), SetupError> {
    build_scanner_bounded(config, session, debug, None)
}
pub fn build_scanner_bounded(
    config: &TenantConfig,
    session: &Session,
    debug: Option<&DebugLog>,
    budget: Option<&Arc<crate::read_budget::ReadBudget>>,
) -> Result<(Scanner, LastStatus), SetupError> {
    let endpoint = admit(
        "airsApiEndpoint",
        config.effective_str("airsApiEndpoint"),
        DEFAULT_SCAN_ENDPOINT,
    )?;
    let api_key = scanner_credential(config, "airsApiKey", "apiKey")?;
    let api_token = scanner_credential(config, "airsApiToken", "apiToken")?;
    let mut options = scm::configuration::InitOptions::default();
    options.api_key = api_key;
    options.api_token = api_token;
    options.num_retries = config.num_retries();
    let last_status = LastStatus::default();
    let transport = transport(&[&endpoint], &last_status, debug)?;
    let transport = budget.map_or_else(|| transport.clone(), |b| b.wrap(transport.clone()));
    options.api_endpoint = Some(endpoint);
    let configuration =
        scm::configuration::init(options).map_err(|_| invalid_endpoint("airsApiEndpoint"))?;
    let failed = || SetupError::sdk("AISEC_SDK_ERROR:The scanner client could not be configured");
    let client = configuration
        .client_builder()
        .map_err(|_| failed())?
        .transport(transport)
        .session(session.clone())
        .build()
        .map_err(|_| failed())?;
    Ok((Scanner::new(client).map_err(|_| failed())?, last_status))
}

/// One read, and one more after an HTTP 401 or 403 rejection (G7: TypeScript's free token
/// replay with `numRetries: 0`; a read duplicates nothing, choice 30). Used by the
/// dashboard and sessions reads and by both environment reports.
pub async fn reread<T, F, Fut>(mut call: F) -> scm::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = scm::Result<T>>,
{
    match call().await {
        Err(error)
            if error.kind() == ErrorKind::Http
                && matches!(error.status(), Some(401 | 403))
                && error.remote_outcome() == scm::RemoteOutcome::Rejected =>
        {
            call().await
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_retry_after_honors_seconds_dates_and_never_rounds_a_future_date_early() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000) + Duration::from_millis(500);
        assert_eq!(retry_after_seconds(Some("120"), now), Some(120));
        assert_eq!(
            retry_after_seconds(Some("9999999999999999999999999999999"), now),
            Some(86400)
        );
        let future = httpdate::fmt_http_date(now + Duration::from_secs(3600));
        assert_eq!(retry_after_seconds(Some(&future), now), Some(3600));
        let near = httpdate::fmt_http_date(UNIX_EPOCH + Duration::from_secs(1_700_000_001));
        assert_eq!(retry_after_seconds(Some(&near), now), Some(1));
        for invalid in ["0", "-1", "invalid", ""] {
            assert_eq!(retry_after_seconds(Some(invalid), now), None);
        }
        assert_eq!(
            retry_after_seconds(
                Some(&httpdate::fmt_http_date(now - Duration::from_secs(1))),
                now
            ),
            None
        );
        assert_eq!(retry_after_seconds(None, SystemTime::now()), None);
    }
    use crate::tenant::config::{ConfigValue, schema_key};

    /// A tenant config from a JSON object of strings and integers (test shorthand).
    fn config(text: &str) -> TenantConfig {
        let serde_json::Value::Object(entries) = serde_json::from_str(text).unwrap() else {
            panic!("object expected");
        };
        TenantConfig::from_values(entries.into_iter().map(|(key, value)| {
            let key = schema_key(&key).unwrap();
            let value = match value {
                serde_json::Value::String(text) => ConfigValue::Text(text),
                serde_json::Value::Number(number) => ConfigValue::Integer(number.as_i64().unwrap()),
                other => panic!("unsupported {other}"),
            };
            (key, value)
        }))
    }

    /// A runtime client that the SDK refuses to build (here an authentication that is not a
    /// runtime key) is the §19.18 construction text, exit 1.
    #[test]
    fn a_runtime_client_failure_is_the_configuration_text() {
        let endpoint = admit(
            "gatewayInferenceEndpoint",
            Some("http://127.0.0.1:9/v1"),
            "",
        )
        .unwrap();
        let transport = transport(&[&endpoint], &LastStatus::default(), None).unwrap();
        let scan_key = Authentication::scan(Some(Secret::new("k-1").unwrap()), None).unwrap();
        let error = runtime_client(endpoint, transport, scan_key, &Session::new())
            .err()
            .unwrap();
        assert_eq!(
            error,
            CliError::operational(
                "AISEC_SDK_ERROR:The gateway runtime client could not be configured"
            )
        );
        assert_eq!(error.message(), GATEWAY_RUNTIME_FAILED);
    }

    #[test]
    fn endpoint_admission() {
        for url in [
            "https://example.com/aisec",
            "http://127.0.0.1:8080/aisec",
            "http://127.9.9.9",
            "http://localhost:1",
            "http://[::1]:9/x",
        ] {
            assert!(
                admit("mgmtEndpoint", Some(url), DEFAULT_MGMT_ENDPOINT).is_ok(),
                "{url}"
            );
        }
        for url in [
            "http://example.test",
            "http://0.0.0.0:1",
            "https://user:pw@example.com",
            "https://example.com/?q=1",
            "https://example.com/#f",
            "not a url",
        ] {
            let error = admit("mgmtEndpoint", Some(url), DEFAULT_MGMT_ENDPOINT).unwrap_err();
            assert_eq!(error.key, Some("mgmtEndpoint"));
            assert!(!error.message().contains(url), "the value is never echoed");
            assert!(
                error
                    .message()
                    .starts_with("Invalid mgmtEndpoint: endpoints must use https")
            );
        }
        assert!(admit("x", None, DEFAULT_SCAN_ENDPOINT).is_ok());
    }

    #[test]
    fn management_configuration_errors_are_local() {
        let session = Session::new();
        let secrets = SecretSet::default();
        let bad_tsg = config(r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"abc"}"#);
        let error = ManagementClients::build(&bad_tsg, &session, &secrets, None)
            .err()
            .unwrap();
        assert_eq!(
            error.message(),
            "Invalid mgmtTsgId: the Rust SDK requires 1-32 ASCII digits"
        );
        let bad_endpoint = config(
            r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","mgmtTokenEndpoint":"http://example.test/t"}"#,
        );
        let error = ManagementClients::build(&bad_endpoint, &session, &secrets, None)
            .err()
            .unwrap();
        assert_eq!(error.key, Some("mgmtTokenEndpoint"));
        let control = config(r#"{"mgmtClientId":"c\n","mgmtClientSecret":"s","mgmtTsgId":"1"}"#);
        assert_eq!(
            ManagementClients::build(&control, &session, &secrets, None)
                .err()
                .unwrap()
                .key,
            Some("mgmtClientId")
        );
        let good = config(
            r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","mgmtEndpoint":"http://127.0.0.1:1/aisec","mgmtTokenEndpoint":"http://127.0.0.1:1/token"}"#,
        );
        assert!(ManagementClients::build(&good, &session, &secrets, None).is_ok());
    }

    #[test]
    fn red_team_clients_admit_the_three_planes_or_their_defaults() {
        let session = Session::new();
        let secrets = SecretSet::default();
        let defaults = config(r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1"}"#);
        assert!(RedTeamClients::build(&defaults, &session, &secrets, None).is_ok());
        let loopback = config(
            r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","redTeamDataEndpoint":"http://127.0.0.1:1/d","redTeamMgmtEndpoint":"http://127.0.0.1:1/m","redTeamNetworkBrokerEndpoint":"http://127.0.0.1:1/b","mgmtTokenEndpoint":"http://127.0.0.1:1/t"}"#,
        );
        assert!(RedTeamClients::build(&loopback, &session, &secrets, None).is_ok());
        for key in [
            "redTeamDataEndpoint",
            "redTeamMgmtEndpoint",
            "redTeamNetworkBrokerEndpoint",
        ] {
            let bad = config(&format!(
                r#"{{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","{key}":"http://example.test/x"}}"#
            ));
            let error = RedTeamClients::build(&bad, &session, &secrets, None)
                .err()
                .unwrap();
            assert_eq!(error.key, Some(key));
        }
        let bad_tsg = config(r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"x"}"#);
        assert_eq!(
            RedTeamClients::build(&bad_tsg, &session, &secrets, None)
                .err()
                .unwrap()
                .key,
            Some("mgmtTsgId")
        );
    }

    /// scm-sdk 0.2.3/0.2.4 keep the `api.apps` defaults and migrate nothing to
    /// `api.sase`; the CLI does the same (§15.24, §15.25): without a tenant value each red
    /// team plane is the SDK's `api.apps` default, and a configured endpoint, an
    /// `api.sase` one included, is used exactly as written.
    #[test]
    fn red_team_endpoints_are_the_defaults_or_the_tenant_values_unchanged() {
        let session = Session::new();
        let secrets = SecretSet::default();
        let planes = |clients: &RedTeamClients| {
            let url = |client: &Client| client.endpoint().url().as_str().to_owned();
            [
                url(clients.red_team.client()),
                url(clients.red_team.targets().client()),
            ]
        };
        let defaults = config(r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1"}"#);
        let clients = RedTeamClients::build(&defaults, &session, &secrets, None).unwrap();
        let [data, management] = planes(&clients);
        assert_eq!(
            data.trim_end_matches('/'),
            scm::constants::DEFAULT_RED_TEAM_DATA_ENDPOINT
        );
        assert_eq!(
            management.trim_end_matches('/'),
            scm::constants::DEFAULT_RED_TEAM_MGMT_ENDPOINT
        );
        for (ours, sdk) in [
            (
                DEFAULT_RED_TEAM_DATA_ENDPOINT,
                scm::constants::DEFAULT_RED_TEAM_DATA_ENDPOINT,
            ),
            (
                DEFAULT_RED_TEAM_MGMT_ENDPOINT,
                scm::constants::DEFAULT_RED_TEAM_MGMT_ENDPOINT,
            ),
            (
                DEFAULT_RED_TEAM_BROKER_ENDPOINT,
                scm::constants::DEFAULT_RED_TEAM_NETWORK_BROKER_ENDPOINT,
            ),
            (DEFAULT_MGMT_ENDPOINT, scm::constants::DEFAULT_MGMT_ENDPOINT),
        ] {
            assert_eq!(ours, sdk);
            assert!(ours.starts_with("https://api.apps.paloaltonetworks.com/"));
        }
        let sase = config(
            r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","redTeamDataEndpoint":"https://api.sase.paloaltonetworks.com/ai-red-teaming/data","redTeamMgmtEndpoint":"https://api.sase.paloaltonetworks.com/ai-red-teaming/mgmt"}"#,
        );
        let clients = RedTeamClients::build(&sase, &session, &secrets, None).unwrap();
        let [data, management] = planes(&clients);
        assert_eq!(
            data.trim_end_matches('/'),
            "https://api.sase.paloaltonetworks.com/ai-red-teaming/data"
        );
        assert_eq!(
            management.trim_end_matches('/'),
            "https://api.sase.paloaltonetworks.com/ai-red-teaming/mgmt"
        );
    }

    /// DESIGN §16.3: the DLP base is only the tenant's `dlpEndpoint`, else the SDK
    /// default (TS `client-wiring.spec.ts`); a bad value names the key.
    #[test]
    fn dlp_clients_admit_the_tenant_endpoint_or_the_default() {
        let session = Session::new();
        let secrets = SecretSet::default();
        let defaults = config(r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1"}"#);
        let clients = DlpClients::build(&defaults, &session, &secrets, None).unwrap();
        assert_eq!(
            clients
                .dlp
                .client()
                .endpoint()
                .url()
                .as_str()
                .trim_end_matches('/'),
            scm::constants::DEFAULT_DLP_ENDPOINT
        );
        let loopback = config(
            r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","dlpEndpoint":"http://127.0.0.1:1/dlp/","mgmtEndpoint":"http://example.test/not-used","mgmtTokenEndpoint":"http://127.0.0.1:1/t"}"#,
        );
        let clients = DlpClients::build(&loopback, &session, &secrets, None).unwrap();
        assert!(
            clients
                .dlp
                .client()
                .endpoint()
                .url()
                .as_str()
                .starts_with("http://127.0.0.1:1/dlp")
        );
        let bad = config(
            r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","dlpEndpoint":"http://dlp.example.test/x"}"#,
        );
        let error = DlpClients::build(&bad, &session, &secrets, None)
            .err()
            .unwrap();
        assert_eq!(error.key, Some("dlpEndpoint"));
        assert!(!error.message().contains("example.test"));
        let bad_tsg = config(r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"x"}"#);
        assert_eq!(
            DlpClients::build(&bad_tsg, &session, &secrets, None)
                .err()
                .unwrap()
                .key,
            Some("mgmtTsgId")
        );
    }

    /// DESIGN §17.3: the model-security bases are only the tenant's
    /// `modelSecDataEndpoint`/`modelSecMgmtEndpoint`, else the SDK defaults (TS
    /// `client-options.spec.ts`); a bad value names the key, never the value.
    #[test]
    fn model_security_clients_admit_the_tenant_endpoints_or_the_defaults() {
        let session = Session::new();
        let secrets = SecretSet::default();
        let base = |client: &scm::Client| {
            client
                .endpoint()
                .url()
                .as_str()
                .trim_end_matches('/')
                .to_owned()
        };
        let defaults = config(r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1"}"#);
        let clients = ModelSecurityClients::build(&defaults, &session, &secrets, None).unwrap();
        assert_eq!(
            base(clients.model_security.data_client()),
            scm::constants::DEFAULT_MODEL_SEC_DATA_ENDPOINT
        );
        assert_eq!(
            base(clients.model_security.management_client()),
            scm::constants::DEFAULT_MODEL_SEC_MGMT_ENDPOINT
        );
        let loopback = config(
            r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","modelSecDataEndpoint":"http://127.0.0.1:1/aims/data/","modelSecMgmtEndpoint":"http://127.0.0.1:1/aims/mgmt","mgmtEndpoint":"http://example.test/not-used","mgmtTokenEndpoint":"http://127.0.0.1:1/t"}"#,
        );
        let clients = ModelSecurityClients::build(&loopback, &session, &secrets, None).unwrap();
        assert_eq!(
            base(clients.model_security.data_client()),
            "http://127.0.0.1:1/aims/data"
        );
        assert_eq!(
            base(clients.model_security.management_client()),
            "http://127.0.0.1:1/aims/mgmt"
        );
        for key in ["modelSecDataEndpoint", "modelSecMgmtEndpoint"] {
            let bad = config(&format!(
                r#"{{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","{key}":"http://ms.example.test/x"}}"#
            ));
            let error = ModelSecurityClients::build(&bad, &session, &secrets, None)
                .err()
                .unwrap();
            assert_eq!(error.key, Some(key));
            assert!(!error.message().contains("example.test"));
        }
    }

    /// DESIGN §18.3 (TS `client-options.spec.ts:113-133`): the dashboard and DLP planes are
    /// admitted by their own keys (the dashboard default is TS's, not `mgmtEndpoint`), only
    /// for the commands that ask for them; an inadmissible value is named by its key.
    #[test]
    fn management_planes_are_admitted_only_when_asked_for() {
        let session = Session::new();
        let secrets = SecretSet::default();
        let defaults = config(r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1"}"#);
        let both = Planes {
            dashboard: true,
            dlp: true,
        };
        assert!(ManagementClients::build_for(&defaults, &session, &secrets, None, both).is_ok());
        for (key, planes) in [
            (
                "mgmtDashboardEndpoint",
                Planes {
                    dashboard: true,
                    dlp: false,
                },
            ),
            (
                "dlpEndpoint",
                Planes {
                    dashboard: false,
                    dlp: true,
                },
            ),
        ] {
            let bad = config(&format!(
                r#"{{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","{key}":"http://plane.example.test/x"}}"#
            ));
            let error = ManagementClients::build_for(&bad, &session, &secrets, None, planes)
                .err()
                .unwrap();
            assert_eq!(error.key, Some(key));
            assert!(!error.message().contains("example.test"));
            // Without the plane the key is not read (no command's policy widens).
            assert!(ManagementClients::build(&bad, &session, &secrets, None).is_ok());
        }
        assert_eq!(
            DEFAULT_MGMT_ENDPOINT,
            "https://api.apps.paloaltonetworks.com/aisec"
        );
    }

    /// DESIGN §19.3: the gateway bases are only the tenant's `gatewayDataEndpoint`,
    /// `gatewayAdminEndpoint` and `iamEndpoint`, else the SDK defaults (TS
    /// `client-options.ts:79-86`); a bad value names the key, never the value.
    #[test]
    fn gateway_clients_admit_the_three_planes_or_their_defaults() {
        let session = Session::new();
        let secrets = SecretSet::default();
        let defaults = config(r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1"}"#);
        let clients = GatewayClients::build(&defaults, &session, &secrets, None).unwrap();
        assert_eq!(
            clients
                .gateway
                .client()
                .endpoint()
                .url()
                .as_str()
                .trim_end_matches('/'),
            scm::constants::DEFAULT_AI_GW_DATA_ENDPOINT
        );
        let loopback = config(
            r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","gatewayDataEndpoint":"http://127.0.0.1:1/ai_gw/v2/","gatewayAdminEndpoint":"http://127.0.0.1:1/ai_gw/admin/v2","iamEndpoint":"http://127.0.0.1:1/iam","mgmtEndpoint":"http://example.test/not-used","mgmtTokenEndpoint":"http://127.0.0.1:1/t"}"#,
        );
        let clients = GatewayClients::build(&loopback, &session, &secrets, None).unwrap();
        assert!(
            clients
                .gateway
                .client()
                .endpoint()
                .url()
                .as_str()
                .starts_with("http://127.0.0.1:1/ai_gw/v2")
        );
        for key in ["gatewayDataEndpoint", "gatewayAdminEndpoint", "iamEndpoint"] {
            let bad = config(&format!(
                r#"{{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","{key}":"http://gw.example.test/x"}}"#
            ));
            let error = GatewayClients::build(&bad, &session, &secrets, None)
                .err()
                .unwrap();
            assert_eq!(error.key, Some(key));
            assert!(!error.message().contains("example.test"));
        }
        let bad_tsg = config(r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"x"}"#);
        assert_eq!(
            GatewayClients::build(&bad_tsg, &session, &secrets, None)
                .err()
                .unwrap()
                .key,
            Some("mgmtTsgId")
        );
    }

    /// DESIGN §20.3: the supplychain planes are the tenant's `supplychainDataEndpoint` and
    /// `supplychainMgmtEndpoint`, else the SDK's AgentGuard defaults (TS
    /// `client-options.ts:53-59`); a bad value names the key, never the value; the data
    /// client carries the 32 MiB response bound (G66).
    #[test]
    fn supplychain_clients_admit_both_planes_or_their_defaults() {
        let session = Session::new();
        let secrets = SecretSet::default();
        let defaults = config(r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1"}"#);
        let clients = SupplychainClients::build(&defaults, &session, &secrets, None).unwrap();
        let base = |client: &Client| {
            client
                .endpoint()
                .url()
                .as_str()
                .trim_end_matches('/')
                .to_owned()
        };
        assert_eq!(
            base(clients.agentguard.data_client()),
            scm::constants::AGENT_GUARD_DATA_ENDPOINT
        );
        assert_eq!(
            base(clients.agentguard.management_client()),
            scm::constants::AGENT_GUARD_MGMT_ENDPOINT
        );
        let loopback = config(
            r#"{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","supplychainDataEndpoint":"http://127.0.0.1:1/agentguard/data/","supplychainMgmtEndpoint":"http://127.0.0.1:1/agentguard/mgmt","mgmtEndpoint":"http://example.test/not-used","mgmtTokenEndpoint":"http://127.0.0.1:1/t"}"#,
        );
        let clients = SupplychainClients::build(&loopback, &session, &secrets, None).unwrap();
        assert_eq!(
            base(clients.agentguard.data_client()),
            "http://127.0.0.1:1/agentguard/data"
        );
        assert_eq!(
            base(clients.agentguard.management_client()),
            "http://127.0.0.1:1/agentguard/mgmt"
        );
        for key in [
            "supplychainDataEndpoint",
            "supplychainMgmtEndpoint",
            "mgmtTokenEndpoint",
        ] {
            let bad = config(&format!(
                r#"{{"mgmtClientId":"c","mgmtClientSecret":"s","mgmtTsgId":"1","{key}":"http://sc.example.test/x"}}"#
            ));
            let error = SupplychainClients::build(&bad, &session, &secrets, None)
                .err()
                .unwrap();
            assert_eq!(error.key, Some(key));
            assert!(error.message().starts_with(&format!("Invalid {key}: ")));
            assert!(!error.message().contains("example.test"));
        }
        assert_eq!(SUPPLYCHAIN_RESPONSE_BYTES, 33_554_432);
        // G66: the SDK's maxima, not the 400,000 values it would scale from 32 MiB.
        let validation = supplychain_validation_limits().unwrap();
        assert_eq!(
            (
                validation.max_nodes,
                validation.max_work,
                validation.max_input_bytes
            ),
            (1_000_000, 16_000_000, SUPPLYCHAIN_RESPONSE_BYTES)
        );
    }

    #[test]
    fn scanner_credentials_are_trimmed_and_bounded() {
        let session = Session::new();
        let good = config(r#"{"airsApiKey":"  key-1 \n","airsApiEndpoint":"http://127.0.0.1:1"}"#);
        assert!(build_scanner(&good, &session, None).is_ok());
        let long = format!(r#"{{"airsApiKey":"{}"}}"#, "k".repeat(2049));
        assert_eq!(
            build_scanner(&config(&long), &session, None)
                .err()
                .unwrap()
                .message(),
            "AISEC_USER_REQUEST_PAYLOAD_ERROR:apiKey exceeds max length of 2048"
        );
        let exact = format!(r#"{{"airsApiToken":"{}"}}"#, "k".repeat(2048));
        assert!(build_scanner(&config(&exact), &session, None).is_ok());
        let spaced = config(r#"{"airsApiToken":"a b"}"#);
        assert_eq!(
            build_scanner(&spaced, &session, None).err().unwrap().key,
            Some("airsApiToken")
        );
        let endpoint = config(r#"{"airsApiKey":"k","airsApiEndpoint":"http://example.test"}"#);
        assert_eq!(
            build_scanner(&endpoint, &session, None).err().unwrap().key,
            Some("airsApiEndpoint")
        );
    }
}
