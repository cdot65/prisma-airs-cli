import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SdkAiGatewayService } from '../../../src/airs/aigateway.js';
import type {
  AiGatewayScope,
  AiGatewayWorkspaceDetail,
  AiGatewayWorkspaceProvisionResult,
} from '../../../src/airs/types.js';
import { buildProgram } from '../../../src/cli/program.js';
import { useTestTenant } from '../../helpers/tenant.js';

const service = {
  createWorkspace: vi.fn(),
  listScopes: vi.fn(),
  getScope: vi.fn(),
  createScope: vi.fn(),
  bindScope: vi.fn(),
  deleteScope: vi.fn(),
};

vi.mock('../../../src/airs/aigateway.js', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../src/airs/aigateway.js')>()),
  SdkAiGatewayService: vi.fn(),
}));

const scope: AiGatewayScope = {
  name: 'ws_truffles_ggolfu',
  description: 'Online recipe generation application',
  resources: [{ resourceType: 'workspace', resourceId: 'ws-truffl-03e7d9' }],
  tsgId: '1001464285',
  id: 'ws_truffles_ggolfu:1001464285',
};
const workspace: AiGatewayWorkspaceDetail = {
  id: '5f2e45ed-6b07-4229-bb16-d0cc4ee8cf0e',
  slug: 'ws-truffl-03e7d9',
  name: 'truffles',
  description: 'Online recipe generation application',
  isDefault: false,
  status: 'active',
  scopeName: 'ws_truffles_ggolfu',
  usageLimits: [],
  rateLimits: [],
};
const provisioned: AiGatewayWorkspaceProvisionResult = { workspace, scope, scopeCreated: true };

async function run(...args: string[]): Promise<void> {
  await buildProgram().parseAsync(['node', 'airs', 'aigateway', ...args]);
}
const stdout = () => vi.mocked(console.log).mock.calls.flat().map(String).join('\n');
const stderr = () => vi.mocked(console.error).mock.calls.flat().map(String).join('\n');
// ui.success()/ui.warn() print to stdout ahead of the payload; pick the JSON document out.
const stdoutJson = () => {
  const line = vi
    .mocked(console.log)
    .mock.calls.map(([first]) => String(first))
    .find((text) => /^[[{]/.test(text.trim()));
  if (!line) throw new Error(`no JSON on stdout:\n${stdout()}`);
  return JSON.parse(line);
};

beforeEach(async () => {
  vi.clearAllMocks();
  await useTestTenant();
  // Re-applied per test: restoreAllMocks() strips vi.fn implementations between tests.
  vi.mocked(SdkAiGatewayService).mockImplementation(
    () => service as unknown as SdkAiGatewayService,
  );
  service.createWorkspace.mockResolvedValue(provisioned);
  service.listScopes.mockResolvedValue([scope]);
  service.getScope.mockResolvedValue(scope);
  service.createScope.mockResolvedValue({ ...scope, resources: [] });
  service.bindScope.mockResolvedValue(scope);
  service.deleteScope.mockResolvedValue(undefined);
  vi.spyOn(console, 'log').mockImplementation(() => undefined);
  vi.spyOn(console, 'error').mockImplementation(() => undefined);
  vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
    throw new Error(`process.exit(${code})`);
  }) as never);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('airs aigateway workspaces create', () => {
  it('provisions with a generated scope when --scope-name is omitted', async () => {
    await run(
      'workspaces',
      'create',
      '--name',
      'truffles',
      '--description',
      'Online recipe generation application',
      '--output',
      'json',
    );
    expect(service.createWorkspace).toHaveBeenCalledWith({
      name: 'truffles',
      description: 'Online recipe generation application',
    });
    expect(stdoutJson().slug).toBe('ws-truffl-03e7d9');
    expect(stderr()).toContain(
      'ws_truffles_ggolfu created and bound to workspace ws-truffl-03e7d9',
    );
  });

  it('passes --scope-name and --existing-scope through and reports a reused scope', async () => {
    service.createWorkspace.mockResolvedValue({ ...provisioned, scopeCreated: false });
    await run(
      'workspaces',
      'create',
      '--name',
      'truffles',
      '--scope-name',
      'ws_truffles_ggolfu',
      '--existing-scope',
      '--output',
      'json',
    );
    expect(service.createWorkspace).toHaveBeenCalledWith({
      name: 'truffles',
      scopeName: 'ws_truffles_ggolfu',
      existingScope: true,
    });
    expect(stderr()).toContain('ws_truffles_ggolfu bound to workspace');
    expect(stderr()).not.toContain('created and bound');
  });

  it('rejects --existing-scope without --scope-name as a usage error before any call', async () => {
    await expect(
      run('workspaces', 'create', '--name', 'truffles', '--existing-scope'),
    ).rejects.toThrow('process.exit(2)');
    expect(service.createWorkspace).not.toHaveBeenCalled();
    expect(stderr()).toContain('--existing-scope requires --scope-name');
  });

  it('warns when an explicit scope name shares nothing with the workspace name', async () => {
    await run('workspaces', 'create', '--name', 'Production', '--scope-name', 'ws_zzz_111111');
    expect(`${stdout()}\n${stderr()}`).toContain('shares no token');
    expect(service.createWorkspace).toHaveBeenCalled();
  });

  it('surfaces provisioning failures with the SDK message and exit 1', async () => {
    service.createWorkspace.mockRejectedValue(
      new Error('workspace ws-x was created but could not be bound to IAM scope ws_y'),
    );
    await expect(run('workspaces', 'create', '--name', 'x')).rejects.toThrow('process.exit(1)');
    expect(stderr()).toContain('could not be bound to IAM scope ws_y');
  });
});

describe('airs aigateway scopes', () => {
  it('lists scopes as json rows', async () => {
    await run('scopes', 'list', '--output', 'json');
    expect(service.listScopes).toHaveBeenCalled();
    expect(stdoutJson()[0]).toMatchObject({ name: 'ws_truffles_ggolfu' });
  });

  it('gets one scope by name', async () => {
    await run('scopes', 'get', 'ws_truffles_ggolfu', '--output', 'json');
    expect(service.getScope).toHaveBeenCalledWith('ws_truffles_ggolfu');
    expect(stdoutJson().id).toBe('ws_truffles_ggolfu:1001464285');
  });

  it('creates an unbound scope and says it still needs binding', async () => {
    await run('scopes', 'create', '--name', 'ws_truffles_ggolfu', '--description', 'd');
    expect(service.createScope).toHaveBeenCalledWith({
      name: 'ws_truffles_ggolfu',
      description: 'd',
    });
    expect(stderr()).toContain('not bound to anything yet');
  });

  it('binds a workspace ref to a scope', async () => {
    await run(
      'scopes',
      'bind',
      'ws_truffles_ggolfu',
      '--workspace',
      'truffles',
      '--output',
      'json',
    );
    expect(service.bindScope).toHaveBeenCalledWith('ws_truffles_ggolfu', 'truffles');
    expect(stdoutJson().resources).toHaveLength(1);
  });

  it('deletes a scope only with --force in a non-TTY run', async () => {
    await run('scopes', 'rm', 'ws_truffles_ggolfu', '--force');
    expect(service.deleteScope).toHaveBeenCalledWith('ws_truffles_ggolfu');
    expect(stdout()).toContain('IAM scope deleted');
  });

  it('refuses to delete without confirmation', async () => {
    await expect(run('scopes', 'delete', 'ws_truffles_ggolfu')).rejects.toThrow(/process.exit/);
    expect(service.deleteScope).not.toHaveBeenCalled();
  });
});
