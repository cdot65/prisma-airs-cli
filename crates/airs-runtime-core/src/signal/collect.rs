//! Bounded SDK-backed evidence collection; no legacy logs or stored content reads.
use super::sink::{CollectContext, HistorySink};
use super::{evidence::*, invalid, privacy};
use crate::{
    clients::{ManagementClients, Planes},
    error::CliError,
    read_budget::ReadBudget,
    scan_logs::{self, CollectionOptions},
};
use scm::airs::management::{
    DashboardAppQuery, DashboardApplicationsOverviewQuery, DashboardSessionQuery,
    DashboardSessionTransactionQuery, DashboardSessionsOverviewQuery, PaginationOptions,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};
fn now() -> i64 {
    crate::time::now_ms()
}
fn source(name: &str, unit: &str, required: bool) -> Source {
    Source {
        name: name.into(),
        status: Status::Complete,
        required,
        records: 0,
        pages: 0,
        unit: unit.into(),
        population: "selected application scope; collected API observations".into(),
        started_ms: now(),
        ended_ms: now(),
        notes: vec![],
    }
}
fn fail(s: &mut Source, note: impl Into<String>) {
    s.status = if s.records == 0 {
        Status::Unavailable
    } else {
        Status::Partial
    };
    s.notes.push(note.into());
    s.ended_ms = now();
}
fn merge_source(sources: &mut Vec<Source>, incoming: Source) {
    if let Some(existing) = sources.iter_mut().find(|s| s.name == incoming.name) {
        let complete = existing.status == Status::Complete && incoming.status == Status::Complete;
        existing.records += incoming.records;
        existing.pages += incoming.pages;
        existing.started_ms = existing.started_ms.min(incoming.started_ms);
        existing.ended_ms = existing.ended_ms.max(incoming.ended_ms);
        existing.notes.extend(incoming.notes);
        existing.status = if complete {
            Status::Complete
        } else if existing.records > 0 {
            Status::Partial
        } else {
            Status::Unavailable
        };
    } else {
        sources.push(incoming);
    }
}
fn string(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}
fn number(v: &Value, key: &str) -> Option<u64> {
    v.get(key)
        .and_then(Value::as_u64)
        .filter(|n| *n <= crate::limits::MAX_SAFE_INTEGER)
}
fn model<T: Serialize>(v: T) -> scm::Result<Value> {
    serde_json::to_value(v).map_err(|_| scm::Error::new(scm::ErrorKind::Decode))
}
fn selected(options: &CollectionOptions, id: &str) -> bool {
    options.app_ids.is_empty() || options.app_ids.iter().any(|v| v == id)
}
fn page_valid(
    value: &Value,
    offset: u64,
    size: usize,
    total: &mut Option<u64>,
    key: &str,
    limit: u64,
) -> Result<bool, &'static str> {
    let Some(meta) = value.get("pagination") else {
        return Err("Missing pagination metadata");
    };
    let Some(count) = number(meta, "total_items") else {
        return Err("Invalid total count");
    };
    if number(meta, "skip") != Some(offset)
        || number(meta, "limit") != Some(limit)
        || total.is_some_and(|old| old != count)
        || size as u64 > limit
        || offset + size as u64 > count
        || (size == 0 && offset < count)
    {
        return Err("Pagination metadata changed or did not advance");
    }
    if value.get(key).and_then(Value::as_array).is_none() {
        return Err("Missing record array does not establish zero activity");
    }
    *total = Some(count);
    Ok(offset + size as u64 >= count)
}
fn initial(
    tenant: &crate::tenant::TenantContext,
    o: &CollectionOptions,
    end: i64,
    version: &str,
) -> Result<Evidence, CliError> {
    let duration = o
        .interval
        .checked_mul(if o.unit.starts_with("hour") {
            3_600_000
        } else {
            86_400_000
        })
        .and_then(|v| i64::try_from(v).ok())
        .ok_or_else(invalid)?;
    Ok(Evidence {
        schema_version: 1,
        rules_version: 2,
        redaction_version: 1,
        collection_id: uuid::Uuid::new_v4().to_string(),
        digest: String::new(),
        cli_version: version.into(),
        sdk_version: "0.2.4".into(),
        tenant: tenant.name().into(),
        app_ids: o.app_ids.clone(),
        window: Window {
            start_ms: end.checked_sub(duration).ok_or_else(invalid)?,
            end_ms: end,
            interval: o.interval,
            unit: o.unit.clone(),
        },
        collected_ms: end,
        sources: vec![],
        events: vec![],
        profiles: vec![],
        applications: vec![],
        metrics: vec![],
        sampling: Sampling {
            algorithm: "coverage-greedy-v2".into(),
            candidates: 0,
            selected_report_ids: vec![],
            uncovered_groups: vec![],
            limit: o.max_reports,
        },
        budgets: Budgets {
            seconds: o.limits.seconds,
            concurrency: o.limits.concurrency,
            requests: o.limits.requests,
            max_pages: o.max_pages,
            max_events: o.max_events,
            max_reports: o.max_reports,
            attempts: 0,
        },
    })
}

pub async fn collect(
    inv: &CollectContext<'_>,
    tenant: &crate::tenant::TenantContext,
    o: &CollectionOptions,
    configuration: bool,
) -> Result<Evidence, CliError> {
    collect_mode(inv, tenant, o, configuration, None)
        .await
        .map(|c| c.evidence)
}

pub struct ManagementCollection {
    pub evidence: Evidence,
    pub clients: Option<ManagementClients>,
    pub telemetry_buckets: BTreeSet<(String, String)>,
}

pub async fn collect_management(
    inv: &CollectContext<'_>,
    tenant: &crate::tenant::TenantContext,
    o: &CollectionOptions,
    history: &dyn HistorySink,
) -> Result<ManagementCollection, CliError> {
    collect_mode(inv, tenant, o, true, Some(history)).await
}

pub async fn complete_management(
    inv: &CollectContext<'_>,
    o: &CollectionOptions,
    history: &dyn HistorySink,
    clients: Option<&ManagementClients>,
    buckets: &BTreeSet<(String, String)>,
    e: &mut Evidence,
) -> Result<(), CliError> {
    if let Some(clients) = clients {
        transaction_metadata(clients, history.budget(), o, e, inv.secrets).await;
        application_telemetry(clients, history.budget(), o, e, buckets).await;
    }
    e.budgets.attempts = history.budget().attempts();
    e.collected_ms = now();
    e.seal_native()?;
    history.checkpoint(e)
}

async fn collect_mode(
    inv: &CollectContext<'_>,
    tenant: &crate::tenant::TenantContext,
    o: &CollectionOptions,
    configuration: bool,
    history: Option<&dyn HistorySink>,
) -> Result<ManagementCollection, CliError> {
    let budget = if let Some(history) = history {
        history.budget().clone()
    } else if configuration {
        ReadBudget::with_stages(
            o.limits.seconds,
            o.limits.requests,
            o.limits.concurrency,
            inv.session.clone(),
            [20, 70, 100],
        )
    } else {
        o.limits.budget(inv.session.clone())
    };
    let mut e = initial(tenant, o, now(), inv.version)?;
    if let Some(history) = history {
        e.collection_id = history.run_id().to_owned();
    }
    let management = ManagementClients::build_for_bounded(
        &tenant.config,
        inv.session,
        inv.secrets,
        None,
        Planes {
            dashboard: true,
            dlp: false,
        },
        Some(&budget),
    );
    let mut telemetry_buckets = BTreeSet::new();
    match &management {
        Ok(clients) => {
            if configuration {
                telemetry_buckets =
                    configuration_reads(clients, &budget, o, &mut e, history.is_some()).await;
                if let Some(history) = history {
                    history.checkpoint(&e)?;
                }
                budget.stage(1);
            }
            discover(clients, &budget, o, &mut e, history).await?;
        }
        Err(_) => {
            let mut s = source("sessions", "session inventory rows", true);
            fail(
                &mut s,
                "Management configuration unavailable; no environment fallback used",
            );
            e.sources.push(s);
            if configuration {
                for name in ["applications", "profiles", "registered-apps"] {
                    let mut s = source(name, "configuration rows", true);
                    fail(&mut s, "Management configuration unavailable");
                    e.sources.push(s);
                }
            }
        }
    }
    budget.stage(2);
    if history.is_none() {
        let mut groups: BTreeMap<String, Vec<Event>> = BTreeMap::new();
        for event in std::mem::take(&mut e.events) {
            groups
                .entry(event.identity.app_id.clone())
                .or_default()
                .push(event);
        }
        let mut scanners = BTreeMap::new();
        for application in groups.keys() {
            let configured = tenant.config.scanner_for_application(application);
            let scanner = configured.and_then(|(_, binding)| {
                crate::clients::build_scanner_bounded(
                    &binding.overlay(&tenant.config),
                    inv.session,
                    None,
                    Some(&budget),
                )
                .ok()
            });
            match scanner {
                Some(scanner) => {
                    scanners.insert(application.clone(), scanner);
                }
                None => {
                    let mut s = source("scan-results", "correlated scan result rows", true);
                    fail(
                        &mut s,
                        format!(
                            "Application {application}: no usable application-bound scanner; management evidence retained. Use 'airs tenant scanners bind-default <tenant> <name> --app-id <app-id>' or 'airs tenant scanners set <tenant> <name> --file <private-file>'. Unbound flat credentials are never tried."
                        ),
                    );
                    let requested = groups[application]
                        .iter()
                        .filter(|event| scan_logs::is_scan_id(&event.identity.scan_id))
                        .map(|event| &event.identity.scan_id)
                        .collect::<BTreeSet<_>>()
                        .len() as u64;
                    s.notes.push(lookup_diagnostics(
                        application,
                        &scan_logs::LookupDiagnostics {
                            requested_ids: requested,
                            missing_ids: requested,
                            ..Default::default()
                        },
                        0,
                        0,
                    ));
                    merge_source(&mut e.sources, s);
                }
            }
        }
        let jobs = groups.into_iter().map(|(application, mut events)| {
            let scanner = scanners.get(&application);
            let budget = &budget;
            let secrets = inv.secrets;
            async move {
                let s = match scanner {
                    Some((scanner, status)) => {
                        Some(enrich(scanner, status, budget, &mut events, secrets).await)
                    }
                    None => None,
                };
                (events, s)
            }
        });
        for (events, s) in futures_util::future::join_all(jobs).await {
            e.events.extend(events);
            if let Some(s) = s {
                merge_source(&mut e.sources, s);
            }
        }
        if e.events.is_empty() && !e.sources.iter().any(|s| s.name == "scan-results") {
            e.sources
                .push(source("scan-results", "correlated scan result rows", true));
        }
        if let Ok(clients) = &management {
            transaction_metadata(clients, &budget, o, &mut e, inv.secrets).await;
        }
        if configuration {
            select_reports(&mut e, o.max_reports);
            let mut owners: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for event in &e.events {
                for id in &event.report_ids {
                    owners
                        .entry(id.clone())
                        .or_default()
                        .insert(event.identity.app_id.clone());
                }
            }
            let ambiguous: BTreeSet<_> = e
                .sampling
                .selected_report_ids
                .iter()
                .filter(|id| owners.get(*id).is_some_and(|apps| apps.len() != 1))
                .cloned()
                .collect();
            if !ambiguous.is_empty() {
                e.sampling
                    .selected_report_ids
                    .retain(|id| !ambiguous.contains(id));
                e.sampling.uncovered_groups.push(
                    "Ambiguous report ownership across applications; no credential was tried"
                        .into(),
                );
            }
            let mut groups: BTreeMap<String, Vec<Event>> = BTreeMap::new();
            for event in std::mem::take(&mut e.events) {
                groups
                    .entry(event.identity.app_id.clone())
                    .or_default()
                    .push(event);
            }
            let mut order = Vec::new();
            for id in &e.sampling.selected_report_ids {
                if let Some(app) = owners.get(id).and_then(|apps| apps.first())
                    && !order.contains(app)
                {
                    order.push(app.clone());
                }
            }
            for application in order {
                let Some(events) = groups.get_mut(&application) else {
                    continue;
                };
                let Some((scanner, status)) = scanners.get(&application) else {
                    continue;
                };
                let ids: Vec<_> = e
                    .sampling
                    .selected_report_ids
                    .iter()
                    .filter(|id| {
                        owners
                            .get(*id)
                            .is_some_and(|apps| apps.contains(&application))
                    })
                    .cloned()
                    .collect();
                if o.max_reports > 0 && !ids.is_empty() {
                    let s = details(
                        scanner,
                        status,
                        &budget,
                        events,
                        &ids,
                        inv.secrets,
                        inv.payload_spans,
                    )
                    .await;
                    merge_source(&mut e.sources, s);
                }
            }
            for (_, events) in groups {
                e.events.extend(events);
            }
            if o.max_reports > 0 && !e.sources.iter().any(|s| s.name == "scan-reports") {
                let mut s = source("scan-reports", "selected detector detail reports", false);
                s.notes.push("No eligible unambiguous application-bound report candidates; no report API calls made".into());
                e.sources.push(s);
            }
        }
    }
    e.events.sort_by(|a, b| a.identity.cmp(&b.identity));
    if configuration
        && history.is_none()
        && let Ok(clients) = &management
    {
        application_telemetry(clients, &budget, o, &mut e, &telemetry_buckets).await;
        if let Some(history) = history {
            history.checkpoint(&e)?;
        }
    }
    let missing_times = e
        .events
        .iter()
        .filter(|event| event.timestamp_ms.is_none())
        .count();
    let outside = e
        .events
        .iter()
        .filter(|event| {
            event
                .timestamp_ms
                .is_some_and(|at| at < e.window.start_ms || at >= e.window.end_ms)
        })
        .count();
    if missing_times + outside > 0 {
        let mut eligibility = source("event-window", "timestamp-eligible events", true);
        eligibility.status = Status::Partial;
        eligibility.records = e.events.len().saturating_sub(missing_times + outside) as u64;
        eligibility.notes.push(format!("{missing_times} events have unknown timestamps; {outside} events fall outside the target window; excluded from eligible output and totals"));
        e.sources.push(eligibility);
    }
    if inv.session.is_cancelled() {
        return Err(CliError::Interrupted(
            crate::error::InterruptContext::NoDataWritten,
        ));
    }
    for source in &mut e.sources {
        if source.status == Status::Unavailable && source.records > 0 {
            source.status = Status::Partial;
        }
    }
    e.collected_ms = now();
    e.budgets.attempts = budget.attempts();
    if history.is_none() {
        privacy::project_with_spans(&mut e, inv.secrets, inv.payload_spans)?;
    }
    let mut stored = source("evidence-storage", "retained normalized observations", true);
    let mut truncated = e.metrics.len() > 10000;
    e.metrics.truncate(10000);
    loop {
        let bytes = serde_json::to_vec(&e).map_err(|_| invalid())?.len();
        if bytes <= 24 * 1024 * 1024 {
            break;
        }
        truncated = true;
        if !e.events.is_empty() {
            e.events.truncate(e.events.len() / 2);
        } else if !e.profiles.is_empty() {
            e.profiles.truncate(e.profiles.len() / 2);
        } else if !e.metrics.is_empty() {
            e.metrics.truncate(e.metrics.len() / 2);
        } else if !e.applications.is_empty() {
            e.applications.truncate(e.applications.len() / 2);
        } else {
            return Err(invalid());
        }
    }
    if truncated {
        stored.status = Status::Partial;
        stored.records = e.events.len() as u64;
        stored.notes.push("Normalized evidence storage cap truncated retained observations; source counts describe API reads before storage truncation".into());
        let reports: BTreeSet<_> = e
            .events
            .iter()
            .flat_map(|ev| ev.report_ids.iter())
            .collect();
        e.sampling
            .selected_report_ids
            .retain(|id| reports.contains(id));
        e.sources.push(stored);
    }
    for source in &mut e.sources {
        source.notes.sort();
        source.notes.dedup();
    }
    if history.is_some() {
        e.seal_native()?;
    } else {
        e.seal()?;
    }
    Ok(ManagementCollection {
        evidence: e,
        clients: management.ok(),
        telemetry_buckets,
    })
}

fn positive_counts(value: &Value) -> bool {
    ["total", "critical", "high", "medium", "low"]
        .iter()
        .any(|key| number(value, key).is_some_and(|n| n > 0))
}

fn management_hints(value: &Value) -> Vec<String> {
    let mut hints: Vec<_> = value
        .get("detection_type_violation_breakdown")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(1000)
        .filter(|row| row.get("violation_breakdown").is_some_and(positive_counts))
        .filter_map(|row| row.get("detection_type").and_then(Value::as_str))
        .filter(|name| {
            !name.is_empty() && name.len() <= 4096 && !name.chars().any(char::is_control)
        })
        .map(|name| format!("unknown/{name}"))
        .collect();
    hints.sort();
    hints.dedup();
    hints
}

fn positive_breakdown(value: &Value) -> bool {
    value.get("violation_status").and_then(Value::as_str) == Some("violated")
        || value
            .get("violation_breakdown")
            .is_some_and(positive_counts)
        || !management_hints(value).is_empty()
}

async fn discover(
    clients: &ManagementClients,
    budget: &Arc<ReadBudget>,
    o: &CollectionOptions,
    e: &mut Evidence,
    history: Option<&dyn HistorySink>,
) -> Result<(), CliError> {
    let event_limit = history.map_or(o.max_events, |h| h.current_event_limit(o.max_events));
    let dashboard = clients.management.dashboard();
    let mut inventory = source("sessions", "session inventory rows", true);
    let mut sessions = VecDeque::new();
    let mut identities = BTreeSet::new();
    let mut positive_sessions = BTreeSet::new();
    let mut session_metric_bytes = 0usize;
    let mut action_hint_bytes = 0usize;
    let mut total = None;
    let mut offset = 0;
    let mut finished = false;
    for _ in 0..o.max_pages {
        let dashboard = &dashboard;
        let mut q = DashboardSessionsOverviewQuery::default();
        q.time_interval = Some(o.interval);
        q.time_unit = Some(o.unit.clone());
        q.limit = Some(25);
        q.offset = Some(offset);
        let q = &q; // the read may be replayed once (explicit 401 replay)
        let result = budget
            .run(|opts| async move {
                dashboard
                    .sessions_overview(q, Some(&opts))
                    .await
                    .and_then(model)
            })
            .await;
        let value = match result {
            Ok(v) => v,
            Err(err) => {
                fail(&mut inventory, scan_logs::note(&err));
                break;
            }
        };
        inventory.pages += 1;
        let Some(items) = value.get("items").and_then(Value::as_array) else {
            fail(
                &mut inventory,
                "Missing record array does not establish zero activity",
            );
            break;
        };
        let mut shifted = false;
        let done = match page_valid(&value, offset, items.len(), &mut total, "items", 25) {
            Ok(v) => v,
            Err(reason) => {
                fail(&mut inventory, reason);
                if history.is_some() {
                    shifted = true;
                    false
                } else {
                    break;
                }
            }
        };
        for item in items {
            let key = (
                string(item, "application_id"),
                string(item, "application_name"),
                string(item, "session_id"),
            );
            if key.0.is_empty() || key.1.is_empty() || key.2.is_empty() {
                fail(&mut inventory, "Session has incomplete query identity");
                continue;
            }
            if !selected(o, &key.0) {
                continue;
            }
            if !identities.insert(key.clone()) {
                fail(
                    &mut inventory,
                    "Repeated session identity; excluded duplicate",
                );
                continue;
            }
            if sessions.len() as u64 >= event_limit {
                fail(
                    &mut inventory,
                    "Session discovery reached the event safety bound",
                );
                break;
            }
            inventory.records += 1;
            if item
                .get("detection_type_violation_breakdown")
                .and_then(Value::as_array)
                .is_some_and(|rows| rows.len() > 1000)
            {
                fail(
                    &mut inventory,
                    "Management session detector breakdown view cap reached",
                );
            }
            if positive_breakdown(item) {
                positive_sessions.insert(key.clone());
                let observations = std::iter::once((
                    "Management positive session observation".to_owned(),
                    "positive session observations (not scans)",
                ))
                .chain(management_hints(item).into_iter().map(|hint| {
                    (
                        format!("Management session detector-positive observation: {hint}"),
                        "positive session/detector observations (not scans)",
                    )
                }));
                for (name, unit) in observations {
                    let metric = Metric {
                        name,
                        value: Some(1.0),
                        unit: unit.into(),
                        scale: None,
                        population: format!("Application {} / {}; session {}", key.0, key.1, key.2),
                        window: format!(
                            "Rolling {} {} (API query); session-level observation",
                            o.interval, o.unit
                        ),
                        source: "sessions".into(),
                        status: Status::Complete,
                    };
                    let bytes = serde_json::to_vec(&metric).map_or(usize::MAX, |v| v.len());
                    if e.metrics.len() >= 10000
                        || session_metric_bytes.saturating_add(bytes) > 4 * 1024 * 1024
                    {
                        fail(
                            &mut inventory,
                            "Management session metadata storage cap reached; positive metadata is partial",
                        );
                        break;
                    }
                    session_metric_bytes += bytes;
                    e.metrics.push(metric);
                }
            }
            sessions.push_back((key, 0u64, None::<u64>));
        }
        if shifted {
            break;
        }
        if done {
            finished = true;
            break;
        }
        offset += items.len() as u64;
    }
    if !finished && inventory.status == Status::Complete {
        fail(
            &mut inventory,
            "Session inventory page cap reached; traversal incomplete",
        );
    }
    inventory.ended_ms = now();
    e.sources.push(inventory);
    if let Some(history) = history {
        history.checkpoint(e)?;
        history.sessions_discovered(
            &sessions
                .iter()
                .map(|(key, ..)| key.clone())
                .collect::<Vec<_>>(),
            &e.window,
        )?;
    }
    let mut details = source("session-details", "session actions", true);
    let mut events = BTreeSet::new();
    let mut pages = 0;
    type SessionWork = ((String, String, String), u64, Option<u64>);
    let mut per_app: BTreeMap<(String, String), VecDeque<SessionWork>> = BTreeMap::new();
    for work in sessions {
        per_app
            .entry((work.0.0.clone(), work.0.1.clone()))
            .or_default()
            .push_back(work);
    }
    for queue in per_app.values_mut() {
        queue
            .make_contiguous()
            .sort_by_key(|work| !positive_sessions.contains(&work.0));
    }
    let mut active: Vec<_> = per_app.keys().cloned().collect();
    active.sort_by_key(|key| {
        !per_app[key]
            .front()
            .is_some_and(|work| positive_sessions.contains(&work.0))
    });
    let mut active: VecDeque<_> = active.into();
    while !active.is_empty() {
        if pages >= o.max_pages || e.events.len() as u64 >= event_limit {
            fail(
                &mut details,
                "Aggregate session-detail page/event cap reached; remaining sessions uncollected",
            );
            break;
        }
        let mut jobs = Vec::new();
        for _ in 0..o.limits.concurrency.min(o.max_pages - pages) {
            let Some(app_key) = active.pop_front() else {
                break;
            };
            let queue = per_app.get_mut(&app_key).expect("scheduled app exists");
            let work = queue.pop_front().expect("scheduled session exists");
            if !queue.is_empty() {
                active.push_back(app_key.clone());
            }
            pages += 1;
            details.pages += 1;
            let dashboard = &dashboard;
            jobs.push(async move {
                let ((app_id, app_name, session), offset, _) = &work;
                let mut q = DashboardSessionQuery::new(session, app_id, app_name);
                q.time_interval = Some(o.interval);
                q.time_unit = Some(o.unit.clone());
                q.limit = Some(25);
                q.offset = Some(*offset);
                let q = &q; // the read may be replayed once (explicit 401 replay)
                let result =
                    budget
                        .run(|opts| async move {
                            dashboard.session(q, Some(&opts)).await.and_then(model)
                        })
                        .await;
                (app_key, work, result)
            });
        }
        let mut exhausted = false;
        for (app_key, ((app_id, app_name, session), offset, mut total), result) in
            futures_util::future::join_all(jobs).await
        {
            let value = match result {
                Ok(v) => v,
                Err(err) => {
                    fail(&mut details, scan_logs::note(&err));
                    if matches!(
                        err.kind(),
                        scm::ErrorKind::Deadline
                            | scm::ErrorKind::Limit
                            | scm::ErrorKind::Cancelled
                    ) {
                        exhausted = true;
                        continue;
                    }
                    continue;
                }
            };
            let Some(items) = value.get("session_actions").and_then(Value::as_array) else {
                fail(&mut details, "Session detail has no action array");
                continue;
            };
            if string(&value, "id") != session
                || value
                    .get("application")
                    .map(|v| (string(v, "id"), string(v, "name")))
                    != Some((app_id.clone(), app_name.clone()))
            {
                fail(
                    &mut details,
                    "Returned session identity does not match requested bucket",
                );
                continue;
            }
            let mut shifted = false;
            let done = match page_valid(
                &value,
                offset,
                items.len(),
                &mut total,
                "session_actions",
                25,
            ) {
                Ok(v) => v,
                Err(reason) => {
                    fail(&mut details, reason);
                    if history.is_some() {
                        shifted = true;
                        false
                    } else {
                        continue;
                    }
                }
            };
            let retained_before = details.records;
            for row in items {
                if e.events.len() as u64 >= event_limit {
                    fail(&mut details, "Event cap reached");
                    break;
                }
                if string(row, "application_id") != app_id
                    || string(row, "application_name") != app_name
                    || string(row, "session_id") != session
                {
                    fail(
                        &mut details,
                        "Session action identity conflicts with requested bucket",
                    );
                    continue;
                }
                let identity = Identity {
                    app_id: app_id.clone(),
                    app_name: app_name.clone(),
                    session_id: session.clone(),
                    scan_id: string(row, "scan_id"),
                    sub_request: number(row, "scan_sub_req_id"),
                };
                if identity.scan_id.is_empty() || identity.sub_request.is_none() {
                    fail(
                        &mut details,
                        "Session action missing scan or valid sub-request identity",
                    );
                    continue;
                }
                if !events.insert(identity.clone()) {
                    fail(&mut details, "Repeated action identity excluded");
                    continue;
                }
                let timestamp_ms = row
                    .get("start_time")
                    .and_then(Value::as_str)
                    .and_then(crate::time::parse_iso);
                if timestamp_ms.is_none() {
                    details.notes.push(
                        "Some actions have no usable event timestamp; excluded from window totals"
                            .into(),
                    );
                }
                let mut hints = management_hints(row);
                let bytes: usize = hints.iter().map(String::len).sum();
                if action_hint_bytes.saturating_add(bytes) > 4 * 1024 * 1024 {
                    hints.clear();
                    fail(
                        &mut details,
                        "Management action hint storage cap reached; positive metadata is partial",
                    );
                } else {
                    action_hint_bytes += bytes;
                }
                if row
                    .get("detection_type_violation_breakdown")
                    .and_then(Value::as_array)
                    .is_some_and(|rows| rows.len() > 1000)
                {
                    fail(
                        &mut details,
                        "Management detector breakdown view cap reached",
                    );
                }
                e.events.push(Event {
                    identity,
                    timestamp_ms,
                    profile: row
                        .get("profile_name")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    verdict: "unknown".into(),
                    action: "unknown".into(),
                    report_ids: vec![],
                    detections: vec![],
                    detector_hints: hints.clone(),
                    sources: vec!["session-details".into()],
                    notes: if hints.is_empty() && !row.get("violation_breakdown").is_some_and(positive_counts) { vec![] } else {
                        vec!["Positive management action breakdown; detector direction and enforcement are not supplied by this source".into()]
                    },
                });
                details.records += 1;
            }
            if let Some(history) = history {
                history.session_finished(
                    (&app_id, &app_name, &session),
                    &e.window,
                    done && details.records - retained_before == items.len() as u64,
                )?;
            }
            if !done && !shifted {
                per_app
                    .get_mut(&app_key)
                    .expect("scheduled app exists")
                    .push_back((
                        (app_id, app_name, session),
                        offset + items.len() as u64,
                        total,
                    ));
                if !active.contains(&app_key) {
                    active.push_back(app_key);
                }
            }
        }
        if let Some(history) = history {
            let mut snapshot = e.clone();
            let mut partial = details.clone();
            partial.ended_ms = now();
            partial.status = Status::Partial;
            snapshot.sources.push(partial);
            snapshot.seal_native()?;
            history.checkpoint(&snapshot)?;
        }
        if exhausted {
            break;
        }
    }
    details.notes.sort();
    details.notes.dedup();
    details.ended_ms = now();
    e.sources.push(details);
    if let Some(history) = history {
        history.checkpoint(e)?;
    }
    Ok(())
}

pub struct Enrichment {
    pub source: Source,
    pub attributed: BTreeSet<Identity>,
}

pub(crate) async fn enrich(
    scanner: &scm::airs::scan::Scanner,
    status: &crate::clients::LastStatus,
    budget: &Arc<ReadBudget>,
    events: &mut [Event],
    secrets: &crate::error::SecretSet,
) -> Source {
    enrich_in_context(scanner, status, budget, events, secrets, None)
        .await
        .source
}

pub async fn enrich_in_context(
    scanner: &scm::airs::scan::Scanner,
    status: &crate::clients::LastStatus,
    budget: &Arc<ReadBudget>,
    events: &mut [Event],
    secrets: &crate::error::SecretSet,
    contexts: Option<&[Event]>,
) -> Enrichment {
    let mut attributed = BTreeSet::new();
    let mut s = source("scan-results", "correlated scan result rows", true);
    let application = events
        .first()
        .map_or("unknown", |event| event.identity.app_id.as_str())
        .to_owned();
    let before = status.attempts("airs.scan.results");
    type ScanContexts = BTreeMap<(String, Option<u64>), BTreeSet<(String, String)>>;
    let mut identities = ScanContexts::new();
    for event in contexts
        .unwrap_or(events)
        .iter()
        .filter(|e| e.identity.app_id == application)
    {
        identities
            .entry((event.identity.scan_id.clone(), event.identity.sub_request))
            .or_default()
            .insert((
                event.identity.app_name.clone(),
                event.identity.session_id.clone(),
            ));
    }
    let ids: Vec<_> = events
        .iter()
        .filter(|event| scan_logs::is_scan_id(&event.identity.scan_id))
        .map(|event| event.identity.scan_id.clone())
        .collect();
    let (rows, missing, notes, diagnostics) =
        scan_logs::lookup(scanner, budget, &ids, false, secrets, None).await;
    for note in notes {
        fail(&mut s, note);
    }
    if !missing.is_empty() {
        fail(
            &mut s,
            format!("{} requested scan IDs missing from results", missing.len()),
        );
    }
    for event in events {
        if !scan_logs::is_scan_id(&event.identity.scan_id) {
            fail(
                &mut s,
                "Opaque dashboard scan ID cannot be queried by the pinned UUID-only scanner method",
            );
            event.notes.push("Unresolved scan identity".into());
            continue;
        }
        let matching: Vec<_> = rows
            .iter()
            .filter(|row| {
                row.get("scan_id").and_then(Value::as_str) == Some(event.identity.scan_id.as_str())
                    && number(row, "req_id") == event.identity.sub_request
            })
            .collect();
        if matching.len() != 1 {
            fail(
                &mut s,
                "Missing or ambiguous scan/sub-request result identity; no positional join used",
            );
            continue;
        }
        let row = matching[0];
        if contexts.is_none() && !scan_logs::usable_result(row) {
            fail(
                &mut s,
                "Scan result pending, failed, timed out or unknown; outcome remains unresolved",
            );
            event
                .notes
                .push("Incomplete scan outcome; never interpreted as allow".into());
            continue;
        }

        let Some(result) = row.get("result").filter(|r| r.is_object()) else {
            fail(&mut s, "Result is pending, unavailable or missing");
            continue;
        };
        let eligible = identities
            .get(&(event.identity.scan_id.clone(), event.identity.sub_request))
            .map_or(0, |contexts| {
                contexts
                    .iter()
                    .filter(|(_, session)| {
                        result
                            .get("session_id")
                            .and_then(Value::as_str)
                            .is_none_or(|returned| returned == session)
                    })
                    .count()
            });
        if eligible != 1 {
            fail(
                &mut s,
                "Scan/sub-request has ambiguous exact application-name/session buckets; result attribution remains unresolved",
            );
            continue;
        }
        if (string(result, "scan_id") != event.identity.scan_id
            && !(contexts.is_some() && string(result, "scan_id").is_empty()))
            || result
                .get("session_id")
                .and_then(Value::as_str)
                .is_some_and(|id| id != event.identity.session_id)
        {
            fail(
                &mut s,
                "Result scan/session identity conflicts with discovery",
            );
            continue;
        }
        if contexts.is_some() {
            let status = string(row, "status").to_ascii_lowercase();
            if ["complete", "completed"].contains(&status.as_str()) {
                let response = scm::airs::scan::models::ScanResponse::deserialize(result)
                    .map_err(|_| ())
                    .ok();
                if let Some(response) = response {
                    crate::scan_verdict::apply_response(event, &response, secrets);
                } else {
                    fail(&mut s, "Typed scan result metadata unavailable");
                    continue;
                }
            } else {
                event.verdict = if ["failed", "error"].contains(&status.as_str()) {
                    "error"
                } else {
                    "unknown"
                }
                .into();
                event.action = "failed".into();
            }
            if !["benign", "malicious"].contains(&event.verdict.as_str())
                || !["allow", "block"].contains(&event.action.as_str())
            {
                fail(
                    &mut s,
                    "Scan result pending, failed, timed out or unknown; outcome remains unresolved",
                );
                event
                    .notes
                    .push("Incomplete scan outcome; never interpreted as allow".into());
            }
            if result.get("session_id").and_then(Value::as_str)
                == Some(event.identity.session_id.as_str())
            {
                event
                    .notes
                    .push("Scanner result supplied matching explicit session identity".into());
            }
        } else {
            event.verdict = string(result, "category").to_ascii_lowercase();
            event.action = string(result, "action").to_ascii_lowercase();
            for direction in ["prompt", "response", "tool"] {
                if let Some(flags) = result
                    .get(format!("{direction}_detected"))
                    .and_then(Value::as_object)
                {
                    for (detector, value) in flags {
                        if value == &Value::Bool(true)
                            && crate::scan_verdict::PROMPT_KEYS
                                .iter()
                                .chain(crate::scan_verdict::RESPONSE_KEYS.iter())
                                .any(|known| known == detector)
                        {
                            event.detector_hints.push(format!("{direction}/{detector}"));
                        }
                    }
                }
            }
            event.detector_hints.sort();
            event.detector_hints.dedup();
        }
        if event.timestamp_ms.is_none() {
            event.timestamp_ms = result
                .get("created_at")
                .and_then(Value::as_str)
                .and_then(crate::time::parse_iso);
        }
        if let Some(name) = result.get("profile_name").and_then(Value::as_str) {
            event.profile = Some(name.into());
        }
        let report = string(result, "report_id");
        if report.is_empty() {
            s.notes.push(
                "Scan result supplied no report identity; optional example unavailable".into(),
            );
        } else if !event.report_ids.contains(&report) {
            event.report_ids.push(report);
        }
        attributed.insert(event.identity.clone());
        event.sources.push("scan-results".into());
        s.records += u64::from(
            ["benign", "malicious"].contains(&event.verdict.as_str())
                && ["allow", "block"].contains(&event.action.as_str()),
        );
    }
    s.pages = status.attempts("airs.scan.results").saturating_sub(before);
    s.notes.push(lookup_diagnostics(
        &application,
        &diagnostics,
        s.records,
        s.pages,
    ));
    s.ended_ms = now();
    Enrichment {
        source: s,
        attributed,
    }
}

fn lookup_diagnostics(
    application: &str,
    counts: &scan_logs::LookupDiagnostics,
    usable_events: u64,
    attempts: u64,
) -> String {
    format!(
        "Application {application}: requested IDs {}; returned rows {}; usable event observations {usable_events}; pending rows {}; rejected/excluded rows {}; missing IDs {}; empty successful responses {}; failed SDK reads {}; actual sends {attempts}. Counts have distinct units; missing or failed reads do not prove a credential cause.",
        counts.requested_ids,
        counts.returned_rows,
        counts.pending_rows,
        counts.rejected_rows,
        counts.missing_ids,
        counts.empty_responses,
        counts.failed_reads
    )
}

/// A management fallback supplies explicit typed metadata, never scanner enforcement.
async fn transaction_metadata(
    clients: &ManagementClients,
    budget: &Arc<ReadBudget>,
    o: &CollectionOptions,
    e: &mut Evidence,
    secrets: &crate::error::SecretSet,
) {
    let unresolved: Vec<_> = e
        .events
        .iter()
        .enumerate()
        .filter(|(_, event)| {
            !matches!(event.verdict.as_str(), "malicious" | "benign")
                || !matches!(event.action.as_str(), "allow" | "block")
        })
        .map(|(i, _)| i)
        .collect();
    if unresolved.is_empty() {
        return;
    }
    let mut s = source(
        "session-transactions",
        "exact-identity management transaction metadata",
        false,
    );
    s.notes.push("Supplemental metadata only: transaction status does not establish scan verdict/action or report identity; scan-results coverage remains unresolved".into());
    let deadline =
        tokio::time::Instant::now() + std::time::Duration::from_millis(o.limits.seconds * 100);
    let candidates = unresolved.len().min(o.max_pages.min(10) as usize);
    if candidates < unresolved.len() {
        fail(
            &mut s,
            "At most ten fallback transaction reads per invocation; remaining unresolved events retained",
        );
    }
    let dashboard = clients.management.dashboard();
    for chunk in unresolved[..candidates].chunks(o.limits.concurrency as usize) {
        let mut jobs = Vec::new();
        for i in chunk {
            let identity = e.events[*i].identity.clone();
            let dashboard = &dashboard;
            jobs.push(async move {
                let Some(sub_request) = identity.sub_request else {
                    return (*i, Err(scm::Error::new(scm::ErrorKind::Validation)));
                };
                let mut q = DashboardSessionTransactionQuery::new(
                    &identity.session_id,
                    &identity.app_id,
                    &identity.app_name,
                    &identity.scan_id,
                    sub_request,
                );
                q.time_interval = Some(o.interval);
                q.time_unit = Some(o.unit.clone());
                let q = &q; // the read may be replayed once (explicit 401 replay)
                let result = tokio::time::timeout_at(
                    deadline,
                    budget.run(|opts| async move {
                        dashboard
                            .session_transaction(q, Some(&opts))
                            .await
                            .and_then(model)
                    }),
                )
                .await
                .unwrap_or_else(|_| Err(scm::Error::new(scm::ErrorKind::Deadline)));
                (*i, result)
            });
        }
        let mut exhausted = false;
        for (i, result) in futures_util::future::join_all(jobs).await {
            s.pages += 1;
            let value = match result {
                Ok(v) => v,
                Err(err) => {
                    fail(&mut s, scan_logs::note(&err));
                    if matches!(
                        err.kind(),
                        scm::ErrorKind::Deadline
                            | scm::ErrorKind::Limit
                            | scm::ErrorKind::Cancelled
                    ) {
                        exhausted = true;
                    }
                    continue;
                }
            };
            // Discover secret-shaped extension values, but persist no extensions.
            if let Ok(mut discovery) = crate::json::to_ordered(&value) {
                crate::withhold::apply(&mut discovery, &crate::withhold::PLAIN, secrets);
            }
            let event = &mut e.events[i];
            let id = &event.identity;
            if string(&value, "application_id") != id.app_id
                || string(&value, "application_name") != id.app_name
                || string(&value, "session_id") != id.session_id
                || string(&value, "scan_id") != id.scan_id
                || number(&value, "scan_sub_req_id") != id.sub_request
            {
                fail(
                    &mut s,
                    "Transaction identity conflicts with requested application/session/scan/sub-request; excluded",
                );
                continue;
            }
            s.records += 1;
            if event.profile.is_none() {
                event.profile = value
                    .get("profile_name")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
            }
            if event.timestamp_ms.is_none() {
                event.timestamp_ms = value
                    .get("start_time")
                    .and_then(Value::as_str)
                    .and_then(crate::time::parse_iso);
            }
            if value.get("timeout").and_then(Value::as_bool) != Some(true)
                && let Some(rows) = value
                    .get("detection_type_violation_breakdown")
                    .and_then(Value::as_array)
            {
                for row in rows {
                    let detector = string(row, "detection_type");
                    let positive = row.get("violation_breakdown").is_some_and(|v| {
                        ["total", "critical", "high", "medium", "low"]
                            .iter()
                            .any(|key| number(v, key).is_some_and(|n| n > 0))
                    });
                    if !detector.is_empty() && positive {
                        event.detector_hints.push(format!("unknown/{detector}"));
                    }
                }
            }
            event.detector_hints.sort();
            event.detector_hints.dedup();
            event.sources.push("session-transactions".into());
            event.notes.push("Management transaction metadata recovered; scanner verdict/action remain unresolved; no report ID inferred".into());
        }
        if exhausted {
            break;
        }
    }
    s.ended_ms = now();
    e.sources.push(s);
}

pub(crate) fn select_reports(e: &mut Evidence, limit: u64) {
    select_reports_policy(e, limit, None, &BTreeSet::new());
}

pub fn select_reports_policy(
    e: &mut Evidence,
    limit: u64,
    preferred: Option<&str>,
    cached: &BTreeSet<String>,
) {
    e.sampling.selected_report_ids.clear();
    e.sampling.uncovered_groups.clear();
    // Each report carries explicit coverage features. Greedily prefer uncovered
    // enforcement/app/detector/direction/profile groups, never repeat an ID.
    type Features = BTreeSet<(u64, String)>;
    let mut candidates: BTreeMap<String, (i64, Features)> = BTreeMap::new();
    let mut strata: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for event in &e.events {
        if event.verdict != "malicious"
            || !event
                .timestamp_ms
                .is_some_and(|at| at >= e.window.start_ms && at < e.window.end_ms)
        {
            continue;
        }
        let mut hints = super::detectors::observation_hints(&event.detector_hints);
        if hints.is_empty() {
            hints.insert(("unknown", "unknown"));
        }
        let raw_hints: BTreeMap<_, _> = event
            .detector_hints
            .iter()
            .filter_map(|hint| {
                let (direction, detector) = hint.split_once('/')?;
                Some((
                    (direction, super::detectors::canonical(detector)),
                    hint.as_str(),
                ))
            })
            .collect();
        for report in event
            .report_ids
            .iter()
            .filter(|id| scan_logs::is_report_id(id))
        {
            let candidate = candidates.entry(report.clone()).or_default();
            candidate.0 = candidate.0.max(event.timestamp_ms.unwrap_or(i64::MIN));
            let f = &mut candidate.1;
            f.insert((32, format!("action:{}", event.action)));
            f.insert((
                16,
                format!(
                    "app:{} / {}",
                    event.identity.app_id, event.identity.app_name
                ),
            ));
            f.insert((
                4,
                format!("profile:{}", event.profile.as_deref().unwrap_or("unknown")),
            ));
            for (direction, detector) in &hints {
                let hint = raw_hints
                    .get(&(*direction, *detector))
                    .copied()
                    .unwrap_or("unknown/unknown");
                f.insert((
                    8,
                    format!("detector:{}", super::detectors::canonical(detector)),
                ));
                f.insert((4, format!("direction:{direction}")));
                let stratum = format!(
                    "{} / {} / {} / {} / {}",
                    event.identity.app_id,
                    event.identity.app_name,
                    event.profile.as_deref().unwrap_or("unknown"),
                    hint,
                    event.action
                );
                f.insert((1, stratum.clone()));
                strata.entry(stratum).or_default().insert(report.clone());
            }
        }
    }
    e.sampling.candidates = candidates.len() as u64;
    let mut covered = BTreeSet::new();
    for id in cached {
        if let Some((_, features)) = candidates.remove(id) {
            covered.extend(features);
        }
    }
    if limit > 0
        && let Some(id) = preferred
        && let Some((_, features)) = candidates.remove(id)
    {
        covered.extend(features);
        e.sampling.selected_report_ids.push(id.into());
    }
    for _ in e.sampling.selected_report_ids.len()..limit.min(10) as usize {
        let selected = candidates
            .iter()
            .max_by(|(ida, (ata, a)), (idb, (atb, b))| {
                let score = |features: &Features| {
                    features
                        .difference(&covered)
                        .map(|(weight, _)| *weight)
                        .sum::<u64>()
                };
                score(a)
                    .cmp(&score(b))
                    .then(ata.cmp(atb))
                    .then_with(|| idb.cmp(ida))
            })
            .map(|(id, _)| id.clone());
        let Some(id) = selected else {
            break;
        };
        let (_, features) = candidates.remove(&id).expect("selected candidate exists");
        covered.extend(features);
        e.sampling.selected_report_ids.push(id);
    }
    for (stratum, ids) in strata {
        if !ids
            .iter()
            .any(|id| e.sampling.selected_report_ids.contains(id) || cached.contains(id))
        {
            e.sampling.uncovered_groups.push(stratum);
        }
    }
}

pub(crate) async fn details(
    scanner: &scm::airs::scan::Scanner,
    status: &crate::clients::LastStatus,
    budget: &Arc<ReadBudget>,
    events: &mut [Event],
    selected_ids: &[String],
    secrets: &crate::error::SecretSet,
    payload: &privacy::PayloadSpans,
) -> Source {
    details_attributed(
        scanner,
        status,
        budget,
        events,
        selected_ids,
        secrets,
        payload,
    )
    .await
    .source
}

pub async fn details_attributed(
    scanner: &scm::airs::scan::Scanner,
    status: &crate::clients::LastStatus,
    budget: &Arc<ReadBudget>,
    events: &mut [Event],
    selected_ids: &[String],
    secrets: &crate::error::SecretSet,
    payload: &privacy::PayloadSpans,
) -> Enrichment {
    let mut attributed = BTreeSet::new();
    let before = status.attempts("airs.scan.reports");
    let application = events
        .first()
        .map_or("unknown", |event| event.identity.app_id.as_str())
        .to_owned();
    let mut s = source("scan-reports", "selected detector detail reports", false);
    s.notes.push("Optional examples selected for application, observed profile, detector, direction and allow/block coverage; default five IDs, hard maximum ten, at most two serial report API attempts; stop on rate limiting without retry".into());
    let (raw, missing, notes, diagnostics) =
        scan_logs::lookup(scanner, budget, selected_ids, true, secrets, Some(payload)).await;
    for note in notes {
        fail(&mut s, note);
    }
    if !missing.is_empty() {
        fail(
            &mut s,
            format!("{} selected reports missing", missing.len()),
        );
    }
    s.pages = status.attempts("airs.scan.reports").saturating_sub(before);
    // Register DLP values across every returned report before projecting any excerpt.
    let views = scan_logs::report_views(&raw, payload);
    for view in &views {
        let report = string(view, "report_id");
        if !selected_ids.contains(&report) {
            fail(
                &mut s,
                "Unrequested report identity returned; excluded from selected evidence",
            );
            continue;
        }

        let matching: Vec<_> = events
            .iter()
            .enumerate()
            .filter(|(_, ev)| {
                ev.report_ids.contains(&report)
                    && view.get("scan_id").and_then(Value::as_str)
                        == Some(ev.identity.scan_id.as_str())
                    && number(view, "req_id") == ev.identity.sub_request
            })
            .map(|(i, _)| i)
            .collect();
        if matching.len() != 1 {
            fail(
                &mut s,
                "Detailed report has missing, conflicting or ambiguous scan/sub-request identities",
            );
            continue;
        }
        let event = &mut events[matching[0]];
        if view
            .get("session_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id != event.identity.session_id)
        {
            fail(
                &mut s,
                "Detailed report session identity conflicts with discovery",
            );
            continue;
        }
        let Some(rows) = view.get("detections").and_then(Value::as_array) else {
            fail(&mut s, "Detailed report lacks detector rows");
            continue;
        };
        for row in rows {
            let snippets =
                serde_json::from_value(row.get("snippets").cloned().unwrap_or(Value::Null))
                    .unwrap_or_default();
            event.detections.push(Detection {
                detector: string(row, "detection_service"),
                direction: string(row, "data_type"),
                verdict: string(row, "verdict"),
                action: string(row, "action"),
                report_id: report.clone(),
                row: number(row, "row").unwrap_or(0),
                snippets,
            });
        }
        attributed.insert(event.identity.clone());
        event.sources.push("scan-reports".into());
        s.records += 1;
    }
    s.notes.push(lookup_diagnostics(
        &application,
        &diagnostics,
        s.records,
        s.pages,
    ));
    s.ended_ms = now();
    Enrichment {
        source: s,
        attributed,
    }
}

async fn configuration_reads(
    clients: &ManagementClients,
    budget: &Arc<ReadBudget>,
    o: &CollectionOptions,
    e: &mut Evidence,
    native: bool,
) -> BTreeSet<(String, String)> {
    // Independent walks queue their first operation together, even at concurrency one.
    // Merge in fixed order so network completion order never determines evidence order.
    let mut p = e.clone();
    let mut r = e.clone();
    let mut a = e.clone();
    let (_, _, buckets) = futures_util::future::join3(
        profile_reads(clients, budget, o, &mut p),
        registered_reads(clients, budget, o, &mut r),
        application_reads(clients, budget, o, &mut a, native),
    )
    .await;
    e.sources.extend(p.sources);
    e.sources.extend(r.sources);
    e.sources.extend(a.sources);
    e.profiles = p.profiles;
    e.applications = r.applications;
    e.metrics = a.metrics;
    buckets
}
async fn profile_reads(
    clients: &ManagementClients,
    budget: &Arc<ReadBudget>,
    o: &CollectionOptions,
    e: &mut Evidence,
) {
    // Inventory walks share per-family caps; these are current settings, not history.
    let mut profiles = source("profiles", "current profile revisions", true);
    profiles.population="Tenant-wide current profile inventory; relevance to selected applications requires explicit association".into();
    let mut offset = 0;
    for page in 0..o.max_pages {
        let mut q = PaginationOptions::default();
        q.offset = offset;
        q.limit = 100;
        q.latest = Some(true);
        let result = budget
            .run_slice(
                Some(std::time::Duration::from_millis(o.limits.seconds * 200 / 3)),
                |opts| async move {
                    clients
                        .management
                        .profiles()
                        .list(q, Some(&opts))
                        .await
                        .and_then(model)
                },
            )
            .await;
        let value = match result {
            Ok(v) => v,
            Err(err) => {
                fail(&mut profiles, scan_logs::note(&err));
                break;
            }
        };
        profiles.pages += 1;
        let Some(items) = value.get("ai_profiles").and_then(Value::as_array) else {
            fail(
                &mut profiles,
                "Missing profile array is not an empty configuration",
            );
            break;
        };
        for item in items {
            let name = string(item, "profile_name");
            if e.profiles
                .iter()
                .any(|p| p.name == name && p.revision == number(item, "revision"))
            {
                fail(&mut profiles, "Repeated profile revision");
                continue;
            }
            e.profiles.push(Profile {
                name,
                revision: number(item, "revision"),
                protections: profile_protections(item),
            });
            profiles.records += 1;
        }
        let next = number(&value, "next_offset");
        if next == Some(0) || (next.is_none() && items.len() < 100) {
            break;
        }
        let next = next.unwrap_or(offset + items.len() as u64);
        if next <= offset || items.is_empty() {
            fail(&mut profiles, "Profile pagination did not advance");
            break;
        }
        offset = next;
        if page + 1 == o.max_pages {
            fail(&mut profiles, "Profile page cap reached");
        }
    }
    profiles.ended_ms = now();
    e.sources.push(profiles);
}
async fn registered_reads(
    clients: &ManagementClients,
    budget: &Arc<ReadBudget>,
    o: &CollectionOptions,
    e: &mut Evidence,
) {
    let mut apps = source("registered-apps", "registered applications", true);
    let mut offset = 0;
    for page in 0..o.max_pages {
        let mut q = PaginationOptions::default();
        q.offset = offset;
        q.limit = 100;
        let result = budget
            .run_slice(
                Some(std::time::Duration::from_millis(o.limits.seconds * 200 / 3)),
                |opts| async move {
                    clients
                        .management
                        .customer_apps()
                        .list(q, Some(&opts))
                        .await
                        .and_then(model)
                },
            )
            .await;
        let value = match result {
            Ok(v) => v,
            Err(err) => {
                fail(&mut apps, scan_logs::note(&err));
                break;
            }
        };
        apps.pages += 1;
        let Some(items) = value.get("customer_apps").and_then(Value::as_array) else {
            fail(&mut apps, "Missing application array");
            break;
        };
        for item in items {
            let id = string(item, "customer_appId");
            let name = string(item, "app_name");
            if !selected(o, &id) {
                continue;
            }
            if e.applications.iter().any(|a| a.id == id && a.name == name) {
                fail(&mut apps, "Repeated application identity");
                continue;
            }
            e.applications.push(Application {
                id,
                name,
                profile: None,
            });
            apps.records += 1;
        }
        let next = number(&value, "next_offset");
        if next == Some(0) || (next.is_none() && items.len() < 100) {
            break;
        }
        let next = next.unwrap_or(offset + items.len() as u64);
        if next <= offset || items.is_empty() {
            fail(&mut apps, "Application pagination did not advance");
            break;
        }
        offset = next;
        if page + 1 == o.max_pages {
            fail(&mut apps, "Registered application page cap reached");
        }
    }
    apps.ended_ms = now();
    e.sources.push(apps);
}
async fn application_reads(
    clients: &ManagementClients,
    budget: &Arc<ReadBudget>,
    o: &CollectionOptions,
    e: &mut Evidence,
    native: bool,
) -> BTreeSet<(String, String)> {
    let dashboard = clients.management.dashboard();
    let mut apps = source("applications", "application activity buckets", true);
    let mut offset = 0;
    let application_window = if o.unit.starts_with("day") && [1, 7, 30, 60].contains(&o.interval) {
        Some((o.interval, "days"))
    } else if o.unit.starts_with("hour") && [1, 7, 30, 60].contains(&o.interval) {
        Some((o.interval, "hour"))
    } else if o.unit.starts_with("hour")
        && o.interval.is_multiple_of(24)
        && [1, 7, 30, 60].contains(&(o.interval / 24))
    {
        Some((o.interval / 24, "day"))
    } else {
        None
    };
    let mut activity_total = None;
    let mut buckets = BTreeSet::new();
    for page in 0..o.max_pages {
        let dashboard = &dashboard;
        let Some((interval, unit)) = application_window else {
            fail(
                &mut apps,
                "Application overview supports interval 1, 7, 30 or 60 only; requested window has no exact representation",
            );
            break;
        };
        let mut q = DashboardApplicationsOverviewQuery::default();
        q.time_interval = Some(interval);
        q.time_unit = Some(unit.into());
        q.limit = Some(100);
        q.offset = Some(offset);
        let q = &q; // the read may be replayed once (explicit 401 replay)
        let result = budget
            .run_slice(
                Some(std::time::Duration::from_millis(o.limits.seconds * 200 / 3)),
                |opts| async move {
                    dashboard
                        .applications_overview(q, Some(&opts))
                        .await
                        .and_then(model)
                },
            )
            .await;
        let value = match result {
            Ok(v) => v,
            Err(err) => {
                fail(&mut apps, scan_logs::note(&err));
                break;
            }
        };
        apps.pages += 1;
        let Some(items) = value.get("items").and_then(Value::as_array) else {
            fail(&mut apps, "Missing application activity array");
            break;
        };
        let mut shifted = false;
        let done = match page_valid(
            &value,
            offset,
            items.len(),
            &mut activity_total,
            "items",
            100,
        ) {
            Ok(v) => v,
            Err(note) => {
                fail(&mut apps, note);
                if !native {
                    break;
                }
                shifted = true;
                false
            }
        };
        for item in items {
            let id = string(item, "id");
            let name = string(item, "name");
            if native && (id.trim().is_empty() || name.trim().is_empty()) {
                fail(
                    &mut apps,
                    "Application activity bucket has incomplete identity",
                );
                continue;
            }
            if selected(o, &id) {
                if !buckets.insert((id.clone(), name.clone())) {
                    fail(&mut apps, "Repeated application activity bucket excluded");
                    continue;
                }
                apps.records += 1;
                for key in ["sessions_total", "sessions_violated"] {
                    let value = number(item, key).map(|v| v as f64);
                    e.metrics.push(Metric {
                        name: key.into(),
                        value,
                        unit: "sessions".into(),
                        scale: None,
                        population: format!("Application {id} / {name}"),
                        window: format!("Rolling {} {} (API query)", o.interval, o.unit),
                        source: "applications".into(),
                        status: if value.is_some() {
                            Status::Complete
                        } else {
                            Status::Unavailable
                        },
                    });
                }
            }
        }
        if done || shifted {
            break;
        }
        offset += items.len() as u64;
        if page + 1 == o.max_pages {
            fail(&mut apps, "Application activity page cap reached");
        }
    }
    apps.ended_ms = now();
    e.sources.push(apps);
    buckets
}
async fn application_telemetry(
    clients: &ManagementClients,
    budget: &Arc<ReadBudget>,
    o: &CollectionOptions,
    e: &mut Evidence,
    buckets: &BTreeSet<(String, String)>,
) {
    let dashboard = clients.management.dashboard();
    let mut telemetry = source(
        "application-telemetry",
        "application detail responses",
        false,
    );
    let days = if o.unit.starts_with("hour") {
        if o.interval.is_multiple_of(24) {
            Some(o.interval / 24)
        } else {
            None
        }
    } else {
        Some(o.interval)
    };
    if let Some(days) = days.filter(|days| [7, 30, 60].contains(days)) {
        for (index, (id, name)) in buckets.iter().enumerate() {
            if index as u64 >= o.max_pages {
                fail(&mut telemetry, "Application detail request cap reached");
                break;
            }
            let mut q = DashboardAppQuery::new(id, name);
            q.time_interval = Some(days);
            q.time_unit = Some("days".into());
            let q = &q; // the read may be replayed once (explicit 401 replay)
            let result = budget
                .run(|opts| {
                    let dashboard = &dashboard;
                    async move { dashboard.application(q, Some(&opts)).await.and_then(model) }
                })
                .await;
            let value = match result {
                Ok(v) => v,
                Err(err) => {
                    fail(&mut telemetry, scan_logs::note(&err));
                    continue;
                }
            };
            if string(&value, "id") != *id || string(&value, "name") != *name {
                fail(
                    &mut telemetry,
                    "Application detail identity conflicts with requested bucket",
                );
                continue;
            }
            telemetry.records += 1;
            telemetry.pages += 1;
            for key in ["average_daily_tokens", "monthly_total_tokens"] {
                let value_number = value
                    .get("token_stats")
                    .and_then(|v| v.get(key))
                    .and_then(Value::as_f64)
                    .filter(|v| {
                        v.is_finite() && *v >= 0.0 && *v <= crate::limits::MAX_SAFE_INTEGER as f64
                    });
                let scale = value
                    .get("token_stats")
                    .and_then(|v| v.get(format!("{key}_scale")))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                e.metrics.push(Metric{name:key.into(),value:value_number,unit:"tokens (server-scaled)".into(),scale,population:format!("Application {id} / {name}"),window:"Server-reported daily average or monthly total; alignment to target window unverified".into(),source:"application-telemetry".into(),status:if value_number.is_some(){Status::Complete}else{Status::Unavailable}});
            }
            let profiles = value.get("profiles").and_then(Value::as_array);
            let profile = profiles
                .filter(|p| p.len() == 1)
                .and_then(|p| p[0].as_str())
                .map(str::to_owned);
            if !e
                .applications
                .iter()
                .any(|a| &a.id == id && &a.name == name)
            {
                e.applications.push(Application {
                    id: id.clone(),
                    name: name.clone(),
                    profile,
                });
            }
        }
    } else {
        fail(
            &mut telemetry,
            "Application detail API supports 7, 30 or 60 days only; requested window has no exact representation",
        );
    }
    telemetry.ended_ms = now();
    e.sources.push(telemetry);
}
fn profile_protections(item: &Value) -> Vec<Protection> {
    let mut out = Vec::new();
    let policy = item.get("policy");
    for location in crate::profile_policy::locations(policy) {
        let entry = policy
            .and_then(|p| p.get("ai-security-profiles"))
            .and_then(Value::as_array)
            .and_then(|a| a.get(location.place.index));
        let known_mode = entry
            .and_then(|v| v.get("content-type-mode"))
            .is_none_or(|v| v.as_str() == Some(crate::profile_policy::PER_CONTENT_TYPE));
        let known_direction = location
            .place
            .direction
            .as_deref()
            .is_none_or(|d| crate::profile_policy::DIRECTIONS.contains(&d));
        let active = item.get("active").and_then(Value::as_bool);
        let confidence = if !known_mode || !known_direction {
            "unknown-layout"
        } else if active.is_none() {
            "unknown-active-state"
        } else {
            "explicit"
        };
        let direction = location
            .place
            .direction
            .clone()
            .unwrap_or_else(|| "shared (legacy)".into());
        let label = crate::profile_policy::location_label(&location);
        let mut add = |detector: &str, action: Option<&str>| {
            out.push(Protection {
                detector: detector.into(),
                direction: direction.clone(),
                action: action.unwrap_or("unknown").into(),
                active: location.active && active == Some(true) && confidence == "explicit",
                location: label.clone(),
                confidence: confidence.into(),
            });
        };
        for key in ["model-protection", "agent-protection"] {
            if let Some(rows) = location.configuration.get(key).and_then(Value::as_array) {
                for row in rows {
                    if let Some(name) = row.get("name").and_then(Value::as_str) {
                        add(name, row.get("action").and_then(Value::as_str));
                    }
                }
            }
        }
        if let Some(app) = location.configuration.get("app-protection") {
            if let Some(control) = app.get("malicious-code-protection") {
                add(
                    "malicious-code",
                    control.get("action").and_then(Value::as_str),
                );
            }
            if let Some(action) = app.get("url-detected-action") {
                add("url-filtering", action.as_str());
            }
        }
        if let Some(data) = location.configuration.get("data-protection") {
            if let Some(control) = data.get("data-leak-detection").filter(|v| v.is_object()) {
                add("dlp", control.get("action").and_then(Value::as_str));
            }
            if let Some(rows) = data.get("database-security").and_then(Value::as_array) {
                for row in rows {
                    if let Some(name) = row.get("name").and_then(Value::as_str) {
                        add(name, row.get("action").and_then(Value::as_str));
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tenant::{
        Selected, TenantContext,
        config::{ConfigValue, KEYS, TenantConfig},
    };
    use airs_test_harness::{FixtureConfig, FixtureServer, Planned, Route, Rx, fixtures};
    use serde_json::json;
    #[test]
    fn history_cached_example_strata_are_covered_without_new_selections() {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        let cached = BTreeSet::from(["report-1".into()]);
        select_reports_policy(&mut e, 5, None, &cached);
        assert!(e.sampling.selected_report_ids.is_empty());
        assert!(e.sampling.uncovered_groups.is_empty());
        let mut fresh = e.events[0].clone();
        fresh.identity.app_id = "app-new".into();
        fresh.identity.scan_id = "11111111-1111-4111-8111-111111111111".into();
        fresh.report_ids = vec!["report-new".into()];
        fresh.detections.clear();
        e.events.push(fresh);
        select_reports_policy(&mut e, 5, None, &cached);
        assert_eq!(e.sampling.selected_report_ids, vec!["report-new"]);
        assert!(e.sampling.uncovered_groups.is_empty());
    }
    #[test]
    fn history_one_old_reservation_leaves_strategic_slots_for_new_application_coverage() {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        let base = e.events[0].clone();
        e.events.clear();
        for n in 0..5 {
            let mut event = base.clone();
            event.report_ids = vec![format!("old-{n}")];
            event.detections.clear();
            event.timestamp_ms = Some(e.window.start_ms + 1000);
            e.events.push(event);
        }
        for n in 0..3 {
            let mut event = base.clone();
            event.identity.app_id = format!("new-app-{n}");
            event.report_ids = vec![format!("new-{n}")];
            event.detections.clear();
            event.timestamp_ms = Some(e.window.end_ms - 1000);
            e.events.push(event);
        }
        select_reports_policy(&mut e, 5, Some("old-0"), &BTreeSet::new());
        assert_eq!(e.sampling.selected_report_ids[0], "old-0");
        for n in 0..3 {
            assert!(e.sampling.selected_report_ids.contains(&format!("new-{n}")));
        }
    }
    #[test]
    fn history_lookup_batch_keeps_competing_buckets_outside_its_five_rows() {
        let server = FixtureServer::start();
        server.add_tenant(
            "100",
            "client-100",
            "FAKE-SECRET-100",
            Some("FAKE-APIKEY-100"),
            None,
        );
        let value = FixtureConfig::standard(&server.base_url(), "100").to_value();
        let config = TenantConfig::from_values(KEYS.iter().filter_map(|key| {
            value
                .get(*key)
                .and_then(Value::as_str)
                .map(|v| (*key, ConfigValue::Text(v.into())))
        }));
        let scan = "00000000-0000-4000-8000-0000000000b2";
        let mut result = fixtures::scan_block("monitor");
        result["scan_id"] = json!(scan);
        result.as_object_mut().unwrap().remove("session_id");
        server.set_default(
            Route::ScanResults,
            Planned::json(
                200,
                &json!([{"scan_id":scan,"req_id":0,"status":"COMPLETED","result":result}]),
            ),
        );
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let session = scm::Session::new();
            let budget = ReadBudget::new(10, 100, 1, session.clone());
            budget.stage(2);
            let (scanner, status) =
                crate::clients::build_scanner_bounded(&config, &session, None, Some(&budget))
                    .unwrap();
            let base = Event {
                identity: Identity {
                    app_id: "app-1".into(),
                    app_name: "Exact A".into(),
                    session_id: "session-1".into(),
                    scan_id: scan.into(),
                    sub_request: Some(0),
                },
                timestamp_ms: Some(now() - 1000),
                profile: None,
                verdict: "unknown".into(),
                action: "unknown".into(),
                report_ids: vec![],
                detections: vec![],
                detector_hints: vec![],
                sources: vec!["session-details".into()],
                notes: vec![],
            };
            let mut whole = vec![base.clone()];
            for n in 1..5 {
                let mut e = base.clone();
                e.identity.scan_id = format!("00000000-0000-4000-8000-{n:012x}");
                whole.push(e);
            }
            let mut competing = base;
            competing.identity.app_name = "Exact B".into();
            whole.push(competing);
            let mut batch = whole[..5].to_vec();
            let observed = enrich_in_context(
                &scanner,
                &status,
                &budget,
                &mut batch,
                &crate::error::SecretSet::default(),
                Some(&whole),
            )
            .await;
            assert_eq!(batch[0].verdict, "unknown");
            assert!(!batch[0].sources.iter().any(|s| s == "scan-results"));
            assert!(
                observed
                    .source
                    .notes
                    .iter()
                    .any(|s| s.contains("ambiguous exact application-name/session"))
            );
            assert_eq!(server.requests_for(Route::ScanResults).len(), 1);
        });
    }
    #[test]
    fn pinned_sdk_collector_samples_details_without_submitting_scans_or_reading_content() {
        let server = FixtureServer::start();
        server.seed_runtime();
        server.add_tenant(
            "100",
            "client-100",
            "FAKE-SECRET-100",
            Some("FAKE-APIKEY-100"),
            None,
        );
        let scan = "00000000-0000-4000-8000-0000000000b2";
        let item = fixtures::runtime::session_item("uuid-shared", "A & B", "session-1");
        server.set_default(
            Route::Runtime(Rx::SessionsOverview),
            Planned::json(
                200,
                &json!({"items":[item],"pagination":{"skip":0,"limit":25,"total_items":1}}),
            ),
        );
        let mut session = fixtures::runtime::session("session-1");
        session["session_actions"][0]["scan_id"] = json!(scan);
        session["session_actions"][0]["start_time"] = json!(crate::time::iso(now() - 1000));
        server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
        let mut result = fixtures::scan_block("monitor");
        result["report_id"] = json!("report-1");
        result["action"] = json!("allow");
        server.set_default(
            Route::ScanResults,
            Planned::json(
                200,
                &json!([{"scan_id":scan,"req_id":0,"status":"COMPLETED","result":result}]),
            ),
        );
        server.set_default(Route::ScanReports,Planned::json(200,&json!([{"scan_id":scan,"report_id":"report-1","req_id":0,"detection_results":[{"detection_service":"prompt_injection","data_type":"prompt","verdict":"malicious","action":"allow","result_detail":{"pi_snippets":vec!["Synthetic collector example";21],"dlp_snippets":{"snippets":["SYNTHETIC-DLP-VALUE"]}}}]}])));
        let value = FixtureConfig::standard(&server.base_url(), "100").to_value();
        let mut config = TenantConfig::from_values(KEYS.iter().filter_map(|key| {
            value
                .get(*key)
                .and_then(Value::as_str)
                .map(|v| (*key, ConfigValue::Text(v.into())))
        }));
        let scanner = crate::tenant::scanners::ScannerCredential::from_flat(
            &config,
            "uuid-shared",
            Some("key"),
        )
        .unwrap();
        config.set_scanner("monitor", scanner, false).unwrap();
        let context = TenantContext {
            selected: Selected {
                name: "synthetic".into(),
                tsg_id: "100".into(),
                by_flag: false,
            },
            config,
        };
        let session = scm::Session::new();
        let secrets = crate::secrets::SecretSet::default();
        secrets.register_config(&context.config);
        let payload_spans = super::super::privacy::PayloadSpans::default();
        let inv = CollectContext {
            session: &session,
            secrets: &secrets,
            payload_spans: &payload_spans,
            version: crate::VERSION,
        };
        let options = CollectionOptions {
            limits: scan_logs::Limits {
                seconds: 120,
                concurrency: 4,
                requests: 1000,
            },
            interval: 24,
            unit: "hours".into(),
            app_ids: vec![],
            max_pages: 40,
            max_events: 10000,
            max_reports: 5,
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let evidence = rt
            .block_on(collect(&inv, &context, &options, true))
            .unwrap();
        assert_eq!(evidence.events.len(), 1);
        assert_eq!(evidence.sampling.selected_report_ids, vec!["report-1"]);
        assert_eq!(evidence.events[0].detections.len(), 1);
        assert_eq!(
            evidence.events[0].detections[0].snippets[0].excerpts.len(),
            20
        );
        assert!(evidence.events[0].detections[0].snippets[0].truncated);
        let serialized = serde_json::to_string(&evidence).unwrap();
        assert!(!serialized.contains("SYNTHETIC-DLP-VALUE"));
        assert!(!serialized.contains("FAKE-SECRET-100"));
        assert!(serialized.contains("Synthetic collector example"));
        assert!(!evidence.metrics.is_empty(), "{:?}", evidence.sources);
        assert!(server.requests().iter().all(|r| !matches!(
            r.route,
            Route::ScanSync | Route::ScanAsync | Route::Runtime(Rx::ScanContent | Rx::ScanLogs)
        )));
        server.assert_no_unknown_routes();
        assert_eq!(evidence.budgets.attempts, server.requests().len() as u64);
    }
}

#[cfg(test)]
mod sampling_tests {
    use super::*;
    #[test]
    fn selection_is_deterministic_balanced_and_counts_shared_report_once() {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        e.events[0].detector_hints = vec!["prompt/injection".into(), "response/dlp".into()];
        let mut other = e.events[0].clone();
        other.identity.app_id = "app-2".into();
        other.report_ids = vec!["report-2".into()];
        other.timestamp_ms = Some(e.window.end_ms - 1);
        e.events.push(other);
        select_reports(&mut e, 1);
        assert_eq!(e.sampling.selected_report_ids, vec!["report-2"]);
        assert_eq!(e.sampling.candidates, 2);
        assert!(!e.sampling.uncovered_groups.is_empty());
        let first = e.sampling.clone();
        e.events.reverse();
        select_reports(&mut e, 1);
        assert_eq!(first, e.sampling);
        select_reports(&mut e, 2);
        assert_eq!(e.sampling.selected_report_ids, vec!["report-2", "report-1"]);
        assert!(e.sampling.uncovered_groups.is_empty());
        select_reports(&mut e, 0);
        assert!(e.sampling.selected_report_ids.is_empty());
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;
    #[test]
    fn known_directional_dlp_and_urls_are_extracted_without_extension_guessing() {
        let mut profile = airs_test_harness::fixtures::runtime::directional_profile();
        profile["policy"]["ai-security-profiles"][0]["content-type-configurations"]["prompt"]["extension"] =
            serde_json::json!({"fake-detector":"allow"});
        profile["policy"]["ai-security-profiles"][0]["model-configuration"]["model-protection"] =
            serde_json::json!([{"name":"retained-inactive","action":"allow"}]);
        let controls = profile_protections(&profile);
        assert!(controls.iter().any(|p| p.detector == "dlp"
            && p.direction == "prompt"
            && p.action == "block"
            && p.active));
        assert!(controls.iter().any(|p| p.detector == "url-filtering"
            && p.direction == "prompt"
            && p.action == "block"));
        assert!(!controls.iter().any(|p| p.detector == "fake-detector"));
        assert!(
            controls
                .iter()
                .any(|p| p.detector == "retained-inactive" && !p.active)
        );
        profile["active"] = serde_json::json!(false);
        assert!(profile_protections(&profile).iter().all(|p| !p.active));
        profile["active"] = serde_json::json!(true);
        profile["policy"]["ai-security-profiles"][0]["content-type-mode"] =
            serde_json::json!("future-mode");
        assert!(
            profile_protections(&profile)
                .iter()
                .all(|p| !p.active && p.confidence == "unknown-layout")
        );
    }
}
