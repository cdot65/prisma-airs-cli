//! The historical report page (`report.html`, plan M6.1 "HTML filters/profile
//! matrix"; plan §Historical reports). Service-only: the CLI's Signal Brief renderer
//! and its script-free output are unchanged.
//!
//! - Self-contained and deterministic: the same input renders the same bytes (no
//!   render clock, no random identifiers); no network, assets, or database paths.
//! - CSP: `default-src 'none'`, one hashed stylesheet and one hashed script, no style
//!   attributes, no event-handler attributes, Trusted Types required. All evidence is
//!   HTML-escaped; the summary data is a non-executable JSON block with `<`, `>`, `&`
//!   and line separators escaped, so no evidence text can end the block.
//! - Filters (report date, application, detector, verdict, action) progressively
//!   enhance the page. Counts, charts and tables are computed from the full-cohort
//!   summary, never from the capped detail; the detector filter uses detector-positive
//!   event counts, which overlap between detectors and are never summed. A filter the
//!   summary budget disabled is absent and stated. The detail table is filtered as
//!   detail and says it is capped.
//! - Without JavaScript, and in print, the page shows the base scope's summary,
//!   detail, posture and coverage.
//! - Bounds: at most 10,000 detail rows; the final page at most 32 MiB. A page over
//!   the byte ceiling is re-rendered without the detail table (stated); one still over
//!   it is refused.

use std::collections::{BTreeMap, BTreeSet};

use airs_history_store::report::{ReportInput, Summary};
use airs_runtime_core::error::SecretSet;
use airs_runtime_core::signal::{analyze::analyze, detectors, privacy};
use sha2::{Digest, Sha256};

pub const MAX_DETAIL_ROWS: usize = 10_000;
pub const MAX_HTML_BYTES: usize = 32 * 1024 * 1024;

/// What a page may hold.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub detail_rows: usize,
    pub html_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            detail_rows: MAX_DETAIL_ROWS,
            html_bytes: MAX_HTML_BYTES,
        }
    }
}

/// The page's identity, from the report's manifest.
pub struct Page<'a> {
    pub title: &'a str,
    pub kind: &'a str,
    pub version: u32,
    pub period: (i64, i64),
    pub cutoff_ms: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The evidence failed its privacy projection.
    Evidence,
    /// Even without the detail table the page exceeds its byte ceiling.
    TooLarge,
}

pub const CSS: &str = r#":root{color-scheme:light;--ink:#182b36;--muted:#526873;--line:#ccd8da;--accent:#13766e;--paper:#f3f5f2}*{box-sizing:border-box}body{overflow-wrap:anywhere;margin:0;background:var(--paper);color:var(--ink);font:16px/1.6 system-ui,sans-serif}a{color:#075e59}header,main,footer{max-width:1200px;margin:auto;padding:32px}header{padding-top:48px;border-bottom:3px solid var(--ink)}.eyebrow{font-size:.8rem;text-transform:uppercase;letter-spacing:.16em;font-weight:700;color:var(--accent)}h1{font-size:clamp(2rem,4vw,3.6rem);line-height:1.05;margin:16px 0}h2{font-size:1.5rem;margin:0 0 16px}h3{margin:20px 0 10px}nav{display:flex;flex-wrap:wrap;gap:16px;margin:16px 0}section{background:white;border:1px solid var(--line);border-radius:10px;padding:24px;margin-bottom:24px}.cards{display:grid;grid-template-columns:repeat(5,1fr);gap:12px;margin:16px 0}.card{border:1px solid var(--line);border-radius:8px;padding:14px}.card strong{display:block;font-size:1.8rem}.card span{font-size:.8rem;color:var(--muted)}.badge{display:inline-block;padding:3px 9px;background:#e4f1ed;border-radius:4px;font-size:.8rem;font-weight:650}.badge.partial{background:#fff0d9}.muted,small{color:var(--muted)}.gap{border-left:4px solid #ab5c17;background:#fff7ea;padding:12px 16px}form{display:flex;flex-wrap:wrap;gap:12px;align-items:end;margin:0 0 16px}form label{display:flex;flex-direction:column;font-size:.85rem;font-weight:600}select,button{font:inherit;padding:6px 8px;min-width:10rem}button{min-width:0;cursor:pointer}select:focus-visible,button:focus-visible,a:focus-visible,summary:focus-visible{outline:3px solid #13766e;outline-offset:2px}meter{width:12rem}table{min-width:560px;width:100%;border-collapse:collapse;font-size:.9rem}th,td{text-align:left;vertical-align:top;padding:8px 10px;border-bottom:1px solid var(--line)}th{color:var(--muted);font-weight:600}td.n{text-align:right;font-variant-numeric:tabular-nums}.table-wrap{overflow-x:auto}details{margin-top:12px}summary{cursor:pointer;font-weight:650}.profile-matrix th:not(:first-child){min-width:76px;text-align:center;font-size:.85rem;padding:10px 6px}.profile-matrix tbody th,.profile-matrix thead th:first-child{position:sticky;left:0;background:white;min-width:180px}.profile-matrix td{text-align:center;font-size:1.2rem}.profile-matrix .enabled{color:var(--accent);font-weight:750}.profile-matrix small{display:block;font-weight:400}.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;border:0}.skip{position:absolute;left:-10000px}.skip:focus{left:16px;top:16px;background:white;padding:12px}footer{font-size:.8rem;color:var(--muted)}@media(max-width:800px){header,main,footer{padding:16px}.cards{grid-template-columns:repeat(2,1fr)}}@media print{body{background:white;font-size:10pt}header,main,footer{max-width:none;padding:8px}nav,.skip,form,#filter-status,#live-summary{display:none!important}#base-summary{display:block!important}#detail-rows tr{display:table-row!important}section{break-inside:avoid;border-radius:0}details>*{display:block!important}meter{display:none}}"#;

/// The filter script. It reads the JSON summary block and the detail rows' `data-`
/// attributes, writes only through `textContent`, and never parses markup. Its pure
/// functions are exported (for tests) when no document exists.
pub const SCRIPT: &str = r##"(function(g){"use strict";
function match(key,dims,f){for(var i=0;i<dims.length;i++){var d=dims[i];if(f[d]!==undefined&&key[i]!==f[d])return false;}return true;}
function tally(D,f){var dims=D.dims,w=dims.length,src=D.cells;
if(f.detector!==undefined){src=(D.det||[]).filter(function(c){return c[w]===f.detector;});}
var t={total:0,by:Object.create(null),detectors:null};dims.forEach(function(d){t.by[d]=Object.create(null);});
src.forEach(function(c){if(!match(c,dims,f))return;var n=c[c.length-1];t.total+=n;dims.forEach(function(d,i){t.by[d][c[i]]=(t.by[d][c[i]]||0)+n;});});
if(D.det){t.detectors=Object.create(null);D.det.forEach(function(c){if(!match(c,dims,f))return;if(f.detector!==undefined&&c[w]!==f.detector)return;t.detectors[c[w]]=(t.detectors[c[w]]||0)+c[c.length-1];});}
return t;}
function rowMatches(attr,f){for(var d in f){if(!Object.prototype.hasOwnProperty.call(f,d))continue;var v=attr(d);if(v===null)return false;if(d==="detector"){if(v.split("\n").indexOf(f[d])<0)return false;}else if(v!==f[d])return false;}return true;}
if(typeof document==="undefined"){g.airsReport={tally:tally,rowMatches:rowMatches};return;}
var D=JSON.parse(document.getElementById("report-data").textContent);
var form=document.getElementById("filters"),live=document.getElementById("live-summary"),base=document.getElementById("base-summary"),status=document.getElementById("filter-status");
var rows=Array.prototype.slice.call(document.querySelectorAll("#detail-rows tr"));
var sels=Array.prototype.slice.call(form.querySelectorAll("select"));
var labels={day:"Report date",application:"Application",verdict:"Verdict",action:"Action"};
function el(tag,text,cls){var e=document.createElement(tag);if(text!==undefined)e.textContent=String(text);if(cls)e.className=cls;return e;}
function table(caption,head,entries,chart){var wrap=el("div",undefined,"table-wrap"),t=el("table"),tb=el("tbody"),tr=el("tr"),th=el("thead"),max=0;
t.appendChild(el("caption",caption));head.forEach(function(h){var c=el("th",h);c.setAttribute("scope","col");tr.appendChild(c);});th.appendChild(tr);t.appendChild(th);
entries.forEach(function(e){if(e[1]>max)max=e[1];});
entries.forEach(function(e){var r=el("tr");r.appendChild(el("td",e[0]));r.appendChild(el("td",e[1],"n"));if(chart){var c=el("td"),m=el("meter");m.min=0;m.max=max||1;m.value=e[1];m.setAttribute("aria-hidden","true");c.appendChild(m);r.appendChild(c);}tb.appendChild(r);});
if(!entries.length){var r=el("tr"),c=el("td","No events match.");c.colSpan=head.length;r.appendChild(c);tb.appendChild(r);}
t.appendChild(tb);wrap.appendChild(t);return wrap;}
function sorted(o){return Object.keys(o).sort().map(function(k){return [k,o[k]];});}
function card(n,label){var c=el("div",undefined,"card");c.appendChild(el("strong",n));c.appendChild(el("span",label));return c;}
function show(t,f){while(live.firstChild)live.removeChild(live.firstChild);
var cards=el("div",undefined,"cards"),v=t.by.verdict||{},a=t.by.action||{};
cards.appendChild(card(t.total,f.detector!==undefined?"Detector-positive events":"Events"));cards.appendChild(card(v.malicious||0,"Malicious verdicts"));cards.appendChild(card(a.block||0,"Blocked"));cards.appendChild(card(a.allow||0,"Allowed"));cards.appendChild(card((v.unknown||0)+(v.error||0),"Unknown or error verdicts"));
live.appendChild(cards);
D.dims.forEach(function(d){live.appendChild(el("h3",(labels[d]||d)));live.appendChild(table("Full-cohort events by "+(labels[d]||d).toLowerCase(),[labels[d]||d,"Events"].concat(d==="day"?["Chart"]:[]),sorted(t.by[d]),d==="day"));});
if(t.detectors){live.appendChild(el("h3","Detectors"));live.appendChild(table("Detector-positive events by detector (overlapping; not summed)",["Detector","Events"],sorted(t.detectors),false));}}
function apply(){var f={},parts=[];sels.forEach(function(s){if(s.value!==""){f[s.getAttribute("data-dim")]=s.value;parts.push((s.getAttribute("data-label")||"")+" "+s.value);}});
var shown=0;rows.forEach(function(r){var ok=rowMatches(function(d){return r.getAttribute("data-"+d);},f);r.hidden=!ok;if(ok)shown++;});
if(!parts.length){live.hidden=true;base.hidden=false;status.textContent="Showing the whole report scope.";return;}
var t=tally(D,f);show(t,f);live.hidden=false;base.hidden=true;
status.textContent="Filtered by "+parts.join("; ")+": "+t.total+" full-cohort event(s); "+shown+" of "+rows.length+" capped detail row(s) match.";}
sels.forEach(function(s){s.addEventListener("change",apply);});
document.getElementById("filters-reset").addEventListener("click",function(){sels.forEach(function(s){s.value="";});apply();});
form.addEventListener("submit",function(e){e.preventDefault();});
form.hidden=false;status.hidden=false;apply();
})(this);"##;

fn h(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() && c != '\n' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// JSON safe inside a non-executable `<script>` block.
fn script_json(value: &serde_json::Value) -> String {
    let mut out = String::new();
    for c in value.to_string().chars() {
        match c {
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c => out.push(c),
        }
    }
    out
}

fn base64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// The page's Content-Security-Policy: exactly this stylesheet and this script.
pub fn policy() -> String {
    format!(
        "default-src 'none'; style-src 'sha256-{}'; script-src 'sha256-{}'; base-uri 'none'; form-action 'none'; connect-src 'none'; img-src 'none'; object-src 'none'; require-trusted-types-for 'script'",
        base64(&Sha256::digest(CSS.as_bytes())),
        base64(&Sha256::digest(SCRIPT.as_bytes()))
    )
}

fn label(dim: &str) -> &'static str {
    match dim {
        "day" => "Report date",
        "application" => "Application",
        "verdict" => "Verdict",
        "action" => "Action",
        _ => "Detector",
    }
}

/// The projected (privacy-safe) summary: every key passes the shared text projection.
fn safe_summary(s: &Summary, secrets: &SecretSet) -> Summary {
    let mut s = s.clone();
    let project = |k: &mut Vec<String>| {
        for v in k.iter_mut() {
            *v = privacy::text(v, secrets, 256);
        }
    };
    s.cells.iter_mut().for_each(|c| project(&mut c.key));
    if let Some(d) = s.detector_cells.as_mut() {
        d.iter_mut().for_each(|c| project(&mut c.key));
    }
    s
}

/// Server-side totals: the same arithmetic as the script's `tally` with no filter.
pub struct Totals {
    pub total: u64,
    pub by: BTreeMap<&'static str, BTreeMap<String, u64>>,
    pub detectors: Option<BTreeMap<String, u64>>,
}

pub fn totals(s: &Summary) -> Totals {
    let mut by: BTreeMap<&'static str, BTreeMap<String, u64>> =
        s.dimensions.iter().map(|d| (*d, BTreeMap::new())).collect();
    let mut total = 0;
    for c in &s.cells {
        total += c.events;
        for (i, d) in s.dimensions.iter().enumerate() {
            *by.get_mut(d)
                .expect("dimension")
                .entry(c.key[i].clone())
                .or_default() += c.events;
        }
    }
    let detectors = s.detector_cells.as_ref().map(|cells| {
        let mut m: BTreeMap<String, u64> = BTreeMap::new();
        for c in cells {
            *m.entry(c.key[s.dimensions.len()].clone()).or_default() += c.events;
        }
        m
    });
    Totals {
        total,
        by,
        detectors,
    }
}

fn table(
    out: &mut String,
    caption: &str,
    head: &[&str],
    rows: &BTreeMap<String, u64>,
    chart: bool,
) {
    out.push_str("<div class=\"table-wrap\"><table><caption>");
    out.push_str(&h(caption));
    out.push_str("</caption><thead><tr>");
    for c in head {
        out.push_str(&format!("<th scope=\"col\">{}</th>", h(c)));
    }
    out.push_str("</tr></thead><tbody>");
    let max = rows.values().copied().max().unwrap_or(0).max(1);
    for (k, n) in rows {
        out.push_str(&format!("<tr><td>{}</td><td class=\"n\">{n}</td>", h(k)));
        if chart {
            out.push_str(&format!(
                "<td><meter min=\"0\" max=\"{max}\" value=\"{n}\" aria-hidden=\"true\"></meter></td>"
            ));
        }
        out.push_str("</tr>");
    }
    if rows.is_empty() {
        out.push_str(&format!(
            "<tr><td colspan=\"{}\">No events match.</td></tr>",
            head.len()
        ));
    }
    out.push_str("</tbody></table></div>");
}

fn base_summary(out: &mut String, s: &Summary) {
    let t = totals(s);
    let get = |d: &str, v: &str| t.by.get(d).and_then(|m| m.get(v)).copied().unwrap_or(0);
    out.push_str("<div id=\"base-summary\"><div class=\"cards\">");
    for (n, l) in [
        (t.total, "Events"),
        (get("verdict", "malicious"), "Malicious verdicts"),
        (get("action", "block"), "Blocked"),
        (get("action", "allow"), "Allowed"),
        (
            get("verdict", "unknown") + get("verdict", "error"),
            "Unknown or error verdicts",
        ),
    ] {
        out.push_str(&format!(
            "<div class=\"card\"><strong>{n}</strong><span>{l}</span></div>"
        ));
    }
    out.push_str("</div>");
    for d in &s.dimensions {
        out.push_str(&format!("<h3>{}</h3>", label(d)));
        let mut head = vec![label(d), "Events"];
        if *d == "day" {
            head.push("Chart");
        }
        table(
            out,
            &format!("Full-cohort events by {}", label(d).to_lowercase()),
            &head,
            &t.by[d],
            *d == "day",
        );
    }
    if let Some(det) = &t.detectors {
        out.push_str("<h3>Detectors</h3>");
        table(
            out,
            "Detector-positive events by detector (overlapping; not summed)",
            &["Detector", "Events"],
            det,
            false,
        );
    }
    out.push_str("</div>");
}

fn filters(out: &mut String, s: &Summary) {
    out.push_str("<form id=\"filters\" hidden aria-label=\"Report filters\">");
    let mut values: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for c in &s.cells {
        for (i, d) in s.dimensions.iter().enumerate() {
            values.entry(d).or_default().insert(&c.key[i]);
        }
    }
    let mut dims: Vec<&str> = s.dimensions.clone();
    if let Some(det) = &s.detector_cells {
        dims.push("detector");
        for c in det {
            values
                .entry("detector")
                .or_default()
                .insert(&c.key[s.dimensions.len()]);
        }
    }
    for d in dims {
        out.push_str(&format!(
            "<label>{l}<select data-dim=\"{d}\" data-label=\"{l}\"><option value=\"\">All</option>",
            l = label(d)
        ));
        for v in values.get(d).into_iter().flatten() {
            out.push_str(&format!("<option>{}</option>", h(v)));
        }
        out.push_str("</select></label>");
    }
    out.push_str("<button type=\"button\" id=\"filters-reset\">Reset filters</button></form><p id=\"filter-status\" role=\"status\" aria-live=\"polite\" hidden></p><noscript><p class=\"muted\">Filters need JavaScript. This view shows the whole report scope.</p></noscript>");
}

/// Render the page within `limits`, falling back to a page without the detail table
/// when the full page exceeds the byte ceiling.
pub fn render(page: &Page<'_>, input: &ReportInput, limits: Limits) -> Result<String, Refusal> {
    let full = render_with(page, input, limits.detail_rows, false)?;
    if full.len() <= limits.html_bytes {
        return Ok(full);
    }
    let reduced = render_with(page, input, 0, true)?;
    if reduced.len() <= limits.html_bytes {
        return Ok(reduced);
    }
    Err(Refusal::TooLarge)
}

fn render_with(
    page: &Page<'_>,
    input: &ReportInput,
    detail_rows: usize,
    detail_dropped: bool,
) -> Result<String, Refusal> {
    let secrets = SecretSet::default();
    let mut e = input.evidence.clone();
    privacy::project_with_spans(&mut e, &secrets, &privacy::PayloadSpans::default())
        .map_err(|_| Refusal::Evidence)?;
    let s = safe_summary(&input.summary, &secrets);
    let t = |v: &str| h(&privacy::text(v, &secrets, 256));
    let iso = airs_runtime_core::time::iso;
    let date = |ms: i64| airs_runtime_core::schedule::report_date(ms).unwrap_or_default();
    let title = t(if page.title.is_empty() {
        "AIRS runtime history report"
    } else {
        page.title
    });
    let complete = input.complete;
    let mut out = String::with_capacity(64 * 1024);
    out.push_str(&format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"{}\"><meta name=\"referrer\" content=\"no-referrer\"><title>{title} — AIRS runtime history</title><style>{CSS}</style></head><body><a class=\"skip\" href=\"#main\">Skip to report</a><header><div class=\"eyebrow\">Prisma AIRS · Runtime history</div><h1>{title}</h1><p>Tenant <strong>{}</strong> · Scope: {}</p><p>Period: {} → {} UTC (report dates {} to {}, {}) · Evidence as of {} UTC</p><p><span class=\"badge{}\">{}</span> <span class=\"badge\">{} report · version {}</span></p></header><main id=\"main\"><nav aria-label=\"Report sections\"><a href=\"#summary\">Summary</a><a href=\"#detail\">Detail</a><a href=\"#findings\">Findings</a><a href=\"#profiles\">Profiles</a><a href=\"#timeline\">Profile timeline</a><a href=\"#coverage\">Coverage</a></nav>",
        h(&policy()),
        t(&e.tenant),
        if e.app_ids.is_empty() {
            "the whole tenant".to_owned()
        } else {
            t(&e.app_ids.join(", "))
        },
        iso(page.period.0),
        iso(page.period.1),
        date(page.period.0),
        date(page.period.1 - 1),
        airs_runtime_core::schedule::REPORT_TZ,
        iso(page.cutoff_ms),
        if complete { "" } else { " partial" },
        if complete {
            "Coverage complete"
        } else {
            "Coverage partial: see gaps"
        },
        t(page.kind),
        page.version,
    ));
    // 01 Summary (full cohort).
    out.push_str("<section id=\"summary\"><div class=\"eyebrow\">01 / Full-cohort summary</div><h2>What the archive observed</h2><p>Counts cover every archived event observed in the period and first seen by the evidence cutoff, independent of the detail bound. Verdict (detection) and action (enforcement) are separate: an allowed malicious verdict can be intentional monitoring. Unknown is never counted as benign.</p>");
    filters(&mut out, &s);
    if !s.disabled_filters.is_empty() {
        out.push_str(&format!(
            "<p class=\"gap\">The {} filter(s) are disabled: their summary would exceed {} cells. The tables below keep the whole scope; create a report over a narrower range or application scope to filter by them.</p>",
            h(&s.disabled_filters.join(", ")),
            s.cell_budget
        ));
    }
    out.push_str("<div id=\"live-summary\" hidden></div>");
    base_summary(&mut out, &s);
    out.push_str(&format!(
        "<script type=\"application/json\" id=\"report-data\">{}</script></section>",
        script_json(&serde_json::json!({
            "dims": s.dimensions,
            "cells": s.cells.iter().map(|c| {
                let mut row: Vec<serde_json::Value> = c.key.iter().map(|k| k.clone().into()).collect();
                row.push(c.events.into());
                row
            }).collect::<Vec<_>>(),
            "det": s.detector_cells.as_ref().map(|d| d.iter().map(|c| {
                let mut row: Vec<serde_json::Value> = c.key.iter().map(|k| k.clone().into()).collect();
                row.push(c.events.into());
                row
            }).collect::<Vec<_>>()),
        }))
    ));
    // 02 Detail (capped).
    let shown = e.events.len().min(detail_rows);
    out.push_str(&format!(
        "<section id=\"detail\"><div class=\"eyebrow\">02 / Capped detail</div><h2>Event detail</h2><p>The first {shown} of {} events by observed time (detail bound {MAX_DETAIL_ROWS}). Filters hide detail rows that do not match; detail is never used for the totals above.</p>",
        s.events
    ));
    if detail_dropped {
        out.push_str("<p class=\"gap\">The detail table is omitted: with it this page would exceed its 32 MiB ceiling. The full-cohort summary, posture and coverage are complete; the bundled evidence.json holds the bounded detail.</p>");
    } else {
        out.push_str("<div class=\"table-wrap\"><table><caption>Archived events (capped detail)</caption><thead><tr><th scope=\"col\">Observed (UTC)</th><th scope=\"col\">Application</th><th scope=\"col\">Scan / sub-request</th><th scope=\"col\">Verdict</th><th scope=\"col\">Action</th><th scope=\"col\">Profile</th><th scope=\"col\">Detectors</th></tr></thead><tbody id=\"detail-rows\">");
        for ev in e.events.iter().take(shown) {
            let mut dets: BTreeSet<String> = BTreeSet::new();
            for hint in &ev.detector_hints {
                if let Some((dir, d)) = hint.split_once('/')
                    && ["prompt", "response", "tool", "unknown"].contains(&dir)
                {
                    dets.insert(detectors::canonical(d).to_owned());
                }
            }
            for d in &ev.detections {
                if d.verdict == "malicious" {
                    dets.insert(detectors::canonical(&d.detector).to_owned());
                }
            }
            let dets: Vec<String> = dets
                .iter()
                .map(|d| privacy::text(d, &secrets, 256))
                .collect();
            let at = ev.timestamp_ms;
            out.push_str(&format!(
                "<tr data-day=\"{}\" data-application=\"{}\" data-verdict=\"{}\" data-action=\"{}\" data-detector=\"{}\"><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                at.map(date).unwrap_or_default(),
                t(&ev.identity.app_id),
                t(&ev.verdict),
                t(&ev.action),
                h(&dets.join("\n")),
                at.map(iso).unwrap_or_else(|| "Unknown".into()),
                t(&ev.identity.app_id),
                t(&ev.identity.scan_id),
                ev.identity.sub_request.map_or_else(|| "Unknown".into(), |n| n.to_string()),
                t(&ev.verdict),
                t(&ev.action),
                t(ev.profile.as_deref().unwrap_or("Unknown")),
                h(&dets.join(", ")),
            ));
        }
        out.push_str("</tbody></table></div>");
    }
    out.push_str("</section>");
    // 03 Findings (from the capped detail).
    let a = analyze(&e);
    out.push_str("<section id=\"findings\"><div class=\"eyebrow\">03 / Findings</div><h2>Grouped detector findings</h2><p class=\"muted\">Grouped from the capped detail (first 250 groups); unreviewed observations, not confirmed attacks. review.md carries the review packet.</p>");
    if a.findings.is_empty() {
        out.push_str("<p>No grouped malicious detector findings in the detail. This does not establish an absence of violations.</p>");
    } else {
        out.push_str("<div class=\"table-wrap\"><table><caption>Findings by detector, application, profile, direction and enforcement</caption><thead><tr><th scope=\"col\">Detector</th><th scope=\"col\">Application</th><th scope=\"col\">Profile</th><th scope=\"col\">Direction</th><th scope=\"col\">Enforcement</th><th scope=\"col\">Events</th><th scope=\"col\">Scans</th></tr></thead><tbody>");
        for f in a.findings.iter().take(250) {
            out.push_str(&format!(
                "<tr><td>{}</td><td>{} ({})</td><td>{}</td><td>{}</td><td>{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
                t(&f.group.detector),
                t(&f.group.app_name),
                t(&f.group.app_id),
                t(f.group.profile.as_deref().unwrap_or("Unknown")),
                t(&f.group.direction),
                t(&f.group.action),
                f.events,
                f.scans
            ));
        }
        out.push_str("</tbody></table></div>");
    }
    out.push_str("</section>");
    // 04 Profiles (as of the period end).
    out.push_str("<section id=\"profiles\"><div class=\"eyebrow\">04 / Profile posture</div><h2>Profiles and detection services</h2><p>Each profile's latest observation at or before the period end; later configuration is never used for this period. An observation is not an exact effective change time.</p>");
    let mut matrix = airs_runtime_core::signal::html::profile_matrix(&e);
    if matrix.contains(" style=") {
        matrix.clear();
    }
    out.push_str(&matrix);
    out.push_str("</section>");
    // 05 Profile timeline.
    out.push_str("<section id=\"timeline\"><div class=\"eyebrow\">05 / Profile timeline</div><h2>Observed profile changes</h2><p>The configuration observed at the period start, then each observed difference. Times are when an observation was made, not when a change took effect.</p>");
    if input.timeline.is_empty() {
        out.push_str("<p>No profile observations cover this period.</p>");
    } else {
        out.push_str("<div class=\"table-wrap\"><table><caption>Profile observations in the period</caption><thead><tr><th scope=\"col\">Profile</th><th scope=\"col\">Revision</th><th scope=\"col\">First observed (UTC)</th><th scope=\"col\">Last observed (UTC)</th><th scope=\"col\">Observation</th></tr></thead><tbody>");
        for x in &input.timeline {
            out.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}{}</td><td>{}</td><td>{}</td></tr>",
                t(&x.profile),
                x.revision
                    .map_or_else(|| "Unknown".into(), |n| n.to_string()),
                iso(x.first_observed_ms),
                if x.at_start {
                    " (current at period start)"
                } else {
                    ""
                },
                iso(x.last_observed_ms),
                h(&x.observation)
            ));
        }
        out.push_str("</tbody></table></div>");
        if input.timeline_truncated {
            out.push_str("<p class=\"gap\">The timeline is capped at 1,000 spans.</p>");
        }
    }
    out.push_str("</section>");
    // 06 Coverage.
    out.push_str("<section id=\"coverage\"><div class=\"eyebrow\">06 / Coverage and method</div><h2>Coverage, gaps and provenance</h2><div class=\"table-wrap\"><table><caption>Archive coverage by source category</caption><thead><tr><th scope=\"col\">Source</th><th scope=\"col\">Status</th><th scope=\"col\">Unit</th><th scope=\"col\">Notes</th></tr></thead><tbody>");
    for src in &e.sources {
        out.push_str(&format!(
            "<tr><td>{} ({})</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            t(&src.name),
            if src.required {
                "required"
            } else {
                "supplemental"
            },
            match src.status {
                airs_runtime_core::signal::evidence::Status::Complete => "complete",
                airs_runtime_core::signal::evidence::Status::Partial => "partial",
                airs_runtime_core::signal::evidence::Status::Unavailable => "unavailable",
            },
            t(&src.unit),
            t(&src.notes.join("; "))
        ));
    }
    out.push_str("</tbody></table></div>");
    if input.gaps.is_empty() {
        out.push_str("<p>No gaps were recorded for this period.</p>");
    } else {
        out.push_str("<ul>");
        for g in input.gaps.iter().take(100) {
            out.push_str(&format!("<li class=\"gap\">{}</li>", t(g)));
        }
        out.push_str("</ul>");
    }
    out.push_str(&format!(
        "<p>Rendered from the archive alone; rendering makes no AIRS request. Summary budget {} cells; detail bound {MAX_DETAIL_ROWS} rows; page ceiling 32 MiB. Missing evidence is a gap, never a clean result. Secrets and matched values are withheld.</p></section></main><footer>Evidence schema {} · Analysis rules {} · Redaction {} · Self-contained HTML; scripts optional; no network access.</footer><script>{SCRIPT}</script></body></html>\n",
        s.cell_budget, e.schema_version, e.rules_version, e.redaction_version
    ));
    Ok(out)
}

#[cfg(test)]
#[path = "report_html_tests.rs"]
mod tests;
