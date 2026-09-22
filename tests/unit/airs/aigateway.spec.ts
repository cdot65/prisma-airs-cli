import { beforeEach, describe, expect, it, vi } from 'vitest';
import { aiGatewayGrantHint, SdkAiGatewayService } from '../../../src/airs/aigateway.js';

const mockWorkspacesList = vi.fn();
const mockWorkspacesGet = vi.fn();
const mockWorkspacesCreate = vi.fn();
const mockWorkspacesUpdate = vi.fn();
const mockWorkspacesDelete = vi.fn();
const mockWorkspacesProvision = vi.fn();
const mockTelemetryCost = vi.fn();
const mockScopesList = vi.fn();
const mockScopesGet = vi.fn();
const mockScopesCreate = vi.fn();
const mockScopesBind = vi.fn();
const mockScopesDelete = vi.fn();

function makeMockClient() {
  return {
    workspaces: {
      list: mockWorkspacesList,
      get: mockWorkspacesGet,
      create: mockWorkspacesCreate,
      update: mockWorkspacesUpdate,
      delete: mockWorkspacesDelete,
      provision: mockWorkspacesProvision,
    },
    iamScopes: {
      list: mockScopesList,
      get: mockScopesGet,
      create: mockScopesCreate,
      bindWorkspace: mockScopesBind,
      delete: mockScopesDelete,
    },
    telemetry: {
      cost: mockTelemetryCost,
    },
  };
}

// Captured from SCM's UI creating a workspace (2026-09-11).
const boundScope = {
  name: 'ws_truffles_ggolfu',
  description: 'Online recipe generation application',
  resources: [{ metadata: [], resource_id: 'ws-truffl-03e7d9', resource_type: 'workspace' }],
  tsg_id: '1001464285',
  id: 'ws_truffles_ggolfu:1001464285',
};
const normalizedBoundScope = {
  name: 'ws_truffles_ggolfu',
  description: 'Online recipe generation application',
  resources: [{ resourceType: 'workspace', resourceId: 'ws-truffl-03e7d9' }],
  tsgId: '1001464285',
  id: 'ws_truffles_ggolfu:1001464285',
};

vi.mock('@cdot65/prisma-airs-sdk', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@cdot65/prisma-airs-sdk')>()),
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

describe('workspace writes', () => {
  let service: SdkAiGatewayService;

  const detail = {
    id: 'ws-uuid-9',
    name: 'Production',
    description: 'All production applications',
    created_at: '2026-08-01T00:00:00Z',
    last_updated_at: '2026-08-01T00:00:00Z',
    is_default: 0,
    slug: 'ws-produc-985697',
    icon: null,
    defaults: null,
    usage_limits: null,
    rate_limits: [{ type: 'requests', unit: 'rpm', value: 100 }],
    status: 'active',
  };

  beforeEach(() => {
    vi.clearAllMocks();
    service = new SdkAiGatewayService();
  });

  describe('createWorkspace', () => {
    const provisioned = {
      scope: boundScope,
      workspace: { id: 'ws-uuid-9', slug: 'ws-produc-985697', scope_name: 'ws_production_bx7qw0' },
      scopeCreated: true,
    };

    it('delegates to SDK provision() (scope → workspace → bind) with a snake_case body', async () => {
      mockWorkspacesProvision.mockResolvedValue(provisioned);
      mockWorkspacesGet.mockResolvedValue(detail);
      const result = await service.createWorkspace({
        name: 'Production',
        scopeName: 'ws_production_bx7qw0',
        description: 'All production applications',
        rateLimits: [{ type: 'requests', unit: 'rpm', value: 100 }],
      });
      expect(mockWorkspacesProvision).toHaveBeenCalledWith(
        {
          name: 'Production',
          scope_name: 'ws_production_bx7qw0',
          description: 'All production applications',
          rate_limits: [{ type: 'requests', unit: 'rpm', value: 100 }],
        },
        {},
      );
      // The bare create() is never called directly any more: it would 400 AB01 on a new scope.
      expect(mockWorkspacesCreate).not.toHaveBeenCalled();
      expect(result.scope).toEqual(normalizedBoundScope);
      expect(result.scopeCreated).toBe(true);
    });

    it('omits scope_name so the SDK generates one, and passes existingScope through', async () => {
      mockWorkspacesProvision.mockResolvedValue({ ...provisioned, scopeCreated: false });
      mockWorkspacesGet.mockResolvedValue(detail);
      const generated = await service.createWorkspace({ name: 'Truffles' });
      expect(mockWorkspacesProvision).toHaveBeenCalledWith({ name: 'Truffles' }, {});
      expect(generated.scopeCreated).toBe(false);

      await service.createWorkspace({ name: 'Staging', scopeName: 'ws_s', existingScope: true });
      expect(mockWorkspacesProvision).toHaveBeenLastCalledWith(
        { name: 'Staging', scope_name: 'ws_s' },
        { existingScope: true },
      );
    });

    it('renders from a follow-up admin-plane get, not the write response', async () => {
      mockWorkspacesProvision.mockResolvedValue(provisioned);
      mockWorkspacesGet.mockResolvedValue(detail);
      const { workspace } = await service.createWorkspace({ name: 'Production' });
      expect(mockWorkspacesGet).toHaveBeenCalledWith('ws-uuid-9', { plane: 'admin' });
      expect(workspace.rateLimits).toEqual([{ type: 'requests', unit: 'rpm', value: 100 }]);
    });

    it('falls back to the normalized create response when the follow-up get fails', async () => {
      mockWorkspacesProvision.mockResolvedValue({
        ...provisioned,
        workspace: { id: 'ws-uuid-9', slug: 'ws-produc-985697', name: 'Production', is_default: 0 },
      });
      mockWorkspacesGet.mockRejectedValue(Object.assign(new Error('boom'), { statusCode: 500 }));
      const { workspace } = await service.createWorkspace({ name: 'Production' });
      expect(workspace.id).toBe('ws-uuid-9');
      expect(workspace.name).toBe('Production');
    });

    it('propagates provisioning failures untouched (the SDK already names the partial state)', async () => {
      mockWorkspacesProvision.mockRejectedValue(
        new Error(
          'workspace create failed after creating its IAM scope — IAM scope ws_x was deleted again',
        ),
      );
      await expect(service.createWorkspace({ name: 'x' })).rejects.toThrow(/was deleted again/);
      expect(mockWorkspacesGet).not.toHaveBeenCalled();
    });
  });

  describe('IAM scopes', () => {
    it('listScopes normalizes items and flags unbound scopes by an empty resources array', async () => {
      mockScopesList.mockResolvedValue({
        count: 2,
        items: [boundScope, { ...boundScope, name: 'ws_orphan_a1b2c3', resources: [] }],
      });
      const scopes = await service.listScopes();
      expect(scopes[0]).toEqual(normalizedBoundScope);
      expect(scopes[1].resources).toEqual([]);
    });

    it('getScope and createScope pass straight through', async () => {
      mockScopesGet.mockResolvedValue(boundScope);
      expect(await service.getScope('ws_truffles_ggolfu')).toEqual(normalizedBoundScope);
      expect(mockScopesGet).toHaveBeenCalledWith('ws_truffles_ggolfu');

      mockScopesCreate.mockResolvedValue({ ...boundScope, resources: [] });
      const created = await service.createScope({ name: 'ws_truffles_ggolfu', description: 'd' });
      expect(mockScopesCreate).toHaveBeenCalledWith({
        name: 'ws_truffles_ggolfu',
        description: 'd',
      });
      expect(created.resources).toEqual([]);
    });

    it('bindScope resolves a display name or UUID to the slug on the admin plane', async () => {
      mockWorkspacesList.mockResolvedValue({ data: [listRow] });
      mockScopesBind.mockResolvedValue(boundScope);
      await service.bindScope('ws_x', 'Main');
      expect(mockWorkspacesList).toHaveBeenCalledWith({ plane: 'admin' });
      expect(mockScopesBind).toHaveBeenCalledWith('ws_x', 'ws-main-a-349e0e');

      await service.bindScope('ws_x', 'ws-uuid-1');
      expect(mockScopesBind).toHaveBeenLastCalledWith('ws_x', 'ws-main-a-349e0e');
    });

    it('bindScope passes an already-slug ref through and returns the normalized scope', async () => {
      mockWorkspacesList.mockResolvedValue({ data: [listRow] });
      mockScopesBind.mockResolvedValue(boundScope);
      const scope = await service.bindScope('ws_x', 'ws-main-a-349e0e');
      expect(mockScopesBind).toHaveBeenCalledWith('ws_x', 'ws-main-a-349e0e');
      expect(scope).toEqual(normalizedBoundScope);
    });

    it('deleteScope delegates by name', async () => {
      mockScopesDelete.mockResolvedValue(undefined);
      await service.deleteScope('ws_truffles_ggolfu');
      expect(mockScopesDelete).toHaveBeenCalledWith('ws_truffles_ggolfu');
    });
  });

  describe('updateWorkspace', () => {
    it('sends a partial snake_case patch and re-reads via admin-plane get (update returns {})', async () => {
      mockWorkspacesUpdate.mockResolvedValue({});
      mockWorkspacesGet.mockResolvedValue(detail);
      const ws = await service.updateWorkspace('ws-produc-985697', {
        description: 'Production workloads, us-east',
      });
      expect(mockWorkspacesUpdate).toHaveBeenCalledWith('ws-produc-985697', {
        description: 'Production workloads, us-east',
      });
      expect(mockWorkspacesGet).toHaveBeenCalledWith('ws-produc-985697', { plane: 'admin' });
      expect(ws.id).toBe('ws-uuid-9');
    });

    it('maps camelCase limit fields to snake_case', async () => {
      mockWorkspacesUpdate.mockResolvedValue({});
      mockWorkspacesGet.mockResolvedValue(detail);
      await service.updateWorkspace('ws-uuid-9', {
        usageLimits: [{ type: 'cost', credit_limit: 5 }],
        rateLimits: [{ type: 'requests', unit: 'rpm', value: 10 }],
      });
      expect(mockWorkspacesUpdate).toHaveBeenCalledWith('ws-uuid-9', {
        usage_limits: [{ type: 'cost', credit_limit: 5 }],
        rate_limits: [{ type: 'requests', unit: 'rpm', value: 10 }],
      });
    });
  });

  describe('deleteWorkspace', () => {
    it('calls delete and does not verify via get (archived workspaces 404)', async () => {
      mockWorkspacesDelete.mockResolvedValue(undefined);
      await service.deleteWorkspace('ws-produc-985697');
      expect(mockWorkspacesDelete).toHaveBeenCalledWith('ws-produc-985697');
      expect(mockWorkspacesGet).not.toHaveBeenCalled();
    });
  });
});

describe('workspace ref resolution (name | slug | uuid)', () => {
  let service: SdkAiGatewayService;

  const rows = [
    {
      ...{
        id: 'ws-uuid-dev',
        slug: 'ws-develo-71f8d8',
        name: 'Development',
        icon: null,
        description: null,
        created_at: '',
        last_updated_at: '',
        is_default: 0,
        status: 'active',
        scope_name: 's',
        object: 'workspace',
      },
    },
    {
      id: 'ws-uuid-prod',
      slug: 'ws-produc-985697',
      name: 'Production',
      icon: null,
      description: null,
      created_at: '',
      last_updated_at: '',
      is_default: 0,
      status: 'active',
      scope_name: 's2',
      object: 'workspace',
    },
  ];

  const detail = {
    id: 'ws-uuid-dev',
    name: 'Development',
    description: 'd',
    created_at: '',
    last_updated_at: '',
    is_default: 0,
    slug: 'ws-develo-71f8d8',
    icon: null,
    defaults: null,
    usage_limits: null,
    rate_limits: null,
    status: 'active',
  };

  beforeEach(() => {
    vi.clearAllMocks();
    service = new SdkAiGatewayService();
  });

  it('update resolves a display name to the slug via an admin-plane list', async () => {
    mockWorkspacesList.mockResolvedValue({ data: rows });
    mockWorkspacesUpdate.mockResolvedValue({});
    mockWorkspacesGet.mockResolvedValue(detail);
    await service.updateWorkspace('Development', { description: 'x' });
    expect(mockWorkspacesList).toHaveBeenCalledWith({ plane: 'admin' });
    expect(mockWorkspacesUpdate).toHaveBeenCalledWith('ws-develo-71f8d8', { description: 'x' });
  });

  it('update passes a slug straight through', async () => {
    mockWorkspacesList.mockResolvedValue({ data: rows });
    mockWorkspacesUpdate.mockResolvedValue({});
    mockWorkspacesGet.mockResolvedValue(detail);
    await service.updateWorkspace('ws-develo-71f8d8', { description: 'x' });
    expect(mockWorkspacesUpdate).toHaveBeenCalledWith('ws-develo-71f8d8', { description: 'x' });
  });

  it('update passes an unmatched ref through unchanged (API produces the error)', async () => {
    mockWorkspacesList.mockResolvedValue({ data: rows });
    mockWorkspacesUpdate.mockResolvedValue({});
    mockWorkspacesGet.mockResolvedValue(detail);
    await service.updateWorkspace('nope', { description: 'x' });
    expect(mockWorkspacesUpdate).toHaveBeenCalledWith('nope', { description: 'x' });
  });

  it('throws a clear error when a display name matches multiple workspaces', async () => {
    mockWorkspacesList.mockResolvedValue({
      data: [rows[0], { ...rows[1], name: 'Development' }],
    });
    await expect(service.updateWorkspace('Development', { description: 'x' })).rejects.toThrow(
      /ambiguous/,
    );
    expect(mockWorkspacesUpdate).not.toHaveBeenCalled();
  });

  it('delete resolves a display name too', async () => {
    mockWorkspacesList.mockResolvedValue({ data: rows });
    mockWorkspacesDelete.mockResolvedValue(undefined);
    await service.deleteWorkspace('Production');
    expect(mockWorkspacesDelete).toHaveBeenCalledWith('ws-produc-985697');
  });

  it('telemetry cost resolves a display name to the slug, falling back to the admin plane', async () => {
    mockWorkspacesList
      .mockRejectedValueOnce(Object.assign(new Error('AB03'), { statusCode: 403 }))
      .mockResolvedValueOnce({ data: rows });
    mockTelemetryCost.mockResolvedValue({
      success: true,
      data: { isQuotaExceeded: false, records: [], total: 0, avg: 0 },
    });
    await service.getTelemetryCost({ workspaceSlug: 'Development' });
    expect(mockTelemetryCost).toHaveBeenCalledWith({ workspaceSlug: 'ws-develo-71f8d8', days: 7 });
  });

  it('get retries once with a resolved ref after a 404', async () => {
    mockWorkspacesGet
      .mockRejectedValueOnce(Object.assign(new Error('not found'), { statusCode: 404 }))
      .mockResolvedValueOnce(detail);
    mockWorkspacesList.mockResolvedValue({ data: rows });
    const ws = await service.getWorkspace('Development');
    expect(mockWorkspacesGet).toHaveBeenLastCalledWith('ws-develo-71f8d8', undefined);
    expect(ws.id).toBe('ws-uuid-dev');
  });
});

describe('getTelemetryCost', () => {
  let service: SdkAiGatewayService;

  beforeEach(() => {
    vi.clearAllMocks();
    service = new SdkAiGatewayService();
  });

  it('normalizes the cost chart keeping raw cents', async () => {
    mockTelemetryCost.mockResolvedValue({
      success: true,
      data: {
        isQuotaExceeded: false,
        records: [
          { x: '2026-07-30', y: 123456.78 },
          { x: '2026-07-31', y: 100 },
        ],
        total: 123556.78,
        avg: 61778.39,
      },
    });
    const report = await service.getTelemetryCost({ workspaceSlug: 'ws-main-a-349e0e', days: 7 });
    expect(mockTelemetryCost).toHaveBeenCalledWith({ workspaceSlug: 'ws-main-a-349e0e', days: 7 });
    expect(report).toEqual({
      workspaceSlug: 'ws-main-a-349e0e',
      days: 7,
      totalCents: 123556.78,
      totalUsd: 1235.5678,
      avgCents: 61778.39,
      avgUsd: 617.7839,
      quotaExceeded: false,
      records: [
        { date: '2026-07-30', costCents: 123456.78, costUsd: 1234.5678 },
        { date: '2026-07-31', costCents: 100, costUsd: 1 },
      ],
    });
  });

  it('defaults days to 7', async () => {
    mockTelemetryCost.mockResolvedValue({
      success: true,
      data: { isQuotaExceeded: false, records: [], total: 0, avg: 0 },
    });
    const report = await service.getTelemetryCost({ workspaceSlug: 'ws-x' });
    expect(mockTelemetryCost).toHaveBeenCalledWith({ workspaceSlug: 'ws-x', days: 7 });
    expect(report.days).toBe(7);
  });
});

describe('aiGatewayGrantHint', () => {
  it('returns undefined for non-403 errors', () => {
    expect(
      aiGatewayGrantHint(Object.assign(new Error('boom'), { statusCode: 500 })),
    ).toBeUndefined();
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
