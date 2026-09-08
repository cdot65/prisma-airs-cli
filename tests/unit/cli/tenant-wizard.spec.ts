import { mkdtemp, readdir, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Command } from 'commander';
import { registerTenantCommand } from '../../../src/cli/commands/tenant.js';
import { promptTenantValue } from '../../../src/cli/tenant-input.js';
import { readTenantStore } from '../../../src/config/tenants.js';

vi.mock('../../../src/cli/tenant-input.js', () => ({
  promptTenantValue: vi.fn(),
  readTenantStdin: vi.fn(),
}));
let directory: string;
let program: Command;
let exitCode: typeof process.exitCode;
beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), 'airs-tenant-wizard-'));
  vi.stubEnv('PRISMA_AIRS_TENANTS_PATH', join(directory, 'tenants.json'));
  exitCode = process.exitCode;
  program = new Command();
  registerTenantCommand(program);
});
afterEach(async () => {
  process.exitCode = exitCode;
  vi.unstubAllEnvs();
  vi.resetAllMocks();
  await rm(directory, { recursive: true, force: true });
});

it('collects three settings in order and hides only the secret prompt', async () => {
  vi.mocked(promptTenantValue)
    .mockResolvedValueOnce('100')
    .mockResolvedValueOnce('client-100')
    .mockResolvedValueOnce('FAKE-SECRET');
  await program.parseAsync(['tenant', 'create', 'dev'], { from: 'user' });
  expect(vi.mocked(promptTenantValue).mock.calls).toEqual([
    ['Tenant service group ID (mgmtTsgId):'],
    ['OAuth client ID (mgmtClientId):'],
    ['OAuth client secret (mgmtClientSecret):', true],
  ]);
  const store = readTenantStore();
  expect(store.active).toBeNull();
  expect(JSON.parse(await readFile(store.tenants[0].configPath, 'utf8'))).toEqual({
    mgmtTsgId: '100',
    mgmtClientId: 'client-100',
    mgmtClientSecret: 'FAKE-SECRET',
  });
});

it.each([
  0, 1, 2,
])('cancels safely at prompt %i without saving files or a partial tenant', async (index) => {
  for (let i = 0; i < index; i++) vi.mocked(promptTenantValue).mockResolvedValueOnce('100');
  vi.mocked(promptTenantValue).mockRejectedValueOnce(
    Object.assign(new Error('cancelled'), { name: 'ExitPromptError' }),
  );
  await program.parseAsync(['tenant', 'create', 'dev'], { from: 'user' });
  expect(process.exitCode).toBe(130);
  expect(await readdir(directory)).toEqual([]);
});
