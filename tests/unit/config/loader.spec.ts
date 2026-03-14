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
    vi.stubEnv('LLM_PROVIDER', '');
    vi.stubEnv('ANTHROPIC_API_KEY', '');
    vi.stubEnv('SCAN_CONCURRENCY', '');
    vi.stubEnv('GOOGLE_CLOUD_PROJECT', '');
    vi.stubEnv('GOOGLE_CLOUD_LOCATION', '');
    vi.stubEnv('AWS_REGION', '');
    vi.stubEnv('MEMORY_ENABLED', '');
    vi.stubEnv('DATA_DIR', '');
    vi.stubEnv('MEMORY_DIR', '');
    vi.stubEnv('MAX_MEMORY_CHARS', '');
    vi.stubEnv('LLM_MODEL', '');
    vi.stubEnv('GOOGLE_API_KEY', '');
    vi.stubEnv('AWS_ACCESS_KEY_ID', '');
    vi.stubEnv('AWS_SECRET_ACCESS_KEY', '');
    vi.stubEnv('PANW_AI_SEC_API_KEY', '');
    vi.stubEnv('PANW_MGMT_CLIENT_ID', '');
    vi.stubEnv('PANW_MGMT_CLIENT_SECRET', '');
    vi.stubEnv('PANW_MGMT_TSG_ID', '');
    vi.stubEnv('PANW_MGMT_ENDPOINT', '');
    vi.stubEnv('PANW_MGMT_TOKEN_ENDPOINT', '');
  });

  afterEach(async () => {
    vi.unstubAllEnvs();
    await rm(tempDir, { recursive: true, force: true });
  });

  it('returns Zod defaults with no env/file/CLI', async () => {
    const config = await loadConfig({}, configPath);
    expect(config.llmProvider).toBe('claude-api');
    expect(config.scanConcurrency).toBe(5);
    expect(config.memoryEnabled).toBe(true);
  });

  it('reads env vars', async () => {
    vi.stubEnv('LLM_PROVIDER', 'gemini-api');
    vi.stubEnv('ANTHROPIC_API_KEY', 'sk-env');
    vi.stubEnv('SCAN_CONCURRENCY', '8');

    const config = await loadConfig({}, configPath);
    expect(config.llmProvider).toBe('gemini-api');
    expect(config.anthropicApiKey).toBe('sk-env');
    expect(config.scanConcurrency).toBe(8);
  });

  it('reads config file JSON', async () => {
    await writeFile(
      configPath,
      JSON.stringify({ llmProvider: 'claude-vertex', scanConcurrency: 3 }),
    );

    const config = await loadConfig({}, configPath);
    expect(config.llmProvider).toBe('claude-vertex');
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

  it('expands ~ in dataDir', async () => {
    const config = await loadConfig({}, configPath);
    expect(config.dataDir).toBe(join(homedir(), '.prisma-airs/runs'));
    expect(config.dataDir).not.toContain('~');
  });

  it('expands ~ in memoryDir', async () => {
    const config = await loadConfig({}, configPath);
    expect(config.memoryDir).toBe(join(homedir(), '.prisma-airs/memory'));
  });

  it('treats empty strings as unset (stripUndefined)', async () => {
    vi.stubEnv('LLM_PROVIDER', '');
    const config = await loadConfig({}, configPath);
    expect(config.llmProvider).toBe('claude-api');
  });

  it('silently falls back on missing config file', async () => {
    const config = await loadConfig({}, join(tempDir, 'nonexistent.json'));
    expect(config.llmProvider).toBe('claude-api');
  });

  it('silently falls back on malformed config file', async () => {
    await writeFile(configPath, 'not-json!!!');
    const config = await loadConfig({}, configPath);
    expect(config.llmProvider).toBe('claude-api');
  });

  it('does not expand absolute paths (non-tilde)', async () => {
    const config = await loadConfig({ dataDir: '/tmp/custom-dir' }, configPath);
    expect(config.dataDir).toBe('/tmp/custom-dir');
  });

  it('uses default config file path when configFilePath not provided', async () => {
    // loadConfig with no configFilePath reads from ~/.prisma-airs/config.json (likely missing)
    const config = await loadConfig({});
    expect(config.llmProvider).toBe('claude-api');
  });
});
