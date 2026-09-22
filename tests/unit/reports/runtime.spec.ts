import { describe, expect, it } from 'vitest';
import { collectRuntimeDailyReport } from '../../../src/reports/runtime.js';
import {
  noViolations,
  reportClient,
  reportClock,
  reportProfile,
  reportSession,
} from '../../helpers/runtime-report.js';

function page(items = [reportSession()], total = items.length, skip = 0) {
  return { items, pagination: { total_items: total, skip, limit: 25 } };
}

describe('daily runtime report collection', () => {
  it('uses seven read-only sources, preserves units and excludes raw identifiers/content', async () => {
    const client = reportClient();
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(client.dashboard.sessionsOverview).toHaveBeenCalledWith({
      timeInterval: 1,
      timeUnit: 'day',
      limit: 25,
      offset: 0,
    });
    expect(client.dashboard.applicationsOverview).toHaveBeenCalledWith({
      timeInterval: 1,
      timeUnit: 'day',
      limit: 100,
      offset: 0,
    });
    expect(client.profiles.list).toHaveBeenCalledWith({ latest: true, limit: 100, offset: 0 });
    expect(report.sources).toHaveLength(7);
    expect(report.sources.every((source) => source.status === 'complete')).toBe(true);
    expect(report).toMatchObject({
      schemaVersion: 2,
      health: 'no-findings',
      activity: { sessions: 25, violatingSessions: 0 },
      sessions: { entries: 1 },
      dailyTelemetry: { chart: { sessions: 1 } },
    });
    expect(report.window).toMatchObject({
      start: '2026-09-06T12:00:00.000Z',
      end: '2026-09-07T12:00:00.000Z',
    });
    expect(report.activity.applications[0].registered).toBe(true);
    for (const secret of [
      'NEVER-EXPORT',
      'private-tenant',
      'private-session',
      'private-key-name',
      'private-deployment',
      'private@example.test',
      'profile-private-id',
    ])
      expect(JSON.stringify(report)).not.toContain(secret);
    expect(report).not.toHaveProperty('logs');
  });

  it('distinguishes missing bodies from explicit empty collections', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview.mockResolvedValue({});
    client.dashboard.sessionsOverview.mockResolvedValue({});
    client.dashboard.sessionsChart.mockResolvedValue({});
    let report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.activity.sessions).toBeNull();
    expect(report.sessions.entries).toBeNull();
    expect(report.dailyTelemetry.chart.sessions).toBeNull();
    expect(report.health).toBe('review');
    client.dashboard.applicationsOverview.mockResolvedValue({ items: [] });
    client.dashboard.sessionsOverview.mockResolvedValue(page([]));
    client.dashboard.sessionsChart.mockResolvedValue({ buckets: [] });
    client.profiles.list.mockResolvedValue({ ai_profiles: [] });
    client.customerApps.list.mockResolvedValue({ customer_apps: [] });
    report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.sources.every((source) => source.status === 'complete')).toBe(true);
    expect(report.activity.sessions).toBe(0);
    expect(report.findings.map((f) => f.title)).toEqual(
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
  ])('withholds raw errors for status %s', async (statusCode) => {
    const client = reportClient();
    for (const method of [
      ...Object.values(client.dashboard),
      client.profiles.list,
      client.customerApps.list,
    ])
      method.mockRejectedValue(Object.assign(new Error('SECRET TOKEN'), { statusCode }));
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.health).toBe('unknown');
    expect(report.sources.every((source) => source.status === 'unavailable')).toBe(true);
    expect(JSON.stringify(report)).not.toContain('SECRET');
  });

  it('walks all session pages using composite identities and retains independent chart counts', async () => {
    const client = reportClient();
    client.dashboard.sessionsOverview
      .mockResolvedValueOnce(page([reportSession()], 2))
      .mockResolvedValueOnce(page([reportSession({ application_id: 'another-app' })], 2, 1));
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(client.dashboard.sessionsOverview.mock.calls[1][0].offset).toBe(1);
    expect(report.sessions.entries).toBe(2);
    expect(report.dailyTelemetry.chart.sessions).toBe(1);
    expect(report.findings.some((f) => f.title === 'Session sources disagree')).toBe(true);
  });

  it.each([
    { items: [], pagination: { limit: 25, skip: 0, total_items: 2 } },
    { items: [reportSession()], pagination: { limit: 25, skip: 1, total_items: 1 } },
    { items: [reportSession()], pagination: { limit: 10, skip: 0, total_items: 1 } },
    { items: [reportSession()], pagination: {} },
    page([reportSession()], 0),
  ])('rejects incoherent initial session metadata', async (response) => {
    const client = reportClient();
    client.dashboard.sessionsOverview.mockResolvedValue(response);
    expect((await collectRuntimeDailyReport(client)).sources[3].status).toBe('unavailable');
  });

  it('marks changing totals, overlapping pages, capped reads and later errors as partial', async () => {
    for (const later of [
      page([reportSession({ session_id: 'second' })], 3, 1),
      page([reportSession()], 2, 1),
      new Error('SECRET'),
    ]) {
      const client = reportClient();
      client.dashboard.sessionsOverview.mockResolvedValueOnce(page([reportSession()], 2));
      if (later instanceof Error) client.dashboard.sessionsOverview.mockRejectedValueOnce(later);
      else client.dashboard.sessionsOverview.mockResolvedValueOnce(later);
      const report = await collectRuntimeDailyReport(client, { now: reportClock });
      expect(report.sources[3].status).toBe('partial');
      expect(report.sessions.entries).toBe(1);
    }
    const client = reportClient();
    client.dashboard.sessionsOverview.mockResolvedValue(page([reportSession()], 2));
    expect(
      (await collectRuntimeDailyReport(client, { maxPages: 1 })).sources[3].notes.join(),
    ).toContain('page budget');
  });

  it('bounds metadata, sanitizes missing counters and withholds inconsistent rates', async () => {
    const client = reportClient();
    client.customerApps.list.mockRejectedValue(new Error());
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [
        { id: 'a', name: `\u001b${'A'.repeat(400)}`, sessions_total: 1, sessions_violated: 2 },
        { id: 'b', sessions_total: -1, sessions_violated: 0.5 },
        { id: 'c', sessions_total: Number.MAX_SAFE_INTEGER + 1 },
      ],
    });
    const report = await collectRuntimeDailyReport(client, { now: reportClock, title: '\u0000 ' });
    expect(report.title).toBe('Unknown');
    expect(report.activity.violationRate).toBeNull();
    expect(report.activity.sessions).toBeNull();
    expect(report.activity.applications[0].name).toHaveLength(240);
    expect(report.activity.applications[0].registered).toBeNull();
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [
        { id: 'a', sessions_total: Number.MAX_SAFE_INTEGER, sessions_violated: 0 },
        { id: 'b', sessions_total: 1, sessions_violated: 0 },
      ],
    });
    expect((await collectRuntimeDailyReport(client)).activity.sessions).toBeNull();
  });

  it('retains page evidence and detects application total changes and incomplete pages', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview
      .mockResolvedValueOnce({
        items: [{ id: 'a', name: 'A', sessions_total: 2, sessions_violated: 1 }],
        pagination: { total_items: 2 },
      })
      .mockResolvedValueOnce({ items: [{ id: 'b' }], pagination: { total_items: 3 } });
    let report = await collectRuntimeDailyReport(client);
    expect(report.sources[0].status).toBe('partial');
    expect(report.activity.sessions).toBe(2);
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [],
      pagination: { total_items: 5 },
    });
    report = await collectRuntimeDailyReport(client);
    expect(report.sources[0].status).toBe('unavailable');
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [{ id: 'a' }],
      pagination: { total_items: 0 },
    });
    expect((await collectRuntimeDailyReport(client)).sources[0].status).toBe('unavailable');
  });

  it('handles absent pagination, next offsets, duplicate buckets and non-advancing inventories', async () => {
    const client = reportClient();
    client.dashboard.applicationsOverview
      .mockResolvedValueOnce({
        items: Array.from({ length: 100 }, (_, i) => ({
          id: String(i),
          sessions_total: 1,
          sessions_violated: 0,
        })),
      })
      .mockResolvedValueOnce({ items: [] });
    client.profiles.list
      .mockResolvedValueOnce({
        ai_profiles: Array.from({ length: 100 }, (_, i) =>
          reportProfile({ profile_name: String(i) }),
        ),
      })
      .mockResolvedValueOnce({ ai_profiles: [] });
    let report = await collectRuntimeDailyReport(client);
    expect(report.sources[0].status).toBe('complete');
    expect(client.profiles.list.mock.calls[1][0].offset).toBe(100);
    client.dashboard.applicationsOverview.mockResolvedValue({
      items: [
        { id: 'a', sessions_total: 1 },
        { id: 'a', sessions_total: 1 },
      ],
    });
    client.profiles.list.mockResolvedValue({ ai_profiles: [reportProfile()], next_offset: 7 });
    client.customerApps.list.mockResolvedValueOnce({ customer_apps: [], next_offset: 5 });
    report = await collectRuntimeDailyReport(client);
    expect(report.sources[0].status).toBe('partial');
    expect(report.sources[1].notes.join()).toContain('Pagination did not advance');
    expect(report.sources[2].status).toBe('partial');
  });

  it('uses explicit current policy settings and latest revisions only', async () => {
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
    const report = await collectRuntimeDailyReport(client);
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

  it('filters sessions to anchored half-open UTC and identifies explicit failures only', async () => {
    const client = reportClient();
    const rows = [
      '2026-09-06T12:00:00Z',
      '2026-09-07T12:00:00Z',
      undefined,
      'bad',
      '2026-09-06T11:59:59Z',
      '2026-09-07T11:00:00Z',
    ].map((time, i) =>
      reportSession({
        session_id: String(i),
        last_session_activity: time,
        violation_status: i === 0 ? 'FAILED' : 'new-state',
      }),
    );
    client.dashboard.sessionsOverview.mockResolvedValue(page(rows));
    const report = await collectRuntimeDailyReport(client, { now: reportClock });
    expect(report.sessions).toMatchObject({ entries: 2, missingTimestamps: 2, outsideWindow: 2 });
    expect(report.findings.some((f) => f.title === 'Session processing failures observed')).toBe(
      true,
    );
  });

  it('retains daily ranking/severity counts without fabricating distinct sessions or tokens', async () => {
    const client = reportClient();
    client.dashboard.topApplicationsViolations.mockResolvedValue({
      applications: [
        {
          id: 'private',
          name: 'Priority',
          total_violations: 12,
          policy_violations: [{ detection_type: 'new-detector', total: 12 }],
        },
      ],
    });
    client.dashboard.applicationsViolationsTrend.mockResolvedValue({
      violations: [
        {
          bucket_number: 0,
          date: 'nanosecond timestamp',
          violation_breakdown: { ...noViolations, high: 4, total: 12 },
        },
      ],
    });
    const report = await collectRuntimeDailyReport(client);
    expect(report.dailyTelemetry.topApplications[0]).toMatchObject({
      name: 'Priority',
      violations: 12,
    });
    expect(report.dailyTelemetry.violationTrend[0].violations.high).toBe(4);
    expect(
      report.findings.some((f) => f.title === 'High or critical detector violations observed'),
    ).toBe(true);
  });

  it.each([
    -1,
    0,
    1.5,
    101,
    NaN,
    Infinity,
  ])('validates budgets before requests: %s', async (maxPages) => {
    const client = reportClient();
    await expect(collectRuntimeDailyReport(client, { maxPages })).rejects.toThrow('maxPages');
    expect(client.profiles.list).not.toHaveBeenCalled();
  });
});
