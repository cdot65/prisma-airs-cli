import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  inspectConfig,
  loadConfig,
  resolveConfigContext,
  resolveConfigFilePath,
} from '../../../src/config/loader.js';
import { useTestTenant } from '../../helpers/tenant.js';

describe('loadConfig', () => {
  let tempDir: string;
  let configPath: string;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'config-test-'));
    configPath = join(tempDir, 'config.json');
    vi.stubEnv('PRISMA_AIRS_TENANTS_PATH', join(tempDir, 'state', 'tenants.json'));
    // The environment must never influence configuration, so poison it.
    vi.stubEnv('PANW_AI_SEC_API_KEY', 'ENV-LEAK');
    vi.stubEnv('PANW_MGMT_CLIENT_ID', 'ENV-LEAK');
    vi.stubEnv('PANW_MGMT_TSG_ID', 'ENV-LEAK');
    vi.stubEnv('PANW_CLI_OUTPUT', 'yaml');
    vi.stubEnv('PANW_CLI_SCAN_CONCURRENCY', '9');
    vi.stubEnv('SCAN_CONCURRENCY', '9');
    vi.stubEnv('PRISMA_AIRS_CONFIG_PATH', join(tempDir, 'ignored.json'));
  });

  afterEach(async () => {
    vi.unstubAllEnvs();
    await rm(tempDir, { recursive: true, force: true });
  });

  it('returns Zod defaults for an explicit empty file and ignores the environment', async () => {
    const config = await loadConfig({}, configPath);
    expect(config.scanConcurrency).toBe(5);
    expect(config.defaultOutput).toBeUndefined();
    expect(config.airsApiKey).toBeUndefined();
    expect(config.mgmtClientId).toBeUndefined();
  });

  it('reads config file JSON', async () => {
    await writeFile(configPath, JSON.stringify({ scanConcurrency: 3, airsApiKey: 'file-key' }));
    const config = await loadConfig({}, configPath);
    expect(config.scanConcurrency).toBe(3);
    expect(config.airsApiKey).toBe('file-key');
  });

  it('applies priority cascade: CLI > file > defaults', async () => {
    await writeFile(configPath, JSON.stringify({ scanConcurrency: 3 }));
    expect((await loadConfig({ scanConcurrency: 15 }, configPath)).scanConcurrency).toBe(15);
    expect((await loadConfig({}, configPath)).scanConcurrency).toBe(3);
  });

  it('reads the selected tenant file when no explicit path is given', async () => {
    await useTestTenant({ scanConcurrency: 4, defaultOutput: 'json', mgmtTsgId: '777' });
    const config = await loadConfig();
    expect(config.mgmtTsgId).toBe('777');
    expect(config.scanConcurrency).toBe(4);
    expect(config.defaultOutput).toBe('json');
    expect(config.airsApiKey).toBeUndefined();
  });

  it('fails clearly when no tenant is selected', async () => {
    await expect(loadConfig()).rejects.toThrow("No tenant selected. Run 'airs tenant create");
    expect(() => resolveConfigFilePath()).toThrow('No tenant selected');
    expect(resolveConfigContext()).toMatchObject({ selection: 'none', registered: [] });
  });

  it('names registered tenants when none is selected', async () => {
    const { registryPath } = await useTestTenant({}, { name: 'dev' });
    const store = JSON.parse(
      await import('node:fs').then((fs) => fs.readFileSync(registryPath, 'utf8')),
    );
    store.active = null;
    await writeFile(registryPath, JSON.stringify(store));
    await expect(loadConfig()).rejects.toThrow("'airs tenant switch <name>' (registered: dev)");
  });

  it('silently ignores retired per-product token endpoint keys in a file', async () => {
    await writeFile(
      configPath,
      JSON.stringify({ redTeamTokenEndpoint: 'https://old.example', dlpEndpoint: 'https://dlp' }),
    );
    const config = (await loadConfig({}, configPath)) as Record<string, unknown>;
    expect(config.redTeamTokenEndpoint).toBeUndefined();
    expect(config.dlpEndpoint).toBe('https://dlp');
  });

  it('expands ~ in dataDir', async () => {
    const config = await loadConfig({}, configPath);
    expect(config.dataDir).toBe(join(homedir(), '.prisma-airs/runs'));
  });

  it('treats empty strings as unset', async () => {
    await writeFile(configPath, JSON.stringify({ scanConcurrency: '' }));
    expect((await loadConfig({}, configPath)).scanConcurrency).toBe(5);
  });

  it('falls back to defaults on a missing or malformed explicit file', async () => {
    expect((await loadConfig({}, join(tempDir, 'nonexistent.json'))).scanConcurrency).toBe(5);
    await writeFile(configPath, 'not-json!!!');
    expect((await loadConfig({}, configPath)).scanConcurrency).toBe(5);
  });

  it('does not expand absolute paths (non-tilde)', async () => {
    const config = await loadConfig({ dataDir: '/tmp/custom-dir' }, configPath);
    expect(config.dataDir).toBe('/tmp/custom-dir');
  });
});

describe('inspectConfig', () => {
  let tempDir: string;
  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'inspect-test-'));
    vi.stubEnv('PRISMA_AIRS_TENANTS_PATH', join(tempDir, 'tenants.json'));
  });
  afterEach(async () => {
    vi.unstubAllEnvs();
    await rm(tempDir, { recursive: true, force: true });
  });

  it('reports file and default sources only', async () => {
    await useTestTenant({ scanConcurrency: 3 });
    const inspected = await inspectConfig();
    expect(inspected.scanConcurrency).toEqual({ value: 3, source: 'file' });
    expect(inspected.dataDir).toEqual({ value: '~/.prisma-airs/runs', source: 'default' });
    expect(inspected.mgmtClientSecret).toEqual({ value: 'secret', source: 'file' });
  });
});
