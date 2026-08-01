import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AiGatewayWorkspace, AiGatewayWorkspaceDetail } from '../../../src/airs/types.js';
import { renderWorkspaceDetail, renderWorkspaceList } from '../../../src/cli/renderer/aigateway.js';

const workspace: AiGatewayWorkspace = {
  id: 'ws-uuid-1',
  slug: 'ws-main-a-349e0e',
  name: 'Main',
  icon: null,
  description: null,
  createdAt: '2026-07-01T00:00:00Z',
  lastUpdatedAt: '2026-07-02T00:00:00Z',
  isDefault: true,
  status: 'active',
  scopeName: 'main_airs_workspace_1852583913',
};

const detail: AiGatewayWorkspaceDetail = {
  ...workspace,
  isDefault: false,
  status: null,
  defaults: { metadata: { env: 'production' } },
  usageLimits: [{ type: 'cost', credit_limit: 10000 }],
  rateLimits: [],
  securitySettings: { membersViewLogs: true },
};

let logSpy: ReturnType<typeof vi.spyOn>;

function captured(): string {
  return logSpy.mock.calls.map((c) => c.join(' ')).join('\n');
}

beforeEach(() => {
  logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
});

afterEach(() => {
  logSpy.mockRestore();
});

describe('renderWorkspaceList', () => {
  it('renders json rows with slug, status, and scope', () => {
    renderWorkspaceList([workspace], 'json');
    const rows = JSON.parse(captured());
    expect(rows).toHaveLength(1);
    expect(rows[0].slug).toBe('ws-main-a-349e0e');
    expect(rows[0].status).toBe('active');
    expect(rows[0].scopeName).toBe('main_airs_workspace_1852583913');
  });

  it('renders a null status as unknown in pretty output', () => {
    renderWorkspaceList([{ ...workspace, status: null }], 'pretty');
    expect(captured()).toContain('unknown');
  });

  it('renders csv with a header row', () => {
    renderWorkspaceList([workspace], 'csv');
    const [header] = captured().split('\n');
    expect(header).toContain('Slug');
  });
});

describe('renderWorkspaceDetail', () => {
  it('emits full record as json', () => {
    renderWorkspaceDetail(detail, 'json');
    const parsed = JSON.parse(captured());
    expect(parsed.usageLimits).toEqual([{ type: 'cost', credit_limit: 10000 }]);
  });

  it('renders null status as unknown, never inactive', () => {
    renderWorkspaceDetail(detail, 'pretty');
    const out = captured();
    expect(out).toContain('unknown');
    expect(out.toLowerCase()).not.toContain('inactive');
  });
});
