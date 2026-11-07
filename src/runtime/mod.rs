//! M6d runtime extras (DESIGN §18): the shared argument checks of §18.3, the request
//! options helpers (every call without the frozen response defaults, G27), the dashboard
//! G7 re-read and the dashboard failure line.
//!
//! The checks run during planning, before the tenant is read (D220), with the TypeScript
//! texts where TypeScript has a check and the §18.3 texts where the port adds one.

pub mod consumption;
pub mod keys;
pub mod list;
pub use airs_runtime_core::scan_logs;
pub mod transfer;
pub mod views;

use crate::error::{CliError, InterruptContext};
use crate::output::{Output, terminal_safe};
use crate::pagination::MAX_SAFE_INTEGER;
use crate::render::style::Style;
use crate::tenant::config::js_trim;
use std::time::Duration;

/// The SDK query-value bound (bytes).
const TEXT_LIMIT: usize = 4096;

/// The units of `dashboard applications` (TS).
pub const APPLICATION_UNITS: &[&str] = &["day", "days", "hour"];
/// The units of `dashboard application` and `application-violations` (TS).
pub const DETAIL_UNITS: &[&str] = &["days"];
/// The units of every other dashboard and sessions command (TS `TIME_UNITS`).
pub const TIME_UNITS: &[&str] = &["hour", "hours", "day", "days"];

/// [`request_options`] with a [`CliError`]: the options of an M6d read.
pub fn options(budget: Duration) -> Result<scm::RequestOptions, CliError> {
    request_options(budget).map_err(|_| options_failed())
}

/// The one builder of every M6d request's options, read or write attempt:
/// `with_timeout(budget)` without the frozen response defaults (G27); the SDK retries a
/// read (D10), and a write is one attempt with the single 401 replay (D95). The SDK deadline is
/// absolute from this call, so a caller builds the options of each request just before
/// sending it (§18.3: the budget is per request). The SDK error is kept for a caller
/// inside an SDK-error flow.
pub fn request_options(budget: Duration) -> scm::Result<scm::RequestOptions> {
    scm::RequestOptions::with_timeout(budget).map(scm::RequestOptions::without_response_defaults)
}

/// The dashboard and sessions options (§18.3): `with_retries(0)` (TS `numRetries: 0`)
/// without the response defaults; the G7 re-read is [`reread`].
pub fn dashboard_options(budget: Duration) -> Result<scm::RequestOptions, CliError> {
    dashboard_read_options(budget).map_err(|_| options_failed())
}

/// [`dashboard_options`] with the SDK error (the runtime report's per-source budget).
pub fn dashboard_read_options(budget: Duration) -> scm::Result<scm::RequestOptions> {
    scm::RequestOptions::with_timeout(budget)
        .and_then(|options| options.with_retries(0))
        .map(scm::RequestOptions::without_response_defaults)
}

fn options_failed() -> CliError {
    CliError::operational("AISEC_SDK_ERROR:The request options could not be configured")
}

/// The G7 re-read of the dashboard and sessions reads (`clients::reread`).
pub use crate::clients::reread;

/// TypeScript's only dashboard failure text (`rd.ts:144-151`): no server text, path or
/// SDK message; ` (HTTP <s>)` for a status 400–599.
pub fn dashboard_failure_text(status: Option<u16>) -> String {
    let status = status
        .filter(|status| (400..=599).contains(status))
        .map_or_else(String::new, |status| format!(" (HTTP {status})"));
    format!(
        "✗ Dashboard operation failed{status}. Check credentials, query options, and any output destination. No existing file is overwritten."
    )
}

/// A dashboard or sessions failure after the client was built (§18.12 "Failures"): a
/// cancellation stays an interruption; anything else prints the TypeScript line on stderr
/// (red, never suppressed) and exits 1.
pub fn dashboard_failure(output: &mut Output, error: &scm::Error) -> CliError {
    if error.kind() == scm::ErrorKind::Cancelled {
        return CliError::Interrupted(InterruptContext::NoDataWritten);
    }
    let line = output.paint_err(Style::Red, &dashboard_failure_text(error.status()));
    output.write_stderr(&format!("  {line}\n"));
    CliError::Reported(crate::error::EXIT_OPERATIONAL)
}

/// `^[0-9]+$` as a safe JavaScript integer (leading zeros accepted), else `None`: the one
/// digit rule of every integer flag of §18.3.
fn digits_to_safe_u64(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let digits = value.trim_start_matches('0');
    if digits.is_empty() {
        return Some(0);
    }
    // 2^53 − 1 has 16 digits: a longer value is not safe and must not overflow `u64`.
    if digits.len() > 16 {
        return None;
    }
    digits
        .parse::<u64>()
        .ok()
        .filter(|number| *number <= MAX_SAFE_INTEGER)
}

/// `^[0-9]+$` and a safe integer of at least `minimum` (TS `integer`, `rd.ts:16-21`;
/// leading zeros accepted), else `<flag> must be a safe integer >= <minimum>`.
pub fn dash_integer(value: &str, flag: &str, minimum: u64) -> Result<u64, CliError> {
    digits_to_safe_u64(value)
        .filter(|number| *number >= minimum)
        .ok_or_else(|| CliError::usage(format!("{flag} must be a safe integer >= {minimum}")))
}

/// A text flag that is sent (TS `text`, `rd.ts:23-26`): non-blank under JavaScript trim,
/// else `<flag> is required`; then the query-value bound. The value is sent untrimmed.
pub fn dash_text(value: &str, flag: &str) -> Result<String, CliError> {
    if js_trim(value).is_empty() {
        return Err(CliError::usage(format!("{flag} is required")));
    }
    check_text(flag, value)?;
    Ok(value.to_owned())
}

/// The query-value bound of every sent free-text value (§18.3): at most 4,096 bytes and no
/// control character.
pub fn check_text(flag: &str, value: &str) -> Result<(), CliError> {
    if value.len() <= TEXT_LIMIT && !value.chars().any(char::is_control) {
        Ok(())
    } else {
        Err(CliError::usage(format!(
            "{flag} must be at most {TEXT_LIMIT} bytes and contain no control characters"
        )))
    }
}

/// `--unit` against the command's vocabulary (TS parse-time check, run first): exact and
/// case-sensitive, else the TypeScript message (the value is not echoed).
pub fn check_unit(value: Option<&str>, units: &[&str]) -> Result<(), CliError> {
    match value {
        Some(unit) if !units.contains(&unit) => Err(CliError::usage(format!(
            "Unsupported --unit. Supported units: {}. For one week, use --interval 7 --unit days.",
            units.join(", ")
        ))),
        _ => Ok(()),
    }
}

/// A sent name or id that must not be blank (D220, choice 33): `message` when it is.
pub fn non_blank(value: &str, message: &str, flag: &str) -> Result<String, CliError> {
    if js_trim(value).is_empty() {
        return Err(CliError::usage(message.to_owned()));
    }
    check_text(flag, value)?;
    Ok(value.to_owned())
}

/// `consumption --time-interval` (§18.9): decimal digits only, 7, 30 or 60 (TS text).
pub fn time_interval(value: Option<&str>) -> Result<u64, CliError> {
    digits_to_safe_u64(value.unwrap_or("30"))
        .filter(|interval| [7, 30, 60].contains(interval))
        .ok_or_else(|| {
            CliError::usage("--time-interval must be 7, 30, or 60 (the API rejects other values)")
        })
}

/// `regenerate --interval` (§18.5): decimal digits only, 1 to 2^53 − 1, else
/// `--interval must be a positive integer` (TS `parseInt` takes `1.9` and sends `-5`).
pub fn rotation_interval(value: &str) -> Result<u64, CliError> {
    digits_to_safe_u64(value)
        .filter(|number| *number >= 1)
        .ok_or_else(|| CliError::usage("--interval must be a positive integer"))
}

/// `--updated-by` when given (D220, choice 33): non-blank, within the query bound.
pub fn updated_by(value: &str) -> Result<String, CliError> {
    non_blank(value, "--updated-by must not be empty", "--updated-by")
}

/// A server string for a message or a layout: terminal-safe, `[withheld]` when it carries
/// a secret of the invocation (§18.3 "Server text").
pub fn shown(text: &str, secrets: &crate::error::SecretSet) -> String {
    terminal_safe(secrets.scrub(text)).into_owned()
}

/// The evidence budget flags (`--timeout`, `--concurrency`, `--max-requests`, DESIGN §17).
pub fn evidence_limits(
    args: &crate::cli::EvidenceBudgetArgs,
) -> Result<scan_logs::Limits, CliError> {
    scan_logs::Limits::parse(
        args.timeout.as_deref(),
        args.concurrency.as_deref(),
        args.max_requests.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(result: Result<impl std::fmt::Debug, CliError>) -> String {
        match result {
            Err(CliError::Usage(message)) => message,
            other => panic!("expected a usage error, got {other:?}"),
        }
    }

    /// TS `integer` (`rd.ts:16-21`, captured in TR§3.1): digits only, a safe integer of at
    /// least the minimum, leading zeros accepted.
    #[test]
    fn dashboard_integers_follow_the_typescript_rule() {
        assert_eq!(dash_integer("01", "--interval", 1).unwrap(), 1);
        assert_eq!(dash_integer("0", "--offset", 0).unwrap(), 0);
        assert_eq!(dash_integer("000", "--offset", 0).unwrap(), 0);
        assert_eq!(
            dash_integer("9007199254740991", "--offset", 0).unwrap(),
            9_007_199_254_740_991
        );
        assert_eq!(
            dash_integer("00000000000000000000001", "--offset", 0).unwrap(),
            1
        );
        for bad in [
            "",
            "+1",
            " 1",
            "1e3",
            "1.5",
            "-1",
            "NaN",
            "1oops",
            "0x1",
            "9007199254740992",
            "99999999999999999999",
        ] {
            assert_eq!(
                usage(dash_integer(bad, "--interval", 1)),
                "--interval must be a safe integer >= 1",
                "{bad:?}"
            );
        }
        assert_eq!(
            usage(dash_integer("0", "--limit", 1)),
            "--limit must be a safe integer >= 1"
        );
    }

    #[test]
    fn text_units_and_names() {
        assert_eq!(
            dash_text(" a ", "--app-id").unwrap(),
            " a ",
            "sent untrimmed"
        );
        assert_eq!(usage(dash_text(" \t", "--app-id")), "--app-id is required");
        assert_eq!(
            usage(dash_text("a\u{1}", "--scan-id")),
            "--scan-id must be at most 4096 bytes and contain no control characters"
        );
        assert_eq!(
            usage(dash_text(&"x".repeat(4097), "--app-name")),
            "--app-name must be at most 4096 bytes and contain no control characters"
        );
        assert!(check_unit(Some("day"), TIME_UNITS).is_ok());
        assert!(check_unit(None, DETAIL_UNITS).is_ok());
        for unit in [
            "week",
            "weeks",
            "month",
            "months",
            "year",
            "years",
            "fortnight",
            "DAY",
            " day ",
        ] {
            assert_eq!(
                usage(check_unit(Some(unit), TIME_UNITS)),
                "Unsupported --unit. Supported units: hour, hours, day, days. For one week, use --interval 7 --unit days.",
                "{unit:?}"
            );
        }
        assert_eq!(
            usage(check_unit(Some("hours"), APPLICATION_UNITS)),
            "Unsupported --unit. Supported units: day, days, hour. For one week, use --interval 7 --unit days."
        );
        assert_eq!(
            usage(non_blank(
                "\u{a0}",
                "Customer app name must not be empty",
                "appName"
            )),
            "Customer app name must not be empty"
        );
    }

    /// The one digit rule behind every integer flag: digits only, leading zeros accepted,
    /// at most 2^53 − 1 (a longer value never overflows).
    #[test]
    fn digits_are_safe_integers() {
        assert_eq!(digits_to_safe_u64("0"), Some(0));
        assert_eq!(digits_to_safe_u64("000"), Some(0));
        assert_eq!(digits_to_safe_u64("0007"), Some(7));
        assert_eq!(
            digits_to_safe_u64("0009007199254740991"),
            Some(MAX_SAFE_INTEGER)
        );
        for bad in [
            "",
            "+1",
            "-1",
            " 1",
            "1 ",
            "1.0",
            "1e3",
            "٣",
            "9007199254740992",
            "9999999999999999",
            "18446744073709551616",
        ] {
            assert_eq!(digits_to_safe_u64(bad), None, "{bad:?}");
        }
    }

    /// `regenerate --interval` (§18.5, D220): TS `parseInt` takes `1.9` and sends `-5`.
    #[test]
    fn rotation_intervals_are_strict() {
        assert_eq!(rotation_interval("30").unwrap(), 30);
        assert_eq!(rotation_interval("007").unwrap(), 7);
        assert_eq!(
            rotation_interval("9007199254740991").unwrap(),
            9_007_199_254_740_991
        );
        for bad in [
            "",
            "0",
            "000",
            "-5",
            "1.9",
            "abc",
            " 1",
            "9007199254740992",
            "1e3",
        ] {
            assert_eq!(
                usage(rotation_interval(bad)),
                "--interval must be a positive integer",
                "{bad:?}"
            );
        }
        assert_eq!(usage(updated_by(" ")), "--updated-by must not be empty");
        assert_eq!(
            usage(updated_by("a\u{7}")),
            "--updated-by must be at most 4096 bytes and contain no control characters"
        );
    }

    /// `--time-interval` (§18.9, D220): TS takes `7.5` and `60abc` through `parseInt`.
    #[test]
    fn time_intervals_are_strict() {
        assert_eq!(time_interval(None).unwrap(), 30);
        assert_eq!(time_interval(Some("7")).unwrap(), 7);
        assert_eq!(time_interval(Some("060")).unwrap(), 60);
        for bad in ["14", "7.5", "60abc", "", "-7", "0", " 30"] {
            assert_eq!(
                usage(time_interval(Some(bad))),
                "--time-interval must be 7, 30, or 60 (the API rejects other values)",
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_failure_line_names_only_an_http_status() {
        assert_eq!(
            dashboard_failure_text(Some(403)),
            "✗ Dashboard operation failed (HTTP 403). Check credentials, query options, and any output destination. No existing file is overwritten."
        );
        assert_eq!(
            dashboard_failure_text(None),
            "✗ Dashboard operation failed. Check credentials, query options, and any output destination. No existing file is overwritten."
        );
        assert!(!dashboard_failure_text(Some(302)).contains("HTTP"));
    }
}
