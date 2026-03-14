/**
 * HTML report builder — self-contained HTML from ReportOutput.
 */

import type { EfficacyMetrics } from '../core/types.js';
import type { IterationSummary, ReportOutput, RunDiff, TestDetail } from './types.js';

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

function delta(n: number): string {
  const sign = n >= 0 ? '+' : '';
  return `${sign}${(n * 100).toFixed(1)}%`;
}

function deltaClass(n: number): string {
  if (n > 0) return 'positive';
  if (n < 0) return 'negative';
  return 'neutral';
}

function renderMetricsTable(m: EfficacyMetrics): string {
  return `<table class="metrics">
    <tr><th>Coverage</th><td>${pct(m.coverage)}</td></tr>
    <tr><th>Accuracy</th><td>${pct(m.accuracy)}</td></tr>
    <tr><th>TPR</th><td>${pct(m.truePositiveRate)}</td></tr>
    <tr><th>TNR</th><td>${pct(m.trueNegativeRate)}</td></tr>
    <tr><th>F1</th><td>${pct(m.f1Score)}</td></tr>
    <tr><th>TP</th><td>${m.truePositives}</td></tr>
    <tr><th>TN</th><td>${m.trueNegatives}</td></tr>
    <tr><th>FP</th><td>${m.falsePositives}</td></tr>
    <tr><th>FN</th><td>${m.falseNegatives}</td></tr>
    <tr><th>Regressions</th><td>${m.regressionCount}</td></tr>
  </table>`;
}

function renderTestTable(tests: TestDetail[]): string {
  const rows = tests
    .map(
      (t) =>
        `<tr class="${t.correct ? 'pass' : 'fail'}">
      <td>${esc(t.prompt)}</td>
      <td>${t.expectedTriggered ? 'Yes' : 'No'}</td>
      <td>${t.actualTriggered ? 'Yes' : 'No'}</td>
      <td>${t.correct ? 'Pass' : 'Fail'}</td>
      <td>${esc(t.category)}</td>
      <td>${t.source ? esc(t.source) : '-'}</td>
      <td>${t.scanAction}</td>
    </tr>`,
    )
    .join('\n');

  return `<table class="tests">
    <thead><tr>
      <th>Prompt</th><th>Expected</th><th>Actual</th><th>Result</th>
      <th>Category</th><th>Source</th><th>Action</th>
    </tr></thead>
    <tbody>${rows}</tbody>
  </table>`;
}

function renderIteration(iter: IterationSummary): string {
  const examplesHtml =
    iter.topic.examples.length > 0
      ? `<ul>${iter.topic.examples.map((e) => `<li>${esc(e)}</li>`).join('')}</ul>`
      : '<p>None</p>';

  const analysisHtml = `
    <p>${esc(iter.analysis.summary)}</p>
    ${iter.analysis.falsePositivePatterns.length > 0 ? `<p><strong>FP Patterns:</strong> ${iter.analysis.falsePositivePatterns.map(esc).join('; ')}</p>` : ''}
    ${iter.analysis.falseNegativePatterns.length > 0 ? `<p><strong>FN Patterns:</strong> ${iter.analysis.falseNegativePatterns.map(esc).join('; ')}</p>` : ''}
    ${iter.analysis.suggestions.length > 0 ? `<p><strong>Suggestions:</strong> ${iter.analysis.suggestions.map(esc).join('; ')}</p>` : ''}
  `;

  const testsHtml = iter.tests ? `<h4>Test Results</h4>${renderTestTable(iter.tests)}` : '';

  return `<section class="iteration">
    <h3>Iteration ${iter.iteration}</h3>
    <p class="meta">Duration: ${(iter.durationMs / 1000).toFixed(1)}s | ${iter.timestamp}</p>
    <h4>Topic</h4>
    <p><strong>${esc(iter.topic.name)}</strong></p>
    <p>${esc(iter.topic.description)}</p>
    <h4>Examples</h4>
    ${examplesHtml}
    <h4>Metrics</h4>
    ${renderMetricsTable(iter.metrics)}
    <h4>Analysis</h4>
    ${analysisHtml}
    ${testsHtml}
  </section>`;
}

function renderTrends(iterations: IterationSummary[]): string {
  if (iterations.length < 2) return '';

  const rows = iterations
    .map(
      (iter) =>
        `<tr>
      <td>${iter.iteration}</td>
      <td>${pct(iter.metrics.coverage)}</td>
      <td>${pct(iter.metrics.truePositiveRate)}</td>
      <td>${pct(iter.metrics.trueNegativeRate)}</td>
      <td>${pct(iter.metrics.accuracy)}</td>
      <td>${pct(iter.metrics.f1Score)}</td>
      <td>${iter.metrics.falsePositives}</td>
      <td>${iter.metrics.falseNegatives}</td>
    </tr>`,
    )
    .join('\n');

  return `<section class="trends">
    <h2>Iteration Trends</h2>
    <table class="metrics">
      <thead><tr>
        <th>Iter</th><th>Coverage</th><th>TPR</th><th>TNR</th>
        <th>Accuracy</th><th>F1</th><th>FP</th><th>FN</th>
      </tr></thead>
      <tbody>${rows}</tbody>
    </table>
  </section>`;
}

function renderDiff(diff: RunDiff): string {
  const d = diff.metricsDelta;
  return `<section class="diff">
    <h2>Run Comparison</h2>
    <table class="metrics">
      <thead><tr><th>Metric</th><th>${esc(diff.baseRunId)}</th><th>${esc(diff.compareRunId)}</th><th>Delta</th></tr></thead>
      <tbody>
        <tr><td>Coverage</td><td>${pct(diff.baseMetrics.coverage)}</td><td>${pct(diff.compareMetrics.coverage)}</td><td class="${deltaClass(d.coverage)}">${delta(d.coverage)}</td></tr>
        <tr><td>TPR</td><td>${pct(diff.baseMetrics.truePositiveRate)}</td><td>${pct(diff.compareMetrics.truePositiveRate)}</td><td class="${deltaClass(d.tpr)}">${delta(d.tpr)}</td></tr>
        <tr><td>TNR</td><td>${pct(diff.baseMetrics.trueNegativeRate)}</td><td>${pct(diff.compareMetrics.trueNegativeRate)}</td><td class="${deltaClass(d.tnr)}">${delta(d.tnr)}</td></tr>
        <tr><td>Accuracy</td><td>${pct(diff.baseMetrics.accuracy)}</td><td>${pct(diff.compareMetrics.accuracy)}</td><td class="${deltaClass(d.accuracy)}">${delta(d.accuracy)}</td></tr>
        <tr><td>F1</td><td>${pct(diff.baseMetrics.f1Score)}</td><td>${pct(diff.compareMetrics.f1Score)}</td><td class="${deltaClass(d.f1)}">${delta(d.f1)}</td></tr>
      </tbody>
    </table>
    <h3>Topic Comparison</h3>
    <div class="topic-diff">
      <div><h4>Base</h4><p><strong>${esc(diff.baseTopic.name)}</strong></p><p>${esc(diff.baseTopic.description)}</p></div>
      <div><h4>Compare</h4><p><strong>${esc(diff.compareTopic.name)}</strong></p><p>${esc(diff.compareTopic.description)}</p></div>
    </div>
  </section>`;
}

export function buildReportHtml(report: ReportOutput): string {
  const iterationsHtml = report.iterations.map(renderIteration).join('\n');
  const trendsHtml = renderTrends(report.iterations);
  const diffHtml = report.diff ? renderDiff(report.diff) : '';

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Prisma AIRS CLI Report — ${esc(report.run.id)}</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; max-width: 1200px; margin: 0 auto; padding: 2rem; background: #f8f9fa; color: #212529; }
    h1 { font-size: 1.8rem; margin-bottom: 0.5rem; }
    h2 { font-size: 1.4rem; margin: 1.5rem 0 0.75rem; border-bottom: 2px solid #dee2e6; padding-bottom: 0.25rem; }
    h3 { font-size: 1.1rem; margin: 1rem 0 0.5rem; }
    h4 { font-size: 0.95rem; margin: 0.75rem 0 0.25rem; color: #495057; }
    .header { background: #212529; color: #fff; padding: 1.5rem; border-radius: 8px; margin-bottom: 1.5rem; }
    .header h1 { color: #fff; }
    .header .meta { color: #adb5bd; font-size: 0.9rem; }
    .summary { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; margin-bottom: 1.5rem; }
    .summary-card { background: #fff; padding: 1rem; border-radius: 6px; border: 1px solid #dee2e6; }
    .summary-card .label { font-size: 0.8rem; color: #6c757d; text-transform: uppercase; }
    .summary-card .value { font-size: 1.3rem; font-weight: 600; }
    table { width: 100%; border-collapse: collapse; margin: 0.5rem 0 1rem; }
    th, td { padding: 0.5rem 0.75rem; text-align: left; border-bottom: 1px solid #dee2e6; }
    th { background: #f1f3f5; font-size: 0.85rem; text-transform: uppercase; color: #495057; }
    .pass td:nth-child(4) { color: #2b8a3e; font-weight: 600; }
    .fail td:nth-child(4) { color: #c92a2a; font-weight: 600; }
    .iteration { background: #fff; padding: 1.25rem; border-radius: 6px; border: 1px solid #dee2e6; margin-bottom: 1rem; }
    .meta { font-size: 0.85rem; color: #868e96; }
    .positive { color: #2b8a3e; }
    .negative { color: #c92a2a; }
    .neutral { color: #868e96; }
    .trends { background: #fff; padding: 1.25rem; border-radius: 6px; border: 1px solid #dee2e6; margin-bottom: 1rem; }
    .diff { background: #fff; padding: 1.25rem; border-radius: 6px; border: 1px solid #dee2e6; margin-bottom: 1rem; }
    .topic-diff { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
    .tests { font-size: 0.85rem; }
    .tests td:first-child { max-width: 400px; word-break: break-word; }
  </style>
</head>
<body>
  <div class="header">
    <h1>Prisma AIRS CLI Evaluation Report</h1>
    <p class="meta">Generated ${esc(report.generatedAt)} | Run ${esc(report.run.id)}</p>
  </div>

  <div class="summary">
    <div class="summary-card"><div class="label">Status</div><div class="value">${esc(report.run.status)}</div></div>
    <div class="summary-card"><div class="label">Intent</div><div class="value">${esc(report.run.intent)}</div></div>
    <div class="summary-card"><div class="label">Best Coverage</div><div class="value">${pct(report.run.bestCoverage)}</div></div>
    <div class="summary-card"><div class="label">Iterations</div><div class="value">${report.run.totalIterations}</div></div>
    <div class="summary-card"><div class="label">Best Iteration</div><div class="value">#${report.run.bestIteration}</div></div>
    <div class="summary-card"><div class="label">Profile</div><div class="value">${esc(report.run.profileName)}</div></div>
  </div>

  <h2>Topic</h2>
  <p>${esc(report.run.topicDescription)}</p>

  ${trendsHtml}

  <h2>Iterations</h2>
  ${iterationsHtml}

  ${diffHtml}
</body>
</html>`;
}
