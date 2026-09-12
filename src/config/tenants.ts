import { randomUUID } from 'node:crypto';
import { lstatSync, readFileSync, realpathSync } from 'node:fs';
import { type FileHandle, mkdir, open, rename, unlink } from 'node:fs/promises';
import { homedir } from 'node:os';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { z } from 'zod';
import { type Config, ConfigSchema } from './schema.js';

const NameSchema = z.string().regex(/^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$/);
const EntrySchema = z
  .object({
    name: NameSchema,
    configPath: z.string().refine(isAbsolute),
    tsgId: z.string().min(1),
  })
  .strict();
const StoreSchema = z
  .object({
    version: z.literal(1),
    active: z.string().nullable(),
    tenants: z.array(EntrySchema).max(1000),
  })
  .strict()
  .superRefine((value, ctx) => {
    const names = new Set(value.tenants.map((entry) => entry.name));
    if (names.size !== value.tenants.length || (value.active !== null && !names.has(value.active)))
      ctx.addIssue({ code: z.ZodIssueCode.custom, message: 'Invalid tenant registry identities' });
  });

export type TenantEntry = z.infer<typeof EntrySchema>;
export type TenantStore = z.infer<typeof StoreSchema>;

/** Registry stores references, never credentials; separate from read-only secret mounts. */
export function tenantStorePath(): string {
  const explicit = process.env.PRISMA_AIRS_TENANTS_PATH;
  if (explicit) return resolve(expandConfigPath(explicit));
  const state = process.env.XDG_STATE_HOME;
  if (state && !isAbsolute(state)) throw new Error('XDG_STATE_HOME must be an absolute path');
  return join(state || join(homedir(), '.local', 'state'), 'prisma-airs', 'tenants.json');
}

export function expandConfigPath(path: string): string {
  return path === '~' ? homedir() : path.startsWith('~/') ? join(homedir(), path.slice(2)) : path;
}

function missing(error: unknown): boolean {
  return (error as NodeJS.ErrnoException)?.code === 'ENOENT';
}

/** A missing registry simply has no tenants; corrupt registries fail closed. */
export function readTenantStore(): TenantStore {
  const path = tenantStorePath();
  try {
    const stat = lstatSync(path);
    if (!stat.isFile() || stat.size > 1_048_576) throw new Error();
    return StoreSchema.parse(JSON.parse(readFileSync(path, 'utf8')));
  } catch (error) {
    if (missing(error)) return { version: 1, active: null, tenants: [] };
    throw new Error(
      `Cannot read tenant registry at ${path}; restore a valid registry before proceeding`,
    );
  }
}

export function selectedTenant(): TenantEntry | undefined {
  const store = readTenantStore();
  return store.tenants.find((entry) => entry.name === store.active);
}

/** Parse without leaking JSON fragments or secret values through validation errors. */
export function readTenantConfig(path: string, expectedTsgId?: string): Config {
  return ConfigSchema.parse(readTenantConfigFile(path, expectedTsgId));
}

export function readTenantConfigFile(
  path: string,
  expectedTsgId?: string,
): Record<string, unknown> {
  let config: Config;
  let parsed: Record<string, unknown>;
  try {
    const stat = lstatSync(realpathSync(path));
    if (!stat.isFile() || stat.size > 1_048_576) throw new Error();
    parsed = JSON.parse(readFileSync(path, 'utf8'));
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) throw new Error();
    config = ConfigSchema.parse(parsed);
  } catch {
    throw new Error(`Cannot read a valid tenant config at ${path}`);
  }
  if (expectedTsgId !== undefined && config.mgmtTsgId !== expectedTsgId)
    throw new Error(
      'Tenant config TSG differs from its registered identity; register it under a new name',
    );
  return parsed;
}

async function changeStore(change: (store: TenantStore) => void): Promise<TenantStore> {
  const path = tenantStorePath();
  await mkdir(dirname(path), { recursive: true, mode: 0o700 });
  const lockPath = `${path}.lock`;
  let lock: FileHandle;
  try {
    lock = await open(lockPath, 'wx', 0o600);
  } catch {
    throw new Error(
      `Cannot lock tenant registry at ${lockPath}; another change may be in progress`,
    );
  }
  const temp = join(dirname(path), `.airs-tenants-${randomUUID()}.tmp`);
  let created = false;
  try {
    const store = readTenantStore();
    change(store);
    StoreSchema.parse(store);
    const file = await open(temp, 'wx', 0o600);
    created = true;
    try {
      await file.writeFile(`${JSON.stringify(store, null, 2)}\n`);
      await file.sync();
    } finally {
      await file.close();
    }
    await rename(temp, path);
    created = false;
    return store;
  } finally {
    if (created) await unlink(temp).catch(() => {});
    await lock.close();
    await unlink(lockPath);
  }
}

/** Validate names before prompting or creating local files. */
export function validateTenantName(name: string): void {
  if (!NameSchema.safeParse(name).success)
    throw new Error(
      'Tenant name must be 1–64 letters, digits, hyphens or underscores and start with a letter/digit',
    );
}

/** Register an existing config by reference. The source file is never copied or changed. */
export async function createTenant(name: string, configPath: string): Promise<TenantEntry> {
  validateTenantName(name);
  let path: string;
  try {
    path = realpathSync(resolve(expandConfigPath(configPath)));
  } catch {
    throw new Error('Tenant config file does not exist or cannot be resolved');
  }
  const config = readTenantConfig(path);
  if (!config.mgmtTsgId?.trim() || !config.mgmtClientId?.trim() || !config.mgmtClientSecret?.trim())
    throw new Error('Tenant config requires mgmtTsgId, mgmtClientId and mgmtClientSecret');
  const entry = { name, configPath: path, tsgId: config.mgmtTsgId };
  await changeStore((store) => {
    if (store.tenants.some((tenant) => tenant.name === name))
      throw new Error('Tenant name already exists');
    store.tenants.push(entry);
    store.tenants.sort((a, b) => a.name.localeCompare(b.name));
  });
  return entry;
}

/** Switch only the registry pointer after validating the target file against its pinned TSG. */
export async function switchTenant(name: string): Promise<TenantEntry> {
  let selected: TenantEntry | undefined;
  await changeStore((store) => {
    selected = store.tenants.find((entry) => entry.name === name);
    if (!selected) throw new Error('Tenant not found');
    readTenantConfig(selected.configPath, selected.tsgId);
    store.active = name;
  });
  return selected as TenantEntry;
}

/**
 * Unregister only; source files are retained. Deleting the selected tenant clears the
 * selection, after which every API command asks for a tenant until one is switched to.
 */
export async function deleteTenant(name: string): Promise<{ selectionCleared: boolean }> {
  let selectionCleared = false;
  await changeStore((store) => {
    if (!store.tenants.some((entry) => entry.name === name)) throw new Error('Tenant not found');
    store.tenants = store.tenants.filter((entry) => entry.name !== name);
    if (store.active === name) {
      store.active = null;
      selectionCleared = true;
    }
  });
  return { selectionCleared };
}
