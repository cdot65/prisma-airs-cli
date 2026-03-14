/**
 * Audit report builders — JSON and HTML export for profile audit results.
 */

import type { AuditResult, ConflictPair, TopicAuditResult } from './types.js';

/** Structured JSON output for audit results. */
export interface AuditReportJson {
  version: 1;
  generatedAt: string;
  profileName: string;
  compositeMetrics: {
    coverage: number;
    accuracy: number;
    tpr: number;
    tnr: number;
    f1: number;
  };
  topics: Array<{
    name: string;
    action: string;
    description: string;
    metrics: {
      coverage: number;
      accuracy: number;
      tpr: number;
      tnr: number;
      f1: number;
      tp: number;
      tn: number;
      fp: number;
      fn: number;
    };
    testCount: number;
  }>;
  conflicts: ConflictPair[];
}

export function buildAuditReportJson(result: AuditResult): AuditReportJson {
  return {
    version: 1,
    generatedAt: result.timestamp,
    profileName: result.profileName,
    compositeMetrics: {
      coverage: result.compositeMetrics.coverage,
      accuracy: result.compositeMetrics.accuracy,
      tpr: result.compositeMetrics.truePositiveRate,
      tnr: result.compositeMetrics.trueNegativeRate,
      f1: result.compositeMetrics.f1Score,
    },
    topics: result.topics.map((tr) => ({
      name: tr.topic.topicName,
      action: tr.topic.action,
      description: tr.topic.description,
      metrics: {
        coverage: tr.metrics.coverage,
        accuracy: tr.metrics.accuracy,
        tpr: tr.metrics.truePositiveRate,
        tnr: tr.metrics.trueNegativeRate,
        f1: tr.metrics.f1Score,
        tp: tr.metrics.truePositives,
        tn: tr.metrics.trueNegatives,
        fp: tr.metrics.falsePositives,
        fn: tr.metrics.falseNegatives,
      },
      testCount: tr.testResults.length,
    })),
    conflicts: result.conflicts,
  };
}

function esc(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function pct(n: number): string {
  return `${(n * 100).toFixed(1)}%`;
}

function renderTopicRow(tr: TopicAuditResult): string {
  const m = tr.metrics;
  const covClass = m.coverage >= 0.9 ? 'good' : m.coverage >= 0.5 ? 'warn' : 'bad';
  return `<tr>
    <td>${esc(tr.topic.topicName)}</td>
    <td>${tr.topic.action}</td>
    <td class="${covClass}">${pct(m.coverage)}</td>
    <td>${pct(m.truePositiveRate)}</td>
    <td>${pct(m.trueNegativeRate)}</td>
    <td>${pct(m.accuracy)}</td>
    <td>${pct(m.f1Score)}</td>
    <td>${tr.testResults.length}</td>
  </tr>`;
}

function renderConflictSection(conflicts: ConflictPair[]): string {
  if (conflicts.length === 0) {
    return '<p class="good">No cross-topic conflicts detected.</p>';
  }
  const rows = conflicts
    .map(
      (c) => `<div class="conflict">
    <h4>${esc(c.topicA)} ↔ ${esc(c.topicB)}</h4>
    <p>${esc(c.description)}</p>
    <ul>${c.evidence.map((e) => `<li>${esc(e)}</li>`).join('')}</ul>
  </div>`,
    )
    .join('\n');
  return rows;
}

export function buildAuditReportHtml(result: AuditResult): string {
  const topicRows = result.topics.map(renderTopicRow).join('\n');
  const conflictHtml = renderConflictSection(result.conflicts);
  const m = result.compositeMetrics;

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Prisma AIRS CLI Audit — ${esc(result.profileName)}</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; max-width: 1200px; margin: 0 auto; padding: 2rem; background: #f8f9fa; color: #212529; }
    h1 { font-size: 1.8rem; margin-bottom: 0.5rem; }
    h2 { font-size: 1.4rem; margin: 1.5rem 0 0.75rem; border-bottom: 2px solid #dee2e6; padding-bottom: 0.25rem; }
    h3 { font-size: 1.1rem; margin: 1rem 0 0.5rem; }
    h4 { font-size: 0.95rem; margin: 0.75rem 0 0.25rem; }
    .header { background: #212529; color: #fff; padding: 1.5rem; border-radius: 8px; margin-bottom: 1.5rem; }
    .header h1 { color: #fff; }
    .header .meta { color: #adb5bd; font-size: 0.9rem; }
    .summary { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 1rem; margin-bottom: 1.5rem; }
    .summary-card { background: #fff; padding: 1rem; border-radius: 6px; border: 1px solid #dee2e6; text-align: center; }
    .summary-card .label { font-size: 0.8rem; color: #6c757d; text-transform: uppercase; }
    .summary-card .value { font-size: 1.3rem; font-weight: 600; }
    table { width: 100%; border-collapse: collapse; margin: 0.5rem 0 1rem; }
    th, td { padding: 0.5rem 0.75rem; text-align: left; border-bottom: 1px solid #dee2e6; }
    th { background: #f1f3f5; font-size: 0.85rem; text-transform: uppercase; color: #495057; }
    .good { color: #2b8a3e; font-weight: 600; }
    .warn { color: #e67700; font-weight: 600; }
    .bad { color: #c92a2a; font-weight: 600; }
    .conflict { background: #fff3cd; padding: 1rem; border-radius: 6px; margin: 0.5rem 0; border-left: 4px solid #e67700; }
    .conflict h4 { color: #e67700; }
    .conflict ul { margin-left: 1.5rem; margin-top: 0.5rem; }
    section { background: #fff; padding: 1.25rem; border-radius: 6px; border: 1px solid #dee2e6; margin-bottom: 1rem; }
  </style>
</head>
<body>
  <div class="header">
    <h1>Prisma AIRS CLI Profile Audit</h1>
    <p class="meta">Profile: ${esc(result.profileName)} | ${esc(result.timestamp)}</p>
  </div>

  <div class="summary">
    <div class="summary-card"><div class="label">Coverage</div><div class="value">${pct(m.coverage)}</div></div>
    <div class="summary-card"><div class="label">Accuracy</div><div class="value">${pct(m.accuracy)}</div></div>
    <div class="summary-card"><div class="label">TPR</div><div class="value">${pct(m.truePositiveRate)}</div></div>
    <div class="summary-card"><div class="label">TNR</div><div class="value">${pct(m.trueNegativeRate)}</div></div>
    <div class="summary-card"><div class="label">Topics</div><div class="value">${result.topics.length}</div></div>
    <div class="summary-card"><div class="label">Conflicts</div><div class="value">${result.conflicts.length}</div></div>
  </div>

  <section>
    <h2>Per-Topic Results</h2>
    <table>
      <thead><tr>
        <th>Topic</th><th>Action</th><th>Coverage</th><th>TPR</th><th>TNR</th><th>Accuracy</th><th>F1</th><th>Tests</th>
      </tr></thead>
      <tbody>${topicRows}</tbody>
    </table>
  </section>

  <section>
    <h2>Conflicts</h2>
    ${conflictHtml}
  </section>
</body>
</html>`;
}
