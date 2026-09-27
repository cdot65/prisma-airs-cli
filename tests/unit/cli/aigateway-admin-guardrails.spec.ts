import type { AIGatewayClient } from '@cdot65/prisma-airs-sdk';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setAiGatewayClientFactoryForTest } from '../../../src/cli/commands/aigateway/shared.js';
import { buildProgram } from '../../../src/cli/program.js';
import { useTestTenant } from '../../helpers/tenant.js';

const id = '16f7e90d-382a-4e78-b577-1b01eb5f8297';
const methods = {
  list: vi.fn(),
  get: vi.fn(),
  create: vi.fn(),
  update: vi.fn(),
  delete: vi.fn(),
  getMcpServers: vi.fn(),
  syncMcpServers: vi.fn(),
  upsertMcpServer: vi.fn(),
};
const factory = vi.fn(async () => ({ adminGuardrails: methods }) as unknown as AIGatewayClient);
let restore: () => void;
beforeEach(async () => {
  vi.clearAllMocks();
  await useTestTenant();
  for (const method of Object.values(methods)) method.mockResolvedValue({ data: [] });
  restore = setAiGatewayClientFactoryForTest(factory);
  vi.spyOn(console, 'log').mockImplementation(() => {});
  vi.spyOn(console, 'error').mockImplementation(() => {});
  vi.spyOn(process, 'exit').mockImplementation((code) => {
    throw new Error(`process.exit(${code})`);
  });
});
afterEach(() => {
  restore();
  vi.restoreAllMocks();
});
async function run(...args: string[]) {
  await buildProgram().parseAsync([
    'node',
    'airs-cli',
    'aigateway',
    'admin-guardrails',
    ...args,
    '--output',
    'json',
  ]);
}

describe('admin guardrails commands', () => {
  it('lists without requiring workspace and preserves explicit page zero', async () => {
    await run('list', '--page-size', '100', '--current-page', '0');
    expect(methods.list).toHaveBeenCalledWith({
      workspaceId: undefined,
      pageSize: 100,
      currentPage: 0,
    });
  });
  it('reads by id and never invokes a workspace client', async () => {
    await run('get', id);
    expect(methods.get).toHaveBeenCalledWith(id);
    await run('mcp-servers', 'list', id);
    expect(methods.getMcpServers).toHaveBeenCalledWith(id);
  });
  it('creates organisation-scoped input and supports named flags', async () => {
    await run('create', '--name', 'Policy', '--target', 'mcp_tools', '--organisation', id);
    expect(methods.create).toHaveBeenCalledWith({
      name: 'Policy',
      target: 'mcp_tools',
      organisation_id: id,
    });
  });
  it('rejects invalid create and mapping input before constructing an authenticated client', async () => {
    await expect(run('create', '--name', 'Policy', '--organisation', '123')).rejects.toThrow(
      'process.exit(2)',
    );
    await expect(
      run('mcp-servers', 'upsert', id, id, '--set', 'run_on=["invalid"]'),
    ).rejects.toThrow('process.exit(2)');
    expect(factory).not.toHaveBeenCalled();
  });
  it('updates and upserts through strict structured requests', async () => {
    await run('update', id, '--name', 'Updated');
    expect(methods.update).toHaveBeenCalledWith(id, { name: 'Updated' });
    await run('mcp-servers', 'upsert', id, id, '--set', 'run_on=["input"]');
    expect(methods.upsertMcpServer).toHaveBeenCalledWith(id, id, { run_on: ['input'] });
  });
  it('requires confirmation before deletion or full mapping replacement', async () => {
    await expect(run('delete', id)).rejects.toThrow();
    await expect(run('mcp-servers', 'sync', id, '--set', 'mcp_servers={}')).rejects.toThrow();
    expect(methods.delete).not.toHaveBeenCalled();
    expect(methods.syncMcpServers).not.toHaveBeenCalled();
    await run('delete', id, '--force');
    expect(methods.delete).toHaveBeenCalledWith(id);
    await run('mcp-servers', 'sync', id, '--set', 'mcp_servers={}', '--force');
    expect(methods.syncMcpServers).toHaveBeenCalledWith(id, { mcp_servers: {} });
  });
  it.each([
    ['--page-size', '0'],
    ['--page-size', '1001'],
    ['--current-page', '-1'],
  ])('rejects out-of-range %s before credential loading', async (flag, value) => {
    await expect(run('list', flag, value)).rejects.toThrow();
    expect(factory).not.toHaveBeenCalled();
  });
  it('rejects malformed integer flags without loading credentials', async () => {
    await expect(run('list', '--current-page', '1.5')).rejects.toThrow();
    expect(factory).not.toHaveBeenCalled();
  });
});
