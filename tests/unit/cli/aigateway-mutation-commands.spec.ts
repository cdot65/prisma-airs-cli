import { mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { AIGatewayClient } from '@cdot65/prisma-airs-sdk';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setAiGatewayClientFactoryForTest } from '../../../src/cli/commands/aigateway/shared.js';
import { buildProgram } from '../../../src/cli/program.js';

const calls = {
  apiKeyCreate: vi.fn(),
  configCreate: vi.fn(),
  configDelete: vi.fn(),
  deploymentCreate: vi.fn(),
  deploymentDelete: vi.fn(),
  guardrailCreate: vi.fn(),
  integrationCreate: vi.fn(),
  mcpCreate: vi.fn(),
  mcpSetWorkspaces: vi.fn(),
  organisationUpdate: vi.fn(),
  pluginCreate: vi.fn(),
  providerCreate: vi.fn(),
};

function fakeClient(): AIGatewayClient {
  return {
    apiKeys: { createService: calls.apiKeyCreate },
    configs: { create: calls.configCreate, delete: calls.configDelete },
    deployments: { create: calls.deploymentCreate, delete: calls.deploymentDelete },
    guardrails: { create: calls.guardrailCreate },
    integrations: { create: calls.integrationCreate },
    mcpIntegrations: { create: calls.mcpCreate, setWorkspaces: calls.mcpSetWorkspaces },
    organisations: { updateSelf: calls.organisationUpdate },
    plugins: { create: calls.pluginCreate },
    providers: { create: calls.providerCreate },
  } as unknown as AIGatewayClient;
}

let directory = '';
let restoreFactory: (() => void) | undefined;

beforeEach(async () => {
  vi.clearAllMocks();
  directory = await mkdtemp(join(tmpdir(), 'airs-aigateway-cli-'));
  restoreFactory = setAiGatewayClientFactoryForTest(async () => fakeClient());
  for (const method of Object.values(calls)) method.mockResolvedValue({});
  vi.spyOn(console, 'log').mockImplementation(() => undefined);
  vi.spyOn(console, 'error').mockImplementation(() => undefined);
});

afterEach(async () => {
  restoreFactory?.();
  vi.restoreAllMocks();
  await rm(directory, { recursive: true, force: true });
});

async function requestFile(value: unknown, extension = 'json'): Promise<string> {
  const path = join(directory, `request.${extension}`);
  await writeFile(path, extension === 'yaml' ? 'name: from-yaml\n' : JSON.stringify(value));
  return path;
}

async function run(...args: string[]): Promise<void> {
  await buildProgram().parseAsync(['node', 'airs', 'aigateway', ...args]);
}

describe('AI Gateway mutation commands', () => {
  const workspaceId = '11111111-1111-4111-8111-111111111111';

  it('passes a validated JSON config escape-hatch body to the SDK without reshaping it', async () => {
    const body = {
      name: 'test-config',
      workspace_id: workspaceId,
      config: { retry: { attempts: 3 } },
    };
    await run('configs', 'create', '--file', await requestFile(body), '--output', 'json');
    expect(calls.configCreate).toHaveBeenCalledWith(body);
  });

  it('builds config bodies from named and dotted flags without a file', async () => {
    await run(
      'configs',
      'create',
      '--name',
      'from-flags',
      '--workspace',
      workspaceId,
      '--set',
      'config.retry.attempts=3',
      '--set',
      'config.strategy.mode=fallback',
      '--output',
      'json',
    );
    expect(calls.configCreate).toHaveBeenCalledWith({
      name: 'from-flags',
      workspace_id: workspaceId,
      config: { retry: { attempts: 3 }, strategy: { mode: 'fallback' } },
    });
  });

  it('accepts a complete YAML request body as the advanced fallback', async () => {
    const path = join(directory, 'request.yaml');
    await writeFile(
      path,
      [
        'name: from-yaml',
        `workspace_id: ${workspaceId}`,
        'config:',
        '  retry:',
        '    attempts: 2',
        '',
      ].join('\n'),
    );
    await run('configs', 'create', '--file', path, '--output', 'json');
    expect(calls.configCreate).toHaveBeenCalledWith({
      name: 'from-yaml',
      workspace_id: workspaceId,
      config: { retry: { attempts: 2 } },
    });
  });

  it('builds each configuration resource from named SDK-backed flags', async () => {
    const providerCatalogId = '22222222-2222-4222-8222-222222222222';
    const integrationId = '33333333-3333-4333-8333-333333333333';

    await run(
      'guardrails',
      'create',
      '--workspace',
      workspaceId,
      '--name',
      'deny-risk',
      '--set',
      'checks[0].id=prompt-injection',
      '--set',
      'actions.deny=true',
    );
    expect(calls.guardrailCreate).toHaveBeenCalledWith({
      workspace_id: workspaceId,
      name: 'deny-risk',
      checks: [{ id: 'prompt-injection' }],
      actions: { deny: true },
    });

    await run(
      'integrations',
      'create',
      '--organisation-id',
      '1188256439',
      '--ai-provider-id',
      providerCatalogId,
      '--name',
      'vertex-prod',
      '--slug',
      'vertex-prod',
      '--set',
      'configurations.vertex_project_id=project-1',
    );
    expect(calls.integrationCreate).toHaveBeenCalledWith({
      organisation_id: '1188256439',
      ai_provider_id: providerCatalogId,
      name: 'vertex-prod',
      slug: 'vertex-prod',
      configurations: { vertex_project_id: 'project-1' },
    });

    await run(
      'mcp',
      'integrations',
      'create',
      '--name',
      'readonly-mcp',
      '--organisation-id',
      '1188256439',
      '--slug',
      'readonly-mcp',
      '--url',
      'https://mcp.example.test/mcp',
      '--auth-type',
      'none',
      '--transport',
      'http',
    );
    expect(calls.mcpCreate).toHaveBeenCalledWith({
      name: 'readonly-mcp',
      organisation_id: '1188256439',
      slug: 'readonly-mcp',
      url: 'https://mcp.example.test/mcp',
      auth_type: 'none',
      transport: 'http',
    });

    await run(
      'providers',
      'create',
      '--workspace',
      workspaceId,
      '--ai-provider-id',
      providerCatalogId,
      '--integration-id',
      integrationId,
      '--name',
      'vertex-binding',
      '--slug',
      'vertex-binding',
    );
    expect(calls.providerCreate).toHaveBeenCalledWith({
      workspace_id: workspaceId,
      ai_provider_id: providerCatalogId,
      integration_id: integrationId,
      name: 'vertex-binding',
      slug: 'vertex-binding',
    });

    await run(
      'deployments',
      'create',
      '--name',
      'private-gateway',
      '--type',
      'production',
      '--organisation-id',
      '1188256439',
      '--show-secret',
    );
    expect(calls.deploymentCreate).toHaveBeenCalledWith({
      name: 'private-gateway',
      type: 'production',
      organisation_id: '1188256439',
    });

    await run(
      'plugins',
      'create',
      '--organisation-id',
      '1188256439',
      '--integration-id',
      integrationId,
      '--credential',
      'token=secret',
    );
    expect(calls.pluginCreate).toHaveBeenCalledWith({
      organisation_id: '1188256439',
      integration_id: integrationId,
      credentials: { token: 'secret' },
    });

    await run('organisations', 'self', 'update', '--name', 'Development');
    expect(calls.organisationUpdate).toHaveBeenCalledWith({ name: 'Development' });
  });

  it('requires force or confirmation for hard delete and archive operations', async () => {
    await run('configs', 'delete', 'config-1', '--force');
    expect(calls.configDelete).toHaveBeenCalledWith('config-1');

    await run(
      'deployments',
      'archive',
      'deployment-1',
      '--organisation-id',
      '1188256439',
      '--force',
    );
    expect(calls.deploymentDelete).toHaveBeenCalledWith('deployment-1', '1188256439');
  });

  it('passes MCP workspace replacement bodies exactly', async () => {
    const body = {
      workspaces: [{ id: 'workspace-1', enabled: false }],
      override_existing_workspace_access: true,
    };
    await run(
      'mcp',
      'integrations',
      'workspaces',
      'set',
      'mcp-1',
      '--file',
      await requestFile(body),
      '--force',
      '--output',
      'json',
    );
    expect(calls.mcpSetWorkspaces).toHaveBeenCalledWith('mcp-1', body);
  });

  it('builds MCP workspace replacement bodies from repeatable bindings', async () => {
    await run(
      'mcp',
      'integrations',
      'workspaces',
      'set',
      'mcp-1',
      '--workspace-binding',
      'workspace-1=false',
      '--global-access',
      'false',
      '--preserve-existing',
      '--force',
      '--output',
      'json',
    );
    expect(calls.mcpSetWorkspaces).toHaveBeenCalledWith('mcp-1', {
      workspaces: [{ id: 'workspace-1', enabled: false }],
      global_workspace_access: { enabled: false },
      override_existing_workspace_access: false,
    });
  });

  it('requires an explicit destination before requesting a one-time API-key secret', async () => {
    const body = {
      name: 'e2e-key',
      scopes: ['completions.write'],
      organisation_id: '1188256439',
      workspace_id: workspaceId,
      type: 'workspace',
    };
    const file = await requestFile(body);
    await expect(run('api-keys', 'service', 'create', '--file', file)).rejects.toThrow();
    expect(calls.apiKeyCreate).not.toHaveBeenCalled();

    await run('api-keys', 'service', 'create', '--file', file, '--show-secret', '--output', 'json');
    expect(calls.apiKeyCreate).toHaveBeenCalledWith(body);
  });

  it('writes one-time credentials to a new owner-only file', async () => {
    calls.apiKeyCreate.mockResolvedValue({ id: 'key-1', api_key: 'secret-value' });
    const destination = join(directory, 'credential.json');

    const args = [
      'api-keys',
      'service',
      'create',
      '--name',
      'e2e-key',
      '--organisation-id',
      '1188256439',
      '--workspace',
      workspaceId,
      '--type',
      'workspace',
      '--scopes',
      'completions.write',
      '--secret-output',
      destination,
    ];
    await run(...args);

    expect((await stat(destination)).mode & 0o777).toBe(0o600);
    expect(JSON.parse(await readFile(destination, 'utf8'))).toEqual({
      id: 'key-1',
      api_key: 'secret-value',
    });
    await expect(run(...args)).rejects.toThrow();
  });
});
