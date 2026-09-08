import { randomUUID } from 'node:crypto';
import { lstatSync, readFileSync, realpathSync } from 'node:fs';
import { type FileHandle, mkdir, open, rename, unlink } from 'node:fs/promises';
import { homedir } from 'node:os';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { z } from 'zod';
import { type Config, ConfigSchema } from './schema.js';

const NameSchema = z
  .string()
  .regex(/^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$/)
  .refine((v) => v !== 'default');
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

export function defaultTenantConfigPath(): string {
  return join(homedir(), '.prisma-airs', 'config.json');
}

function missing(error: unknown): boolean {
  return (error as NodeJS.ErrnoException)?.code === 'ENOENT';
}

/** Missing registry means legacy/default behavior; corrupt registries fail closed. */
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

/** Prevent mixed credentials/endpoints when using a named tenant. */
export function assertTenantEnvironment(): void {
  const conflicts = Object.keys(process.env).filter(
    (key) =>
      /^PANW_(?:MGMT_|MODEL_SEC_|RED_TEAM_|AGENT_GUARD_|AI_GW_|DLP_|AI_SEC_API_)/.test(key) &&
      Boolean(process.env[key]),
  );
  if (conflicts.length)
    throw new Error(
      `Named tenant selection conflicts with environment overrides: ${conflicts.sort().join(', ')}. Unset them or switch to default.`,
    );
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

/** Register an existing config by reference. The source file is never copied or changed. */
export async function createTenant(name: string, configPath: string): Promise<TenantEntry> {
  if (!NameSchema.safeParse(name).success)
    throw new Error(
      'Tenant name must be 1–64 letters, digits, hyphens or underscores, start with a letter/digit, and not be default',
    );
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

/** Switch only the registry pointer; default restores legacy config/environment resolution. */
export async function switchTenant(name: string): Promise<TenantEntry | undefined> {
  let selected: TenantEntry | undefined;
  await changeStore((store) => {
    if (name === 'default') {
      store.active = null;
      return;
    }
    if (process.env.PRISMA_AIRS_CONFIG_PATH)
      throw new Error(
        'Unset PRISMA_AIRS_CONFIG_PATH before switching tenants; it overrides named selection',
      );
    assertTenantEnvironment();
    selected = store.tenants.find((entry) => entry.name === name);
    if (!selected) throw new Error('Tenant not found');
    readTenantConfig(selected.configPath, selected.tsgId);
    store.active = name;
  });
  return selected;
}

/** Unregister only; deleting the active entry is refused and source files are retained. */
export async function deleteTenant(name: string): Promise<void> {
  await changeStore((store) => {
    if (name === 'default') throw new Error('The default tenant cannot be deleted');
    if (store.active === name)
      throw new Error('Switch to another tenant or default before deleting the active tenant');
    if (!store.tenants.some((entry) => entry.name === name)) throw new Error('Tenant not found');
    store.tenants = store.tenants.filter((entry) => entry.name !== name);
  });
}
