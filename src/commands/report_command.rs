//! The command stages shared by the HTML environment reports, `redteam report`
//! (DESIGN §15.17), `runtime report` (§18.14), `gateway report` (§19.15) and
//! `supplychain report` (§20.9): the destination checks before the
//! tenant (D214, D226), the per-source read budget with `with_retries(0)` and the G7
//! re-read, the interruption context before the report exists, the publishing of the
//! HTML (one new 0600 file with its status lines, or stdout), the incomplete-source line
//! and exit code, and the single failure line every other failure becomes.
//!
//! Each report keeps only its collector, its reader and its collection status line.

use super::Invocation;
use crate::error::{CliError, InterruptContext};
use crate::json::OValue;
use crate::report::ReadError;
pub use airs_runtime_core::time::now_ms;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// Where the HTML goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReportDestination {
    Stdout,
    /// As given (made absolute when the command runs, TS `resolve`).
    File(PathBuf),
}

/// Whether the destination's parent is checked before the tenant: `runtime report` does
/// (D214, choice 29), `redteam report` finds a missing parent at the final write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParentCheck {
    Checked,
    AtWrite,
}

/// The only failure text of an environment report (TS; no path, OS error or server
/// text).
pub const GENERATION_FAILED: &str = "✗ Report generation failed. Verify Management API configuration and a writable, new output path. Existing files are never overwritten.";

/// What an interruption before the report exists reports.
pub const NO_REPORT: &str = "no report was written";

/// `Interrupted; no report was written`.
pub fn no_report() -> CliError {
    CliError::Interrupted(InterruptContext::Context(NO_REPORT.to_owned()))
}

/// An error of the tenant and client steps: an interruption becomes [`no_report`].
pub fn before_report(error: CliError) -> CliError {
    match error {
        CliError::Interrupted(_) => no_report(),
        other => other,
    }
}

/// Any failure that [`conclude`] turns into [`GENERATION_FAILED`].
fn failed() -> CliError {
    CliError::operational("report generation failed")
}

/// A path for the status line: `\p{Cc}\p{Cf}` → `_` (TS).
pub fn status_path(path: &Path) -> String {
    path.display()
        .to_string()
        .chars()
        .map(|c| {
            if crate::report::is_control_or_format(c) {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// The destination checks before the tenant and any request: a path that names no file
/// (`sub/`, `a/.`; D226), anything at the absolute path (a dangling symlink included)
/// and, with [`ParentCheck::Checked`], a parent that is not an existing directory (D214)
/// are failures. `None` for stdout.
pub fn check_destination(
    destination: &ReportDestination,
    parent: ParentCheck,
) -> Result<Option<PathBuf>, CliError> {
    let ReportDestination::File(path) = destination else {
        return Ok(None);
    };
    if !crate::files::names_a_file(path) {
        return Err(failed());
    }
    let path = std::path::absolute(path).map_err(|_| failed())?;
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(failed()),
    }
    if parent == ParentCheck::Checked {
        let parent = path.parent().ok_or_else(failed)?;
        if !std::fs::metadata(parent).is_ok_and(|metadata| metadata.is_dir()) {
            return Err(failed());
        }
    }
    Ok(Some(path))
}

/// How a collected report ends: its incomplete sources, whether its health could not be
/// assessed, and `--strict`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub incomplete: usize,
    pub health_unknown: bool,
    pub strict: bool,
}

/// Publish the rendered `html` once the collection ended: a signal that already arrived
/// writes nothing ([`no_report`]); a file is published new at 0600 (the interruption
/// context then names it; a directory that could not be synced gets the §3.9 warning),
/// stdout is committed (a signal during the write is `OutputIncomplete`). Then the
/// incomplete-source line; `Ok(exit code)`: 1 when the health is unknown or `--strict`
/// meets an incomplete source.
pub fn publish(
    invocation: &mut Invocation,
    path: Option<&Path>,
    html: &str,
    verdict: Verdict,
) -> Result<i32, CliError> {
    publish_with_path_label(invocation, path, html, verdict, None)
}
pub fn publish_with_path_label(
    invocation: &mut Invocation,
    path: Option<&Path>,
    html: &str,
    verdict: Verdict,
    label: Option<&str>,
) -> Result<i32, CliError> {
    if invocation.output.pending_signal().is_some() {
        return Err(no_report());
    }
    match path {
        Some(path) => {
            let published = crate::files::publish_new(path, html.as_bytes(), "--output-file")
                .map_err(|_| failed())?;
            let shown = label
                .map(str::to_owned)
                .unwrap_or_else(|| status_path(path));
            invocation.interrupt = Some(InterruptContext::Context(format!(
                "the report was saved in {shown}"
            )));
            if published == crate::files::Published::DirectoryNotSynced {
                if label.is_some() {
                    invocation.output.warning(&format!("Report at {shown} was saved, but its directory could not be synced; it may not survive a system crash"));
                } else {
                    invocation
                        .output
                        .warning(&crate::store::atomic::dir_sync_warning(path));
                }
            }
            invocation
                .output
                .status(&format!("Report → {shown} (mode 0600)"));
        }
        None => {
            invocation.output.data(html);
            if invocation.output.commit().is_err() {
                if invocation.output.pending_signal().is_some() {
                    return Err(CliError::Interrupted(InterruptContext::OutputIncomplete));
                }
                return Err(failed());
            }
        }
    }
    if verdict.incomplete > 0 {
        invocation.output.status(&format!(
            "{} source(s) incomplete; review the evidence section.",
            verdict.incomplete
        ));
    }
    Ok(
        if verdict.health_unknown || (verdict.strict && verdict.incomplete > 0) {
            crate::error::EXIT_OPERATIONAL
        } else {
            0
        },
    )
}

/// The end of a report run: `Ok(0)` succeeds, another code exits with it after the
/// output is committed, an interruption keeps its context, and any other failure after
/// the argument checks prints only [`GENERATION_FAILED`] (red, exit 1).
pub fn conclude(
    invocation: &mut Invocation,
    result: Result<i32, CliError>,
) -> Result<(), CliError> {
    conclude_with(invocation, result, GENERATION_FAILED)
}

/// [`conclude`] with the report's own failure line (`supplychain report`, D311).
pub fn conclude_with(
    invocation: &mut Invocation,
    result: Result<i32, CliError>,
    failure: &str,
) -> Result<(), CliError> {
    match result {
        Ok(0) => Ok(()),
        Ok(code) => {
            let _ = invocation.output.commit();
            Err(CliError::Reported(code))
        }
        Err(CliError::Interrupted(context)) => Err(CliError::Interrupted(context)),
        Err(_) => {
            invocation.output.discard();
            let line = invocation
                .output
                .paint_err(crate::render::style::Style::Red, failure);
            invocation.output.write_stderr(&format!("  {line}\n"));
            Err(CliError::Reported(crate::error::EXIT_OPERATIONAL))
        }
    }
}

/// The read budget of a report: one options value per source (built when the source's
/// first read is sent, so its walk shares one budget) by `build`.
pub struct SourceBudget {
    build: fn() -> scm::Result<scm::RequestOptions>,
    current: RefCell<Option<(&'static str, scm::RequestOptions)>>,
}

impl SourceBudget {
    pub fn new(build: fn() -> scm::Result<scm::RequestOptions>) -> Self {
        Self {
            build,
            current: RefCell::new(None),
        }
    }

    /// The options of `source`: the running ones while the same source reads on, new
    /// ones for the next source.
    fn options(&self, source: &'static str) -> Result<scm::RequestOptions, ReadError> {
        let mut current = self.current.borrow_mut();
        if let Some((name, options)) = current.as_ref()
            && *name == source
        {
            return Ok(options.clone());
        }
        let options = (self.build)().map_err(|_| ReadError::Failed)?;
        *current = Some((source, options.clone()));
        Ok(options)
    }

    /// One read of `source` under its budget, re-read once after an HTTP 401 or 403
    /// rejection (G7); the answer as ordered JSON, a cancellation as
    /// [`ReadError::Cancelled`] and any other failure by its status only.
    pub async fn read<T, F, Fut>(&self, source: &'static str, call: F) -> Result<OValue, ReadError>
    where
        T: serde::Serialize,
        F: FnMut(scm::RequestOptions) -> Fut,
        Fut: std::future::Future<Output = scm::Result<T>>,
    {
        let answer = self.read_typed(source, call).await?;
        crate::json::to_ordered(&answer).map_err(|_| ReadError::Failed)
    }

    /// [`SourceBudget::read`] keeping the typed answer (the supplychain collector reads the
    /// SDK models, DESIGN §20.9).
    pub async fn read_typed<T, F, Fut>(
        &self,
        source: &'static str,
        mut call: F,
    ) -> Result<T, ReadError>
    where
        F: FnMut(scm::RequestOptions) -> Fut,
        Fut: std::future::Future<Output = scm::Result<T>>,
    {
        let options = self.options(source)?;
        match crate::clients::reread(|| call(options.clone())).await {
            Ok(answer) => Ok(answer),
            Err(error) if error.kind() == scm::ErrorKind::Cancelled => Err(ReadError::Cancelled),
            Err(error) => Err(match error.status() {
                Some(status) => ReadError::Status(status),
                None => ReadError::Failed,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The budget is kept while one source reads on and rebuilt for the next source.
    #[test]
    fn one_budget_per_source() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static BUILT: AtomicUsize = AtomicUsize::new(0);
        fn build() -> scm::Result<scm::RequestOptions> {
            BUILT.fetch_add(1, Ordering::SeqCst);
            scm::RequestOptions::with_timeout(std::time::Duration::from_secs(5))
        }
        let budget = SourceBudget::new(build);
        budget.options("a").unwrap();
        budget.options("a").unwrap();
        assert_eq!(BUILT.load(Ordering::SeqCst), 1);
        budget.options("b").unwrap();
        budget.options("a").unwrap();
        assert_eq!(BUILT.load(Ordering::SeqCst), 3);
    }

    /// A destination that names no file, an existing path and (when checked) a missing
    /// parent are refused; stdout needs no check.
    #[test]
    fn destination_checks() {
        let dir = crate::test_support::TempDir::new("report-dest");
        let file = |path: PathBuf| ReportDestination::File(path);
        assert_eq!(
            check_destination(&ReportDestination::Stdout, ParentCheck::Checked).unwrap(),
            None
        );
        let new = dir.join("r.html");
        assert_eq!(
            check_destination(&file(new.clone()), ParentCheck::Checked).unwrap(),
            Some(new.clone())
        );
        std::fs::write(&new, "x").unwrap();
        assert!(check_destination(&file(new), ParentCheck::AtWrite).is_err());
        assert!(check_destination(&file(dir.join("sub/")), ParentCheck::AtWrite).is_err());
        let orphan = dir.join("missing").join("r.html");
        assert!(check_destination(&file(orphan.clone()), ParentCheck::Checked).is_err());
        assert!(check_destination(&file(orphan), ParentCheck::AtWrite).is_ok());
    }
}
