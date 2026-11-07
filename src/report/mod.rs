//! The environment-report model and collector (TS `src/reports/redteam.ts`, DESIGN
//! §15.17), shared by the later `runtime`, `gateway` and `supplychain` reports.
//!
//! The collector reads seven read-only sources in TypeScript's order, each under its own
//! budget, and keeps only aggregates: no target names, ids, connection settings,
//! scripts, variables, prompts or error bodies. A source that fails is `unavailable`
//! (no records) or `partial` (some records); missing data is never shown as zero.

pub mod gateway;
pub mod html;
#[cfg(test)]
pub mod runtime;
#[cfg(test)]
pub mod runtime_html;
pub use airs_runtime_core::signal;
pub use airs_runtime_core::time::{iso, parse_iso};
pub mod supplychain;

use crate::json::OValue;

/// The page size of the paged sources.
pub const PAGE_SIZE: u32 = 15;

/// The 24-hour creation window, in milliseconds.
const DAY_MS: i64 = 86_400_000;

/// The scope statements of the red team report (TS `limitations`; its profile is
/// [`html::RED_TEAM`]).
pub const LIMITATIONS: [&str; 7] = [
    "This is a read-only snapshot, not a security certification, availability SLA or numerical health score.",
    "Only scan creation timestamps are filtered locally to the preceding 24 hours. Current scan status is not a history of status changes.",
    "Dashboard statistics use server defaults with an unverified measurement window. They are not daily totals and need not match the paginated scan inventory.",
    "Targets and adapters are current configuration inventories, not configuration change logs. Broker channels not online may be intentionally idle; no outage is inferred.",
    "Risk categories are server-reported. Quota consumption is not reset or reinterpreted as daily usage; unlimited zero allocation is not exhaustion.",
    "Partial sources describe collected records only. Missing data is unknown, never zero. Independently read pages are not an atomic snapshot.",
    "Only aggregate counters and category labels are included. Target names, IDs, connection settings, scripts, variables, prompts and scan content are excluded.",
];

/// A source's completeness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Complete,
    Partial,
    Unavailable,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }
}

/// One read-only source and what was collected from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub name: &'static str,
    pub method: &'static str,
    /// The window column of the evidence table (owned: the supplychain Scans window holds
    /// the run's times, DESIGN §20.9).
    pub window: String,
    pub status: Status,
    pub records: u64,
    pub pages: u64,
    pub notes: Vec<String>,
}

/// A finding's priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Attention,
    Review,
}

impl Priority {
    pub fn name(self) -> &'static str {
        match self {
            Self::Attention => "attention",
            Self::Review => "review",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub priority: Priority,
    pub title: String,
    pub evidence: String,
    pub recommendation: String,
    pub source: String,
}

/// A table cell: text, a counter, `null` (rendered `Unknown`) or a server number
/// (rendered as ECMAScript `Number::toString`).
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Text(String),
    Count(u64),
    Null,
    Number(f64),
}

impl From<&str> for Cell {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub title: String,
    pub note: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
}

/// The assessment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Attention,
    Review,
    NoFindings,
    Unknown,
}

impl Health {
    /// The badge text (TS `HEALTH`).
    pub fn label(self) -> &'static str {
        match self {
            Self::Attention => "Attention required",
            Self::Review => "Review required",
            Self::NoFindings => "No findings in available evidence",
            Self::Unknown => "Unable to assess",
        }
    }
}

/// An environment report (schema 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub title: String,
    pub collection_started_at: String,
    pub generated_at: String,
    pub window_start: String,
    pub window_end: String,
    pub health: Health,
    pub sources: Vec<Source>,
    pub findings: Vec<Finding>,
    pub tables: Vec<Table>,
}

impl Report {
    /// The collected tables plus `Evidence and completeness` (TS `tables(r)`).
    pub fn all_tables(&self) -> Vec<Table> {
        let mut tables = self.tables.clone();
        tables.push(Table {
            title: "Evidence and completeness".to_owned(),
            note: "Complete means the source was collected within its budget, not that the environment is secure. Unavailable is not zero.".to_owned(),
            headers: ["Source", "SDK method", "Window", "Status", "Records", "Pages", "Notes"]
                .map(str::to_owned)
                .to_vec(),
            rows: self
                .sources
                .iter()
                .map(|source| {
                    let notes = source.notes.join(" ");
                    vec![
                        Cell::from(source.name),
                        Cell::from(source.method),
                        Cell::Text(source.window.clone()),
                        Cell::from(source.status.name()),
                        Cell::Count(source.records),
                        Cell::Count(source.pages),
                        Cell::Text(if notes.is_empty() {
                            "Collection completed.".to_owned()
                        } else {
                            notes
                        }),
                    ]
                })
                .collect(),
        });
        tables
    }

    /// Sources that are not complete.
    pub fn incomplete(&self) -> usize {
        self.sources
            .iter()
            .filter(|source| source.status != Status::Complete)
            .count()
    }
}

/// Why a source read failed (no upstream text is kept).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// An HTTP answer with this status (the TS `statusCode`).
    Status(u16),
    /// Any other request or validation failure.
    Failed,
    /// A local data check (TS `DataError`) with its fixed text.
    Data(&'static str),
    /// A signal: the report stops.
    Cancelled,
}

impl ReadError {
    /// TS `failure(error)`.
    fn note(&self) -> String {
        match self {
            Self::Data(text) => (*text).to_owned(),
            Self::Status(code) if (400..=599).contains(code) => format!(
                "HTTP {code}: source unavailable; check credentials, service availability and SDK compatibility."
            ),
            _ => "Source request or validation failed. No upstream error body is included."
                .to_owned(),
        }
    }
}

/// A signal stopped the collection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interrupted;

/// The seven reads, as ordered JSON documents. The paged reads take the `skip` of the
/// next page (page size [`PAGE_SIZE`]); `adapters` asks for `include_target_count`.
pub(crate) trait Reader {
    async fn overview(&self) -> Result<OValue, ReadError>;
    async fn statistics(&self) -> Result<OValue, ReadError>;
    async fn targets(&self, skip: u64) -> Result<OValue, ReadError>;
    async fn quota(&self) -> Result<OValue, ReadError>;
    async fn scans(&self, skip: u64) -> Result<OValue, ReadError>;
    async fn channel_stats(&self) -> Result<OValue, ReadError>;
    async fn adapters(&self, skip: u64) -> Result<OValue, ReadError>;
}

/// The report options and clock.
pub struct Options<'a> {
    pub title: &'a str,
    pub max_pages: u32,
    /// Milliseconds since the Unix epoch.
    pub now: &'a dyn Fn() -> i64,
}

/// TS `count`: a safe non-negative integer.
fn count(value: Option<&OValue>) -> Result<u64, ReadError> {
    const INVALID: ReadError = ReadError::Data("A required counter is missing or invalid.");
    let Some(OValue::Number(number)) = value else {
        return Err(INVALID);
    };
    if let Some(integer) = number.as_u64() {
        return (integer <= 9_007_199_254_740_991)
            .then_some(integer)
            .ok_or(INVALID);
    }
    if number.is_i64() {
        return Err(INVALID);
    }
    match number.as_f64() {
        Some(value) if value >= 0.0 && value.fract() == 0.0 && value <= 9_007_199_254_740_991.0 => {
            // An integral value within the safe range converts exactly.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let integer = value as u64;
            Ok(integer)
        }
        _ => Err(INVALID),
    }
}

/// Whether `c` is a Unicode control (Cc) or format (Cf) character (`\p{Cc}\p{Cf}`).
pub fn is_control_or_format(c: char) -> bool {
    if c.is_control() {
        return true;
    }
    matches!(
        u32::from(c),
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

/// How a report cleans a label: the UTF-16 unit limit and whether it is trimmed first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LabelRule {
    /// The UTF-16 units kept.
    pub units: usize,
    /// JavaScript trim after the cleaning, and an empty result is `Unknown`.
    pub trim: bool,
}

/// The red team report's rule (TS `label` of `redteam.ts`): 120 units, no trim.
pub(crate) const RED_TEAM_LABEL: LabelRule = LabelRule {
    units: 120,
    trim: false,
};

/// TS `label`: a string with `\p{Cc}\p{Cf}` as spaces, cut to 120 UTF-16 units (a cut
/// surrogate pair leaves the lone high surrogate, which Node writes as U+FFFD); anything
/// else is `Unknown`.
pub fn label(value: Option<&OValue>) -> String {
    label_with(value, RED_TEAM_LABEL)
}

/// A label under `rule`; anything but a string is `Unknown`.
pub(crate) fn label_with(value: Option<&OValue>, rule: LabelRule) -> String {
    let Some(OValue::String(text)) = value else {
        return "Unknown".to_owned();
    };
    label_text_with(text, rule)
}

/// The label of a string under `rule`: `\p{Cc}\p{Cf}` → space, the trim of the rule
/// (empty → `Unknown`), then the first `rule.units` UTF-16 units (a cut surrogate pair
/// leaves the lone high surrogate, which Node writes as U+FFFD).
pub(crate) fn label_text_with(text: &str, rule: LabelRule) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if is_control_or_format(c) { ' ' } else { c })
        .collect();
    let kept = if rule.trim {
        let trimmed = crate::tenant::config::js_trim(&cleaned);
        if trimmed.is_empty() {
            return "Unknown".to_owned();
        }
        trimmed
    } else {
        cleaned.as_str()
    };
    let mut out = String::new();
    let mut units = 0;
    for c in kept.chars() {
        let width = c.len_utf16();
        if units + width > rule.units {
            if units < rule.units {
                out.push('\u{fffd}');
            }
            break;
        }
        units += width;
        out.push(c);
    }
    out
}

/// An approximation of `String.prototype.localeCompare` for the group labels: ASCII
/// case-insensitive first (punctuation, digits, letters in ICU's order for ASCII), then
/// lowercase before uppercase, then code points.
pub(crate) fn locale_order(a: &str, b: &str) -> std::cmp::Ordering {
    let folded = |text: &str| text.to_lowercase();
    folded(a)
        .cmp(&folded(b))
        .then_with(|| {
            let lower_first = |text: &str| {
                text.chars()
                    .map(|c| if c.is_uppercase() { 1u8 } else { 0 })
                    .collect::<Vec<_>>()
            };
            lower_first(a).cmp(&lower_first(b))
        })
        .then_with(|| a.cmp(b))
}

/// TS `groups`: counts per label, sorted by label.
fn groups<'a>(values: impl Iterator<Item = Option<&'a OValue>>) -> Vec<Vec<Cell>> {
    let mut counts: Vec<(String, u64)> = Vec::new();
    for value in values {
        let key = label(value);
        match counts.iter_mut().find(|(name, _)| *name == key) {
            Some((_, count)) => *count += 1,
            None => counts.push((key, 1)),
        }
    }
    counts.sort_by(|(a, _), (b, _)| locale_order(a, b));
    counts
        .into_iter()
        .map(|(name, count)| vec![Cell::Text(name), Cell::Count(count)])
        .collect()
}

/// The collection in progress.
struct Collector {
    report: Report,
    max_pages: u32,
    end: i64,
}

/// The rows of one paged source (TS `pages`): the identity, total and page-size checks
/// and the page budget. With some rows a failure makes the source `partial`.
async fn pages<F, Fut>(
    source: &mut Source,
    max_pages: u32,
    mut read: F,
) -> Result<Vec<OValue>, ReadError>
where
    F: FnMut(u64) -> Fut,
    Fut: std::future::Future<Output = Result<OValue, ReadError>>,
{
    let mut rows: Vec<OValue> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut total: Option<u64> = None;
    let outcome: Result<(), ReadError> = async {
        for _ in 0..max_pages {
            let skip = rows.len() as u64;
            let page = read(skip).await?;
            source.pages += 1;
            let Some(data) = page.get("data").and_then(OValue::as_array) else {
                return Err(ReadError::Data("Missing inventory array."));
            };
            let reported = count(
                page.get("pagination")
                    .and_then(|pagination| pagination.get("total_items")),
            )?;
            let expected = *total.get_or_insert(reported);
            if expected != reported {
                return Err(ReadError::Data("Inventory changed during pagination."));
            }
            if data.len() > PAGE_SIZE as usize {
                return Err(ReadError::Data("Page exceeds requested size."));
            }
            for row in data {
                let identity = match row.get("uuid") {
                    Some(OValue::String(uuid)) if !uuid.is_empty() => uuid.clone(),
                    _ => {
                        return Err(ReadError::Data(
                            "Missing or repeated identity; pagination stopped.",
                        ));
                    }
                };
                if !seen.insert(identity) {
                    return Err(ReadError::Data(
                        "Missing or repeated identity; pagination stopped.",
                    ));
                }
                rows.push(row.clone());
                source.records = rows.len() as u64;
            }
            let collected = rows.len() as u64;
            if collected > expected {
                return Err(ReadError::Data("Inventory exceeds reported total."));
            }
            if collected == expected {
                return Ok(());
            }
            if data.is_empty() {
                return Err(ReadError::Data("Premature empty page."));
            }
        }
        Err(ReadError::Data(budget_note(max_pages)))
    }
    .await;
    match outcome {
        Ok(()) => Ok(rows),
        Err(ReadError::Cancelled) => Err(ReadError::Cancelled),
        Err(error) if rows.is_empty() => Err(error),
        Err(error) => {
            source.status = Status::Partial;
            source.notes.push(error.note());
            Ok(rows)
        }
    }
}

/// `Reached the <n>-page budget; inventory is incomplete.` (a fixed text per budget).
fn budget_note(max_pages: u32) -> &'static str {
    const NOTES: [&str; 100] = budget_notes();
    NOTES[usize::try_from(max_pages.clamp(1, 100) - 1).unwrap_or(0)]
}

macro_rules! budget_list {
    ($($n:literal)*) => {
        [$(concat!("Reached the ", $n, "-page budget; inventory is incomplete.")),*]
    };
}

const fn budget_notes() -> [&'static str; 100] {
    budget_list!(1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29
        30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59
        60 61 62 63 64 65 66 67 68 69 70 71 72 73 74 75 76 77 78 79 80 81 82 83 84 85 86 87 88 89
        90 91 92 93 94 95 96 97 98 99 100)
}

impl Collector {
    fn finding(
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

    fn table(&mut self, title: &str, note: &str, headers: &[&str], rows: Vec<Vec<Cell>>) {
        self.report.tables.push(Table {
            title: title.to_owned(),
            note: note.to_owned(),
            headers: headers.iter().map(|header| (*header).to_owned()).collect(),
            rows,
        });
    }

    /// Runs one source (TS `source`): its status, notes and the `<name> incomplete`
    /// finding.
    fn finish(&mut self, mut source: Source, result: Result<(), ReadError>) {
        match result {
            Ok(()) => {
                if source.status != Status::Partial {
                    source.status = Status::Complete;
                }
            }
            Err(error) => {
                source.status = if source.records > 0 {
                    Status::Partial
                } else {
                    Status::Unavailable
                };
                source.notes.push(error.note());
            }
        }
        if source.status != Status::Complete {
            self.finding(
                Priority::Review,
                format!("{} incomplete", source.name),
                source.notes.join(" "),
                "Resolve the source error or increase the page budget, then regenerate.",
                source.name,
            );
        }
        self.report.sources.push(source);
    }
}

/// A source that has not been read yet (unavailable, nothing counted).
pub(crate) fn source(name: &'static str, method: &'static str, window: &str) -> Source {
    Source {
        name,
        method,
        window: window.to_owned(),
        status: Status::Unavailable,
        records: 0,
        pages: 0,
        notes: Vec::new(),
    }
}

/// A cancelled read stops the report; every other failure belongs to its source.
fn cancelled<T>(result: &Result<T, ReadError>) -> bool {
    matches!(result, Err(ReadError::Cancelled))
}

/// Collects the seven sources (TS `collectRedTeamEnvironmentReport`).
pub(crate) async fn collect(
    reader: &impl Reader,
    options: &Options<'_>,
) -> Result<Report, Interrupted> {
    let end = (options.now)();
    let mut collector = Collector {
        report: Report {
            title: options.title.to_owned(),
            collection_started_at: iso(end),
            generated_at: String::new(),
            window_start: iso(end - DAY_MS),
            window_end: iso(end),
            health: Health::Unknown,
            sources: Vec::new(),
            findings: Vec::new(),
            tables: Vec::new(),
        },
        max_pages: options.max_pages,
        end,
    };
    let c = &mut collector;

    // 1. Overview.
    let mut s = source(
        "Overview",
        "getDashboardOverview()",
        "Current dashboard snapshot",
    );
    let read = reader.overview().await;
    if cancelled(&read) {
        return Err(Interrupted);
    }
    let result = read.and_then(|overview| {
        let total = count(overview.get("total_targets"))?;
        c.table(
            "Dashboard overview",
            &s.window,
            &["Metric", "Count"],
            vec![vec![Cell::from("Targets"), Cell::Count(total)]],
        );
        s.records = 1;
        s.pages = 1;
        Ok(())
    });
    c.finish(s, result);

    // 2. Statistics.
    let mut s = source(
        "Statistics",
        "getScanStatistics()",
        "Server-default window (unverified)",
    );
    let read = reader.statistics().await;
    if cancelled(&read) {
        return Err(Interrupted);
    }
    let result = read.and_then(|stats| {
        let mut rows = vec![
            vec![
                Cell::from("Scans"),
                Cell::Count(count(stats.get("total_scans"))?),
            ],
            vec![
                Cell::from("Targets scanned"),
                Cell::Count(count(stats.get("targets_scanned"))?),
            ],
        ];
        for risk in stats
            .get("risk_profile")
            .and_then(OValue::as_array)
            .unwrap_or_default()
        {
            let n = count(risk.get("total"))?;
            let rating = label(risk.get("risk_rating"));
            rows.push(vec![Cell::Text(format!("Risk: {rating}")), Cell::Count(n)]);
            let raw = risk
                .get("risk_rating")
                .and_then(OValue::as_str)
                .unwrap_or_default();
            if (raw.eq_ignore_ascii_case("high") || raw.eq_ignore_ascii_case("critical")) && n > 0 {
                c.finding(
                    Priority::Attention,
                    "Elevated risk reported".to_owned(),
                    format!("{n} in the {rating} risk category."),
                    "Review affected targets in Red Team; the statistics window is server-defined.",
                    s.name,
                );
            }
        }
        for status in stats
            .get("scan_status")
            .and_then(OValue::as_array)
            .unwrap_or_default()
        {
            rows.push(vec![
                Cell::Text(format!("Status: {}", label(status.get("name")))),
                Cell::Count(count(status.get("count"))?),
            ]);
        }
        c.table(
            "Dashboard scan statistics",
            &s.window,
            &["Metric", "Count"],
            rows,
        );
        s.records = 1;
        s.pages = 1;
        Ok(())
    });
    c.finish(s, result);

    // 3. Targets.
    let mut s = source("Targets", "targets.list()", "Current paginated inventory");
    let rows = pages(&mut s, c.max_pages, |skip| reader.targets(skip)).await;
    if cancelled(&rows) {
        return Err(Interrupted);
    }
    let result = rows.map(|rows| {
        c.table(
            "Target inventory",
            "Collected targets by type; consult source completeness.",
            &["Type", "Count"],
            groups(rows.iter().map(|row| row.get("target_type"))),
        );
    });
    c.finish(s, result);

    // 4. Quota.
    let mut s = source("Quota", "getQuotaSummary() — GET", "Current quota snapshot");
    let read = reader.quota().await;
    if cancelled(&read) {
        return Err(Interrupted);
    }
    let result = read.and_then(|quota| {
        let mut rows = Vec::new();
        for kind in ["static", "dynamic", "custom"] {
            let q = quota.get(kind);
            let allocated = count(q.and_then(|q| q.get("allocated")))?;
            let consumed = count(q.and_then(|q| q.get("consumed")))?;
            let Some(OValue::Bool(unlimited)) = q.and_then(|q| q.get("unlimited")) else {
                return Err(ReadError::Data("Missing unlimited quota flag."));
            };
            rows.push(vec![
                Cell::from(kind),
                Cell::Count(allocated),
                Cell::Count(consumed),
                Cell::Text(if *unlimited {
                    "Unlimited".to_owned()
                } else {
                    allocated.saturating_sub(consumed).to_string()
                }),
            ]);
            if !*unlimited && consumed >= allocated {
                c.finding(
                    Priority::Attention,
                    format!("{kind} quota has no remaining allocation"),
                    format!("{consumed} consumed; {allocated} allocated."),
                    "Review quota allocation before scheduling more scans.",
                    s.name,
                );
            }
        }
        c.table(
            "Quota",
            "Remaining is shown only for finite quotas; these are not daily usage counters.",
            &["Type", "Allocated", "Consumed", "Remaining"],
            rows,
        );
        s.records = 3;
        s.pages = 1;
        Ok(())
    });
    c.finish(s, result);

    // 5. Scans.
    let mut s = source(
        "Scans",
        "scans.list()",
        "Paginated inventory; local 24-hour creation filter",
    );
    let rows = pages(&mut s, c.max_pages, |skip| reader.scans(skip)).await;
    if cancelled(&rows) {
        return Err(Interrupted);
    }
    let end = c.end;
    let result = rows.map(|rows| {
        let mut unknown = 0u64;
        let mut recent = 0u64;
        for row in &rows {
            let time = match row.get("created_at") {
                Some(OValue::String(text)) if has_zone(text) => parse_iso(text),
                _ => None,
            };
            match time {
                Some(time) if time <= end => {
                    if time >= end - DAY_MS {
                        recent += 1;
                    }
                }
                _ => unknown += 1,
            }
        }
        c.table(
            "Scan inventory and daily creation activity",
            "Daily count covers collected records created in the stated UTC window, not scans completed or attacks executed.",
            &["Metric", "Count"],
            vec![
                vec![Cell::from("Collected scans"), Cell::Count(rows.len() as u64)],
                vec![Cell::from("Created in preceding 24 hours"), Cell::Count(recent)],
                vec![
                    Cell::from("Unusable or future creation timestamps"),
                    Cell::Count(unknown),
                ],
            ],
        );
        c.table(
            "Current status of collected scans",
            "All collected scans, independent of creation window.",
            &["Status", "Count"],
            groups(rows.iter().map(|row| row.get("status"))),
        );
        let failed = rows
            .iter()
            .filter(|row| {
                let status = row.get("status").and_then(OValue::as_str).unwrap_or_default();
                status.eq_ignore_ascii_case("failed") || status.eq_ignore_ascii_case("error")
            })
            .count();
        if failed > 0 {
            c.finding(
                Priority::Attention,
                "Scans require failure review".to_owned(),
                format!("{failed} collected scans currently report failed/error status."),
                "Inspect individual scan reports and error logs before rerunning; no reruns are performed here.",
                s.name,
            );
        }
        if unknown > 0 {
            c.finding(
                Priority::Review,
                "Daily activity has timestamp gaps".to_owned(),
                format!("{unknown} records could not be assigned to the daily window."),
                "Review source timestamps; do not interpret the daily count as complete.",
                s.name,
            );
        }
    });
    c.finish(s, result);

    // 6. Network broker.
    let mut s = source(
        "Network broker",
        "networkBroker.getChannelStats()",
        "Current channel snapshot",
    );
    let read = reader.channel_stats().await;
    if cancelled(&read) {
        return Err(Interrupted);
    }
    let result = read.and_then(|stats| {
        let total = count(stats.get("total_channels"))?;
        let online = count(stats.get("online_channels"))?;
        if online > total {
            return Err(ReadError::Data("Online channel count exceeds total."));
        }
        c.table(
            "Network broker",
            "Not-online channels may be intentionally idle; this does not establish an outage.",
            &["Metric", "Count"],
            vec![
                vec![Cell::from("Configured channels"), Cell::Count(total)],
                vec![Cell::from("Online channels"), Cell::Count(online)],
            ],
        );
        if total > online {
            c.finding(
                Priority::Review,
                "Confirm expected broker connectivity".to_owned(),
                format!("{online} of {total} configured channels are online."),
                "Compare with intended channel schedules before investigating connectivity.",
                s.name,
            );
        }
        s.records = 1;
        s.pages = 1;
        Ok(())
    });
    c.finish(s, result);

    // 7. Adapters.
    let mut s = source(
        "Adapters",
        "adapters.list({ include_target_count: true })",
        "Current paginated configuration",
    );
    let rows = pages(&mut s, c.max_pages, |skip| reader.adapters(skip)).await;
    if cancelled(&rows) {
        return Err(Interrupted);
    }
    let result = rows.map(|rows| {
        c.table(
            "Adapter inventory",
            "Configuration status only; scripts and variables are excluded.",
            &["Status", "Count"],
            groups(rows.iter().map(|row| row.get("status"))),
        );
    });
    c.finish(s, result);

    let report = &mut collector.report;
    report.health = if report
        .sources
        .iter()
        .all(|source| source.status == Status::Unavailable)
    {
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
    Ok(collector.report)
}

/// TS `/(?:Z|[+-]\d{2}:\d{2})$/i`.
fn has_zone(text: &str) -> bool {
    if text.ends_with('Z') || text.ends_with('z') {
        return true;
    }
    let bytes = text.as_bytes();
    bytes.len() >= 6 && {
        let tail = &bytes[bytes.len() - 6..];
        matches!(tail[0], b'+' | b'-')
            && tail[1].is_ascii_digit()
            && tail[2].is_ascii_digit()
            && tail[3] == b':'
            && tail[4].is_ascii_digit()
            && tail[5].is_ascii_digit()
    }
}

/// A zoned ISO instant or date-only UTC midnight, shared by inventory reports.
pub(crate) fn parse_date_text(text: &str) -> Option<i64> {
    parse_iso(text).or_else(|| {
        let bytes = text.as_bytes();
        (bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-')
            .then(|| parse_iso(&format!("{text}T00:00:00Z")))
            .flatten()
    })
}

#[cfg(test)]
mod tests;
