//! Runtime Signal Brief: collect or load -> safe evidence -> analyze -> publish.
use super::{
    Invocation,
    report_command::{self as rc, ParentCheck, ReportDestination, Verdict},
};
use crate::{
    error::{CliError, InterruptContext},
    report::signal::{self, privacy},
    runtime::scan_logs::CollectionOptions,
};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeReportPlan {
    pub destination: ReportDestination,
    pub title: String,
    pub strict: bool,
    pub from_evidence: Option<PathBuf>,
    pub review_file: Option<PathBuf>,
    pub bundle_dir: Option<PathBuf>,
    pub collection: CollectionOptions,
}
pub fn default_name(now: std::time::SystemTime) -> String {
    format!(
        "airs-runtime-report-{}-{}.html",
        crate::debug::iso_timestamp(now).replace(':', "-"),
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    )
}
pub async fn run(inv: &mut Invocation, plan: &RuntimeReportPlan) -> Result<(), CliError> {
    let result = generate(inv, plan).await;
    rc::conclude(inv, result)
}
fn path_label(path: &std::path::Path, inv: &Invocation) -> String {
    privacy::text_with_spans(
        &rc::status_path(path),
        &inv.secrets,
        &inv.payload_spans,
        "output_path",
        4096,
    )
}
async fn generate(inv: &mut Invocation, plan: &RuntimeReportPlan) -> Result<i32, CliError> {
    let bundle = plan
        .bundle_dir
        .as_deref()
        .map(signal::bundle::preflight)
        .transpose()?;
    let path = if bundle.is_none() {
        rc::check_destination(&plan.destination, ParentCheck::Checked)?
    } else {
        None
    };
    // Discover all user-visible text before projecting evidence, including path labels.
    let _ = privacy::text(&plan.title, &inv.secrets, usize::MAX);
    if let Some(path) = bundle.as_ref().or(path.as_ref()) {
        let _ = privacy::text(&rc::status_path(path), &inv.secrets, usize::MAX);
    }
    let (evidence, saved, changed) = if let Some(input) = &plan.from_evidence {
        let loaded = signal::load(input, &inv.secrets, &inv.payload_spans)?;
        (loaded.evidence, loaded.original, loaded.changed)
    } else {
        let tenant = inv.load_tenant().map_err(rc::before_report)?;
        inv.output
            .status("Collecting runtime Signal Brief evidence (read-only)...");
        let evidence =
            signal::collect::collect(&inv.collect_context(), &tenant, &plan.collection, true)
                .await
                .map_err(rc::before_report)?;
        (evidence.clone(), evidence, false)
    };
    let review = plan
        .review_file
        .as_deref()
        .map(|path| signal::review::load(path, &saved, &evidence, &inv.secrets, &inv.payload_spans))
        .transpose()?;
    let html = signal::html::render_reviewed(
        &evidence,
        &plan.title,
        &inv.secrets,
        &inv.payload_spans,
        review.as_ref(),
    )?;
    let verdict = Verdict {
        incomplete: evidence.incomplete(),
        health_unknown: evidence.health_unknown(),
        strict: plan.strict,
    };
    if let Some(dir) = bundle {
        let mut verify = saved.clone();
        let additional =
            privacy::project_with_spans(&mut verify, &inv.secrets, &inv.payload_spans)?;
        if changed || additional {
            return Err(CliError::operational(
                "Saved evidence requires additional privacy projection; use HTML-only output",
            ));
        }
        if inv.session.is_cancelled() || inv.output.pending_signal().is_some() {
            return Err(rc::no_report());
        }
        let json = serde_json::to_string_pretty(&saved).map_err(|_| signal::invalid())?;
        let packet = signal::review::packet_with_review(&evidence, review.as_ref())?;
        let warning = signal::bundle::publish(&dir, &html, &json, &packet)?;
        let label = path_label(&dir, inv);
        inv.interrupt = Some(InterruptContext::Context(format!(
            "the report bundle was saved in {label}"
        )));
        if warning {
            inv.output.warning(&format!(
                "Bundle at {label} was saved, but directory durability could not be confirmed"
            ));
        }
        inv.output.status(&format!(
            "Report bundle → {label} (directory 0700; files 0600)"
        ));
        if verdict.incomplete > 0 {
            inv.output.status(&format!(
                "{} required source(s) incomplete; review the evidence section.",
                verdict.incomplete
            ));
        }
        return Ok(
            if verdict.health_unknown || (verdict.strict && verdict.incomplete > 0) {
                1
            } else {
                0
            },
        );
    }
    let label = path.as_ref().map(|path| path_label(path, inv));
    rc::publish_with_path_label(inv, path.as_deref(), &html, verdict, label.as_deref())
}
