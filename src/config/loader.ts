import { readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { type Config, ConfigSchema } from './schema.js';

function expandHome(p: string): string {
  return p.startsWith('~') ? join(homedir(), p.slice(1)) : p;
}

function fromEnv(): Record<string, unknown> {
  const env = process.env;
  return {
    airsApiKey: env.PANW_AI_SEC_API_KEY,
    airsApiToken: env.PANW_AI_SEC_API_TOKEN,
    airsApiEndpoint: env.PANW_AI_SEC_API_ENDPOINT,
    airsNumRetries: env.PANW_AI_SEC_NUM_RETRIES,
    mgmtClientId: env.PANW_MGMT_CLIENT_ID,
    mgmtClientSecret: env.PANW_MGMT_CLIENT_SECRET,
    mgmtTsgId: env.PANW_MGMT_TSG_ID,
    mgmtEndpoint: env.PANW_MGMT_ENDPOINT,
    mgmtTokenEndpoint: env.PANW_MGMT_TOKEN_ENDPOINT,
    dlpEndpoint: env.PANW_DLP_ENDPOINT,
    redTeamDataEndpoint: env.PANW_RED_TEAM_DATA_ENDPOINT,
    redTeamMgmtEndpoint: env.PANW_RED_TEAM_MGMT_ENDPOINT,
    redTeamTokenEndpoint: env.PANW_RED_TEAM_TOKEN_ENDPOINT,
    modelSecDataEndpoint: env.PANW_MODEL_SEC_DATA_ENDPOINT,
    modelSecMgmtEndpoint: env.PANW_MODEL_SEC_MGMT_ENDPOINT,
    modelSecTokenEndpoint: env.PANW_MODEL_SEC_TOKEN_ENDPOINT,
    scanConcurrency: env.SCAN_CONCURRENCY,
    dataDir: env.DATA_DIR,
  };
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
 * Resolve the config file path: explicit param > PRISMA_AIRS_CONFIG_PATH env
 * var > ~/.prisma-airs/config.json.
 */
export function resolveConfigFilePath(configFilePath?: string): string {
  if (configFilePath) return configFilePath;
  const envPath = process.env.PRISMA_AIRS_CONFIG_PATH;
  if (envPath) return expandHome(envPath);
  return join(homedir(), '.prisma-airs', 'config.json');
}

export async function loadConfig(
  cliOverrides: Record<string, unknown> = {},
  configFilePath?: string,
): Promise<Config> {
  const filePath = resolveConfigFilePath(configFilePath);
  const fileConfig = await fromFile(filePath);
  const envConfig = fromEnv();

  // Priority: CLI > env > file > defaults
  const merged = {
    ...stripUndefined(fileConfig),
    ...stripUndefined(envConfig),
    ...stripUndefined(cliOverrides),
  };

  const config = ConfigSchema.parse(merged);

  return {
    ...config,
    dataDir: expandHome(config.dataDir),
  };
}

export type ConfigSource = 'env' | 'file' | 'default';

export interface ConfigEntry {
  value: unknown;
  source: ConfigSource;
}

/**
 * Compute the effective config (env > file > defaults — no CLI overrides)
 * with per-key source tracking. Keys are exactly the ConfigSchema keys.
 */
export async function inspectConfig(configFilePath?: string): Promise<Record<string, ConfigEntry>> {
  const filePath = resolveConfigFilePath(configFilePath);
  const fileConfig = stripUndefined(await fromFile(filePath));
  const envConfig = stripUndefined(fromEnv());

  const merged = { ...fileConfig, ...envConfig };
  const config = ConfigSchema.parse(merged) as Record<string, unknown>;

  const entries: Record<string, ConfigEntry> = {};
  for (const key of Object.keys(ConfigSchema.shape)) {
    const source: ConfigSource = key in envConfig ? 'env' : key in fileConfig ? 'file' : 'default';
    entries[key] = { value: config[key], source };
  }
  return entries;
}
