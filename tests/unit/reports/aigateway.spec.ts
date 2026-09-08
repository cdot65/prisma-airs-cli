import { describe, expect, it } from 'vitest';
import { collectGatewayEnvironmentReport } from '../../../src/reports/aigateway.js';
import {
  renderEnvironmentReportHtml,
  renderEnvironmentReportMarkdown,
} from '../../../src/reports/environment-render.js';
import { gatewayReportFixtures as fixtures } from '../../helpers/aigateway-report.js';

const options = {
  workspace: 'dev',
  tsgId: '123',
  start: new Date('2026-09-01T00:00:00Z'),
  end: new Date('2026-09-02T00:00:00Z'),
  now: () => new Date('2026-09-03T00:00:00Z'),
};

const row = (id: string) => ({
  id,
  workspace_slug: 'workspace-slug',
  created_at: '2026-09-01T12:00:00Z',
  response_status_code: 200,
  metadataValue: ['PRIVATE-CANARY'],
});

describe('AI Gateway aggregate report', () => {
  it('highlights HTTP 2xx error classifications without changing server totals', async () => {
    const { client, telemetry, data } = fixtures();
    telemetry.errorCategoryTrends.mockResolvedValue({
      success: true,
      data: { ...data, trend: [{ response_status_code: 200, count: 2 }] },
    } as never);
    const report = await collectGatewayEnvironmentReport(client, options);
    expect(report.findings.some((f) => f.title.includes('Success-status'))).toBe(true);
    expect(report.tables.find((t) => t.title === 'Error categories')?.rows).toContainEqual([
      200, 2,
    ]);
  });
  it('distinguishes expected policy blocks from server outages', async () => {
    const { client, telemetry, data } = fixtures();
    telemetry.errorCategoryTrends.mockResolvedValue({
      success: true,
      data: { ...data, trend: [{ response_status_code: 446, count: 2 }] },
    } as never);
    const report = await collectGatewayEnvironmentReport(client, options);
    expect(report.health).toBe('review');
    expect(report.findings[0].title).toBe('Gateway errors or policy blocks observed');
  });
  it.each([-1, 0.5, Infinity, undefined])('rejects invalid request count %s', async (total) => {
    const { client, telemetry, data } = fixtures();
    telemetry.requests.mockResolvedValue({ success: true, data: { ...data, total } } as never);
    const report = await collectGatewayEnvironmentReport(client, options);
    expect(report.sources.find((s) => s.name === 'telemetry.requests')?.status).toBe('unavailable');
  });
  it('preserves signed feedback and fractional cost', async () => {
    const { client, telemetry, data } = fixtures();
    telemetry.feedbackWeighted.mockResolvedValue({ success: true, data: { ...data, total: -2.5 } });
    telemetry.cost.mockResolvedValue({ success: true, data: { ...data, total: 0.031 } });
    const report = await collectGatewayEnvironmentReport(client, options);
    expect(report.sources.every((s) => s.status === 'complete')).toBe(true);
    expect(report.tables[0].rows).toContainEqual(['feedbackWeighted', 'total', -2.5]);
    expect(report.tables[0].rows).toContainEqual(['cost', 'total', 0.031]);
  });
  it('does not fetch a different workspace when the requested name is absent', async () => {
    const { client, telemetry } = fixtures();
    const report = await collectGatewayEnvironmentReport(client, {
      ...options,
      workspace: 'missing',
    });
    expect(report.health).toBe('unknown');
    expect(telemetry.logs).not.toHaveBeenCalled();
    expect(client.configs.list).not.toHaveBeenCalled();
  });
  it('collects all 25 sources with one fixed window and no sensitive fields', async () => {
    const { client, telemetry } = fixtures();
    const report = await collectGatewayEnvironmentReport(client, options);
    expect(report.sources).toHaveLength(25);
    expect(report.sources.every((s) => s.status === 'complete')).toBe(true);
    expect(report.health).toBe('no-findings');
    for (const method of Object.values(telemetry))
      expect(method).toHaveBeenCalledWith(
        expect.objectContaining({
          workspaceSlug: 'workspace-slug',
          start: options.start,
          end: options.end,
        }),
      );
    for (const text of [
      JSON.stringify(report),
      renderEnvironmentReportHtml(report),
      renderEnvironmentReportMarkdown(report),
    ]) {
      expect(text).not.toContain('PRIVATE');
      expect(text).not.toContain('workspace-id');
    }
    expect(renderEnvironmentReportMarkdown(report)).toContain('Telemetry window &#40;UTC&#41;');
    expect(renderEnvironmentReportHtml(report)).toContain('connect-src &#39;none&#39;');
  });
  it.each([0, -1, 101, 1.5, NaN])('rejects invalid budget %s before I/O', async (maxPages) => {
    const { client } = fixtures();
    await expect(
      collectGatewayEnvironmentReport(client, { ...options, maxPages }),
    ).rejects.toThrow();
    expect(client.workspaces.list).not.toHaveBeenCalled();
  });
  it('paginates zero-based pages and counts unique transactions only', async () => {
    const { client, telemetry, data } = fixtures();
    telemetry.logs
      .mockResolvedValueOnce({
        success: true,
        data: { ...data, total: 2, records: [row('a')] },
      } as never)
      .mockResolvedValueOnce({
        success: true,
        data: { ...data, total: 2, records: [row('b')] },
      } as never);
    const r = await collectGatewayEnvironmentReport(client, options);
    expect(r.sources.find((s) => s.name === 'telemetry.logs')).toMatchObject({
      status: 'complete',
      records: 2,
      pages: 2,
    });
    expect(
      telemetry.logs.mock.calls.map(
        (c) => (c as unknown as [{ currentPage: number }])[0].currentPage,
      ),
    ).toEqual([0, 1]);
  });
  it.each([
    'duplicate',
    'empty',
    'changed-total',
    'wrong-workspace',
    'outside-window',
  ])('marks %s pagination partial', async (problem) => {
    const { client, telemetry, data } = fixtures();
    telemetry.logs.mockResolvedValueOnce({
      success: true,
      data: { ...data, total: 2, records: [row('a')] },
    } as never);
    const second =
      problem === 'duplicate'
        ? row('a')
        : problem === 'wrong-workspace'
          ? { ...row('b'), workspace_slug: 'other' }
          : problem === 'outside-window'
            ? { ...row('b'), created_at: '2000-01-01' }
            : row('b');
    telemetry.logs.mockResolvedValueOnce({
      success: true,
      data: {
        ...data,
        total: problem === 'changed-total' ? 3 : 2,
        records: problem === 'empty' ? [] : [second],
      },
    } as never);
    const r = await collectGatewayEnvironmentReport(client, options);
    expect(r.sources.find((s) => s.name === 'telemetry.logs')?.status).toBe('partial');
  });
  it('marks budget truncation and inventory truncation partial', async () => {
    const { client, telemetry, data, listed } = fixtures();
    telemetry.logs.mockResolvedValue({
      success: true,
      data: { ...data, total: 2, records: [row('a')] },
    } as never);
    listed.mockResolvedValue({ total: 3, has_more: true, data: [] } as never);
    const r = await collectGatewayEnvironmentReport(client, { ...options, maxPages: 1 });
    expect(r.sources.filter((s) => s.status === 'partial')).toHaveLength(4);
  });
  it.each([
    400, 401, 403, 429, 500,
  ])('keeps HTTP %s failure evidence without error-body leaks', async (statusCode) => {
    const { client, telemetry } = fixtures();
    telemetry.errors.mockRejectedValue({ statusCode, message: 'PRIVATE-CANARY' });
    const r = await collectGatewayEnvironmentReport(client, options);
    expect(r.sources.find((s) => s.name === 'telemetry.errors')).toMatchObject({
      status: 'unavailable',
      records: 0,
    });
    expect(JSON.stringify(r)).toContain(`HTTP ${statusCode}`);
    expect(JSON.stringify(r)).not.toContain('PRIVATE');
  });
  it('flags quota limits and missing metrics without replacing them with zero', async () => {
    const { client, telemetry, data } = fixtures();
    telemetry.errors.mockResolvedValue({
      success: true,
      data: { ...data, isQuotaExceeded: true, total: null },
    });
    telemetry.cost.mockResolvedValue({
      success: true,
      data: { ...data, total: undefined },
    } as never);
    const r = await collectGatewayEnvironmentReport(client, options);
    expect(r.sources.find((s) => s.name === 'telemetry.errors')?.status).toBe('partial');
    expect(r.sources.find((s) => s.name === 'telemetry.cost')?.status).toBe('unavailable');
  });
  it('rejects success false and returns unknown without available telemetry', async () => {
    const { client, telemetry, data } = fixtures();
    for (const m of Object.values(telemetry)) m.mockResolvedValue({ success: false, data });
    const r = await collectGatewayEnvironmentReport(client, options);
    expect(r.health).toBe('unknown');
    expect(r.sources.filter((s) => s.status === 'unavailable')).toHaveLength(19);
  });
  it('flags server errors, formats UTC trends, and escapes report titles', async () => {
    const { client, telemetry, data } = fixtures();
    telemetry.errorCategoryTrends.mockResolvedValue({
      success: true,
      data: { ...data, trend: [{ response_status_code: 503, count: 2 }] },
    } as never);
    telemetry.groupedErrors.mockResolvedValue({
      success: true,
      data: {
        ...data,
        trend: [{ x: '2026-09-01T01:00:00Z', y: [{ response_status_code: 503, count: 2 }] }],
      },
    } as never);
    const r = await collectGatewayEnvironmentReport(client, {
      ...options,
      title: '<script>alert(1)</script>',
    });
    expect(r.health).toBe('attention');
    expect(renderEnvironmentReportHtml(r)).not.toContain('<script>alert');
    expect(renderEnvironmentReportMarkdown(r)).not.toContain('<script>');
    expect(r.tables.find((t) => t.title === 'Error timeline')?.rows).toEqual([
      ['2026-09-01T01:00:00.000Z', 503, 2],
    ]);
  });
});
