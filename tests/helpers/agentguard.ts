import type { AgentGuardScan } from '@cdot65/prisma-airs-sdk';
import { vi } from 'vitest';

export const scan = (uuid = '550e8400-e29b-41d4-a716-446655440000'): AgentGuardScan => ({
  uuid,
  tsg_id: '123',
  created_at: '2026-09-08T10:00:00Z',
  updated_at: '2026-09-08T10:00:00Z',
  name: 'PRIVATE-NAME',
  git_url: 'PRIVATE-URL',
  artifact_type: 'SKILL',
  status: 'COMPLETED',
  error_message: null,
  summary: { vulnerability_count: 2, attack_chain_count: 0 },
  eval_outcome: 'BLOCKED',
  eval_summary: null,
  eval_details: null,
  duration_seconds: 1,
  fingerprint: 'PRIVATE-FINGERPRINT',
  scanner_version: null,
  device_metadata: { token: 'PRIVATE-TOKEN' },
  parent_uuid: null,
  is_batch: false,
  child_scan_uuids: [],
  batch_summary: null,
});
export const client = () => ({
  listScans: vi.fn().mockResolvedValue({ scans: [scan()], pagination: { total_items: 1 } }),
  getScanStats: vi.fn().mockResolvedValue({
    unique_skills_scanned: { count: 1, percent_change: null },
    total_vulnerabilities_found: { count: 2, percent_change: null },
    top_vulnerability: null,
  }),
  listRules: vi.fn().mockResolvedValue({
    rules: [
      {
        uuid: 'rule',
        name: 'PRIVATE-RULE',
        default_state: 'BLOCKING',
        vulnerability_types: ['SECRET_EXPOSURE'],
        description: 'PRIVATE-DESCRIPTION',
        remediation: 'PRIVATE-REMEDIATION',
      },
    ],
    pagination: { total_items: 1 },
  }),
  listScanVulnerabilities: vi.fn().mockResolvedValue({
    vulnerabilities: [
      {
        uuid: 'finding',
        scan_uuid: 'scan',
        vulnerability_type: 'SECRET_EXPOSURE',
        attack_chain_count: 0,
        original_code: 'PRIVATE-CODE',
        file_path: 'PRIVATE-PATH',
        description: 'PRIVATE-DESCRIPTION',
      },
    ],
    pagination: { total_items: 1 },
  }),
});
