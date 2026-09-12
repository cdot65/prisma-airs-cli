import { type AIGatewayClient, AIGatewayTelemetryClient } from '@cdot65/prisma-airs-sdk';
import { load } from 'js-yaml';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SdkAiGatewayService } from '../../src/airs/aigateway.js';
import { setAiGatewayClientFactoryForTest } from '../../src/cli/commands/aigateway/shared.js';
import { buildProgram } from '../../src/cli/program.js';

const state = vi.hoisted(() => ({
  client: {} as unknown,
  construct: vi.fn(),
  workspaces: vi.fn(),
}));
vi.mock('@cdot65/prisma-airs-sdk', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@cdot65/prisma-airs-sdk')>()),
  AIGatewayClient: class {
    constructor() {
      state.construct();
      Object.assign(this, state.client);
    }
  },
}));
vi.mock('../../src/config/loader.js', () => ({
  loadConfig: async () => ({
    defaultOutput: 'json',
    mgmtClientId: 'client',
    mgmtClientSecret: 'secret',
    mgmtTsgId: '1234',
  }),
}));

describe('public CLI chart flags through the registry SDK transport', () => {
  let restoreFactory: () => void;
  const fetchMock = vi.fn();
  const auth = vi.fn(async (request) => request);
  beforeEach(() => {
    vi.clearAllMocks();
    state.workspaces.mockResolvedValue({
      data: [{ id: 'workspace-id', slug: 'ws-dev', name: 'Development' }],
    });
    const telemetry = new AIGatewayTelemetryClient({
      baseUrl: 'https://contract.invalid',
      tsgId: '1234',
      numRetries: 0,
      auth: { prepare: auth },
    });
    state.client = { telemetry, workspaces: { list: state.workspaces } };
    restoreFactory = setAiGatewayClientFactoryForTest(async () => {
      state.construct();
      return state.client as AIGatewayClient;
    });
    vi.stubGlobal('fetch', fetchMock);
    vi.spyOn(console, 'log').mockImplementation(() => undefined);
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    vi.spyOn(process, 'exit').mockImplementation((code) => {
      throw new Error(`exit ${code}: ${vi.mocked(console.error).mock.calls.flat().join(' ')}`);
    });
  });
  afterEach(() => {
    restoreFactory();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  const filterArgs = [
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
  function run(metric: string, args: string[], format = 'json') {
    return buildProgram().parseAsync([
      'node',
      'airs',
      'aigateway',
      'telemetry',
      metric,
      '--workspace',
      'Development',
      '--output',
      format,
      ...args,
    ]);
  }
  const response = {
    success: true,
    data: {
      records: [
        {
          x: '2026-09-06T00:00:00.000Z',
          y: 0.125,
          p50: 1,
          p90: 2,
          p99: 3,
          avg: 0.125,
          total_request_units: 20,
          total_response_units: 22,
        },
      ],
      total: 0.125,
      avg: 0.125,
      total_request_units: 20,
      total_response_units: 22,
      p50: 1,
      p90: 2,
      p99: 3,
      isQuotaExceeded: false,
    },
  };

  for (const metric of ['requests', 'cost', 'tokens', 'latency']) {
    it.each([
      'json',
      'yaml',
    ])(`${metric} maps every flag and preserves %s output`, async (format) => {
      const metricResponse =
        metric === 'requests' || metric === 'tokens'
          ? {
              ...response,
              data: {
                ...response.data,
                total: 42,
                records: [{ ...response.data.records[0], y: 42 }],
              },
            }
          : response;
      fetchMock.mockResolvedValue(new Response(JSON.stringify(metricResponse)));
      await run(metric, filterArgs, format);
      expect(process.exit).not.toHaveBeenCalled();
      expect(fetchMock).toHaveBeenCalledOnce();
      const url = new URL(String(fetchMock.mock.calls[0][0]));
      expect(url.pathname).toBe(`/logs/charts/${metric}`);
      for (const [key, value] of Object.entries({
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
      const output = vi
        .mocked(console.log)
        .mock.calls.map((call) => String(call[0]))
        .join('\n');
      const parsed = format === 'json' ? JSON.parse(output) : load(output);
      if (metric === 'cost') {
        expect(state.workspaces).toHaveBeenCalledOnce();
        expect(parsed).toEqual({
          workspaceSlug: 'ws-dev',
          days: 7,
          totalCents: 0.125,
          totalUsd: 0.00125,
          avgCents: 0.125,
          avgUsd: 0.00125,
          quotaExceeded: false,
          records: [{ date: '2026-09-06T00:00:00.000Z', costCents: 0.125, costUsd: 0.00125 }],
        });
      } else {
        expect(state.workspaces).not.toHaveBeenCalled();
        expect(parsed).toEqual(metricResponse);
      }
    });
    it.each([
      ['--cost-min', '1', '--cost-max', '0'],
      ['--status-codes', '200,'],
      ['--metadata', '{"sensitive":{"secret":"do-not-echo"}}'],
      ['--api-key-ids', 'not-a-uuid'],
      ['--ai-org-models', '@openai/gpt-5.6-terra'],
      ['--total-units-min', '1.5'],
      ['--cost-min', 'NaN'],
      ['--days', '7junk'],
      ['--days', '0'],
      ['--days', '9007199254740992'],
    ])(`${metric} rejects malformed flags before client creation: %j`, async (...args) => {
      await expect(run(metric, args)).rejects.toThrow('exit 2');
      expect(state.construct).not.toHaveBeenCalled();
      expect(state.workspaces).not.toHaveBeenCalled();
      expect(auth).not.toHaveBeenCalled();
      expect(fetchMock).not.toHaveBeenCalled();
      expect(console.log).not.toHaveBeenCalled();
      expect(vi.mocked(console.error).mock.calls.flat().join(' ')).not.toContain('do-not-echo');
    });
  }

  it('resolves a cost workspace UUID to the required slug without rescaling query bounds', async () => {
    fetchMock.mockResolvedValue(new Response(JSON.stringify(response)));
    const service = new SdkAiGatewayService();
    const report = await service.getTelemetryCost({
      workspaceSlug: 'workspace-id',
      costMax: 0.125,
    });
    expect(report.workspaceSlug).toBe('ws-dev');
    const url = new URL(String(fetchMock.mock.calls[0][0]));
    expect(url.searchParams.get('workspaceSlug')).toBe('ws-dev');
    expect(url.searchParams.get('costMax')).toBe('0.125');
  });

  it.each([
    { costMin: 2, costMax: 1 },
    { days: 0 },
    { days: 1.5 },
    { days: Number.MAX_SAFE_INTEGER + 1 },
  ])('library validates before workspace lookup: %j', async (invalid) => {
    const service = new SdkAiGatewayService();
    await expect(
      service.getTelemetryCost({ workspaceSlug: 'Development', ...invalid }),
    ).rejects.toThrow();
    expect(state.workspaces).not.toHaveBeenCalled();
    expect(auth).not.toHaveBeenCalled();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
