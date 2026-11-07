//! CLI failures, their stderr rendering and exit codes, and the secret-safe mapping
//! from SDK errors to messages (DESIGN §6).
//!
//! The SDK retains no server text; this module adds none either. Server-derived
//! metadata (request ID, service code) is dropped when it carries a configured secret.

use scm::{ErrorKind, ProblemDetails, RemoteOutcome};
use std::time::Duration;

pub const EXIT_OK: i32 = 0;
pub const EXIT_OPERATIONAL: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_SIGINT: i32 = 130;
pub const EXIT_SIGTERM: i32 = 143;

/// A failed invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliError {
    /// Exit 2: invalid flags, values or local request input.
    Usage(String),
    /// Exit 1: tenant, credential, network or service failure.
    Operational(Failure),
    /// Exit 130/143: a signal cancelled the invocation.
    Interrupted(InterruptContext),
    /// The command already reported the failure on its own streams.
    Reported(i32),
}

impl CliError {
    /// Apply all secrets learned during this invocation to its final failure.
    /// Local paths and state references can coincide with configured credentials.
    pub fn with_secrets(mut self, secrets: &SecretSet) -> Self {
        let scrub = |value: &mut String| *value = secrets.scrub(value).to_owned();
        match &mut self {
            Self::Usage(message) => scrub(message),
            Self::Operational(failure) => {
                scrub(&mut failure.message);
                failure.details.iter_mut().for_each(scrub);
            }
            Self::Interrupted(
                InterruptContext::Context(message) | InterruptContext::Bulk(message),
            ) => scrub(message),
            _ => {}
        }
        self
    }
}

/// An operational failure: the message, an optional `HTTP <status>` line and extra lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Failure {
    pub message: String,
    pub status: Option<u16>,
    pub details: Vec<String>,
}

/// What an interrupted invocation tells the user about remote side effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InterruptContext {
    NoDataWritten,
    ScanNotSubmitted,
    ScanOutcomeUnknown,
    /// The signal arrived while stdout was being written.
    OutputIncomplete,
    /// A tenant write command stopped before its commit point (DESIGN §2.5, §3.9).
    NoConfigurationChanged,
    /// A tenant write command passed its commit point before the signal was acted on.
    ChangeSaved,
    /// A runtime write command stopped before any write was sent (DESIGN §13.2).
    NothingChanged,
    /// A runtime write was sent and then cancelled: it may have been applied.
    ChangeMayBeApplied,
    /// `topics revert` updated the profile; its topic delete was not sent (§13.2).
    ProfileUpdatedTopicKept,
    /// `topics revert` updated the profile; its topic delete was sent and cancelled.
    ProfileUpdatedTopicUnknown,
    /// `topics eval` stopped: `sent` of `total` scans were dispatched (§13.10).
    EvalStopped {
        sent: usize,
        total: usize,
    },
    /// `profiles cleanup` stopped in its deletion loop (§13.15).
    CleanupStopped {
        deleted: usize,
        failed: usize,
        unknown: usize,
        not_attempted: usize,
    },
    /// A bulk-scan job stopped by a signal: the job's own context (DESIGN §14.16).
    Bulk(String),
    /// A red team download stopped before its file was published (DESIGN §15.9).
    NoFileWritten,
    /// A red team context with its own text (DESIGN §15.20): `no scan was started`,
    /// `scan <uuid> was started and may still be running on AIRS; it was not aborted`, …
    Context(String),
}

impl InterruptContext {
    pub fn text(&self) -> std::borrow::Cow<'static, str> {
        let text = match self {
            Self::NoDataWritten => "no data was written",
            Self::ScanNotSubmitted => "no scan was submitted",
            Self::ScanOutcomeUnknown => {
                "the scan request may already have reached AIRS (outcome unknown)"
            }
            Self::OutputIncomplete => "the output was not completely written",
            Self::NoFileWritten => "no file was written",
            Self::NoConfigurationChanged => "no configuration was changed",
            Self::ChangeSaved => "the change was saved",
            Self::NothingChanged => "nothing was changed",
            Self::ChangeMayBeApplied => {
                "the change may already have been applied (outcome unknown)"
            }
            Self::ProfileUpdatedTopicKept => "the profile was updated; the topic was not deleted",
            Self::ProfileUpdatedTopicUnknown => {
                "the profile was updated; the topic may already have been deleted (outcome unknown)"
            }
            Self::Bulk(text) | Self::Context(text) => {
                return std::borrow::Cow::Owned(text.clone());
            }
            Self::EvalStopped { sent, total } => {
                return std::borrow::Cow::Owned(format!(
                    "{sent} of {total} scans were sent; no metrics were computed"
                ));
            }
            Self::CleanupStopped {
                deleted,
                failed,
                unknown,
                not_attempted,
            } => {
                let mut text = format!("{deleted} deleted");
                if *failed > 0 {
                    text.push_str(&format!(", {failed} failed"));
                }
                if *unknown > 0 {
                    text.push_str(&format!(", {unknown} outcome unknown"));
                }
                text.push_str(&format!(", {not_attempted} not attempted"));
                return std::borrow::Cow::Owned(text);
            }
        };
        std::borrow::Cow::Borrowed(text)
    }
}

impl CliError {
    pub fn usage(message: impl Into<String>) -> Self {
        Self::Usage(message.into())
    }
    pub fn operational(message: impl Into<String>) -> Self {
        Self::Operational(Failure {
            message: message.into(),
            ..Failure::default()
        })
    }

    /// The process exit code; `signal_code` is 130/143 when a signal arrived.
    pub fn exit_code(&self, signal_code: Option<i32>) -> i32 {
        match self {
            Self::Usage(_) => EXIT_USAGE,
            Self::Operational(_) => EXIT_OPERATIONAL,
            Self::Interrupted(_) => signal_code.unwrap_or(EXIT_SIGINT),
            Self::Reported(code) => *code,
        }
    }

    /// The message alone: the operational message, or the rendered text of any other
    /// failure without its surrounding blank lines (doctor details, setup errors).
    pub fn message(&self) -> String {
        match self {
            Self::Operational(failure) => failure.message.clone(),
            other => other.render().trim().to_owned(),
        }
    }

    /// The exact uncoloured stderr bytes for this failure (empty when already
    /// reported).
    pub fn render(&self) -> String {
        self.render_colored(false)
    }

    /// The exact stderr bytes for this failure, in TypeScript's colours when `color`
    /// (DESIGN §2.8), for a run without `--debug` (the `Re-run with --debug` hint
    /// follows an `HTTP <status>` line).
    pub fn render_colored(&self, color: bool) -> String {
        self.render_styled(color, true)
    }

    /// The exact stderr bytes for this failure. `debug_hint`: print the TS
    /// `Re-run with --debug …` line after an `HTTP <status>` line (DESIGN §2.2; off under
    /// `--debug`, D69). Control characters in messages are replaced (DESIGN §2.2), so no
    /// server- or file-derived text can drive the terminal.
    pub fn render_styled(&self, color: bool, debug_hint: bool) -> String {
        use crate::style::{Style, paint};
        use crate::text::terminal_safe;
        let red = |text: &str| paint(Style::Red, text, color);
        match self {
            // chalk.red(`\n  ✗ ${message}\n`) + console.error's newline.
            Self::Usage(message) => {
                format!("{}\n", red(&format!("\n  ✗ {}\n", terminal_safe(message))))
            }
            Self::Operational(failure) => {
                let mut text = format!(
                    "{}\n",
                    red(&format!("\n  ✗ Error: {}", terminal_safe(&failure.message)))
                );
                if let Some(status) = failure.status {
                    text.push_str(&format!("{}\n", red(&format!("    HTTP {status}"))));
                    if debug_hint {
                        text.push_str(&format!("{}\n", paint(Style::Dim, DEBUG_HINT, color)));
                    }
                }
                for line in &failure.details {
                    text.push_str(&format!(
                        "{}\n",
                        red(&format!("    {}", terminal_safe(line)))
                    ));
                }
                text.push('\n');
                text
            }
            Self::Interrupted(context) => format!(
                "{}\n\n",
                red(&format!("\n  ✗ Error: Interrupted; {}", context.text()))
            ),
            Self::Reported(_) => String::new(),
        }
    }
}

/// The TS hint after an `HTTP <status>` line (`src/cli/renderer/common.ts:21`).
pub const DEBUG_HINT: &str = "    Re-run with --debug to capture redacted API diagnostics.";

// The secret set lives in its own module (choice 187); callers name it here as before.
pub use crate::secrets::SecretSet;
pub use crate::secrets::base64_standard;

/// What a server-provided string that carried a secret is printed as.
pub const WITHHELD: &str = "[withheld]";

/// Which client produced the error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plane {
    Management,
    Scanner,
}

/// Whether the failing call was the explicit OAuth token request or an API call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Token,
    Api,
}

/// Reads are side-effect free; a scan submission or a management write may have been
/// processed remotely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Read,
    ScanSubmit,
    /// A management POST, PUT or DELETE (DESIGN §13.2).
    Write,
}

/// Everything the mapping needs besides the error itself.
#[derive(Clone, Copy)]
pub struct SdkContext<'a> {
    pub plane: Plane,
    pub phase: Phase,
    pub operation: Operation,
    /// The total budget the call ran under, for the timeout message.
    pub timeout: Duration,
    pub secrets: &'a SecretSet,
}

/// The safe facts of an SDK error that the CLI messages are derived from.
#[derive(Clone)]
pub struct ErrorFacts {
    pub kind: ErrorKind,
    pub status: Option<u16>,
    pub error_type: Option<&'static str>,
    pub problem: Option<ProblemDetails>,
    pub service_code: Option<String>,
    pub request_id: Option<String>,
    pub remote_outcome: RemoteOutcome,
}

impl ErrorFacts {
    pub fn of(error: &scm::Error) -> Self {
        Self {
            kind: error.kind(),
            status: error.status(),
            error_type: error.error_type().map(scm::ErrorType::as_str),
            problem: error.problem().cloned(),
            service_code: error.service_code().map(str::to_owned),
            request_id: error.request_id().map(str::to_owned),
            remote_outcome: error.remote_outcome(),
        }
    }

    /// An HTTP error status, including a body that exceeded the client limit.
    pub fn http_status(&self) -> Option<u16> {
        match self.kind {
            ErrorKind::Http => self.status,
            ErrorKind::Limit => self.status.filter(|status| *status >= 400),
            _ => None,
        }
    }
}

/// Map an SDK error to the CLI failure (DESIGN §6).
pub fn from_sdk(error: &scm::Error, context: &SdkContext<'_>) -> CliError {
    describe(&ErrorFacts::of(error), context)
}

/// The `✗ Error:` message for an SDK error, without status or detail lines.
pub fn sdk_message(error: &scm::Error, context: &SdkContext<'_>) -> String {
    facts_message(&ErrorFacts::of(error), context)
}

/// [`sdk_message`] for already extracted (possibly adjusted) facts.
pub fn facts_message(facts: &ErrorFacts, context: &SdkContext<'_>) -> String {
    match describe(facts, context) {
        CliError::Operational(failure) => failure.message,
        CliError::Interrupted(interrupt) => format!("Interrupted; {}", interrupt.text()),
        CliError::Usage(message) => message,
        CliError::Reported(_) => String::new(),
    }
}

pub fn describe(facts: &ErrorFacts, context: &SdkContext<'_>) -> CliError {
    if facts.kind == ErrorKind::Cancelled {
        return CliError::Interrupted(match context.operation {
            Operation::Read => InterruptContext::NoDataWritten,
            Operation::ScanSubmit if facts.remote_outcome == RemoteOutcome::NotSent => {
                InterruptContext::ScanNotSubmitted
            }
            Operation::ScanSubmit => InterruptContext::ScanOutcomeUnknown,
            Operation::Write if facts.remote_outcome == RemoteOutcome::NotSent => {
                InterruptContext::NothingChanged
            }
            Operation::Write => InterruptContext::ChangeMayBeApplied,
        });
    }
    let mut failure = Failure::default();
    let management = context.plane == Plane::Management;
    if let Some(status) = facts.http_status() {
        failure.message = api_error_message(facts, status, context.secrets);
        failure.status = Some(status);
        if let Some(id) = facts
            .request_id
            .as_deref()
            .filter(|id| !context.secrets.taints(id))
        {
            failure.details.push(format!("Request ID: {id}"));
        }
    } else {
        failure.message = match facts.kind {
            ErrorKind::Authentication => match facts.status {
                Some(status) => {
                    failure.status = Some(status);
                    format!("AISEC_OAUTH_ERROR:Token request failed with status {status}")
                }
                None => OAUTH_SCHEMA.to_owned(),
            },
            ErrorKind::Decode if management && context.phase == Phase::Token => {
                OAUTH_SCHEMA.to_owned()
            }
            ErrorKind::Transport | ErrorKind::Io
                if management && context.phase == Phase::Token =>
            {
                "AISEC_OAUTH_ERROR:Token request failed: network error".to_owned()
            }
            ErrorKind::Transport | ErrorKind::Io => {
                "AISEC_CLIENT_SIDE_ERROR:Network request failed (connection refused, reset or unreachable)"
                    .to_owned()
            }
            ErrorKind::Deadline => format!(
                "AISEC_SDK_ERROR:Request timed out after {}s",
                context.timeout.as_secs()
            ),
            ErrorKind::Decode => {
                "AISEC_RESPONSE_VALIDATION:Response did not match the expected schema".to_owned()
            }
            // A scan submission the SDK refused to send for its size (no response,
            // nothing sent; a submission has no pagination or response-side limit yet).
            ErrorKind::Limit
                if context.operation == Operation::ScanSubmit
                    && facts.status.is_none()
                    && facts.remote_outcome == RemoteOutcome::NotSent =>
            {
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request exceeded a client safety limit".to_owned()
            }
            ErrorKind::Limit => "AISEC_SDK_ERROR:Response exceeded a client safety limit".to_owned(),
            ErrorKind::Pagination => "AISEC_SDK_ERROR:Pagination stopped: the service returned a repeated cursor or an empty intermediate page".to_owned(),
            ErrorKind::Destination if facts.status.is_some_and(|s| (300..400).contains(&s)) => {
                failure.status = facts.status;
                "AISEC_CLIENT_SIDE_ERROR:Redirect responses are not followed".to_owned()
            }
            kind => format!(
                "{}:{} error",
                facts.error_type.unwrap_or("AISEC_SDK_ERROR"),
                kind_name(kind)
            ),
        };
    }
    if context.operation == Operation::ScanSubmit
        && facts.remote_outcome == RemoteOutcome::Unknown
        && !facts.status.is_some_and(|s| (400..500).contains(&s))
    {
        failure
            .details
            .push("Outcome unknown: the scan may have been received and processed by AIRS.".into());
    }
    CliError::Operational(failure)
}

const OAUTH_SCHEMA: &str = "AISEC_OAUTH_ERROR:Token response did not match the OAuth schema";

/// `<type>:API error <status>[; <title>; <detail>; <prop>: <reason>][ (errorCode: <code>)]`
fn api_error_message(facts: &ErrorFacts, status: u16, secrets: &SecretSet) -> String {
    let error_type = if status >= 500 {
        "AISEC_SERVER_SIDE_ERROR"
    } else {
        "AISEC_CLIENT_SIDE_ERROR"
    };
    let mut parts = vec![format!("API error {status}")];
    if let Some(problem) = &facts.problem {
        for text in [problem.title(), problem.detail()].into_iter().flatten() {
            if !parts.iter().skip(1).any(|part| part == text) {
                parts.push(text.to_owned());
            }
        }
        for field in problem.errors().unwrap_or_default() {
            parts.push(format!("{}: {}", field.property(), field.reason()));
        }
    }
    let mut message = format!("{error_type}:{}", parts.join("; "));
    if let Some(code) = facts
        .service_code
        .as_deref()
        .filter(|code| !secrets.taints(code))
    {
        message.push_str(&format!(" (errorCode: {code})"));
    }
    message
}

pub fn kind_name(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::Configuration => "Configuration",
        ErrorKind::Validation => "Validation",
        ErrorKind::Destination => "Destination",
        ErrorKind::Authentication => "Authentication",
        ErrorKind::Io => "Io",
        ErrorKind::Transport => "Transport",
        ErrorKind::Http => "Http",
        ErrorKind::Decode => "Decode",
        ErrorKind::Limit => "Limit",
        ErrorKind::Deadline => "Deadline",
        ErrorKind::Cancelled => "Cancelled",
        ErrorKind::Protocol => "Protocol",
        ErrorKind::Pagination => "Pagination",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn facts(kind: ErrorKind, status: Option<u16>) -> ErrorFacts {
        let error_type = match (kind, status) {
            (ErrorKind::Http, Some(s)) if s >= 500 => Some("AISEC_SERVER_SIDE_ERROR"),
            (ErrorKind::Http, _) => Some("AISEC_CLIENT_SIDE_ERROR"),
            _ => scm::Error::new(kind)
                .error_type()
                .map(scm::ErrorType::as_str),
        };
        ErrorFacts {
            kind,
            status,
            error_type,
            problem: None,
            service_code: None,
            request_id: None,
            remote_outcome: RemoteOutcome::NotSent,
        }
    }

    fn context<'a>(secrets: &'a SecretSet, plane: Plane, phase: Phase) -> SdkContext<'a> {
        SdkContext {
            plane,
            phase,
            operation: Operation::Read,
            timeout: Duration::from_secs(60),
            secrets,
        }
    }

    fn failure(error: CliError) -> Failure {
        match error {
            CliError::Operational(failure) => failure,
            other => panic!("expected operational, got {other:?}"),
        }
    }

    #[test]
    fn renders_exact_stream_bytes() {
        assert_eq!(CliError::usage("bad").render(), "\n  ✗ bad\n\n");
        assert_eq!(
            CliError::operational("boom").render(),
            "\n  ✗ Error: boom\n\n"
        );
        let error = CliError::Operational(Failure {
            message: "m".into(),
            status: Some(404),
            details: vec!["Request ID: r-1".into()],
        });
        assert_eq!(
            error.render(),
            "\n  ✗ Error: m\n    HTTP 404\n    Re-run with --debug to capture redacted API diagnostics.\n    Request ID: r-1\n\n"
        );
        // Under --debug the hint is omitted (D69).
        assert_eq!(
            error.render_styled(false, false),
            "\n  ✗ Error: m\n    HTTP 404\n    Request ID: r-1\n\n"
        );
        assert_eq!(
            CliError::operational("no status").render_styled(false, true),
            "\n  ✗ Error: no status\n\n",
            "the hint follows the HTTP line only"
        );
        assert_eq!(
            CliError::Interrupted(InterruptContext::NoDataWritten).render(),
            "\n  ✗ Error: Interrupted; no data was written\n\n"
        );
        assert_eq!(CliError::Reported(1).render(), "");
    }

    #[test]
    fn coloured_blocks_follow_chalk() {
        assert_eq!(
            CliError::usage("bad").render_colored(true),
            "\u{1b}[31m\u{1b}[39m\n\u{1b}[31m  ✗ bad\u{1b}[39m\n\u{1b}[31m\u{1b}[39m\n"
        );
        let error = CliError::Operational(Failure {
            message: "AISEC_CLIENT_SIDE_ERROR:API error 404".into(),
            status: Some(404),
            details: vec!["Request ID: r-1".into()],
        });
        assert_eq!(
            error.render_colored(true),
            "\u{1b}[31m\u{1b}[39m\n\u{1b}[31m  ✗ Error: AISEC_CLIENT_SIDE_ERROR:API error 404\u{1b}[39m\n\u{1b}[31m    HTTP 404\u{1b}[39m\n\u{1b}[2m    Re-run with --debug to capture redacted API diagnostics.\u{1b}[22m\n\u{1b}[31m    Request ID: r-1\u{1b}[39m\n\n"
        );
        assert_eq!(error.render_colored(false), error.render());
        assert_eq!(
            CliError::Interrupted(InterruptContext::NoDataWritten).render_colored(true),
            "\u{1b}[31m\u{1b}[39m\n\u{1b}[31m  ✗ Error: Interrupted; no data was written\u{1b}[39m\n\n"
        );
    }

    #[test]
    fn exit_codes() {
        assert_eq!(CliError::usage("x").exit_code(None), 2);
        assert_eq!(CliError::operational("x").exit_code(Some(130)), 1);
        let interrupted = CliError::Interrupted(InterruptContext::NoDataWritten);
        assert_eq!(interrupted.exit_code(Some(143)), 143);
        assert_eq!(interrupted.exit_code(None), 130);
        assert_eq!(CliError::Reported(1).exit_code(None), 1);
    }

    #[test]
    fn http_errors_with_problem_code_and_request_id() {
        let secrets = SecretSet::default();
        let mut f = facts(ErrorKind::Http, Some(400));
        f.problem = ProblemDetails::from_value(&json!({
            "title": "Bad Request",
            "detail": "secret text",
            "errors": [{"property": "name", "reason": "is required"}]
        }));
        f.service_code = Some("E400".into());
        f.request_id = Some("req-1".into());
        let failure = failure(describe(
            &f,
            &context(&secrets, Plane::Management, Phase::Api),
        ));
        assert_eq!(
            failure.message,
            "AISEC_CLIENT_SIDE_ERROR:API error 400; Bad Request; [withheld]; name: is required (errorCode: E400)"
        );
        assert_eq!(failure.status, Some(400));
        assert_eq!(failure.details, ["Request ID: req-1"]);

        let failure = failure_of(ErrorKind::Http, Some(503), Plane::Scanner, Phase::Api);
        assert_eq!(failure.message, "AISEC_SERVER_SIDE_ERROR:API error 503");
        let failure = failure_of(ErrorKind::Limit, Some(401), Plane::Scanner, Phase::Api);
        assert_eq!(failure.message, "AISEC_CLIENT_SIDE_ERROR:API error 401");
        assert_eq!(failure.status, Some(401));
    }

    fn failure_of(kind: ErrorKind, status: Option<u16>, plane: Plane, phase: Phase) -> Failure {
        let secrets = SecretSet::default();
        failure(describe(
            &facts(kind, status),
            &context(&secrets, plane, phase),
        ))
    }

    #[test]
    fn duplicate_problem_title_and_detail_are_shown_once() {
        let secrets = SecretSet::default();
        let mut f = facts(ErrorKind::Http, Some(404));
        f.problem =
            ProblemDetails::from_value(&json!({"title": "Not Found", "detail": "Not Found"}));
        let failure = failure(describe(&f, &context(&secrets, Plane::Scanner, Phase::Api)));
        assert_eq!(
            failure.message,
            "AISEC_CLIENT_SIDE_ERROR:API error 404; Not Found"
        );
    }

    #[test]
    fn tainted_metadata_is_dropped() {
        let secrets = SecretSet::default();
        secrets.add("FAKE-SECRET-100");
        let mut f = facts(ErrorKind::Http, Some(401));
        f.service_code = Some("FAKE-SECRET-100".into());
        f.request_id = Some("RkFLRS1TRUNSRVQtMTAw".into());
        let failure = failure(describe(
            &f,
            &context(&secrets, Plane::Management, Phase::Api),
        ));
        assert_eq!(failure.message, "AISEC_CLIENT_SIDE_ERROR:API error 401");
        assert!(failure.details.is_empty());
    }

    #[test]
    fn oauth_failures_are_distinguished() {
        let f = failure_of(
            ErrorKind::Authentication,
            Some(401),
            Plane::Management,
            Phase::Token,
        );
        assert_eq!(
            f.message,
            "AISEC_OAUTH_ERROR:Token request failed with status 401"
        );
        assert_eq!(f.status, Some(401));
        let f = failure_of(ErrorKind::Transport, None, Plane::Management, Phase::Token);
        assert_eq!(
            f.message,
            "AISEC_OAUTH_ERROR:Token request failed: network error"
        );
        let f = failure_of(ErrorKind::Decode, None, Plane::Management, Phase::Token);
        assert_eq!(f.message, OAUTH_SCHEMA);
        let f = failure_of(
            ErrorKind::Authentication,
            None,
            Plane::Management,
            Phase::Token,
        );
        assert_eq!(f.message, OAUTH_SCHEMA);
        for status in [Some(200), None] {
            let f = failure_of(ErrorKind::Decode, status, Plane::Management, Phase::Api);
            assert_eq!(
                f.message,
                "AISEC_RESPONSE_VALIDATION:Response did not match the expected schema"
            );
        }
    }

    #[test]
    fn local_and_transport_kinds() {
        let f = failure_of(ErrorKind::Transport, None, Plane::Scanner, Phase::Api);
        assert_eq!(
            f.message,
            "AISEC_CLIENT_SIDE_ERROR:Network request failed (connection refused, reset or unreachable)"
        );
        assert_eq!(f.status, None);
        let f = failure_of(ErrorKind::Deadline, None, Plane::Scanner, Phase::Api);
        assert_eq!(f.message, "AISEC_SDK_ERROR:Request timed out after 60s");
        let f = failure_of(ErrorKind::Limit, Some(200), Plane::Scanner, Phase::Api);
        assert_eq!(
            f.message,
            "AISEC_SDK_ERROR:Response exceeded a client safety limit"
        );
        // A local limit of another operation (a walk's row bound) keeps the text.
        let f = failure_of(ErrorKind::Limit, None, Plane::Scanner, Phase::Api);
        assert_eq!(
            f.message,
            "AISEC_SDK_ERROR:Response exceeded a client safety limit"
        );
        // A scan submission the SDK refused for its size: no status, nothing sent.
        let secrets = SecretSet::default();
        let submit = SdkContext {
            operation: Operation::ScanSubmit,
            ..context(&secrets, Plane::Scanner, Phase::Api)
        };
        let f = failure(describe(&facts(ErrorKind::Limit, None), &submit));
        assert_eq!(
            f.message,
            "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request exceeded a client safety limit"
        );
        let mut response_side = facts(ErrorKind::Limit, None);
        response_side.remote_outcome = RemoteOutcome::Unknown;
        let f = failure(describe(&response_side, &submit));
        assert_eq!(
            f.message,
            "AISEC_SDK_ERROR:Response exceeded a client safety limit"
        );
        let f = failure_of(ErrorKind::Pagination, None, Plane::Management, Phase::Api);
        assert!(f.message.starts_with("AISEC_SDK_ERROR:Pagination stopped"));
        let f = failure_of(
            ErrorKind::Destination,
            Some(302),
            Plane::Scanner,
            Phase::Api,
        );
        assert_eq!(
            f.message,
            "AISEC_CLIENT_SIDE_ERROR:Redirect responses are not followed"
        );
        assert_eq!(f.status, Some(302));
        let f = failure_of(ErrorKind::Validation, None, Plane::Management, Phase::Api);
        assert_eq!(
            f.message,
            "AISEC_USER_REQUEST_PAYLOAD_ERROR:Validation error"
        );
        let f = failure_of(ErrorKind::Configuration, None, Plane::Scanner, Phase::Api);
        assert_eq!(f.message, "AISEC_MISSING_VARIABLE:Configuration error");
    }

    #[test]
    fn scan_submissions_report_unknown_outcomes() {
        let secrets = SecretSet::default();
        let mut ctx = context(&secrets, Plane::Scanner, Phase::Api);
        ctx.operation = Operation::ScanSubmit;
        let mut f = facts(ErrorKind::Http, Some(503));
        f.remote_outcome = RemoteOutcome::Unknown;
        let failure_503 = failure(describe(&f, &ctx));
        assert_eq!(
            failure_503.details,
            ["Outcome unknown: the scan may have been received and processed by AIRS."]
        );
        let mut f = facts(ErrorKind::Limit, Some(401));
        f.remote_outcome = RemoteOutcome::Unknown;
        assert!(failure(describe(&f, &ctx)).details.is_empty());
        let mut f = facts(ErrorKind::Cancelled, None);
        f.remote_outcome = RemoteOutcome::Unknown;
        assert_eq!(
            describe(&f, &ctx),
            CliError::Interrupted(InterruptContext::ScanOutcomeUnknown)
        );
        f.remote_outcome = RemoteOutcome::NotSent;
        assert_eq!(
            describe(&f, &ctx),
            CliError::Interrupted(InterruptContext::ScanNotSubmitted)
        );
        ctx.operation = Operation::Read;
        assert_eq!(
            describe(&f, &ctx),
            CliError::Interrupted(InterruptContext::NoDataWritten)
        );
    }

    #[test]
    fn facts_from_real_sdk_errors() {
        let facts = ErrorFacts::of(&scm::Error::new(ErrorKind::Transport));
        assert_eq!(facts.kind, ErrorKind::Transport);
        assert_eq!(facts.error_type, Some("AISEC_CLIENT_SIDE_ERROR"));
        assert_eq!(facts.remote_outcome, RemoteOutcome::NotSent);
        let secrets = SecretSet::default();
        let message = sdk_message(
            &scm::Error::new(ErrorKind::Decode),
            &context(&secrets, Plane::Scanner, Phase::Api),
        );
        assert_eq!(
            message,
            "AISEC_RESPONSE_VALIDATION:Response did not match the expected schema"
        );
    }
}
