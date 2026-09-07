import { type AIGatewayClient, AIGatewayTelemetryClient } from '@cdot65/prisma-airs-sdk';
import { load } from 'js-yaml';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setAiGatewayClientFactoryForTest } from '../../src/cli/commands/aigateway/shared.js';
import { buildProgram } from '../../src/cli/program.js';

vi.mock('../../src/config/loader.js', () => ({
  loadConfig: async () => ({ defaultOutput: 'json' }),
}));

describe('published SDK empty latency through the public CLI', () => {
  let restoreFactory: (() => void) | undefined;
  afterEach(() => {
    restoreFactory?.();
    restoreFactory = undefined;
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it.each(['json', 'yaml'])('preserves null aggregates in %s output', async (format) => {
    const response = {
      success: true,
      data: {
        records: [{ x: '2026-09-06T00:00:00.000Z', y: 0, p50: 0, p90: 0, p99: 0 }],
        total: null,
        p50: null,
        p90: null,
        p99: null,
        isQuotaExceeded: false,
      },
    };
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify(response)));
    vi.stubGlobal('fetch', fetchMock);
    const telemetry = new AIGatewayTelemetryClient({
      baseUrl: 'https://contract.invalid',
      tsgId: '1234',
      numRetries: 0,
      auth: { prepare: async (request) => request },
    });
    restoreFactory = setAiGatewayClientFactoryForTest(
      async () => ({ telemetry }) as AIGatewayClient,
    );
    const output = vi.spyOn(console, 'log').mockImplementation(() => undefined);
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const exit = vi.spyOn(process, 'exit').mockImplementation((code) => {
      throw new Error(`Unexpected exit ${code}`);
    });
    await buildProgram().parseAsync([
      'node',
      'airs',
      'aigateway',
      'telemetry',
      'latency',
      '--workspace',
      'ws-dev',
      '--start',
      '2026-09-06T00:00:00Z',
      '--end',
      '2026-09-07T00:00:00Z',
      '--output',
      format,
    ]);
    expect(exit).not.toHaveBeenCalled();
    expect(fetchMock).toHaveBeenCalledOnce();
    expect(new URL(String(fetchMock.mock.calls[0][0])).pathname).toBe('/logs/charts/latency');
    const rendered = output.mock.calls.map((call) => String(call[0])).join('\n');
    expect(format === 'json' ? JSON.parse(rendered) : load(rendered)).toEqual(response);
  });
});
