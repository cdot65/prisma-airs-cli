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
    llmProvider: env.LLM_PROVIDER,
    llmModel: env.LLM_MODEL,
    anthropicApiKey: env.ANTHROPIC_API_KEY,
    googleApiKey: env.GOOGLE_API_KEY,
    googleCloudProject: env.GOOGLE_CLOUD_PROJECT,
    googleCloudLocation: env.GOOGLE_CLOUD_LOCATION,
    awsRegion: env.AWS_REGION,
    awsAccessKeyId: env.AWS_ACCESS_KEY_ID,
    awsSecretAccessKey: env.AWS_SECRET_ACCESS_KEY,
    airsApiKey: env.PANW_AI_SEC_API_KEY,
    mgmtClientId: env.PANW_MGMT_CLIENT_ID,
    mgmtClientSecret: env.PANW_MGMT_CLIENT_SECRET,
    mgmtTsgId: env.PANW_MGMT_TSG_ID,
    mgmtEndpoint: env.PANW_MGMT_ENDPOINT,
    mgmtTokenEndpoint: env.PANW_MGMT_TOKEN_ENDPOINT,
    scanConcurrency: env.SCAN_CONCURRENCY,
    propagationDelayMs: env.PROPAGATION_DELAY_MS,
    accumulateTests: env.ACCUMULATE_TESTS,
    maxAccumulatedTests: env.MAX_ACCUMULATED_TESTS,
    dataDir: env.DATA_DIR,
    memoryEnabled: env.MEMORY_ENABLED,
    memoryDir: env.MEMORY_DIR,
    maxMemoryChars: env.MAX_MEMORY_CHARS,
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

export async function loadConfig(
  cliOverrides: Record<string, unknown> = {},
  configFilePath?: string,
): Promise<Config> {
  const filePath = configFilePath ?? join(homedir(), '.prisma-airs', 'config.json');
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
    memoryDir: expandHome(config.memoryDir),
  };
}
