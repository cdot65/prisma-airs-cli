import type { ScanResultEntry, SecurityProfile } from '@cdot65/prisma-airs-sdk';
import { vi } from 'vitest';

export const REPORT_NOW = '2026-09-07T12:00:00.000Z';
export const reportClock = () => new Date(REPORT_NOW);

export function reportProfile(overrides: Partial<SecurityProfile> = {}): SecurityProfile {
  return {
    profile_name: 'Production guardrail',
    profile_id: 'profile-private-id',
    revision: 4,
    active: true,
    policy: {
      'ai-security-profiles': [
        {
          'model-configuration': {
            'mask-data-in-storage': true,
            latency: { 'inline-timeout-action': 'block' },
          },
        },
      ],
    },
    ...overrides,
  };
}

export function reportLog(overrides: Partial<ScanResultEntry> = {}): ScanResultEntry {
  return {
    csp_id: 'private-csp',
    tsg_id: 'private-tenant',
    scan_id: 'private-scan',
    scan_sub_req_id: 0,
    api_key_name: 'private-key-name',
    app_name: 'Assistant',
    tokens: 50,
    text_records: 1,
    received_ts: '2026-09-07T11:00:00Z',
    action: 'allow',
    verdict: 'benign',
    prompt: 'NEVER-EXPORT-PROMPT',
    response: 'NEVER-EXPORT-RESPONSE',
    user: 'private@example.test',
    ...overrides,
  };
}

export function reportClient() {
  return {
    dashboard: {
      applicationsOverview: vi.fn().mockResolvedValue({
        items: [{ id: 'app-1', name: 'Assistant', sessions_total: 25, sessions_violated: 0 }],
        pagination: { total_items: 1 },
      }),
    },
    profiles: {
      list: vi.fn().mockResolvedValue({ ai_profiles: [reportProfile()], next_offset: 0 }),
    },
    customerApps: {
      list: vi.fn().mockResolvedValue({
        customer_apps: [
          {
            customer_appId: 'app-1',
            tsg_id: 'private-tenant',
            app_name: 'Registered assistant',
            environment: 'production',
            cloud_provider: 'aws',
            model_name: 'Example model',
            api_keys_dp_info: [
              {
                api_key_name: 'private-key-name',
                dp_name: 'private-deployment',
                auth_code: 'NEVER-EXPORT-AUTH-CODE',
              },
            ],
          },
        ],
        next_offset: 0,
      }),
    },
    scanLogs: {
      query: vi.fn().mockResolvedValue({
        scan_result_for_dashboard: { scan_result_entries: [reportLog()], api_calls_count: 9999 },
        total_pages: 1,
        page_number: 1,
      }),
    },
  };
}
