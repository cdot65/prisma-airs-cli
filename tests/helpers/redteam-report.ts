import { vi } from 'vitest';
export function redTeamReportClient() {
  return {
    getDashboardOverview: vi.fn().mockResolvedValue({ total_targets: 1 }),
    getScanStatistics: vi.fn().mockResolvedValue({
      total_scans: 4,
      targets_scanned: 1,
      risk_profile: [{ risk_rating: 'HIGH', total: 1 }],
      scan_status: [{ name: 'COMPLETED', count: 4 }],
    }),
    getQuotaSummary: vi.fn().mockResolvedValue({
      static: { allocated: 0, consumed: 0, unlimited: true },
      dynamic: { allocated: 50, consumed: 0, unlimited: false },
      custom: { allocated: 50, consumed: 5, unlimited: false },
    }),
    targets: {
      list: vi.fn().mockResolvedValue({
        data: [
          {
            uuid: 'target',
            target_type: 'APPLICATION',
            connection_params: { password: 'PRIVATE-CONNECTION' },
          },
        ],
        pagination: { total_items: 1 },
      }),
    },
    scans: {
      list: vi.fn().mockResolvedValue({
        data: [
          {
            uuid: 'scan',
            created_at: '2026-09-07T12:00:00Z',
            status: 'FAILED',
            name: 'PRIVATE-SCAN',
            job_metadata: { prompt: 'PRIVATE-PROMPT' },
          },
        ],
        pagination: { total_items: 1 },
      }),
    },
    adapters: {
      list: vi.fn().mockResolvedValue({
        data: [{ uuid: 'adapter', status: 'ACTIVE', script_b64: 'PRIVATE-SCRIPT' }],
        pagination: { total_items: 1 },
      }),
    },
    networkBroker: {
      getChannelStats: vi.fn().mockResolvedValue({ total_channels: 7, online_channels: 1 }),
    },
  };
}
