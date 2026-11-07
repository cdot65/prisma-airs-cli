//! `airs-mcp-client`: the optional runtime MCP client of the `airs` CLI
//! (RUNTIME_MCP_CONTRACT §7). No SQLite, no server code and no backend credentials:
//! gateway OAuth (PKCE with a loopback callback, exact state/issuer checks, origin
//! allowlist for every discovered endpoint), a private session cache with
//! cross-process refresh serialization, typed bounded tool calls and durable
//! operation receipts. On an ordinary connection every command except `login` is
//! noninteractive: a missing or unusable session is "authentication required" (exit 1),
//! never a prompt. An admin connection keeps no session; each administrative command
//! starts its own fresh, nonce-bound sign-in.

use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientConfig},
    transport::{
        StreamableHttpClientTransport,
        auth::{
            AuthError, AuthorizationManager, AuthorizationRequest, AuthorizationSession,
            CredentialRefreshGuard, CredentialStore, OAuthHttpClient, OAuthHttpClientFuture,
            OAuthHttpRedirectPolicy, OAuthHttpRequest, StoredCredentials,
        },
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

pub use airs_runtime_core::tenant::mcp::{Cache, McpConnection, Role};

pub mod previews;
pub mod receipts;

/// Standard base64 (the chunk encoding of `artifacts_download`, `export_read` and
/// `transfer_upload_chunk`, contract §5).
pub fn b64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Decode standard base64; `None` for anything else.
pub fn b64_decode(text: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(text).ok()
}

/// Lowercase hex SHA-256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

const FORMAT: &str = "airs-mcp-session/1";
const MAX_SESSION_BYTES: u64 = 64 * 1024;
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
/// Client bound on one complete tool result (contract §5).
pub const MAX_RESULT_BYTES: usize = 256 * 1024;

/// A fixed, terminal-safe client error (never server text, tokens or URLs with
/// credentials).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientError(pub String);

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for ClientError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ClientError {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// The one client-side bound on a COMPLETE tool result (rmcp duplicates structured
/// content into text), shared by every call path.
fn bounded_result(r: &rmcp::model::CallToolResult) -> Result<usize, String> {
    let complete = serde_json::to_vec(r).map(|b| b.len()).unwrap_or(usize::MAX);
    if complete > MAX_RESULT_BYTES {
        return Err("server result exceeds the 256 KiB client bound".into());
    }
    Ok(complete)
}

// ---------------------------------------------------------------------------
// URL trust (contract §2.4, §7.3).

fn origin_of(raw: &str) -> Result<String, String> {
    let u = url::Url::parse(raw).map_err(|_| "invalid URL".to_owned())?;
    if !u.username().is_empty() || u.password().is_some() || u.fragment().is_some() {
        return Err("URL must not carry credentials or a fragment".into());
    }
    let host = u.host_str().ok_or("URL needs a host")?;
    let loopback = matches!(host, "127.0.0.1" | "[::1]" | "localhost");
    match u.scheme() {
        "https" => {}
        "http" if loopback => {}
        _ => return Err("only HTTPS (or loopback HTTP) URLs are accepted".into()),
    }
    Ok(u.origin().ascii_serialization())
}

#[derive(Clone, Debug)]
pub struct Trust {
    pub resource: String,
    origins: Vec<String>,
}

impl Trust {
    pub fn new(url: &str, approved: &[&str]) -> Result<Self, String> {
        let u = url::Url::parse(url).map_err(|_| "invalid connection url".to_owned())?;
        if u.query().is_some() {
            return Err("the connection url must not carry a query".into());
        }
        let resource_origin = origin_of(url)?;
        if approved.is_empty() {
            return Err("at least one discovery origin is required".into());
        }
        let origins = approved
            .iter()
            .map(|o| origin_of(o))
            .collect::<Result<Vec<_>, _>>()?;
        if !origins.contains(&resource_origin) {
            return Err("the connection url origin is not a discovery origin".into());
        }
        let resource = u.as_str().trim_end_matches('/').to_owned();
        Ok(Self { resource, origins })
    }

    fn check(&self, endpoint: &str) -> Result<(), String> {
        let o =
            origin_of(endpoint).map_err(|_| "authorization server endpoint refused".to_owned())?;
        if self.origins.contains(&o) {
            Ok(())
        } else {
            Err("authorization server origin is not approved; nothing was sent to it".into())
        }
    }
}

/// Request-layer origin guard for every OAuth/discovery HTTP request rmcp makes
/// (protected-resource metadata, authorization-server metadata, token and
/// refresh). An unapproved or non-HTTPS origin is refused before anything is
/// sent, and redirects are never followed (a 3xx is returned unfollowed, which
/// rmcp treats as a failure), so discovery cannot be steered off the allowlist.
struct OriginGuard {
    trust: Trust,
    inner: Box<dyn OAuthHttpClient>,
    refused: std::sync::atomic::AtomicUsize,
}

impl OAuthHttpClient for OriginGuard {
    fn execute(&self, mut request: OAuthHttpRequest) -> OAuthHttpClientFuture<'_> {
        Box::pin(async move {
            let uri = request.request.uri().to_string();
            if self.trust.check(&uri).is_err() {
                self.refused
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return Err("request to an unapproved origin refused before sending".into());
            }
            request.redirect_policy = OAuthHttpRedirectPolicy::Stop;
            self.inner.execute(request).await
        })
    }
}

// ---------------------------------------------------------------------------
// Private session cache (contract §7.4)

/// `$XDG_STATE_HOME/airs-cli/mcp/` (absolute), else `$HOME/.local/state/airs-cli/mcp/`.
/// Paths only: no credential or endpoint is read from the environment.
pub fn state_root() -> Result<PathBuf, &'static str> {
    if let Some(x) = std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
        let p = PathBuf::from(x);
        return if p.is_absolute() {
            Ok(p.join("airs-cli/mcp"))
        } else {
            Err("XDG_STATE_HOME must be absolute")
        };
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or("HOME must be absolute")?;
    Ok(home.join(".local/state/airs-cli/mcp"))
}

fn euid() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// Existing ancestors must be directories owned by root or this user and not
/// group/other writable (root-owned sticky directories such as /tmp excepted).
fn check_ancestors(path: &Path) -> Result<(), &'static str> {
    for a in path.ancestors().skip(1) {
        let Ok(m) = fs::symlink_metadata(a) else {
            continue;
        };
        let sticky_root = m.uid() == 0 && m.mode() & 0o1000 != 0;
        if !m.is_dir()
            || (m.uid() != 0 && m.uid() != euid())
            || (m.mode() & 0o022 != 0 && !sticky_root)
        {
            return Err(
                "an ancestor of the session directory is unsafe (foreign-owned or group/other writable)",
            );
        }
    }
    Ok(())
}

fn private_dir(path: &Path) -> Result<(), &'static str> {
    check_ancestors(path)?;
    match fs::symlink_metadata(path) {
        Ok(m) => {
            if !m.is_dir() || m.uid() != euid() || m.mode() & 0o077 != 0 {
                return Err("session directory must be a private 0700 directory owned by you");
            }
            Ok(())
        }
        Err(_) => fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)
            .map_err(|_| "cannot create session directory"),
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionFile {
    format: String,
    /// The binding (contract §7.4): any change makes the session unusable.
    resource: String,
    #[serde(rename = "clientId")]
    client_id: String,
    #[serde(rename = "connectionRevision")]
    connection_revision: String,
    credentials: StoredCredentials,
}

/// What a session is bound to (contract §7.4): the canonical gateway resource, the
/// public client and the connection's revision. A change to any of them (an explicit
/// `connections set`) makes the session unusable until a new `login`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub resource: String,
    pub client_id: String,
    pub revision: String,
}

impl Binding {
    pub fn of(trust: &Trust, connection: &McpConnection) -> Self {
        Self {
            resource: trust.resource.clone(),
            client_id: connection.client_id.clone(),
            revision: connection.revision.clone(),
        }
    }

    fn id(&self) -> String {
        Sha256::digest(
            format!(
                "{}\u{1f}{}\u{1f}{}",
                self.resource, self.client_id, self.revision
            )
            .as_bytes(),
        )
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect()
    }
}

#[derive(Clone)]
pub struct FileStore {
    binding: Binding,
    path: PathBuf,
    lock: PathBuf,
}

impl FileStore {
    pub fn new(binding: &Binding) -> Result<Self, &'static str> {
        Self::at(&state_root()?, binding)
    }

    pub fn at(root: &Path, binding: &Binding) -> Result<Self, &'static str> {
        private_dir(root)?;
        let dir = root.join("sessions");
        private_dir(&dir)?;
        let id = binding.id();
        Ok(Self {
            binding: binding.clone(),
            path: dir.join(format!("{id}.json")),
            lock: dir.join(format!(".{id}.lock")),
        })
    }

    /// Whether a usable session file exists (no token value is ever returned).
    pub fn present(&self) -> bool {
        self.read().ok().flatten().is_some()
    }

    pub fn remove(&self) -> Result<bool, &'static str> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(_) => Err("cannot remove the session"),
        }
    }

    pub fn read(&self) -> Result<Option<StoredCredentials>, AuthError> {
        let io = |_| AuthError::InternalError("session cache unreadable".into());
        let mut f = match fs::OpenOptions::new()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(&self.path)
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io(e)),
        };
        let m = f.metadata().map_err(io)?;
        if !m.is_file()
            || m.uid() != euid()
            || m.mode() & 0o077 != 0
            || m.nlink() != 1
            || m.size() > MAX_SESSION_BYTES
        {
            return Err(AuthError::InternalError(
                "session cache is not private".into(),
            ));
        }
        // Read through the verified descriptor, never the pathname again.
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(
            &mut std::io::Read::take(&mut f, MAX_SESSION_BYTES + 1),
            &mut bytes,
        )
        .map_err(io)?;
        if bytes.len() as u64 > MAX_SESSION_BYTES {
            return Err(AuthError::InternalError(
                "session cache is oversized".into(),
            ));
        }
        let file: SessionFile = serde_json::from_slice(&bytes)
            .map_err(|_| AuthError::InternalError("session cache invalid".into()))?;
        // A session bound to another resource is never reused.
        if file.format != FORMAT
            || file.resource != self.binding.resource
            || file.client_id != self.binding.client_id
            || file.connection_revision != self.binding.revision
        {
            return Ok(None);
        }
        Ok(Some(file.credentials))
    }

    pub fn write(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        let io = |_| AuthError::InternalError("session cache write failed".into());
        let file = SessionFile {
            format: FORMAT.into(),
            resource: self.binding.resource.clone(),
            client_id: self.binding.client_id.clone(),
            connection_revision: self.binding.revision.clone(),
            credentials,
        };
        let bytes =
            serde_json::to_vec(&file).map_err(|_| AuthError::InternalError("encode".into()))?;
        let dir = self
            .path
            .parent()
            .ok_or_else(|| AuthError::InternalError("path".into()))?;
        let tmp = dir.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(io)?;
        f.write_all(&bytes).and_then(|_| f.sync_all()).map_err(io)?;
        drop(f);
        fs::rename(&tmp, &self.path).map_err(io)?;
        File::open(dir).and_then(|d| d.sync_all()).map_err(io)
    }
}

#[async_trait::async_trait]
impl CredentialStore for FileStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        self.read()
    }
    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        self.write(credentials)
    }
    async fn clear(&self) -> Result<(), AuthError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(AuthError::InternalError("cannot remove session".into())),
        }
    }
    /// Serialize refresh-token rotation across CLI processes (contract §7.3).
    async fn acquire_refresh_guard(&self) -> Result<Option<CredentialRefreshGuard>, AuthError> {
        let path = self.lock.clone();
        let f = tokio::task::spawn_blocking(move || -> Result<File, AuthError> {
            let f = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
                .open(&path)
                .map_err(|_| AuthError::InternalError("lock".into()))?;
            let m = f
                .metadata()
                .map_err(|_| AuthError::InternalError("lock".into()))?;
            if !m.is_file() || m.uid() != euid() || m.mode() & 0o077 != 0 || m.nlink() != 1 {
                return Err(AuthError::InternalError(
                    "refresh lock is not private".into(),
                ));
            }
            rustix::fs::flock(&f, rustix::fs::FlockOperation::LockExclusive)
                .map_err(|_| AuthError::InternalError("lock".into()))?;
            Ok(f)
        })
        .await
        .map_err(|_| AuthError::InternalError("lock".into()))??;
        Ok(Some(CredentialRefreshGuard::new(f)))
    }
}

async fn manager(trust: &Trust, store: &FileStore) -> Result<AuthorizationManager, String> {
    manager_for(trust, store.clone()).await
}

/// An authorization manager over any credential store, after discovery with every
/// endpoint checked against the approved origins.
async fn manager_for<S: CredentialStore + 'static>(
    trust: &Trust,
    store: S,
) -> Result<AuthorizationManager, String> {
    let inner = rmcp::transport::auth::default_oauth_http_client()
        .map_err(|_| "cannot create OAuth HTTP client".to_owned())?;
    let guard = std::sync::Arc::new(OriginGuard {
        trust: trust.clone(),
        inner: Box::new(inner),
        refused: std::sync::atomic::AtomicUsize::new(0),
    });
    let mut mgr =
        AuthorizationManager::new_with_oauth_http_client(trust.resource.as_str(), guard.clone())
            .await
            .map_err(|_| "cannot create OAuth manager".to_owned())?;
    let res = mgr.resolve_metadata().await.map_err(|_| {
        if guard.refused.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            "discovery named an unapproved origin; nothing was sent to it".to_owned()
        } else {
            "authorization metadata discovery failed".to_owned()
        }
    })?;
    // Every endpoint a token, code or verifier would be sent to must be approved.
    let m = &res.metadata;
    trust.check(&m.authorization_endpoint)?;
    trust.check(&m.token_endpoint)?;
    for extra in [m.issuer.as_deref(), m.registration_endpoint.as_deref()]
        .into_iter()
        .flatten()
    {
        trust.check(extra)?;
    }
    mgr.set_metadata(res.metadata);
    mgr.set_credential_store(store);
    Ok(mgr)
}

/// Interactive PKCE sign-in (the only interactive command on an ordinary connection;
/// admin commands use [`fresh_token`] for their own sign-in): prints the authorization
/// URL on stderr (opens a browser only with `open`), accepts one loopback callback,
/// checks state and issuer through rmcp, and saves the session privately.
pub async fn login(
    trust: &Trust,
    store: &FileStore,
    client_id: &str,
    scopes: Vec<&str>,
    port: u16,
    open: bool,
) -> Result<(), String> {
    let mgr = manager(trust, store).await?;
    authorize(mgr, client_id, scopes, port, open, &[]).await
}

/// One administrative sign-in for one command (contract §3 option B; the admin
/// connection is `cache = "none"`): the same PKCE flow as [`login`], with `extra`
/// authorization parameters (the preview's proof nonce and a fresh-authentication
/// request) added to the authorization URL. The credentials live only in memory for
/// this process and are never written; the access token is returned.
pub async fn fresh_token(
    trust: &Trust,
    client_id: &str,
    port: u16,
    open: bool,
    extra: &[(&str, &str)],
) -> Result<String, String> {
    use oauth2::TokenResponse;
    let memory = rmcp::transport::auth::InMemoryCredentialStore::new();
    let mgr = manager_for(trust, memory.clone()).await?;
    authorize(mgr, client_id, vec![], port, open, extra).await?;
    let creds = memory
        .load()
        .await
        .map_err(|_| "sign-in produced no credentials".to_owned())?
        .and_then(|c| c.token_response)
        .ok_or_else(|| "sign-in produced no access token".to_owned())?;
    Ok(creds.access_token().secret().to_owned())
}

/// The shared PKCE flow: print the authorization URL on stderr (with `extra`
/// parameters appended), accept one loopback callback, and exchange the code
/// (state, issuer and resource checked by rmcp) into the manager's store.
async fn authorize(
    mgr: AuthorizationManager,
    client_id: &str,
    scopes: Vec<&str>,
    port: u16,
    open: bool,
    extra: &[(&str, &str)],
) -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|_| "cannot bind loopback callback".to_owned())?;
    let port = listener
        .local_addr()
        .map_err(|_| "callback address".to_owned())?
        .port();
    let redirect = format!("http://127.0.0.1:{port}/callback");
    let mut req = AuthorizationRequest::new(redirect).with_preregistered_client(client_id);
    req = if scopes.is_empty() {
        req.with_initial_scope_selection()
    } else {
        req.with_scopes(scopes)
    };
    let session = AuthorizationSession::new(mgr, req)
        .await
        .map_err(|_| "authorization request failed".to_owned())?;
    let mut url = url::Url::parse(session.get_authorization_url())
        .map_err(|_| "authorization URL invalid".to_owned())?;
    // The state this sign-in expects: callbacks carrying any other state are unrelated
    // and ignored (contract P5).
    let expected = url
        .query_pairs()
        .find(|(n, _)| n == "state")
        .map(|(_, v)| v.into_owned())
        .ok_or_else(|| "authorization URL has no state".to_owned())?;
    if !extra.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (k, v) in extra {
            pairs.append_pair(k, v);
        }
    }
    eprintln!(
        "Open this URL to sign in (expires in {}s):\n{}",
        LOGIN_TIMEOUT.as_secs(),
        url
    );
    if open {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let _ = std::process::Command::new(opener).arg(url.as_str()).spawn();
    }
    let (code, state, iss) = wait_for_callback(&listener, &expected, LOGIN_TIMEOUT).await?;
    session
        .handle_callback_with_issuer(&code, &state, iss.as_deref())
        .await
        .map_err(|_| {
            "authorization code exchange failed (state/issuer/resource checks)".to_owned()
        })?;
    Ok(())
}

/// Wait, until `deadline` elapses, for the loopback callback carrying `expected` state.
async fn wait_for_callback(
    listener: &tokio::net::TcpListener,
    expected: &str,
    deadline: Duration,
) -> Result<(String, String, Option<String>), String> {
    tokio::time::timeout(deadline, callback(listener, expected))
        .await
        .map_err(|_| "login timed out; no session was saved".to_owned())?
}

/// Accept loopback callback requests until one carries the `expected` state, and parse
/// its code/state/iss. A callback with a missing or different state is unrelated (a
/// stale tab, another sign-in, or a forged request): it is answered 400 and ignored, and
/// its code is never exchanged (contract P5). The caller bounds the wait.
async fn callback(
    listener: &tokio::net::TcpListener,
    expected: &str,
) -> Result<(String, String, Option<String>), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    loop {
        let (mut sock, _) = listener
            .accept()
            .await
            .map_err(|_| "callback accept failed".to_owned())?;
        let mut buf = vec![0u8; 8192];
        let n = sock.read(&mut buf).await.unwrap_or(0);
        let head = String::from_utf8_lossy(&buf[..n]);
        let Some(target) = head
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("GET "))
            .and_then(|l| l.split(' ').next())
        else {
            continue;
        };
        let Ok(u) = url::Url::parse(&format!("http://127.0.0.1{target}")) else {
            continue;
        };
        if u.path() != "/callback" {
            let _ = sock
                .write_all(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\n\r\n")
                .await;
            continue;
        }
        let q = |k: &str| {
            u.query_pairs()
                .find(|(n, _)| n == k)
                .map(|(_, v)| v.into_owned())
        };
        if q("state").as_deref() != Some(expected) {
            let body = "Unrelated sign-in callback ignored.";
            let _ = sock.write_all(format!("HTTP/1.1 400 Bad Request\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes()).await;
            continue;
        }
        let body = "Sign-in received. You can close this window.";
        let _ = sock.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes()).await;
        if q("error").is_some() {
            return Err("authorization server returned an error".into());
        }
        return match (q("code"), q("state")) {
            (Some(c), Some(s)) => Ok((c, s, q("iss"))),
            _ => Err("callback missing code or state".into()),
        };
    }
}

/// Noninteractive access-token resolution: refreshes if needed, never prompts.
pub async fn access_token(trust: &Trust, store: &FileStore) -> Result<String, String> {
    if store
        .read()
        .map_err(|_| "session cache unreadable".to_owned())?
        .is_none()
    {
        return Err(
            "authentication required: run 'airs mcp login CONNECTION' (on an ordinary connection no other command signs in; admin commands start their own sign-in)"
                .into(),
        );
    }
    let mut mgr = manager(trust, store).await?;
    mgr.initialize_from_store()
        .await
        .map_err(|_| "session cache invalid".to_owned())?;
    mgr.get_access_token().await.map_err(|e| match e {
        AuthError::AuthorizationRequired => {
            "authentication required: run 'airs mcp login CONNECTION'".to_owned()
        }
        _ => "token refresh failed; run 'airs mcp login CONNECTION'".to_owned(),
    })
}

/// One bounded authentication recovery (contract §7.4): refresh the session's access
/// token now (serialized by the refresh lock), never prompting. A failure means the
/// user must sign in again.
pub async fn refreshed_token(trust: &Trust, store: &FileStore) -> Result<String, String> {
    if store
        .read()
        .map_err(|_| "session cache unreadable".to_owned())?
        .is_none()
    {
        return Err("authentication required: run 'airs mcp login CONNECTION'".into());
    }
    let mut mgr = manager(trust, store).await?;
    mgr.initialize_from_store()
        .await
        .map_err(|_| "session cache invalid".to_owned())?;
    mgr.refresh_token()
        .await
        .map_err(|_| "token refresh failed; run 'airs mcp login CONNECTION'".to_owned())?;
    mgr.get_access_token()
        .await
        .map_err(|_| "authentication required: run 'airs mcp login CONNECTION'".to_owned())
}

/// A tool's refusal: the service's fixed code and reason (and retry hint).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub code: String,
    pub reason: String,
    pub retry_after_seconds: Option<u64>,
}

/// The outcome of one tool call.
#[derive(Debug, Clone, PartialEq)]
pub enum Called {
    Ok(Value),
    Refused(Refusal),
}

/// One request envelope (contract §4.2).
pub fn envelope(tenant: &str, params: Value, key: Option<(&str, &str)>) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("schema_version".into(), json!(1));
    m.insert("tenant".into(), json!(tenant));
    if let Some((k, epoch)) = key {
        m.insert("idempotency_key".into(), json!(k));
        m.insert("expected_epoch".into(), json!(epoch));
    }
    m.insert("params".into(), params);
    m
}

/// Call one tool on an authenticated connection; the complete result is bounded and
/// only its structured content is returned (never transport or server text).
pub async fn call(
    url: &str,
    token: String,
    tool: &str,
    args: Map<String, Value>,
) -> Result<Called, String> {
    let cfg = StreamableHttpClientTransportConfig::with_uri(url.to_owned()).auth_header(token);
    let client = ClientConfig::default()
        .serve(StreamableHttpClientTransport::from_config(cfg))
        .await
        .map_err(|_| {
            "MCP connection failed (endpoint unavailable or authentication refused)".to_owned()
        })?;
    let r = client
        .call_tool(CallToolRequestParams::new(tool.to_owned()).with_arguments(args))
        .await
        .map_err(|_| "MCP tool call failed".to_owned());
    let _ = client.cancel().await;
    let r = r?;
    bounded_result(&r)?;
    let v = r.structured_content.clone().unwrap_or(Value::Null);
    if r.is_error.unwrap_or(false) {
        let text = |k: &str| {
            v[k].as_str()
                .unwrap_or_default()
                .chars()
                .filter(|c| !c.is_control())
                .take(256)
                .collect::<String>()
        };
        return Ok(Called::Refused(Refusal {
            code: text("code"),
            reason: text("reason"),
            retry_after_seconds: v["retry_after_seconds"].as_u64(),
        }));
    }
    Ok(Called::Ok(v))
}

#[cfg(test)]
mod tests {
    /// Send one raw loopback callback request and return the response status line.
    async fn send_callback(port: u16, query: &str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        s.write_all(
            format!("GET /callback?{query} HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n").as_bytes(),
        )
        .await
        .unwrap();
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out).await;
        String::from_utf8_lossy(&out)
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned()
    }

    /// Contract P5 (M7 review 1 finding 8): an unrelated callback (wrong or missing
    /// state, with its own code, or an error) is answered 400 and ignored; the wait
    /// continues and returns only the matching callback's code, so the unrelated code is
    /// never exchanged.
    #[tokio::test]
    async fn an_unrelated_callback_is_ignored_and_the_matching_one_is_used() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let client = tokio::spawn(async move {
            let wrong = send_callback(port, "code=code-unrelated&state=state-other").await;
            let missing = send_callback(port, "code=code-nostate").await;
            let error = send_callback(port, "error=access_denied&state=state-other").await;
            let right = send_callback(
                port,
                "code=code-expected&state=state-expected&iss=https%3A%2F%2Fi.example",
            )
            .await;
            (wrong, missing, error, right)
        });
        let got = wait_for_callback(&listener, "state-expected", Duration::from_secs(10))
            .await
            .unwrap();
        // Checked before joining the client: had an unrelated callback been returned,
        // its later requests would wait unanswered.
        assert_eq!(got.0, "code-expected");
        assert_eq!(got.1, "state-expected");
        assert_eq!(got.2.as_deref(), Some("https://i.example"));
        let (wrong, missing, error, right) = client.await.unwrap();
        assert!(wrong.contains(" 400 "), "{wrong}");
        assert!(missing.contains(" 400 "), "{missing}");
        assert!(
            error.contains(" 400 "),
            "an unrelated error does not end sign-in: {error}"
        );
        assert!(right.contains(" 200 "), "{right}");
    }

    /// Only unrelated callbacks: the bounded wait ends with the existing timeout error
    /// and nothing is returned for exchange.
    #[tokio::test]
    async fn only_unrelated_callbacks_end_at_the_deadline() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let client = tokio::spawn(async move {
            for n in 0..3 {
                send_callback(port, &format!("code=code-{n}&state=state-other")).await;
            }
        });
        let got = wait_for_callback(&listener, "state-expected", Duration::from_millis(500)).await;
        assert_eq!(got.unwrap_err(), "login timed out; no session was saved");
        client.await.unwrap();
    }

    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn temp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("airs-mcp-client-unit-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&d).unwrap();
        d.canonicalize().unwrap()
    }

    fn binding(resource: &str) -> Binding {
        Binding {
            resource: resource.into(),
            client_id: "client".into(),
            revision: "00000000-0000-4000-8000-0000000000aa".into(),
        }
    }

    fn creds() -> StoredCredentials {
        StoredCredentials::new("client".into(), None, vec![], None)
    }

    #[test]
    fn trust_requires_https_or_loopback_and_approved_origins() {
        assert!(Trust::new("https://mcp.example/ws/airs/mcp", &["https://mcp.example"]).is_ok());
        assert!(
            Trust::new("http://mcp.example/mcp", &["http://mcp.example"]).is_err(),
            "downgrade"
        );
        assert!(Trust::new("https://mcp.example/mcp", &[]).is_err());
        assert!(Trust::new("https://mcp.example/mcp", &["https://other.example"]).is_err());
        assert!(Trust::new("https://u:p@mcp.example/mcp", &["https://mcp.example"]).is_err());
        assert!(Trust::new("https://mcp.example/mcp?x=1", &["https://mcp.example"]).is_err());
        let t = Trust::new("http://127.0.0.1:9/mcp", &["http://127.0.0.1:9"]).unwrap();
        assert!(t.check("http://127.0.0.1:9/idira/token").is_ok());
        assert!(
            t.check("http://127.0.0.1:10/token").is_err(),
            "other port is another origin"
        );
        assert!(t.check("https://evil.example/token").is_err());
    }

    /// Contract §7.4: the session cache refuses symlinks, hard links, oversize files,
    /// loose modes and unsafe ancestors; files are 0600 in a 0700 directory.
    #[test]
    fn session_cache_is_private_and_refuses_unsafe_files() {
        let root = temp();
        let store = FileStore::at(&root.join("state"), &binding("https://r.example/mcp")).unwrap();
        store.write(creds()).unwrap();
        assert!(store.read().unwrap().is_some());
        assert_eq!(
            fs::metadata(&store.path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let real = store.path.with_extension("real");
        fs::rename(&store.path, &real).unwrap();
        std::os::unix::fs::symlink(&real, &store.path).unwrap();
        assert!(store.read().is_err(), "symlink");
        fs::remove_file(&store.path).unwrap();
        fs::hard_link(&real, &store.path).unwrap();
        assert!(store.read().is_err(), "hard link");
        fs::remove_file(&store.path).unwrap();
        fs::write(&store.path, vec![b' '; (MAX_SESSION_BYTES + 1) as usize]).unwrap();
        fs::set_permissions(&store.path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(store.read().is_err(), "oversize");
        fs::set_permissions(&store.path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(store.read().is_err(), "loose mode");
        let shared = root.join("shared");
        fs::DirBuilder::new().mode(0o700).create(&shared).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o770)).unwrap();
        assert!(FileStore::at(&shared.join("state"), &binding("https://r.example/mcp")).is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    /// Contract §7.4: any binding change (resource, client, connection revision) makes
    /// a session unusable; logout removes only that session.
    #[test]
    fn a_session_bound_elsewhere_is_never_reused() {
        let root = temp();
        let a = FileStore::at(&root.join("state"), &binding("https://a.example/mcp")).unwrap();
        a.write(creds()).unwrap();
        for other in [
            binding("https://b.example/mcp"),
            Binding {
                client_id: "another".into(),
                ..binding("https://a.example/mcp")
            },
            Binding {
                revision: "00000000-0000-4000-8000-0000000000bb".into(),
                ..binding("https://a.example/mcp")
            },
        ] {
            let mut b = FileStore::at(&root.join("state"), &other).unwrap();
            assert_ne!(
                b.path, a.path,
                "a different binding names a different session"
            );
            b.path = a.path.clone();
            assert!(b.read().unwrap().is_none(), "{other:?}");
        }
        assert!(a.remove().unwrap());
        assert!(!a.present());
        fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn refresh_lock_refuses_symlinks_and_loose_files() {
        let root = temp();
        let store = FileStore::at(&root.join("state"), &binding("https://r.example/mcp")).unwrap();
        assert!(store.acquire_refresh_guard().await.unwrap().is_some());
        fs::remove_file(&store.lock).unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere"), &store.lock).unwrap();
        assert!(store.acquire_refresh_guard().await.is_err());
        assert!(
            !root.join("elsewhere").exists(),
            "the lock never follows a symlink"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_complete_result_bound_counts_duplicated_content() {
        let big = rmcp::model::CallToolResult::structured(json!({"text": "x".repeat(140 * 1024)}));
        assert!(bounded_result(&big).is_err());
        let small =
            rmcp::model::CallToolResult::structured(json!({"text": "x".repeat(100 * 1024)}));
        assert!(bounded_result(&small).unwrap() <= MAX_RESULT_BYTES);
    }
}
