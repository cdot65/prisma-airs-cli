import { beforeEach, describe, expect, it, vi } from 'vitest';
import { aiGatewayGrantHint, SdkAiGatewayService } from '../../../src/airs/aigateway.js';

const mockWorkspacesList = vi.fn();
const mockWorkspacesGet = vi.fn();

function makeMockClient() {
  return {
    workspaces: {
      list: mockWorkspacesList,
      get: mockWorkspacesGet,
    },
  };
}

vi.mock('@cdot65/prisma-airs-sdk', () => ({
  AIGatewayClient: vi.fn().mockImplementation(() => makeMockClient()),
}));

const listRow = {
  id: 'ws-uuid-1',
  slug: 'ws-main-a-349e0e',
  name: 'Main',
  icon: null,
  description: null,
  created_at: '2026-07-01T00:00:00Z',
  last_updated_at: '2026-07-02T00:00:00Z',
  is_default: 1,
  status: 'active',
  scope_name: 'main_airs_workspace_1852583913',
  object: 'workspace',
};

describe('SdkAiGatewayService', () => {
  let service: SdkAiGatewayService;

  beforeEach(() => {
    vi.clearAllMocks();
    service = new SdkAiGatewayService();
  });

  describe('listWorkspaces', () => {
    it('normalizes list rows to camelCase with boolean isDefault', async () => {
      mockWorkspacesList.mockResolvedValue({ data: [listRow] });
      const workspaces = await service.listWorkspaces();
      expect(mockWorkspacesList).toHaveBeenCalledWith(undefined);
      expect(workspaces).toEqual([
        {
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
        },
      ]);
    });

    it('passes plane and status through to the SDK', async () => {
      mockWorkspacesList.mockResolvedValue({ data: [] });
      await service.listWorkspaces({ plane: 'admin', status: 'archived' });
      expect(mockWorkspacesList).toHaveBeenCalledWith({ plane: 'admin', status: 'archived' });
    });

    it('normalizes is_default 0 to false', async () => {
      mockWorkspacesList.mockResolvedValue({ data: [{ ...listRow, is_default: 0 }] });
      const [ws] = await service.listWorkspaces();
      expect(ws.isDefault).toBe(false);
    });
  });

  describe('listAllWorkspaces', () => {
    it('merges an active and an archived admin-plane read', async () => {
      mockWorkspacesList
        .mockResolvedValueOnce({ data: [listRow] })
        .mockResolvedValueOnce({ data: [{ ...listRow, id: 'ws-uuid-2', status: 'archived' }] });
      const workspaces = await service.listAllWorkspaces();
      expect(mockWorkspacesList).toHaveBeenCalledTimes(2);
      expect(mockWorkspacesList).toHaveBeenCalledWith({ plane: 'admin' });
      expect(mockWorkspacesList).toHaveBeenCalledWith({ plane: 'admin', status: 'archived' });
      expect(workspaces.map((w) => w.id)).toEqual(['ws-uuid-1', 'ws-uuid-2']);
      expect(workspaces[1].status).toBe('archived');
    });
  });

  describe('getWorkspace', () => {
    const detail = {
      id: 'ws-uuid-1',
      name: 'Main',
      description: null,
      created_at: '2026-07-01T00:00:00Z',
      last_updated_at: '2026-07-02T00:00:00Z',
      is_default: 0,
      slug: 'ws-main-a-349e0e',
      icon: null,
      defaults: { metadata: { env: 'production' } },
      usage_limits: [{ type: 'cost', credit_limit: 10000 }],
      rate_limits: null,
      security_settings: { membersViewLogs: true },
      status: null,
    };

    it('normalizes detail and preserves null status (unknown, not inactive)', async () => {
      mockWorkspacesGet.mockResolvedValue(detail);
      const ws = await service.getWorkspace('ws-main-a-349e0e');
      expect(mockWorkspacesGet).toHaveBeenCalledWith('ws-main-a-349e0e', undefined);
      expect(ws.status).toBeNull();
      expect(ws.defaults).toEqual({ metadata: { env: 'production' } });
      expect(ws.securitySettings).toEqual({ membersViewLogs: true });
    });

    it('passes plane through to the SDK', async () => {
      mockWorkspacesGet.mockResolvedValue(detail);
      await service.getWorkspace('ws-uuid-1', { plane: 'admin' });
      expect(mockWorkspacesGet).toHaveBeenCalledWith('ws-uuid-1', { plane: 'admin' });
    });

    it('keeps array usage_limits and normalizes null limits to empty arrays', async () => {
      mockWorkspacesGet.mockResolvedValue(detail);
      const ws = await service.getWorkspace('ws-uuid-1');
      expect(ws.usageLimits).toEqual([{ type: 'cost', credit_limit: 10000 }]);
      expect(ws.rateLimits).toEqual([]);
    });

    it('wraps legacy single-object limits into an array', async () => {
      mockWorkspacesGet.mockResolvedValue({
        ...detail,
        usage_limits: { type: 'cost', credit_limit: 5 },
        rate_limits: { type: 'requests', unit: 'rpm', value: 100 },
      });
      const ws = await service.getWorkspace('ws-uuid-1');
      expect(ws.usageLimits).toEqual([{ type: 'cost', credit_limit: 5 }]);
      expect(ws.rateLimits).toEqual([{ type: 'requests', unit: 'rpm', value: 100 }]);
    });
  });
});

describe('aiGatewayGrantHint', () => {
  it('returns undefined for non-403 errors', () => {
    expect(aiGatewayGrantHint(Object.assign(new Error('boom'), { statusCode: 500 }))).toBeUndefined();
    expect(aiGatewayGrantHint(new Error('plain'))).toBeUndefined();
  });

  it('maps 403 AB03 to the missing workspace-scope grant hint', () => {
    const err = Object.assign(new Error('Forbidden: errorCode AB03'), { statusCode: 403 });
    const hint = aiGatewayGrantHint(err);
    expect(hint).toContain('workspace-scope');
    expect(hint).toContain('Add Role');
  });

  it('maps other 403s to the tenant-root admin grant hint', () => {
    const err = Object.assign(new Error('Forbidden'), { statusCode: 403 });
    const hint = aiGatewayGrantHint(err);
    expect(hint).toContain('tenant-root');
    expect(hint).toContain('Add Role');
  });
});
