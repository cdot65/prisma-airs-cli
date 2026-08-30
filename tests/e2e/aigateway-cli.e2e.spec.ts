import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { promisify } from 'node:util';
import { config } from 'dotenv';
import { describe, expect, it } from 'vitest';

config({ path: '.env.ai-gateway.local', quiet: true });

const execFileAsync = promisify(execFile);
const hasCredentials = Boolean(
  process.env.PANW_MGMT_CLIENT_ID &&
    process.env.PANW_MGMT_CLIENT_SECRET &&
    process.env.PANW_MGMT_TSG_ID,
);
const workspaceSlug = process.env.AI_GATEWAY_E2E_WORKSPACE_SLUG ?? 'ws-develo-71f8d8';

async function runJson<T = unknown>(...args: string[]): Promise<T> {
  const { stdout } = await execFileAsync(
    process.execPath,
    ['--import', 'tsx', 'src/cli/index.ts', '--quiet', '--output', 'json', 'aigateway', ...args],
    {
      cwd: process.cwd(),
      env: process.env,
      maxBuffer: 10 * 1024 * 1024,
      timeout: 60_000,
    },
  );
  return JSON.parse(stdout) as T;
}

async function devWorkspace(): Promise<{ id: string; slug: string }> {
  const workspaces = await runJson<Array<{ id: string; slug: string }>>(
    'workspaces',
    'list',
    '--plane',
    'admin',
  );
  const workspace = workspaces.find((item) => item.slug === workspaceSlug);
  if (!workspace) throw new Error(`workspace ${workspaceSlug} must exist`);
  return workspace;
}

async function runJsonSeries(commands: string[][]): Promise<unknown[]> {
  const results: unknown[] = [];
  for (const command of commands) results.push(await runJson(...command));
  return results;
}

describe.skipIf(!hasCredentials)('AI Gateway CLI live E2E', () => {
  it('reads the dev workspace and every workspace-scoped collection', async () => {
    const workspace = await devWorkspace();

    const detail = await runJson<{ id: string }>(
      'workspaces',
      'get',
      workspaceSlug,
      '--plane',
      'admin',
    );
    expect(detail.id).toBe(workspace.id);

    const results = await runJsonSeries([
      ['api-keys', 'service', 'list', '--workspace', workspace.id],
      ['api-keys', 'user', 'list', '--workspace', workspace.id],
      ['configs', 'list', '--workspace', workspace.id],
      ['guardrails', 'list', '--workspace', workspace.id],
      ['providers', 'list', '--workspace', workspace.id],
    ]);
    for (const result of results) expect(Array.isArray(result)).toBe(true);
  }, 120_000);

  it('reads admin-plane inventory without changing tenant state', async () => {
    const results = await runJsonSeries([
      ['audit-logs', 'list', '--days', '1'],
      ['deployments', 'list'],
      ['integrations', 'list'],
      ['mcp', 'integrations', 'list'],
      ['organisations', 'self', 'get'],
      ['organisations', 'auth-settings', 'get', '--tsg-id', process.env.PANW_MGMT_TSG_ID ?? ''],
      ['plugins', 'list'],
    ]);
    expect(results).toHaveLength(7);
    expect(results.every((value) => value !== undefined && value !== null)).toBe(true);
  }, 120_000);

  it('reads representative telemetry endpoints for the dev workspace', async () => {
    const results = await runJsonSeries([
      ['telemetry', 'cache', 'summary', '--workspace', workspaceSlug, '--days', '1'],
      ['telemetry', 'errors', '--workspace', workspaceSlug, '--days', '1'],
      // A wider window includes known traffic. The SDK currently rejects the valid all-null
      // aggregate returned for an entirely empty latency window.
      ['telemetry', 'latency', '--workspace', workspaceSlug, '--days', '30'],
      ['telemetry', 'logs', 'list', '--workspace', workspaceSlug, '--days', '1'],
      ['telemetry', 'requests', '--workspace', workspaceSlug, '--days', '1'],
      ['telemetry', 'tokens', '--workspace', workspaceSlug, '--days', '1'],
    ]);
    expect(results).toHaveLength(6);
    expect(results.every((value) => value !== undefined && value !== null)).toBe(true);
  }, 120_000);

  it('creates, updates, verifies, and deletes one disposable config from structured flags', async () => {
    const workspace = await devWorkspace();
    const marker = randomUUID().slice(0, 8);
    let configId: string | undefined;

    try {
      const receipt = await runJson<{ id: string }>(
        'configs',
        'create',
        '--name',
        `cli-e2e-${marker}`,
        '--workspace',
        workspace.id,
        '--set',
        'config.retry.attempts=2',
      );
      configId = receipt.id;
      expect(configId).toBeTruthy();

      const created = await runJson<{ id: string; name: string }>('configs', 'get', configId);
      expect(created.name).toBe(`cli-e2e-${marker}`);

      await runJson(
        'configs',
        'update',
        configId,
        '--name',
        `cli-e2e-${marker}-updated`,
        '--set',
        'config.retry.attempts=3',
      );
      const updated = await runJson<{ name: string }>('configs', 'get', configId);
      expect(updated.name).toBe(`cli-e2e-${marker}-updated`);

      await runJson('configs', 'delete', configId, '--force');
      const remaining = await runJson<Array<{ id: string }>>(
        'configs',
        'list',
        '--workspace',
        workspace.id,
      );
      expect(remaining.some((item) => item.id === configId)).toBe(false);
      configId = undefined;
    } finally {
      if (configId) {
        await runJson('configs', 'delete', configId, '--force').catch(() => undefined);
      }
    }
  }, 120_000);
});
