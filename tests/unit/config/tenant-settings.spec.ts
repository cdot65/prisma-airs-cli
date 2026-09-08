import { chmod, mkdtemp, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import {
  createManagedTenant,
  isTenantSecret,
  setTenantSetting,
} from '../../../src/config/tenant-settings.js';
import { readTenantStore, tenantStorePath } from '../../../src/config/tenants.js';

let directory: string;
const config = { mgmtTsgId: '100', mgmtClientId: 'client-100', mgmtClientSecret: 'FAKE-SECRET' };
beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), 'airs-tenant-settings-'));
  vi.stubEnv('PRISMA_AIRS_TENANTS_PATH', join(directory, 'tenants.json'));
});
afterEach(async () => {
  vi.unstubAllEnvs();
  await rm(directory, { recursive: true, force: true });
});

it('creates private credentials separately from the registry without persisting defaults', async () => {
  const entry = await createManagedTenant('dev', config);
  expect(JSON.parse(await readFile(entry.configPath, 'utf8'))).toEqual(config);
  expect((await stat(entry.configPath)).mode & 0o777).toBe(0o600);
  expect((await stat(dirname(entry.configPath))).mode & 0o777).toBe(0o700);
  expect(await readFile(tenantStorePath(), 'utf8')).not.toContain('FAKE-SECRET');
  expect(readTenantStore().active).toBeNull();
});

it.each(['default', '../escape', ''])('rejects invalid name %j before writing', async (name) => {
  await expect(createManagedTenant(name, config)).rejects.toThrow('Tenant name');
  expect(await readdir(directory)).toEqual([]);
});

it.each([
  { ...config, mgmtTsgId: '' },
  { ...config, mgmtClientSecret: ' ' },
  { ...config, scanConcurrency: 100 },
  { ...config, unknown: 'FAKE-SECRET' },
])('rejects invalid configuration without writing', async (value) => {
  await expect(createManagedTenant('dev', value)).rejects.toThrow();
  expect(await readdir(directory)).toEqual([]);
});

it('refuses duplicate names without leaving a second config', async () => {
  const entry = await createManagedTenant('dev', config);
  await expect(createManagedTenant('dev', config)).rejects.toThrow('already exists');
  expect(await readdir(dirname(entry.configPath))).toHaveLength(1);
});

it('removes its new config if registration is locked', async () => {
  await writeFile(`${tenantStorePath()}.lock`, 'existing lock');
  await expect(createManagedTenant('dev', config)).rejects.toThrow(
    'registration was not completed',
  );
  expect(await readdir(join(directory, 'configs'))).toEqual([]);
  expect(await readFile(`${tenantStorePath()}.lock`, 'utf8')).toBe('existing lock');
});

it('sets and coerces one field, retaining unknown fields and existing secrets', async () => {
  const entry = await createManagedTenant('dev', config);
  await writeFile(entry.configPath, JSON.stringify({ ...config, extension: { enabled: true } }));
  await setTenantSetting('dev', 'scanConcurrency', '7');
  await setTenantSetting('dev', 'defaultOutput', 'yaml');
  const updated = JSON.parse(await readFile(entry.configPath, 'utf8'));
  expect(updated).toEqual({
    ...config,
    extension: { enabled: true },
    scanConcurrency: 7,
    defaultOutput: 'yaml',
  });
  expect((await stat(entry.configPath)).mode & 0o777).toBe(0o600);
  expect(readTenantStore().active).toBeNull();
  expect(await readdir(dirname(entry.configPath))).toHaveLength(1);
});

it.each([
  ['mgmtTsgId', '200'],
  ['mgmtClientId', ''],
  ['mgmtClientSecret', ' '],
  ['scanConcurrency', '0'],
  ['defaultOutput', 'FAKE-SECRET'],
  ['__proto__', 'x'],
  ['constructor', 'x'],
  ['unknown', 'FAKE-SECRET'],
])('rejects invalid update to %s without modifying anything or leaking input', async (key, value) => {
  const entry = await createManagedTenant('dev', config);
  const before = await readFile(entry.configPath);
  const error = await setTenantSetting('dev', key, value).catch((error: Error) => error);
  expect(error).toBeInstanceOf(Error);
  expect((error as Error).message).not.toContain('FAKE-SECRET');
  expect(await readFile(entry.configPath)).toEqual(before);
  expect(await readdir(dirname(entry.configPath))).toHaveLength(1);
});

it('refuses read-only files without bypassing their mode through atomic replacement', async () => {
  const entry = await createManagedTenant('dev', config);
  await chmod(entry.configPath, 0o400);
  await expect(setTenantSetting('dev', 'defaultOutput', 'json')).rejects.toThrow('writable');
  expect(JSON.parse(await readFile(entry.configPath, 'utf8'))).toEqual(config);
  expect((await stat(entry.configPath)).mode & 0o777).toBe(0o400);
});

it('refuses competing config writers and leaves their lock intact', async () => {
  const entry = await createManagedTenant('dev', config);
  await writeFile(`${entry.configPath}.lock`, 'another writer');
  await expect(setTenantSetting('dev', 'defaultOutput', 'json')).rejects.toThrow('Cannot lock');
  expect(await readFile(`${entry.configPath}.lock`, 'utf8')).toBe('another writer');
});

it('allows rotating credentials without exposing them in the registry', async () => {
  const entry = await createManagedTenant('dev', config);
  await setTenantSetting('dev', 'mgmtClientSecret', 'FAKE-SECRET-ROTATED');
  expect(JSON.parse(await readFile(entry.configPath, 'utf8')).mgmtClientSecret).toBe(
    'FAKE-SECRET-ROTATED',
  );
  expect(await readFile(tenantStorePath(), 'utf8')).not.toContain('FAKE-SECRET');
});

it('distinguishes credentials from token endpoints', () => {
  for (const key of ['airsApiKey', 'airsApiToken', 'mgmtClientSecret', 'aiGwInferenceApiKey'])
    expect(isTenantSecret(key)).toBe(true);
  for (const key of ['mgmtTokenEndpoint', 'mgmtClientId', 'defaultOutput'])
    expect(isTenantSecret(key)).toBe(false);
});
