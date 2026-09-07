import { describe, expect, it } from 'vitest';
import { collectRuntimeDailyReport } from '../../../src/reports/runtime.js';
import {
  reportClient,
  reportClock,
  reportLog,
  reportProfile,
} from '../../helpers/runtime-report.js';

describe('daily runtime report collection', () => {
  it('prioritizes explicit scan processing failures without counting unknown statuses', async () => {
    const client = reportClient();
    client.scanLogs.query.mockResolvedValue({
      scan_result_for_dashboard: {
        scan_result_entries: [
          reportLog({ status: 'FAILED' }),
          reportLog({ scan_sub_req_id: 1, status: 'new-state' }),
        ],
      },
    });
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.findings[0]).toMatchObject({
      priority: 'attention',
      title: 'Scan processing failures observed',
    });
    expect(report.findings[0].evidence).toContain('1 timestamp-eligible');
  });
  it('uses only daily SDK reads and separates current inventory, sessions, and log entries', async () => {
    const client = reportClient();
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(client.dashboard.applicationsOverview).toHaveBeenCalledWith({
      timeInterval: 1,
      timeUnit: 'day',
      limit: 100,
      offset: 0,
    });
    expect(client.profiles.list).toHaveBeenCalledWith({ latest: true, limit: 100, offset: 0 });
    expect(client.scanLogs.query).toHaveBeenCalledWith({
      time_interval: 24,
      time_unit: 'hours',
      pageNumber: 1,
      pageSize: 100,
      filter: 'all',
      page_token: undefined,
    });
    expect(report.window).toMatchObject({
      start: '2026-09-06T12:00:00.000Z',
      end: '2026-09-07T12:00:00.000Z',
    });
    expect(report.health).toBe('no-findings');
    expect(report.activity).toMatchObject({ sessions: 25, violatingSessions: 0, violationRate: 0 });
    expect(report.logs).toMatchObject({ entries: 1, tokens: 50 });
    expect(report.sources.every((source) => source.status === 'complete')).toBe(true);
    expect(report.activity.applications[0].registered).toBe(true);
    const content = JSON.stringify(report);
    for (const secret of [
      'NEVER-EXPORT',
      'private-tenant',
      'private-csp',
      'private-scan',
      'private-key-name',
      'private-deployment',
      'private@example.test',
      'profile-private-id',
      '9999',
    ])
      expect(content).not.toContain(secret);
  });

  it('does not confuse a missing response with an empty environment', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview.mockResolvedValue({});
    client.scanLogs.query.mockResolvedValue({});
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.activity.sessions).toBeNull();
    expect(report.activity.violationRate).toBeNull();
    expect(report.logs.tokens).toBeNull();
    expect(report.sources[0].status).toBe('unavailable');
    expect(report.sources[3].status).toBe('unavailable');
    expect(report.health).toBe('review');
    expect(report.findings.some((f) => f.title.includes('No sessions'))).toBe(false);
  });

  it('distinguishes explicit empty lists from unavailable sources', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview.mockResolvedValue({ items: [] });
    client.profiles.list.mockResolvedValue({ ai_profiles: [] });
    client.customerApps.list.mockResolvedValue({ customer_apps: [] });
    client.scanLogs.query.mockResolvedValue({
      scan_result_for_dashboard: { scan_result_entries: [] },
      total_pages: 0,
    });
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.activity.sessions).toBe(0);
    expect(report.sources.every((source) => source.status === 'complete')).toBe(true);
    expect(report.findings.map((finding) => finding.title)).toEqual(
      expect.arrayContaining([
        'No sessions reported in the daily window',
        'No security profiles returned',
      ]),
    );
  });

  it.each([
    401,
    403,
    429,
    500,
    undefined,
    'oops',
    999,
  ])('redacts error details for status %s', async (statusCode) => {
    const client = reportClient();
    for (const method of [
      client.dashboard.applicationsOverview,
      client.profiles.list,
      client.customerApps.list,
      client.scanLogs.query,
    ])
      method.mockRejectedValue(
        Object.assign(new Error('SECRET TOKEN raw request'), { statusCode }),
      );
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.health).toBe('unknown');
    expect(JSON.stringify(report)).not.toContain('SECRET');
    expect(report.sources.every((source) => source.status === 'unavailable')).toBe(true);
  });

  it('walks offset and token pages and keeps same-name buckets with different IDs', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview
      .mockResolvedValueOnce({
        items: [{ id: 'app-1', name: 'Assistant', sessions_total: 25, sessions_violated: 2 }],
        pagination: { total_items: 2 },
      })
      .mockResolvedValueOnce({
        items: [{ id: 'app-2', name: 'Assistant', sessions_total: 5, sessions_violated: 1 }],
        pagination: { total_items: 2 },
      });
    client.profiles.list
      .mockResolvedValueOnce({ ai_profiles: [reportProfile({ revision: 3 })], next_offset: 7 })
      .mockResolvedValueOnce({ ai_profiles: [reportProfile()], next_offset: 0 });
    client.customerApps.list
      .mockResolvedValueOnce({
        customer_apps: [
          {
            customer_appId: 'app-1',
            app_name: 'Registered',
            environment: 'dev',
            cloud_provider: 'aws',
          },
        ],
        next_offset: 5,
      })
      .mockResolvedValueOnce({ customer_apps: [] });
    client.scanLogs.query
      .mockResolvedValueOnce({
        scan_result_for_dashboard: { scan_result_entries: [reportLog()] },
        total_pages: 2,
        page_number: 1,
        page_token: 'PRIVATE-PAGE-TOKEN',
      })
      .mockResolvedValueOnce({
        scan_result_for_dashboard: {
          scan_result_entries: [
            reportLog({ scan_sub_req_id: 1, action: 'block', verdict: 'malicious' }),
          ],
        },
        total_pages: 2,
        page_number: 2,
      });
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(client.profiles.list.mock.calls[1][0].offset).toBe(7);
    expect(client.customerApps.list.mock.calls[1][0].offset).toBe(5);
    expect(client.scanLogs.query.mock.calls[1][0]).toMatchObject({
      pageNumber: 2,
      page_token: 'PRIVATE-PAGE-TOKEN',
    });
    expect(report.activity.sessions).toBe(30);
    expect(report.activity.violatingSessions).toBe(3);
    expect(report.activity.violationRate).toBe(10);
    expect(report.activity.applications).toHaveLength(2);
    expect(report.profiles).toHaveLength(1);
    expect(report.profiles[0].revision).toBe(4);
    expect(report.health).toBe('attention');
    expect(report.logs.entries).toBe(2);
    expect(JSON.stringify(report)).not.toContain('PRIVATE-PAGE-TOKEN');
  });

  it('retains first-page evidence on a later error and never calls it complete', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview
      .mockResolvedValueOnce({
        items: [{ id: 'a', name: 'A', sessions_total: 2, sessions_violated: 1 }],
        pagination: { total_items: 2 },
      })
      .mockRejectedValueOnce(new Error('later failure'));
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.activity.sessions).toBe(2);
    expect(report.sources[0]).toMatchObject({ status: 'partial', records: 1 });
  });

  it('enforces page budgets and detects non-advancing/repeated pages', async () => {
    const client = reportClient();
    client.profiles.list.mockResolvedValue({ ai_profiles: [reportProfile()], next_offset: 7 });
    const report = await collectRuntimeDailyReport(client, { maxPages: 1, now: reportClock });
    expect(report.sources[1].notes.join()).toContain('page budget');
    const second = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(second.sources[1].status).toBe('partial');
    expect(second.sources[1].notes.join()).toContain('Pagination did not advance');
  });

  it('deduplicates overlapping pages without hiding the overlap', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [
        { id: 'a', name: 'A', sessions_total: 1, sessions_violated: 0 },
        { id: 'a', name: 'A', sessions_total: 1, sessions_violated: 0 },
      ],
    });
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.activity.sessions).toBe(1);
    expect(report.sources[0].status).toBe('partial');
  });

  it('does not silently complete an empty page before the advertised total', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [],
      pagination: { total_items: 5 },
    });
    expect((await collectRuntimeDailyReport(client)).sources[0].status).toBe('unavailable');
  });

  it('handles full pages without pagination metadata by probing for the next page', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview
      .mockResolvedValueOnce({
        items: Array.from({ length: 100 }, (_, i) => ({
          id: `${i}`,
          name: `${i}`,
          sessions_total: 1,
          sessions_violated: 0,
        })),
      })
      .mockResolvedValueOnce({ items: [] });
    client.profiles.list
      .mockResolvedValueOnce({
        ai_profiles: Array.from({ length: 100 }, (_, i) => reportProfile({ profile_name: `${i}` })),
      })
      .mockResolvedValueOnce({ ai_profiles: [] });
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(client.dashboard.applicationsOverview.mock.calls[1][0].offset).toBe(100);
    expect(client.profiles.list.mock.calls[1][0].offset).toBe(100);
    expect(report.sources[0].status).toBe('complete');
  });

  it.each([
    -1,
    0,
    1.5,
    101,
    NaN,
    Infinity,
  ])('rejects invalid page budget %s before requests', async (maxPages) => {
    const client = reportClient();
    await expect(collectRuntimeDailyReport(client, { maxPages })).rejects.toThrow('maxPages');
    expect(client.profiles.list).not.toHaveBeenCalled();
  });

  it('treats missing, negative, fractional, overflowing and inconsistent counters as unknown', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [
        { id: 'a', name: null, sessions_total: 1, sessions_violated: 2 },
        { id: 'b', name: 'B', sessions_total: -1, sessions_violated: 0.5 },
        { id: 'c', sessions_total: Number.MAX_SAFE_INTEGER + 1 },
      ],
    });
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.activity.sessions).toBeNull();
    expect(report.activity.violationRate).toBeNull();
    expect(report.findings.some((f) => f.title === 'Activity counters need verification')).toBe(
      true,
    );
  });

  it('withholds a rate for an individual inconsistent bucket even when aggregate totals would fit', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [
        { id: 'a', sessions_total: 1, sessions_violated: 2 },
        { id: 'b', sessions_total: 20, sessions_violated: 0 },
      ],
    });
    expect((await collectRuntimeDailyReport(client)).activity.violationRate).toBeNull();
  });

  it('handles a safe-integer overflow in the aggregate', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [
        { id: 'a', sessions_total: Number.MAX_SAFE_INTEGER, sessions_violated: 0 },
        { id: 'b', sessions_total: 1, sessions_violated: 0 },
      ],
    });
    expect((await collectRuntimeDailyReport(client)).activity.sessions).toBeNull();
  });

  it('uses explicit current policy evidence, not missing settings or old revisions', async () => {
    const client = reportClient();
    client.profiles.list.mockResolvedValue({
      ai_profiles: [
        reportProfile({ revision: 1, active: false }),
        reportProfile(),
        reportProfile({ profile_name: 'Inactive', active: false }),
        reportProfile({
          profile_name: 'Review',
          last_modified_ts: '2026-09-07T01:00:00Z',
          policy: {
            'ai-security-profiles': [
              {
                'model-configuration': {
                  latency: { 'inline-timeout-action': 'allow' },
                  'mask-data-in-storage': false,
                },
              },
            ],
          },
        }),
        {
          profile_name: 'Missing',
          policy: { 'ai-security-profiles': [{}] },
          last_modified_ts: 'bad',
        },
      ],
    });
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.findings.map((f) => f.title)).toEqual(
      expect.arrayContaining([
        'Inactive profile: Inactive',
        'Timeout allows traffic: Review',
        'Storage masking disabled: Review',
      ]),
    );
    expect(
      report.findings.some(
        (f) => f.title.includes('Production guardrail') || f.title.includes('Missing'),
      ),
    ).toBe(false);
    expect(report.profiles.find((p) => p.name === 'Missing')).toMatchObject({
      active: null,
      revision: null,
      modified: null,
      storageMasking: 'Unknown',
    });
  });

  it('filters logs to the anchored half-open UTC window and excludes missing timestamps', async () => {
    const client = reportClient();
    client.scanLogs.query.mockResolvedValue({
      scan_result_for_dashboard: {
        scan_result_entries: [
          reportLog({ scan_sub_req_id: 0, received_ts: '2026-09-06T12:00:00Z' }),
          reportLog({ scan_sub_req_id: 1, received_ts: '2026-09-07T12:00:00Z' }),
          reportLog({ scan_sub_req_id: 2, received_ts: undefined }),
          reportLog({ scan_sub_req_id: 3, received_ts: 'bad' }),
          reportLog({ scan_sub_req_id: 4, received_ts: '2026-09-06T11:59:59Z' }),
          reportLog({ scan_sub_req_id: 5, tokens: -1, action: undefined, verdict: undefined }),
        ],
      },
    });
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.logs).toMatchObject({
      entries: 2,
      tokens: null,
      missingTimestamps: 2,
      outsideWindow: 2,
    });
    expect(report.logs.actions).toContainEqual({ name: 'unknown', count: 1 });
  });

  it('stops repeated scan tokens and incorrect page numbers', async () => {
    const client = reportClient();
    client.scanLogs.query.mockResolvedValue({
      scan_result_for_dashboard: { scan_result_entries: [reportLog()] },
      page_token: 'repeat',
    });
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.sources[3].status).toBe('partial');
    expect(client.scanLogs.query).toHaveBeenCalledTimes(2);
    client.scanLogs.query.mockResolvedValue({ page_number: 9 });
    expect((await collectRuntimeDailyReport(client)).sources[3].status).toBe('unavailable');
  });

  it('bounds and removes controls from display metadata and preserves unknown registration', async () => {
    const client = reportClient();
    client.customerApps.list.mockRejectedValue(new Error());
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [{ name: `\u001b[31m\n${'A'.repeat(400)}`, sessions_total: 1, sessions_violated: 0 }],
    });
    const report = await collectRuntimeDailyReport(client, { title: '\u0000  ', now: reportClock });
    expect(report.title).toBe('Unknown');
    expect(report.activity.applications[0].name.length).toBe(240);
    expect(report.activity.applications[0].name).not.toContain('\u001b');
    expect(report.activity.applications[0].registered).toBeNull();
  });
});
