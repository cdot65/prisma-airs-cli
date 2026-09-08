import { createHash } from 'node:crypto';
import type { EnvironmentReport, EnvironmentReportTable } from './types.js';

const HEALTH = {
  attention: 'Attention required',
  review: 'Review required',
  'no-findings': 'No findings in available evidence',
  unknown: 'Unable to assess',
};
const html = (v: unknown) =>
  String(v ?? 'Unknown').replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
const md = (v: unknown) =>
  String(v ?? 'Unknown').replace(/[&<>"'\\`*_[\]{}()|!#~\r\n]/g, (c) =>
    /[\r\n]/.test(c) ? ' ' : `&#${c.charCodeAt(0)};`,
  );
function tables(r: EnvironmentReport): EnvironmentReportTable[] {
  return [
    ...r.tables,
    {
      title: 'Evidence and completeness',
      note: 'Complete means the source was collected within its budget, not that the environment is secure. Unavailable is not zero.',
      headers: ['Source', 'SDK method', 'Window', 'Status', 'Records', 'Pages', 'Notes'],
      rows: r.sources.map((s) => [
        s.name,
        s.method,
        s.window,
        s.status,
        s.records,
        s.pages,
        s.notes.join(' ') || 'Collection completed.',
      ]),
    },
  ];
}

/** Portable Markdown using the same allowlisted evidence as the HTML dashboard. */
export function renderEnvironmentReportMarkdown(r: EnvironmentReport): string {
  const lines = [
    `# ${md(r.title)}`,
    '',
    `**${md(r.product)}**`,
    '',
    `Assessment: **${HEALTH[r.health]}**`,
    '',
    `Collection: ${r.collectionStartedAt} → ${r.generatedAt}`,
    '',
    `${md(r.windowLabel ?? 'Scan creation window (UTC)')}: ${r.window.start} → ${r.window.end}`,
    '',
    '## What needs attention',
    '',
  ];
  for (const f of r.findings)
    lines.push(
      `### ${f.priority.toUpperCase()}: ${md(f.title)}`,
      '',
      `Evidence: ${md(f.evidence)}`,
      '',
      `Next step: ${md(f.recommendation)}`,
      '',
      `Source: ${md(f.source)}`,
      '',
    );
  if (!r.findings.length)
    lines.push('No findings in available evidence; this does not establish absence of risk.', '');
  for (const t of tables(r)) {
    lines.push(`## ${md(t.title)}`, '', md(t.note), '');
    if (t.rows.length)
      lines.push(
        `| ${t.headers.map(md).join(' | ')} |`,
        `| ${t.headers.map(() => '---').join(' | ')} |`,
        ...t.rows.map((row) => `| ${row.map(md).join(' | ')} |`),
        '',
      );
    else
      lines.push(
        'No rows collected; consult source completeness before interpreting this as empty.',
        '',
      );
  }
  lines.push(
    '## Scope and limitations',
    '',
    ...r.limitations.map((v) => `- ${md(v)}`),
    '',
    `Prisma AIRS CLI · ${md(r.schemaLabel ?? 'Red Team')} report schema ${r.schemaVersion}`,
    '',
  );
  return lines.join('\n');
}
const CSS = `
:root{font:16px/1.6 system-ui,sans-serif;color:#193449;background:#f0f4f7}*{box-sizing:border-box}body{margin:0}header{background:#152e43;color:white;padding:3rem max(1rem,calc((100vw - 1160px)/2))}h1{font-size:clamp(1.8rem,4vw,3rem);line-height:1.15}h1,h2,h3,p,td{overflow-wrap:anywhere}main{max-width:1200px;margin:auto;padding:1rem}section,article{padding:1.4rem;background:white;border:1px solid #cddbe4;border-radius:10px;margin:1rem 0}.finding{border-left:5px solid #b08528}.finding[data-priority=attention]{border-left-color:#b1392d}.note{color:#536575;font-size:.9rem}.badge{border:1px solid;padding:.3rem .8rem;border-radius:30px;display:inline-block}nav,.controls{display:flex;gap:1rem;flex-wrap:wrap}button,select{font:inherit;padding:.4rem;border:1px solid #526778;border-radius:5px;background:white;color:#193449}a{color:#006b63}.table-wrap{overflow:auto}table{border-collapse:collapse;width:100%;text-align:left}th,td{padding:.7rem;border-bottom:1px solid #d4dfe6;vertical-align:top}th{background:#edf3f7}caption{text-align:left}footer{padding:1rem;color:#536575}.js-only{display:none}.ready .js-only{display:flex}[hidden]{display:none!important}:focus-visible{outline:3px solid #ae6816;outline-offset:3px}.skip{position:absolute;left:-10000px}.skip:focus{left:1rem;top:1rem;background:white;padding:1rem}@media(max-width:600px){header{padding:1.5rem 1rem}section{padding:1rem}th,td{padding:.5rem}}@media print{body{background:white;font-size:10pt}header{background:white;color:black;padding:0}main{max-width:none;padding:0}nav,.controls,.skip{display:none!important}[hidden]{display:revert!important}.table-wrap{overflow:visible}section{border:0;padding:0}article{break-inside:avoid}}
`;
const JS = `'use strict';document.documentElement.classList.add('ready');document.getElementById('print').addEventListener('click',function(){window.print()});document.getElementById('priority').addEventListener('change',function(){var n=0;document.querySelectorAll('.finding').forEach(function(e){e.hidden=this.value!=='all'&&e.dataset.priority!==this.value;if(!e.hidden)n++},this);document.getElementById('count').textContent=n+' findings shown'});`;

/** Self-contained HTML with hash-based CSP, static inline JS/CSS and no remote resources. */
export function renderEnvironmentReportHtml(r: EnvironmentReport): string {
  const hash = (v: string) => createHash('sha256').update(v).digest('base64');
  const policy = `default-src 'none'; script-src 'sha256-${hash(JS)}'; style-src 'sha256-${hash(CSS)}'; connect-src 'none'; base-uri 'none'; form-action 'none'`;
  const sections = tables(r)
    .map(
      (t, i) =>
        `<section id="table-${i}"><h2>${html(t.title)}</h2><p class="note">${html(t.note)}</p>${t.rows.length ? `<div class="table-wrap" role="region" aria-label="${html(t.title)}" tabindex="0"><table><caption>${t.rows.length} aggregate rows</caption><thead><tr>${t.headers.map((v) => `<th scope="col">${html(v)}</th>`).join('')}</tr></thead><tbody>${t.rows.map((row) => `<tr>${row.map((v) => `<td>${html(v)}</td>`).join('')}</tr>`).join('')}</tbody></table></div>` : '<p>No rows collected; consult source completeness before interpreting this as empty.</p>'}</section>`,
    )
    .join('');
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="${html(policy)}"><meta name="referrer" content="no-referrer"><title>${html(r.title)}</title><style>${CSS}</style></head><body><a class="skip" href="#main">Skip to report</a><header><p>${html(r.product)}</p><h1>${html(r.title)}</h1><span class="badge">${HEALTH[r.health]}</span><p>Collection: ${html(r.collectionStartedAt)} → ${html(r.generatedAt)}</p><p>${html(r.windowLabel ?? 'Scan creation window (UTC)')}: ${html(r.window.start)} → ${html(r.window.end)}</p><p>Read-only evidence · Confidential environment metadata</p></header><main id="main"><nav aria-label="Report sections"><a href="#findings">Attention</a><a href="#table-0">Evidence tables</a><a href="#scope">Scope</a></nav><section id="findings"><h2>What needs attention</h2><div class="controls js-only"><label for="priority">Priority</label><select id="priority"><option value="all">All</option><option value="attention">Attention</option><option value="review">Review</option></select><span id="count" role="status"></span><button id="print" type="button">Print / save PDF</button></div>${r.findings.map((f) => `<article class="finding" data-priority="${html(f.priority)}"><h3>${html(f.title)}</h3><p>${html(f.evidence)}</p><p><strong>Next step:</strong> ${html(f.recommendation)}</p><p class="note">${html(f.source)}</p></article>`).join('') || '<p>No findings in available evidence; this does not establish absence of risk.</p>'}</section>${sections}<section id="scope"><h2>Scope and limitations</h2><ul>${r.limitations.map((v) => `<li>${html(v)}</li>`).join('')}</ul></section><footer>Prisma AIRS CLI · ${html(r.schemaLabel ?? 'Red Team')} report schema ${r.schemaVersion} · Works offline and without JavaScript.</footer></main><script>${JS}</script></body></html>\n`;
}
