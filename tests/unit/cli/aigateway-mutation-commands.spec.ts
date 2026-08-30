import { execFile } from 'node:child_process';
import { mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import type { AIGatewayClient } from '@cdot65/prisma-airs-sdk';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setAiGatewayClientFactoryForTest } from '../../../src/cli/commands/aigateway/shared.js';
import { buildProgram } from '../../../src/cli/program.js';

const execFileAsync = promisify(execFile);

const calls = {
  apiKeyCreate: vi.fn(),
  configCreate: vi.fn(),
  configDelete: vi.fn(),
  deploymentCreate: vi.fn(),
  deploymentDelete: vi.fn(),
  deploymentUpdate: vi.fn(),
  guardrailCreate: vi.fn(),
  integrationCreate: vi.fn(),
  integrationSetWorkspaces: vi.fn(),
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
    deployments: {
      create: calls.deploymentCreate,
      delete: calls.deploymentDelete,
      update: calls.deploymentUpdate,
    },
    guardrails: { create: calls.guardrailCreate },
    integrations: {
      create: calls.integrationCreate,
      setWorkspaces: calls.integrationSetWorkspaces,
    },
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

  it.each([
    {
      args: [
        'configs',
        'create',
        '--name',
        'invalid',
        '--workspace',
        workspaceId,
        '--set',
        'config.a=1',
        '--set',
        'config.a=2',
      ],
      message: 'Duplicate',
    },
    {
      args: ['configs', 'create', '--name', 'missing-workspace'],
      message: 'Invalid AI Gateway request',
    },
    {
      args: ['deployments', 'update', 'deployment-1', '--is-default', 'yes'],
      message: 'Invalid --is-default',
    },
  ])('reports structured-input failures as usage errors: $message', async ({ args, message }) => {
    const exit = vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new Error(`process.exit(${code})`);
    }) as never);

    await expect(run(...args)).rejects.toThrow('process.exit(2)');
    expect(exit).toHaveBeenCalledWith(2);
    expect(calls.configCreate).not.toHaveBeenCalled();
    expect(calls.deploymentUpdate).not.toHaveBeenCalled();
    const stderr = vi.mocked(console.error).mock.calls.flat().map(String).join('\n');
    expect(stderr).toContain(message);
    expect(stderr).not.toMatch(/\n\s+at /);
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
      '1234567890',
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
      organisation_id: '1234567890',
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
      '1234567890',
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
      organisation_id: '1234567890',
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
      '1234567890',
      '--show-secret',
    );
    expect(calls.deploymentCreate).toHaveBeenCalledWith({
      name: 'private-gateway',
      type: 'production',
      organisation_id: '1234567890',
    });

    await run(
      'plugins',
      'create',
      '--organisation-id',
      '1234567890',
      '--integration-id',
      integrationId,
      '--credential',
      'token=secret',
    );
    expect(calls.pluginCreate).toHaveBeenCalledWith({
      organisation_id: '1234567890',
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
      '1234567890',
      '--force',
    );
    expect(calls.deploymentDelete).toHaveBeenCalledWith('deployment-1', '1234567890');
  });

  it('uses an imperative non-interactive refusal message', async () => {
    const execution = execFileAsync(
      process.execPath,
      ['--import', 'tsx', 'src/cli/index.ts', 'aigateway', 'configs', 'delete', 'config-1'],
      { cwd: process.cwd(), env: { ...process.env, NO_COLOR: '1' } },
    );

    await expect(execution).rejects.toMatchObject({
      code: 2,
      stderr: expect.stringContaining(
        'refusing to permanently delete config config-1 without --force',
      ),
    });
  });

  it('removes a reserved secret destination before a non-interactive decline exits', async () => {
    const destination = join(directory, 'declined-rotation.json');
    const execution = execFileAsync(
      process.execPath,
      [
        '--import',
        'tsx',
        'src/cli/index.ts',
        'aigateway',
        'api-keys',
        'service',
        'rotate',
        'key-1',
        '--secret-output',
        destination,
      ],
      { cwd: process.cwd(), env: { ...process.env, NO_COLOR: '1' } },
    );

    await expect(execution).rejects.toMatchObject({ code: 2 });
    await expect(stat(destination)).rejects.toMatchObject({ code: 'ENOENT' });
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

    await run(
      'mcp',
      'integrations',
      'workspaces',
      'set',
      'mcp-1',
      '--workspace-binding',
      'workspace-2=true',
      '--force',
    );
    expect(calls.mcpSetWorkspaces).toHaveBeenLastCalledWith('mcp-1', {
      workspaces: [{ id: 'workspace-2', enabled: true }],
      override_existing_workspace_access: true,
    });
  });

  it('replaces integration workspace bindings by default and preserves only when requested', async () => {
    const base = [
      'integrations',
      'workspaces',
      'set',
      'integration-1',
      '--workspace-binding',
      'workspace-1=true',
      '--force',
    ];
    await run(...base);
    expect(calls.integrationSetWorkspaces).toHaveBeenLastCalledWith('integration-1', {
      workspaces: [{ id: 'workspace-1', enabled: true }],
      override_existing_workspace_access: true,
    });

    await run(...base, '--preserve-existing');
    expect(calls.integrationSetWorkspaces).toHaveBeenLastCalledWith('integration-1', {
      workspaces: [{ id: 'workspace-1', enabled: true }],
      override_existing_workspace_access: false,
    });
  });

  it('requires an explicit destination before requesting a one-time API-key secret', async () => {
    const body = {
      name: 'e2e-key',
      scopes: ['completions.write'],
      organisation_id: '1234567890',
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
      '1234567890',
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
    expect(calls.apiKeyCreate).toHaveBeenCalledTimes(1);
    expect(JSON.parse(await readFile(destination, 'utf8'))).toEqual({
      id: 'key-1',
      api_key: 'secret-value',
    });
  });

  it('removes a reserved secret file when the API mutation fails', async () => {
    calls.apiKeyCreate.mockRejectedValue(new Error('API unavailable'));
    const destination = join(directory, 'failed-credential.json');

    await expect(
      run(
        'api-keys',
        'service',
        'create',
        '--name',
        'failed-key',
        '--organisation-id',
        '1234567890',
        '--workspace',
        workspaceId,
        '--type',
        'workspace',
        '--scopes',
        'completions.write',
        '--secret-output',
        destination,
      ),
    ).rejects.toThrow();
    await expect(stat(destination)).rejects.toMatchObject({ code: 'ENOENT' });
  });

  it('gates and safely renders deployment auth rotation responses', async () => {
    calls.deploymentUpdate.mockResolvedValue({
      id: 'deployment-1',
      client_auth: 'one-time-client-auth',
      credentials: { password: 'one-time-password' },
    });

    await run('deployments', 'update', 'deployment-1', '--name', 'renamed', '--output', 'json');
    const stdout = vi
      .mocked(console.log)
      .mock.calls.map(([line]) => String(line))
      .join('\n');
    expect(stdout).not.toContain('one-time-client-auth');
    expect(stdout).not.toContain('one-time-password');

    vi.mocked(console.log).mockClear();
    calls.deploymentUpdate.mockClear();
    await expect(
      run('deployments', 'update', 'deployment-1', '--rotate-auth', 'true'),
    ).rejects.toThrow();
    expect(calls.deploymentUpdate).not.toHaveBeenCalled();

    const destination = join(directory, 'deployment-credentials.json');
    await run(
      'deployments',
      'update',
      'deployment-1',
      '--rotate-auth',
      'true',
      '--secret-output',
      destination,
    );
    expect(JSON.parse(await readFile(destination, 'utf8'))).toMatchObject({
      client_auth: 'one-time-client-auth',
      credentials: { password: 'one-time-password' },
    });
  });
});
