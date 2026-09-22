import { createHash } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import {
  renderRuntimeReportHtml,
  renderRuntimeReportMarkdown,
} from '../../../src/reports/render.js';
import { collectRuntimeDailyReport } from '../../../src/reports/runtime.js';
import { reportClient, reportClock } from '../../helpers/runtime-report.js';

describe('report deliverable renderers', () => {
  it('renders a complete document with no external resources and hash-pinned inline assets', async () => {
    const report = await collectRuntimeDailyReport(reportClient(), { now: reportClock });
    const html = renderRuntimeReportHtml(report);
    expect(html).toMatch(/^<!doctype html>/);
    expect(html).toContain('<html lang="en">');
    expect(html).toContain('No findings in available evidence');
    expect(html).toContain('default-src &#39;none&#39;');
    expect(html).not.toMatch(/<(link|iframe|img)|\bsrc=/i);
    for (const tag of ['style', 'script']) {
      const text = html.match(new RegExp(`<${tag}>([\\s\\S]*?)</${tag}>`))?.[1];
      expect(text).toBeTruthy();
      const hash = createHash('sha256')
        .update(text ?? '')
        .digest('base64');
      expect(html).toContain(`sha256-${hash}`);
    }
    expect(html).toContain('25');
    expect(html).toContain('@media print');
    expect(html).toContain('scope="col"');
    expect(html).toContain('role="status"');
  });

  it('escapes script, attribute, HTML and Markdown injection in every dynamic surface', async () => {
    const attack =
      '</script><script>globalThis.PWNED=1</script><img src=x onerror=alert(1)> " & [click](https://evil.test) | \n';
    const report = await collectRuntimeDailyReport(reportClient(), { now: reportClock });
    report.title = attack;
    report.activity.applications[0].name = attack;
    report.profiles[0].name = attack;
    report.registeredApps[0].name = attack;
    report.findings.push({
      priority: 'review',
      title: attack,
      evidence: attack,
      recommendation: attack,
      source: attack,
    });
    report.sources[0].notes.push(attack);
    report.limitations.push(attack);
    report.dailyTelemetry.topApplications.push({
      name: attack,
      violations: 40,
      detectors: [{ name: attack, count: 40 }],
    });
    report.dailyTelemetry.violationTrend.push({
      time: attack,
      violations: { critical: 0, high: 0, medium: 26, low: 14, total: 40 },
    });
    report.dailyTelemetry.chart.buckets.push({
      time: attack,
      sessions: 174,
      violatingSessions: 34,
      violations: { critical: 0, high: 0, medium: 26, low: 14, total: 40 },
    });
    const html = renderRuntimeReportHtml(report);
    expect(html.match(/<script>/g)).toHaveLength(1);
    expect(html).not.toContain('<img');
    expect(html).not.toContain('<script>globalThis');
    expect(html).toContain('&lt;/script&gt;');
    expect(html).not.toContain('innerHTML');
    const markdown = renderRuntimeReportMarkdown(report);
    expect(markdown).not.toContain('<script>');
    expect(markdown).not.toContain('[click]');
    expect(markdown).not.toContain('| \n');
    expect(markdown).toContain('&#60;/script&#62;');
  });

  it('keeps the Markdown and HTML evidence aligned', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [{ name: 'Priority app', sessions_total: 40, sessions_violated: 10 }],
    });
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    for (const output of [renderRuntimeReportHtml(report), renderRuntimeReportMarkdown(report)]) {
      expect(output).toContain('Attention required');
      expect(output).toContain('Priority app');
      expect(output).toContain('25%');
      expect(output).toContain('Violating sessions observed');
      expect(output).toContain('Next step:');
      expect(output).toContain('Current security profiles');
      expect(output).toContain('Evidence and collection coverage');
      expect(output).toContain('not a configuration-change audit');
      expect(output).toContain('Report schema 2');
    }
  });

  it('renders unknown and empty sources without presenting fabricated zero activity', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview.mockResolvedValue({});
    client.profiles.list.mockResolvedValue({ ai_profiles: [] });
    client.customerApps.list.mockResolvedValue({});
    client.dashboard.sessionsOverview.mockResolvedValue({});
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    for (const output of [renderRuntimeReportHtml(report), renderRuntimeReportMarkdown(report)]) {
      expect(output).toContain('Unknown');
      expect(output).toContain('No rows available');
      expect(output).toContain('unavailable');
    }
  });

  it('covers null policy fields and inconsistent per-app rate rendering', async () => {
    const client = reportClient();
    client.profiles.list.mockResolvedValue({
      ai_profiles: [{ profile_name: 'Unknown' }, { profile_name: 'Inactive', active: false }],
    });
    client.customerApps.list.mockResolvedValue({
      customer_apps: [
        { customer_appId: 'a', app_name: 'A', cloud_provider: 'aws', environment: 'dev' },
      ],
    });
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [{ id: 'b', name: 'B', sessions_total: 1, sessions_violated: 2 }],
    });
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(renderRuntimeReportHtml(report)).toContain('Not in returned inventory');
    expect(renderRuntimeReportMarkdown(report)).not.toContain('200%');
    report.sources = [];
    expect(renderRuntimeReportHtml(report)).toContain('Source: unavailable');
  });
});
