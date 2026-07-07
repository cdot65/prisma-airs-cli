import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  buildConfigRows,
  CONFIG_KEYS,
  isKnownKey,
  isSecretKey,
  maskSecret,
  setConfigValue,
  unsetConfigValue,
} from '../../../src/cli/commands/config.js';
import { inspectConfig, resolveConfigFilePath } from '../../../src/config/loader.js';

const ENV_VARS = [
  'SCAN_CONCURRENCY',
  'DATA_DIR',
  'PANW_AI_SEC_API_KEY',
  'PANW_AI_SEC_API_TOKEN',
  'PANW_AI_SEC_API_ENDPOINT',
  'PANW_AI_SEC_NUM_RETRIES',
  'PANW_MGMT_CLIENT_ID',
  'PANW_MGMT_CLIENT_SECRET',
  'PANW_MGMT_TSG_ID',
  'PANW_MGMT_ENDPOINT',
  'PANW_MGMT_TOKEN_ENDPOINT',
  'PANW_DLP_ENDPOINT',
  'PANW_RED_TEAM_DATA_ENDPOINT',
  'PANW_RED_TEAM_MGMT_ENDPOINT',
  'PANW_RED_TEAM_TOKEN_ENDPOINT',
  'PANW_MODEL_SEC_DATA_ENDPOINT',
  'PANW_MODEL_SEC_MGMT_ENDPOINT',
  'PANW_MODEL_SEC_TOKEN_ENDPOINT',
  'PRISMA_AIRS_CONFIG_PATH',
];

describe('config command', () => {
  let tempDir: string;
  let configPath: string;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'config-cmd-test-'));
    configPath = join(tempDir, 'config.json');
    for (const v of ENV_VARS) vi.stubEnv(v, '');
  });

  afterEach(async () => {
    vi.unstubAllEnvs();
    await rm(tempDir, { recursive: true, force: true });
  });

  describe('CONFIG_KEYS / isKnownKey', () => {
    it('includes all ConfigSchema keys', () => {
      expect(CONFIG_KEYS).toContain('airsApiKey');
      expect(CONFIG_KEYS).toContain('mgmtClientSecret');
      expect(CONFIG_KEYS).toContain('scanConcurrency');
      expect(CONFIG_KEYS).toContain('dataDir');
    });

    it('isKnownKey accepts schema keys and rejects others', () => {
      expect(isKnownKey('scanConcurrency')).toBe(true);
      expect(isKnownKey('notAKey')).toBe(false);
    });
  });

  describe('isSecretKey', () => {
    it('flags keys matching key/secret/token/password', () => {
      expect(isSecretKey('airsApiKey')).toBe(true);
      expect(isSecretKey('airsApiToken')).toBe(true);
      expect(isSecretKey('mgmtClientSecret')).toBe(true);
      expect(isSecretKey('somePassword')).toBe(true);
    });

    it('does not flag non-secret keys', () => {
      expect(isSecretKey('scanConcurrency')).toBe(false);
      expect(isSecretKey('dataDir')).toBe(false);
      expect(isSecretKey('mgmtEndpoint')).toBe(false);
    });
  });

  describe('maskSecret', () => {
    it('shows last 4 chars for long values', () => {
      expect(maskSecret('sk-abcdef123456')).toBe('***3456');
    });

    it('fully masks short values', () => {
      expect(maskSecret('short')).toBe('***');
    });

    it('masks empty/undefined as ***', () => {
      expect(maskSecret(undefined)).toBe('***');
      expect(maskSecret('')).toBe('***');
    });
  });

  describe('resolveConfigFilePath', () => {
    it('prefers explicit path', () => {
      vi.stubEnv('PRISMA_AIRS_CONFIG_PATH', '/env/path.json');
      expect(resolveConfigFilePath('/explicit/path.json')).toBe('/explicit/path.json');
    });

    it('falls back to PRISMA_AIRS_CONFIG_PATH env var', () => {
      vi.stubEnv('PRISMA_AIRS_CONFIG_PATH', '/env/path.json');
      expect(resolveConfigFilePath()).toBe('/env/path.json');
    });

    it('defaults to ~/.prisma-airs/config.json', () => {
      const p = resolveConfigFilePath();
      expect(p).toMatch(/\.prisma-airs[/\\]config\.json$/);
    });
  });

  describe('inspectConfig', () => {
    it('reports default source when nothing is set', async () => {
      const inspected = await inspectConfig(configPath);
      expect(inspected.scanConcurrency).toEqual({ value: 5, source: 'default' });
      expect(inspected.airsApiKey.source).toBe('default');
      expect(inspected.airsApiKey.value).toBeUndefined();
    });

    it('reports file source for file-provided keys', async () => {
      await writeFile(configPath, JSON.stringify({ scanConcurrency: 3 }));
      const inspected = await inspectConfig(configPath);
      expect(inspected.scanConcurrency).toEqual({ value: 3, source: 'file' });
    });

    it('reports env source and env wins over file', async () => {
      await writeFile(configPath, JSON.stringify({ scanConcurrency: 3 }));
      vi.stubEnv('SCAN_CONCURRENCY', '7');
      const inspected = await inspectConfig(configPath);
      expect(inspected.scanConcurrency).toEqual({ value: 7, source: 'env' });
    });

    it('honors PRISMA_AIRS_CONFIG_PATH when no explicit path given', async () => {
      await writeFile(configPath, JSON.stringify({ scanConcurrency: 2 }));
      vi.stubEnv('PRISMA_AIRS_CONFIG_PATH', configPath);
      const inspected = await inspectConfig();
      expect(inspected.scanConcurrency).toEqual({ value: 2, source: 'file' });
    });

    it('covers every ConfigSchema key', async () => {
      const inspected = await inspectConfig(configPath);
      for (const key of CONFIG_KEYS) {
        expect(inspected[key]).toBeDefined();
      }
    });
  });

  describe('buildConfigRows', () => {
    it('masks secret values by default', async () => {
      await writeFile(configPath, JSON.stringify({ airsApiKey: 'sk-abcdef123456' }));
      const inspected = await inspectConfig(configPath);
      const rows = buildConfigRows(inspected, false);
      const row = rows.find((r) => r.key === 'airsApiKey');
      expect(row?.value).toBe('***3456');
      expect(row?.source).toBe('file');
    });

    it('reveals secret values when reveal=true', async () => {
      await writeFile(configPath, JSON.stringify({ airsApiKey: 'sk-abcdef123456' }));
      const inspected = await inspectConfig(configPath);
      const rows = buildConfigRows(inspected, true);
      const row = rows.find((r) => r.key === 'airsApiKey');
      expect(row?.value).toBe('sk-abcdef123456');
    });

    it('does not mask unset secrets', async () => {
      const inspected = await inspectConfig(configPath);
      const row = buildConfigRows(inspected, false).find((r) => r.key === 'airsApiKey');
      expect(row?.value).toBe('');
      expect(row?.source).toBe('default');
    });

    it('leaves non-secret values untouched', async () => {
      const inspected = await inspectConfig(configPath);
      const row = buildConfigRows(inspected, false).find((r) => r.key === 'scanConcurrency');
      expect(row?.value).toBe('5');
      expect(row?.source).toBe('default');
    });
  });

  describe('setConfigValue', () => {
    it('creates file and directory if missing', async () => {
      const nested = join(tempDir, 'deep', 'dir', 'config.json');
      const result = await setConfigValue(nested, 'scanConcurrency', '7');
      expect(result.ok).toBe(true);
      const raw = JSON.parse(await readFile(nested, 'utf-8'));
      expect(raw.scanConcurrency).toBe(7);
    });

    it('coerces numeric values via schema', async () => {
      const result = await setConfigValue(configPath, 'scanConcurrency', '9');
      expect(result.ok).toBe(true);
      if (result.ok) expect(result.value).toBe(9);
    });

    it('preserves unknown keys already in the file', async () => {
      await writeFile(configPath, JSON.stringify({ customUnknown: 'keep-me', dataDir: '/x' }));
      const result = await setConfigValue(configPath, 'scanConcurrency', '4');
      expect(result.ok).toBe(true);
      const raw = JSON.parse(await readFile(configPath, 'utf-8'));
      expect(raw.customUnknown).toBe('keep-me');
      expect(raw.dataDir).toBe('/x');
      expect(raw.scanConcurrency).toBe(4);
    });

    it('rejects values that fail schema validation', async () => {
      const result = await setConfigValue(configPath, 'scanConcurrency', '99');
      expect(result.ok).toBe(false);
      if (!result.ok) expect(result.error.toLowerCase()).toContain('20');
    });

    it('does not write the file on validation failure', async () => {
      await setConfigValue(configPath, 'scanConcurrency', '99');
      await expect(readFile(configPath, 'utf-8')).rejects.toThrow();
    });

    it('errors on malformed existing config file instead of clobbering', async () => {
      await writeFile(configPath, 'not-json!!!');
      const result = await setConfigValue(configPath, 'scanConcurrency', '4');
      expect(result.ok).toBe(false);
      expect(await readFile(configPath, 'utf-8')).toBe('not-json!!!');
    });

    it('sets string values', async () => {
      const result = await setConfigValue(configPath, 'airsApiKey', 'sk-test-value');
      expect(result.ok).toBe(true);
      const raw = JSON.parse(await readFile(configPath, 'utf-8'));
      expect(raw.airsApiKey).toBe('sk-test-value');
    });
  });

  describe('unsetConfigValue', () => {
    it('removes the key and preserves the rest', async () => {
      await writeFile(configPath, JSON.stringify({ scanConcurrency: 3, customUnknown: 'keep-me' }));
      const result = await unsetConfigValue(configPath, 'scanConcurrency');
      expect(result.ok).toBe(true);
      if (result.ok) expect(result.removed).toBe(true);
      const raw = JSON.parse(await readFile(configPath, 'utf-8'));
      expect(raw.scanConcurrency).toBeUndefined();
      expect(raw.customUnknown).toBe('keep-me');
    });

    it('reports removed=false when key absent', async () => {
      await writeFile(configPath, JSON.stringify({ dataDir: '/x' }));
      const result = await unsetConfigValue(configPath, 'scanConcurrency');
      expect(result.ok).toBe(true);
      if (result.ok) expect(result.removed).toBe(false);
    });

    it('reports removed=false when file missing', async () => {
      const result = await unsetConfigValue(join(tempDir, 'nope.json'), 'scanConcurrency');
      expect(result.ok).toBe(true);
      if (result.ok) expect(result.removed).toBe(false);
    });

    it('errors on malformed existing config file', async () => {
      await writeFile(configPath, 'not-json!!!');
      const result = await unsetConfigValue(configPath, 'scanConcurrency');
      expect(result.ok).toBe(false);
      expect(await readFile(configPath, 'utf-8')).toBe('not-json!!!');
    });
  });
});
