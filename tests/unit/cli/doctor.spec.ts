import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  checkAiGatewayApi,
  checkConfigFile,
  checkManagementAuth,
  checkManagementCredentials,
  checkNodeVersion,
  checkScannerApi,
  checkScannerCredentials,
  type DoctorCheck,
  hasFailure,
  runDoctor,
} from '../../../src/cli/commands/doctor.js';
import type { ConfigEntry } from '../../../src/config/loader.js';

function entry(value: unknown, source: 'env' | 'file' | 'default' = 'env'): ConfigEntry {
  return { value, source };
}

function inspected(overrides: Record<string, ConfigEntry> = {}): Record<string, ConfigEntry> {
  return {
    airsApiKey: entry(undefined, 'default'),
    mgmtClientId: entry(undefined, 'default'),
    mgmtClientSecret: entry(undefined, 'default'),
    mgmtTsgId: entry(undefined, 'default'),
    ...overrides,
  };
}

const never = () => new Promise<never>(() => {});

describe('doctor command', () => {
  describe('checkNodeVersion', () => {
    it('passes on node 20', () => {
      const check = checkNodeVersion('v20.11.0');
      expect(check.status).toBe('pass');
      expect(check.name).toBe('Node.js version');
      expect(check.detail).toContain('v20.11.0');
    });

    it('passes on node 22', () => {
      expect(checkNodeVersion('v22.1.0').status).toBe('pass');
    });

    it('fails on node 18 with an upgrade hint', () => {
      const check = checkNodeVersion('v18.19.0');
      expect(check.status).toBe('fail');
      expect(check.hint).toMatch(/20/);
    });
  });

  describe('checkConfigFile', () => {
    let tempDir: string;

    beforeEach(async () => {
      tempDir = await mkdtemp(join(tmpdir(), 'doctor-test-'));
    });

    afterEach(async () => {
      await rm(tempDir, { recursive: true, force: true });
    });

    it('warns when the file is absent (env-only setups are fine)', async () => {
      const check = await checkConfigFile(join(tempDir, 'nope.json'));
      expect(check.status).toBe('warn');
      expect(check.detail).toContain('not found');
    });

    it('passes on a valid JSON object', async () => {
      const p = join(tempDir, 'config.json');
      await writeFile(p, '{"scanConcurrency": 5}\n');
      const check = await checkConfigFile(p);
      expect(check.status).toBe('pass');
    });

    it('fails on malformed JSON', async () => {
      const p = join(tempDir, 'config.json');
      await writeFile(p, '{oops');
      const check = await checkConfigFile(p);
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('not valid JSON');
    });

    it('fails when the JSON is not an object', async () => {
      const p = join(tempDir, 'config.json');
      await writeFile(p, '[1,2]');
      const check = await checkConfigFile(p);
      expect(check.status).toBe('fail');
    });
  });

  describe('checkScannerCredentials', () => {
    it('passes and reports the source when the key is set', () => {
      const check = checkScannerCredentials(inspected({ airsApiKey: entry('sk-123', 'env') }));
      expect(check.status).toBe('pass');
      expect(check.detail).toContain('env');
      expect(check.detail).not.toContain('sk-123');
    });

    it('fails with the env var name when missing', () => {
      const check = checkScannerCredentials(inspected());
      expect(check.status).toBe('fail');
      expect(check.hint).toContain('PANW_AI_SEC_API_KEY');
    });
  });

  describe('checkManagementCredentials', () => {
    it('passes when all three are set', () => {
      const check = checkManagementCredentials(
        inspected({
          mgmtClientId: entry('id', 'env'),
          mgmtClientSecret: entry('secret', 'file'),
          mgmtTsgId: entry('123', 'env'),
        }),
      );
      expect(check.status).toBe('pass');
      expect(check.detail).toContain('file');
      expect(check.detail).not.toContain('secret');
    });

    it('fails naming only the missing vars', () => {
      const check = checkManagementCredentials(inspected({ mgmtClientId: entry('id', 'env') }));
      expect(check.status).toBe('fail');
      expect(check.hint).toContain('PANW_MGMT_CLIENT_SECRET');
      expect(check.hint).toContain('PANW_MGMT_TSG_ID');
      expect(check.hint).not.toContain('PANW_MGMT_CLIENT_ID');
    });
  });

  describe('checkScannerApi', () => {
    it('warns (skips) when no API key is configured', async () => {
      const check = await checkScannerApi(never, false, 50);
      expect(check.status).toBe('warn');
      expect(check.detail).toContain('skipped');
    });

    it('passes when the probe resolves', async () => {
      const check = await checkScannerApi(() => Promise.resolve([]), true, 50);
      expect(check.status).toBe('pass');
    });

    it('fails on 401/403 (bad key)', async () => {
      const err = Object.assign(new Error('Unauthorized'), { status: 401 });
      const check = await checkScannerApi(() => Promise.reject(err), true, 50);
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('401');
    });

    it('fails on SDK auth errors that carry no HTTP status (real SDK shape)', async () => {
      // Scanner SDK throws AISecSDKException with errorType but no .status:
      const err = new Error('AISEC_CLIENT_SIDE_ERROR:Invalid API Key or OAuth Token');
      const check = await checkScannerApi(() => Promise.reject(err), true, 50);
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('rejected');
    });

    it('passes on non-auth HTTP errors (endpoint reachable, key accepted)', async () => {
      const err = Object.assign(new Error('Not found'), { status: 404 });
      const check = await checkScannerApi(() => Promise.reject(err), true, 50);
      expect(check.status).toBe('pass');
    });

    it('fails with network-unreachable detail on plain fetch errors', async () => {
      const check = await checkScannerApi(
        () => Promise.reject(new TypeError('fetch failed')),
        true,
        50,
      );
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('network unreachable');
    });

    it('fails when the probe hangs past the timeout', async () => {
      const check = await checkScannerApi(never, true, 20);
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('timed out');
    });
  });

  describe('checkManagementAuth', () => {
    it('warns (skips) when management creds are missing', async () => {
      const check = await checkManagementAuth(never, false, 50);
      expect(check.status).toBe('warn');
      expect(check.detail).toContain('skipped');
    });

    it('passes when the probe resolves', async () => {
      const check = await checkManagementAuth(() => Promise.resolve(3), true, 50);
      expect(check.status).toBe('pass');
    });

    it('fails with the API error message on rejection', async () => {
      const err = Object.assign(new Error('invalid_client'), { status: 401 });
      const check = await checkManagementAuth(() => Promise.reject(err), true, 50);
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('invalid_client');
      expect(check.detail).toContain('401');
    });

    it('fails as auth error on status-less OAuth failures (real SDK shape)', async () => {
      const err = new Error('AISEC_OAUTH_ERROR:invalid_client');
      const check = await checkManagementAuth(() => Promise.reject(err), true, 50);
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('authentication failed');
      expect(check.detail).toContain('invalid_client');
    });

    it('fails when the probe hangs past the timeout', async () => {
      const check = await checkManagementAuth(never, true, 20);
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('timed out');
    });
  });

  describe('checkAiGatewayApi', () => {
    it('warns (skips) when management creds are missing', async () => {
      const check = await checkAiGatewayApi(never, false, 50);
      expect(check.status).toBe('warn');
      expect(check.detail).toContain('skipped');
    });

    it('passes when the probe resolves with a workspace count', async () => {
      const check = await checkAiGatewayApi(() => Promise.resolve(3), true, 50);
      expect(check.status).toBe('pass');
      expect(check.name).toBe('AI Gateway API');
      expect(check.detail).toContain('3');
    });

    it('fails on a 403 with a grant hint', async () => {
      const err = Object.assign(new Error('Forbidden: errorCode AB03'), { statusCode: 403 });
      const check = await checkAiGatewayApi(() => Promise.reject(err), true, 50);
      expect(check.status).toBe('fail');
      expect(check.hint).toContain('workspace-scope');
    });

    it('fails when the probe hangs past the timeout', async () => {
      const check = await checkAiGatewayApi(never, true, 20);
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('timed out');
    });
  });

  describe('runDoctor', () => {
    let tempDir: string;

    beforeEach(async () => {
      tempDir = await mkdtemp(join(tmpdir(), 'doctor-run-'));
    });

    afterEach(async () => {
      await rm(tempDir, { recursive: true, force: true });
    });

    it('runs all seven checks and never throws, even when probes reject', async () => {
      const checks = await runDoctor({
        nodeVersion: 'v22.0.0',
        configFilePath: join(tempDir, 'absent.json'),
        inspect: async () =>
          inspected({
            airsApiKey: entry('sk-123', 'env'),
            mgmtClientId: entry('id', 'env'),
            mgmtClientSecret: entry('secret', 'env'),
            mgmtTsgId: entry('123', 'env'),
          }),
        scannerProbe: () => Promise.reject(new TypeError('fetch failed')),
        mgmtProbe: () => Promise.reject(Object.assign(new Error('boom'), { status: 500 })),
        aiGwProbe: () => Promise.reject(Object.assign(new Error('boom'), { status: 500 })),
        timeoutMs: 50,
      });

      expect(checks).toHaveLength(7);
      expect(checks.map((c) => c.name)).toEqual([
        'Node.js version',
        'Config file',
        'Scanner credentials',
        'Management credentials',
        'Scanner API',
        'Management OAuth',
        'AI Gateway API',
      ]);
      for (const c of checks) {
        expect(['pass', 'warn', 'fail']).toContain(c.status);
        expect(typeof c.detail).toBe('string');
      }
    });

    it('skips network checks when credentials are missing', async () => {
      const checks = await runDoctor({
        nodeVersion: 'v20.0.0',
        configFilePath: join(tempDir, 'absent.json'),
        inspect: async () => inspected(),
        scannerProbe: never,
        mgmtProbe: never,
        aiGwProbe: never,
        timeoutMs: 20,
      });

      const byName = Object.fromEntries(checks.map((c) => [c.name, c]));
      expect(byName['Scanner API'].status).toBe('warn');
      expect(byName['Management OAuth'].status).toBe('warn');
      expect(byName['AI Gateway API'].status).toBe('warn');
      // creds checks themselves fail
      expect(byName['Scanner credentials'].status).toBe('fail');
      expect(byName['Management credentials'].status).toBe('fail');
    });

    it('produces all-pass output on a healthy setup', async () => {
      const p = join(tempDir, 'config.json');
      await writeFile(p, '{}\n');
      const checks = await runDoctor({
        nodeVersion: 'v22.0.0',
        configFilePath: p,
        inspect: async () =>
          inspected({
            airsApiKey: entry('sk-123', 'env'),
            mgmtClientId: entry('id', 'file'),
            mgmtClientSecret: entry('secret', 'file'),
            mgmtTsgId: entry('123', 'file'),
          }),
        scannerProbe: () => Promise.resolve([]),
        mgmtProbe: () => Promise.resolve(2),
        aiGwProbe: () => Promise.resolve(1),
        timeoutMs: 50,
      });

      expect(checks.every((c) => c.status === 'pass')).toBe(true);
    });

    it('json shape: each check has name/status/detail and only string hints', async () => {
      const checks = await runDoctor({
        nodeVersion: 'v18.0.0',
        configFilePath: join(tempDir, 'absent.json'),
        inspect: async () => inspected(),
        scannerProbe: never,
        mgmtProbe: never,
        aiGwProbe: never,
        timeoutMs: 20,
      });

      const parsed = JSON.parse(JSON.stringify(checks)) as DoctorCheck[];
      for (const c of parsed) {
        expect(Object.keys(c).sort()).toEqual(expect.arrayContaining(['detail', 'name', 'status']));
        if ('hint' in c) expect(typeof c.hint).toBe('string');
      }
    });
  });

  describe('hasFailure (exit-code logic)', () => {
    const check = (status: DoctorCheck['status']): DoctorCheck => ({
      name: 'x',
      status,
      detail: 'd',
    });

    it('false when all pass', () => {
      expect(hasFailure([check('pass'), check('pass')])).toBe(false);
    });

    it('false when only warns (warns OK, exit 0)', () => {
      expect(hasFailure([check('pass'), check('warn')])).toBe(false);
    });

    it('true when any fail (exit 1)', () => {
      expect(hasFailure([check('pass'), check('warn'), check('fail')])).toBe(true);
    });
  });
});
