//! Shared gateway rules (DESIGN §19.3): the argument checks with the TypeScript texts,
//! the planes, the options of every SDK call (without the frozen response defaults, built
//! just before each request), the 403 grant hint (G42, D234) and the banner.
//!
//! Every check here is pure and runs during planning, before the tenant is read (D232).

pub mod body;
pub mod inference;
pub mod inventory;
pub mod provider;
pub mod provision;
pub mod resolve;
pub mod secrets;
pub mod views;
pub mod window;
/// The withholding traversal and its rules (DESIGN §19.3) live in airs-runtime-core.
pub use airs_runtime_core::withhold;
#[cfg(test)]
mod withhold_printed;
pub mod writes;

use crate::error::CliError;
use crate::output::terminal_safe;
use std::time::Duration;

/// The gateway banner (TS `renderAiGatewayHeader`; product text, D227).
pub const BANNER_TITLE: &str = "Prisma AIRS — AI Gateway";
pub const BANNER_SUBTITLE: &str = "Gateway workspace operations";

/// The largest safe JavaScript integer.
pub const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// The SDK query-value bound (bytes) of resolved references and trace ids.
pub const TEXT_LIMIT: usize = 4096;

/// The longest user value a message quotes (characters, then `…`).
const SHOWN_LIMIT: usize = 200;

/// The plane a gateway request goes to (§19.3 "Clients and planes").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GwPlane {
    Data,
    Admin,
    Iam,
}

impl GwPlane {
    /// The SDK's plane of a workspace read (`None` for IAM, which has no workspaces).
    pub fn sdk(self) -> scm::airs::gateway::AIGatewayPlane {
        match self {
            Self::Admin => scm::airs::gateway::AIGatewayPlane::Admin,
            Self::Data | Self::Iam => scm::airs::gateway::AIGatewayPlane::Data,
        }
    }

    /// `data` or `admin` (the `--plane` vocabulary and the resolution messages).
    pub fn name(self) -> &'static str {
        match self {
            Self::Data => "data",
            Self::Admin => "admin",
            Self::Iam => "IAM",
        }
    }
}

/// The options of every gateway SDK call (choice 2): `with_timeout(budget)` without the
/// frozen response defaults. The SDK deadline is absolute from this call, so a caller
/// builds the options of each request just before sending it.
pub fn options(budget: Duration) -> Result<scm::RequestOptions, CliError> {
    scm::RequestOptions::with_timeout(budget)
        .map(scm::RequestOptions::without_response_defaults)
        .map_err(|_| {
            CliError::operational("AISEC_SDK_ERROR:The request options could not be configured")
        })
}

/// How a list answer says it is incomplete (G39, D248).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Incomplete {
    /// Its `total` (a safe non-negative integer) is this, not the number of rows received.
    Of(u64),
    /// Only `has_more: true` says so (no `total`, or one equal to the rows).
    MoreThanRows,
}

/// The one G39 rule (D248) of every gateway list: `rows` received, the answer's `total`
/// (or the IAM scopes' `count`) and its `has_more`. A `total` that is not a safe
/// non-negative integer is ignored. `None` when the answer says nothing is missing. The
/// incomplete-list warnings ([`crate::commands::gateway::incomplete_warning`]) and the
/// workspace resolution ([`resolve::incomplete`], D254) both read it.
pub fn incomplete_total(
    rows: usize,
    total: Option<&crate::json::OValue>,
    has_more: bool,
) -> Option<Incomplete> {
    let total = total.and_then(|value| match value {
        crate::json::OValue::Number(number) => number.as_f64(),
        _ => None,
    });
    match crate::walk::total_of(total) {
        Some(total) if usize::try_from(total).ok() != Some(rows) => Some(Incomplete::Of(total)),
        _ if has_more => Some(Incomplete::MoreThanRows),
        _ => None,
    }
}

/// [`options`] for a call whose failures are SDK errors: a write's `writes::attempt`
/// closure, a step of a composed write (`workspaces create`) and the workspace reads they
/// share. The options cannot fail for the CLI's fixed budgets; if they ever did, the
/// failure is the SDK's `Configuration` error (printed by the §6 mapping as an
/// `AISEC_SDK_ERROR` with no server text), raised before anything is sent, so the write's
/// "not sent" context applies.
pub fn sdk_options(budget: Duration) -> scm::Result<scm::RequestOptions> {
    options(budget).map_err(|_| scm::Error::new(scm::ErrorKind::Configuration))
}

/// A user value quoted in a message: terminal-safe and at most 200 characters, then `…`
/// (§19.3 "Argument checks").
pub fn shown(value: &str) -> String {
    let safe = terminal_safe(value);
    let mut chars = safe.chars();
    let head: String = chars.by_ref().take(SHOWN_LIMIT).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// `AISEC_USER_REQUEST_PAYLOAD_ERROR:<text>` as a usage error.
fn payload_error(text: String) -> CliError {
    CliError::usage(format!("AISEC_USER_REQUEST_PAYLOAD_ERROR:{text}"))
}

/// `workspaces get <ref>`: a UUID or `^[A-Za-z0-9][A-Za-z0-9_-]*$` (the SDK grammar, at
/// most 512 bytes), else the TypeScript SDK text.
pub fn check_workspace_ref(value: &str) -> Result<(), CliError> {
    let slug = value.len() <= 512
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
    if slug || crate::pagination::is_uuid(value) {
        Ok(())
    } else {
        Err(payload_error(format!(
            "Invalid workspaceRef: {} (expected a workspace UUID or slug)",
            shown(value)
        )))
    }
}

/// A reference that is resolved (§19.4): non-blank under JavaScript trim
/// (`<flag> cannot be empty`) and at most 4,096 bytes without control characters.
pub fn check_resolved_ref(flag: &str, value: &str) -> Result<(), CliError> {
    if crate::tenant::config::js_trim(value).is_empty() {
        return Err(CliError::usage(format!("{flag} cannot be empty")));
    }
    check_text(flag, value)
}

/// At most 4,096 bytes and no control character.
pub fn check_text(flag: &str, value: &str) -> Result<(), CliError> {
    if value.len() <= TEXT_LIMIT && !value.chars().any(char::is_control) {
        Ok(())
    } else {
        Err(CliError::usage(format!(
            "{flag} must be at most {TEXT_LIMIT} bytes and contain no control characters"
        )))
    }
}

/// An IAM scope name (`scm::shared::iam::validate_scope_name`), else the TypeScript SDK
/// text.
pub fn check_scope_name(value: &str) -> Result<(), CliError> {
    scm::shared::iam::validate_scope_name(value).map_err(|_| {
        payload_error(format!(
            "Invalid name: {} (expected an IAM scope name such as ws_production_bx7qw0)",
            shown(value)
        ))
    })
}

/// A UUID argument (the §7.1 shape), else the TypeScript SDK text
/// `AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid <field>: <value>` (§19.3; TS checks it after
/// the tenant, exit 1; D232).
pub fn check_uuid(field: &str, value: &str) -> Result<(), CliError> {
    if crate::pagination::is_uuid(value) {
        Ok(())
    } else {
        Err(payload_error(format!("Invalid {field}: {}", shown(value))))
    }
}

/// A numeric TSG argument: 1–32 ASCII digits (the Rust SDK's `mgmtTsgId` rule), else
/// `AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid <field>: <value>` (TS SDK text).
pub fn check_tsg(field: &str, value: &str) -> Result<(), CliError> {
    if (1..=32).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit()) {
        Ok(())
    } else {
        Err(payload_error(format!("Invalid {field}: {}", shown(value))))
    }
}

/// The message of a TSG argument that is not the selected tenant's (D245; exit 1, after
/// the tenant, before any request). `value` passed [`check_tsg`].
pub fn foreign_tsg(flag: &str, value: &str) -> CliError {
    CliError::operational(format!(
        "{flag} {value} is not the selected tenant's TSG; airs reads and changes only the selected tenant's organisation"
    ))
}

/// TS `paginationInteger` (`ag.ts:26-36`): [`integer`] (the bare reason), then within
/// `min..=max`, else `<flag> must be an integer from <min> to <max>`.
pub fn paging_integer(flag: &str, value: &str, min: i64, max: i64) -> Result<u64, CliError> {
    match integer(value) {
        Ok(number) if (min..=max).contains(&number) => Ok(number.unsigned_abs()),
        Ok(_) => Err(CliError::usage(format!(
            "{flag} must be an integer from {min} to {max}"
        ))),
        Err(reason) => Err(CliError::usage(reason)),
    }
}

/// TS `parseIntegerOption`: `^-?(0|[1-9]\d*)$` and a safe integer, else
/// `Expected an integer` / `Expected a safe integer` (the reason only).
pub fn integer(value: &str) -> Result<i64, &'static str> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    let shaped = !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'));
    if !shaped {
        return Err("Expected an integer");
    }
    // 2^53 − 1 has 16 digits: a longer value is not safe and must not overflow.
    if digits.len() > 16 {
        return Err("Expected a safe integer");
    }
    value
        .parse::<i64>()
        .ok()
        .filter(|number| number.abs() <= MAX_SAFE_INTEGER)
        .ok_or("Expected a safe integer")
}

/// TS `parsePositiveInteger`: [`integer`], then greater than zero, else
/// `Invalid <flag>: <reason>`.
pub fn positive(flag: &str, value: &str) -> Result<u64, CliError> {
    let reason = match integer(value) {
        Ok(number) if number > 0 => return Ok(number.unsigned_abs()),
        Ok(_) => "Expected a positive integer",
        Err(reason) => reason,
    };
    Err(CliError::usage(format!("Invalid {flag}: {reason}")))
}

/// The grant hint of a 403 (G42, D234): the service code `AB03` or a data-plane request
/// without a code names the workspace-scope grant; an admin-plane request without `AB03`
/// the tenant-root admin grant; an IAM request gets none.
pub fn grant_hint(error: &scm::Error, plane: GwPlane) -> Option<String> {
    if error.kind() != scm::ErrorKind::Http || error.status() != Some(403) {
        return None;
    }
    let code = error.service_code();
    let grant = match (plane, code) {
        (GwPlane::Iam, _) => return None,
        (_, Some("AB03")) | (GwPlane::Data, None) => {
            "the service account is missing a workspace-scope grant (data plane, /ai_gw/v2)"
        }
        (GwPlane::Admin, _) => {
            "the service account is missing a tenant-root admin grant (admin plane, /ai_gw/admin/v2)"
        }
        (GwPlane::Data, Some(_)) => return None,
    };
    Some(format!(
        "⚠ 403: {grant}. SCM Access Management edits the existing role row by default — use \"Add Role\" so the account ends up with both role rows, not one row moved."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// G39 (D248): one rule for the warnings and the workspace resolution (review B4).
    #[test]
    fn incomplete_total_is_the_one_g39_rule() {
        let number = |text: &str| crate::json::parse(text).unwrap();
        assert_eq!(incomplete_total(2, Some(&number("2")), false), None);
        assert_eq!(incomplete_total(2, None, false), None);
        assert_eq!(
            incomplete_total(2, Some(&number("5")), false),
            Some(Incomplete::Of(5))
        );
        assert_eq!(
            incomplete_total(2, Some(&number("5")), true),
            Some(Incomplete::Of(5)),
            "a differing total is named even with has_more"
        );
        assert_eq!(
            incomplete_total(2, Some(&number("2")), true),
            Some(Incomplete::MoreThanRows)
        );
        assert_eq!(
            incomplete_total(0, None, true),
            Some(Incomplete::MoreThanRows)
        );
        for ignored in ["2.5", "-1", "9007199254740992", "\"5\""] {
            assert_eq!(
                incomplete_total(2, Some(&number(ignored)), false),
                None,
                "{ignored}"
            );
        }
        for (answer, incomplete) in [
            (r#"{"data":[{}],"total":3}"#, true),
            (r#"{"data":[{}],"has_more":true}"#, true),
            (r#"{"data":[{}],"total":1,"has_more":false}"#, false),
            (r#"{"data":[],"total":"many"}"#, false),
        ] {
            let answer = number(answer);
            assert_eq!(resolve::incomplete(&answer), incomplete, "{answer:?}");
            let rows = answer
                .get("data")
                .and_then(crate::json::OValue::as_array)
                .unwrap();
            let has_more = matches!(
                answer.get("has_more"),
                Some(crate::json::OValue::Bool(true))
            );
            assert_eq!(
                crate::commands::gateway::incomplete_warning(
                    rows.len(),
                    answer.get("total"),
                    has_more,
                    "rows"
                )
                .is_some(),
                incomplete
            );
        }
    }

    #[test]
    fn shown_values_are_terminal_safe_and_bounded() {
        assert_eq!(shown("a\u{1b}b"), "a\u{fffd}b");
        let long = "x".repeat(250);
        assert_eq!(shown(&long), format!("{}…", "x".repeat(200)));
        assert_eq!(shown(&"y".repeat(200)), "y".repeat(200));
    }

    #[test]
    fn workspace_refs_scope_names_and_texts() {
        for good in ["ws-main", "A_b-1", "11111111-1111-4111-8111-000000000001"] {
            assert!(check_workspace_ref(good).is_ok(), "{good}");
        }
        assert_eq!(
            check_workspace_ref("Main A").unwrap_err(),
            CliError::usage(
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid workspaceRef: Main A (expected a workspace UUID or slug)"
            )
        );
        assert!(check_workspace_ref("-x").is_err());
        assert!(check_workspace_ref(&"a".repeat(513)).is_err());
        assert!(check_scope_name("ws_production_bx7qw0").is_ok());
        assert!(check_scope_name("_x.y-z").is_ok());
        assert_eq!(
            check_scope_name("bad/name").unwrap_err(),
            CliError::usage(
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid name: bad/name (expected an IAM scope name such as ws_production_bx7qw0)"
            )
        );
        assert_eq!(
            check_resolved_ref("--workspace", " \u{a0}").unwrap_err(),
            CliError::usage("--workspace cannot be empty")
        );
        assert_eq!(
            check_resolved_ref("--workspace", "a\u{7}").unwrap_err(),
            CliError::usage(
                "--workspace must be at most 4096 bytes and contain no control characters"
            )
        );
        assert!(check_resolved_ref("--workspace", "Main A").is_ok());
    }

    /// The id, TSG and paging checks of the inventory reads (§19.3, §19.13).
    #[test]
    fn ids_tsgs_and_paging() {
        assert!(check_uuid("configId", "11111111-1111-4111-8111-000000000301").is_ok());
        assert_eq!(
            check_uuid("configId", "config-1").unwrap_err(),
            CliError::usage("AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid configId: config-1")
        );
        assert!(check_tsg("tsgId", "1234567890").is_ok());
        for bad in ["", "12a", "-1", &"1".repeat(33)] {
            assert!(check_tsg("tsgId", bad).is_err(), "{bad}");
        }
        assert_eq!(
            check_tsg("tsgId", "x\u{1b}").unwrap_err(),
            CliError::usage("AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid tsgId: x\u{fffd}")
        );
        assert_eq!(paging_integer("--page-size", "1000", 1, 1000), Ok(1000));
        assert_eq!(
            paging_integer("--page-size", "0", 1, 1000).unwrap_err(),
            CliError::usage("--page-size must be an integer from 1 to 1000")
        );
        assert_eq!(
            paging_integer("--page-size", "x", 1, 1000).unwrap_err(),
            CliError::usage("Expected an integer")
        );
        assert_eq!(
            paging_integer("--current-page", "-1", 0, MAX_SAFE_INTEGER).unwrap_err(),
            CliError::usage("--current-page must be an integer from 0 to 9007199254740991")
        );
        assert_eq!(
            paging_integer("--current-page", "9007199254740992", 0, MAX_SAFE_INTEGER).unwrap_err(),
            CliError::usage("Expected a safe integer")
        );
    }

    /// TS `parseIntegerOption` and `parsePositiveInteger` (`si.ts:60-66`, `tel.ts:46-57`).
    #[test]
    fn integers_follow_the_typescript_parsers() {
        assert_eq!(integer("0"), Ok(0));
        assert_eq!(integer("-12"), Ok(-12));
        assert_eq!(integer("9007199254740991"), Ok(MAX_SAFE_INTEGER));
        for bad in ["", "-", "01", "1.0", "1e2", "+1", " 1", "7junk", "0x10"] {
            assert_eq!(integer(bad), Err("Expected an integer"), "{bad}");
        }
        for unsafe_value in ["9007199254740992", "99999999999999999999"] {
            assert_eq!(integer(unsafe_value), Err("Expected a safe integer"));
        }
        assert_eq!(positive("--days", "7"), Ok(7));
        assert_eq!(
            positive("--days", "0").unwrap_err(),
            CliError::usage("Invalid --days: Expected a positive integer")
        );
        assert_eq!(
            positive("--days", "-3").unwrap_err(),
            CliError::usage("Invalid --days: Expected a positive integer")
        );
        assert_eq!(
            positive("--page-size", "abc").unwrap_err(),
            CliError::usage("Invalid --page-size: Expected an integer")
        );
    }
}
