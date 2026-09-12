import { readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { type Config, ConfigSchema } from './schema.js';
import {
  readTenantConfigFile,
  readTenantStore,
  type TenantEntry,
  tenantStorePath,
} from './tenants.js';

function expandHome(p: string): string {
  return p.startsWith('~') ? join(homedir(), p.slice(1)) : p;
}

async function fromFile(path: string): Promise<Record<string, unknown>> {
  try {
    const data = await readFile(path, 'utf-8');
    return JSON.parse(data);
  } catch {
    return {};
  }
}

function stripUndefined(obj: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(Object.entries(obj).filter(([, v]) => v !== undefined && v !== ''));
}

/**
 * Where configuration comes from. `explicit` is the library API (a caller-supplied
 * file); the CLI always resolves through the tenant registry.
 */
export type ConfigContext =
  | { selection: 'explicit'; path: string; registryPath: string }
  | { selection: 'tenant'; path: string; tenant: TenantEntry; registryPath: string }
  | { selection: 'none'; registryPath: string; registered: string[] };

export function noTenantSelectedMessage(registered: string[]): string {
  return registered.length
    ? `No tenant selected. Run 'airs tenant switch <name>' (registered: ${registered.join(', ')})`
    : "No tenant selected. Run 'airs tenant create <name>' and then 'airs tenant switch <name>'";
}

export class NoTenantSelectedError extends Error {
  constructor(registered: string[]) {
    super(noTenantSelectedMessage(registered));
    this.name = 'NoTenantSelectedError';
  }
}

/** Resolve the config source: explicit param, else the selected tenant, else none. */
export function resolveConfigContext(configFilePath?: string): ConfigContext {
  const registryPath = tenantStorePath();
  if (configFilePath) {
    return { path: expandHome(configFilePath), selection: 'explicit', registryPath };
  }
  const store = readTenantStore();
  const tenant = store.tenants.find((entry) => entry.name === store.active);
  if (tenant) return { path: tenant.configPath, selection: 'tenant', tenant, registryPath };
  return { selection: 'none', registryPath, registered: store.tenants.map((entry) => entry.name) };
}

/** Config file path in force; throws when no tenant is selected. */
export function resolveConfigFilePath(configFilePath?: string): string {
  const context = resolveConfigContext(configFilePath);
  if (context.selection === 'none') throw new NoTenantSelectedError(context.registered);
  return context.path;
}

async function readConfigSource(configFilePath?: string): Promise<Record<string, unknown>> {
  const context = resolveConfigContext(configFilePath);
  if (context.selection === 'none') throw new NoTenantSelectedError(context.registered);
  if (context.selection === 'explicit') return fromFile(context.path);
  return readTenantConfigFile(context.path, context.tenant.tsgId);
}

/**
 * Load the effective config: CLI overrides > selected tenant file > defaults.
 * The environment is never consulted.
 */
export async function loadConfig(
  cliOverrides: Record<string, unknown> = {},
  configFilePath?: string,
): Promise<Config> {
  const fileConfig = await readConfigSource(configFilePath);
  const merged = { ...stripUndefined(fileConfig), ...stripUndefined(cliOverrides) };
  const config = ConfigSchema.parse(merged);
  return { ...config, dataDir: expandHome(config.dataDir) };
}

export type ConfigSource = 'file' | 'default';

export interface ConfigEntry {
  value: unknown;
  source: ConfigSource;
}

/** Effective config (file > defaults, no CLI overrides) with per-key source tracking. */
export async function inspectConfig(configFilePath?: string): Promise<Record<string, ConfigEntry>> {
  const fileConfig = stripUndefined(await readConfigSource(configFilePath));
  const config = ConfigSchema.parse(fileConfig) as Record<string, unknown>;
  const entries: Record<string, ConfigEntry> = {};
  for (const key of Object.keys(ConfigSchema.shape)) {
    entries[key] = { value: config[key], source: key in fileConfig ? 'file' : 'default' };
  }
  return entries;
}
