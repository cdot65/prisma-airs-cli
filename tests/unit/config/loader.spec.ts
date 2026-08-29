import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { loadConfig } from '../../../src/config/loader.js';

describe('loadConfig', () => {
  let tempDir: string;
  let configPath: string;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'config-test-'));
    configPath = join(tempDir, 'config.json');
    // Clear env vars that might leak from host
    vi.stubEnv('SCAN_CONCURRENCY', '');
    vi.stubEnv('DATA_DIR', '');
    vi.stubEnv('PANW_AI_SEC_API_KEY', '');
    vi.stubEnv('PANW_MGMT_CLIENT_ID', '');
    vi.stubEnv('PANW_MGMT_CLIENT_SECRET', '');
    vi.stubEnv('PANW_MGMT_TSG_ID', '');
    vi.stubEnv('PANW_MGMT_ENDPOINT', '');
    vi.stubEnv('PANW_MGMT_TOKEN_ENDPOINT', '');
    vi.stubEnv('PANW_AI_SEC_API_TOKEN', '');
    vi.stubEnv('PANW_AI_SEC_API_ENDPOINT', '');
    vi.stubEnv('PANW_AI_SEC_NUM_RETRIES', '');
    vi.stubEnv('PANW_RED_TEAM_DATA_ENDPOINT', '');
    vi.stubEnv('PANW_RED_TEAM_MGMT_ENDPOINT', '');
    vi.stubEnv('PANW_RED_TEAM_TOKEN_ENDPOINT', '');
    vi.stubEnv('PANW_MODEL_SEC_DATA_ENDPOINT', '');
    vi.stubEnv('PANW_MODEL_SEC_MGMT_ENDPOINT', '');
    vi.stubEnv('PANW_MODEL_SEC_TOKEN_ENDPOINT', '');
    vi.stubEnv('PANW_AI_GW_DATA_ENDPOINT', '');
    vi.stubEnv('PANW_AI_GW_ADMIN_ENDPOINT', '');
    vi.stubEnv('PANW_AI_GW_TOKEN_ENDPOINT', '');
    vi.stubEnv('PANW_CLI_OUTPUT', '');
  });

  afterEach(async () => {
    vi.unstubAllEnvs();
    await rm(tempDir, { recursive: true, force: true });
  });

  it('returns Zod defaults with no env/file/CLI', async () => {
    const config = await loadConfig({}, configPath);
    expect(config.scanConcurrency).toBe(5);
  });

  it('reads env vars', async () => {
    vi.stubEnv('PANW_AI_SEC_API_KEY', 'sk-env');
    vi.stubEnv('SCAN_CONCURRENCY', '8');

    const config = await loadConfig({}, configPath);
    expect(config.airsApiKey).toBe('sk-env');
    expect(config.scanConcurrency).toBe(8);
  });

  it('reads PANW_CLI_OUTPUT and lets CLI overrides win', async () => {
    vi.stubEnv('PANW_CLI_OUTPUT', 'markdown');
    await expect(loadConfig({}, configPath)).resolves.toMatchObject({ defaultOutput: 'markdown' });
    await expect(loadConfig({ defaultOutput: 'json' }, configPath)).resolves.toMatchObject({
      defaultOutput: 'json',
    });
  });

  it('reads config file JSON', async () => {
    await writeFile(configPath, JSON.stringify({ scanConcurrency: 3 }));

    const config = await loadConfig({}, configPath);
    expect(config.scanConcurrency).toBe(3);
  });

  it('applies priority cascade: CLI > env > file > defaults', async () => {
    await writeFile(configPath, JSON.stringify({ scanConcurrency: 3 }));
    vi.stubEnv('SCAN_CONCURRENCY', '7');

    const config = await loadConfig({ scanConcurrency: 15 }, configPath);
    expect(config.scanConcurrency).toBe(15);
  });

  it('env overrides file', async () => {
    await writeFile(configPath, JSON.stringify({ scanConcurrency: 3 }));
    vi.stubEnv('SCAN_CONCURRENCY', '7');

    const config = await loadConfig({}, configPath);
    expect(config.scanConcurrency).toBe(7);
  });

  it('reads endpoint/auth override env vars', async () => {
    vi.stubEnv('PANW_AI_SEC_API_TOKEN', 'tok-env');
    vi.stubEnv('PANW_AI_SEC_API_ENDPOINT', 'https://airs.example.com');
    vi.stubEnv('PANW_AI_SEC_NUM_RETRIES', '2');
    vi.stubEnv('PANW_RED_TEAM_DATA_ENDPOINT', 'https://rt-data.example.com');
    vi.stubEnv('PANW_RED_TEAM_MGMT_ENDPOINT', 'https://rt-mgmt.example.com');
    vi.stubEnv('PANW_RED_TEAM_TOKEN_ENDPOINT', 'https://rt-token.example.com');
    vi.stubEnv('PANW_RED_TEAM_NETWORK_BROKER_ENDPOINT', 'https://rt-nb.example.com');
    vi.stubEnv('PANW_MODEL_SEC_DATA_ENDPOINT', 'https://ms-data.example.com');
    vi.stubEnv('PANW_MODEL_SEC_MGMT_ENDPOINT', 'https://ms-mgmt.example.com');
    vi.stubEnv('PANW_MODEL_SEC_TOKEN_ENDPOINT', 'https://ms-token.example.com');

    vi.stubEnv('PANW_AI_GW_DATA_ENDPOINT', 'https://gw-data.example.com/ai_gw/v2');
    vi.stubEnv('PANW_AI_GW_ADMIN_ENDPOINT', 'https://gw-admin.example.com/ai_gw/admin/v2');
    vi.stubEnv('PANW_AI_GW_TOKEN_ENDPOINT', 'https://gw-token.example.com');

    const config = await loadConfig({}, configPath);
    expect(config.aiGwDataEndpoint).toBe('https://gw-data.example.com/ai_gw/v2');
    expect(config.aiGwAdminEndpoint).toBe('https://gw-admin.example.com/ai_gw/admin/v2');
    expect(config.aiGwTokenEndpoint).toBe('https://gw-token.example.com');
    expect(config.airsApiToken).toBe('tok-env');
    expect(config.airsApiEndpoint).toBe('https://airs.example.com');
    expect(config.airsNumRetries).toBe(2);
    expect(config.redTeamDataEndpoint).toBe('https://rt-data.example.com');
    expect(config.redTeamMgmtEndpoint).toBe('https://rt-mgmt.example.com');
    expect(config.redTeamTokenEndpoint).toBe('https://rt-token.example.com');
    expect(config.redTeamNetworkBrokerEndpoint).toBe('https://rt-nb.example.com');
    expect(config.modelSecDataEndpoint).toBe('https://ms-data.example.com');
    expect(config.modelSecMgmtEndpoint).toBe('https://ms-mgmt.example.com');
    expect(config.modelSecTokenEndpoint).toBe('https://ms-token.example.com');
  });

  it('reads endpoint/auth overrides from config file with env taking precedence', async () => {
    await writeFile(
      configPath,
      JSON.stringify({
        airsApiEndpoint: 'https://file.example.com',
        redTeamDataEndpoint: 'https://rt-file.example.com',
      }),
    );
    vi.stubEnv('PANW_AI_SEC_API_ENDPOINT', 'https://env.example.com');

    const config = await loadConfig({}, configPath);
    expect(config.airsApiEndpoint).toBe('https://env.example.com');
    expect(config.redTeamDataEndpoint).toBe('https://rt-file.example.com');
  });

  it('expands ~ in dataDir', async () => {
    const config = await loadConfig({}, configPath);
    expect(config.dataDir).toBe(join(homedir(), '.prisma-airs/runs'));
    expect(config.dataDir).not.toContain('~');
  });

  it('treats empty strings as unset (stripUndefined)', async () => {
    vi.stubEnv('SCAN_CONCURRENCY', '');
    const config = await loadConfig({}, configPath);
    expect(config.scanConcurrency).toBe(5);
  });

  it('silently falls back on missing config file', async () => {
    const config = await loadConfig({}, join(tempDir, 'nonexistent.json'));
    expect(config.scanConcurrency).toBe(5);
  });

  it('silently falls back on malformed config file', async () => {
    await writeFile(configPath, 'not-json!!!');
    const config = await loadConfig({}, configPath);
    expect(config.scanConcurrency).toBe(5);
  });

  it('does not expand absolute paths (non-tilde)', async () => {
    const config = await loadConfig({ dataDir: '/tmp/custom-dir' }, configPath);
    expect(config.dataDir).toBe('/tmp/custom-dir');
  });

  it('uses default config file path when configFilePath not provided', async () => {
    // loadConfig with no configFilePath reads from ~/.prisma-airs/config.json (likely missing)
    const config = await loadConfig({});
    expect(config.scanConcurrency).toBe(5);
  });
});
