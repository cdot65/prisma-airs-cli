//! `airs-mcp-spike-cli`: the CLI half of the M0.6 feasibility spike.
//!
//! - `login` is the only command that starts a browser flow (PKCE, loopback
//!   callback, exact state/issuer checks through rmcp). It prints the URL; it opens
//!   a browser only with an explicit `--open`.
//! - Every other command is noninteractive: missing/expired sessions fail with
//!   "authentication required" (exit 1) and never prompt.
//! - Session material lives in a private, versioned, atomically written cache
//!   under `$XDG_STATE_HOME/airs-cli-spike/` (else `~/.local/state/...`). Token
//!   values are never printed. Refresh is serialized across processes by `flock`.

use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

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
use sha2::{Digest, Sha256};

const FORMAT: &str = "airs-mcp-spike-session/1";
const MAX_SESSION_BYTES: u64 = 64 * 1024;
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
/// Client bound on one complete tool result (contract §5).
const MAX_RESULT_BYTES: usize = 256 * 1024;

/// The one client-side bound on a COMPLETE tool result (rmcp duplicates
/// structured content into text), shared by every call path.
fn bounded_result(r: &rmcp::model::CallToolResult) -> Result<usize, String> {
    let complete = serde_json::to_vec(r).map(|b| b.len()).unwrap_or(usize::MAX);
    if complete > MAX_RESULT_BYTES {
        return Err("server result exceeds the 256 KiB client bound".into());
    }
    Ok(complete)
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("airs-mcp-spike-cli: {msg}");
    ExitCode::from(1)
}

fn usage() -> ExitCode {
    eprintln!(
        "usage:\n  airs-mcp-spike-cli login --url GATEWAY_MCP_URL --approved-origin ORIGIN... --client-id ID [--scope S]... [--port N] [--open]\n  \
         airs-mcp-spike-cli status|refresh|logout --url URL\n  \
         airs-mcp-spike-cli call --url URL --tool NAME [--args JSON]\n  \
         airs-mcp-spike-cli download --url URL --out NEW_PATH [--chunk BYTES]"
    );
    ExitCode::from(2)
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let mut it = args.iter();
    let mut found = None;
    while let Some(a) = it.next() {
        if a == name {
            found = it.next().map(String::as_str);
        }
    }
    found
}

fn flags<'a>(args: &'a [String], name: &str) -> Vec<&'a str> {
    args.windows(2)
        .filter(|w| w[0] == name)
        .map(|w| w[1].as_str())
        .collect()
}

// ---------------------------------------------------------------------------
// URL trust (contract §2.4, §7.3): the resource and every discovered OAuth
// endpoint must be on an explicitly approved origin; HTTPS only, except
// loopback HTTP (tests). Validated before any state or network access.

#[derive(Clone, Debug)]
struct Trust {
    resource: String,
    origins: Vec<String>,
}

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

impl Trust {
    fn new(url: &str, approved: &[&str]) -> Result<Self, String> {
        let u = url::Url::parse(url).map_err(|_| "invalid --url".to_owned())?;
        if u.query().is_some() {
            return Err("--url must not carry a query".into());
        }
        let resource_origin = origin_of(url)?;
        if approved.is_empty() {
            return Err("at least one --approved-origin is required".into());
        }
        let origins = approved
            .iter()
            .map(|o| origin_of(o))
            .collect::<Result<Vec<_>, _>>()?;
        if !origins.contains(&resource_origin) {
            return Err("the --url origin is not an approved origin".into());
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
// Private session cache

fn state_root() -> Result<PathBuf, &'static str> {
    // Paths only; no credentials or endpoints are read from the environment.
    if let Some(x) = std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
        let p = PathBuf::from(x);
        return if p.is_absolute() {
            Ok(p.join("airs-cli-spike"))
        } else {
            Err("XDG_STATE_HOME must be absolute")
        };
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or("HOME must be absolute")?;
    Ok(home.join(".local/state/airs-cli-spike"))
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
    /// The canonical gateway resource this session is bound to.
    resource: String,
    credentials: StoredCredentials,
}

#[derive(Clone)]
struct FileStore {
    resource: String,
    path: PathBuf,
    lock: PathBuf,
}

impl FileStore {
    fn new(resource: &str) -> Result<Self, &'static str> {
        Self::at(&state_root()?, resource)
    }

    fn at(root: &Path, resource: &str) -> Result<Self, &'static str> {
        private_dir(root)?;
        let dir = root.join("sessions");
        private_dir(&dir)?;
        let id: String = Sha256::digest(resource.as_bytes())
            .iter()
            .take(16)
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(Self {
            resource: resource.to_owned(),
            path: dir.join(format!("{id}.json")),
            lock: dir.join(format!("{id}.lock")),
        })
    }

    fn read(&self) -> Result<Option<StoredCredentials>, AuthError> {
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
        if file.format != FORMAT || file.resource != self.resource {
            return Ok(None);
        }
        Ok(Some(file.credentials))
    }

    fn write(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        let io = |_| AuthError::InternalError("session cache write failed".into());
        let file = SessionFile {
            format: FORMAT.into(),
            resource: self.resource.clone(),
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
    mgr.set_credential_store(store.clone());
    Ok(mgr)
}

/// Noninteractive access-token resolution: refreshes if needed, never prompts.
async fn access_token(trust: &Trust) -> Result<String, String> {
    let store = FileStore::new(&trust.resource).map_err(str::to_owned)?;
    if store
        .read()
        .map_err(|_| "session cache unreadable".to_owned())?
        .is_none()
    {
        return Err(
            "authentication required: run `login` (no browser is opened by this command)".into(),
        );
    }
    let mut mgr = manager(trust, &store).await?;
    mgr.initialize_from_store()
        .await
        .map_err(|_| "session cache invalid".to_owned())?;
    mgr.get_access_token().await.map_err(|e| match e {
        AuthError::AuthorizationRequired => "authentication required: run `login`".to_owned(),
        _ => "token refresh failed".to_owned(),
    })
}

async fn connect(
    url: &str,
    token: String,
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ClientConfig>, String> {
    let cfg = StreamableHttpClientTransportConfig::with_uri(url.to_owned()).auth_header(token);
    ClientConfig::default()
        .serve(StreamableHttpClientTransport::from_config(cfg))
        .await
        .map_err(|_| "MCP connection failed".to_owned())
}

async fn login(
    trust: &Trust,
    client_id: &str,
    scopes: Vec<&str>,
    port: u16,
    open: bool,
) -> Result<(), String> {
    let store = FileStore::new(&trust.resource).map_err(str::to_owned)?;
    let mgr = manager(trust, &store).await?;
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
    eprintln!(
        "Open this URL to sign in (expires in {}s):\n{}",
        LOGIN_TIMEOUT.as_secs(),
        session.get_authorization_url()
    );
    if open {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let _ = std::process::Command::new(opener)
            .arg(session.get_authorization_url())
            .spawn();
    }
    let (code, state, iss) = tokio::time::timeout(LOGIN_TIMEOUT, callback(&listener))
        .await
        .map_err(|_| "login timed out; no session was saved".to_owned())??;
    session
        .handle_callback_with_issuer(&code, &state, iss.as_deref())
        .await
        .map_err(|_| {
            "authorization code exchange failed (state/issuer/resource checks)".to_owned()
        })?;
    eprintln!("Signed in; session saved privately.");
    Ok(())
}

/// Accept exactly one loopback callback request and parse code/state/iss.
async fn callback(
    listener: &tokio::net::TcpListener,
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

struct PartialFile(PathBuf);

impl Drop for PartialFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

async fn download(trust: &Trust, out: &Path, chunk: u64) -> Result<serde_json::Value, String> {
    use base64::Engine;
    if out.exists() {
        return Err("output exists; refusing to overwrite".into());
    }
    let dir = out.parent().ok_or("output needs a parent directory")?;
    let partial = dir.join(format!(".{}.partial", uuid::Uuid::new_v4()));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&partial)
        .map_err(|_| "cannot create private partial file".to_owned())?;
    // Removes the private temporary name on EVERY exit: an early session or
    // connection failure, a chunk error, an fsync or publication failure, and
    // after a successful no-clobber hard link (where it is just the spare name).
    let _cleanup = PartialFile(partial.clone());
    let client = connect(&trust.resource, access_token(trust).await?).await?;
    let start = Instant::now();
    let (mut offset, mut calls, mut total, mut expect) = (0u64, 0u64, None, String::new());
    let mut hasher = Sha256::new();
    let mut max_result = 0usize;
    let result: Result<(), String> = async {
        loop {
            let mut args = serde_json::Map::new();
            args.insert("offset".into(), offset.into());
            args.insert("length".into(), chunk.into());
            let r = client
                .call_tool(CallToolRequestParams::new("artifact_chunk").with_arguments(args))
                .await
                .map_err(|_| "chunk call failed".to_owned())?;
            calls += 1;
            let v = r.structured_content.clone().unwrap_or_default();
            let complete = bounded_result(&r)?;
            max_result = max_result.max(complete);
            if r.is_error.unwrap_or(false) {
                return Err("server refused a chunk".into());
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(v["data_base64"].as_str().unwrap_or(""))
                .map_err(|_| "bad chunk".to_owned())?;
            let chunk_hash: String = Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            if Some(chunk_hash.as_str()) != v["chunk_sha256"].as_str()
                || v["offset"].as_u64() != Some(offset)
            {
                return Err("chunk integrity check failed".into());
            }
            total = v["total_bytes"].as_u64();
            expect = v["total_sha256"].as_str().unwrap_or("").to_owned();
            f.write_all(&bytes).map_err(|_| "write failed".to_owned())?;
            hasher.update(&bytes);
            offset += bytes.len() as u64;
            if Some(offset) >= total {
                return Ok(());
            }
        }
    }
    .await;
    let _ = client.cancel().await;
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let verified = result.is_ok() && Some(offset) == total && digest == expect;
    if !verified {
        let _ = fs::remove_file(&partial);
        return Err(result
            .err()
            .unwrap_or_else(|| "total length or hash mismatch".into()));
    }
    f.sync_all().map_err(|_| "fsync failed".to_owned())?;
    drop(f);
    // No-clobber publish: hard link then remove the private temporary name.
    fs::hard_link(&partial, out)
        .map_err(|_| "cannot publish output without overwriting".to_owned())?;
    let _ = fs::remove_file(&partial);
    File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|_| "directory fsync failed".to_owned())?;
    Ok(
        serde_json::json!({"event": "download_verified", "bytes": offset, "calls": calls, "chunk_bytes": chunk,
        "elapsed_ms": start.elapsed().as_millis() as u64, "max_result_bytes": max_result, "sha256_matches": true}),
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().map(String::as_str) else {
        return usage();
    };
    let Some(raw_url) = flag(&args, "--url") else {
        return usage();
    };
    // URL trust is validated before any state or network access.
    let trust = match Trust::new(raw_url, &flags(&args, "--approved-origin")) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("airs-mcp-spike-cli: {e}");
            return ExitCode::from(2);
        }
    };
    let url = trust.resource.clone();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let outcome: Result<serde_json::Value, String> = rt.block_on(async {
        match cmd {
            "login" => {
                let Some(client_id) = flag(&args, "--client-id") else { return Err("--client-id is required".into()) };
                let port = flag(&args, "--port").and_then(|p| p.parse().ok()).unwrap_or(0);
                login(&trust, client_id, flags(&args, "--scope"), port, args.iter().any(|a| a == "--open")).await?;
                Ok(serde_json::json!({"event": "login_ok"}))
            }
            "status" => {
                let store = FileStore::new(&url)?;
                let c = store.read().map_err(|_| "session cache unreadable".to_owned())?;
                Ok(serde_json::json!({"event": "status", "session": c.as_ref().map(|c| serde_json::json!({
                    "has_access_token": c.token_response.is_some(),
                    "has_refresh_token": c.token_response.as_ref().is_some_and(|t| oauth2::TokenResponse::refresh_token(t).is_some()),
                    "granted_scopes": c.granted_scopes, "issuer": c.issuer, "received_at": c.token_received_at}))}))
            }
            "refresh" => {
                let store = FileStore::new(&url)?;
                let before = store.read().ok().flatten().and_then(|c| c.token_response).and_then(|t| oauth2::TokenResponse::refresh_token(&t).map(|r| Sha256::digest(r.secret().as_bytes())));
                let mut mgr = manager(&trust, &store).await?;
                mgr.initialize_from_store().await.map_err(|_| "authentication required: run `login`".to_owned())?;
                mgr.refresh_token().await.map_err(|_| "refresh failed; explicit login may be required".to_owned())?;
                let after = store.read().ok().flatten().and_then(|c| c.token_response).and_then(|t| oauth2::TokenResponse::refresh_token(&t).map(|r| Sha256::digest(r.secret().as_bytes())));
                // Only a boolean is reported; digests stay in memory.
                Ok(serde_json::json!({"event": "refresh_ok", "refresh_token_rotated": before.is_some() && after.is_some() && before != after}))
            }
            "logout" => {
                FileStore::new(&url)?.clear().await.map_err(|_| "logout failed".to_owned())?;
                Ok(serde_json::json!({"event": "logout_ok"}))
            }
            "call" => {
                let Some(tool) = flag(&args, "--tool") else { return Err("--tool is required".into()) };
                let a: serde_json::Map<String, serde_json::Value> = match flag(&args, "--args") {
                    Some(j) => serde_json::from_str(j).map_err(|_| "--args must be a JSON object".to_owned())?,
                    None => Default::default(),
                };
                let client = connect(&url, access_token(&trust).await?).await?;
                let r = client.call_tool(CallToolRequestParams::new(tool.to_owned()).with_arguments(a)).await.map_err(|_| "tool call failed".to_owned())?;
                let _ = client.cancel().await;
                let size = bounded_result(&r)?;
                let v = serde_json::to_value(&r).unwrap_or_default();
                Ok(serde_json::json!({"event": "call", "tool": tool, "is_error": r.is_error, "result_bytes": size, "result": v}))
            }
            "download" => {
                let Some(out) = flag(&args, "--out") else { return Err("--out is required".into()) };
                let chunk = flag(&args, "--chunk").and_then(|c| c.parse().ok()).unwrap_or(65536);
                download(&trust, Path::new(out), chunk).await
            }
            _ => Err("unknown command".into()),
        }
    });
    match outcome {
        Ok(v) => {
            println!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn temp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("airs-spike-cli-unit-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&d).unwrap();
        d.canonicalize().unwrap()
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
        assert!(t.check("http://10.0.0.5/token").is_err());
    }

    #[test]
    fn session_cache_refuses_symlinks_oversize_hardlinks_and_unsafe_ancestors() {
        let root = temp();
        let store = FileStore::at(&root.join("state"), "https://r.example/mcp").unwrap();
        store.write(creds()).unwrap();
        assert!(store.read().unwrap().is_some());
        // Symlink replacement of the session file is refused.
        let real = store.path.with_extension("real");
        fs::rename(&store.path, &real).unwrap();
        std::os::unix::fs::symlink(&real, &store.path).unwrap();
        assert!(store.read().is_err());
        fs::remove_file(&store.path).unwrap();
        // Hard links (nlink > 1) are refused.
        fs::hard_link(&real, &store.path).unwrap();
        assert!(store.read().is_err());
        fs::remove_file(&store.path).unwrap();
        // Oversized content is refused (bounded read through the descriptor).
        fs::write(&store.path, vec![b' '; (MAX_SESSION_BYTES + 1) as usize]).unwrap();
        fs::set_permissions(&store.path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(store.read().is_err());
        // Loose mode is refused.
        fs::set_permissions(&store.path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(store.read().is_err());
        // A group-writable ancestor makes the store unusable.
        let shared = root.join("shared");
        fs::DirBuilder::new().mode(0o700).create(&shared).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o770)).unwrap();
        assert!(FileStore::at(&shared.join("state"), "https://r.example/mcp").is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn refresh_lock_refuses_symlinks_and_loose_files() {
        let root = temp();
        let store = FileStore::at(&root.join("state"), "https://r.example/mcp").unwrap();
        assert!(store.acquire_refresh_guard().await.unwrap().is_some());
        fs::remove_file(&store.lock).unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere"), &store.lock).unwrap();
        assert!(store.acquire_refresh_guard().await.is_err());
        assert!(
            !root.join("elsewhere").exists(),
            "lock must not follow a symlink"
        );
        fs::remove_file(&store.lock).unwrap();
        fs::write(&store.lock, b"").unwrap();
        fs::set_permissions(&store.lock, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(store.acquire_refresh_guard().await.is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn complete_result_bound_applies_to_duplicated_content() {
        // 140 KiB of structured text becomes ~280 KiB once duplicated as text content.
        let big = rmcp::model::CallToolResult::structured(
            serde_json::json!({"text": "x".repeat(140 * 1024)}),
        );
        assert!(bounded_result(&big).is_err());
        let small = rmcp::model::CallToolResult::structured(
            serde_json::json!({"text": "x".repeat(100 * 1024)}),
        );
        assert!(bounded_result(&small).unwrap() <= MAX_RESULT_BYTES);
    }

    #[test]
    fn a_session_for_another_resource_is_never_reused() {
        let root = temp();
        let a = FileStore::at(&root.join("state"), "https://a.example/mcp").unwrap();
        a.write(creds()).unwrap();
        let mut b = FileStore::at(&root.join("state"), "https://b.example/mcp").unwrap();
        b.path = a.path.clone(); // same file, different bound resource
        assert!(b.read().unwrap().is_none());
        fs::remove_dir_all(&root).unwrap();
    }
}
