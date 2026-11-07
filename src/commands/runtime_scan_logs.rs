//! Read-only scan-log discovery and scanner ID lookup command adapters.
use super::Invocation;
use crate::{
    cli::ScanLogsCommand,
    error::CliError,
    output::Format,
    runtime::scan_logs::{self, CollectionOptions, Limits},
};
use serde_json::json;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanLogsPlan {
    Query {
        options: CollectionOptions,
        filter: String,
        all: bool,
        limit: u64,
        offset: u64,
        format: Option<Format>,
    },
    Lookup {
        scanner: Option<String>,
        ids: Vec<String>,
        reports: bool,
        limits: Limits,
        format: Option<Format>,
    },
}
pub fn plan(
    command: &ScanLogsCommand,
    format: Option<Format>,
    debug: bool,
) -> Result<ScanLogsPlan, CliError> {
    if debug {
        return Err(CliError::usage(
            "Disable --debug for scan-log evidence reads",
        ));
    }
    match command {
        ScanLogsCommand::Results(args) => {
            if let Some(name) = &args.scanner {
                crate::tenant::scanners::validate_name(name)?;
            }
            scan_logs::valid_ids(&args.ids, true)?;
            Ok(ScanLogsPlan::Lookup {
                scanner: args.scanner.clone(),
                ids: args.ids.clone(),
                reports: false,
                limits: crate::runtime::evidence_limits(&args.budget)?,
                format,
            })
        }
        ScanLogsCommand::Reports(args) => {
            if let Some(name) = &args.scanner {
                crate::tenant::scanners::validate_name(name)?;
            }
            scan_logs::valid_ids(&args.ids, false)?;
            Ok(ScanLogsPlan::Lookup {
                scanner: args.scanner.clone(),
                ids: args.ids.clone(),
                reports: true,
                limits: crate::runtime::evidence_limits(&args.budget)?,
                format,
            })
        }
        ScanLogsCommand::Query(args) => {
            if args.page.is_some() || args.page_size.is_some() {
                return Err(CliError::usage(
                    "--page and --page-size are no longer supported; use --limit and --offset for the normalized output slice",
                ));
            }
            if args.all && (args.limit.is_some() || args.offset.is_some()) {
                return Err(CliError::usage("--all conflicts with --limit and --offset"));
            }
            let interval = scan_logs::number(args.interval.as_deref(), 24, 1, 87600, "--interval")?;
            let unit = args.unit.clone().unwrap_or_else(|| "hours".into());
            if !["hour", "hours", "day", "days"].contains(&unit.as_str()) {
                return Err(CliError::usage("--unit must be hour, hours, day or days"));
            }
            if args.app_ids.len() > 1000
                || args.app_ids.iter().any(|id| {
                    id.trim().is_empty() || id.len() > 4096 || id.chars().any(char::is_control)
                })
            {
                return Err(CliError::usage(
                    "Supply at most 1000 nonempty --app-id values without controls",
                ));
            }
            let filter = args.filter.clone().unwrap_or_else(|| "all".into());
            if !["all", "benign", "threat"].contains(&filter.as_str()) {
                return Err(CliError::usage("--filter must be all, benign or threat"));
            }
            Ok(ScanLogsPlan::Query {
                options: CollectionOptions {
                    limits: crate::runtime::evidence_limits(&args.budget)?,
                    interval,
                    unit,
                    app_ids: args.app_ids.clone(),
                    max_pages: scan_logs::number(
                        args.max_pages.as_deref(),
                        40,
                        1,
                        100,
                        "--max-pages",
                    )?,
                    max_events: scan_logs::number(
                        args.max_events.as_deref(),
                        10000,
                        1,
                        10000,
                        "--max-events",
                    )?,
                    max_reports: 0,
                },
                filter,
                all: args.all,
                limit: scan_logs::number(args.limit.as_deref(), 50, 1, 100, "--limit")?,
                offset: scan_logs::number(args.offset.as_deref(), 0, 0, 9999, "--offset")?,
                format,
            })
        }
    }
}
pub async fn run(inv: &mut Invocation, plan: &ScanLogsPlan) -> Result<(), CliError> {
    match plan {
        ScanLogsPlan::Lookup {
            scanner: scanner_name,
            ids,
            reports,
            limits,
            format,
        } => {
            let tenant = inv.load_tenant()?;
            let (scanner_config, _) = crate::tenant::scanners::select(
                &tenant.config,
                scanner_name.as_deref(),
                Some(tenant.name()),
            )?;
            let budget = limits.budget(inv.session.clone());
            budget.stage(2);
            let (scanner, _) = crate::clients::build_scanner_bounded(
                &scanner_config,
                &inv.session,
                None,
                Some(&budget),
            )?;
            let (raw, missing, notes, _) = scan_logs::lookup(
                &scanner,
                &budget,
                ids,
                *reports,
                &inv.secrets,
                Some(&inv.payload_spans),
            )
            .await;
            if inv.session.is_cancelled() {
                return Err(CliError::Interrupted(
                    crate::error::InterruptContext::NoDataWritten,
                ));
            }
            let rows = if *reports {
                scan_logs::report_views(&raw, &inv.payload_spans)
            } else {
                raw.iter().map(scan_logs::result_view).collect()
            };
            if inv.payload_spans.exceeded() {
                return Err(crate::report::signal::invalid());
            }
            let complete = missing.is_empty() && notes.is_empty();
            let mut document = json!({"kind":if *reports{"scan_reports"}else{"scan_results"},"requested_ids":ids,"items":rows,"missing_ids":missing,"notes":notes,"complete":complete,"attempts":budget.attempts()});
            scan_logs::project_document(&mut document, &inv.secrets, &inv.payload_spans);
            let format =
                crate::output::resolve_tenant_default(*format, tenant.config.default_output());
            emit(inv, &document, format, complete)
        }
        ScanLogsPlan::Query {
            options,
            filter,
            all,
            limit,
            offset,
            format,
        } => {
            let tenant = inv.load_tenant()?;
            let format =
                crate::output::resolve_tenant_default(*format, tenant.config.default_output());
            let mut evidence = crate::report::signal::collect::collect(
                &inv.collect_context(),
                &tenant,
                options,
                false,
            )
            .await?;
            evidence.events.sort_by(|a, b| {
                b.timestamp_ms
                    .cmp(&a.timestamp_ms)
                    .then(a.identity.cmp(&b.identity))
            });
            let eligible: Vec<_> = evidence
                .events
                .iter()
                .filter(|event| {
                    event.timestamp_ms.is_some_and(|at| {
                        at >= evidence.window.start_ms && at < evidence.window.end_ms
                    })
                })
                .filter(|event| match filter.as_str() {
                    "benign" => event.verdict == "benign",
                    "threat" => event.verdict == "malicious",
                    _ => true,
                })
                .collect();
            let items: Vec<_> = if *all {
                eligible
            } else {
                eligible
                    .into_iter()
                    .skip(*offset as usize)
                    .take(*limit as usize)
                    .collect()
            };
            let complete = evidence.incomplete() == 0;
            let mut document = json!({"kind":"scan_log_discovery","items":items,"scope":evidence.app_ids,"window":evidence.window,"coverage":evidence.sources,"complete":complete,"slice":{"all":all,"limit":limit,"offset":offset},"budgets":evidence.budgets});
            scan_logs::project_document(&mut document, &inv.secrets, &inv.payload_spans);
            emit(inv, &document, format, complete)
        }
    }
}
fn emit(
    inv: &mut Invocation,
    value: &serde_json::Value,
    format: Format,
    complete: bool,
) -> Result<(), CliError> {
    let document = crate::json::to_ordered(value).map_err(|_| crate::report::signal::invalid())?;
    crate::runtime::views::render_dashboard(
        &mut inv.output,
        &document,
        format,
        crate::runtime::views::DashTable::Envelope,
    )?;
    if !complete {
        inv.output
            .status("Evidence retrieval incomplete; review coverage and missing IDs.");
        inv.output.commit_data()?;
        return Err(CliError::Reported(1));
    }
    Ok(())
}
