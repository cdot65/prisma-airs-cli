//! The gateway environment report's collector and model (TS `src/reports/aigateway.ts`,
//! DESIGN §19.15).
//!
//! The 25 read-only sources are read one after the other in TypeScript's order, each
//! under its own budget, and only aggregates are kept: counts, numeric server metrics,
//! HTTP status codes and bucket times. No workspace name, slug or id, key, user, model,
//! URL, metadata, filter-boundary value, catalog entry or server text reaches the model.
//! A source that fails is `unavailable` (no records) or `partial` (some records); missing
//! data is never shown as zero. The rendering is the shared environment template
//! ([`super::html::render`] with [`super::html::GATEWAY`]).

use super::{
    Cell, Finding, Health, Interrupted, Priority, ReadError, Report, Source, Status, Table, iso,
};
use crate::json::OValue;

/// The page size of the transaction log reads (TS `pageSize: 50`).
pub const LOG_PAGE_SIZE: u32 = 50;

/// The scope statements of the gateway report (TS `limitations`, `aigateway.ts:92-100`).
pub const LIMITATIONS: [&str; 7] = [
    "Read-only operational evidence, not a security certification or availability SLA. No inference or configuration changes are performed.",
    "All telemetry uses the displayed fixed window. Configuration and organisation/catalog reads describe current state, not daily changes.",
    "Requests, errors, feedback and log totals are independent server aggregates, not an atomic snapshot. An AIRS block is not necessarily an outage.",
    "Cost values are cents; latency values are milliseconds. Feedback scores may be negative. Null means unknown, not zero.",
    "Only aggregate numbers and HTTP status codes are retained. Keys, user identities, workspace names/IDs, metadata, URLs, prompts, raw logs and configuration bodies are excluded.",
    "Catalog entries describe available capabilities, not enabled protection. Organisation limit sentinel values are not interpreted as quota exhaustion.",
    "Incomplete sources and page budgets are explicit. Current inventory lists have no verified continuation contract; total/has_more mismatches are partial.",
];

/// The note of an HTTP failure (`statusCode` an integer 400–599) and of every other
/// failure (TS `source`).
const GENERIC_NOTE: &str =
    "Request, scope or response validation failed; upstream details omitted.";
const QUOTA_NOTE: &str = "Quota exceeded or quota state unknown; aggregates may be incomplete.";
const LIST_NOTE: &str = "List is truncated or total differs; continuation is not documented.";
const BUDGET_NOTE: &str =
    "Transaction page budget reached. Increase --max-pages to collect the remaining rows.";

const TELEMETRY_WINDOW: &str = "Displayed telemetry window";
const WORKSPACE_CONFIGURATION: &str = "Current selected workspace configuration";

/// One of the 17 charts (TS `CHARTS`, in its order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chart {
    Requests,
    Errors,
    Users,
    Cost,
    Latency,
    Tokens,
    CacheSummary,
    CacheHitTrend,
    UserTrends,
    ErrorTrends,
    ErrorCategoryTrends,
    GroupedErrors,
    RescuedRetries,
    FeedbackTrend,
    FeedbackWeighted,
    FeedbackScoreDistribution,
    FeedbackModels,
}

impl Chart {
    /// Every chart, in TS order.
    pub const ALL: [Chart; 17] = [
        Chart::Requests,
        Chart::Errors,
        Chart::Users,
        Chart::Cost,
        Chart::Latency,
        Chart::Tokens,
        Chart::CacheSummary,
        Chart::CacheHitTrend,
        Chart::UserTrends,
        Chart::ErrorTrends,
        Chart::ErrorCategoryTrends,
        Chart::GroupedErrors,
        Chart::RescuedRetries,
        Chart::FeedbackTrend,
        Chart::FeedbackWeighted,
        Chart::FeedbackScoreDistribution,
        Chart::FeedbackModels,
    ];

    /// The TS method name (the `Source` column of the metric rows).
    pub fn method(self) -> &'static str {
        self.spec().0
    }

    /// The source name `telemetry.<method>`.
    pub fn source(self) -> &'static str {
        self.spec().1
    }

    /// The fields kept (TS `CHARTS[m]`).
    fn fields(self) -> &'static [&'static str] {
        self.spec().2
    }

    fn spec(self) -> (&'static str, &'static str, &'static [&'static str]) {
        match self {
            Chart::Requests => ("requests", "telemetry.requests", &["total"]),
            Chart::Errors => ("errors", "telemetry.errors", &["total"]),
            Chart::Users => ("users", "telemetry.users", &["total"]),
            Chart::Cost => ("cost", "telemetry.cost", &["total", "avg"]),
            Chart::Latency => (
                "latency",
                "telemetry.latency",
                &["total", "p50", "p90", "p99"],
            ),
            Chart::Tokens => (
                "tokens",
                "telemetry.tokens",
                &["total", "total_request_units", "total_response_units"],
            ),
            Chart::CacheSummary => (
                "cacheSummary",
                "telemetry.cacheSummary",
                &[
                    "summary.cacheHits",
                    "summary.avgCacheLatency",
                    "summary.totalRequests",
                    "summary.cacheSpeedup",
                ],
            ),
            Chart::CacheHitTrend => (
                "cacheHitTrend",
                "telemetry.cacheHitTrend",
                &["summary.totalCacheHits", "summary.hitRate"],
            ),
            Chart::UserTrends => (
                "userTrends",
                "telemetry.userTrends",
                &["summary.total", "summary.unique", "summary.avg"],
            ),
            Chart::ErrorTrends => (
                "errorTrends",
                "telemetry.errorTrends",
                &["summary.errorPercent"],
            ),
            Chart::ErrorCategoryTrends => (
                "errorCategoryTrends",
                "telemetry.errorCategoryTrends",
                &["summary.totalErrors", "summary.totalUniqueErrorCodes"],
            ),
            Chart::GroupedErrors => ("groupedErrors", "telemetry.groupedErrors", &["total"]),
            Chart::RescuedRetries => (
                "rescuedRetries",
                "telemetry.rescuedRetries",
                &["total", "retryTotal", "fallbackTotal"],
            ),
            Chart::FeedbackTrend => ("feedbackTrend", "telemetry.feedbackTrend", &["total"]),
            Chart::FeedbackWeighted => {
                ("feedbackWeighted", "telemetry.feedbackWeighted", &["total"])
            }
            Chart::FeedbackScoreDistribution => (
                "feedbackScoreDistribution",
                "telemetry.feedbackScoreDistribution",
                &["total"],
            ),
            Chart::FeedbackModels => ("feedbackModels", "telemetry.feedbackModels", &[]),
        }
    }
}

/// The 25 reads, as ordered JSON documents. The reader holds the fixed window and the
/// tenant's TSG; the collector supplies the resolved workspace (its slug for the
/// telemetry reads, its id for the workspace inventories).
pub(crate) trait Reader {
    /// `GET <admin>/workspaces` (the tenant's active workspaces).
    async fn workspaces(&self) -> Result<OValue, ReadError>;
    /// `GET <data>/logs/charts/<chart>?<window>`.
    async fn chart(&self, chart: Chart, slug: &str) -> Result<OValue, ReadError>;
    /// `GET <data>/analytics/filter-boundaries?<window>`.
    async fn filter_boundaries(&self, slug: &str) -> Result<OValue, ReadError>;
    /// `GET <data>/logs?<window>&pageSize=50&currentPage=<page>`.
    async fn logs(&self, slug: &str, page: u32) -> Result<OValue, ReadError>;
    /// `GET <data>/configs?workspace_id=<id>`.
    async fn configs(&self, workspace_id: &str) -> Result<OValue, ReadError>;
    /// `GET <data>/api-keys/service?workspace_id=<id>`.
    async fn service_keys(&self, workspace_id: &str) -> Result<OValue, ReadError>;
    /// `GET <data>/api-keys/user?workspace_id=<id>`.
    async fn user_keys(&self, workspace_id: &str) -> Result<OValue, ReadError>;
    /// `GET <admin>/organisations/<mgmtTsgId>/info`.
    async fn organisation_info(&self) -> Result<OValue, ReadError>;
    /// `GET <admin>/utils/static-resources/schema?resource=guardrails`.
    async fn catalog(&self) -> Result<OValue, ReadError>;
}

/// The report options and clock.
pub struct Options<'a> {
    pub title: &'a str,
    pub max_pages: u32,
    /// The `--workspace` reference, as given (not trimmed, TS).
    pub workspace: &'a str,
    /// The fixed window, epoch milliseconds (`start < end`).
    pub start_ms: i64,
    pub end_ms: i64,
    /// Milliseconds since the Unix epoch.
    pub now: &'a dyn Fn() -> i64,
}

/// A failure inside a source: a read's error, or a local check (TS `throw new Error`).
type Step<T> = Result<T, ReadError>;

/// Any local check failure (TS throws an `Error` without `statusCode`).
const INVALID: ReadError = ReadError::Failed;

/// The note of a failed source (TS `source`).
fn note(error: &ReadError) -> String {
    match error {
        ReadError::Status(code) if (400..=599).contains(code) => {
            format!("HTTP {code}: request failed; check access and service availability.")
        }
        _ => GENERIC_NOTE.to_owned(),
    }
}

/// TS `record`: a plain object.
fn record(value: Option<&OValue>) -> Step<&OValue> {
    match value {
        Some(object @ OValue::Object(_)) => Ok(object),
        _ => Err(INVALID),
    }
}

/// TS `numeric(v)`: `null` is kept; anything but a finite number fails (an absent value
/// too).
fn numeric(value: Option<&OValue>) -> Step<Option<f64>> {
    match value {
        Some(OValue::Null) => Ok(None),
        Some(OValue::Number(number)) => number
            .as_f64()
            .filter(|value| value.is_finite())
            .map(Some)
            .ok_or(INVALID),
        _ => Err(INVALID),
    }
}

/// `Number.isSafeInteger`.
fn safe_integer(value: f64) -> bool {
    value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_991.0
}

/// TS `count`: a safe non-negative integer (`null` fails).
fn count(value: Option<&OValue>) -> Step<u64> {
    match numeric(value)? {
        Some(value) if safe_integer(value) && value >= 0.0 => {
            // A safe non-negative integral value converts exactly.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let integer = value as u64;
            Ok(integer)
        }
        _ => Err(INVALID),
    }
}

/// TS `Date.parse` (the §15.17 approximation plus the date-only form, §19.15).
fn date(value: Option<&OValue>) -> Option<i64> {
    super::parse_date_text(value?.as_str()?)
}

/// The collection in progress.
struct Collector {
    report: Report,
    metrics: Vec<Vec<Cell>>,
    statuses: Vec<Vec<Cell>>,
    trends: Vec<Vec<Cell>>,
    inventory: Vec<Vec<Cell>>,
    coverage: Vec<Vec<Cell>>,
}

impl Collector {
    fn finding(&mut self, priority: Priority, title: &str, evidence: &str, recommendation: &str) {
        self.finding_owned(
            priority,
            title.to_owned(),
            evidence.to_owned(),
            recommendation,
            "telemetry.errorCategoryTrends",
        );
    }

    fn finding_owned(
        &mut self,
        priority: Priority,
        title: String,
        evidence: String,
        recommendation: &str,
        source: &str,
    ) {
        self.report.findings.push(Finding {
            priority,
            title,
            evidence,
            recommendation: recommendation.to_owned(),
            source: source.to_owned(),
        });
    }

    /// TS `source`: a failure makes the source `partial` (with records) or
    /// `unavailable` and adds its note; a source that already became `partial` keeps it.
    fn finish(&mut self, mut source: Source, result: Step<()>) -> Result<(), Interrupted> {
        if let Err(error) = result {
            if error == ReadError::Cancelled {
                return Err(Interrupted);
            }
            source.status = if source.records > 0 {
                Status::Partial
            } else {
                Status::Unavailable
            };
            source.notes.push(note(&error));
        }
        self.report.sources.push(source);
        Ok(())
    }
}

/// A source before it is read (TS: `complete`, nothing counted).
fn open(name: &'static str, window: &'static str) -> Source {
    Source {
        name,
        method: name,
        window: window.to_owned(),
        status: Status::Complete,
        records: 0,
        pages: 0,
        notes: Vec::new(),
    }
}

/// TS `envelope`: `success: true` and an object `data`; a quota flag that is not
/// `false` makes the source `partial` with the quota note (once per call).
fn envelope<'a>(value: &'a OValue, source: &mut Source) -> Step<&'a OValue> {
    let value = record(Some(value))?;
    if !matches!(value.get("success"), Some(OValue::Bool(true))) {
        return Err(INVALID);
    }
    let data = record(value.get("data"))?;
    if !matches!(data.get("isQuotaExceeded"), Some(OValue::Bool(false))) {
        source.status = Status::Partial;
        source.notes.push(QUOTA_NOTE.to_owned());
    }
    Ok(data)
}

/// TS `list`: `data` an array of objects, `total` a safe count; `has_more: true` or a
/// `total` other than the rows makes the source `partial` with the list note.
fn list<'a>(value: &'a OValue, source: &mut Source) -> Step<&'a [OValue]> {
    let value = record(Some(value))?;
    if matches!(value.get("success"), Some(OValue::Bool(false))) {
        return Err(INVALID);
    }
    let Some(rows) = value.get("data").and_then(OValue::as_array) else {
        return Err(INVALID);
    };
    for row in rows {
        record(Some(row))?;
    }
    let total = count(value.get("total"))?;
    if matches!(value.get("has_more"), Some(OValue::Bool(true))) || total != rows.len() as u64 {
        source.status = Status::Partial;
        source.notes.push(LIST_NOTE.to_owned());
    }
    source.records = rows.len() as u64;
    source.pages = 1;
    Ok(rows)
}

/// The value at a dotted `field` of `data` (TS: `record()` at every step).
fn field<'a>(data: &'a OValue, field: &str) -> Step<Option<&'a OValue>> {
    let mut node = data;
    let mut parts = field.split('.').peekable();
    while let Some(part) = parts.next() {
        let object = record(Some(node))?;
        match object.get(part) {
            Some(child) if parts.peek().is_some() => node = child,
            found if parts.peek().is_none() => return Ok(found),
            _ => return Err(INVALID),
        }
    }
    Ok(None)
}

/// The metric rows of one chart (TS: the sign and integer rules per field).
fn metric_rows(chart: Chart, data: &OValue) -> Step<Vec<Vec<Cell>>> {
    let method = chart.method();
    let signed = method.starts_with("feedback");
    let mut rows = Vec::new();
    for name in chart.fields() {
        let value = numeric(field(data, name)?)?;
        let fractional = signed
            || chart == Chart::Cost
            || chart == Chart::Latency
            || ["avg", "Latency", "Speedup", "Rate", "Percent"]
                .iter()
                .any(|suffix| name.ends_with(suffix));
        if let Some(value) = value
            && ((!signed && value < 0.0) || (!fractional && !safe_integer(value)))
        {
            return Err(INVALID);
        }
        rows.push(vec![
            Cell::from(method),
            Cell::from(*name),
            value.map_or(Cell::Null, Cell::Number),
        ]);
    }
    Ok(rows)
}

/// Collects the 25 sources (TS `collectGatewayEnvironmentReport`).
pub(crate) async fn collect(
    reader: &impl Reader,
    options: &Options<'_>,
) -> Result<Report, Interrupted> {
    let mut c = Collector {
        report: Report {
            title: options.title.to_owned(),
            collection_started_at: iso((options.now)()),
            generated_at: String::new(),
            window_start: iso(options.start_ms),
            window_end: iso(options.end_ms),
            health: Health::Unknown,
            sources: Vec::new(),
            findings: Vec::new(),
            tables: Vec::new(),
        },
        metrics: Vec::new(),
        statuses: Vec::new(),
        trends: Vec::new(),
        inventory: Vec::new(),
        coverage: Vec::new(),
    };

    // 1. The tenant's active workspaces (admin) and the workspace reference.
    let mut s = open(
        "workspaces.list",
        "Current active tenant workspaces (admin)",
    );
    let mut workspace: Option<(String, String)> = None;
    let result = async {
        let answer = reader.workspaces().await?;
        let rows = list(&answer, &mut s)?;
        c.inventory.push(vec![
            Cell::from("workspaces.list"),
            Cell::from("Active workspaces returned"),
            Cell::Count(rows.len() as u64),
        ]);
        let text =
            |row: &OValue, key: &str| row.get(key).and_then(OValue::as_str).map(str::to_owned);
        let by = |keys: &[&str]| -> Vec<&OValue> {
            rows.iter()
                .filter(|row| {
                    keys.iter()
                        .any(|key| text(row, key).as_deref() == Some(options.workspace))
                })
                .collect()
        };
        let exact = by(&["id", "slug"]);
        // D254: on an incomplete list (the source is partial) only an exact id or slug
        // resolves; a name may also sit beyond the answer (TS resolves the visible one).
        let matches = if !exact.is_empty() {
            exact
        } else if s.status == Status::Partial {
            Vec::new()
        } else {
            by(&["name"])
        };
        match matches.as_slice() {
            [only] => match (text(only, "id"), text(only, "slug")) {
                (Some(id), Some(slug)) => {
                    workspace = Some((id, slug));
                    Ok(())
                }
                _ => Err(INVALID),
            },
            _ => Err(INVALID),
        }
    }
    .await;
    c.finish(s, result)?;
    let slug = workspace.as_ref().map(|(_, slug)| slug.as_str());
    let workspace_id = workspace.as_ref().map(|(id, _)| id.as_str());

    // 2–18. The charts.
    for chart in Chart::ALL {
        let mut s = open(chart.source(), TELEMETRY_WINDOW);
        let result = async {
            let slug = slug.ok_or(INVALID)?;
            let answer = reader.chart(chart, slug).await?;
            let data = envelope(&answer, &mut s)?;
            let mut rows = metric_rows(chart, data)?;
            let buckets = match data.get("records") {
                None | Some(OValue::Null) => data.get("trend"),
                found => found,
            };
            s.records = match buckets {
                None => 1,
                Some(OValue::Array(items)) => items.len() as u64,
                Some(_) => return Err(INVALID),
            };
            s.pages = 1;
            if chart == Chart::FeedbackModels {
                rows.push(vec![
                    Cell::from("feedbackModels"),
                    Cell::from("Models with feedback"),
                    Cell::Count(s.records),
                ]);
            }
            if chart == Chart::ErrorCategoryTrends {
                let Some(trend) = data.get("trend").and_then(OValue::as_array) else {
                    return Err(INVALID);
                };
                let mut entries = Vec::new();
                for entry in trend {
                    let entry = record(Some(entry))?;
                    entries.push((
                        count(entry.get("response_status_code"))?,
                        count(entry.get("count"))?,
                    ));
                }
                c.statuses.extend(
                    entries
                        .iter()
                        .map(|(code, n)| vec![Cell::Count(*code), Cell::Count(*n)]),
                );
                if entries
                    .iter()
                    .any(|(code, n)| (200..300).contains(code) && *n > 0)
                {
                    c.finding(
                        Priority::Review,
                        "Success-status responses appear in error analytics",
                        "The server classifies some HTTP 2xx responses as errors. Error analytics are not identical to non-2xx transaction counts.",
                        "Inspect the underlying application/provider error classifications before calculating an HTTP failure rate.",
                    );
                }
                if entries
                    .iter()
                    .any(|(code, n)| *n > 0 && (*code == 429 || *code >= 500))
                {
                    c.finding(
                        Priority::Attention,
                        "Rate limiting or server errors observed",
                        "Error categories include HTTP 429 or 5xx responses in the selected window.",
                        "Review provider availability, quotas and retry policies; correlate with transaction logs.",
                    );
                } else if entries.iter().any(|(_, n)| *n > 0) {
                    c.finding(
                        Priority::Review,
                        "Gateway errors or policy blocks observed",
                        "Nonzero error-category counts were returned.",
                        "Review status categories; distinguish expected AIRS blocks from integration failures.",
                    );
                }
            }
            if chart == Chart::GroupedErrors {
                let Some(trend) = data.get("trend").and_then(OValue::as_array) else {
                    return Err(INVALID);
                };
                let mut entries = Vec::new();
                for bucket in trend {
                    let bucket = record(Some(bucket))?;
                    let (Some(time), Some(OValue::Array(items))) =
                        (date(bucket.get("x")), bucket.get("y"))
                    else {
                        return Err(INVALID);
                    };
                    for item in items {
                        let item = record(Some(item))?;
                        entries.push(vec![
                            Cell::Text(iso(time)),
                            Cell::Count(count(item.get("response_status_code"))?),
                            Cell::Count(count(item.get("count"))?),
                        ]);
                    }
                }
                c.trends.extend(entries);
            }
            c.metrics.extend(rows);
            Ok(())
        }
        .await;
        c.finish(s, result)?;
    }

    // 19. The filter boundaries: their counts, never their values.
    let mut s = open("telemetry.filterBoundaries", TELEMETRY_WINDOW);
    let result = async {
        let slug = slug.ok_or(INVALID)?;
        let answer = reader.filter_boundaries(slug).await?;
        let data = envelope(&answer, &mut s)?;
        for key in ["unique_ai_models", "unique_status_codes"] {
            let Some(values) = data.get(key).and_then(OValue::as_array) else {
                return Err(INVALID);
            };
            c.metrics.push(vec![
                Cell::from("filterBoundaries"),
                Cell::Text(format!("{key} count")),
                Cell::Count(values.len() as u64),
            ]);
        }
        s.records = 1;
        s.pages = 1;
        Ok(())
    }
    .await;
    c.finish(s, result)?;

    // 20. The transaction logs, page by page.
    let mut s = open("telemetry.logs", TELEMETRY_WINDOW);
    let mut ids = std::collections::BTreeSet::new();
    let mut codes: std::collections::BTreeMap<u64, u64> = std::collections::BTreeMap::new();
    let mut expected: Option<u64> = None;
    let result = async {
        for page in 0..options.max_pages {
            let slug = slug.ok_or(INVALID)?;
            let answer = reader.logs(slug, page).await?;
            let data = envelope(&answer, &mut s)?;
            s.pages += 1;
            let total = count(data.get("total"))?;
            let wanted = *expected.get_or_insert(total);
            let Some(records) = data.get("records").and_then(OValue::as_array) else {
                return Err(INVALID);
            };
            if total != wanted || records.len() > LOG_PAGE_SIZE as usize {
                return Err(INVALID);
            }
            for row in records {
                let row = record(Some(row))?;
                let id = match row.get("id") {
                    Some(OValue::String(id)) if !id.is_empty() && !ids.contains(id) => id.clone(),
                    _ => return Err(INVALID),
                };
                if row.get("workspace_slug").and_then(OValue::as_str) != Some(slug) {
                    return Err(INVALID);
                }
                match date(row.get("created_at")) {
                    Some(time) if time >= options.start_ms && time <= options.end_ms => {}
                    _ => return Err(INVALID),
                }
                let code = count(row.get("response_status_code"))?;
                ids.insert(id);
                *codes.entry(code).or_default() += 1;
                s.records = ids.len() as u64;
            }
            let collected = ids.len() as u64;
            if collected > wanted {
                return Err(INVALID);
            }
            if collected == wanted {
                return Ok(());
            }
            if records.is_empty() {
                return Err(INVALID);
            }
        }
        s.status = Status::Partial;
        s.notes.push(BUDGET_NOTE.to_owned());
        Ok(())
    }
    .await;
    c.coverage.push(vec![
        Cell::from("Server total"),
        expected.map_or(Cell::Null, Cell::Count),
    ]);
    c.coverage.push(vec![
        Cell::from("Unique collected"),
        Cell::Count(ids.len() as u64),
    ]);
    for (code, n) in &codes {
        c.coverage
            .push(vec![Cell::Text(format!("HTTP {code}")), Cell::Count(*n)]);
    }
    c.finish(s, result)?;

    // 21–23. The selected workspace's configuration lists.
    for name in ["configs.list", "apiKeys.listService", "apiKeys.listUser"] {
        let mut s = open(name, WORKSPACE_CONFIGURATION);
        let result = async {
            let id = workspace_id.ok_or(INVALID)?;
            let answer = match name {
                "configs.list" => reader.configs(id).await?,
                "apiKeys.listService" => reader.service_keys(id).await?,
                _ => reader.user_keys(id).await?,
            };
            let rows = list(&answer, &mut s)?;
            c.inventory.push(vec![
                Cell::from(name),
                Cell::from("Rows returned"),
                Cell::Count(rows.len() as u64),
            ]);
            Ok(())
        }
        .await;
        c.finish(s, result)?;
    }

    // 24. The organisation: the number of configured limit categories only.
    let mut s = open("organisations.getInfo", "Current tenant configuration");
    let result = async {
        let answer = reader.organisation_info().await?;
        let limits = record(Some(&answer))
            .and_then(|info| record(info.get("benefits")))
            .and_then(|benefits| record(benefits.get("limits")))?;
        c.inventory.push(vec![
            Cell::from("organisations.getInfo"),
            Cell::from("Configured limit categories"),
            Cell::Count(limits.as_object().map_or(0, <[(String, OValue)]>::len) as u64),
        ]);
        s.records = 1;
        s.pages = 1;
        Ok(())
    }
    .await;
    c.finish(s, result)?;

    // 25. The guardrail catalog: the number of evaluators only.
    let mut s = open(
        "guardrails.getCatalog",
        "Current tenant capability catalog (not enabled guardrails)",
    );
    let result = async {
        let answer = reader.catalog().await?;
        let Some(evals) = record(Some(&answer))?
            .get("evals")
            .and_then(OValue::as_array)
        else {
            return Err(INVALID);
        };
        c.inventory.push(vec![
            Cell::from("guardrails.getCatalog"),
            Cell::from("Available evaluators"),
            Cell::Count(evals.len() as u64),
        ]);
        s.records = evals.len() as u64;
        s.pages = 1;
        Ok(())
    }
    .await;
    c.finish(s, result)?;

    // One finding per incomplete source, after the error-category findings (not sorted).
    let incomplete: Vec<(&'static str, String)> = c
        .report
        .sources
        .iter()
        .filter(|source| source.status != Status::Complete)
        .map(|source| (source.name, source.notes.join(" ")))
        .collect();
    for (name, evidence) in incomplete {
        c.finding_owned(
            Priority::Review,
            format!("Incomplete evidence: {name}"),
            evidence,
            "Resolve source access or validation problems and rerun. Do not interpret missing metrics as zero.",
            name,
        );
    }
    let table = |title: &str, note: &str, headers: &[&str], rows: Vec<Vec<Cell>>| Table {
        title: title.to_owned(),
        note: note.to_owned(),
        headers: headers.iter().map(|header| (*header).to_owned()).collect(),
        rows,
    };
    c.report.tables = vec![
        table(
            "Telemetry metrics",
            "Server field names are preserved. cost: cents; latency and avgCacheLatency: ms; errorPercent: percent. Feedback is not a request count.",
            &["Source", "Metric", "Value"],
            std::mem::take(&mut c.metrics),
        ),
        table(
            "Error categories",
            "Server-reported errors by HTTP status; 446 denotes an AIRS block.",
            &["HTTP status", "Errors"],
            std::mem::take(&mut c.statuses),
        ),
        table(
            "Error timeline",
            "Server UTC buckets; counts by HTTP status. Not a count of unique violating sessions.",
            &["UTC bucket", "HTTP status", "Errors"],
            std::mem::take(&mut c.trends),
        ),
        table(
            "Current configuration and capabilities",
            "Aggregate counts only. Active tenant workspaces are tenant-wide; keys and configs use the selected workspace.",
            &["Source", "Metric", "Value"],
            std::mem::take(&mut c.inventory),
        ),
        table(
            "Transaction coverage",
            "Unique collected transactions by HTTP status; only complete pagination represents the entire window. Raw rows are excluded.",
            &["Metric", "Value"],
            std::mem::take(&mut c.coverage),
        ),
    ];
    let report = &mut c.report;
    let telemetry = report
        .sources
        .iter()
        .any(|source| source.name.starts_with("telemetry.") && source.status == Status::Complete);
    report.health = if !telemetry {
        Health::Unknown
    } else if report
        .findings
        .iter()
        .any(|finding| finding.priority == Priority::Attention)
    {
        Health::Attention
    } else if !report.findings.is_empty() {
        Health::Review
    } else {
        Health::NoFindings
    };
    report.generated_at = iso((options.now)());
    Ok(c.report)
}

#[cfg(test)]
#[path = "gateway_tests.rs"]
mod tests;
