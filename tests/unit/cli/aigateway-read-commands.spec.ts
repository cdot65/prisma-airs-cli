import type { AIGatewayClient } from '@cdot65/prisma-airs-sdk';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setAiGatewayClientFactoryForTest } from '../../../src/cli/commands/aigateway/shared.js';
import { buildProgram } from '../../../src/cli/program.js';

const methods = {
  apiKeysGetService: vi.fn(),
  apiKeysGetUser: vi.fn(),
  apiKeysListService: vi.fn(),
  apiKeysListUser: vi.fn(),
  auditLogsList: vi.fn(),
  configsGet: vi.fn(),
  configsList: vi.fn(),
  configsVersions: vi.fn(),
  deploymentsGet: vi.fn(),
  deploymentsList: vi.fn(),
  deploymentsPing: vi.fn(),
  guardrailsList: vi.fn(),
  integrationsModels: vi.fn(),
  integrationsWorkspaces: vi.fn(),
  mcpCapabilities: vi.fn(),
  mcpMetadata: vi.fn(),
  organisationsAuth: vi.fn(),
  organisationsSelf: vi.fn(),
  pluginsList: vi.fn(),
  providersGet: vi.fn(),
  providersList: vi.fn(),
  telemetryRequests: vi.fn(),
};

function fakeClient(): AIGatewayClient {
  return {
    apiKeys: {
      getService: methods.apiKeysGetService,
      getUser: methods.apiKeysGetUser,
      listService: methods.apiKeysListService,
      listUser: methods.apiKeysListUser,
    },
    auditLogs: { list: methods.auditLogsList },
    configs: {
      get: methods.configsGet,
      list: methods.configsList,
      listVersions: methods.configsVersions,
    },
    deployments: {
      get: methods.deploymentsGet,
      list: methods.deploymentsList,
      ping: methods.deploymentsPing,
    },
    guardrails: { list: methods.guardrailsList },
    integrations: {
      getModels: methods.integrationsModels,
      getWorkspaces: methods.integrationsWorkspaces,
    },
    mcpIntegrations: {
      getCapabilities: methods.mcpCapabilities,
      getMetadata: methods.mcpMetadata,
    },
    organisations: {
      getAuthSettings: methods.organisationsAuth,
      getSelf: methods.organisationsSelf,
    },
    plugins: { list: methods.pluginsList },
    providers: { get: methods.providersGet, list: methods.providersList },
    telemetry: { requests: methods.telemetryRequests },
  } as unknown as AIGatewayClient;
}

let restoreFactory: (() => void) | undefined;

beforeEach(() => {
  vi.clearAllMocks();
  for (const method of Object.values(methods)) method.mockResolvedValue({ data: [] });
  methods.configsGet.mockResolvedValue({ id: 'config-1' });
  methods.deploymentsGet.mockResolvedValue({ id: 'deployment-1' });
  methods.deploymentsPing.mockResolvedValue({ status: 'healthy' });
  methods.integrationsWorkspaces.mockResolvedValue({ workspaces: [] });
  methods.mcpCapabilities.mockResolvedValue({ capabilities: [] });
  methods.mcpMetadata.mockResolvedValue({ name: 'server' });
  methods.organisationsAuth.mockResolvedValue({ auth_type: 'local' });
  methods.organisationsSelf.mockResolvedValue({ id: 'organisation-1' });
  methods.providersGet.mockResolvedValue({
    id: 'provider-1',
    api_key: 'provider-secret',
    model_config: { token: 'provider-model-secret' },
  });
  restoreFactory = setAiGatewayClientFactoryForTest(async () => fakeClient());
  vi.spyOn(console, 'log').mockImplementation(() => undefined);
  vi.spyOn(console, 'error').mockImplementation(() => undefined);
});

afterEach(() => {
  restoreFactory?.();
  vi.restoreAllMocks();
});

async function run(...args: string[]): Promise<void> {
  await buildProgram().parseAsync(['node', 'airs', 'aigateway', ...args, '--output', 'json']);
}

describe('AI Gateway read command SDK mappings', () => {
  it('maps workspace-scoped collection reads to workspace UUID options', async () => {
    await run('configs', 'list', '--workspace', 'workspace-1');
    expect(methods.configsList).toHaveBeenCalledWith({ workspaceId: 'workspace-1' });

    await run('guardrails', 'list', '--workspace', 'workspace-1');
    expect(methods.guardrailsList).toHaveBeenCalledWith({ workspaceId: 'workspace-1' });

    await run('providers', 'list', '--workspace', 'workspace-1');
    expect(methods.providersList).toHaveBeenCalledWith({ workspaceId: 'workspace-1' });

    await run('api-keys', 'service', 'list', '--workspace', 'workspace-1');
    expect(methods.apiKeysListService).toHaveBeenCalledWith({ workspaceId: 'workspace-1' });
  });

  it('maps config detail and version reads', async () => {
    await run('configs', 'get', 'config-1');
    expect(methods.configsGet).toHaveBeenCalledWith('config-1');
    await run('configs', 'versions', 'config-1');
    expect(methods.configsVersions).toHaveBeenCalledWith('config-1');
  });

  it('keeps deployment heartbeat detail and ingress ping as separate reads', async () => {
    await run('deployments', 'list');
    expect(methods.deploymentsList).toHaveBeenCalledOnce();
    await run('deployments', 'get', 'deployment-1');
    expect(methods.deploymentsGet).toHaveBeenCalledWith('deployment-1');
    await run('deployments', 'ping', 'deployment-1');
    expect(methods.deploymentsPing).toHaveBeenCalledWith('deployment-1');
  });

  it('maps relationship and organisation reads without changing identifiers', async () => {
    await run('integrations', 'models', 'list', 'integration-1');
    expect(methods.integrationsModels).toHaveBeenCalledWith('integration-1');
    await run('integrations', 'workspaces', 'list', 'integration-1');
    expect(methods.integrationsWorkspaces).toHaveBeenCalledWith('integration-1');
    await run('mcp', 'integrations', 'capabilities', 'list', 'mcp-1');
    expect(methods.mcpCapabilities).toHaveBeenCalledWith('mcp-1');
    await run('mcp', 'integrations', 'metadata', 'mcp-1');
    expect(methods.mcpMetadata).toHaveBeenCalledWith('mcp-1');
    await run('organisations', 'self', 'get');
    expect(methods.organisationsSelf).toHaveBeenCalledOnce();
    await run('organisations', 'auth-settings', 'get', '--tsg-id', '1234567890');
    expect(methods.organisationsAuth).toHaveBeenCalledWith('1234567890');
  });

  it('emits structured data only on stdout for automation output', async () => {
    methods.pluginsList.mockResolvedValue({ data: [{ id: 'plugin-1', name: 'AIRS' }] });
    await run('plugins', 'list');
    const stdout = vi
      .mocked(console.log)
      .mock.calls.map(([line]) => String(line))
      .join('\n');
    expect(JSON.parse(stdout)).toEqual([{ id: 'plugin-1', name: 'AIRS' }]);
  });

  it('redacts provider credentials unless explicitly revealed', async () => {
    await run('providers', 'get', 'provider-1');
    let stdout = vi
      .mocked(console.log)
      .mock.calls.map(([line]) => String(line))
      .join('\n');
    expect(stdout).not.toContain('provider-secret');
    expect(stdout).not.toContain('provider-model-secret');
    expect(JSON.parse(stdout).api_key).toBe('***');

    vi.mocked(console.log).mockClear();
    await run('providers', 'get', 'provider-1', '--reveal-sensitive');
    stdout = vi
      .mocked(console.log)
      .mock.calls.map(([line]) => String(line))
      .join('\n');
    expect(JSON.parse(stdout).api_key).toBe('provider-secret');
    expect(stdout).toContain('provider-model-secret');
  });

  it('uses SDK operation metadata to redact organisation auth secrets by default', async () => {
    methods.organisationsAuth.mockResolvedValue({ auth_type: 'scim', scim_token: 'scim-secret' });
    await run('organisations', 'auth-settings', 'get', '--tsg-id', '1234567890');
    let stdout = vi
      .mocked(console.log)
      .mock.calls.map(([line]) => String(line))
      .join('\n');
    expect(JSON.parse(stdout).scim_token).toBe('[REDACTED]');

    vi.mocked(console.log).mockClear();
    await run(
      'organisations',
      'auth-settings',
      'get',
      '--tsg-id',
      '1234567890',
      '--reveal-sensitive',
    );
    stdout = vi
      .mocked(console.log)
      .mock.calls.map(([line]) => String(line))
      .join('\n');
    expect(JSON.parse(stdout).scim_token).toBe('scim-secret');
  });

  it.each([
    'service',
    'user',
  ] as const)('redacts %s API keys on list and detail reads unless explicitly revealed', async (kind) => {
    const listMethod = kind === 'service' ? methods.apiKeysListService : methods.apiKeysListUser;
    const getMethod = kind === 'service' ? methods.apiKeysGetService : methods.apiKeysGetUser;
    listMethod.mockResolvedValue({
      data: [{ api_key_defaults_id: 'defaults-1', id: 'key-1', key: 'live-list-secret' }],
    });
    getMethod.mockResolvedValue({
      api_key_defaults_id: 'defaults-1',
      id: 'key-1',
      key: 'live-detail-secret',
    });

    await run('api-keys', kind, 'list', '--workspace', 'workspace-1');
    let stdout = vi
      .mocked(console.log)
      .mock.calls.map(([line]) => String(line))
      .join('\n');
    expect(stdout).not.toContain('live-list-secret');
    expect(JSON.parse(stdout)[0].key).toBe('***');
    expect(JSON.parse(stdout)[0].api_key_defaults_id).toBe('defaults-1');

    vi.mocked(console.log).mockClear();
    await run('api-keys', kind, 'get', 'key-1');
    stdout = vi
      .mocked(console.log)
      .mock.calls.map(([line]) => String(line))
      .join('\n');
    expect(stdout).not.toContain('live-detail-secret');
    expect(JSON.parse(stdout).key).toBe('***');
    expect(JSON.parse(stdout).api_key_defaults_id).toBe('defaults-1');

    vi.mocked(console.log).mockClear();
    await run('api-keys', kind, 'list', '--workspace', 'workspace-1', '--reveal-sensitive');
    stdout = vi
      .mocked(console.log)
      .mock.calls.map(([line]) => String(line))
      .join('\n');
    expect(JSON.parse(stdout)[0].key).toBe('live-list-secret');

    vi.mocked(console.log).mockClear();
    await run('api-keys', kind, 'get', 'key-1', '--reveal-sensitive');
    stdout = vi
      .mocked(console.log)
      .mock.calls.map(([line]) => String(line))
      .join('\n');
    expect(JSON.parse(stdout).key).toBe('live-detail-secret');
  });

  it.each([
    {
      args: ['telemetry', 'requests', '--workspace', 'ws-test', '--days', 'abc'],
      message: 'Invalid --days',
    },
    {
      args: ['telemetry', 'requests', '--workspace', 'ws-test', '--start', 'not-a-date'],
      message: 'Invalid --start',
    },
    {
      args: ['audit-logs', 'list', '--end', 'not-a-date'],
      message: 'Invalid --end',
    },
  ])('reports invalid time-window flags as usage errors: $message', async ({ args, message }) => {
    const exit = vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new Error(`process.exit(${code})`);
    }) as never);

    await expect(run(...args)).rejects.toThrow('process.exit(2)');
    expect(exit).toHaveBeenCalledWith(2);
    const stderr = vi.mocked(console.error).mock.calls.flat().map(String).join('\n');
    expect(stderr).toContain(message);
  });
});
