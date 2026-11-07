//! Historical schema-2 runtime collector retained for frozen-reference unit tests.
//! The runtime environment report (TS `src/reports/runtime.ts`, DESIGN §18.14): seven
//! read-only sources collected one after the other (choice 17) through TypeScript's
//! `collect` loop, the analysis and its eight finding rules, and the schema-2 model the
//! HTML renderer reads (`runtime_html.rs`).
//!
//! Only aggregates and labels are kept: no ids, tenant ids, session ids, prompts, users,
//! key names, auth codes or error bodies. Missing data is unknown, never zero; with the
//! response defaults off an absent profile list is no record array (D214, G27).

use super::{
    Finding, Health, Interrupted, LabelRule, Priority, ReadError, Source, Status, iso, source,
};
use crate::json::OValue;
use std::collections::HashMap;

/// The report schema.
pub const SCHEMA_VERSION: u32 = 2;
/// The window description (TS).
pub const WINDOW: &str =
    "Rolling last 24 hours, UTC (approximate for server-relative application queries)";

/// The page size of the inventories and the application overview.
pub const PAGE_SIZE: u64 = 100;
/// The page size of the session inventory.
pub const SESSION_PAGE: u64 = 25;
const DAY_MS: i64 = 86_400_000;
const MAX_SAFE: u64 = 9_007_199_254_740_991;

/// The scope statements (TS `limitations`).
pub const LIMITATIONS: [&str; 8] = [
    "This is a read-only operational review, not an uptime SLA, compliance attestation, or proof that attacks succeeded or were blocked. No health score is invented.",
    "Application activity uses the API’s rolling one-day window; each paginated request evaluates its own server-relative window. Collection is not a transactionally consistent historical snapshot. Ingestion may lag.",
    "Application buckets, session inventory, chart sessions, and detector violations are different measurements. They are not added together. Session inventory summaries use timestamp-eligible entries only; absent data is unknown.",
    "Application buckets are keyed by registered application ID plus the literal scan metadata.app_name, which can differ from registered names. Do not sum session counts as unique users or unique tenant-wide sessions.",
    "Current configuration is not a configuration-change audit. Missing policy settings are unknown, not disabled. Inactive profiles can be intentional; current profile state may differ from the revision used by a historical scan.",
    "Daily charts, rankings and severity trends come from their own one-day endpoints. Rankings are a server-selected subset, not a complete inventory. Per-app token summaries and drill-downs require longer windows; no daily token usage or previous-day comparison is inferred.",
    "The legacy ScanLogsClient / scan-logs query path is broken and under refactor. This report uses the verified dashboard session APIs instead. It never automatically fetches transactions or stored scan content.",
    "Prompts, responses, user identities/IPs, API keys, auth codes, tenant IDs, and raw errors are omitted. Application/profile names and configuration metadata remain confidential; review before sharing.",
];

/// The seven reads, as ordered JSON documents (the paged ones take the offset of the next
/// page).
pub(crate) trait Reader {
    async fn applications(&self, offset: u64) -> Result<OValue, ReadError>;
    async fn profiles(&self, offset: u64) -> Result<OValue, ReadError>;
    async fn customer_apps(&self, offset: u64) -> Result<OValue, ReadError>;
    async fn sessions(&self, offset: u64) -> Result<OValue, ReadError>;
    async fn sessions_chart(&self) -> Result<OValue, ReadError>;
    async fn top_applications(&self) -> Result<OValue, ReadError>;
    async fn violations_trend(&self) -> Result<OValue, ReadError>;
}

/// The report options and clock.
pub struct Options<'a> {
    pub title: &'a str,
    pub max_pages: u32,
    /// Milliseconds since the Unix epoch.
    pub now: &'a dyn Fn() -> i64,
}

/// A severity breakdown (TS `ReportSeverity`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Severity {
    pub critical: Option<u64>,
    pub high: Option<u64>,
    pub medium: Option<u64>,
    pub low: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Application {
    pub name: String,
    pub sessions: Option<u64>,
    pub violating_sessions: Option<u64>,
    pub registered: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub revision: Option<u64>,
    pub active: Option<bool>,
    pub modified: Option<String>,
    pub timeout_actions: Vec<String>,
    pub storage_masking: &'static str,
    /// One row per active protection block (TS 7.3.0 `protectionConfigurations`, §13.17).
    pub protection_configurations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredApp {
    pub name: String,
    pub environment: String,
    pub cloud: String,
    pub model: String,
    pub key_associations: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sessions {
    pub entries: Option<u64>,
    pub statuses: Vec<(String, u64)>,
    pub violating_sessions: Option<u64>,
    pub missing_timestamps: u64,
    pub outside_window: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChartBucket {
    pub time: String,
    pub sessions: Option<u64>,
    pub violating_sessions: Option<u64>,
    pub violations: Severity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopApplication {
    pub name: String,
    pub violations: Option<u64>,
    pub detectors: Vec<(String, Option<u64>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrendBucket {
    pub time: String,
    pub violations: Severity,
}

/// The schema-2 model (TS `RuntimeDailyReport`).
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub title: String,
    pub generated_at: String,
    pub collection_started_at: String,
    pub window_start: String,
    pub window_end: String,
    pub health: Health,
    pub sources: Vec<Source>,
    pub findings: Vec<Finding>,
    pub sessions_total: Option<u64>,
    pub violating_total: Option<u64>,
    pub violation_rate: Option<f64>,
    pub applications: Vec<Application>,
    pub profiles: Vec<Profile>,
    pub registered_apps: Vec<RegisteredApp>,
    pub sessions: Sessions,
    pub chart_sessions: Option<u64>,
    pub chart_violating: Option<u64>,
    pub chart_buckets: Vec<ChartBucket>,
    pub top_applications: Vec<TopApplication>,
    pub violation_trend: Vec<TrendBucket>,
    pub limitations: Vec<String>,
}

/// The runtime report's label rule (TS `label` of `runtime.ts`): `\p{Cc}\p{Cf}` →
/// space, JavaScript trim, empty → `Unknown`, then the first 240 UTF-16 units.
const RUNTIME_LABEL: LabelRule = LabelRule {
    units: 240,
    trim: true,
};

/// TS `label` ([`RUNTIME_LABEL`]); anything but a string is `Unknown`.
pub fn label(value: Option<&OValue>) -> String {
    super::label_with(value, RUNTIME_LABEL)
}

/// [`label`] of a string.
pub fn label_text(text: &str) -> String {
    super::label_text_with(text, RUNTIME_LABEL)
}

/// The sections a protection block row names, in TS 7.3.0's order.
const PROTECTION_SECTIONS: [&str; 4] = [
    "model-protection",
    "agent-protection",
    "app-protection",
    "data-protection",
];

/// One protection row: `AI[<i>] <model-type> / <direction|legacy>: <sections>`, the
/// sections that hold a non-empty object or array.
fn protection_row(index: usize, model_type: &str, place: &str, block: &OValue) -> String {
    let sections: Vec<&str> = PROTECTION_SECTIONS
        .into_iter()
        .filter(|key| match block.get(key) {
            Some(OValue::Object(fields)) => !fields.is_empty(),
            Some(OValue::Array(items)) => !items.is_empty(),
            _ => false,
        })
        .collect();
    let sections = if sections.is_empty() {
        "No modeled protection sections returned".to_owned()
    } else {
        sections.join(", ")
    };
    format!("AI[{index}] {model_type} / {place}: {sections}")
}

/// TS 7.3.0 `protectionConfigurations`: every active protection block of a policy (a
/// legacy entry's `model-configuration`, a directional entry's directions; §13.17), in
/// the answer's order. The page is read as the SDK serialises it, so known directions
/// come in `prompt`, `response`, `tool-call`, `tool-response` order.
pub fn protection_configurations(policy: Option<&OValue>) -> Vec<String> {
    let mut rows = Vec::new();
    let entries = policy
        .and_then(|policy| policy.get("ai-security-profiles"))
        .and_then(OValue::as_array)
        .unwrap_or_default();
    for (index, entry) in entries.iter().enumerate() {
        if entry.as_object().is_none() {
            continue;
        }
        let directions = entry.get("content-type-configurations");
        let directional = entry.get("content-type-mode").and_then(OValue::as_str)
            == Some(crate::profile_policy::PER_CONTENT_TYPE)
            || directions.is_some();
        let model_type = label(entry.get("model-type"));
        if !directional && let Some(block @ OValue::Object(_)) = entry.get("model-configuration") {
            rows.push(protection_row(index, &model_type, "legacy", block));
        }
        for (direction, block) in directions.and_then(OValue::as_object).unwrap_or_default() {
            if matches!(block, OValue::Object(_)) {
                rows.push(protection_row(
                    index,
                    &model_type,
                    &label_text(direction),
                    block,
                ));
            }
        }
    }
    rows
}

/// TS `count`: a safe integer ≥ 0, else `None`.
pub fn count(value: Option<&OValue>) -> Option<u64> {
    let Some(OValue::Number(number)) = value else {
        return None;
    };
    if let Some(integer) = number.as_u64() {
        return (integer <= MAX_SAFE).then_some(integer);
    }
    if number.is_i64() {
        return None;
    }
    match number.as_f64() {
        Some(value) if value >= 0.0 && value.fract() == 0.0 && value <= MAX_SAFE as f64 => {
            // An integral value within the safe range converts exactly.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let integer = value as u64;
            Some(integer)
        }
        _ => None,
    }
}

/// TS `total`: `None` when any value is, else the sum through [`count`].
fn total(values: impl IntoIterator<Item = Option<u64>>) -> Option<u64> {
    let mut sum: u128 = 0;
    for value in values {
        sum += u128::from(value?);
    }
    u64::try_from(sum).ok().filter(|sum| *sum <= MAX_SAFE)
}

/// A JSON number as `f64` (`None` for anything else).
fn number(value: Option<&OValue>) -> Option<f64> {
    match value {
        Some(OValue::Number(number)) => number.as_f64(),
        _ => None,
    }
}

/// JavaScript truthiness of a JSON value (`undefined` is `None`).
fn truthy(value: Option<&OValue>) -> bool {
    match value {
        None | Some(OValue::Null) | Some(OValue::Bool(false)) => false,
        Some(OValue::String(text)) => !text.is_empty(),
        Some(OValue::Number(number)) => number.as_f64().is_some_and(|n| n != 0.0),
        Some(_) => true,
    }
}

/// `JSON.stringify([..])` of some members (an absent member is `null`).
fn identity(item: &OValue, keys: &[&str]) -> String {
    let values: Vec<OValue> = keys
        .iter()
        .map(|key| item.get(key).cloned().unwrap_or(OValue::Null))
        .collect();
    crate::json::to_compact(&OValue::Array(values)).unwrap_or_default()
}

/// `String(item[key])` (`undefined` for an absent member).
fn string_of(item: &OValue, key: &str) -> String {
    match item.get(key) {
        None => "\u{0}undefined".to_owned(),
        Some(OValue::String(text)) => text.clone(),
        Some(OValue::Number(number)) => crate::render::js_json_number(number),
        Some(other) => crate::json::to_compact(other).unwrap_or_default(),
    }
}

/// TS `Date.parse` (the §15.17 approximation, plus a date-only form as UTC midnight).
pub(crate) fn parse_date(value: Option<&OValue>) -> Option<i64> {
    let Some(OValue::String(text)) = value else {
        return None;
    };
    parse_date_text(text)
}

/// [`parse_date`] of a string: a zoned ISO-8601 date-time, or `YYYY-MM-DD` as UTC
/// midnight (also the supplychain report's rule, D322).
pub(crate) use super::parse_date_text;

/// One page of a source (TS `Page`): `items` is `None` when the answer has no record
/// array; `next` the offset of the next page (`None` ends the source).
struct Page {
    items: Option<Vec<OValue>>,
    next: Option<f64>,
}

fn items(page: &OValue, key: &str) -> Option<Vec<OValue>> {
    page.get(key)
        .and_then(OValue::as_array)
        .map(<[OValue]>::to_vec)
}

/// TS `safeFailure`: no upstream text.
fn safe_failure(error: &ReadError) -> String {
    match error {
        ReadError::Data(text) => (*text).to_owned(),
        ReadError::Status(code @ (401 | 403)) => {
            format!("HTTP {code}: check Management API credentials and grants.")
        }
        ReadError::Status(429) => "HTTP 429: rate limited; retry the report later.".to_owned(),
        ReadError::Status(code) if (400..=599).contains(code) => format!(
            "HTTP {code}: source request failed; verify service availability and API compatibility."
        ),
        _ => "Source request or response validation failed; check connectivity and SDK compatibility."
            .to_owned(),
    }
}

const NO_ARRAY: &str = "The API returned no record array. An empty object or absent collection does not establish zero activity.";
const OVERLAP: &str = "Overlapping or repeated records detected; duplicates excluded.";
const NO_ADVANCE: &str = "Pagination did not advance; collection stopped safely.";

/// TS `collect`: pages until no next, the budget, or an error; records by identity.
async fn collect<F, Fut>(
    source: &mut Source,
    max_pages: u32,
    mut read: F,
    identity: impl Fn(&OValue) -> String,
) -> Result<Vec<OValue>, Interrupted>
where
    F: FnMut(u64) -> Fut,
    Fut: std::future::Future<Output = Result<Page, ReadError>>,
{
    let mut order: Vec<String> = Vec::new();
    let mut records: HashMap<String, OValue> = HashMap::new();
    let values = |order: &[String], records: &HashMap<String, OValue>| -> Vec<OValue> {
        order
            .iter()
            .filter_map(|key| records.get(key).cloned())
            .collect()
    };
    let mut offset: u64 = 0;
    let outcome: Result<(), ReadError> = async {
        for _ in 0..max_pages {
            let page = read(offset).await?;
            source.pages += 1;
            let Some(page_items) = page.items else {
                return Err(ReadError::Data(NO_ARRAY));
            };
            let before = records.len();
            for item in &page_items {
                let key = identity(item);
                if records.insert(key.clone(), item.clone()).is_none() {
                    order.push(key);
                }
            }
            source.records = records.len() as u64;
            if records.len() - before != page_items.len() {
                source.notes.push(OVERLAP.to_owned());
                source.status = Status::Partial;
            }
            let Some(next) = page.next else {
                if source.status != Status::Partial {
                    source.status = Status::Complete;
                }
                return Ok(());
            };
            let safe = next.fract() == 0.0 && next.abs() <= MAX_SAFE as f64;
            #[allow(clippy::cast_precision_loss)]
            let advanced = next > offset as f64;
            if !safe || !advanced || records.len() == before {
                source.notes.push(NO_ADVANCE.to_owned());
                source.status = Status::Partial;
                return Ok(());
            }
            // A safe integer above the current offset converts exactly.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let next = next as u64;
            offset = next;
        }
        source.notes.push(format!(
            "Reached the {max_pages}-page budget; this is not a complete inventory."
        ));
        source.status = Status::Partial;
        Ok(())
    }
    .await;
    match outcome {
        Ok(()) => {}
        Err(ReadError::Cancelled) => return Err(Interrupted),
        Err(error) => {
            source.status = if source.pages > 0 && !records.is_empty() {
                Status::Partial
            } else {
                Status::Unavailable
            };
            source.notes.push(safe_failure(&error));
        }
    }
    Ok(values(&order, &records))
}

/// TS `inventoryNext`: an explicit zero ends; absent metadata with a full page probes.
fn inventory_next(next: Option<f64>, size: u64, offset: u64) -> Option<f64> {
    match next {
        #[allow(clippy::cast_precision_loss)]
        None => (size == PAGE_SIZE).then(|| (offset + size) as f64),
        Some(next) => (next != 0.0).then_some(next),
    }
}

#[allow(clippy::cast_precision_loss)]
fn offset_f64(value: u64) -> f64 {
    value as f64
}

fn severity(value: Option<&OValue>) -> Severity {
    let get = |key: &str| count(value.and_then(|value| value.get(key)));
    Severity {
        critical: get("critical"),
        high: get("high"),
        medium: get("medium"),
        low: get("low"),
        total: get("total"),
    }
}

/// The findings in progress.
struct Findings(Vec<Finding>);

impl Findings {
    fn add(
        &mut self,
        priority: Priority,
        title: String,
        evidence: String,
        recommendation: &str,
        source: &str,
    ) {
        self.0.push(Finding {
            priority,
            title,
            evidence,
            recommendation: recommendation.to_owned(),
            source: source.to_owned(),
        });
    }
}

const SESSION_INVENTORY: &str = "Daily session inventory";

/// Collects the seven sources and builds the model (TS `collectRuntimeDailyReport`).
#[allow(clippy::too_many_lines)]
pub(crate) async fn collect_report(
    reader: &impl Reader,
    options: &Options<'_>,
) -> Result<Report, Interrupted> {
    let max_pages = options.max_pages;
    let end_ms = (options.now)();
    let start_ms = end_ms - DAY_MS;
    let mut activity_source = source(
        "Daily application activity",
        "dashboard.applicationsOverview",
        "Rolling 1 day",
    );
    let mut profile_source = source(
        "Security profiles",
        "profiles.list (latest=true)",
        "Current configuration",
    );
    let mut app_source = source(
        "Registered applications",
        "customerApps.list",
        "Current configuration",
    );
    let mut session_source = source(
        SESSION_INVENTORY,
        "dashboard.sessionsOverview",
        "Rolling 1 day",
    );
    let mut chart_source = source(
        "Daily session chart",
        "dashboard.sessionsChart",
        "Rolling 1 day",
    );
    let mut ranking_source = source(
        "Top application violations",
        "dashboard.topApplicationsViolations",
        "Rolling 1 day; server-ranked subset",
    );
    let mut trend_source = source(
        "Daily violation trend",
        "dashboard.applicationsViolationsTrend",
        "Rolling 1 day",
    );

    // 1. The application overview.
    let expected_app_total: std::cell::Cell<Option<u64>> = std::cell::Cell::new(None);
    let applications = collect(
        &mut activity_source,
        max_pages,
        |offset| {
            let expected = &expected_app_total;
            async move {
                let page = reader.applications(offset).await?;
                let items = items(&page, "items");
                let size = items.as_ref().map_or(0, Vec::len) as u64;
                let total_items = count(
                    page.get("pagination")
                        .and_then(|pagination| pagination.get("total_items")),
                );
                if let Some(total_items) = total_items {
                    if expected.get().is_some_and(|expected| expected != total_items) {
                        return Err(ReadError::Data(
                            "Application total changed during pagination; retry for a stable inventory.",
                        ));
                    }
                    expected.set(Some(total_items));
                    if offset + size > total_items {
                        return Err(ReadError::Data(
                            "Application records exceed the advertised total.",
                        ));
                    }
                    if total_items > offset + size && size == 0 {
                        return Err(ReadError::Data(
                            "The API returned an empty page before its advertised total.",
                        ));
                    }
                }
                let next = match total_items {
                    Some(total_items) => {
                        (offset + size < total_items).then(|| offset_f64(offset + size))
                    }
                    None => (size == PAGE_SIZE).then(|| offset_f64(offset + size)),
                };
                Ok(Page { items, next })
            }
        },
        |item| identity(item, &["id", "name"]),
    )
    .await?;

    // 2. The latest security profiles.
    let profile_rows = collect(
        &mut profile_source,
        max_pages,
        |offset| async move {
            let page = reader.profiles(offset).await?;
            let items = items(&page, "ai_profiles");
            let size = items.as_ref().map_or(0, Vec::len) as u64;
            Ok(Page {
                next: inventory_next(number(page.get("next_offset")), size, offset),
                items,
            })
        },
        |item| identity(item, &["profile_name", "revision"]),
    )
    .await?;

    // 3. The registered applications.
    let registered = collect(
        &mut app_source,
        max_pages,
        |offset| async move {
            let page = reader.customer_apps(offset).await?;
            let items = items(&page, "customer_apps");
            let size = items.as_ref().map_or(0, Vec::len) as u64;
            Ok(Page {
                next: inventory_next(number(page.get("next_offset")), size, offset),
                items,
            })
        },
        |item| string_of(item, "customer_appId"),
    )
    .await?;

    // 4. The session inventory.
    let expected_session_total: std::cell::Cell<Option<u64>> = std::cell::Cell::new(None);
    let raw_sessions = collect(
        &mut session_source,
        max_pages,
        |offset| {
            let expected = &expected_session_total;
            async move {
                let page = reader.sessions(offset).await?;
                let items = items(&page, "items");
                let size = items.as_ref().map_or(0, Vec::len) as u64;
                let pagination = page.get("pagination");
                let total_items = count(pagination.and_then(|p| p.get("total_items")));
                let field = |key: &str| number(pagination.and_then(|p| p.get(key)));
                let Some(total_items) = total_items.filter(|_| {
                    field("skip") == Some(offset_f64(offset))
                        && field("limit") == Some(offset_f64(SESSION_PAGE))
                }) else {
                    return Err(ReadError::Data(
                        "Missing or mismatched session pagination metadata.",
                    ));
                };
                if expected
                    .get()
                    .is_some_and(|expected| expected != total_items)
                {
                    return Err(ReadError::Data(
                        "Session total changed during pagination; retry for a stable inventory.",
                    ));
                }
                expected.set(Some(total_items));
                if offset + size > total_items
                    || size > SESSION_PAGE
                    || (size == 0 && offset < total_items)
                {
                    return Err(ReadError::Data(
                        "Session page does not agree with its advertised total.",
                    ));
                }
                Ok(Page {
                    items,
                    next: (offset + size < total_items).then(|| offset_f64(offset + size)),
                })
            }
        },
        |item| identity(item, &["application_id", "application_name", "session_id"]),
    )
    .await?;

    // 5–7. The one-page daily telemetry.
    let chart = collect(
        &mut chart_source,
        1,
        |_| async move {
            let page = reader.sessions_chart().await?;
            Ok(Page {
                items: items(&page, "buckets"),
                next: None,
            })
        },
        |item| string_of(item, "bucket_number"),
    )
    .await?;
    let rankings = collect(
        &mut ranking_source,
        1,
        |_| async move {
            let page = reader.top_applications().await?;
            Ok(Page {
                items: items(&page, "applications"),
                next: None,
            })
        },
        |item| identity(item, &["id", "name"]),
    )
    .await?;
    let trend = collect(
        &mut trend_source,
        1,
        |_| async move {
            let page = reader.violations_trend().await?;
            Ok(Page {
                items: items(&page, "violations"),
                next: None,
            })
        },
        |item| string_of(item, "bucket_number"),
    )
    .await?;

    let sources = vec![
        activity_source,
        profile_source,
        app_source,
        session_source,
        chart_source,
        ranking_source,
        trend_source,
    ];
    let status_of = |index: usize| sources[index].status;
    let name_of = |index: usize| sources[index].name;
    let mut findings = Findings(Vec::new());
    for s in &sources {
        if s.status != Status::Complete {
            findings.add(
                Priority::Review,
                format!("{}: {}", s.name, s.status.name()),
                s.notes.join(" "),
                "Restore access or retry collection; do not treat missing data as zero. Increase --max-pages if the page budget was reached.",
                s.name,
            );
        }
    }

    // Activity.
    let app_ids: std::collections::HashSet<&str> = registered
        .iter()
        .filter_map(|app| app.get("customer_appId").and_then(OValue::as_str))
        .collect();
    let mut report_apps: Vec<Application> = applications
        .iter()
        .map(|app| {
            let id = app.get("id");
            let id_text = id.and_then(OValue::as_str);
            Application {
                name: label(app.get("name")),
                sessions: count(app.get("sessions_total")),
                violating_sessions: count(app.get("sessions_violated")),
                registered: if truthy(id) && id_text.is_some_and(|id| app_ids.contains(id)) {
                    Some(true)
                } else if status_of(2) == Status::Complete && truthy(id) {
                    Some(false)
                } else {
                    None
                },
            }
        })
        .collect();
    let rank = |value: Option<u64>| value.map_or(-1, i128::from);
    report_apps.sort_by(|a, b| {
        rank(b.violating_sessions)
            .cmp(&rank(a.violating_sessions))
            .then_with(|| super::locale_order(&a.name, &b.name))
    });
    let activity_unavailable = status_of(0) == Status::Unavailable;
    let sessions_total = if activity_unavailable {
        None
    } else {
        total(report_apps.iter().map(|app| app.sessions))
    };
    let violating_total = if activity_unavailable {
        None
    } else {
        total(report_apps.iter().map(|app| app.violating_sessions))
    };
    let inconsistent = report_apps
        .iter()
        .any(|app| matches!((app.sessions, app.violating_sessions), (Some(s), Some(v)) if v > s));
    if inconsistent
        || report_apps
            .iter()
            .any(|app| app.sessions.is_none() || app.violating_sessions.is_none())
    {
        findings.add(
            Priority::Review,
            "Activity counters need verification".to_owned(),
            "Some application counters are absent, invalid, or report more violating sessions than total sessions.".to_owned(),
            "Compare these buckets with the SCM dashboard. A violation rate is withheld when counters are inconsistent.",
            name_of(0),
        );
    }
    if violating_total.unwrap_or(0) > 0 {
        let buckets = report_apps
            .iter()
            .filter(|app| app.violating_sessions.unwrap_or(0) > 0)
            .count();
        findings.add(
            Priority::Attention,
            "Violating sessions observed".to_owned(),
            format!(
                "{} violating sessions across {buckets} application buckets in the collected daily data.",
                violating_total.unwrap_or(0)
            ),
            "Review the highest-volume applications in SCM, confirm intended enforcement, and investigate unexpected activity. Violations are not proof of a successful attack.",
            name_of(0),
        );
    }
    if sessions_total == Some(0) && status_of(0) == Status::Complete {
        findings.add(
            Priority::Review,
            "No sessions reported in the daily window".to_owned(),
            "The complete application overview reports zero sessions.".to_owned(),
            "If traffic was expected, check the integration, application metadata, and telemetry ingestion. No activity is not proof of health.",
            name_of(0),
        );
    }

    // Profiles: the latest revision per name.
    let mut latest_order: Vec<String> = Vec::new();
    let mut latest: HashMap<String, &OValue> = HashMap::new();
    for profile in &profile_rows {
        let key = string_of(profile, "profile_name");
        let revision = |row: &OValue| number(row.get("revision")).unwrap_or(-1.0);
        match latest.get(&key) {
            Some(old) if revision(profile) <= revision(old) => {}
            Some(_) => {
                latest.insert(key, profile);
            }
            None => {
                latest_order.push(key.clone());
                latest.insert(key, profile);
            }
        }
    }
    let mut profiles: Vec<Profile> = latest_order
        .iter()
        .filter_map(|key| latest.get(key).copied())
        .map(|profile| {
            let configs: Vec<&OValue> = profile
                .get("policy")
                .and_then(|policy| policy.get("ai-security-profiles"))
                .and_then(OValue::as_array)
                .unwrap_or_default()
                .iter()
                .filter_map(|entry| {
                    let config = entry.get("model-configuration");
                    truthy(config).then_some(config).flatten()
                })
                .collect();
            let mut timeout_actions: Vec<String> = Vec::new();
            for config in &configs {
                let action = label(
                    config
                        .get("latency")
                        .and_then(|latency| latency.get("inline-timeout-action")),
                );
                if !timeout_actions.contains(&action) {
                    timeout_actions.push(action);
                }
            }
            let name = label(profile.get("profile_name"));
            let active = match profile.get("active") {
                Some(OValue::Bool(active)) => Some(*active),
                _ => None,
            };
            if active == Some(false) {
                findings.add(
                    Priority::Review,
                    format!("Inactive profile: {name}"),
                    "The latest returned revision is explicitly inactive.".to_owned(),
                    "Confirm this is intentional before assigning the profile to an integration.",
                    name_of(1),
                );
            }
            if active == Some(true)
                && timeout_actions
                    .iter()
                    .any(|action| action.to_lowercase() == "allow")
            {
                findings.add(
                    Priority::Review,
                    format!("Timeout allows traffic: {name}"),
                    "An active profile explicitly sets inline-timeout-action to allow.".to_owned(),
                    "Review the availability-versus-enforcement tradeoff with the application owner; this is not evidence of a timeout or bypass.",
                    name_of(1),
                );
            }
            let masking: Vec<Option<&OValue>> = configs
                .iter()
                .map(|config| config.get("mask-data-in-storage"))
                .collect();
            let off = masking.contains(&Some(&OValue::Bool(false)));
            let storage_masking = if off {
                "Off in at least one configuration"
            } else if !masking.is_empty()
                && masking.iter().all(|value| *value == Some(&OValue::Bool(true)))
            {
                "On in all returned configurations"
            } else {
                "Unknown"
            };
            if active == Some(true) && off {
                findings.add(
                    Priority::Review,
                    format!("Storage masking disabled: {name}"),
                    "An active profile explicitly disables mask-data-in-storage in a model configuration.".to_owned(),
                    "Confirm retention and privacy requirements. This flag alone does not establish that sensitive data was stored.",
                    name_of(1),
                );
            }
            let modified = truthy(profile.get("last_modified_ts"))
                .then(|| parse_date(profile.get("last_modified_ts")))
                .flatten()
                .map(iso);
            Profile {
                name,
                revision: count(profile.get("revision")),
                active,
                modified,
                timeout_actions,
                storage_masking,
                protection_configurations: protection_configurations(profile.get("policy")),
            }
        })
        .collect();
    profiles.sort_by(|a, b| super::locale_order(&a.name, &b.name));
    if profiles.is_empty() && status_of(1) == Status::Complete {
        findings.add(
            Priority::Attention,
            "No security profiles returned".to_owned(),
            "The current profile inventory is empty.".to_owned(),
            "Verify tenant scope and configure a security profile before routing application scans.",
            name_of(1),
        );
    }

    // Sessions: the anchored half-open window.
    let mut missing_timestamps = 0u64;
    let mut outside_window = 0u64;
    let session_rows: Vec<&OValue> = raw_sessions
        .iter()
        .filter(|row| match parse_date(row.get("last_session_activity")) {
            None => {
                missing_timestamps += 1;
                false
            }
            Some(at) if at < start_ms || at >= end_ms => {
                outside_window += 1;
                false
            }
            Some(_) => true,
        })
        .collect();
    if missing_timestamps > 0 || outside_window > 0 {
        findings.add(
            Priority::Review,
            "Some session records excluded from daily analysis".to_owned(),
            format!(
                "{missing_timestamps} records lack a valid last-activity timestamp; {outside_window} fall outside the anchored UTC window."
            ),
            "Verify ingestion timestamps and collection-window drift before using session counts as daily totals.",
            name_of(3),
        );
    }
    let status_text = |row: &OValue| {
        row.get("violation_status")
            .and_then(OValue::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let failed_sessions = session_rows
        .iter()
        .filter(|row| {
            let status = crate::tenant::config::js_trim(&status_text(row)).to_lowercase();
            matches!(status.as_str(), "failed" | "error" | "timeout")
        })
        .count();
    if failed_sessions > 0 {
        findings.add(
            Priority::Attention,
            "Session processing failures observed".to_owned(),
            format!(
                "{failed_sessions} timestamp-eligible collected sessions explicitly report failed, error, or timeout status."
            ),
            "Review these sessions in SCM and check integration error handling. This is not a tenant-wide failure rate.",
            name_of(3),
        );
    }
    let chart_unavailable = status_of(4) == Status::Unavailable;
    let chart_sessions = if chart_unavailable {
        None
    } else {
        total(chart.iter().map(|bucket| count(bucket.get("total"))))
    };
    let chart_violating = if chart_unavailable {
        None
    } else {
        total(chart.iter().map(|bucket| count(bucket.get("violating"))))
    };
    if let Some(chart_total) = chart_sessions
        && status_of(3) == Status::Complete
        && chart_total != raw_sessions.len() as u64
    {
        findings.add(
            Priority::Review,
            "Session sources disagree".to_owned(),
            format!(
                "The chart reports {chart_total} sessions; the independently paginated inventory returned {}.",
                raw_sessions.len()
            ),
            "Preserve both measurements and verify in SCM. Separate rolling queries are not a consistent snapshot; the discrepancy has no verified cause.",
            name_of(4),
        );
    }
    let urgent = total(trend.iter().map(|bucket| {
        let breakdown = bucket.get("violation_breakdown");
        total([
            count(breakdown.and_then(|b| b.get("critical"))),
            count(breakdown.and_then(|b| b.get("high"))),
        ])
    }));
    if urgent.unwrap_or(0) > 0 {
        findings.add(
            Priority::Attention,
            "High or critical detector violations observed".to_owned(),
            format!(
                "{} high/critical detector violations in the returned daily trend. These are not distinct sessions.",
                urgent.unwrap_or(0)
            ),
            "Investigate the affected applications and confirm enforcement decisions before assigning incident severity.",
            name_of(6),
        );
    }
    let mut findings = findings.0;
    findings.sort_by_key(|finding| match finding.priority {
        Priority::Attention => 0,
        Priority::Review => 1,
    });

    let health = if sources
        .iter()
        .all(|source| source.status == Status::Unavailable)
    {
        Health::Unknown
    } else if findings
        .iter()
        .any(|finding| finding.priority == Priority::Attention)
    {
        Health::Attention
    } else if findings
        .iter()
        .any(|finding| finding.priority == Priority::Review)
    {
        Health::Review
    } else {
        Health::NoFindings
    };

    let mut registered_apps: Vec<RegisteredApp> = registered
        .iter()
        .map(|app| RegisteredApp {
            name: label(app.get("app_name")),
            environment: label(app.get("environment")),
            cloud: label(app.get("cloud_provider")),
            model: label(app.get("model_name")),
            key_associations: app
                .get("api_keys_dp_info")
                .and_then(OValue::as_array)
                .map(|keys| keys.len() as u64),
        })
        .collect();
    registered_apps.sort_by(|a, b| super::locale_order(&a.name, &b.name));

    let session_unavailable = status_of(3) == Status::Unavailable;
    let mut statuses: Vec<(String, u64)> = Vec::new();
    for row in &session_rows {
        let name = label(row.get("violation_status")).to_lowercase();
        match statuses.iter_mut().find(|(known, _)| *known == name) {
            Some((_, count)) => *count += 1,
            None => statuses.push((name, 1)),
        }
    }
    statuses.sort_by_key(|status| std::cmp::Reverse(status.1));
    let sessions = Sessions {
        entries: (!session_unavailable).then_some(session_rows.len() as u64),
        statuses,
        violating_sessions: (!session_unavailable).then(|| {
            session_rows
                .iter()
                .filter(|row| status_text(row) == "violated")
                .count() as u64
        }),
        missing_timestamps,
        outside_window,
    };
    #[allow(clippy::cast_precision_loss)]
    let violation_rate = match (inconsistent, sessions_total, violating_total) {
        (false, Some(sessions), Some(violating)) if sessions > 0 => {
            Some(violating as f64 / sessions as f64 * 100.0)
        }
        _ => None,
    };
    let generated = (options.now)();
    Ok(Report {
        title: label_text(options.title),
        generated_at: iso(generated),
        collection_started_at: iso(end_ms),
        window_start: iso(start_ms),
        window_end: iso(end_ms),
        health,
        findings,
        sessions_total,
        violating_total,
        violation_rate,
        applications: report_apps,
        profiles,
        registered_apps,
        sessions,
        chart_sessions,
        chart_violating,
        chart_buckets: chart
            .iter()
            .map(|bucket| ChartBucket {
                time: label(bucket.get("time")),
                sessions: count(bucket.get("total")),
                violating_sessions: count(bucket.get("violating")),
                violations: severity(bucket.get("violation_breakdown")),
            })
            .collect(),
        top_applications: rankings
            .iter()
            .map(|app| TopApplication {
                name: label(app.get("name")),
                violations: count(app.get("total_violations")),
                detectors: app
                    .get("policy_violations")
                    .and_then(OValue::as_array)
                    .unwrap_or_default()
                    .iter()
                    .map(|detector| {
                        (
                            label(detector.get("detection_type")),
                            count(detector.get("total")),
                        )
                    })
                    .collect(),
            })
            .collect(),
        violation_trend: trend
            .iter()
            .map(|bucket| TrendBucket {
                time: label(bucket.get("date")),
                violations: severity(bucket.get("violation_breakdown")),
            })
            .collect(),
        limitations: LIMITATIONS.iter().map(|text| (*text).to_owned()).collect(),
        sources,
    })
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
