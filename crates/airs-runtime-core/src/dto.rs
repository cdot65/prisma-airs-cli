//! Versioned strict request/response envelopes (contract §4). Unknown fields and
//! versions are refused before effects.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

/// Stable error codes (contract §4.4). Never carry raw SDK/SQLite/HTTP text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    UnsupportedVersion,
    Unauthenticated,
    Forbidden,
    NotFound,
    Conflict,
    IdempotencyConflict,
    ControlRestored,
    Overloaded,
    QuotaExhausted,
    StorageUnavailable,
    MaintenanceRequired,
    LowSpace,
    CredentialBlocked,
    UpstreamUnavailable,
    Canceled,
    /// A result would exceed its response bound (contract §5; M0.6 finding).
    ResultTooLarge,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ErrorEnvelope {
    pub schema_version: u32,
    pub code: ErrorCode,
    /// Fixed, terminal-safe reason from a compiled vocabulary.
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_status: Option<CredentialStatus>,
}

/// Bounded credential state carried by error envelopes (schema `$defs/error`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStatus {
    Ok,
    NeedsCredentials,
    Cooldown,
}

/// Decode an error envelope strictly: known fields only, version 1, bounded reason.
pub fn parse_error(raw: &str) -> Result<ErrorEnvelope, ErrorCode> {
    // Optional members are absent or valid, never an explicit null.
    let shape: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| ErrorCode::InvalidRequest)?;
    for k in ["retry_after_seconds", "job_id", "credential_status"] {
        if shape.get(k).is_some_and(serde_json::Value::is_null) {
            return Err(ErrorCode::InvalidRequest);
        }
    }
    let e: ErrorEnvelope = serde_json::from_str(raw).map_err(|_| ErrorCode::InvalidRequest)?;
    if e.job_id.as_deref().is_some_and(|j| !is_job_id(j)) {
        return Err(ErrorCode::InvalidRequest);
    }
    if e.schema_version != SCHEMA_VERSION {
        return Err(ErrorCode::UnsupportedVersion);
    }
    if e.reason.chars().count() > 256 {
        return Err(ErrorCode::InvalidRequest);
    }
    Ok(e)
}

/// Envelope for every tool input: version first, then a typed body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Request<T> {
    pub schema_version: u32,
    pub tenant: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    /// Recovery epoch the client learned from `service_info` when it created the
    /// key (contract §4.2). Required exactly when `idempotency_key` is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_epoch: Option<String>,
    pub params: T,
}

/// Canonical lowercase UUIDv4: 8-4-4-4-12 hex, version nibble 4, variant 8/9/a/b.
pub fn is_uuid_v4(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
        && b[14] == b'4'
        && matches!(b[19], b'8' | b'9' | b'a' | b'b')
}

/// `j1_<epoch 8 hex>_<uuid v4>` (schema `$defs/job_id`).
pub fn is_job_id(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("j1_") else {
        return false;
    };
    let (epoch, uuid) = rest.split_at_checked(8).unwrap_or(("", ""));
    is_epoch(epoch) && uuid.strip_prefix('_').is_some_and(is_uuid_v4)
}

fn is_tenant(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 63
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

fn is_key(s: &str) -> bool {
    (16..=64).contains(&s.len())
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

fn is_epoch(s: &str) -> bool {
    s.len() == 8
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// Decode a tool request and enforce the envelope rules that depend on the
/// operation: idempotent tools require both `idempotency_key` and
/// `expected_epoch`; other tools must carry neither.
pub fn parse_tool_request<T: for<'de> Deserialize<'de>>(
    raw: &str,
    idempotent: bool,
) -> Result<Request<T>, ErrorCode> {
    // An explicit `null` is not "absent": the frozen schema forbids the property
    // itself on non-idempotent tools and requires a string on idempotent ones.
    let shape: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| ErrorCode::InvalidRequest)?;
    for k in ["idempotency_key", "expected_epoch"] {
        if shape.get(k).is_some_and(serde_json::Value::is_null) {
            return Err(ErrorCode::InvalidRequest);
        }
    }
    let req: Request<T> = parse_request(raw)?;
    if !is_tenant(&req.tenant) {
        return Err(ErrorCode::InvalidRequest);
    }
    match (
        idempotent,
        req.idempotency_key.as_deref(),
        req.expected_epoch.as_deref(),
    ) {
        (true, Some(k), Some(e)) if is_key(k) && is_epoch(e) => Ok(req),
        (false, None, None) => Ok(req),
        _ => Err(ErrorCode::InvalidRequest),
    }
}

/// A key minted under another recovery epoch never creates or joins a job.
pub fn check_epoch<T>(req: &Request<T>, current_epoch: &str) -> Result<(), ErrorCode> {
    match req.expected_epoch.as_deref() {
        Some(e) if e != current_epoch => Err(ErrorCode::ControlRestored),
        _ => Ok(()),
    }
}

/// Parse a request strictly: unknown fields anywhere and other versions are refused.
pub fn parse_request<T: for<'de> Deserialize<'de>>(raw: &str) -> Result<Request<T>, ErrorCode> {
    #[derive(Deserialize)]
    struct VersionOnly {
        schema_version: u32,
    }
    let version: VersionOnly = serde_json::from_str(raw).map_err(|_| ErrorCode::InvalidRequest)?;
    if version.schema_version != SCHEMA_VERSION {
        return Err(ErrorCode::UnsupportedVersion);
    }
    serde_json::from_str(raw).map_err(|_| ErrorCode::InvalidRequest)
}

/// The frozen tool catalogue and schemas (contract §4.3; generated by
/// `schemas/generate.py`). The server lists and validates tools from it; a change is a
/// contract revision.
pub const TOOLS_V1: &str = include_str!("../schemas/tools.v1.json");

/// The longest `history_coverage` range (contract §4.3: ≤ 90 days, half-open).
pub const MAX_COVERAGE_RANGE_MS: i64 = 90 * 86_400_000;
/// Page sizes (contract §4.2): 1–100 items, default 50.
pub const MAX_PAGE_ITEMS: usize = 100;
pub const DEFAULT_PAGE_ITEMS: usize = 50;

/// `params` of a tool that takes none.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

/// A half-open UTC range (`$defs/range`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Range {
    pub since: String,
    pub until: String,
}

impl Range {
    /// Epoch milliseconds `[since, until)`: both `$defs/utc` instants, since < until,
    /// and at most `max_ms` long.
    pub fn bounds(&self, max_ms: i64) -> Result<(i64, i64), ErrorCode> {
        let since = utc_ms(&self.since).ok_or(ErrorCode::InvalidRequest)?;
        let until = utc_ms(&self.until).ok_or(ErrorCode::InvalidRequest)?;
        if since >= until || until - since > max_ms {
            return Err(ErrorCode::InvalidRequest);
        }
        Ok((since, until))
    }
}

/// `$defs/utc`: `YYYY-MM-DDTHH:MM:SS[.f{1,9}]Z`, as epoch milliseconds.
pub fn utc_ms(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    let shape = b.len() >= 20
        && b.last() == Some(&b'Z')
        && b[..19].iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            _ => c.is_ascii_digit(),
        })
        && match &b[19..b.len() - 1] {
            [] => true,
            [b'.', digits @ ..] => {
                (1..=9).contains(&digits.len()) && digits.iter().all(u8::is_ascii_digit)
            }
            _ => false,
        };
    if !shape {
        return None;
    }
    crate::time::parse_iso(text)
}

/// `$defs/page`: an optional signed cursor and an item limit.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Page {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

impl Page {
    /// The requested item count: 1–100, default 50.
    pub fn limit(&self) -> Result<usize, ErrorCode> {
        match self.limit {
            None => Ok(DEFAULT_PAGE_ITEMS),
            Some(n) if (1..=MAX_PAGE_ITEMS as u64).contains(&n) => Ok(n as usize),
            Some(_) => Err(ErrorCode::InvalidRequest),
        }
    }
}

/// `history_coverage` parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CoverageParams {
    pub range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<Page>,
}

impl CoverageParams {
    /// `$defs/app_ids`: at most 100 IDs of 1–4096 characters, without controls.
    pub fn app_ids(&self) -> Result<Vec<String>, ErrorCode> {
        let ids = self.app_ids.clone().unwrap_or_default();
        if ids.len() > 100
            || ids.iter().any(|id| {
                id.is_empty() || id.chars().count() > 4096 || id.chars().any(char::is_control)
            })
        {
            return Err(ErrorCode::InvalidRequest);
        }
        let mut ids = ids;
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}

/// `jobs_list` parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct JobsListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<Page>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub states: Option<Vec<String>>,
}

/// `jobs_get` and `jobs_cancel` parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct JobIdParams {
    pub job_id: String,
}

/// `collection_start` parameters: `recent` (the narrowest rolling selector, 24 h on a
/// first collection) or an explicit `backfill` range.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CollectionParams {
    pub mode: CollectionMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CollectionMode {
    Recent,
    Backfill,
}

/// The longest explicit backfill selector (contract §6: ≤ 87,600 h).
pub const MAX_BACKFILL_RANGE_MS: i64 = 87_600 * 3_600_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    struct Echo {
        text: String,
    }

    #[test]
    fn coverage_parameters_follow_the_frozen_definitions() {
        assert_eq!(utc_ms("1970-01-01T00:00:01Z"), Some(1000));
        assert_eq!(utc_ms("1970-01-01T00:00:01.5Z"), Some(1500));
        for bad in [
            "1970-01-01T00:00:01+00:00",
            "1970-01-01 00:00:01Z",
            "1970-01-01T00:00:01.Z",
            "1970-01-01T00:00:01.1234567890Z",
            "1970-01-01T00:00Z",
        ] {
            assert_eq!(utc_ms(bad), None, "{bad}");
        }
        let range = |a: &str, b: &str| Range {
            since: a.into(),
            until: b.into(),
        };
        assert_eq!(
            range("2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z").bounds(MAX_COVERAGE_RANGE_MS),
            Ok((1_767_225_600_000, 1_767_312_000_000))
        );
        for (a, b) in [
            ("2026-01-02T00:00:00Z", "2026-01-01T00:00:00Z"),
            ("2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"),
            ("2026-01-01T00:00:00Z", "2026-04-02T00:00:00Z"),
        ] {
            assert_eq!(
                range(a, b).bounds(MAX_COVERAGE_RANGE_MS),
                Err(ErrorCode::InvalidRequest)
            );
        }
        assert_eq!(Page::default().limit(), Ok(50));
        for n in [0, 101] {
            let page = Page {
                cursor: None,
                limit: Some(n),
            };
            assert_eq!(page.limit(), Err(ErrorCode::InvalidRequest));
        }
        let params = |ids: Vec<&str>| CoverageParams {
            range: range("2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z"),
            app_ids: Some(ids.into_iter().map(String::from).collect()),
            page: None,
        };
        assert_eq!(
            params(vec!["b", "a", "b"]).app_ids(),
            Ok(vec!["a".into(), "b".into()])
        );
        assert!(params(vec![""]).app_ids().is_err());
        assert!(params(vec!["a\u{85}"]).app_ids().is_err());
        assert!(params(vec!["x"; 101]).app_ids().is_err());
        assert!(
            serde_json::from_str::<CoverageParams>(
                r#"{"range":{"since":"a","until":"b"},"sql":"x"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn strict_envelope_refuses_unknown_fields_and_versions() {
        let ok = r#"{"schema_version":1,"tenant":"t1","params":{"text":"hi"}}"#;
        assert_eq!(parse_request::<Echo>(ok).unwrap().params.text, "hi");
        let extra = r#"{"schema_version":1,"tenant":"t1","params":{"text":"hi","x":1}}"#;
        assert_eq!(
            parse_request::<Echo>(extra).unwrap_err(),
            ErrorCode::InvalidRequest
        );
        let top = r#"{"schema_version":1,"tenant":"t1","sql":"drop","params":{"text":"hi"}}"#;
        assert_eq!(
            parse_request::<Echo>(top).unwrap_err(),
            ErrorCode::InvalidRequest
        );
        let v2 = r#"{"schema_version":2,"tenant":"t1","params":{"text":"hi"}}"#;
        assert_eq!(
            parse_request::<Echo>(v2).unwrap_err(),
            ErrorCode::UnsupportedVersion
        );
    }
}
