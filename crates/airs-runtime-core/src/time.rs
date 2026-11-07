//! UTC instants: JavaScript ISO text and epoch milliseconds (DESIGN §2.7).

const DAY_MS: i64 = 86_400_000;

/// `Date.prototype.toISOString` of epoch milliseconds (years 0000–9999; a time before
/// 1970 too, the gateway report's window).
pub fn iso(ms: i64) -> String {
    let days = ms.div_euclid(DAY_MS);
    let rest = ms.rem_euclid(DAY_MS);
    let (year, month, day) = crate::debug::civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3_600_000,
        rest / 60_000 % 60,
        rest / 1000 % 60,
        rest % 1000
    )
}

/// `Date.parse` of an ISO-8601 date-time with a `Z` or `±hh:mm` offset (the only form
/// the TS filter admits): `YYYY-MM-DDTHH:mm[:ss[.fff…]]`, then the zone. `None` for
/// anything else (TS `NaN`).
pub fn parse_iso(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| -> Option<i64> {
        let part = text.get(range)?;
        (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse().ok())
            .flatten()
    };
    if bytes.len() < 17 || bytes[4] != b'-' || bytes[7] != b'-' || !matches!(bytes[10], b'T' | b't')
    {
        return None;
    }
    let (year, month, day) = (digits(0..4)?, digits(5..7)?, digits(8..10)?);
    if bytes[13] != b':' {
        return None;
    }
    let (hour, minute) = (digits(11..13)?, digits(14..16)?);
    let mut index = 16;
    let mut second = 0;
    let mut millis = 0;
    if bytes.get(index) == Some(&b':') {
        second = digits(index + 1..index + 3)?;
        index += 3;
        if bytes.get(index) == Some(&b'.') {
            let start = index + 1;
            let mut end = start;
            while bytes.get(end).is_some_and(u8::is_ascii_digit) {
                end += 1;
            }
            if end == start {
                return None;
            }
            let fraction = &text[start..end];
            let padded = format!("{fraction:0<3}");
            millis = padded[..3].parse::<i64>().ok()?;
            index = end;
        }
    }
    let zone = &text[index..];
    let offset_minutes = match zone {
        "Z" | "z" => 0,
        _ => {
            let zb = zone.as_bytes();
            if zb.len() != 6 || !matches!(zb[0], b'+' | b'-') || zb[3] != b':' {
                return None;
            }
            let hours: i64 = zone[1..3].parse().ok()?;
            let minutes: i64 = zone[4..6].parse().ok()?;
            if !zone[1..3].bytes().all(|b| b.is_ascii_digit())
                || !zone[4..6].bytes().all(|b| b.is_ascii_digit())
                || hours > 23
                || minutes > 59
            {
                return None;
            }
            let sign = if zb[0] == b'-' { -1 } else { 1 };
            sign * (hours * 60 + minutes)
        }
    };
    let leap = |y: i64| (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days_in_month = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&month)
        || day < 1
        || day > days_in_month[usize::try_from(month - 1).ok()?]
        || hour > 24
        || minute > 59
        || second > 59
        || (hour == 24 && (minute != 0 || second != 0 || millis != 0))
    {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second - offset_minutes * 60;
    Some(seconds * 1000 + millis)
}
/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}
