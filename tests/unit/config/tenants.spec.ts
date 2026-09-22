import { chmod, mkdtemp, readFile, rm, stat, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { inspectConfig, loadConfig, resolveConfigFilePath } from '../../../src/config/loader.js';
import {
  createTenant,
  deleteTenant,
  expandConfigPath,
  readTenantConfig,
  readTenantStore,
  selectedTenant,
  switchTenant,
  tenantStorePath,
} from '../../../src/config/tenants.js';

let directory: string;
let source: string;
let destination: string;
const config = (tsg: string) => ({
  mgmtTsgId: tsg,
  mgmtClientId: `client-${tsg}`,
  mgmtClientSecret: `SECRET-${tsg}`,
  scanConcurrency: 3,
});

beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), 'airs-tenants-test-'));
  source = join(directory, 'source.json');
  destination = join(directory, 'destination.json');
  for (const key of Object.keys(process.env))
    if (key.startsWith('PANW_')) vi.stubEnv(key, undefined);
  vi.stubEnv('PRISMA_AIRS_CONFIG_PATH', undefined);
  vi.stubEnv('PRISMA_AIRS_TENANTS_PATH', join(directory, 'state', 'tenants.json'));
  await writeFile(source, JSON.stringify(config('100')), { mode: 0o400 });
  await writeFile(destination, JSON.stringify(config('200')), { mode: 0o400 });
});

afterEach(async () => {
  vi.unstubAllEnvs();
  await rm(directory, { recursive: true, force: true });
});

describe('tenant registration and isolation', () => {
  it('has no tenants and no config source without a registry', () => {
    expect(readTenantStore()).toEqual({ version: 1, active: null, tenants: [] });
    expect(selectedTenant()).toBeUndefined();
    expect(() => resolveConfigFilePath()).toThrow('No tenant selected');
  });

  it('registers and switches read-only files without copying their secrets', async () => {
    const before = await readFile(source);
    await createTenant('source', source);
    await createTenant('destination', destination);
    expect(selectedTenant()).toBeUndefined();
    await switchTenant('source');
    expect((await loadConfig()).mgmtTsgId).toBe('100');
    await switchTenant('destination');
    expect((await loadConfig()).mgmtTsgId).toBe('200');
    expect((await loadConfig()).mgmtClientSecret).toBe('SECRET-200');
    expect((await inspectConfig()).scanConcurrency.source).toBe('file');
    expect((await inspectConfig()).dataDir.source).toBe('default');
    expect(await readFile(source)).toEqual(before);
    expect(await readFile(tenantStorePath(), 'utf8')).not.toContain('SECRET');
    expect((await stat(tenantStorePath())).mode & 0o777).toBe(0o600);
    expect((await stat(join(directory, 'state'))).mode & 0o777).toBe(0o700);
    expect(resolveConfigFilePath()).toBe(destination);
  });

  it('ignores every environment variable while a tenant is selected', async () => {
    await createTenant('source', source);
    await switchTenant('source');
    vi.stubEnv('PANW_MGMT_TSG_ID', '200');
    vi.stubEnv('PANW_MGMT_CLIENT_SECRET', 'ENV-LEAK');
    vi.stubEnv('PANW_CLI_OUTPUT', 'yaml');
    vi.stubEnv('PRISMA_AIRS_CONFIG_PATH', destination);
    const config = await loadConfig();
    expect(config.mgmtTsgId).toBe('100');
    expect(config.mgmtClientSecret).toBe('SECRET-100');
    expect(config.defaultOutput).toBeUndefined();
    expect((await loadConfig({}, destination)).mgmtTsgId).toBe('200');
  });

  it('never deletes the referenced config and clears the selection when deleting the selected tenant', async () => {
    await createTenant('source', source);
    await createTenant('destination', destination);
    await switchTenant('source');
    expect(await deleteTenant('destination')).toEqual({ selectionCleared: false });
    expect(readTenantStore().active).toBe('source');
    expect(await deleteTenant('source')).toEqual({ selectionCleared: true });
    expect(readTenantStore()).toEqual({ version: 1, active: null, tenants: [] });
    expect(await readFile(source, 'utf8')).toContain('SECRET-100');
    await expect(loadConfig()).rejects.toThrow('No tenant selected');
    await expect(deleteTenant('absent')).rejects.toThrow('not found');
  });

  it.each([
    '',
    '../escape',
    'a/b',
    'space name',
    '-bad',
    'a'.repeat(65),
  ])('rejects invalid name %j', async (name) => {
    await expect(createTenant(name, source)).rejects.toThrow('Tenant name');
    expect(selectedTenant()).toBeUndefined();
  });

  it('rejects duplicates without changing the registry', async () => {
    await createTenant('source', source);
    const before = await readFile(tenantStorePath());
    await expect(createTenant('source', destination)).rejects.toThrow('already exists');
    expect(await readFile(tenantStorePath())).toEqual(before);
  });

  it('resolves config symlinks to their original target', async () => {
    const link = join(directory, 'linked.json');
    await symlink(source, link);
    expect((await createTenant('source', link)).configPath).toBe(source);
  });

  it('fails closed if the selected config disappears', async () => {
    await createTenant('source', source);
    await switchTenant('source');
    await rm(source);
    await expect(loadConfig()).rejects.toThrow('valid tenant config');
    expect(readTenantStore().active).toBe('source');
    await deleteTenant('source');
    expect(readTenantStore().active).toBeNull();
  });

  it('pins the tenant identity and rejects changed TSGs', async () => {
    await createTenant('source', source);
    await chmod(source, 0o600);
    await writeFile(source, JSON.stringify(config('200')));
    await expect(switchTenant('source')).rejects.toThrow('registered identity');
    expect(readTenantStore().active).toBeNull();
  });

  it('keeps the prior selection after a failed switch', async () => {
    await createTenant('source', source);
    await switchTenant('source');
    await expect(switchTenant('absent')).rejects.toThrow('not found');
    expect(readTenantStore().active).toBe('source');
  });

  it.each([
    'null',
    '[]',
    '{"mgmtClientSecret":"SUPER-SECRET",oops',
    '{"scanConcurrency":99}',
  ])('rejects malformed configs without echoing contents', async (body) => {
    const path = join(directory, 'bad.json');
    await writeFile(path, body);
    expect(() => readTenantConfig(path)).toThrow('valid tenant config');
    expect(() => readTenantConfig(path)).not.toThrow('SUPER-SECRET');
  });

  it('requires management credentials at registration', async () => {
    const path = join(directory, 'empty.json');
    await writeFile(path, '{}');
    await expect(createTenant('empty', path)).rejects.toThrow('requires mgmtTsgId');
    await expect(createTenant('missing', join(directory, 'missing'))).rejects.toThrow(
      'cannot be resolved',
    );
  });

  it('refuses registry symlinks and malformed registries', async () => {
    vi.stubEnv('PRISMA_AIRS_TENANTS_PATH', join(directory, 'registry.json'));
    await symlink(source, tenantStorePath());
    expect(() => readTenantStore()).toThrow('Cannot read tenant registry');
    await rm(tenantStorePath());
    await writeFile(
      tenantStorePath(),
      JSON.stringify({ version: 1, active: 'missing', tenants: [] }),
    );
    expect(() => readTenantStore()).toThrow('Cannot read tenant registry');
  });

  it('refuses concurrent changes without stealing a lock', async () => {
    await createTenant('source', source);
    await writeFile(`${tenantStorePath()}.lock`, 'existing');
    await expect(switchTenant('source')).rejects.toThrow('Cannot lock');
    expect(await readFile(`${tenantStorePath()}.lock`, 'utf8')).toBe('existing');
  });

  it('supports XDG state and tilde expansion', () => {
    vi.stubEnv('PRISMA_AIRS_TENANTS_PATH', undefined);
    vi.stubEnv('XDG_STATE_HOME', directory);
    expect(tenantStorePath()).toBe(join(directory, 'prisma-airs', 'tenants.json'));
    expect(expandConfigPath('~/config.json')).toMatch(/\/config.json$/);
    expect(expandConfigPath('~someone/config.json')).toBe('~someone/config.json');
    vi.stubEnv('XDG_STATE_HOME', 'relative');
    expect(() => tenantStorePath()).toThrow('absolute');
  });
});
