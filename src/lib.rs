//! `airs`: a native Prisma AIRS CLI built on the `scm-sdk` crate.
//!
//! [`run`] is the whole process: version pre-scan, argument parsing, usage
//! validation, a current-thread Tokio runtime with signal-driven cancellation, the
//! command, and the exit-code mapping (DESIGN §2).
#![forbid(unsafe_code)]

// Every module is private: the binary is the only consumer, and private modules keep
// the dead-code lint working across the crate.
mod bulk;
mod cli;
mod commands;
mod debug;
mod dlp;
mod env;
mod files;
mod gateway;
mod input;
mod modelsec;
mod output;
mod pagination;
mod policy;
mod redteam;
mod render;
mod report;
mod runtime;
mod segment;
mod signals;
mod store;
mod supplychain;
mod tenant;
#[cfg(test)]
mod test_support;
mod views;
mod walk;
mod writes;

// The shared foundation lives in airs-runtime-core; its modules keep their CLI paths.
use airs_runtime_core::{clients, error, json, profile_policy, secrets};

use clap::FromArgMatches;
use commands::{Invocation, Plan};
use error::{CliError, EXIT_OK, EXIT_OPERATIONAL, EXIT_USAGE};
use output::Output;
use std::ffi::OsString;
use std::io::{ErrorKind, Write};

/// The bare version printed by `-V` / `--version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Run the CLI with `args` (including the program name) and return the exit code.
pub fn run<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    if version_requested(&args) {
        return print_version();
    }
    let parsed = cli::command()
        .try_get_matches_from(&args)
        .and_then(|matches| cli::Cli::from_arg_matches(&matches));
    let cli = match parsed {
        Ok(cli) => cli,
        Err(error) => {
            // Help goes to stdout (exit 0); parse errors to stderr (exit 2), without
            // argument values for the commands that take secrets (D91).
            let error = cli::redacted_parse_error(&args, &error).unwrap_or(error);
            let _ = error.print();
            return error.exit_code();
        }
    };
    let mut output = Output::stdio(cli.global.quiet);
    if let Some(message) = cli::evidence_repetitions(&args, &cli) {
        let error = error::CliError::usage(message);
        output.report(&error);
        return EXIT_USAGE;
    }

    output.set_debug(cli.global.debug);
    // The deprecated `--format` alias warns before any other validation (DESIGN §13.1).
    if commands::uses_format_alias(&cli) {
        output.warning(commands::FORMAT_DEPRECATED);
    }
    // So do the red team aliases (DESIGN §15.3 "Validation order").
    if let Some(cli::TopCommand::Redteam(group)) = &cli.command {
        for warning in commands::redteam::deprecation_warnings(group) {
            output.warning(warning);
        }
    }
    // So do the hidden `--size` and `--page` of the DLP lists (DESIGN §16.3).
    if let Some(group) = commands::dlp_group(&cli) {
        for warning in commands::dlp::deprecation_warnings(group) {
            output.warning(warning);
        }
    }
    // So does the hidden `--input` of `runtime bulk-scan` (DESIGN §14.1).
    if commands::uses_input_alias(&cli) {
        output.warning(commands::INPUT_DEPRECATED);
    }
    let plan = match commands::plan(&cli) {
        Ok(plan) => plan,
        Err(error) => {
            output.report(&error);
            return error.exit_code(None);
        }
    };
    if let Plan::GroupHelp(path) = plan {
        output.write_stderr(&cli::group_help(path));
        return EXIT_USAGE;
    }
    let mut invocation = Invocation::new(output, cli.global.tenant.clone());
    // After all usage validation, before the tenant is read (DESIGN §2.9).
    if cli.global.debug
        && plan.debug_policy() == debug::DebugPolicy::Log
        && let Err(error) = invocation.open_debug_log()
    {
        invocation.output.report(&error);
        return error.exit_code(None);
    }
    execute(plan, invocation)
}

/// `-V` / `--version` anywhere before a literal `--` (commander fires it during parse,
/// even where an option value is expected).
pub fn version_requested(args: &[OsString]) -> bool {
    args.iter()
        .skip(1)
        .take_while(|arg| *arg != "--")
        .any(|arg| arg == "-V" || arg == "--version")
}

/// The failure for a stdout write that failed for a reason other than a closed pipe.
/// Fixed text: the OS error is never echoed.
fn print_version() -> i32 {
    let mut stdout = std::io::stdout().lock();
    match writeln!(stdout, "{VERSION}").and_then(|()| stdout.flush()) {
        Err(error) if error.kind() != ErrorKind::BrokenPipe => {
            let text = output::stdout_write_failed().render_colored(render::style::stderr_color());
            let _ = std::io::stderr().write_all(text.as_bytes());
            EXIT_OPERATIONAL
        }
        _ => EXIT_OK,
    }
}

fn execute(plan: Plan, mut invocation: Invocation) -> i32 {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            let error = CliError::operational("AISEC_SDK_ERROR:The async runtime could not start");
            invocation.output.report(&error);
            return EXIT_OPERATIONAL;
        }
    };
    let signals = signals::install(invocation.session.clone());
    signals.set_quiet(invocation.output.quiet());
    invocation.output.attach_signals(signals.clone());
    let fallback = plan.interrupt_context();
    let result = runtime.block_on(commands::dispatch(&mut invocation, plan));
    // A tenant write that passed its commit point reports that the change was saved.
    let fallback = if invocation.committed {
        error::InterruptContext::ChangeSaved
    } else {
        invocation.interrupt.take().unwrap_or(fallback)
    };
    // Never wait for leftover blocking tasks (a slow `getaddrinfo` after a cancel or a
    // deadline): the process exits right after `finish`.
    runtime.shutdown_background();
    let signal = signals.code();
    // A signal always ends the run as interrupted; partial results are never written.
    let result = match (result, signal) {
        (Err(CliError::Interrupted(context)), _) => Err(CliError::Interrupted(context)),
        (_, Some(_)) => Err(CliError::Interrupted(fallback)),
        (result, None) => result,
    };
    let result = result.map_err(|error| {
        if invocation.secrets.guards_local_output() {
            error.with_secrets(&invocation.secrets)
        } else {
            error
        }
    });
    finish(&mut invocation.output, result, signal)
}

fn finish(output: &mut Output, result: Result<(), CliError>, signal: Option<i32>) -> i32 {
    match result {
        // Node's console ignores EPIPE; a signal that arrived just before the write is
        // reported as the interruption (`Output::commit_data`).
        Ok(()) => match output.commit_data() {
            Ok(()) => EXIT_OK,
            Err(error) => {
                output.report(&error);
                error.exit_code(output.pending_signal())
            }
        },
        Err(error) => {
            output.discard();
            output.report(&error);
            error.exit_code(signal)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn version_pre_scan() {
        assert!(version_requested(&args(&["airs", "-V"])));
        assert!(version_requested(&args(&[
            "airs",
            "runtime",
            "scan",
            "--profile",
            "-V",
            "hi"
        ])));
        assert!(version_requested(&args(&["airs", "doctor", "--version"])));
        assert!(!version_requested(&args(&[
            "airs",
            "runtime",
            "scan",
            "--profile",
            "p",
            "--",
            "-V"
        ])));
        assert!(
            !version_requested(&args(&["-V"])),
            "the program name is skipped"
        );
        assert!(!version_requested(&args(&["airs", "-v"])));
    }

    #[test]
    fn finish_maps_results_to_exit_codes() {
        let (mut out, stdout, stderr) = output::testing::capture(false);
        out.data_line("data");
        assert_eq!(finish(&mut out, Ok(()), None), 0);
        assert_eq!(stdout.text(), "data\n");

        let (mut out, stdout, stderr2) = output::testing::capture(false);
        out.data_line("partial");
        assert_eq!(finish(&mut out, Err(CliError::usage("bad")), None), 2);
        assert_eq!(stdout.text(), "");
        assert_eq!(stderr2.text(), "\n  ✗ bad\n\n");

        let (mut out, _, stderr3) = output::testing::capture(false);
        let code = finish(
            &mut out,
            Err(CliError::Interrupted(
                error::InterruptContext::NoDataWritten,
            )),
            Some(143),
        );
        assert_eq!(code, 143);
        assert!(stderr3.text().contains("Interrupted; no data was written"));
        assert_eq!(stderr.text(), "");
    }

    #[test]
    fn broken_pipe_is_success() {
        struct Closed;
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from(ErrorKind::BrokenPipe))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut out = Output::with_sinks(false, Box::new(Closed), Box::new(std::io::sink()));
        out.data_line("data");
        assert_eq!(finish(&mut out, Ok(()), None), 0);
    }

    #[test]
    fn other_stdout_write_failures_are_reported() {
        struct Full;
        impl Write for Full {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from(ErrorKind::StorageFull))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let stderr = output::testing::Capture::default();
        let mut out = Output::with_sinks(false, Box::new(Full), Box::new(stderr.clone()));
        out.data_line("data");
        assert_eq!(finish(&mut out, Ok(()), None), 1);
        assert_eq!(
            stderr.text(),
            "\n  ✗ Error: AISEC_SDK_ERROR:Output could not be written to stdout\n\n"
        );
    }

    #[test]
    fn a_signal_before_the_final_write_reports_the_interruption() {
        let (mut out, stdout, stderr) = output::testing::capture(false);
        let signals = std::sync::Arc::new(signals::SignalState::default());
        assert_eq!(signals.on_signal(143), signals::Action::Cancel);
        out.attach_signals(signals);
        out.data_line("data");
        assert_eq!(finish(&mut out, Ok(()), None), 143);
        assert_eq!(stdout.text(), "", "nothing is written after a signal");
        assert_eq!(
            stderr.text(),
            "\n  ✗ Error: Interrupted; the output was not completely written\n\n"
        );
    }
}
