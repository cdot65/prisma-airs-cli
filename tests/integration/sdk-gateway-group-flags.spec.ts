import { type AIGatewayClient, AIGatewayTelemetryClient } from '@cdot65/prisma-airs-sdk';
import { load } from 'js-yaml';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setAiGatewayClientFactoryForTest } from '../../src/cli/commands/aigateway/shared.js';
import { buildProgram } from '../../src/cli/program.js';

vi.mock('../../src/config/loader.js', () => ({
  loadConfig: async () => ({ defaultOutput: 'json' }),
}));

describe('public CLI group flags through the published SDK transport', () => {
  let restoreFactory: () => void;
  const fetchMock = vi.fn();
  const construct = vi.fn();
  const auth = vi.fn(async (request) => request);
  beforeEach(() => {
    vi.clearAllMocks();
    const telemetry = new AIGatewayTelemetryClient({
      baseUrl: 'https://contract.invalid',
      tsgId: '1234',
      numRetries: 0,
      auth: { prepare: auth },
    });
    restoreFactory = setAiGatewayClientFactoryForTest(async () => {
      construct();
      return { telemetry } as AIGatewayClient;
    });
    vi.stubGlobal('fetch', fetchMock);
    vi.spyOn(console, 'log').mockImplementation(() => undefined);
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    vi.spyOn(process, 'exit').mockImplementation((code) => {
      throw new Error(`exit ${code}`);
    });
  });
  afterEach(() => {
    restoreFactory();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  function run(dimension: string, args: string[], format = 'json') {
    return buildProgram()
      .exitOverride()
      .parseAsync([
        'node',
        'airs',
        'aigateway',
        'telemetry',
        'group-by',
        dimension,
        '--workspace',
        'ws-dev',
        '--output',
        format,
        '--start',
        '2026-09-06T00:00:00Z',
        '--end',
        '2026-09-07T00:00:00Z',
        ...args,
      ]);
  }
  const filters = [
    '--trace-id',
    'owned',
    '--metadata',
    '{"environment":"dev"}',
    '--status-codes',
    '200,446',
    '--api-key-ids',
    '11111111-1111-4111-8111-111111111111',
    '--ai-org-models',
    'openai__gpt-5.6-terra',
    '--total-units-min',
    '0',
    '--total-units-max',
    '42',
    '--cost-min',
    '0',
    '--cost-max',
    '0.125',
  ];
  const groupResponse = {
    object: 'list',
    is_quota_exceeded: false,
    total: 1,
    data: [{ object: 'model', model: 'gpt-5.6-terra', requests: 1, cost: 0.125, total_tokens: 42 }],
  };
  const userResponse = {
    success: true,
    data: { records: [{ _user: '', count: 1, cost: 0.125 }], total: 1, isQuotaExceeded: false },
  };
  for (const dimension of ['ai_service', 'model', 'api_key', 'provider', 'status_code', 'users']) {
    it.each([
      'json',
      'yaml',
    ])(`${dimension} preserves filters and the %s envelope`, async (format) => {
      const response = dimension === 'users' ? userResponse : groupResponse;
      fetchMock.mockResolvedValue(new Response(JSON.stringify(response)));
      await run(dimension, filters, format);
      expect(process.exit).not.toHaveBeenCalled();
      expect(fetchMock).toHaveBeenCalledOnce();
      const [input, init] = fetchMock.mock.calls[0];
      const url = new URL(String(input));
      expect(init.method).toBe('GET');
      expect(url.pathname).toBe(`/logs/groups/${dimension}`);
      for (const [key, value] of Object.entries({
        workspaceSlug: 'ws-dev',
        traceId: 'owned',
        metadata: '{"environment":"dev"}',
        statusCode: '200,446',
        apiKeyIds: '11111111-1111-4111-8111-111111111111',
        aiOrgModel: 'openai__gpt-5.6-terra',
        totalUnitsMin: '0',
        totalUnitsMax: '42',
        costMin: '0',
        costMax: '0.125',
      }))
        expect(url.searchParams.get(key)).toBe(value);
      expect(url.searchParams.has('columns')).toBe(false);
      const output = vi
        .mocked(console.log)
        .mock.calls.map((call) => String(call[0]))
        .join('\n');
      expect(format === 'json' ? JSON.parse(output) : load(output)).toEqual(response);
      expect(console.error).not.toHaveBeenCalled();
    });
    it(`${dimension} retains zero/equal bounds`, async () => {
      fetchMock.mockResolvedValue(
        new Response(JSON.stringify(dimension === 'users' ? userResponse : groupResponse)),
      );
      await run(dimension, [
        '--cost-min',
        '0',
        '--cost-max',
        '0',
        '--total-units-min',
        '0',
        '--total-units-max',
        '0',
      ]);
      const url = new URL(String(fetchMock.mock.calls[0][0]));
      for (const field of ['costMin', 'costMax', 'totalUnitsMin', 'totalUnitsMax']) {
        expect(url.searchParams.get(field)).toBe('0');
      }
    });
    if (dimension !== 'users') {
      it(`${dimension} retains supported columns alongside filters`, async () => {
        fetchMock.mockResolvedValue(new Response(JSON.stringify(groupResponse)));
        await run(dimension, [...filters, '--columns', 'cost, total_tokens']);
        const url = new URL(String(fetchMock.mock.calls[0][0]));
        expect(url.searchParams.get('columns')).toBe('cost,total_tokens');
        expect(url.searchParams.get('traceId')).toBe('owned');
      });
    }
    it.each([
      ['--cost-min', '1', '--cost-max', '0'],
      ['--status-codes', '200,'],
      ['--metadata', '{"sensitive":{"secret":"do-not-echo"}}'],
      ['--api-key-ids', 'not-a-uuid'],
      ['--ai-org-models', '@openai/gpt-5.6-terra'],
      ['--total-units-min', '1.5'],
      ['--cost-min', 'NaN'],
      ['--columns', ''],
      ['--columns', 'cost,'],
      ['--columns', 'invented'],
      ['--start', 'not-a-date'],
    ])(`${dimension} rejects invalid input before client creation: %j`, async (...args) => {
      await expect(run(dimension, args)).rejects.toThrow('exit 2');
      expect(construct).not.toHaveBeenCalled();
      expect(auth).not.toHaveBeenCalled();
      expect(fetchMock).not.toHaveBeenCalled();
      expect(console.log).not.toHaveBeenCalled();
      expect(vi.mocked(console.error).mock.calls.flat().join(' ')).not.toContain('do-not-echo');
    });
  }
  it.each([
    'cost',
    'cost,total_tokens',
  ])('rejects unsupported user columns: %s', async (columns) => {
    await expect(run('users', ['--columns', columns])).rejects.toThrow('exit 2');
    expect(construct).not.toHaveBeenCalled();
    expect(auth).not.toHaveBeenCalled();
    expect(fetchMock).not.toHaveBeenCalled();
  });
  it.each([
    'status',
    'user',
    'api-key',
    'provider/../../secret',
  ])('rejects invalid dimensions before client creation: %s', async (dimension) => {
    await expect(run(dimension, [])).rejects.toThrow('exit 2');
    expect(construct).not.toHaveBeenCalled();
    expect(auth).not.toHaveBeenCalled();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
