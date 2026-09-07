import type { DashboardSessionOverviewItem, SecurityProfile } from '@cdot65/prisma-airs-sdk';
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

export const noViolations = { critical: 0, high: 0, medium: 0, low: 0, total: 0 };

export function reportSession(
  overrides: Partial<DashboardSessionOverviewItem> = {},
): DashboardSessionOverviewItem {
  return {
    session_id: 'private-session',
    application_id: 'app-1',
    application_name: 'Assistant',
    last_session_activity: '2026-09-07T11:00:00Z',
    violation_status: 'passed',
    violation_breakdown: noViolations,
    detection_type_violation_breakdown: [],
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
      sessionsOverview: vi.fn().mockResolvedValue({
        items: [reportSession()],
        pagination: { limit: 25, skip: 0, total_items: 1 },
      }),
      sessionsChart: vi.fn().mockResolvedValue({
        bucket_size_seconds: 8640,
        buckets: [
          {
            bucket_number: 0,
            time: REPORT_NOW,
            total: 1,
            violating: 0,
            violation_breakdown: noViolations,
            detection_type_violation_breakdown: [],
          },
        ],
      }),
      topApplicationsViolations: vi.fn().mockResolvedValue({ applications: [] }),
      applicationsViolationsTrend: vi.fn().mockResolvedValue({
        violations: [{ bucket_number: 0, date: REPORT_NOW, violation_breakdown: noViolations }],
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
  };
}
