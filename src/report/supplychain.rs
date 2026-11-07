//! The supplychain environment report's collector and model (TS
//! `src/reports/agentguard.ts`, DESIGN §20.9; TypeScript `agentguard report`, renamed,
//! D308).
//!
//! Three read-only sources are read one after the other in TypeScript's order (`Scans`,
//! `Statistics`, `Rules`), each under its own budget, and only aggregates are kept: label
//! counts over closed vocabularies and the two server statistics. No scan name, id, URL,
//! fingerprint, device metadata, rule text, finding text or server string reaches the
//! model: the reader hands the collector only the members of [`ScanRow`], [`RuleRow`] and
//! [`Stats`], and every label outside its vocabulary is `OTHER`. A source that fails is
//! `unavailable` (no records) or `partial` (some records); missing data is never shown as
//! zero. The rendering is the shared environment template ([`super::html::render`] with
//! [`super::html::SUPPLYCHAIN`]).

use std::collections::HashSet;

use super::{
    Cell, Finding, Health, Interrupted, Priority, ReadError, Report, Source, Status, Table, iso,
    locale_order, source,
};
use crate::walk::total_of;

/// The default title (TS `AgentGuard environment report`, D309).
pub const DEFAULT_TITLE: &str = "Supply chain environment report";
/// The page size of both paged sources (TS `limit: 10`).
pub const PAGE_SIZE: u32 = 10;
/// The default page budget per paged source (TS `--max-pages` 40).
pub const DEFAULT_MAX_PAGES: u32 = 40;
/// The scan creation window, in milliseconds (30 × 24 h).
const WINDOW_MS: i64 = 30 * 86_400_000;

/// The scope statements of the supplychain report (TS `limitations`, `agentguard.ts:37-41`).
pub const LIMITATIONS: [&str; 5] = [
    "Read-only evidence, not a security certification or numerical health score. Browser APIs are undocumented and may change.",
    "Scans use explicit start/end timestamps; statistics use the server rolling 30_DAYS window. These independently read sources are not an atomic snapshot and need not reconcile.",
    "Scan rows are not unique skills. Parent/batch rows are excluded from outcome counts to avoid counting them alongside children; per-scan vulnerability counts are never summed.",
    "Rule default states describe the catalog, not effective tenant policies. No scan creation, policy changes or skill uploads are performed.",
    "Missing or incomplete evidence is unknown, not zero. Scan names, identifiers, URLs, device metadata, source code and finding text are excluded.",
];

/// The note of a failed source (TS `source`; no `HTTP <code>` note, choice 32).
pub const FAILED_NOTE: &str = "Request or validation failed; no upstream error body included.";
/// The note of a paged source that stopped with rows collected (TS `pages`).
pub const INCOMPLETE_NOTE: &str = "Inventory incomplete: pagination changed, repeated, stopped early, failed or reached its budget.";
/// The note of a Scans source that excluded rows by their creation time.
pub const EXCLUDED_NOTE: &str =
    "Rows with missing or out-of-window creation timestamps were excluded.";
/// The note of a Scans source that left rows out of the outcome counts because neither
/// `is_batch` nor `child_scan_uuids` decided their membership (a `null` where the other
/// does not show a parent, G71): a parent counted beside its children would count it twice.
pub const UNKNOWN_BATCH_NOTE: &str =
    "Rows with unknown batch membership were excluded from outcome counts.";
const RECOMMENDATION: &str = "Check service access or increase the page budget, then regenerate.";

const ARTIFACTS: [&str; 2] = ["SKILL", "AGENT"];
const OUTCOMES: [&str; 4] = ["BLOCKED", "ALLOWED", "PENDING", "ERROR"];
const STATES: [&str; 2] = ["BLOCKING", "ALLOWING"];

/// One scan as the collector sees it: only what the aggregates need. `None` is a JSON
/// `null` or an absent member (G71: the service sends `null` where the SDK contract
/// requires a value).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanRow {
    pub uuid: String,
    /// `None`: a missing creation time (the row is excluded, [`EXCLUDED_NOTE`]).
    pub created_at: Option<String>,
    /// `None` for a JSON `null` (TS `label` gives `UNKNOWN`).
    pub artifact_type: Option<String>,
    /// `None`: an unfamiliar status (not `COMPLETED`).
    pub status: Option<String>,
    pub eval_outcome: Option<String>,
    /// `None`: unknown, unless `children` shows a parent ([`batch_membership`],
    /// [`UNKNOWN_BATCH_NOTE`]).
    pub is_batch: Option<bool>,
    /// The number of `child_scan_uuids`; `None` when the list is `null` (unknown, unless
    /// `is_batch` is `true`).
    pub children: Option<usize>,
}

/// A scan row's batch membership as the outcome counts read it (TS `agentguard.ts:131-171`,
/// `!s.is_batch && !s.child_scan_uuids?.length` is a leaf).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Membership {
    /// `is_batch` true or at least one child: excluded as a parent/batch row.
    Parent,
    /// `is_batch` false and no child: counted.
    Leaf,
    /// Neither signal decides (a `null` where the other does not show a parent).
    Unknown,
}

/// Either signal alone decides a parent; a leaf needs both (choice 84).
fn batch_membership(row: &ScanRow) -> Membership {
    match (row.is_batch, row.children) {
        (Some(true), _) | (_, Some(1..)) => Membership::Parent,
        (Some(false), Some(0)) => Membership::Leaf,
        _ => Membership::Unknown,
    }
}

/// One rule as the collector sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleRow {
    pub uuid: String,
    pub default_state: Option<String>,
}

/// One page of a paged source: its rows and `pagination.total_items` (a JavaScript
/// number).
#[derive(Clone, Debug, PartialEq)]
pub struct Page<T> {
    pub rows: Vec<T>,
    pub total: f64,
}

/// One server statistic (`{count, percent_change}`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metric {
    pub count: f64,
    pub percent_change: Option<f64>,
}

/// The statistics answer: the two metrics the report shows (`top_vulnerability` unused).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub unique_skills_scanned: Metric,
    pub total_vulnerabilities_found: Metric,
}

/// A row with an identity (TS `{ uuid: string }`).
trait Identified {
    fn uuid(&self) -> &str;
}

impl Identified for ScanRow {
    fn uuid(&self) -> &str {
        &self.uuid
    }
}

impl Identified for RuleRow {
    fn uuid(&self) -> &str {
        &self.uuid
    }
}

/// The three reads. The paged reads take the `skip` of the next page (page size
/// [`PAGE_SIZE`]); `scans` takes the window's `start_time` and `end_time` as sent.
pub(crate) trait Reader {
    /// `GET <data>/v1/scans?skip=<skip>&limit=10&isBackgroundRefresh=false&start_time=…&end_time=…`.
    async fn scans(&self, skip: u64, start: &str, end: &str) -> Result<Page<ScanRow>, ReadError>;
    /// `GET <data>/v1/stats/scans?time_period=30_DAYS`.
    async fn stats(&self) -> Result<Stats, ReadError>;
    /// `GET <mgmt>/v1/rules?skip=<skip>&limit=10`.
    async fn rules(&self, skip: u64) -> Result<Page<RuleRow>, ReadError>;
}

/// The report options and clock.
pub struct Options<'a> {
    pub title: &'a str,
    /// 1 to 100 (checked during planning, D314).
    pub max_pages: u32,
    /// Milliseconds since the Unix epoch.
    pub now: &'a dyn Fn() -> i64,
}

/// A collected row count as a JavaScript number (exact below 2^53).
fn number(count: usize) -> f64 {
    u32::try_from(count).map_or(f64::MAX, f64::from)
}

/// TS `label(v, allowed)`: `null` → `UNKNOWN`, a value outside `allowed` → `OTHER`
/// (case-sensitive).
fn label(value: Option<&str>, allowed: &[&'static str]) -> &'static str {
    match value {
        None => "UNKNOWN",
        Some(value) => allowed
            .iter()
            .find(|candidate| **candidate == value)
            .copied()
            .unwrap_or("OTHER"),
    }
}

/// Counts per label, sorted (TS `[...groups].sort(([a], [b]) => a.localeCompare(b))`).
fn groups(labels: impl Iterator<Item = String>) -> Vec<Vec<Cell>> {
    let mut counts: Vec<(String, u64)> = Vec::new();
    for key in labels {
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

/// The rows of one paged source (TS `pages`, `agentguard.ts:78-119`): per page the total
/// and size checks, for `Scans` (`!page_count`) the first page's total and the rows within
/// it, a non-empty and new `uuid` per row; `Scans` ends at the total, `Rules` at a short
/// page; an empty page that does not end the walk and the page budget fail. A failure with
/// rows collected makes the source `partial` and keeps the rows.
async fn pages<T, F, Fut>(
    source: &mut Source,
    max_pages: u32,
    page_count: bool,
    mut read: F,
) -> Result<Vec<T>, ReadError>
where
    T: Identified,
    F: FnMut(u64) -> Fut,
    Fut: std::future::Future<Output = Result<Page<T>, ReadError>>,
{
    let mut rows: Vec<T> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut total: Option<f64> = None;
    let outcome: Result<(), ReadError> = async {
        for _ in 0..max_pages {
            let skip = u64::try_from(rows.len()).unwrap_or(u64::MAX);
            let page = read(skip).await?;
            source.pages += 1;
            if total_of(Some(page.total)).is_none() || page.rows.len() > PAGE_SIZE as usize {
                return Err(ReadError::Failed);
            }
            let first = *total.get_or_insert(page.total);
            if !page_count && (page.total != first || number(rows.len() + page.rows.len()) > first)
            {
                return Err(ReadError::Failed);
            }
            let received = page.rows.len();
            for row in page.rows {
                if row.uuid().is_empty() || !seen.insert(row.uuid().to_owned()) {
                    return Err(ReadError::Failed);
                }
                rows.push(row);
                source.records = u64::try_from(rows.len()).unwrap_or(u64::MAX);
            }
            let done = if page_count {
                received < PAGE_SIZE as usize
            } else {
                number(rows.len()) == first
            };
            if done {
                return Ok(());
            }
            if received == 0 {
                return Err(ReadError::Failed);
            }
        }
        source
            .notes
            .push(format!("Reached the {max_pages}-page budget."));
        Err(ReadError::Failed)
    }
    .await;
    match outcome {
        Ok(()) => Ok(rows),
        Err(ReadError::Cancelled) => Err(ReadError::Cancelled),
        Err(error) if rows.is_empty() => Err(error),
        Err(_) => {
            source.status = Status::Partial;
            source.notes.push(INCOMPLETE_NOTE.to_owned());
            Ok(rows)
        }
    }
}

/// The collection in progress.
struct Collector {
    report: Report,
}

impl Collector {
    fn finding(
        &mut self,
        priority: Priority,
        title: &str,
        evidence: String,
        recommendation: &str,
        source: &str,
    ) {
        self.report.findings.push(Finding {
            priority,
            title: title.to_owned(),
            evidence,
            recommendation: recommendation.to_owned(),
            source: source.to_owned(),
        });
    }

    fn table(&mut self, title: &str, note: String, headers: &[&str], rows: Vec<Vec<Cell>>) {
        self.report.tables.push(Table {
            title: title.to_owned(),
            note,
            headers: headers.iter().map(|header| (*header).to_owned()).collect(),
            rows,
        });
    }

    /// TS `source`: a finished read is `complete` unless it became `partial`; a failed one
    /// is `partial` with records, else `unavailable`, with [`FAILED_NOTE`]; every source
    /// that is not complete adds the `<name> incomplete` finding. A cancellation stops the
    /// report.
    fn finish(
        &mut self,
        mut source: Source,
        result: Result<(), ReadError>,
    ) -> Result<(), Interrupted> {
        match result {
            Ok(()) => {
                if source.status != Status::Partial {
                    source.status = Status::Complete;
                }
            }
            Err(ReadError::Cancelled) => return Err(Interrupted),
            Err(_) => {
                source.status = if source.records > 0 {
                    Status::Partial
                } else {
                    Status::Unavailable
                };
                source.notes.push(FAILED_NOTE.to_owned());
            }
        }
        if source.status != Status::Complete {
            self.report.findings.push(Finding {
                priority: Priority::Review,
                title: format!("{} incomplete", source.name),
                evidence: source.notes.join(" "),
                recommendation: RECOMMENDATION.to_owned(),
                source: source.name.to_owned(),
            });
        }
        self.report.sources.push(source);
        Ok(())
    }

    /// The Scans analysis (TS `agentguard.ts:131-171`): the window filter, the leaf rows,
    /// the `Scan outcomes` table and the two findings.
    fn scans(&mut self, source: &mut Source, rows: &[ScanRow], start_ms: i64, end_ms: i64) {
        let inside: Vec<&ScanRow> = rows
            .iter()
            .filter(|row| {
                row.created_at
                    .as_deref()
                    .and_then(crate::report::parse_date_text)
                    .is_some_and(|at| at >= start_ms && at <= end_ms)
            })
            .collect();
        if inside.len() != rows.len() {
            source.status = Status::Partial;
            source.notes.push(EXCLUDED_NOTE.to_owned());
        }
        // TS `!is_batch && !child_scan_uuids.length` is a leaf. Either signal alone shows a
        // parent (`is_batch` true, or a child listed), so it is excluded whatever the other
        // holds; a leaf needs both known (`false` and no child). Only a row neither signal
        // decides is of unknown membership: left out of the counts with its note (G71).
        let mut parents = 0usize;
        let mut unknown = 0usize;
        let mut leaf: Vec<&ScanRow> = Vec::new();
        for row in &inside {
            match batch_membership(row) {
                Membership::Parent => parents += 1,
                Membership::Leaf => leaf.push(row),
                Membership::Unknown => unknown += 1,
            }
        }
        if unknown > 0 {
            source.status = Status::Partial;
            source.notes.push(UNKNOWN_BATCH_NOTE.to_owned());
        }
        let counted = groups(leaf.iter().map(|row| {
            format!(
                "{} / {}",
                label(row.artifact_type.as_deref(), &ARTIFACTS),
                label(row.eval_outcome.as_deref(), &OUTCOMES)
            )
        }));
        let completeness = if source.status == Status::Partial {
            "Partial inventory only."
        } else {
            "Complete inventory for requested window."
        };
        self.table(
            "Scan outcomes",
            format!(
                "Collected {} rows; excluded {parents} parent/batch rows. {completeness}",
                source.records,
            ),
            &["Artifact / outcome", "Scan rows"],
            counted,
        );
        let blocked = leaf
            .iter()
            .filter(|row| row.eval_outcome.as_deref() == Some("BLOCKED"))
            .count();
        if blocked > 0 {
            self.finding(
                Priority::Attention,
                "Blocked scans require review",
                format!("{blocked} collected non-parent scan rows have BLOCKED evaluations."),
                "Inspect the affected scans and findings before approving the artifacts.",
                "Scans",
            );
        }
        let unsettled = leaf
            .iter()
            .filter(|row| {
                row.status.as_deref() != Some("COMPLETED")
                    || !matches!(row.eval_outcome.as_deref(), Some("ALLOWED" | "BLOCKED"))
            })
            .count();
        if unsettled > 0 {
            self.finding(
                Priority::Review,
                "Scans without a completed decision",
                format!(
                    "{unsettled} collected non-parent rows are pending, failed or have an unfamiliar status."
                ),
                "Review scan status; absence of a completed decision is not a passing evaluation.",
                "Scans",
            );
        }
    }

    /// The Statistics analysis (TS `agentguard.ts:173-203`).
    fn stats(&mut self, source: &mut Source, stats: &Stats) {
        let metric = |name: &str, metric: &Metric| {
            vec![
                Cell::from(name),
                Cell::Number(metric.count),
                metric.percent_change.map_or(Cell::Null, Cell::Number),
            ]
        };
        self.table(
            "Skill statistics",
            "Server aggregates; percentage changes may be unavailable.".to_owned(),
            &["Metric", "Count", "Change (%)"],
            vec![
                metric("Unique skills scanned", &stats.unique_skills_scanned),
                metric("Vulnerabilities found", &stats.total_vulnerabilities_found),
            ],
        );
        let found = stats.total_vulnerabilities_found.count;
        if found > 0.0 {
            self.finding(
                Priority::Attention,
                "Skill vulnerabilities reported",
                format!(
                    "{} findings reported in the server statistics window.",
                    crate::render::js_number(found)
                ),
                "Review vulnerability details and remediate before use; findings are not a count of distinct vulnerable skills.",
                "Statistics",
            );
        }
        source.records = 1;
        source.pages = 1;
    }

    /// The Rules analysis (TS `agentguard.ts:204-224`).
    fn rules(&mut self, rows: &[RuleRow]) {
        let counted = groups(
            rows.iter()
                .map(|row| label(row.default_state.as_deref(), &STATES).to_owned()),
        );
        self.table(
            "Rule catalog defaults",
            "These are not effective tenant policy settings.".to_owned(),
            &["Default state", "Rules"],
            counted,
        );
    }
}

/// Collects the three sources (TS `collectAgentGuardReport`). A signal stops the report.
pub(crate) async fn collect(
    reader: &impl Reader,
    options: &Options<'_>,
) -> Result<Report, Interrupted> {
    let end = (options.now)();
    let start = end - WINDOW_MS;
    let (start_iso, end_iso) = (iso(start), iso(end));
    let mut c = Collector {
        report: Report {
            title: options.title.to_owned(),
            collection_started_at: end_iso.clone(),
            generated_at: String::new(),
            window_start: start_iso.clone(),
            window_end: end_iso.clone(),
            health: Health::Unknown,
            sources: Vec::new(),
            findings: Vec::new(),
            tables: Vec::new(),
        },
    };
    let max_pages = options.max_pages.clamp(1, 100);

    // 1. Scans.
    let mut s = source("Scans", "listScans()", &format!("{start_iso} to {end_iso}"));
    let read = pages(&mut s, max_pages, false, |skip| {
        reader.scans(skip, &start_iso, &end_iso)
    })
    .await;
    let result = read.map(|rows| c.scans(&mut s, &rows, start, end));
    c.finish(s, result)?;

    // 2. Statistics.
    let mut s = source("Statistics", "getScanStats()", "Server rolling 30_DAYS");
    let result = reader.stats().await.map(|stats| c.stats(&mut s, &stats));
    c.finish(s, result)?;

    // 3. Rules.
    let mut s = source("Rules", "listRules()", "Current catalog");
    let read = pages(&mut s, max_pages, true, |skip| reader.rules(skip)).await;
    let result = read.map(|rows| c.rules(&rows));
    c.finish(s, result)?;

    let report = &mut c.report;
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
    } else if report.findings.is_empty() {
        Health::NoFindings
    } else {
        Health::Review
    };
    report.generated_at = iso((options.now)());
    Ok(c.report)
}

#[cfg(test)]
#[path = "supplychain_tests.rs"]
mod tests;
