import { createHash } from 'node:crypto';
import type { RuntimeDailyReport } from './types.js';

const HEALTH = {
  attention: 'Attention required',
  review: 'Review required',
  'no-findings': 'No findings in available evidence',
  unknown: 'Unable to assess',
};

function html(value: unknown): string {
  return String(value ?? 'Unknown').replace(
    /[&<>"']/g,
    (char) =>
      ({
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        '"': '&quot;',
        "'": '&#39;',
      })[char] ?? char,
  );
}

function md(value: unknown): string {
  // Entities also prevent Markdown links/images and raw HTML from becoming active content.
  return String(value ?? 'Unknown').replace(/[&<>"'\\`*_[\]{}()|!#~\r\n]/g, (char) =>
    char === '\r' || char === '\n' ? ' ' : `&#${char.charCodeAt(0)};`,
  );
}

function number(value: number | null): string {
  return value === null ? 'Unknown' : value.toLocaleString('en-US', { maximumFractionDigits: 2 });
}

type Table = { title: string; headers: string[]; rows: unknown[][]; note: string; id: string };

function tables(report: RuntimeDailyReport): Table[] {
  const logSource = report.sources.find((source) => source.name === 'Scan log detail');
  return [
    {
      title: 'Daily application activity',
      id: 'applications',
      headers: [
        'Application bucket',
        'Sessions',
        'Violating sessions',
        'Violation rate',
        'Registered ID match',
      ],
      rows: report.activity.applications.map((app) => [
        app.name,
        number(app.sessions),
        number(app.violatingSessions),
        app.sessions !== null &&
        app.sessions > 0 &&
        app.violatingSessions !== null &&
        app.violatingSessions <= app.sessions
          ? `${number((app.violatingSessions / app.sessions) * 100)}%`
          : 'Unknown',
        app.registered === null ? 'Unknown' : app.registered ? 'Yes' : 'Not in returned inventory',
      ]),
      note: 'API-reported rolling one-day application buckets, ranked by violating sessions. Partial-source totals cover collected buckets only. Violating sessions are not necessarily blocked sessions.',
    },
    {
      title: 'Current security profiles',
      id: 'profiles',
      headers: [
        'Profile',
        'Revision',
        'Active',
        'Timeout action',
        'Storage masking',
        'Last modified (UTC)',
      ],
      rows: report.profiles.map((profile) => [
        profile.name,
        number(profile.revision),
        profile.active === null ? 'Unknown' : profile.active ? 'Yes' : 'No',
        profile.timeoutActions.join(', ') || 'Unknown',
        profile.storageMasking,
        profile.modified,
      ]),
      note: 'Latest returned revision per profile name. These are current settings, not a record of changes during the daily window.',
    },
    {
      title: 'Registered application inventory',
      id: 'inventory',
      headers: ['Application', 'Environment', 'Cloud', 'Model', 'Key associations'],
      rows: report.registeredApps.map((app) => [
        app.name,
        app.environment,
        app.cloud,
        app.model,
        number(app.keyAssociations),
      ]),
      note: 'Current registered applications, separate from scan-metadata application buckets. Association counts do not establish key validity or deployment health. No key values or auth codes are included.',
    },
    {
      title: 'Collected scan-log observations',
      id: 'logs',
      headers: ['Dimension', 'API value', 'Entries'],
      rows: [
        ...report.logs.actions.map((entry) => ['Action', entry.name, number(entry.count)]),
        ...report.logs.verdicts.map((entry) => ['Verdict', entry.name, number(entry.count)]),
      ],
      note: `Source: ${logSource?.status ?? 'unavailable'}. Timestamp-eligible entries: ${logSource?.status === 'unavailable' ? 'Unknown' : number(report.logs.entries)}. Tokens across eligible collected entries: ${number(report.logs.tokens)}. Missing timestamps: ${report.logs.missingTimestamps}; outside window: ${report.logs.outsideWindow}. These are sample/collection counts, not tenant totals.`,
    },
    {
      title: 'Evidence and collection coverage',
      id: 'evidence',
      headers: ['Source / SDK method', 'Window', 'Status', 'Records', 'Pages', 'Collection notes'],
      rows: report.sources.map((source) => [
        `${source.name} — ${source.method}`,
        source.window,
        source.status,
        number(source.records),
        source.pages,
        source.notes.join(' ') || 'Collection completed.',
      ]),
      note: 'Complete means pagination finished for that source, not that the environment is secure or that all traffic was ingested. Unavailable does not mean zero.',
    },
  ];
}

function metrics(report: RuntimeDailyReport): Array<[string, string]> {
  return [
    ['Sessions in collected app buckets', number(report.activity.sessions)],
    ['Violating sessions', number(report.activity.violatingSessions)],
    [
      'Violation rate',
      report.activity.violationRate === null
        ? 'Unknown'
        : `${number(report.activity.violationRate)}%`,
    ],
    [
      'Complete evidence sources',
      `${report.sources.filter((source) => source.status === 'complete').length} / ${report.sources.length}`,
    ],
  ];
}

/** Render the same evidence model as a portable, inert Markdown deliverable. */
export function renderRuntimeReportMarkdown(report: RuntimeDailyReport): string {
  const lines = [
    `# ${md(report.title)}`,
    '',
    `**${md(report.product)}**`,
    '',
    `Assessment: **${HEALTH[report.health]}**`,
    '',
    `Window: ${md(report.window.start)} → ${md(report.window.end)}`,
    '',
    `${md(report.window.description)}. Collection: ${md(report.collectionStartedAt)} → ${md(report.generatedAt)}.`,
    '',
    '## At a glance',
    '',
    ...metrics(report).map(([key, value]) => `- ${key}: **${value}**`),
    '',
    '## What needs attention',
    '',
  ];
  if (report.findings.length === 0)
    lines.push(
      'No findings in the available evidence. This does not establish an absence of risk.',
      '',
    );
  for (const finding of report.findings)
    lines.push(
      `### ${finding.priority.toUpperCase()}: ${md(finding.title)}`,
      '',
      `Evidence: ${md(finding.evidence)}`,
      '',
      `Next step: ${md(finding.recommendation)}`,
      '',
      `Source: ${md(finding.source)}`,
      '',
    );
  for (const table of tables(report)) {
    lines.push(`## ${table.title}`, '', md(table.note), '');
    if (table.rows.length === 0)
      lines.push(
        'No rows available. Consult the source status before interpreting this as an empty result.',
        '',
      );
    else
      lines.push(
        `| ${table.headers.join(' | ')} |`,
        `| ${table.headers.map(() => '---').join(' | ')} |`,
        ...table.rows.map((row) => `| ${row.map(md).join(' | ')} |`),
        '',
      );
  }
  lines.push(
    '## Scope, privacy, and limitations',
    '',
    ...report.limitations.map((note) => `- ${md(note)}`),
    '',
    'Generated by Prisma AIRS CLI · Report schema 1',
    '',
  );
  return lines.join('\n');
}

const CSS = `
#evidence td:nth-child(3){white-space:nowrap}
:root{color-scheme:light;--ink:#172b40;--muted:#526476;--line:#d8e2e9;--bg:#f2f5f8;--accent:#00786e;font:16px/1.55 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink)}a{color:var(--accent)}header{background:#11283d;color:#fff;padding:2.5rem max(1.25rem,calc((100vw - 1240px)/2))}header p{color:#d7e4ec;max-width:85ch}h1{font-size:clamp(1.8rem,4vw,2.7rem);line-height:1.15;margin:.6rem 0 1rem;overflow-wrap:anywhere}h2{font-size:1.35rem;margin:0 0 .6rem}h3{font-size:1.05rem;margin:.35rem 0}p{margin:.5rem 0 1rem}.eyebrow{font-size:.8rem;letter-spacing:.12em;text-transform:uppercase}.badge{display:inline-block;padding:.35rem .7rem;border:1px solid currentColor;border-radius:100px;font-weight:650;background:#fff;color:var(--ink)}.attention .badge{color:#a62b1b}.review .badge{color:#795500}.unknown .badge{color:#69439b}.no-findings .badge{color:#006a54}
main{max-width:1280px;margin:auto;padding:1.5rem 1.25rem 3rem}nav{display:flex;gap:1.25rem;flex-wrap:wrap;margin-bottom:1.5rem}section{background:#fff;border:1px solid var(--line);border-radius:12px;padding:1.4rem;margin:1.3rem 0}.cards{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:1rem}.card{background:#fff;border:1px solid var(--line);border-top:4px solid var(--accent);border-radius:9px;padding:1.2rem}.card strong{display:block;font-size:2rem;font-variant-numeric:tabular-nums}.card span,.note{color:var(--muted);font-size:.9rem}.toolbar{display:flex;align-items:center;gap:1rem;flex-wrap:wrap;margin:1rem 0}button,input,select{font:inherit;border:1px solid #8b9eaf;border-radius:6px;padding:.5rem .7rem;background:#fff;color:var(--ink)}button{cursor:pointer}input{max-width:100%}:focus-visible{outline:3px solid #c06605;outline-offset:3px}.finding{border-left:4px solid #b3c3cf;padding:.5rem 1rem;margin:1rem 0;background:#f7f9fb}.finding[data-priority=attention]{border-color:#c74429}.finding[data-priority=review]{border-color:#b78712}.priority{font-size:.75rem;text-transform:uppercase;font-weight:750;letter-spacing:.07em}.finding p{margin:.4rem 0}.table-wrap{overflow:auto}table{border-collapse:collapse;width:100%;font-size:.9rem;text-align:left}th{background:#edf3f7;font-weight:700}th,td{padding:.7rem .8rem;border-bottom:1px solid var(--line);vertical-align:top;overflow-wrap:anywhere;min-width:90px}tbody tr:hover{background:#f8fbfd}caption{text-align:left;font-weight:600;padding:.5rem 0}footer{color:var(--muted);font-size:.8rem;margin-top:2rem}.hidden,[hidden]{display:none!important}.js-only{display:none}.js-ready .js-only{display:flex}.skip{position:absolute;left:-9999px}.skip:focus{left:1rem;top:1rem;background:#fff;padding:1rem;z-index:1}li{margin:.5rem 0}
@media(max-width:760px){.cards{grid-template-columns:repeat(2,minmax(0,1fr))}section{padding:1rem}header{padding:1.5rem 1.25rem}.card strong{font-size:1.5rem}}
@media print{body{background:#fff;font-size:10pt}header{background:#fff;color:#000;padding:0}header p{color:#333}main{padding:0;max-width:none}nav,.toolbar,.skip{display:none!important}section,.card{border-radius:0;break-inside:auto}.finding,.card{break-inside:avoid}.table-wrap{overflow:visible}table{font-size:8pt}th,td{min-width:0;padding:.3rem}.hidden,[hidden]{display:revert!important}h2,h3{break-after:avoid}a{color:#000}}
`;

// Static code only: API strings are never interpolated into JavaScript or an HTML attribute.
const JS = `
'use strict';
document.documentElement.classList.add('js-ready');
document.getElementById('print-report').addEventListener('click',function(){window.print();});
var filter=document.getElementById('finding-filter');
filter.addEventListener('change',function(){
  var visible=0;
  document.querySelectorAll('.finding').forEach(function(row){row.hidden=filter.value!=='all'&&row.dataset.priority!==filter.value;if(!row.hidden)visible++;});
  document.getElementById('finding-count').textContent=visible+' findings shown';
});
var search=document.getElementById('app-search');
search.addEventListener('input',function(){
  var query=search.value.toLowerCase().trim(),visible=0;
  document.querySelectorAll('#applications tbody tr').forEach(function(row){row.hidden=!row.textContent.toLowerCase().includes(query);if(!row.hidden)visible++;});
  document.getElementById('app-count').textContent=visible+' application buckets shown';
});
`;

/** Standalone HTML: inline CSS/JS, no dependencies, external requests, or embedded raw data. */
export function renderRuntimeReportHtml(report: RuntimeDailyReport): string {
  const hash = (text: string) => createHash('sha256').update(text).digest('base64');
  const policy = `default-src 'none'; script-src 'sha256-${hash(JS)}'; style-src 'sha256-${hash(CSS)}'; base-uri 'none'; form-action 'none'; connect-src 'none'; img-src 'none'`;
  const findingCards = report.findings
    .map(
      (finding) =>
        `<article class="finding" data-priority="${html(finding.priority)}"><span class="priority">${html(finding.priority)}</span><h3>${html(finding.title)}</h3><p><strong>Evidence:</strong> ${html(finding.evidence)}</p><p><strong>Next step:</strong> ${html(finding.recommendation)}</p><p class="note">Source: ${html(finding.source)}</p></article>`,
    )
    .join('');
  const sections = tables(report)
    .map(
      (table) =>
        `<section id="${table.id}" aria-labelledby="${table.id}-title"><h2 id="${table.id}-title">${table.title}</h2><p class="note">${html(table.note)}</p>${table.id === 'applications' ? '<div class="toolbar js-only"><label for="app-search">Find an application</label><input id="app-search" type="search" placeholder="Filter application buckets"><span id="app-count" role="status"></span></div>' : ''}${table.rows.length ? `<div class="table-wrap" role="region" aria-label="${table.title}" tabindex="0"><table><caption>${table.rows.length} collected rows</caption><thead><tr>${table.headers.map((header) => `<th scope="col">${header}</th>`).join('')}</tr></thead><tbody>${table.rows.map((row) => `<tr>${row.map((cell) => `<td>${html(cell)}</td>`).join('')}</tr>`).join('')}</tbody></table></div>` : '<p>No rows available. Consult the source status before interpreting this as an empty result.</p>'}</section>`,
    )
    .join('');
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="${html(policy)}"><meta name="referrer" content="no-referrer"><title>${html(report.title)} — Prisma AIRS</title><style>${CSS}</style></head>
<body class="${html(report.health)}"><a class="skip" href="#main">Skip to report</a><header><div class="eyebrow">Prisma AIRS · AI Runtime Security</div><h1>${html(report.title)}</h1><span class="badge">${HEALTH[report.health]}</span><p>${html(report.window.description)}<br><time>${html(report.window.start)}</time> → <time>${html(report.window.end)}</time></p><p>Collected ${html(report.collectionStartedAt)} → ${html(report.generatedAt)}<br>Read-only evidence · Confidential environment metadata</p></header>
<main id="main"><nav aria-label="Report sections"><a href="#findings">Attention</a><a href="#applications">Activity</a><a href="#profiles">Profiles</a><a href="#inventory">Inventory</a><a href="#evidence">Evidence</a><a href="#scope">Scope</a></nav>
<div class="cards">${metrics(report)
    .map(([name, value]) => `<div class="card"><span>${name}</span><strong>${value}</strong></div>`)
    .join('')}</div>
<section id="findings"><h2>What needs attention</h2><p class="note">Observed violations and configuration review items, not a numerical health score. Collection gaps remain visible in the evidence table.</p><div class="toolbar js-only"><label for="finding-filter">Priority</label><select id="finding-filter"><option value="all">All findings</option><option value="attention">Attention</option><option value="review">Review</option><option value="info">Information</option></select><span id="finding-count" role="status"></span><button id="print-report" type="button">Print / save PDF</button></div>${findingCards || '<p>No findings in the available evidence. This does not establish an absence of risk.</p>'}</section>
${sections}<section id="scope"><h2>Scope, privacy, and limitations</h2><ul>${report.limitations.map((note) => `<li>${html(note)}</li>`).join('')}</ul></section><footer>Generated by Prisma AIRS CLI · Report schema 1 · Self-contained HTML; no network access required. All evidence remains readable with JavaScript disabled.</footer></main><script>${JS}</script></body></html>\n`;
}
