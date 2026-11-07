//! Contract-level scheduling rules (docs/RUNTIME_MCP_CONTRACT.md §8), proven on the
//! pinned timezone dependency. M6 implements the production scheduler from these.

use jiff::{Timestamp, ToSpan, civil::Date, tz::TimeZone};

/// The fixed reporting timezone (plan: `00:00 America/Chicago`, CST/CDT).
pub const REPORT_TZ: &str = "America/Chicago";
/// Default automatic revision interval and allowance (+4 … +24, six revisions).
pub const DEFAULT_REVISION_INTERVAL_HOURS: u32 = 4;
pub const DEFAULT_REVISION_ALLOWANCE: u32 = 6;
/// Compiled ceilings (plan §Collection and daily report scheduling).
pub const MAX_REVISION_ALLOWANCE: u32 = 24;
pub const MIN_REVISION_INTERVAL_HOURS: u32 = 1;
pub const WINDOW_HOURS: u32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleError {
    TimeZone,
    Arithmetic,
    IntervalOutOfRange,
    AllowanceOutOfRange,
}

pub fn report_tz() -> Result<TimeZone, ScheduleError> {
    TimeZone::get(REPORT_TZ).map_err(|_| ScheduleError::TimeZone)
}

/// Half-open UTC bounds `[start, end)` of one local calendar day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayBounds {
    pub start: Timestamp,
    pub end: Timestamp,
}

impl DayBounds {
    pub fn hours(&self) -> i64 {
        (self.end.as_second() - self.start.as_second()) / 3600
    }
}

/// Bounds of `day` in `tz`. A local day may be 23, 24 or 25 hours long.
pub fn day_bounds(day: Date, tz: &TimeZone) -> Result<DayBounds, ScheduleError> {
    let start = day
        .to_zoned(tz.clone())
        .map_err(|_| ScheduleError::Arithmetic)?;
    let next = day
        .checked_add(1.day())
        .map_err(|_| ScheduleError::Arithmetic)?
        .to_zoned(tz.clone())
        .map_err(|_| ScheduleError::Arithmetic)?;
    Ok(DayBounds {
        start: start.timestamp(),
        end: next.timestamp(),
    })
}

/// The report timezone's UTC offset over `[since_ms, until_ms)`, as segments
/// `(segment_end_ms, offset_ms)` in order, the last ending at `until_ms`. America/Chicago
/// changes offset only on whole UTC hours, so sampling each hour start is exact; a
/// period of at most 90 days has at most four segments. Used to place events on local
/// report dates without a timezone database in SQL.
pub fn report_offset_segments(
    since_ms: i64,
    until_ms: i64,
) -> Result<Vec<(i64, i64)>, ScheduleError> {
    let tz = report_tz()?;
    let offset = |ms: i64| -> Result<i64, ScheduleError> {
        let at = Timestamp::from_millisecond(ms).map_err(|_| ScheduleError::Arithmetic)?;
        Ok(i64::from(tz.to_offset(at).seconds()) * 1000)
    };
    let mut segments: Vec<(i64, i64)> = Vec::new();
    let mut at = since_ms.div_euclid(3_600_000) * 3_600_000;
    while at < until_ms {
        let next = (at + 3_600_000).min(until_ms);
        let o = offset(at.max(since_ms))?;
        match segments.last_mut() {
            Some(last) if last.1 == o => last.0 = next,
            _ => segments.push((next, o)),
        }
        at += 3_600_000;
    }
    Ok(segments)
}

/// The report-timezone calendar date (`YYYY-MM-DD`) of an instant.
pub fn report_date(ms: i64) -> Result<String, ScheduleError> {
    let at = Timestamp::from_millisecond(ms).map_err(|_| ScheduleError::Arithmetic)?;
    Ok(at.to_zoned(report_tz()?).date().to_string())
}

/// The daily slot fired at local midnight starting `day` reports the previous day.
pub fn report_period_for_midnight(day: Date, tz: &TimeZone) -> Result<DayBounds, ScheduleError> {
    let previous = day
        .checked_sub(1.day())
        .map_err(|_| ScheduleError::Arithmetic)?;
    day_bounds(previous, tz)
}

/// Revision opportunities, in elapsed hours after scheduled midnight: every
/// integer multiple of `interval` strictly below +24, then a final +24 slot. A
/// non-dividing interval never postpones the final slot.
pub fn revision_opportunities(interval_hours: u32) -> Result<Vec<u32>, ScheduleError> {
    if !(MIN_REVISION_INTERVAL_HOURS..=WINDOW_HOURS).contains(&interval_hours) {
        return Err(ScheduleError::IntervalOutOfRange);
    }
    let mut slots: Vec<u32> = (1..)
        .map(|k| k * interval_hours)
        .take_while(|h| *h < WINDOW_HOURS)
        .collect();
    slots.push(WINDOW_HOURS);
    Ok(slots)
}

/// Whether a changed-evidence revision may be created at an opportunity, given how
/// many automatic revisions are already published. The final allowance is always
/// reserved for the closing (+24) snapshot, including when `allowance` is smaller
/// than the opportunity count.
pub fn may_revise(is_final: bool, used: u32, allowance: u32) -> Result<bool, ScheduleError> {
    if !(1..=MAX_REVISION_ALLOWANCE).contains(&allowance) {
        return Err(ScheduleError::AllowanceOutOfRange);
    }
    Ok(if is_final {
        used < allowance
    } else {
        used + 1 < allowance
    })
}

/// Coalesce delayed opportunities: given the elapsed hour at which the worker
/// actually runs, return the one opportunity to process (the latest one due), or
/// `None` before the first. The evidence cutoff of the final slot stays at +24
/// even when processed later; the caller filters committed evidence by that cutoff.
pub fn due_opportunity(opportunities: &[u32], elapsed_hours: u32) -> Option<u32> {
    opportunities
        .iter()
        .copied()
        .filter(|h| *h <= elapsed_hours)
        .max()
}

/// Shared Reports allowance day key with a stored high-water guard: the admission
/// day is the America/Chicago date of `max(now, high_water)`, so a wall-clock
/// rollback or timezone/policy change cannot reopen an earlier day's allowance.
pub fn admission_day(
    now: Timestamp,
    high_water: Option<Timestamp>,
    tz: &TimeZone,
) -> (Date, Timestamp) {
    let effective = match high_water {
        Some(h) if h > now => h,
        _ => now,
    };
    (effective.to_zoned(tz.clone()).date(), effective)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn tz() -> TimeZone {
        report_tz().expect("embedded tzdb has America/Chicago")
    }

    #[test]
    fn offset_segments_place_every_hour_of_dst_days_on_its_local_date() {
        for day in [date(2026, 3, 8), date(2026, 11, 1), date(2026, 7, 4)] {
            let b = day_bounds(day, &tz()).unwrap();
            let (since, until) = (b.start.as_millisecond(), b.end.as_millisecond());
            let segments = report_offset_segments(since, until).unwrap();
            assert_eq!(segments.last().unwrap().0, until);
            let mut start = since;
            for (end, offset) in &segments {
                let mut at = start;
                while at < *end {
                    let sql_like = jiff::Timestamp::from_millisecond(at + offset)
                        .unwrap()
                        .to_zoned(TimeZone::UTC)
                        .date();
                    assert_eq!(sql_like, day, "{at}");
                    assert_eq!(report_date(at).unwrap(), day.to_string());
                    at += 600_000;
                }
                start = *end;
            }
        }
        // 90 days crossing both 2026 transitions: three segments.
        let since = day_bounds(date(2026, 2, 1), &tz())
            .unwrap()
            .start
            .as_millisecond();
        let until = day_bounds(date(2026, 11, 30), &tz())
            .unwrap()
            .start
            .as_millisecond();
        assert_eq!(report_offset_segments(since, until).unwrap().len(), 3);
    }

    #[test]
    fn dst_days_have_23_24_25_hours_and_half_open_utc_bounds() {
        // 2026: spring forward 8 March, fall back 1 November.
        let spring = day_bounds(date(2026, 3, 8), &tz()).unwrap();
        assert_eq!(spring.hours(), 23);
        assert_eq!(spring.start.to_string(), "2026-03-08T06:00:00Z");
        assert_eq!(spring.end.to_string(), "2026-03-09T05:00:00Z");
        let fall = day_bounds(date(2026, 11, 1), &tz()).unwrap();
        assert_eq!(fall.hours(), 25);
        assert_eq!(fall.start.to_string(), "2026-11-01T05:00:00Z");
        assert_eq!(fall.end.to_string(), "2026-11-02T06:00:00Z");
        let plain = day_bounds(date(2026, 7, 4), &tz()).unwrap();
        assert_eq!(plain.hours(), 24);
        // Consecutive days tile exactly: end of one is the start of the next.
        let next = day_bounds(date(2026, 3, 9), &tz()).unwrap();
        assert_eq!(spring.end, next.start);
    }

    #[test]
    fn midnight_slot_reports_the_previous_local_day() {
        let p = report_period_for_midnight(date(2026, 11, 2), &tz()).unwrap();
        assert_eq!(p, day_bounds(date(2026, 11, 1), &tz()).unwrap());
        assert_eq!(p.hours(), 25);
    }

    #[test]
    fn revision_anchor_rules() {
        assert_eq!(
            revision_opportunities(4).unwrap(),
            vec![4, 8, 12, 16, 20, 24]
        );
        assert_eq!(revision_opportunities(7).unwrap(), vec![7, 14, 21, 24]);
        assert_eq!(revision_opportunities(1).unwrap().len(), 24);
        assert_eq!(revision_opportunities(24).unwrap(), vec![24]);
        assert_eq!(
            revision_opportunities(0),
            Err(ScheduleError::IntervalOutOfRange)
        );
        assert_eq!(
            revision_opportunities(25),
            Err(ScheduleError::IntervalOutOfRange)
        );
    }

    #[test]
    fn final_allowance_is_reserved_for_the_closing_snapshot() {
        // Allowance 2 with six opportunities: one early revision, final reserved.
        assert!(may_revise(false, 0, 2).unwrap());
        assert!(!may_revise(false, 1, 2).unwrap());
        assert!(may_revise(true, 1, 2).unwrap());
        assert!(!may_revise(true, 2, 2).unwrap());
        // Allowance 1: no early revisions at all.
        assert!(!may_revise(false, 0, 1).unwrap());
        assert!(may_revise(true, 0, 1).unwrap());
        assert_eq!(
            may_revise(true, 0, 25),
            Err(ScheduleError::AllowanceOutOfRange)
        );
    }

    #[test]
    fn delayed_opportunities_coalesce_without_moving_the_cutoff() {
        let slots = revision_opportunities(4).unwrap();
        assert_eq!(due_opportunity(&slots, 3), None);
        assert_eq!(due_opportunity(&slots, 13), Some(12)); // +4, +8, +12 coalesce
        assert_eq!(due_opportunity(&slots, 30), Some(24)); // late final keeps +24 cutoff
    }

    #[test]
    fn allowance_day_cannot_be_reopened_by_clock_rollback() {
        let late: Timestamp = "2026-11-04T05:30:00Z".parse().unwrap(); // 23:30 CST 3 Nov
        let next: Timestamp = "2026-11-04T06:10:00Z".parse().unwrap(); // 00:10 CST 4 Nov
        let (d1, hw) = admission_day(next, None, &tz());
        assert_eq!(d1, date(2026, 11, 4));
        // Clock steps back before midnight: still charged to 4 November.
        let (d2, _) = admission_day(late, Some(hw), &tz());
        assert_eq!(d2, date(2026, 11, 4));
    }
}
