import { describe, expect, it } from 'vitest';
import { collectAgentGuardReport } from '../../../src/reports/agentguard.js';
import {
  renderEnvironmentReportHtml,
  renderEnvironmentReportMarkdown,
} from '../../../src/reports/environment-render.js';
import { client, scan } from '../../helpers/agentguard.js';

const options = { now: () => new Date('2026-09-08T12:00:00Z') };
describe('AgentGuard report', () => {
  it('collects all three sources, reports findings and excludes sensitive records', async () => {
    const c = client();
    const r = await collectAgentGuardReport(c, options);
    expect(r.health).toBe('attention');
    expect(r.sources.every((s) => s.status === 'complete')).toBe(true);
    expect(c.listScans).toHaveBeenCalledWith({
      skip: 0,
      limit: 10,
      start_time: '2026-08-09T12:00:00.000Z',
      end_time: '2026-09-08T12:00:00.000Z',
      isBackgroundRefresh: false,
    });
    expect(c.listScanVulnerabilities).not.toHaveBeenCalled();
    expect(JSON.stringify(r)).not.toContain('PRIVATE-');
    expect(renderEnvironmentReportHtml(r)).toMatch(/^<!doctype html>/);
    expect(renderEnvironmentReportMarkdown(r)).toContain('Skill statistics');
    expect(r.tables[1].rows[0][2]).toBeNull();
  });
  it('walks pages based on total and actual rows returned', async () => {
    const c = client();
    c.listScans
      .mockResolvedValueOnce({ scans: [scan('first')], pagination: { total_items: 2 } })
      .mockResolvedValueOnce({ scans: [scan('second')], pagination: { total_items: 2 } });
    const r = await collectAgentGuardReport(c, options);
    expect(c.listScans.mock.calls[1][0].skip).toBe(1);
    expect(r.sources[0]).toMatchObject({ records: 2, pages: 2, status: 'complete' });
  });
  it.each([
    'repeated',
    'empty',
    'changed',
    'failed',
    'oversized',
  ])('marks %s pagination as partial', async (mode) => {
    const c = client();
    c.listScans.mockResolvedValueOnce({ scans: [scan()], pagination: { total_items: 2 } });
    if (mode === 'failed') c.listScans.mockRejectedValue(new Error('PRIVATE-ERROR'));
    else
      c.listScans.mockResolvedValue({
        scans:
          mode === 'empty'
            ? []
            : mode === 'oversized'
              ? Array.from({ length: 11 }, (_, i) => scan(String(i)))
              : [scan()],
        pagination: { total_items: mode === 'changed' ? 3 : 2 },
      });
    const r = await collectAgentGuardReport(c, options);
    expect(r.sources[0].status).toBe('partial');
    expect(JSON.stringify(r)).not.toContain('PRIVATE-');
  });
  it('honors budgets, excludes parent rows and flags incomplete decisions', async () => {
    const c = client();
    c.listScans.mockResolvedValue({
      scans: [
        { ...scan('parent'), is_batch: true },
        { ...scan('child'), parent_uuid: 'parent', eval_outcome: 'PENDING', status: 'RUNNING' },
      ],
      pagination: { total_items: 3 },
    });
    const r = await collectAgentGuardReport(c, { ...options, maxPages: 1 });
    expect(r.sources[0].status).toBe('partial');
    expect(r.tables[0].rows).toEqual([['SKILL / PENDING', 1]]);
    expect(r.findings.some((f) => f.title.includes('completed decision'))).toBe(true);
  });
  it('never converts source failures to zero or leaks upstream errors', async () => {
    const c = client();
    c.listScans.mockRejectedValue(new Error('PRIVATE-ERROR'));
    c.listRules.mockRejectedValue(new Error('PRIVATE-ERROR'));
    c.getScanStats.mockRejectedValue(new Error('PRIVATE-ERROR'));
    const r = await collectAgentGuardReport(c, options);
    expect(r.health).toBe('unknown');
    expect(r.tables).toEqual([]);
    expect(JSON.stringify(r)).not.toContain('PRIVATE-');
  });
  it('excludes timestamps outside the requested window and unfamiliar category text', async () => {
    const c = client();
    c.listScans.mockResolvedValue({
      scans: [
        { ...scan('old'), created_at: 'invalid' },
        { ...scan('new'), artifact_type: 'PRIVATE-NEW', eval_outcome: 'PRIVATE-NEW' },
      ],
      pagination: { total_items: 2 },
    });
    const r = await collectAgentGuardReport(c, options);
    expect(r.sources[0].status).toBe('partial');
    expect(r.tables[0].rows).toEqual([['OTHER / OTHER', 1]]);
    expect(JSON.stringify(r)).not.toContain('PRIVATE-');
  });
  it('distinguishes valid empty data from unavailable data', async () => {
    const c = client();
    c.listScans.mockResolvedValue({ scans: [], pagination: { total_items: 0 } });
    c.listRules.mockResolvedValue({ rules: [], pagination: { total_items: 0 } });
    c.getScanStats.mockResolvedValue({
      unique_skills_scanned: { count: 0, percent_change: null },
      total_vulnerabilities_found: { count: 0, percent_change: null },
      top_vulnerability: null,
    });
    const r = await collectAgentGuardReport(c, options);
    expect(r.health).toBe('no-findings');
    expect(r.sources.every((s) => s.status === 'complete')).toBe(true);
  });

  it('does not truncate the rule catalog at a page-count total', async () => {
    const c = client();
    c.listRules
      .mockResolvedValueOnce({
        rules: Array.from({ length: 10 }, (_, i) => ({
          uuid: String(i),
          default_state: 'BLOCKING',
        })),
        pagination: { total_items: 10 },
      })
      .mockResolvedValueOnce({
        rules: [{ uuid: 'last', default_state: 'ALLOWING' }],
        pagination: { total_items: 1 },
      });
    const r = await collectAgentGuardReport(c, options);
    expect(r.sources[2]).toMatchObject({ records: 11, pages: 2, status: 'complete' });
  });
  it.each([0, -1, 101, 1.5, NaN])('rejects invalid budget %s before I/O', async (maxPages) => {
    const c = client();
    await expect(collectAgentGuardReport(c, { maxPages })).rejects.toThrow();
    expect(c.listScans).not.toHaveBeenCalled();
  });
});
