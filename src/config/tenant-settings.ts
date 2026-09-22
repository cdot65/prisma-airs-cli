import { randomUUID } from 'node:crypto';
import { constants } from 'node:fs';
import { access, lstat, mkdir, open, rename, unlink } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { ConfigSchema } from './schema.js';
import {
  createTenant,
  readTenantConfigFile,
  readTenantStore,
  tenantStorePath,
  validateTenantName,
} from './tenants.js';

export const TENANT_CONFIG_KEYS = Object.keys(ConfigSchema.shape);

export function validateTenantSettingKey(key: string): void {
  if (!Object.hasOwn(ConfigSchema.shape, key))
    throw new Error(`Unknown configuration key. Supported keys: ${TENANT_CONFIG_KEYS.join(', ')}`);
}

export function isTenantSecret(key: string): boolean {
  return /(?:key|secret|token|password)$/i.test(key);
}

function validateCredentials(config: Record<string, unknown>): void {
  for (const key of ['mgmtTsgId', 'mgmtClientId', 'mgmtClientSecret']) {
    if (typeof config[key] !== 'string' || !config[key].trim())
      throw new Error(
        'Tenant config requires nonempty mgmtTsgId, mgmtClientId and mgmtClientSecret',
      );
  }
}

/** Create a private config only after all input has been collected and validated. */
export async function createManagedTenant(name: string, config: Record<string, unknown>) {
  validateTenantName(name);
  for (const key of Object.keys(config)) validateTenantSettingKey(key);
  const parsed = ConfigSchema.safeParse(config);
  if (!parsed.success) throw new Error('Invalid tenant configuration');
  validateCredentials(config);
  if (readTenantStore().tenants.some((entry) => entry.name === name))
    throw new Error('Tenant name already exists');
  const directory = join(dirname(tenantStorePath()), 'configs');
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const path = join(directory, `${name}-${randomUUID()}.json`);
  const file = await open(path, 'wx', 0o600);
  try {
    // Persist explicit settings only; do not freeze schema defaults into the file.
    const data = Object.fromEntries(
      Object.keys(config).map((key) => [key, Reflect.get(parsed.data, key)]),
    );
    await file.writeFile(`${JSON.stringify(data, null, 2)}\n`);
    await file.sync();
    await file.close();
    return await createTenant(name, path);
  } catch {
    await file.close();
    await unlink(path);
    throw new Error('Could not create tenant configuration; registration was not completed');
  }
}

/** Credential keys can be rotated but never removed from a registered tenant. */
export const CREDENTIAL_KEYS = ['mgmtTsgId', 'mgmtClientId', 'mgmtClientSecret'] as const;

/** Explicitly edit one registered tenant, preserving its identity and unrelated settings. */
export async function setTenantSetting(name: string, key: string, value: string): Promise<void> {
  validateTenantSettingKey(key);
  const entry = findTenant(name);
  if (key === 'mgmtTsgId' && value !== entry.tsgId)
    throw new Error('TSG identity is pinned; create another tenant instead of changing mgmtTsgId');
  await rewriteTenantConfig(entry, (current) => {
    const result = ConfigSchema.safeParse({ ...current, [key]: value });
    if (!result.success) throw new Error('Invalid value for configuration setting');
    return { ...current, [key]: Reflect.get(result.data, key) };
  });
}

/**
 * Remove one setting so the schema default (or nothing) applies again. Returns false
 * when the key was not present. Credentials cannot be cleared.
 */
export async function unsetTenantSetting(name: string, key: string): Promise<boolean> {
  validateTenantSettingKey(key);
  if ((CREDENTIAL_KEYS as readonly string[]).includes(key))
    throw new Error(`${key} is a credential and cannot be cleared; set a new value instead`);
  const entry = findTenant(name);
  let removed = false;
  await rewriteTenantConfig(entry, (current) => {
    if (!Object.hasOwn(current, key)) return current;
    removed = true;
    const { [key]: _dropped, ...rest } = current;
    return rest;
  });
  return removed;
}

function findTenant(name: string) {
  const entry = readTenantStore().tenants.find((tenant) => tenant.name === name);
  if (!entry) throw new Error('Tenant not found; register a named tenant first');
  return entry;
}

/** Locked, atomic rewrite of one tenant file; the change callback receives the raw current JSON. */
async function rewriteTenantConfig(
  entry: { configPath: string; tsgId: string },
  change: (current: Record<string, unknown>) => Record<string, unknown>,
): Promise<void> {
  const path = entry.configPath;
  const lockPath = `${path}.lock`;
  const lock = await open(lockPath, 'wx', 0o600).catch(() => {
    throw new Error(
      'Cannot lock tenant config; it may be read-only or another edit is in progress',
    );
  });
  const temporary = join(dirname(path), `.airs-config-${randomUUID()}.tmp`);
  let created = false;
  try {
    const stat = await lstat(path);
    // Atomic replacement must not bypass read-only mounts/files, even when run as root.
    if (!stat.isFile() || (stat.mode & 0o222) === 0)
      throw new Error('Tenant config is not a writable regular file');
    await access(path, constants.W_OK);
    const current = readTenantConfigFile(path, entry.tsgId);
    const next = change(current);
    validateCredentials(next);
    if (next === current) return;
    const file = await open(temporary, 'wx', 0o600);
    created = true;
    try {
      await file.writeFile(`${JSON.stringify(next, null, 2)}\n`);
      await file.sync();
    } finally {
      await file.close();
    }
    await rename(temporary, path);
    created = false;
  } finally {
    if (created) await unlink(temporary).catch(() => {});
    await lock.close();
    await unlink(lockPath);
  }
}
