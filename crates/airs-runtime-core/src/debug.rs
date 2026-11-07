//! The `--debug` API log's records and transport (DESIGN §2.9).
//!
//! One record per transport attempt, written to a caller-supplied sink (the CLI opens
//! the per-run file and owns its directory, retention and stderr). A record holds
//! status and headers only: never a request or response body, never a URL query or
//! fragment. Header values are written only for an allow-list of names; every other
//! value is `***`. Before each write the whole line is checked against the
//! invocation's [`SecretSet`] (defence in depth). A failed write never changes the
//! command: logging stops and one warning goes to stderr.

use crate::error::SecretSet;
use crate::json::{self, OValue};
use crate::style::{self, Style};
use futures_util::future::BoxFuture;
use reqwest::header::HeaderMap;
use reqwest::{Method, StatusCode};
use scm::transport::{Transport, TransportRequest, TransportResponse};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use url::Url;

/// What a header value outside the allow-list (or carrying a secret) is written as.
pub const MASK: &str = "***";
/// The one stderr warning after a failed write (yellow, TS `ui.warning`).
pub const WRITE_WARNING: &str = "⚠ Debug: the API log could not be written; it is incomplete";
/// `error` of an attempt whose `send` returned an error.
pub const REQUEST_FAILED: &str = "Request failed";
/// `error` of an attempt whose `send` future was dropped first (deadline, cancel).
pub const REQUEST_CANCELLED: &str = "Request cancelled before a response";
/// The SDK operation of an OAuth token request: its record waits for the lease
/// ([`TokenHold`]).
pub const TOKEN_OPERATION: &str = "oauth.token";

/// Request headers whose values are written.
const REQUEST_HEADERS: &[&str] = &[
    "accept",
    "content-length",
    "content-type",
    "service-name",
    "user-agent",
    "x-tsg-id",
];
/// Response headers whose values are written (`location` is sanitised separately).
const RESPONSE_HEADERS: &[&str] = &[
    "cache-control",
    "content-length",
    "content-type",
    "date",
    "request-id",
    "retry-after",
    "server",
    "vary",
    "x-correlation-id",
    "x-request-id",
];

/// The invocation's API log. Clones share one file.
#[derive(Clone)]
pub struct DebugLog {
    inner: Arc<Inner>,
}

struct Inner {
    path: PathBuf,
    /// `None` once a write failed (the log is closed for the rest of the run).
    sink: Mutex<Option<Box<dyn Write + Send>>>,
    /// Where the write warning goes; taken by the first failure, so it prints once.
    warning: Mutex<Option<Box<dyn Write + Send>>>,
    warning_color: bool,
    secrets: SecretSet,
    /// Token records waiting for their lease (see [`TokenHold`]).
    holds: Mutex<Holds>,
}

/// The token-record holds of a log.
#[derive(Default)]
struct Holds {
    /// Live [`TokenHold`]s.
    active: usize,
    /// Whether a hold was dropped unreleased (cancelled) since the last flush.
    cancelled: bool,
    /// Finished token attempts not yet written.
    pending: Vec<Finished>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl DebugLog {
    /// A log writing finished lines to `sink` and its one write-failure warning to
    /// `warning` (painted when `warning_color`). The caller owns where they go: the CLI
    /// opens a private file and stderr; tests use memory.
    pub fn with_sinks(
        path: PathBuf,
        sink: Box<dyn Write + Send>,
        warning: Box<dyn Write + Send>,
        warning_color: bool,
        secrets: SecretSet,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                path,
                sink: Mutex::new(Some(sink)),
                warning: Mutex::new(Some(warning)),
                warning_color,
                secrets,
                holds: Mutex::new(Holds::default()),
            }),
        }
    }

    /// The log file.
    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    /// Start recording one attempt (at entry to `Transport::send`).
    pub fn begin(
        &self,
        method: &Method,
        url: &Url,
        headers: &HeaderMap,
        operation: &'static str,
    ) -> Attempt {
        Attempt {
            log: self.clone(),
            started: Instant::now(),
            fields: Some(Fields {
                timestamp: iso_timestamp(SystemTime::now()),
                operation,
                method: method.as_str().to_owned(),
                url: sanitize_url(url),
                request_headers: header_object(
                    headers,
                    REQUEST_HEADERS,
                    false,
                    &self.inner.secrets,
                ),
            }),
        }
    }

    /// Hold every `oauth.token` record finished from now until the hold is released
    /// (the lease is registered) or dropped (DESIGN §2.9).
    pub fn hold_tokens(&self) -> TokenHold {
        lock(&self.inner.holds).active += 1;
        TokenHold {
            log: self.clone(),
            released: false,
        }
    }

    /// Write a finished attempt now, or keep it when it is a token record and a hold
    /// is live.
    fn finished(&self, finished: Finished) {
        if finished.fields.operation == TOKEN_OPERATION {
            let mut holds = lock(&self.inner.holds);
            if holds.active > 0 {
                holds.pending.push(finished);
                return;
            }
        }
        self.write_record(&finished, false);
    }

    /// End one hold; the last one out writes the held records, every header withheld
    /// (the fallback record) when any hold since the last flush was cancelled.
    fn end_hold(&self, cancelled: bool) {
        let (pending, withhold) = {
            let mut holds = lock(&self.inner.holds);
            holds.active = holds.active.saturating_sub(1);
            holds.cancelled |= cancelled;
            if holds.active > 0 {
                return;
            }
            let withhold = std::mem::take(&mut holds.cancelled);
            (std::mem::take(&mut holds.pending), withhold)
        };
        for finished in &pending {
            self.write_record(finished, withhold);
        }
    }

    /// Build the record with the secrets known now, run the line check
    /// (case-insensitive) and write it; `withhold` writes the fallback record.
    fn write_record(&self, finished: &Finished, withhold: bool) {
        let secrets = &self.inner.secrets;
        if !withhold {
            let full = line(&finished.record(secrets, false));
            if !secrets.appears_in_ignoring_case(&full)
                && !secrets.appears_in_ignoring_case(&finished.fields.url)
            {
                self.write_line(&full);
                return;
            }
        }
        let fallback = line(&finished.record(secrets, true));
        if !secrets.appears_in_ignoring_case(&fallback) {
            self.write_line(&fallback);
        }
    }

    /// Write one finished line (with its `"\n"`) in a single `write_all`. The first
    /// failure closes the log and prints the warning once; nothing is retried.
    fn write_line(&self, line: &str) {
        let mut sink = lock(&self.inner.sink);
        let Some(file) = sink.as_mut() else {
            return;
        };
        if file.write_all(line.as_bytes()).is_ok() {
            return;
        }
        *sink = None;
        drop(sink);
        if let Some(mut stderr) = lock(&self.inner.warning).take() {
            let text = format!(
                "  {}\n",
                style::paint(Style::Yellow, WRITE_WARNING, self.inner.warning_color)
            );
            let _ = stderr.write_all(text.as_bytes());
            let _ = stderr.flush();
        }
    }
}

/// Keeps `oauth.token` records unwritten until the lease they issued is in the
/// [`SecretSet`], so a token the server echoes in a header of its own response is
/// masked like every other lease. [`TokenHold::release`] after registering the lease;
/// dropping the hold unreleased (the token future was cancelled) writes the held
/// records without URL or headers (`redactionFallback`).
pub struct TokenHold {
    log: DebugLog,
    released: bool,
}

impl TokenHold {
    /// The lease (if any) is registered: write the held records normally.
    pub fn release(mut self) {
        self.released = true;
        self.log.end_hold(false);
    }
}

impl Drop for TokenHold {
    fn drop(&mut self) {
        if !self.released {
            self.log.end_hold(true);
        }
    }
}

/// One attempt being recorded. Dropping it unfinished (the `send` future was dropped
/// by a deadline, a cancel or a signal) writes the cancelled record.
pub struct Attempt {
    log: DebugLog,
    started: Instant,
    /// `None` once finished.
    fields: Option<Fields>,
}

/// What an attempt records at entry to `send`.
struct Fields {
    timestamp: String,
    operation: &'static str,
    method: String,
    url: String,
    request_headers: OValue,
}

/// An attempt that ended, its response headers still raw (they are filtered when the
/// record is written, against the secrets known then).
struct Finished {
    fields: Fields,
    duration_ms: u64,
    outcome: Outcome,
}

/// How an attempt ended.
enum Outcome {
    Response {
        status: u16,
        status_text: &'static str,
        headers: HeaderMap,
    },
    Failed,
    Cancelled,
}

impl Attempt {
    /// The response headers arrived.
    pub fn response(mut self, status: StatusCode, headers: &HeaderMap) {
        self.finish(Outcome::Response {
            status: status.as_u16(),
            status_text: status.canonical_reason().unwrap_or(""),
            headers: headers.clone(),
        });
    }

    /// `send` returned an error (its text is never logged).
    pub fn failed(mut self) {
        self.finish(Outcome::Failed);
    }

    fn finish(&mut self, outcome: Outcome) {
        let Some(fields) = self.fields.take() else {
            return;
        };
        let duration_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.log.finished(Finished {
            fields,
            duration_ms,
            outcome,
        });
    }
}

impl Finished {
    /// The record in field order; `fallback` drops the URL and every header and adds
    /// `redactionFallback` (after a line-check hit or a cancelled token hold).
    fn record(&self, secrets: &SecretSet, fallback: bool) -> OValue {
        let fields = &self.fields;
        let text = |value: &str| OValue::String(value.to_owned());
        let mut request = vec![("method".to_owned(), text(&fields.method))];
        if !fallback {
            request.push(("url".to_owned(), text(&fields.url)));
            request.push(("headers".to_owned(), fields.request_headers.clone()));
        }
        let mut entries = vec![
            ("timestamp".to_owned(), text(&fields.timestamp)),
            (
                "durationMs".to_owned(),
                OValue::Number(serde_json::Number::from(self.duration_ms)),
            ),
            ("operation".to_owned(), text(fields.operation)),
            ("request".to_owned(), OValue::Object(request)),
        ];
        match &self.outcome {
            Outcome::Response {
                status,
                status_text,
                headers,
            } => {
                let mut response = vec![
                    (
                        "status".to_owned(),
                        OValue::Number(serde_json::Number::from(*status)),
                    ),
                    ("statusText".to_owned(), text(status_text)),
                ];
                if !fallback {
                    response.push((
                        "headers".to_owned(),
                        header_object(headers, RESPONSE_HEADERS, true, secrets),
                    ));
                }
                entries.push(("response".to_owned(), OValue::Object(response)));
            }
            Outcome::Failed => entries.push(("error".to_owned(), text(REQUEST_FAILED))),
            Outcome::Cancelled => entries.push(("error".to_owned(), text(REQUEST_CANCELLED))),
        }
        if fallback {
            entries.push(("redactionFallback".to_owned(), OValue::Bool(true)));
        }
        OValue::Object(entries)
    }
}

impl Drop for Attempt {
    fn drop(&mut self) {
        self.finish(Outcome::Cancelled);
    }
}

/// Compact JSON plus `"\n"` (serialising an `OValue` cannot fail).
fn line(record: &OValue) -> String {
    let mut text = json::to_compact(record).unwrap_or_default();
    text.push('\n');
    text
}

/// Every header name once (lowercase, byte order), repeated values joined by `, `,
/// each value written only when allowed.
fn header_object(
    headers: &HeaderMap,
    allowed: &[&str],
    response: bool,
    secrets: &SecretSet,
) -> OValue {
    let names: BTreeSet<&str> = headers.keys().map(|name| name.as_str()).collect();
    OValue::Object(
        names
            .into_iter()
            .map(|name| {
                let values: Vec<String> = headers
                    .get_all(name)
                    .iter()
                    .map(|value| header_value(name, value.as_bytes(), allowed, response, secrets))
                    .collect();
                (name.to_owned(), OValue::String(values.join(", ")))
            })
            .collect(),
    )
}

/// One header value per the allow-list (DESIGN §2.9).
pub fn header_value(
    name: &str,
    bytes: &[u8],
    allowed: &[&str],
    response: bool,
    secrets: &SecretSet,
) -> String {
    let location = response && name == "location";
    if !location && !allowed.contains(&name) {
        return MASK.to_owned();
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return MASK.to_owned();
    };
    let value = if location {
        sanitize_location(text)
    } else {
        text.to_owned()
    };
    if secrets.taints(text) || secrets.taints(&value) {
        MASK.to_owned()
    } else {
        value
    }
}

/// Scheme, host, port and path: no query, no fragment; userinfo becomes `[REDACTED]@`.
pub fn sanitize_url(url: &Url) -> String {
    let mut out = format!("{}://", url.scheme());
    if !url.username().is_empty() || url.password().is_some() {
        out.push_str("[REDACTED]@");
    }
    out.push_str(url.host_str().unwrap_or_default());
    if let Some(port) = url.port() {
        out.push_str(&format!(":{port}"));
    }
    out.push_str(url.path());
    out
}

/// A `location` value: an absolute URL as [`sanitize_url`], a relative reference as its
/// path alone, anything that does not parse as `***`.
pub fn sanitize_location(text: &str) -> String {
    match Url::parse(text) {
        Ok(url) if !url.cannot_be_a_base() && url.has_host() => sanitize_url(&url),
        Ok(_) => MASK.to_owned(),
        Err(url::ParseError::RelativeUrlWithoutBase) => Url::parse("http://relative.invalid/")
            .and_then(|base| base.join(text))
            .map(|url| url.path().to_owned())
            .unwrap_or_else(|_| MASK.to_owned()),
        Err(_) => MASK.to_owned(),
    }
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` (UTC), as JavaScript's `Date.prototype.toISOString`.
pub fn iso_timestamp(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = since.as_secs();
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX);
    let rest = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60,
        since.subsec_millis()
    )
}

/// Days since 1970-01-01 → (year, month, day) in the proleptic Gregorian calendar
/// (Howard Hinnant's `civil_from_days`).
pub(crate) fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month as u32, day as u32)
}

/// The SDK's default operation name of a raw read (`scm::RawRequest::read`; the SDK's
/// `RawRequest::named` is crate-private).
pub const RAW_READ_OPERATION: &str = "custom.read";

tokio::task_local! {
    /// The operation name a raw read sent inside [`named`] carries in the log.
    static RAW_READ_NAME: &'static str;
}

/// Run `future` so that every raw read it sends (operation [`RAW_READ_OPERATION`]) is
/// logged as `operation`: a CLI-side read that replaces a typed SDK call keeps that call's
/// operation name in the `--debug` log (G71, `airs.agentguard.list_scans`). Every other
/// operation (the token request included) keeps its own name.
///
/// Pin constraint: the name is a tokio task-local, so it reaches [`DebugTransport::send`]
/// only because scm-sdk 0.2.2 and 0.2.3 await `Transport::send` inside the caller's future (the
/// engine's `send_buffered` polls it in place; it spawns no task and hands no attempt to
/// another one). An SDK release that moved the attempt onto a spawned task, a channel or
/// a worker pool would log the read as `custom.read` again, with no compile error. Check
/// this before moving the pin; `tests/supplychain_reads.rs::g71::no_scan_page_is_logged_as_a_raw_read`
/// fails if any scan page (a single page, the G63 re-read, every page of a walk) is
/// logged under another name.
pub async fn named<F: std::future::Future>(operation: &'static str, future: F) -> F::Output {
    RAW_READ_NAME.scope(operation, future).await
}

/// The operation an attempt is logged under: a raw read inside [`named`] takes its name.
fn logged_operation(operation: &'static str) -> &'static str {
    if operation == RAW_READ_OPERATION {
        RAW_READ_NAME.try_with(|name| *name).unwrap_or(operation)
    } else {
        operation
    }
}

/// Records every attempt of the transport it wraps; a pass-through without a log, so a
/// `--debug` run uses the same HTTP stack as any other.
pub struct DebugTransport {
    inner: Arc<dyn Transport>,
    log: Option<DebugLog>,
}

impl DebugTransport {
    pub fn wrap(inner: Arc<dyn Transport>, log: Option<&DebugLog>) -> Arc<dyn Transport> {
        Arc::new(Self {
            inner,
            log: log.cloned(),
        })
    }
}

impl Transport for DebugTransport {
    fn send<'a>(
        &'a self,
        request: TransportRequest,
    ) -> BoxFuture<'a, scm::Result<TransportResponse>> {
        let Some(log) = &self.log else {
            return self.inner.send(request);
        };
        let attempt = log.begin(
            request.method(),
            request.url(),
            request.headers(),
            logged_operation(request.operation()),
        );
        observe(attempt, self.inner.send(request))
    }
}

/// Await `sent` and finish `attempt` from its result, which is returned unchanged.
/// Dropping the returned future first records a cancelled attempt.
fn observe<'a>(
    attempt: Attempt,
    sent: BoxFuture<'a, scm::Result<TransportResponse>>,
) -> BoxFuture<'a, scm::Result<TransportResponse>> {
    Box::pin(async move {
        let result = sent.await;
        match &result {
            Ok(response) => attempt.response(response.status(), response.headers()),
            Err(_) => attempt.failed(),
        }
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderName, HeaderValue};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A shared in-memory sink (thread-safe, for the log and the warning).
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);

    impl Shared {
        fn text(&self) -> String {
            String::from_utf8(lock(&self.0).clone()).unwrap()
        }
    }

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            lock(&self.0).extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Fails every write and counts the attempts.
    #[derive(Clone, Default)]
    struct Failing(Arc<AtomicUsize>);

    impl Write for Failing {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn memory_log(secrets: SecretSet) -> (DebugLog, Shared, Shared) {
        let sink = Shared::default();
        let warning = Shared::default();
        let log = DebugLog::with_sinks(
            PathBuf::from("/x/debug-api-1-a.jsonl"),
            Box::new(sink.clone()),
            Box::new(warning.clone()),
            false,
            secrets,
        );
        (log, sink, warning)
    }

    fn headers(pairs: &[(&str, &[u8])]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_bytes(value).unwrap(),
            );
        }
        map
    }

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    fn records(sink: &Shared) -> Vec<serde_json::Value> {
        sink.text()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    /// Replace the volatile timestamp and duration so whole lines can be compared.
    fn stable(line: &str) -> String {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let timestamp = value["timestamp"].as_str().unwrap().to_owned();
        let duration = value["durationMs"].to_string();
        line.replacen(&timestamp, "T", 1).replacen(
            &format!("\"durationMs\":{duration}"),
            "\"durationMs\":0",
            1,
        )
    }

    #[test]
    fn header_allow_list_and_masking() {
        let secrets = SecretSet::default();
        secrets.add("s3cr3t-value");
        let request = headers(&[
            ("authorization", b"Basic YWJjOmRlZg"),
            ("x-pan-token", b"tok"),
            ("x-payload-hash", b"abcdef0123"),
            ("cookie", b"a=b"),
            ("proxy-authorization", b"Bearer x"),
            ("x-unknown", b"plain"),
            ("user-agent", b"PAN-AIRS/0.1.1-rust-sdk"),
            ("service-name", b"api"),
            ("content-type", b"application/json"),
            ("accept", b"application/json"),
            ("x-tsg-id", b"100"),
            ("content-length", b"s3cr3t-value"),
        ]);
        let object = header_object(&request, REQUEST_HEADERS, false, &secrets);
        assert_eq!(
            json::to_compact(&object).unwrap(),
            r#"{"accept":"application/json","authorization":"***","content-length":"***","content-type":"application/json","cookie":"***","proxy-authorization":"***","service-name":"api","user-agent":"PAN-AIRS/0.1.1-rust-sdk","x-pan-token":"***","x-payload-hash":"***","x-tsg-id":"100","x-unknown":"***"}"#,
            "names sorted; only allow-listed values; an allow-listed secret masked"
        );
        let response = headers(&[
            ("set-cookie", b"id=1"),
            ("set-cookie", b"id=2"),
            ("vary", b"accept"),
            ("vary", b"origin"),
            ("x-request-id", b"r-1"),
            ("x-request-id", b"s3cr3t-value"),
            ("server", b"\xffbin"),
            ("date", b"Sat, 17 Oct 2026 11:03:44 GMT"),
            ("retry-after", b"0"),
            ("authorization", b"Bearer x"),
        ]);
        let object = header_object(&response, RESPONSE_HEADERS, true, &secrets);
        assert_eq!(
            json::to_compact(&object).unwrap(),
            r#"{"authorization":"***","date":"Sat, 17 Oct 2026 11:03:44 GMT","retry-after":"0","server":"***","set-cookie":"***, ***","vary":"accept, origin","x-request-id":"r-1, ***"}"#,
            "repeated values joined; non-UTF-8 and tainted values masked"
        );
        assert_eq!(
            header_value("x-request-id", b"r-1", REQUEST_HEADERS, false, &secrets),
            MASK,
            "response names are not allowed on requests"
        );
        assert_eq!(
            header_value("location", b"/x?y", REQUEST_HEADERS, false, &secrets),
            MASK,
            "a request location is not allow-listed"
        );
    }

    #[test]
    fn location_is_sanitised() {
        let secrets = SecretSet::default();
        let value = |text: &str| header_value("location", text.as_bytes(), &[], true, &secrets);
        assert_eq!(
            value("https://user:pw@example.com:8443/a/b?token=x#f"),
            "https://[REDACTED]@example.com:8443/a/b"
        );
        assert_eq!(value("/next/page?offset=100&k=v"), "/next/page");
        assert_eq!(value("next?x=1"), "/next");
        assert_eq!(value("//other.example/p?q"), "/p");
        assert_eq!(value("http://[::1]:9/x"), "http://[::1]:9/x");
        assert_eq!(value("http://[bad"), MASK);
        assert_eq!(value("mailto:a@b"), MASK);
        secrets.add("leaky-secret");
        assert_eq!(value("/a/leaky-secret?x"), MASK);
    }

    #[test]
    fn urls_lose_query_fragment_and_userinfo() {
        assert_eq!(
            sanitize_url(&url(
                "https://api.example.com/aisec/v1/mgmt/profiles/tsg/100?offset=0&limit=100&latest=true"
            )),
            "https://api.example.com/aisec/v1/mgmt/profiles/tsg/100"
        );
        assert_eq!(
            sanitize_url(&url(
                "http://127.0.0.1:8080/v1/scan/results?scan_ids=a#frag"
            )),
            "http://127.0.0.1:8080/v1/scan/results"
        );
        assert_eq!(
            sanitize_url(&url("https://u:p@h.example/x?")),
            "https://[REDACTED]@h.example/x"
        );
        assert_eq!(
            sanitize_url(&url("https://u@h.example:443/")),
            "https://[REDACTED]@h.example/",
            "the default port is not written"
        );
        assert_eq!(sanitize_url(&url("http://[::1]:9/x")), "http://[::1]:9/x");
    }

    #[test]
    fn record_schema_and_field_order() {
        let (log, sink, warning) = memory_log(SecretSet::default());
        let attempt = log.begin(
            &Method::GET,
            &url("http://127.0.0.1:9/aisec/v1/mgmt/profiles/tsg/100?offset=0&limit=100"),
            &headers(&[("authorization", b"Bearer t"), ("service-name", b"api")]),
            "airs.management.profiles.list",
        );
        attempt.response(
            StatusCode::OK,
            &headers(&[("content-type", b"application/json"), ("x-other", b"v")]),
        );
        log.begin(
            &Method::POST,
            &url("http://h.example/oauth/token"),
            &HeaderMap::new(),
            "oauth.token",
        )
        .failed();
        drop(log.begin(
            &Method::POST,
            &url("http://h.example/v1/scan/sync/request"),
            &HeaderMap::new(),
            "airs.scan.sync",
        ));
        log.begin(
            &Method::GET,
            &url("http://h.example/x"),
            &HeaderMap::new(),
            "op",
        )
        .response(StatusCode::from_u16(599).unwrap(), &HeaderMap::new());
        let lines: Vec<String> = sink.text().lines().map(stable).collect();
        assert_eq!(
            lines,
            [
                r#"{"timestamp":"T","durationMs":0,"operation":"airs.management.profiles.list","request":{"method":"GET","url":"http://127.0.0.1:9/aisec/v1/mgmt/profiles/tsg/100","headers":{"authorization":"***","service-name":"api"}},"response":{"status":200,"statusText":"OK","headers":{"content-type":"application/json","x-other":"***"}}}"#,
                r#"{"timestamp":"T","durationMs":0,"operation":"oauth.token","request":{"method":"POST","url":"http://h.example/oauth/token","headers":{}},"error":"Request failed"}"#,
                r#"{"timestamp":"T","durationMs":0,"operation":"airs.scan.sync","request":{"method":"POST","url":"http://h.example/v1/scan/sync/request","headers":{}},"error":"Request cancelled before a response"}"#,
                r#"{"timestamp":"T","durationMs":0,"operation":"op","request":{"method":"GET","url":"http://h.example/x","headers":{}},"response":{"status":599,"statusText":"","headers":{}}}"#,
            ]
        );
        assert!(sink.text().ends_with("}\n"));
        assert_eq!(warning.text(), "");
        for record in records(&sink) {
            let timestamp = record["timestamp"].as_str().unwrap();
            assert_eq!(timestamp.len(), 24, "{timestamp}");
            assert!(timestamp.ends_with('Z'));
            assert!(record.get("body").is_none());
        }
    }

    #[test]
    fn line_check_falls_back_then_skips() {
        let secrets = SecretSet::default();
        secrets.add("path-secret-1");
        let (log, sink, _) = memory_log(secrets.clone());
        log.begin(
            &Method::GET,
            &url("http://h.example/a/path-secret-1/b"),
            &headers(&[("user-agent", b"ua")]),
            "airs.op",
        )
        .response(StatusCode::NOT_FOUND, &headers(&[("date", b"d")]));
        let lines: Vec<String> = sink.text().lines().map(stable).collect();
        assert_eq!(
            lines,
            [
                r#"{"timestamp":"T","durationMs":0,"operation":"airs.op","request":{"method":"GET"},"response":{"status":404,"statusText":"Not Found"},"redactionFallback":true}"#
            ]
        );
        // A secret the fallback itself would carry: nothing is written.
        secrets.add("airs.secret.op");
        log.begin(
            &Method::GET,
            &url("http://h.example/"),
            &HeaderMap::new(),
            "airs.secret.op",
        )
        .failed();
        assert_eq!(sink.text().lines().count(), 1);
        // A lease registered after `begin` is still caught at write time.
        let attempt = log.begin(
            &Method::GET,
            &url("http://h.example/late-lease-9"),
            &HeaderMap::new(),
            "airs.op",
        );
        secrets.add("late-lease-9");
        attempt.failed();
        assert!(
            sink.text()
                .lines()
                .nth(1)
                .unwrap()
                .contains("\"redactionFallback\":true")
        );
        assert!(!sink.text().contains("path-secret-1"));
        assert!(!sink.text().contains("late-lease-9"));
    }

    /// A secret echoed as a response header name is written lowercased; the line
    /// check ignores case, so the record falls back.
    #[test]
    fn line_check_catches_a_case_folded_header_name() {
        let secrets = SecretSet::default();
        secrets.add("FAKE-SECRET-100");
        let (log, sink, _) = memory_log(secrets);
        log.begin(
            &Method::GET,
            &url("http://h.example/"),
            &HeaderMap::new(),
            "airs.op",
        )
        .response(StatusCode::OK, &headers(&[("FAKE-SECRET-100", b"1")]));
        let text = sink.text();
        assert!(text.contains("\"redactionFallback\":true"), "{text}");
        assert!(
            !text.to_ascii_lowercase().contains("fake-secret-100"),
            "{text}"
        );
    }

    /// A token record waits for its hold: the lease registered before `release` masks
    /// the token echoed in the response's own headers; other records are not held.
    #[test]
    fn token_records_wait_for_the_lease() {
        let secrets = SecretSet::default();
        let (log, sink, _) = memory_log(secrets.clone());
        let hold = log.hold_tokens();
        log.begin(
            &Method::POST,
            &url("http://h.example/oauth/token"),
            &HeaderMap::new(),
            TOKEN_OPERATION,
        )
        .response(
            StatusCode::OK,
            &headers(&[
                ("x-request-id", b"lease-77"),
                ("content-type", b"application/json"),
            ]),
        );
        assert_eq!(sink.text(), "", "held until the lease is registered");
        log.begin(
            &Method::GET,
            &url("http://h.example/a"),
            &HeaderMap::new(),
            "airs.op",
        )
        .failed();
        assert_eq!(
            sink.text().lines().count(),
            1,
            "only token records are held"
        );
        secrets.add("lease-77");
        hold.release();
        let records = records(&sink);
        assert_eq!(records.len(), 2);
        assert_eq!(records[1]["operation"], TOKEN_OPERATION);
        assert_eq!(records[1]["response"]["headers"]["x-request-id"], MASK);
        assert_eq!(
            records[1]["response"]["headers"]["content-type"],
            "application/json"
        );
        assert!(!sink.text().contains("lease-77"));
        // Without a hold, a token record is written at once.
        log.begin(
            &Method::POST,
            &url("http://h.example/oauth/token"),
            &HeaderMap::new(),
            TOKEN_OPERATION,
        )
        .failed();
        assert_eq!(sink.text().lines().count(), 3);
    }

    /// A hold dropped unreleased (the token future was cancelled) writes the held
    /// records without URL or headers; nested holds flush when the last one ends.
    #[test]
    fn cancelled_token_hold_withholds_headers() {
        let (log, sink, _) = memory_log(SecretSet::default());
        let outer = log.hold_tokens();
        let inner = log.hold_tokens();
        log.begin(
            &Method::POST,
            &url("http://h.example/oauth/token"),
            &HeaderMap::new(),
            TOKEN_OPERATION,
        )
        .response(
            StatusCode::OK,
            &headers(&[("x-request-id", b"unregistered-lease")]),
        );
        drop(inner);
        assert_eq!(sink.text(), "", "an outer hold is still live");
        outer.release();
        let lines: Vec<String> = sink.text().lines().map(stable).collect();
        assert_eq!(
            lines,
            [
                r#"{"timestamp":"T","durationMs":0,"operation":"oauth.token","request":{"method":"POST"},"response":{"status":200,"statusText":"OK"},"redactionFallback":true}"#
            ]
        );
        // The cancellation does not outlive its flush.
        let hold = log.hold_tokens();
        log.begin(
            &Method::POST,
            &url("http://h.example/oauth/token"),
            &HeaderMap::new(),
            TOKEN_OPERATION,
        )
        .response(StatusCode::OK, &HeaderMap::new());
        hold.release();
        assert!(
            !sink
                .text()
                .lines()
                .nth(1)
                .unwrap()
                .contains("redactionFallback")
        );
    }

    #[test]
    fn write_failure_closes_the_log_and_warns_once() {
        let failing = Failing::default();
        let warning = Shared::default();
        let log = DebugLog::with_sinks(
            PathBuf::from("/x"),
            Box::new(failing.clone()),
            Box::new(warning.clone()),
            false,
            SecretSet::default(),
        );
        for _ in 0..3 {
            log.begin(
                &Method::GET,
                &url("http://h.example/"),
                &HeaderMap::new(),
                "op",
            )
            .response(StatusCode::OK, &HeaderMap::new());
        }
        assert_eq!(failing.0.load(Ordering::SeqCst), 1, "no write is retried");
        assert_eq!(
            warning.text(),
            "  ⚠ Debug: the API log could not be written; it is incomplete\n"
        );
        let coloured = Shared::default();
        let log = DebugLog::with_sinks(
            PathBuf::from("/x"),
            Box::new(Failing::default()),
            Box::new(coloured.clone()),
            true,
            SecretSet::default(),
        );
        log.begin(
            &Method::GET,
            &url("http://h.example/"),
            &HeaderMap::new(),
            "op",
        )
        .failed();
        assert_eq!(
            coloured.text(),
            "  \u{1b}[33m⚠ Debug: the API log could not be written; it is incomplete\u{1b}[39m\n"
        );
    }

    #[test]
    fn observed_results_are_returned_unchanged() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        // A log that cannot be written must not change the response.
        let warning = Shared::default();
        let log = DebugLog::with_sinks(
            PathBuf::from("/x"),
            Box::new(Failing::default()),
            Box::new(warning.clone()),
            false,
            SecretSet::default(),
        );
        let attempt = log.begin(
            &Method::GET,
            &url("http://h.example/"),
            &HeaderMap::new(),
            "op",
        );
        let response = runtime
            .block_on(observe(
                attempt,
                Box::pin(async {
                    Ok(TransportResponse::new(
                        StatusCode::ACCEPTED,
                        headers(&[("x-a", b"1")]),
                        Box::pin(futures_util::stream::empty()),
                    ))
                }),
            ))
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.headers()["x-a"], "1");
        assert!(!warning.text().is_empty());

        let (log, sink, _) = memory_log(SecretSet::default());
        let attempt = log.begin(
            &Method::GET,
            &url("http://h.example/"),
            &HeaderMap::new(),
            "op",
        );
        let failure = runtime.block_on(observe(
            attempt,
            Box::pin(async { Err(scm::Error::new(scm::ErrorKind::Transport)) }),
        ));
        assert_eq!(failure.err().unwrap().kind(), scm::ErrorKind::Transport);
        // Dropped before completion: the cancelled record.
        let attempt = log.begin(
            &Method::GET,
            &url("http://h.example/"),
            &HeaderMap::new(),
            "op",
        );
        drop(observe(attempt, Box::pin(futures_util::future::pending())));
        let errors: Vec<String> = records(&sink)
            .iter()
            .map(|record| record["error"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(errors, [REQUEST_FAILED, REQUEST_CANCELLED]);
    }

    #[test]
    fn timestamps_are_iso_utc_with_milliseconds() {
        let at = |ms: u64| iso_timestamp(UNIX_EPOCH + std::time::Duration::from_millis(ms));
        assert_eq!(at(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(at(1_792_062_224_500), "2026-10-15T11:03:44.500Z");
        assert_eq!(at(951_782_400_007), "2000-02-29T00:00:00.007Z");
        assert_eq!(at(1_709_251_199_999), "2024-02-29T23:59:59.999Z");
        assert_eq!(at(4_107_542_400_000), "2100-03-01T00:00:00.000Z");
        assert_eq!(at(253_402_300_799_999), "9999-12-31T23:59:59.999Z");
    }

    /// G71: a raw read sent inside `named` is logged under the typed call's operation name;
    /// outside it, and for every other operation (the token request included), the
    /// request's own name stays.
    #[test]
    fn named_raw_reads_take_the_scoped_operation() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let inside = runtime.block_on(named("airs.agentguard.list_scans", async {
            (
                logged_operation(RAW_READ_OPERATION),
                logged_operation(TOKEN_OPERATION),
                logged_operation("custom.request"),
            )
        }));
        assert_eq!(
            inside,
            (
                "airs.agentguard.list_scans",
                TOKEN_OPERATION,
                "custom.request"
            )
        );
        assert_eq!(logged_operation(RAW_READ_OPERATION), RAW_READ_OPERATION);
    }
}
