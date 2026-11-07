//! Self-contained HTML: native evidence controls, CSP, no executable script or callbacks.
use super::{
    analyze::analyze,
    evidence::{Evidence, Status},
    privacy,
};
use crate::error::SecretSet;
fn h(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}
use sha2::{Digest, Sha256};

const CSS: &str = r#"
:root{color-scheme:light;--ink:#182b36;--muted:#526873;--line:#ccd8da;--accent:#13766e;--paper:#f3f5f2}*{box-sizing:border-box}body{overflow-wrap:anywhere;margin:0;background:var(--paper);color:var(--ink);font:16px/1.6 system-ui,sans-serif}a{color:#075e59}header,main,footer{max-width:1200px;margin:auto;padding:32px}header{padding-top:56px;border-bottom:3px solid var(--ink)}.eyebrow{font-size:.8rem;text-transform:uppercase;letter-spacing:.16em;font-weight:700;color:var(--accent)}h1{font-size:clamp(2.4rem,5vw,4.6rem);line-height:1.05;letter-spacing:-.05em;margin:16px 0}h2{font-size:1.6rem;margin:0 0 18px}h3{margin:0 0 12px}p{margin:10px 0}.muted,small{color:var(--muted)}nav{display:flex;flex-wrap:wrap;gap:16px;margin:16px 0}.cards{display:grid;grid-template-columns:repeat(4,1fr);gap:16px;margin:24px 0}.card,section,article{background:white;border:1px solid var(--line);border-radius:10px;padding:24px}.card strong{display:block;font-size:2rem}.card span{font-size:.8rem;color:var(--muted)}section{margin-bottom:24px}article{margin:20px 0;padding:20px;border-left:4px solid var(--accent)}.badge{display:inline-block;padding:3px 9px;background:#e4f1ed;border-radius:4px;font-size:.8rem;font-weight:650}.gap{border-left:4px solid #ab5c17;background:#fff7ea;padding:16px}.conflict{border-left-color:#ab5c17}details{margin-top:16px;border-top:1px solid var(--line);padding-top:12px}summary{cursor:pointer;font-weight:650;padding:6px}summary:focus-visible,a:focus-visible{outline:3px solid #13766e;outline-offset:3px}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#eef3f1;padding:16px;font: .9rem/1.6 ui-monospace,monospace}table{min-width:640px;width:100%;border-collapse:collapse;font-size:.9rem}th,td{text-align:left;vertical-align:top;padding:12px;border-bottom:1px solid var(--line);overflow-wrap:anywhere}th{color:var(--muted);font-weight:600}.profile-matrix th:not(:first-child){min-width:76px;text-align:center;font-size:.85rem;padding:10px 6px}.profile-matrix tbody th,.profile-matrix thead th:first-child{position:sticky;left:0;background:white;z-index:1;min-width:180px;max-width:260px}.profile-matrix tbody th{color:var(--ink)}.profile-matrix td{position:relative;text-align:center;font-size:1.2rem}.profile-matrix .enabled{color:var(--accent);font-weight:750}.profile-matrix small{display:block;font-weight:400}.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;border:0}.table-wrap{overflow-x:auto}.skip{position:absolute;left:-10000px}.skip:focus{left:16px;top:16px;background:white;padding:12px}footer{font-size:.8rem;color:var(--muted)}@media(max-width:700px){header,main,footer{padding:20px}.cards{grid-template-columns:repeat(2,1fr)}section,article{padding:16px}}@media print{.profile-matrix{table-layout:fixed;min-width:0;width:100%;font-size:7pt}.profile-matrix th:not(:first-child){min-width:0;font-size:6pt;padding:4px}.profile-matrix tbody th,.profile-matrix thead th:first-child{position:static;min-width:0;max-width:none;width:22%;font-size:7pt;padding:4px}.profile-matrix td{font-size:8pt;padding:4px}body{background:white;font-size:10pt}header,main,footer{max-width:none;padding:12px}nav,.skip{display:none}section,article,.card{break-inside:avoid;border-radius:0}details::details-content{content-visibility:visible!important;display:block!important}details>*{display:block!important}summary{list-style:none}pre{white-space:pre-wrap}a{color:inherit;text-decoration:none}.cards{grid-template-columns:repeat(4,1fr)}}
"#;
fn state(status: Status) -> &'static str {
    match status {
        Status::Complete => "complete",
        Status::Partial => "partial",
        Status::Unavailable => "unavailable",
    }
}

#[cfg(test)]
fn render(
    e: &Evidence,
    title: &str,
    secrets: &SecretSet,
) -> Result<String, crate::error::CliError> {
    render_with_spans(e, title, secrets, &privacy::PayloadSpans::default())
}

#[cfg(test)]
pub fn render_with_spans(
    e: &Evidence,
    title: &str,
    secrets: &SecretSet,
    payload: &privacy::PayloadSpans,
) -> Result<String, crate::error::CliError> {
    render_reviewed(e, title, secrets, payload, None)
}
pub fn render_reviewed(
    e: &Evidence,
    title: &str,
    secrets: &SecretSet,
    payload: &privacy::PayloadSpans,
    review: Option<&super::review::Review>,
) -> Result<String, crate::error::CliError> {
    let _ = review.map(|r| r.safe_clone(secrets, payload));
    // Discover title credentials first, but finalize title only after evidence discovery.
    let _ = privacy::text(title, secrets, 240);
    // Title discovery can register credentials carried by earlier evidence fields.
    let prior_projection = e.digest()? != e.digest;
    let mut safe = e.clone();
    let additional_projection = privacy::project_with_spans(&mut safe, secrets, payload)?;
    let title = h(&privacy::text_with_spans(
        title, secrets, payload, "title", 240,
    ));
    let e = &safe;
    let review = review.map(|r| r.safe_clone(secrets, payload));
    if let Some(r) = &review {
        r.validate_findings(e)?;
    }
    let a = analyze(e);
    let timestamp = |ms| {
        h(&privacy::text_with_spans(
            &crate::time::iso(ms),
            secrets,
            payload,
            "timestamp",
            120,
        ))
    };
    let css_hash = format!("sha256-{}", base64(&Sha256::digest(CSS.as_bytes())));
    let policy = format!(
        "default-src 'none'; style-src '{css_hash}'; base-uri 'none'; form-action 'none'; connect-src 'none'; img-src 'none'"
    );
    let mut out = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"{}\"><meta name=\"referrer\" content=\"no-referrer\"><title>{title} — AIRS Signal Brief</title><style>{CSS}</style></head><body><a class=\"skip\" href=\"#main\">Skip to brief</a><header><div class=\"eyebrow\">Prisma AIRS · Security intelligence</div><h1>AIRS Signal Brief</h1><p>{title}</p><p>Tenant <strong>{}</strong> · Scope: {}</p><p class=\"muted\">Reporting window: {} → {} UTC · Collected: {} · Rendered: {}</p><span class=\"badge\">Read-only evidence · Rules v{}</span></header><main id=\"main\"><nav aria-label=\"Brief sections\"><a href=\"#summary\">Summary</a><a href=\"#findings\">Findings</a><a href=\"#profiles\">Profile posture</a><a href=\"#review\">Review queue</a><a href=\"#coverage\">Coverage</a></nav>",
        h(&policy),
        h(&e.tenant),
        if e.app_ids.is_empty() {
            "Entire selected tenant".into()
        } else {
            h(&e.app_ids.join(", "))
        },
        timestamp(e.window.start_ms),
        timestamp(e.window.end_ms),
        timestamp(e.collected_ms),
        timestamp(crate::time::now_ms()),
        e.rules_version
    );
    out.push_str("<section id=\"summary\"><div class=\"eyebrow\">01 / Executive summary</div><h2>What the evidence shows</h2>");
    if prior_projection || additional_projection {
        out.push_str(&format!("<p class=\"gap\">Additional privacy withholding was applied to this view. Original evidence redaction version: {}; applied view redaction version: 1. The source file and its collection digest are unchanged.</p>",e.redaction_version));
    }
    if a.required_incomplete > 0 || e.sources.is_empty() {
        out.push_str("<p class=\"gap\">Required collection is incomplete or unavailable. Findings describe available evidence; missing data is not a clean environment.</p>");
    }
    if e.health_unknown() {
        out.push_str("<p class=\"gap\">Unable to assess: no primary source established usable observations.</p>");
    }
    out.push_str("<div class=\"cards\">");
    for (n, label) in [
        (a.eligible_events, "Collected in-window events"),
        (a.distinct_scans, "Distinct in-window scans"),
        (
            if e.rules_version >= 2 {
                a.metadata_observations
            } else {
                a.violation_rows
            },
            if e.rules_version >= 2 {
                "Detector-positive metadata observations"
            } else {
                "Sampled malicious detector rows"
            },
        ),
        (a.findings.len(), "Grouped review findings"),
    ] {
        out.push_str(&format!(
            "<div class=\"card\"><strong>{n}</strong><span>{label}</span></div>"
        ));
    }
    out.push_str("</div><p>Detector verdicts describe detections. Enforcement is the observed scan action. Allowed detections can be intentional monitoring; they do not establish a misconfiguration or confirmed attack.</p>");
    session_activity(&mut out, e);
    out.push_str(&format!("<p class=\"muted\">Detail sampling: {} selected report IDs, cap {}, from {} discovered candidates. Selection is coverage-oriented, not statistically representative. Display limits: first 250 ranked groups, 10 detector rows per group and 2 excerpts per snippet family; counts retain all collected eligible evidence.</p></section>",e.sampling.selected_report_ids.len(),e.sampling.limit,e.sampling.candidates));
    out.push_str("<section id=\"findings\"><div class=\"eyebrow\">02 / Findings &amp; prioritized actions</div><h2>Patterns to investigate</h2><p class=\"muted\">Unreviewed observations. Priority follows detector review category, recurrence, then stable group keys; allow alone does not increase priority.</p>");
    if a.findings.is_empty() {
        out.push_str("<p>No grouped malicious detector findings in eligible collected evidence. This does not establish an absence of violations in uncollected traffic.</p>");
    }
    let (volume_title, volume_unit) = if e.rules_version >= 2 {
        (
            "Detector volumes in collected metadata",
            "Detector-positive metadata observations",
        )
    } else {
        (
            "Detector volumes in sampled evidence",
            "Malicious detector rows",
        )
    };
    out.push_str(&format!("<h3>{volume_title}</h3><div class=\"table-wrap\" tabindex=\"0\"><table><caption>Counts from eligible collected evidence, not population estimates</caption><thead><tr><th scope=\"col\">Detector / direction</th><th scope=\"col\">{volume_unit}</th><th scope=\"col\">Distinct events</th><th scope=\"col\">Distinct scans</th></tr></thead><tbody>"));
    #[derive(Default)]
    struct Volume<'a> {
        rows: usize,
        events: std::collections::BTreeSet<usize>,
        scans: std::collections::BTreeSet<&'a str>,
    }
    let mut volumes: std::collections::BTreeMap<(&str, &str), Volume<'_>> =
        std::collections::BTreeMap::new();
    for f in &a.findings {
        let entry = volumes
            .entry((&f.group.detector, &f.group.direction))
            .or_default();
        entry.rows += if e.rules_version >= 2 {
            f.metadata_observations
        } else {
            f.rows.len()
        };
        for i in &f.event_indices {
            entry.events.insert(*i);
            entry.scans.insert(e.events[*i].identity.scan_id.as_str());
        }
    }
    for ((detector, direction), volume) in volumes.iter().take(250) {
        out.push_str(&format!(
            "<tr><td>{} / {}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            h(detector),
            h(direction),
            volume.rows,
            volume.events.len(),
            volume.scans.len()
        ));
    }
    out.push_str("</tbody></table><p>Display cap: 250 detector/direction rows. Units differ; no percentages are compared across populations.</p></div>");
    if e.rules_version >= 2 {
        out.push_str("<p>Detailed detector rows and snippets are supplemental examples. Metadata observation counts are event/detector/direction combinations, not report rows. Unknown-detector malicious classifications have zero detector-positive counts. </p>");
    }
    if a.ambiguous_rows > 0 {
        out.push_str(&format!("<p class=\"gap\">{} repeated report row(s) have conflicting identity associations. They remain in evidence provenance and are excluded from findings and malicious-row counts until attribution is resolved.</p>",a.ambiguous_rows));
    }
    for (rank, f) in a.findings.iter().take(250).enumerate() {
        let assessment = review
            .as_ref()
            .and_then(|r| r.assessments.iter().find(|a| a.finding_id == f.id));
        let review_status = match (
            assessment,
            review
                .as_ref()
                .and_then(|r| r.reviewer.as_ref())
                .map(|r| r.kind),
        ) {
            (Some(_), Some(super::review::ReviewerKind::Agent)) => "Agent-assessed",
            (Some(_), Some(super::review::ReviewerKind::Human)) => "Human-reviewed",
            _ => "Unreviewed",
        };
        out.push_str(&format!("<article id=\"{}\"{}><span class=\"badge\">Review priority {} · {review_status}</span><h3>{} · {}</h3><p>Application: {} ({}) · Profile observed: {}<br>Direction: {} · Scan enforcement: {}</p><p>{}</p><p><strong>Recommended review:</strong> {}</p><details><summary>Expand evidence — {} event(s), {} distinct scan(s), {} detector row(s)</summary>",h(&f.id),if f.conflicting{" class=\"conflict\""}else{""},rank+1,h(&f.group.detector),h(&f.group.app_name),h(&f.group.app_name),h(&f.group.app_id),h(f.group.profile.as_deref().unwrap_or("Unknown")),h(&f.group.direction),h(&f.group.action),h(&f.rationale),h(&f.recommendation),f.events,f.scans,f.rows.len()));
        if let (Some(assessment), Some(reviewer)) = (
            assessment,
            review.as_ref().and_then(|r| r.reviewer.as_ref()),
        ) {
            out.push_str(&format!("<aside><h4>{review_status} assessment</h4><p>Reviewer: {} · Model: {} · Time: {} (self-attributed; not authenticated)</p><p>Assessment: {} · Proposed priority: {}</p><p>{}</p><p>API verdicts, enforcement, collection completeness and counts are unchanged. Proposed priority does not reorder deterministic CLI findings.</p></aside>",h(&reviewer.name),h(reviewer.model.as_deref().unwrap_or("Not supplied")),h(&reviewer.reviewed_at),assessment.classification.label(),assessment.priority.label(),h(&assessment.rationale)));
        }
        if e.rules_version >= 2 {
            for i in f.event_indices.iter().take(10) {
                let ev = &e.events[*i];
                out.push_str(&format!("<p><strong>Metadata observation:</strong> Scan {} · Sub-request {} · Session {}<br>Observed detector hints: {}<br>Scan verdict/action: {} / {} · Sources: {}<br>Notes: {}</p>",h(&ev.identity.scan_id),ev.identity.sub_request.map_or_else(||"Unknown".into(),|n|n.to_string()),h(&ev.identity.session_id),h(&ev.detector_hints.join(", ")),h(&ev.verdict),h(&ev.action),h(&ev.sources.join(", ")),h(&ev.notes.join("; "))));
            }
            if f.rows.is_empty() {
                out.push_str("<p class=\"muted\">No detailed example attached; this finding is supported by metadata. No detector-level action or payload assessment is inferred.</p>");
            }
        }
        for (i, j) in f.rows.iter().take(10) {
            let ev = &e.events[*i];
            let d = &ev.detections[*j];
            out.push_str(&format!("<div><p><strong>Scan:</strong> {} · Sub-request: {} · Session: {}<br>Report: {} · Row: {}<br>Scan verdict/action: {} / {} · Detector verdict/action: {} / {}<br>Raw detector / direction: {} / {}</p>",h(&ev.identity.scan_id),ev.identity.sub_request.map_or_else(||"Unknown".into(),|n|n.to_string()),h(&ev.identity.session_id),h(&d.report_id),d.row,h(&ev.verdict),h(&ev.action),h(&d.verdict),h(&d.action),h(&d.detector),h(&d.direction)));
            for snippet in &d.snippets {
                out.push_str(&format!(
                    "<p class=\"muted\">{} snippets: {:?} · {} sampled variant(s)</p>",
                    h(&snippet.family),
                    snippet.state,
                    snippet.variants
                ));
                if snippet.truncated {
                    out.push_str("<p class=\"muted\">Stored snippet excerpts were capped; variant counts describe the received snippet values.</p>");
                }
                for text in snippet.excerpts.iter().take(2) {
                    out.push_str(&format!("<pre>{}</pre>", h(text)));
                }
            }
            out.push_str("</div>");
        }
        out.push_str("</details></article>");
    }
    out.push_str("</section><section id=\"profiles\"><div class=\"eyebrow\">03 / Current profile posture</div><h2>Compare configured protections</h2><p>Profiles are a tenant-wide current inventory; relevance to filtered applications requires explicit association. These are current configuration observations, not historical event-time policy.</p>");
    profile_posture(&mut out, e);
    out.push_str("</section><section id=\"review\"><div class=\"eyebrow\">04 / Noise &amp; unreviewed evidence</div><h2>Decisions still requiring review</h2>");
    let assessed = review.as_ref().map_or(0, |r| r.assessments.len());
    let kind = review
        .as_ref()
        .and_then(|r| r.reviewer.as_ref())
        .map(|r| r.kind);
    out.push_str(&format!("<p>{} agent-assessed finding(s); {} human-reviewed finding(s). These are self-attributed judgments supplied by the packet reviewer.</p>",if kind==Some(super::review::ReviewerKind::Agent){assessed}else{0},if kind==Some(super::review::ReviewerKind::Human){assessed}else{0}));
    out.push_str(&format!("<p>{} grouped finding(s) are unreviewed. {} benign or unknown detector row(s) are retained as context and excluded from malicious-row totals. {} event(s) lack timestamps; {} are outside the target window.</p><p>No test-traffic, false-positive or confirmed-threat classification has been inferred. Imported assessments are attributed separately from API observations.</p></section>",a.findings.len().saturating_sub(assessed),a.nonviolation_rows,a.unknown_time_events,a.outside_window_events));
    out.push_str("<section id=\"coverage\"><div class=\"eyebrow\">05 / Collection gaps &amp; methodology</div><h2>Coverage and provenance</h2><div class=\"table-wrap\" tabindex=\"0\"><table><caption>Source collection status</caption><thead><tr><th scope=\"col\">Source</th><th scope=\"col\">Status</th><th scope=\"col\">Population / unit</th><th scope=\"col\">Collected</th><th scope=\"col\">Notes</th></tr></thead><tbody>");
    for s in &e.sources {
        out.push_str(&format!("<tr><td>{} ({})</td><td>{}</td><td>{} / {}</td><td>{} records / {} pages</td><td>{}</td></tr>",h(&s.name),if s.required{"required"}else{"supplemental"},state(s.status),h(&s.population),h(&s.unit),s.records,s.pages,h(&s.notes.join("; "))));
    }
    out.push_str("</tbody></table></div>");
    out.push_str("<div class=\"table-wrap\" tabindex=\"0\"><table><caption>Collected telemetry with explicit populations and scales</caption><thead><tr><th scope=\"col\">Metric</th><th scope=\"col\">Value / unit / scale</th><th scope=\"col\">Population</th><th scope=\"col\">Window</th><th scope=\"col\">Source / status</th></tr></thead><tbody>");
    for m in e.metrics.iter().take(250) {
        out.push_str(&format!(
            "<tr><td>{}</td><td>{} / {} / {}</td><td>{}</td><td>{}</td><td>{} / {}</td></tr>",
            h(&m.name),
            m.value.map_or_else(|| "Unknown".into(), |v| v.to_string()),
            h(&m.unit),
            h(m.scale.as_deref().unwrap_or("unscaled or unknown")),
            h(&m.population),
            h(&m.window),
            h(&m.source),
            state(m.status)
        ));
    }
    out.push_str("</tbody></table><p>Display cap: 250 telemetry rows. Server averages, monthly totals and event counts describe different populations and windows.</p></div>");
    for group in &e.sampling.uncovered_groups {
        out.push_str(&format!(
            "<p class=\"gap\">Uncovered sampling group: {}</p>",
            h(group)
        ));
    }
    out.push_str(&format!("<p>Requests: {} / {}; collection deadline: {} seconds; concurrency cap: {}. Missing snippets differ from empty or withheld snippets. Population totals are never extrapolated from report samples.</p><p>Events are keyed by application ID, exact application name, session, scan and sub-request; detector rows retain report identity. Grouping retains profile, detector, direction and scan enforcement. Current configuration is compared without claiming historical drift. Owner mappings and prior snapshots are not included.</p><p>Secrets and DLP matched values are withheld; excerpts may retain confidential context. No full stored prompts/responses are retrieved automatically. This artifact is private, not a security certification.</p></section></main><footer>Collection {} · Evidence schema {} · Analysis rules {} · Redaction {} · Self-contained HTML; no network access or JavaScript required.</footer></body></html>\n",e.budgets.attempts,e.budgets.requests,e.budgets.seconds,e.budgets.concurrency,h(&e.collection_id),e.schema_version,e.rules_version,e.redaction_version));
    if out.len() > 32 * 1024 * 1024 {
        return Err(super::invalid());
    }
    Ok(out)
}
fn session_activity(out: &mut String, e: &Evidence) {
    let mut buckets: std::collections::BTreeMap<(&str, &str), Vec<&super::evidence::Metric>> =
        std::collections::BTreeMap::new();
    for metric in &e.metrics {
        if metric.source == "applications"
            && metric.unit == "sessions"
            && ["sessions_total", "sessions_violated"].contains(&metric.name.as_str())
        {
            buckets
                .entry((&metric.population, &metric.window))
                .or_default()
                .push(metric);
        }
    }
    if buckets.is_empty() {
        return;
    }
    out.push_str("<h3>Management session activity</h3><p>These API aggregates describe sessions, including activity outside the retained event sample. Violated sessions are separate from scans, detector observations and blocked requests. Unresolved scanner outcomes do not erase reported session violations.</p><div class=\"table-wrap\" tabindex=\"0\"><table><caption>Reported session aggregates by application bucket</caption><thead><tr><th scope=\"col\">Population</th><th scope=\"col\">Reported window</th><th scope=\"col\">Total sessions</th><th scope=\"col\">Violated sessions</th><th scope=\"col\">Data quality</th></tr></thead><tbody>");
    let mut contradictory = false;
    for ((population, window), metrics) in buckets.iter().take(20) {
        let value = |name: &str| {
            let values: Vec<_> = metrics.iter().filter(|m| m.name == name).collect();
            if values.len() == 1
                && values[0].status == Status::Complete
                && values[0].scale.is_none()
            {
                values[0].value.filter(|v| {
                    v.is_finite()
                        && *v >= 0.0
                        && *v <= crate::limits::MAX_SAFE_INTEGER as f64
                        && v.fract() == 0.0
                })
            } else {
                None
            }
        };
        let total = value("sessions_total");
        let violated = value("sessions_violated");
        let inconsistent = total
            .zip(violated)
            .is_some_and(|(total, violated)| violated > total);
        contradictory |= inconsistent;
        let shown = |value: Option<f64>| {
            if inconsistent {
                "Unknown".into()
            } else {
                value.map_or_else(|| "Unknown".into(), |v| v.to_string())
            }
        };
        out.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            h(population),
            h(window),
            shown(total),
            shown(violated),
            if inconsistent {
                "Contradictory counts withheld"
            } else if total.is_none() || violated.is_none() {
                "Missing, unavailable, ambiguous or scaled metric"
            } else {
                "Reported aggregate"
            }
        ));
    }
    out.push_str("</tbody></table></div>");
    if contradictory {
        out.push_str("<p class=\"gap\">Some reported violated-session counts exceed total sessions for the same population and window. Contradictory counts are shown as Unknown; original metrics remain in the evidence.</p>");
    }

    if buckets.len() > 20 {
        out.push_str(&format!("<p class=\"muted\">Showing 20 of {} reported application buckets; all retained metrics remain in the evidence.</p>", buckets.len()));
    }
}

fn service_label(detector: &str) -> &str {
    match detector {
        "prompt-injection" => "Prompt injection",
        "toxic-content" => "Toxic content",
        "dlp" => "DLP",
        "url-filtering" => "URL filtering",
        "malicious-code" => "Malicious code",
        "source-code" => "Source code",
        "topic-violation" => "Topic guardrails",
        "database-security" => "Database security",
        "agent" => "Agent",
        "agent-security" => "Agent security",
        "topic-guardrails" => "Topic guardrails",
        "database-security-create" => "DB create",
        "database-security-read" => "DB read",
        "database-security-update" => "DB update",
        "database-security-delete" => "DB delete",
        "ungrounded" => "Grounding",
        "" => "Unspecified service",
        other => other,
    }
}
/// The profile/detection-service matrix (profiles on rows, services on columns,
/// explicit enablement only) and its configuration details, as rendered in the brief.
/// Reused by the runtime service's historical report; uses this module's CSS classes.
pub fn profile_matrix(e: &Evidence) -> String {
    let mut out = String::new();
    profile_posture(&mut out, e);
    out
}
fn profile_posture(out: &mut String, e: &Evidence) {
    use std::collections::{BTreeMap, BTreeSet};
    const MAX_PROFILES: usize = 250;
    const MAX_SERVICES: usize = 32;
    const MAX_DETAIL_ROWS: usize = 250;
    if e.profiles.is_empty() {
        out.push_str("<p>No security profiles are available in the collected evidence. Check source coverage before drawing conclusions about configured protections.</p>");
        return;
    }
    let services: BTreeSet<_> = e
        .profiles
        .iter()
        .flat_map(|p| {
            p.protections
                .iter()
                .map(|c| super::detectors::canonical(&c.detector))
        })
        .collect();
    let columns: Vec<_> = services.iter().copied().take(MAX_SERVICES).collect();
    let mut profiles: Vec<_> = e.profiles.iter().collect();
    profiles.sort_by(|a, b| a.name.cmp(&b.name).then(b.revision.cmp(&a.revision)));
    out.push_str("<p id=\"profile-legend\">✓ explicitly enabled in at least one direction or shared section. Blank cells do not establish enablement; expand configuration details for inactive or unknown settings. A check mark does not imply block enforcement or coverage in every direction.</p><div class=\"table-wrap\" tabindex=\"0\"><table id=\"profile-enablement\" class=\"profile-matrix\" aria-describedby=\"profile-legend\"><caption>Detection services enabled by security profile</caption><thead><tr><th scope=\"col\">Security profile</th>");
    for detector in &columns {
        out.push_str(&format!(
            "<th scope=\"col\">{}</th>",
            h(service_label(detector))
        ));
    }
    out.push_str("</tr></thead><tbody>");
    for p in profiles.iter().take(MAX_PROFILES) {
        out.push_str(&format!(
            "<tr><th scope=\"row\">{}<small>Revision {}</small></th>",
            h(&p.name),
            p.revision
                .map_or_else(|| "unknown".into(), |n| n.to_string())
        ));
        let mut controls: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for c in &p.protections {
            controls
                .entry(super::detectors::canonical(&c.detector))
                .or_default()
                .push(c);
        }
        for detector in &columns {
            let observed = controls.get(detector);
            let enabled = !detector.is_empty()
                && observed.is_some_and(|rows| {
                    rows.iter().any(|c| c.active && c.confidence == "explicit")
                });
            if enabled {
                out.push_str("<td class=\"enabled\" data-state=\"enabled\"><span aria-hidden=\"true\">✓</span><span class=\"sr-only\">Enabled</span></td>");
            } else {
                let inactive = observed.is_some_and(|rows| {
                    rows.iter().all(|c| !c.active && c.confidence == "explicit")
                });
                let (state, label) = if inactive {
                    ("inactive", "Explicitly inactive")
                } else {
                    ("unknown", "Enablement unknown")
                };
                out.push_str(&format!("<td data-state=\"{state}\" title=\"{label}\"><span class=\"sr-only\">{label}</span></td>"));
            }
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div>");
    if columns.is_empty() {
        out.push_str("<p class=\"muted\">Detection-service configuration is unavailable or unrecognized; no enabled service is established.</p>");
    }
    if e.profiles.len() > MAX_PROFILES || services.len() > MAX_SERVICES {
        out.push_str(&format!("<p class=\"gap\">Showing {} of {} profiles and {} of {} detection services. Matrix display caps: 250 profiles and 32 services; full collected configuration remains in bundled evidence.</p>",e.profiles.len().min(MAX_PROFILES),e.profiles.len(),columns.len(),services.len()));
    }
    out.push_str("<details><summary>Direction and action details</summary><div class=\"table-wrap\" tabindex=\"0\"><table><caption>Observed configuration by profile, raw service and direction</caption><thead><tr><th scope=\"col\">Profile / revision</th><th scope=\"col\">Detection service</th><th scope=\"col\">Direction</th><th scope=\"col\">Configured action and state</th></tr></thead><tbody>");
    let mut rows = 0;
    for p in profiles {
        for c in &p.protections {
            if rows == MAX_DETAIL_ROWS {
                break;
            }
            rows += 1;
            out.push_str(&format!(
                "<tr><td>{} / {}</td><td>{}</td><td>{}</td><td>{} · {} · {} · {}</td></tr>",
                h(&p.name),
                p.revision
                    .map_or_else(|| "Unknown".into(), |n| n.to_string()),
                h(&c.detector),
                h(&c.direction),
                h(&c.action),
                if c.active {
                    "active"
                } else {
                    "inactive or unknown"
                },
                h(&c.confidence),
                h(&c.location)
            ));
        }
        if rows == MAX_DETAIL_ROWS {
            break;
        }
    }
    out.push_str("</tbody></table><p>Display cap: 250 configuration detail rows; remaining configuration is retained in saved evidence when bundled.</p></div></details>");
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_summary_keeps_windows_distinct_and_refuses_ambiguous_counts() {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        e.metrics.clear();
        let metric = |name: &str, value: f64, window: &str| super::super::evidence::Metric {
            name: name.into(),
            value: Some(value),
            unit: "sessions".into(),
            scale: None,
            population: "Application <synthetic>".into(),
            window: window.into(),
            source: "applications".into(),
            status: Status::Complete,
        };
        e.metrics
            .push(metric("sessions_total", 256.0, "window-one"));
        e.metrics
            .push(metric("sessions_violated", 33.0, "window-two"));
        e.metrics
            .push(metric("sessions_violated", 34.0, "window-two"));
        let mut out = String::new();
        session_activity(&mut out, &e);
        assert!(out.contains("&lt;synthetic&gt;"));
        assert!(out.contains("<td>window-one</td><td>256</td><td>Unknown</td>"));
        assert!(out.contains("<td>window-two</td><td>Unknown</td><td>Unknown</td>"));
        assert!(!out.contains(">33</td>"));
        for n in 0..25 {
            let mut m = metric("sessions_total", -1.0, "window-three");
            m.population = format!("bounded-{n:03}");
            e.metrics.push(m);
        }
        out.clear();
        session_activity(&mut out, &e);
        assert!(out.contains("Showing 20 of 27"));
        assert!(out.contains("bounded-017"));
        assert!(!out.contains("bounded-018"));
        assert!(!out.contains(">-1</td>"));
        e.metrics = vec![
            metric("sessions_total", 1.0, "same"),
            metric("sessions_violated", 33.0, "same"),
        ];
        out.clear();
        session_activity(&mut out, &e);
        assert!(out.contains("Contradictory counts withheld"));
        assert!(out.contains("class=\"gap\""));
        assert!(!out.contains(">33</td>"));
        e.metrics[0].value = Some(256.0);
        e.metrics[0].scale = Some("thousands".into());
        out.clear();
        session_activity(&mut out, &e);
        assert!(out.contains("scaled metric"));
        assert!(!out.contains(">256</td>"));
    }
    #[test]
    fn standalone_document_escapes_every_evidence_surface() {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        let attack = "</details><script>alert(1)</script>";
        e.tenant = attack.into();
        e.events[0].identity.app_name = attack.into();
        e.events[0].detections[0].snippets[0].excerpts = vec![attack.into()];
        let document = render(&e, attack, &SecretSet::default()).unwrap();
        assert!(!document.contains("<script"));
        assert!(!document.contains("<img"));
        assert!(!document.contains(" src="));
        assert!(!document.contains("https://"));
        assert!(document.contains("&lt;script&gt;"));
        assert!(document.contains(&format!(
            "style-src &#39;sha256-{}&#39;",
            base64(&Sha256::digest(CSS.as_bytes()))
        )));
        assert!(document.contains("<summary>"));
        assert!(document.contains("@media print"));
        assert!(document.contains("scope=\"col\""));
        assert!(document.len() < 100_000);
    }
    #[test]
    fn empty_and_incomplete_are_not_clean_environment_claims() {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        e.events.clear();
        e.sources[0].status = Status::Unavailable;
        let doc = render(&e, "Brief", &SecretSet::default()).unwrap();
        assert!(doc.contains("missing data is not a clean environment"));
        assert!(doc.contains("does not establish an absence"));
        assert!(doc.contains("unavailable"));
    }
}

#[cfg(test)]
mod bound_tests {
    use super::*;
    #[test]
    fn profile_rows_and_evidence_rows_are_actually_capped() {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        let template = e.profiles[0].protections[0].clone();
        e.profiles[0].protections = (0..300)
            .map(|i| {
                let mut p = template.clone();
                p.detector = format!("bounded-detector-{i:04}");
                p
            })
            .collect();
        let detection = e.events[0].detections[0].clone();
        e.events[0].detections = (0..20)
            .map(|i| {
                let mut d = detection.clone();
                d.row = i;
                d.snippets[0].excerpts = (0..10).map(|j| format!("example-{i}-{j}")).collect();
                d
            })
            .collect();
        let doc = render(&e, "Brief", &SecretSet::default()).unwrap();
        assert!(doc.contains("bounded-detector-0249"));
        assert!(!doc.contains("bounded-detector-0250"));
        assert!(doc.contains("example-9-1"));
        assert!(!doc.contains("example-10-0"));
        assert!(!doc.contains("example-0-2"));
        assert!(doc.contains("Detector volumes in sampled evidence"));
        assert!(doc.contains("250 configuration detail rows"));
    }
}

#[cfg(test)]
mod discovery_tests {
    use super::*;
    #[test]
    fn renderer_finalizes_review_after_evidence_secret_discovery() {
        use super::super::review::*;
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        let canary = "REVIEW-EVIDENCE-CANARY-0123456789";
        e.events[0]
            .notes
            .push(format!("Authorization: Bearer {canary}"));
        let review = Review {
            reviewer: Some(Reviewer {
                kind: ReviewerKind::Agent,
                name: canary.into(),
                model: Some("Synthetic model".into()),
                reviewed_at: "2026-11-02T12:00:00Z".into(),
            }),
            assessments: vec![Assessment {
                finding_id: analyze(&e).findings[0].id.clone(),
                classification: Classification::NeedsInvestigation,
                priority: Priority::Medium,
                rationale: canary.into(),
            }],
        };
        let doc = render_reviewed(
            &e,
            "Brief",
            &SecretSet::default(),
            &privacy::PayloadSpans::default(),
            Some(&review),
        )
        .unwrap();
        assert!(!doc.contains(canary));
        assert!(doc.contains("Agent-assessed"));
    }
    #[test]
    fn renderer_discovers_evidence_secrets_before_finalizing_title() {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        let canary = "RENDER-CANARY-secret-123456789";
        e.events[0]
            .notes
            .push(format!("Authorization: Bearer {canary}"));
        let doc = render(&e, canary, &SecretSet::default()).unwrap();
        assert!(!doc.contains(canary));
    }
}

#[cfg(test)]
mod profile_matrix_tests {
    use super::*;
    #[test]
    fn matrix_has_profile_rows_service_columns_and_checks_only_explicit_enablement() {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence-v2.json"
        ))
        .unwrap();
        let mut inactive = e.profiles[0].clone();
        inactive.name = "Inactive".into();
        inactive.protections[0].active = false;
        let mut unknown = e.profiles[0].clone();
        unknown.name = "Unknown state".into();
        unknown.protections[0].confidence = "unknown-layout".into();
        e.profiles.push(inactive);
        e.profiles.push(unknown);
        let mut alias = e.profiles[0].protections[0].clone();
        alias.detector = "pi".into();
        alias.direction = "response".into();
        alias.action = "block".into();
        e.profiles[0].protections.push(alias);
        let doc = render(&e, "Brief", &SecretSet::default()).unwrap();
        let matrix = doc
            .split("<table id=\"profile-enablement\"")
            .nth(1)
            .unwrap()
            .split("</table>")
            .next()
            .unwrap();
        assert_eq!(matrix.matches("scope=\"row\"").count(), 4);
        assert_eq!(
            matrix
                .matches("<th scope=\"col\">Prompt injection</th>")
                .count(),
            1
        );
        assert_eq!(matrix.matches("data-state=\"enabled\"").count(), 2);
        assert!(matrix.contains("data-state=\"inactive\""));
        assert!(matrix.contains("data-state=\"unknown\""));
        assert!(!matrix.contains("Configured action"));
        assert!(doc.contains("Direction and action details"));
        assert!(doc.contains("pi</td><td>response</td><td>block"));
        assert!(doc.contains("A check mark does not imply block enforcement"));
    }
    #[test]
    fn matrix_caps_rows_and_columns_and_escapes_profile_and_service_names() {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence-v2.json"
        ))
        .unwrap();
        let template = e.profiles[0].clone();
        e.profiles = (0..251)
            .map(|i| {
                let mut p = template.clone();
                p.name = format!("profile-{i:03}");
                p
            })
            .collect();
        let control = e.profiles[0].protections[0].clone();
        e.profiles[0].protections = (0..33)
            .map(|i| {
                let mut c = control.clone();
                c.detector = format!("service-{i:03}");
                c
            })
            .collect();
        e.profiles[0].name = "<script>profile</script>".into();
        e.profiles[0].protections[0].detector = "<script>service</script>".into();
        let doc = render(&e, "Brief", &SecretSet::default()).unwrap();
        let matrix = doc
            .split("<table id=\"profile-enablement\"")
            .nth(1)
            .unwrap()
            .split("</table>")
            .next()
            .unwrap();
        assert_eq!(matrix.matches("scope=\"row\"").count(), 250);
        assert_eq!(matrix.matches("scope=\"col\"").count(), 33);
        assert!(doc.contains("Showing 250 of 251 profiles and 32 of 34 detection services"));
        assert!(matrix.contains("&lt;script&gt;profile&lt;/script&gt;"));
        assert!(matrix.contains("&lt;script&gt;service&lt;/script&gt;"));
        assert!(!matrix.contains("<script>"));
    }
}
