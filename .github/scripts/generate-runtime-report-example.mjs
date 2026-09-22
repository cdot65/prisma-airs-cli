/** Deterministic, synthetic docs sample; never reads config or contacts a service. */
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import {
  collectRuntimeDailyReport,
  renderRuntimeReportHtml,
  renderRuntimeReportMarkdown,
} from '../../dist/index.js';

export async function exampleReport() {
  const sessionItems = Array.from({ length: 1300 }, (_, index) => ({
    application_id: index < 860 ? 'a' : index < 1180 ? 'b' : 'c',
    application_name:
      index < 860 ? 'Customer support' : index < 1180 ? 'Engineering assistant' : 'Document search',
    session_id: `synthetic-${index}`,
    last_session_activity: '2026-09-07T07:00:00Z',
    violation_status: index < 43 || (index >= 860 && index < 868) ? 'violated' : 'passed',
  }));
  const client = {
    dashboard: {
      sessionsOverview: async ({ offset, limit }) => ({
        items: sessionItems.slice(offset, offset + limit),
        pagination: { limit, skip: offset, total_items: sessionItems.length },
      }),
      sessionsChart: async () => ({
        buckets: [
          {
            bucket_number: 0,
            time: '2026-09-07T07:00:00Z',
            total: 1300,
            violating: 51,
            violation_breakdown: { critical: 0, high: 0, medium: 52, low: 0, total: 52 },
          },
        ],
      }),
      topApplicationsViolations: async () => ({
        applications: [
          {
            id: 'a',
            name: 'Customer support',
            total_violations: 52,
            policy_violations: [{ detection_type: 'pi', total: 52 }],
          },
        ],
      }),
      applicationsViolationsTrend: async () => ({
        violations: [
          {
            bucket_number: 0,
            date: '2026-09-07T07:00:00Z',
            violation_breakdown: { critical: 0, high: 0, medium: 52, low: 0, total: 52 },
          },
        ],
      }),
      applicationsOverview: async () => ({
        items: [
          { id: 'a', name: 'Customer support', sessions_total: 860, sessions_violated: 43 },
          { id: 'b', name: 'Engineering assistant', sessions_total: 320, sessions_violated: 8 },
          { id: 'c', name: 'Document search', sessions_total: 120, sessions_violated: 0 },
        ],
        pagination: { total_items: 3 },
      }),
    },
    profiles: {
      list: async () => ({
        ai_profiles: [
          {
            profile_name: 'Production protection',
            revision: 8,
            active: true,
            last_modified_ts: '2026-09-06T08:00:00Z',
            policy: {
              'ai-security-profiles': [
                {
                  'model-configuration': {
                    latency: { 'inline-timeout-action': 'block' },
                    'mask-data-in-storage': true,
                  },
                },
              ],
            },
          },
          {
            profile_name: 'Staging availability',
            revision: 2,
            active: true,
            policy: {
              'ai-security-profiles': [
                {
                  'model-configuration': {
                    latency: { 'inline-timeout-action': 'allow' },
                    'mask-data-in-storage': true,
                  },
                },
              ],
            },
          },
          { profile_name: 'Retired experiment', revision: 1, active: false },
        ],
        next_offset: 0,
      }),
    },
    customerApps: {
      list: async () => ({
        customer_apps: [
          {
            customer_appId: 'a',
            app_name: 'Support service',
            environment: 'production',
            cloud_provider: 'aws',
            model_name: 'Example model',
            api_keys_dp_info: [],
          },
          {
            customer_appId: 'b',
            app_name: 'Engineering service',
            environment: 'staging',
            cloud_provider: 'azure',
            model_name: 'Example model',
          },
          {
            customer_appId: 'c',
            app_name: 'Search service',
            environment: 'production',
            cloud_provider: 'gcp',
          },
        ],
      }),
    },
  };
  const report = await collectRuntimeDailyReport(client, {
    title: 'Daily security review · Synthetic example',
    now: () => new Date('2026-09-07T08:00:00Z'),
    maxPages: 60,
  });
  report.limitations.unshift(
    'SYNTHETIC EXAMPLE: all application names, counts, and configurations are fabricated documentation fixtures, not live customer data.',
    'This fixture uses a 60-page budget to collect 1,300 sessions; CLI default is 40 pages and would explicitly mark that capped collection partial.',
  );
  return report;
}

if (import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const report = await exampleReport();
  const destination = resolve('docs-site/static/examples/runtime-daily-report');
  await mkdir(dirname(destination), { recursive: true });
  // .htm remains an HTML deliverable and avoids the preview server's broken .html clean-URL redirect.
  await writeFile(`${destination}.htm`, renderRuntimeReportHtml(report));
  await writeFile(`${destination}.md`, renderRuntimeReportMarkdown(report));
  console.log('Generated synthetic runtime report examples (no network or credentials).');
}
