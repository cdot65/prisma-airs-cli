import { vi } from 'vitest';
import type { GatewayReportClient } from '../../src/reports/aigateway.js';
export function gatewayReportFixtures() {
  const data = {
    total: 0,
    avg: 0,
    p50: null,
    p90: null,
    p99: null,
    total_request_units: 0,
    total_response_units: 0,
    retryTotal: 0,
    fallbackTotal: 0,
    isQuotaExceeded: false,
    records: [],
    trend: [],
    summary: {
      cacheHits: 0,
      avgCacheLatency: null,
      totalRequests: 0,
      cacheSpeedup: 0,
      totalCacheHits: 0,
      hitRate: 0,
      total: 0,
      unique: 0,
      avg: 0,
      errorPercent: 0,
      totalErrors: 0,
      totalUniqueErrorCodes: 0,
    },
    unique_ai_models: [],
    unique_status_codes: [],
    secret: 'PRIVATE-CANARY',
  };
  const telemetry = Object.fromEntries(
    [
      'requests',
      'errors',
      'users',
      'cost',
      'latency',
      'tokens',
      'cacheSummary',
      'cacheHitTrend',
      'userTrends',
      'errorTrends',
      'errorCategoryTrends',
      'groupedErrors',
      'rescuedRetries',
      'feedbackTrend',
      'feedbackWeighted',
      'feedbackScoreDistribution',
      'feedbackModels',
      'filterBoundaries',
      'logs',
    ].map((m) => [m, vi.fn(async () => ({ success: true, data: structuredClone(data) }))]),
  );
  const listed = vi.fn(async () => ({
    total: 1,
    data: [{ id: 'PRIVATE-ID', key: 'PRIVATE-CANARY' }],
  }));
  const client = {
    telemetry,
    workspaces: {
      list: vi.fn(async () => ({
        total: 1,
        data: [
          { id: 'workspace-id', slug: 'workspace-slug', name: 'dev', secret: 'PRIVATE-CANARY' },
        ],
      })),
    },
    configs: { list: listed },
    apiKeys: { listService: listed, listUser: listed },
    organisations: {
      getInfo: vi.fn(async () => ({
        benefits: { limits: { workspaces: -1 } },
        settings: { secret: 'PRIVATE-CANARY' },
      })),
    },
    guardrails: { getCatalog: vi.fn(async () => ({ evals: [{ secret: 'PRIVATE-CANARY' }] })) },
  };
  return { client: client as unknown as GatewayReportClient, telemetry, data, listed };
}
