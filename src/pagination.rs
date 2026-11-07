//! Management list/get flag validation, walk limits and page status lines
//! (DESIGN §7.1–§7.3). Numbers are decimal digits only (D17).

use crate::cli::{GetFlags, ListFlags};
use crate::error::CliError;
pub use airs_runtime_core::limits::MAX_SAFE_INTEGER;
use scm::pagination::{CollectOptions, Completion, Truncation};

pub const DEFAULT_LIMIT: u32 = 100;
pub const DEFAULT_MAX: u64 = 10_000;
/// The SDK's item ceiling; `--max 0` walks up to this many rows.
pub const MAX_ITEMS_CEILING: u64 = 1_000_000;
const MAX_PAGES: usize = 100_000;
const MAX_BYTES: usize = 512 * 1024 * 1024;
/// [`MAX_SAFE_INTEGER`] as an `f64` (exact: 2^53 − 1 is representable).
#[allow(clippy::cast_precision_loss)]
pub const MAX_SAFE_INTEGER_F64: f64 = MAX_SAFE_INTEGER as f64;

/// Validated list flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListParams {
    pub limit: u32,
    pub offset: u64,
    pub all: bool,
    /// `--max` as given; `0` means no item cap.
    pub max: u64,
    pub all_versions: bool,
}

/// Validated get flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetParams {
    pub name_or_id: String,
    pub revision: Option<u64>,
    pub all_versions: bool,
}

impl GetParams {
    /// Whether the lookup value is a UUID (matched case-insensitively).
    pub fn is_id(&self) -> bool {
        is_uuid(&self.name_or_id)
    }
}

/// A non-negative safe integer written with decimal digits only.
fn decimal(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse::<u64>().ok().filter(|n| *n <= MAX_SAFE_INTEGER)
}

/// Validate list flags in the TypeScript order: limit, offset, max, then `--all`.
pub fn parse_list(flags: &ListFlags, all_versions: bool) -> Result<ListParams, CliError> {
    let limit = match flags.limit.as_deref() {
        None => DEFAULT_LIMIT,
        Some(text) => decimal(text)
            .filter(|n| *n >= 1)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| CliError::usage("--limit must be a positive integer"))?,
    };
    let offset = match flags.offset.as_deref() {
        None => 0,
        Some(text) => decimal(text)
            .ok_or_else(|| CliError::usage("--offset must be a non-negative integer"))?,
    };
    let max = match flags.max.as_deref() {
        None => DEFAULT_MAX,
        Some(text) => {
            decimal(text).ok_or_else(|| CliError::usage("--max must be a non-negative integer"))?
        }
    };
    if flags.all && flags.offset.is_some() {
        return Err(CliError::usage("--all cannot be combined with --offset"));
    }
    Ok(ListParams {
        limit,
        offset,
        all: flags.all,
        max,
        all_versions,
    })
}

pub fn parse_get(flags: &GetFlags) -> Result<GetParams, CliError> {
    let revision = match flags.revision.as_deref() {
        None => None,
        Some(text) => Some(
            decimal(text)
                .ok_or_else(|| CliError::usage("--revision must be a non-negative integer"))?,
        ),
    };
    Ok(GetParams {
        name_or_id: flags.name_or_id.clone(),
        revision,
        all_versions: flags.all_versions,
    })
}

/// `^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$`, case-insensitive.
pub fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// Walk limits for `--all` listings: `--max` items (`0` → the SDK ceiling; larger
/// values are clamped to it), 100,000 pages, 512 MiB.
pub fn collect_options(max: u64) -> Result<CollectOptions, scm::Error> {
    let items = if max == 0 {
        MAX_ITEMS_CEILING
    } else {
        max.min(MAX_ITEMS_CEILING)
    };
    CollectOptions::default()
        .with_max_items(usize::try_from(items).unwrap_or(1_000_000))?
        .with_max_pages(MAX_PAGES)?
        .with_max_bytes(MAX_BYTES)
}

/// Single page: `Showing <n>` plus ` (more available)` when more rows exist.
pub fn showing_page(returned: usize, more: bool) -> String {
    if more {
        format!("Showing {returned} (more available)")
    } else {
        format!("Showing {returned}")
    }
}

/// `--all` walk that completed.
pub fn showing_all(returned: usize) -> String {
    format!("Showing all {returned}")
}

/// `--all` walk stopped by `--max`.
pub fn showing_first(returned: usize, max: u64) -> String {
    format!("Showing first {returned} (stopped at --max {max}; more available)")
}

/// Topics `--all-versions` page sliced locally from `total` rows.
pub fn showing_slice(returned: usize, total: usize, offset: u64, limit: u32) -> String {
    let next = offset.saturating_add(u64::from(limit));
    if next < total as u64 {
        format!("Showing {returned} of {total} (next --offset {next})")
    } else {
        format!("Showing {returned} of {total}")
    }
}

/// How an `--all` walk ended, once a safety-limit stop has been ruled out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkEnd {
    Complete,
    /// Stopped by the user's `--max` with more rows available.
    StoppedAtMax,
}

/// Classify an `--all` walk. Any truncation other than the user's own `--max` (a
/// page or byte bound, or the item ceiling behind `--max 0`) is an incomplete walk
/// and yields `None`: the caller fails rather than print a partial list.
pub fn walk_end(completion: Completion, max: u64) -> Option<WalkEnd> {
    match completion {
        Completion::Complete => Some(WalkEnd::Complete),
        Completion::Truncated(Truncation::Items) if max != 0 && max <= MAX_ITEMS_CEILING => {
            Some(WalkEnd::StoppedAtMax)
        }
        _ => None,
    }
}

/// The status line of an `--all` listing.
pub fn all_status(returned: usize, end: WalkEnd, max: u64) -> String {
    match end {
        WalkEnd::Complete => showing_all(returned),
        WalkEnd::StoppedAtMax => showing_first(returned, max),
    }
}

/// The error for a walk that had to be complete but was cut short by a client bound.
pub fn incomplete_walk() -> scm::Error {
    scm::Error::new(scm::ErrorKind::Limit)
}

/// The local page `[offset, offset + limit)`.
pub fn slice<T>(rows: Vec<T>, offset: u64, limit: u32) -> Vec<T> {
    let skip = usize::try_from(offset).unwrap_or(usize::MAX);
    let take = usize::try_from(limit).unwrap_or(usize::MAX);
    rows.into_iter().skip(skip).take(take).collect()
}

/// The first `max` rows (`0` keeps all) and how the listing ended.
pub fn first<T>(mut rows: Vec<T>, max: u64) -> (Vec<T>, WalkEnd) {
    let cap = usize::try_from(max).unwrap_or(usize::MAX);
    if max != 0 && rows.len() > cap {
        rows.truncate(cap);
        (rows, WalkEnd::StoppedAtMax)
    } else {
        (rows, WalkEnd::Complete)
    }
}

/// `--max-pages <n>` of the DLP and runtime profile backups and restores (TS): decimal
/// digits, 1–1000.
pub fn parse_max_pages(text: &str) -> Result<u64, CliError> {
    let invalid = || CliError::usage("--max-pages must be an integer from 1 to 1000");
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    let digits = text.trim_start_matches('0');
    if digits.len() > 4 {
        return Err(invalid());
    }
    let value: u64 = if digits.is_empty() {
        0
    } else {
        digits.parse().map_err(|_| invalid())?
    };
    if (1..=1000).contains(&value) {
        Ok(value)
    } else {
        Err(invalid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(limit: Option<&str>, offset: Option<&str>, all: bool, max: Option<&str>) -> ListFlags {
        ListFlags {
            limit: limit.map(str::to_owned),
            offset: offset.map(str::to_owned),
            all,
            max: max.map(str::to_owned),
        }
    }

    #[test]
    fn list_defaults() {
        assert_eq!(
            parse_list(&ListFlags::default(), false).unwrap(),
            ListParams {
                limit: 100,
                offset: 0,
                all: false,
                max: 10_000,
                all_versions: false
            }
        );
    }

    #[test]
    fn list_validation_messages_and_order() {
        let limit = CliError::usage("--limit must be a positive integer");
        let offset = CliError::usage("--offset must be a non-negative integer");
        let max = CliError::usage("--max must be a non-negative integer");
        let all = CliError::usage("--all cannot be combined with --offset");
        for value in [
            "0",
            "-1",
            "abc",
            "1.5",
            "1e2",
            "0x10",
            " 5",
            "+5",
            "",
            "9007199254740992",
        ] {
            assert_eq!(
                parse_list(&flags(Some(value), None, false, None), false),
                Err(limit.clone()),
                "{value}"
            );
        }
        assert_eq!(
            parse_list(&flags(Some("4294967296"), None, false, None), false),
            Err(limit.clone())
        );
        assert_eq!(
            parse_list(&flags(None, Some("-1"), false, None), false),
            Err(offset.clone())
        );
        assert_eq!(
            parse_list(&flags(None, None, false, Some("-1")), false),
            Err(max.clone())
        );
        assert_eq!(
            parse_list(&flags(None, Some("0"), true, None), false),
            Err(all.clone())
        );
        assert_eq!(
            parse_list(&flags(Some("0"), Some("x"), true, Some("y")), false),
            Err(limit)
        );
        assert_eq!(
            parse_list(&flags(None, Some("x"), true, Some("y")), false),
            Err(offset)
        );
        assert_eq!(
            parse_list(&flags(None, Some("1"), true, Some("y")), false),
            Err(max)
        );
        let ok = parse_list(
            &flags(Some("007"), Some("9007199254740991"), false, Some("0")),
            true,
        )
        .unwrap();
        assert_eq!(
            (ok.limit, ok.offset, ok.max, ok.all_versions),
            (7, 9_007_199_254_740_991, 0, true)
        );
        assert!(
            parse_list(&flags(None, None, true, Some("5")), false)
                .unwrap()
                .all
        );
    }

    #[test]
    fn get_validation() {
        let base = GetFlags {
            name_or_id: "prof".into(),
            revision: None,
            all_versions: false,
        };
        assert_eq!(parse_get(&base).unwrap().revision, None);
        for value in ["abc", "-1", "1.0", ""] {
            let flags = GetFlags {
                revision: Some(value.into()),
                ..base.clone()
            };
            assert_eq!(
                parse_get(&flags),
                Err(CliError::usage("--revision must be a non-negative integer"))
            );
        }
        let flags = GetFlags {
            revision: Some("3".into()),
            ..base
        };
        assert_eq!(parse_get(&flags).unwrap().revision, Some(3));
    }

    #[test]
    fn uuid_detection() {
        assert!(is_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(is_uuid("550E8400-E29B-41D4-A716-446655440000"));
        assert!(!is_uuid("550e8400e29b41d4a716446655440000"));
        assert!(!is_uuid("550e8400-e29b-41d4-a716-44665544000g"));
        assert!(!is_uuid("prod-guard"));
    }

    #[test]
    fn collect_limits() {
        assert_eq!(collect_options(0).unwrap().max_items, 1_000_000);
        assert_eq!(collect_options(5).unwrap().max_items, 5);
        assert_eq!(collect_options(5_000_000).unwrap().max_items, 1_000_000);
        assert_eq!(collect_options(1).unwrap().max_pages, 100_000);
    }

    #[test]
    fn status_strings() {
        assert_eq!(showing_page(3, false), "Showing 3");
        assert_eq!(showing_page(3, true), "Showing 3 (more available)");
        assert_eq!(showing_all(7), "Showing all 7");
        assert_eq!(
            showing_first(2, 2),
            "Showing first 2 (stopped at --max 2; more available)"
        );
        assert_eq!(
            showing_slice(1, 3, 1, 1),
            "Showing 1 of 3 (next --offset 2)"
        );
        assert_eq!(showing_slice(1, 3, 2, 1), "Showing 1 of 3");
    }

    #[test]
    fn explicit_offset_zero_conflicts_with_all_but_default_does_not() {
        assert_eq!(
            parse_list(&flags(None, Some("0"), true, None), true),
            Err(CliError::usage("--all cannot be combined with --offset"))
        );
        assert!(parse_list(&flags(None, None, true, None), true).is_ok());
        // --max is validated even without --all.
        assert_eq!(
            parse_list(&flags(None, None, false, Some("x")), false),
            Err(CliError::usage("--max must be a non-negative integer"))
        );
    }

    #[test]
    fn walk_end_classification() {
        assert_eq!(walk_end(Completion::Complete, 0), Some(WalkEnd::Complete));
        assert_eq!(
            walk_end(Completion::Truncated(Truncation::Items), 2),
            Some(WalkEnd::StoppedAtMax)
        );
        assert_eq!(
            walk_end(Completion::Truncated(Truncation::Items), MAX_ITEMS_CEILING),
            Some(WalkEnd::StoppedAtMax)
        );
        // The ceiling behind --max 0 or a clamped --max is a safety limit, not --max.
        assert_eq!(walk_end(Completion::Truncated(Truncation::Items), 0), None);
        assert_eq!(
            walk_end(
                Completion::Truncated(Truncation::Items),
                MAX_ITEMS_CEILING + 1
            ),
            None
        );
        assert_eq!(walk_end(Completion::Truncated(Truncation::Pages), 5), None);
        assert_eq!(walk_end(Completion::Truncated(Truncation::Bytes), 5), None);
        assert_eq!(walk_end(Completion::Interrupted, 5), None);
        assert_eq!(incomplete_walk().kind(), scm::ErrorKind::Limit);
    }

    #[test]
    fn every_status_line() {
        // Single page (profiles, topics latest page).
        assert_eq!(showing_page(0, false), "Showing 0");
        assert_eq!(showing_page(2, true), "Showing 2 (more available)");
        // --all complete / stopped by --max.
        assert_eq!(all_status(4, WalkEnd::Complete, 10_000), "Showing all 4");
        assert_eq!(all_status(0, WalkEnd::Complete, 0), "Showing all 0");
        assert_eq!(
            all_status(2, WalkEnd::StoppedAtMax, 2),
            "Showing first 2 (stopped at --max 2; more available)"
        );
        // Topics --all-versions page.
        assert_eq!(
            showing_slice(2, 5, 0, 2),
            "Showing 2 of 5 (next --offset 2)"
        );
        assert_eq!(showing_slice(1, 5, 4, 2), "Showing 1 of 5");
        assert_eq!(showing_slice(1, 3, 2, 1), "Showing 1 of 3");
        assert_eq!(showing_slice(0, 3, 7, 1), "Showing 0 of 3");
        assert_eq!(showing_slice(0, 0, 0, 100), "Showing 0 of 0");
        assert_eq!(
            showing_slice(1, 3, u64::MAX - 1, u32::MAX),
            "Showing 1 of 3"
        );
    }

    #[test]
    fn local_slicing() {
        let rows: Vec<u32> = (0..5).collect();
        assert_eq!(slice(rows.clone(), 0, 2), [0, 1]);
        assert_eq!(slice(rows.clone(), 3, 100), [3, 4]);
        assert_eq!(slice(rows.clone(), 5, 1), Vec::<u32>::new());
        assert_eq!(
            slice(rows.clone(), 9_007_199_254_740_991, 1),
            Vec::<u32>::new()
        );
        assert_eq!(first(rows.clone(), 0), (rows.clone(), WalkEnd::Complete));
        assert_eq!(first(rows.clone(), 5), (rows.clone(), WalkEnd::Complete));
        assert_eq!(first(rows.clone(), 2), (vec![0, 1], WalkEnd::StoppedAtMax));
        assert_eq!(first(Vec::<u32>::new(), 1), (vec![], WalkEnd::Complete));
    }
}
