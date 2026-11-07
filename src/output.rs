//! Streams and output formats (DESIGN §2.2, §2.4).
//!
//! stdout carries data only and is buffered until the command succeeds (or a command
//! commits explicitly); stderr carries status lines and errors and is written
//! immediately. Colour follows the per-stream decision of DESIGN §2.8
//! ([`crate::render::style`]).

use crate::error::CliError;
use crate::render::style::{self, Style};
use crate::signals::SignalState;
pub use airs_runtime_core::output_format::Format;
pub use airs_runtime_core::text::{terminal_safe, terminal_safe_lines};
use serde::Serialize;
use std::io::{self, Write};
use std::sync::Arc;

/// How one command uses `--output` (the per-command table of DESIGN §2.4; the help
/// text and planning both read it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatPolicy {
    /// Renders every format; `tenant_default`: the tenant's `defaultOutput` applies
    /// when `--output` is absent (management reads).
    Renders { tenant_default: bool },
    /// Prints the same bytes in every format (`tenant path`); the value is validated
    /// and then ignored (D59).
    Ignored,
    /// Prints no data (the tenant write commands, §3.11); the value is validated and
    /// then ignored (D59).
    NoData,
    /// Prints the same CSV in every format (`runtime topics sample`, §13.11); the value
    /// is validated and then ignored.
    Csv,
    /// `redteam report`: a scan job report renders every format; the environment report
    /// is HTML and refuses an explicit `--output` (DESIGN §15.17, D155).
    Report,
    /// `runtime report`: always HTML; any explicit `--output` is a usage error (DESIGN
    /// §18.14, D155).
    HtmlReport,
    /// `gateway inference`: every format and the tenant's `defaultOutput`; a streamed
    /// json answer is one event per line (DESIGN §19.14).
    Streams,
}

/// Commands that render every format and default to `pretty`.
pub const RENDERS: FormatPolicy = FormatPolicy::Renders {
    tenant_default: false,
};

/// Management reads: every format, the tenant's `defaultOutput`, else `pretty`.
pub const TENANT_DEFAULT: FormatPolicy = FormatPolicy::Renders {
    tenant_default: true,
};

/// Validate an explicit `--output` value (usage error, exit 2, on every command).
pub fn validate_explicit(explicit: Option<&str>) -> Result<Option<Format>, CliError> {
    let Some(name) = explicit else {
        return Ok(None);
    };
    Format::parse(name).map(Some).ok_or_else(|| {
        CliError::usage(format!(
            "Invalid output format '{name}'. Expected: {}",
            Format::valid_names()
        ))
    })
}

/// The explicit format, or `pretty` (commands that ignore `defaultOutput`).
pub fn resolve(explicit: Option<&str>) -> Result<Format, CliError> {
    Ok(validate_explicit(explicit)?.unwrap_or(Format::Pretty))
}

/// The format of a management read: the explicit one, else the tenant's
/// `defaultOutput` (always a valid name: any other value makes the config invalid,
/// D51), else `pretty`.
pub fn resolve_tenant_default(explicit: Option<Format>, configured: Option<&str>) -> Format {
    explicit
        .or_else(|| configured.and_then(Format::parse))
        .unwrap_or(Format::Pretty)
}

/// Per-invocation streams.
pub struct Output {
    quiet: bool,
    /// Whether stdout / stderr are coloured (decided per stream, DESIGN §2.8).
    color_stdout: bool,
    color_stderr: bool,
    buffer: Vec<u8>,
    stdout: Box<dyn Write>,
    stderr: Box<dyn Write>,
    /// The process signal state: a commit is announced to it, so a signal that
    /// arrives while stdout blocks ends the process at once (DESIGN §2.5).
    signals: Option<Arc<SignalState>>,
    /// Whether error blocks carry the `Re-run with --debug` hint (off under `--debug`).
    debug_hint: bool,
}

/// The failure of a stdout write that is not a closed pipe (§2.5).
pub fn stdout_write_failed() -> CliError {
    CliError::operational("AISEC_SDK_ERROR:Output could not be written to stdout")
}

impl Output {
    /// Output bound to the process streams.
    pub fn stdio(quiet: bool) -> Self {
        Self::with_sinks(quiet, Box::new(io::stdout()), Box::new(io::stderr()))
            .with_colors(style::stdout_color(), style::stderr_color())
    }

    /// Uncoloured output to the given sinks.
    pub fn with_sinks(quiet: bool, stdout: Box<dyn Write>, stderr: Box<dyn Write>) -> Self {
        Self {
            quiet,
            color_stdout: false,
            color_stderr: false,
            buffer: Vec::new(),
            stdout,
            stderr,
            signals: None,
            debug_hint: true,
        }
    }

    /// Under `--debug` the error blocks omit the `Re-run with --debug` hint (D69).
    pub fn set_debug(&mut self, debug: bool) {
        self.debug_hint = !debug;
    }

    /// Set the colour decision of each stream.
    pub fn with_colors(mut self, stdout: bool, stderr: bool) -> Self {
        self.color_stdout = stdout;
        self.color_stderr = stderr;
        self
    }

    /// `text` styled for stdout (plain when stdout is not coloured).
    pub fn paint(&self, style: Style, text: &str) -> String {
        style::paint(style, text, self.color_stdout)
    }

    /// `text` styled for stderr (plain when stderr is not coloured).
    pub fn paint_err(&self, style: Style, text: &str) -> String {
        style::paint(style, text, self.color_stderr)
    }

    /// Write a failure's stderr block (coloured per the stderr decision).
    pub fn report(&mut self, error: &CliError) {
        let text = error.render_styled(self.color_stderr, self.debug_hint);
        self.write_stderr(&text);
    }

    /// Announce commits to the process signal state.
    pub fn attach_signals(&mut self, signals: Arc<SignalState>) {
        self.signals = Some(signals);
    }

    /// The exit code of a signal that already arrived, if any.
    pub fn pending_signal(&self) -> Option<i32> {
        self.signals.as_ref().and_then(|signals| signals.code())
    }

    /// The process signal state, when attached.
    pub fn signals(&self) -> Option<Arc<SignalState>> {
        self.signals.clone()
    }

    /// Whether `--quiet` is on.
    pub fn quiet(&self) -> bool {
        self.quiet
    }

    /// A tenant write success line, `"  ✓ <message>\n"` on stderr (`✓ <message>`
    /// green), quiet-suppressed and not printed after a signal (DESIGN §3.9, §3.11,
    /// D74).
    pub fn success(&mut self, message: &str) {
        self.outcome(Style::Green, '✓', message);
    }

    /// A tenant write info line, `"  ℹ <message>\n"` on stderr (`ℹ <message>` cyan),
    /// quiet-suppressed and not printed after a signal (§3.11, D74).
    pub fn info(&mut self, message: &str) {
        self.outcome(Style::Cyan, 'ℹ', message);
    }

    fn outcome(&mut self, style: Style, glyph: char, message: &str) {
        if self.quiet || self.pending_signal().is_some() {
            return;
        }
        let text = self.paint_err(style, &format!("{glyph} {}", terminal_safe(message)));
        self.write_stderr(&format!("  {text}\n"));
    }

    /// `"  <warning>\n"` on stderr in yellow (the text carries its `⚠` glyph), never
    /// quiet-suppressed (the directory-sync warning, §3.9).
    pub fn warning(&mut self, warning: &str) {
        let text = self.paint_err(Style::Yellow, warning);
        self.write_stderr(&format!("  {text}\n"));
    }

    /// `"  <message>\n"` on stderr unless `--quiet`; the message is dim.
    pub fn status(&mut self, message: &str) {
        if !self.quiet {
            let message = self.paint_err(Style::Dim, message);
            self.write_stderr(&format!("  {message}\n"));
        }
    }

    /// Text on stderr that `--quiet` never suppresses (errors, failure summaries).
    pub fn write_stderr(&mut self, text: &str) {
        // A closed stderr must not turn into a second failure.
        let _ = self.stderr.write_all(text.as_bytes());
        let _ = self.stderr.flush();
    }

    /// Buffered stdout data.
    pub fn data(&mut self, text: &str) {
        self.buffer.extend_from_slice(text.as_bytes());
    }

    /// Buffered stdout data followed by a newline.
    pub fn data_line(&mut self, text: &str) {
        self.data(text);
        self.data("\n");
    }

    /// Buffered stdout decoration (headings, hints) that `--quiet` suppresses.
    pub fn decoration(&mut self, text: &str) {
        if !self.quiet {
            self.data(text);
        }
    }

    /// `JSON.stringify(value, null, 2)` plus one newline, numbers in JavaScript form.
    pub fn json<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), CliError> {
        let text = crate::json::to_pretty(value).map_err(|_| {
            CliError::operational("AISEC_SDK_ERROR:Output could not be serialized as JSON")
        })?;
        self.data_line(&text);
        Ok(())
    }

    /// Write the buffered stdout data now. `BrokenPipe` is reported to the caller,
    /// which treats it as success. After a signal nothing is written
    /// (`ErrorKind::Interrupted`); a signal that arrives during the write ends the
    /// process from the signal thread, however long the write blocks.
    pub fn commit(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let _writing = self.signals.as_deref().map(SignalState::begin_write);
        if self.pending_signal().is_some() {
            self.buffer.clear();
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        let buffer = std::mem::take(&mut self.buffer);
        self.stdout.write_all(&buffer)?;
        self.stdout.flush()
    }

    /// [`Output::commit`] as the command's result (§2.5): a closed pipe is success (Node's
    /// console ignores EPIPE); a signal before or during the write is the interruption
    /// with `the output was not completely written`; any other failure is the stdout
    /// write failure. The final write and a command that prints its rows before reporting
    /// a failure line (D319, D210's partial dashboard) both go through it, so that line
    /// follows only rows that were written.
    pub fn commit_data(&mut self) -> Result<(), CliError> {
        match self.commit() {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            Err(_) if self.pending_signal().is_some() => Err(CliError::Interrupted(
                crate::error::InterruptContext::OutputIncomplete,
            )),
            Err(_) => Err(stdout_write_failed()),
        }
    }

    /// Drop buffered data (a failing command leaves stdout empty).
    pub fn discard(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::Output;
    use std::cell::RefCell;
    use std::io::{self, Write};
    use std::rc::Rc;

    /// A shared in-memory sink for capturing a stream in tests.
    #[derive(Clone, Default)]
    pub struct Capture(pub Rc<RefCell<Vec<u8>>>);

    impl Capture {
        pub fn text(&self) -> String {
            String::from_utf8(self.0.borrow().clone()).unwrap()
        }
    }

    impl Write for Capture {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    pub fn capture(quiet: bool) -> (Output, Capture, Capture) {
        let stdout = Capture::default();
        let stderr = Capture::default();
        let output = Output::with_sinks(quiet, Box::new(stdout.clone()), Box::new(stderr.clone()));
        (output, stdout, stderr)
    }

    /// Like [`capture`], with both streams coloured.
    pub fn capture_colored(quiet: bool) -> (Output, Capture, Capture) {
        let (output, stdout, stderr) = capture(quiet);
        (output.with_colors(true, true), stdout, stderr)
    }
}

#[cfg(test)]
mod tests {
    use super::testing::capture;
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn explicit_formats_are_validated() {
        assert_eq!(resolve(None).unwrap(), Format::Pretty);
        assert_eq!(resolve(Some("json")).unwrap(), Format::Json);
        assert_eq!(resolve(Some("table")).unwrap(), Format::Table);
        for dead in ["xml", "markdown", "csv", "yaml", "JSON", ""] {
            assert_eq!(
                resolve(Some(dead)),
                Err(CliError::usage(format!(
                    "Invalid output format '{dead}'. Expected: pretty, table, json"
                ))),
                "{dead}"
            );
        }
    }

    #[test]
    fn tenant_default_matrix() {
        assert_eq!(resolve_tenant_default(None, None), Format::Pretty);
        assert_eq!(resolve_tenant_default(None, Some("table")), Format::Table);
        assert_eq!(resolve_tenant_default(None, Some("json")), Format::Json);
        assert_eq!(
            resolve_tenant_default(Some(Format::Pretty), Some("table")),
            Format::Pretty,
            "an explicit --output wins"
        );
    }

    #[test]
    fn status_lines_are_dim_on_a_coloured_stderr() {
        let (mut output, stdout, stderr) = testing::capture_colored(false);
        output.status("Showing 2");
        output.report(&CliError::usage("bad"));
        output.commit().unwrap();
        assert_eq!(stdout.text(), "");
        assert_eq!(
            stderr.text(),
            "  \u{1b}[2mShowing 2\u{1b}[22m\n\u{1b}[31m\u{1b}[39m\n\u{1b}[31m  ✗ bad\u{1b}[39m\n\u{1b}[31m\u{1b}[39m\n"
        );
        let (mut output, _, stderr) = testing::capture_colored(false);
        assert_eq!(output.paint(Style::Bold, "x"), "\u{1b}[1mx\u{1b}[22m");
        output.success("Selected dev (TSG 100)");
        output.info("Aborted");
        output.warning("⚠ x was saved");
        assert_eq!(
            stderr.text(),
            "  \u{1b}[32m✓ Selected dev (TSG 100)\u{1b}[39m\n  \u{1b}[36mℹ Aborted\u{1b}[39m\n  \u{1b}[33m⚠ x was saved\u{1b}[39m\n"
        );
        let (mut quiet, _, stderr) = testing::capture(true);
        quiet.success("hidden");
        quiet.info("hidden");
        quiet.warning("⚠ shown");
        assert_eq!(
            stderr.text(),
            "  ⚠ shown\n",
            "only the warning survives --quiet"
        );
        let plain = Output::with_sinks(false, Box::new(io::sink()), Box::new(io::sink()))
            .with_colors(false, true);
        assert_eq!(
            plain.paint(Style::Bold, "x"),
            "x",
            "stdout decided on its own"
        );
        assert_eq!(plain.paint_err(Style::Bold, "x"), "\u{1b}[1mx\u{1b}[22m");
    }

    #[test]
    fn status_and_decoration_respect_quiet() {
        let (mut output, stdout, stderr) = capture(true);
        output.status("hidden");
        output.decoration("hidden\n");
        output.data_line("data");
        output.write_stderr("err\n");
        output.commit().unwrap();
        assert_eq!(stdout.text(), "data\n");
        assert_eq!(stderr.text(), "err\n");

        let (mut output, stdout, stderr) = capture(false);
        output.status("Showing 1");
        output.decoration("\n  Heading\n\n");
        output.json(&[1.0_f64]).unwrap();
        assert_eq!(stdout.text(), "", "stdout is buffered until commit");
        output.commit().unwrap();
        assert_eq!(stdout.text(), "\n  Heading\n\n[\n  1\n]\n");
        assert_eq!(stderr.text(), "  Showing 1\n");
    }

    #[test]
    fn control_characters_are_replaced() {
        assert_eq!(terminal_safe("plain é ✓"), "plain é ✓");
        assert!(matches!(terminal_safe("plain"), Cow::Borrowed(_)));
        assert_eq!(
            terminal_safe("R\u{1b}]0;pwned\u{7}\u{9b}2K\u{7f}\n\t"),
            "R\u{fffd}]0;pwned\u{fffd}\u{fffd}2K\u{fffd}\u{fffd}\u{fffd}"
        );
    }

    #[test]
    fn multi_line_text_keeps_line_breaks_and_tabs_only() {
        let summary = "### Summary\n\n- one\r\n- two\twide";
        assert_eq!(terminal_safe_lines(summary), summary);
        assert!(matches!(terminal_safe_lines(summary), Cow::Borrowed(_)));
        // Every other control character, a lone CR included, is still replaced (D52).
        assert_eq!(
            terminal_safe_lines("a\u{1b}[31mb\rc\u{7}d\u{7f}e\u{9b}2Kf\u{0}\u{b}\u{c}\n\r"),
            "a\u{fffd}[31mb\u{fffd}c\u{fffd}d\u{fffd}e\u{fffd}2Kf\u{fffd}\u{fffd}\u{fffd}\n\u{fffd}"
        );
        assert_eq!(terminal_safe_lines("\r\r\n"), "\u{fffd}\r\n");
    }

    #[test]
    fn discard_drops_buffered_data() {
        let (mut output, stdout, _) = capture(false);
        output.data_line("partial");
        output.discard();
        output.commit().unwrap();
        assert_eq!(stdout.text(), "");
    }

    /// A stdout sink whose every write fails with `kind`.
    struct Failing(std::io::ErrorKind);

    impl std::io::Write for Failing {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(self.0))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// §2.5 for a commit before a failure line (D319): written rows are `Ok`, a closed pipe
    /// is `Ok` (EPIPE ignored), another write failure is the stdout failure, and a pending
    /// signal writes nothing and is the interruption, never `Ok`.
    #[test]
    fn commit_data_reports_what_the_write_did() {
        use crate::error::InterruptContext;
        let (mut output, stdout, _) = capture(false);
        output.data_line("row");
        assert_eq!(output.commit_data(), Ok(()));
        assert_eq!(stdout.text(), "row\n");

        let mut closed = Output::with_sinks(
            false,
            Box::new(Failing(std::io::ErrorKind::BrokenPipe)),
            Box::new(std::io::sink()),
        );
        closed.data_line("row");
        assert_eq!(closed.commit_data(), Ok(()));

        let mut full = Output::with_sinks(
            false,
            Box::new(Failing(std::io::ErrorKind::StorageFull)),
            Box::new(std::io::sink()),
        );
        full.data_line("row");
        let error = full.commit_data().unwrap_err();
        assert_eq!(error, stdout_write_failed());
        assert_eq!(error.exit_code(None), 1);

        let (mut signalled, stdout, _) = capture(false);
        let signals = std::sync::Arc::new(SignalState::default());
        assert_eq!(signals.on_signal(143), crate::signals::Action::Cancel);
        signalled.attach_signals(signals);
        signalled.data_line("row");
        assert_eq!(
            signalled.commit_data(),
            Err(CliError::Interrupted(InterruptContext::OutputIncomplete))
        );
        assert_eq!(stdout.text(), "", "nothing is written after a signal");
    }
}
