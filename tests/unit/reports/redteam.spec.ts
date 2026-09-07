import { describe, expect, it } from 'vitest';
import { collectRedTeamEnvironmentReport } from '../../../src/reports/redteam.js';
import {
  renderRedTeamReportHtml,
  renderRedTeamReportMarkdown,
} from '../../../src/reports/redteam-render.js';
import { redTeamReportClient } from '../../helpers/redteam-report.js';

const options = { now: () => new Date('2026-09-07T22:00:00Z') };
describe('Red Team environment report', () => {
  it('collects seven reads, preserves independent windows and excludes sensitive fields', async () => {
    const c = redTeamReportClient();
    const r = await collectRedTeamEnvironmentReport(c, options);
    expect(r.sources).toHaveLength(7);
    expect(r.sources.every((s) => s.status === 'complete')).toBe(true);
    expect(r.health).toBe('attention');
    expect(JSON.stringify(r)).not.toContain('PRIVATE');
    expect(r.findings.some((f) => f.title.includes('static quota'))).toBe(false);
    expect(r.tables.find((t) => t.title.includes('daily creation'))?.rows).toContainEqual([
      'Created in preceding 24 hours',
      1,
    ]);
    expect(c.adapters.list).toHaveBeenCalledWith({
      skip: 0,
      limit: 15,
      include_target_count: true,
    });
  });
  it('retains partial records on budget, repeated identity and later page failure', async () => {
    for (const mode of ['budget', 'repeat', 'failure']) {
      const c = redTeamReportClient();
      c.scans.list.mockResolvedValue({
        data: [{ uuid: 'scan', status: 'COMPLETED' }],
        pagination: { total_items: 2 },
      });
      if (mode === 'failure')
        c.scans.list
          .mockResolvedValueOnce({ data: [{ uuid: 'scan' }], pagination: { total_items: 2 } })
          .mockRejectedValueOnce({ statusCode: 403, message: 'PRIVATE' });
      const r = await collectRedTeamEnvironmentReport(c, {
        ...options,
        maxPages: mode === 'budget' ? 1 : 2,
      });
      expect(r.sources.find((s) => s.name === 'Scans')).toMatchObject({
        status: 'partial',
        records: 1,
      });
      expect(JSON.stringify(r)).not.toContain('PRIVATE');
    }
  });
  it('paginates until the reported total without fetching extra pages', async () => {
    const c = redTeamReportClient();
    c.scans.list
      .mockResolvedValueOnce({ data: [{ uuid: 'one' }], pagination: { total_items: 2 } })
      .mockResolvedValueOnce({ data: [{ uuid: 'two' }], pagination: { total_items: 2 } });
    const r = await collectRedTeamEnvironmentReport(c, options);
    expect(c.scans.list).toHaveBeenNthCalledWith(2, { skip: 1, limit: 15 });
    expect(r.sources.find((s) => s.name === 'Scans')).toMatchObject({
      status: 'complete',
      records: 2,
      pages: 2,
    });
  });
  it.each([
    {},
    { data: [], pagination: { total_items: null } },
    { data: [], pagination: { total_items: 1 } },
    { data: [{ uuid: '' }], pagination: { total_items: 1 } },
  ])('does not report invalid inventory as zero: %j', async (body) => {
    const c = redTeamReportClient();
    c.targets.list.mockResolvedValue(body);
    const r = await collectRedTeamEnvironmentReport(c, options);
    expect(r.sources.find((s) => s.name === 'Targets')?.status).toBe('unavailable');
    expect(r.tables.find((t) => t.title === 'Target inventory')).toBeUndefined();
  });
  it('fails closed on missing quota method without using POST', async () => {
    const c = { ...redTeamReportClient(), getQuotaSummary: undefined };
    const r = await collectRedTeamEnvironmentReport(c, options);
    expect(r.sources.find((s) => s.name === 'Quota')).toMatchObject({ status: 'unavailable' });
  });
  it('flags finite exhausted quotas but not unlimited zero allocation', async () => {
    const c = redTeamReportClient();
    const q = await c.getQuotaSummary();
    q.custom.consumed = 50;
    const r = await collectRedTeamEnvironmentReport(c, options);
    expect(r.findings.filter((f) => f.title.includes('quota'))).toHaveLength(1);
  });
  it('does not infer broker outages or equate statistics and inventory counts', async () => {
    const r = await collectRedTeamEnvironmentReport(redTeamReportClient(), options);
    expect(r.findings.find((f) => f.source === 'Network broker')?.priority).toBe('review');
    expect(r.tables.find((t) => t.title === 'Dashboard scan statistics')?.rows).toContainEqual([
      'Scans',
      4,
    ]);
  });
  it('excludes out-of-window timestamps and flags missing/future/ambiguous timestamps', async () => {
    const c = redTeamReportClient();
    c.scans.list.mockResolvedValue({
      data: [
        '2026-09-06T21:59:59Z',
        '2026-09-06T22:00:00Z',
        '2026-09-07T22:00:00Z',
        '2026-09-08T00:00:00Z',
        '2026-09-07T12:00:00',
        null,
      ].map((created_at, i) => ({ uuid: String(i), created_at })),
      pagination: { total_items: 6 },
    });
    const r = await collectRedTeamEnvironmentReport(c, options);
    expect(r.tables.find((t) => t.title.includes('daily creation'))?.rows).toContainEqual([
      'Created in preceding 24 hours',
      2,
    ]);
    expect(r.findings.some((f) => f.title.includes('timestamp gaps'))).toBe(true);
  });
  it('renders both formats with escaped API text, no raw JSON, inline assets and CSP', async () => {
    const r = await collectRedTeamEnvironmentReport(redTeamReportClient(), {
      ...options,
      title: '</script><img src=x onerror=alert(1)>',
    });
    for (const render of [renderRedTeamReportHtml, renderRedTeamReportMarkdown]) {
      const text = render(r);
      expect(text).not.toContain('<img');
      expect(text).not.toContain('PRIVATE');
      expect(text).toContain('Evidence and completeness');
    }
    const html = renderRedTeamReportHtml(r);
    expect(html).toContain('Content-Security-Policy');
    expect(html).toContain('<style>');
    expect(html).toContain('<script>');
    expect(html).not.toMatch(/<script[^>]+src=/);
  });
  it.each([0, 101, 1.5, Number.NaN])('rejects invalid page budget %s', async (maxPages) => {
    await expect(
      collectRedTeamEnvironmentReport(redTeamReportClient(), { maxPages }),
    ).rejects.toThrow();
  });
});
