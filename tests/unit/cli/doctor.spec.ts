import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  checkAiGatewayApi,
  checkConfigFile,
  checkEnvironment,
  checkManagementAuth,
  checkManagementCredentials,
  checkNodeVersion,
  checkScannerApi,
  checkScannerCredentials,
  checkTenant,
  DOCTOR_CHECK_NAMES,
  type DoctorCheck,
  hasFailure,
  runDoctor,
  SUPPORTED_NODE_VERSIONS,
  summarize,
} from '../../../src/cli/commands/doctor.js';
import type { ConfigContext, ConfigEntry } from '../../../src/config/loader.js';

function entry(value: unknown, source: 'file' | 'default' = 'file'): ConfigEntry {
  return { value, source };
}

function inspected(overrides: Record<string, ConfigEntry> = {}): Record<string, ConfigEntry> {
  return {
    airsApiKey: entry(undefined, 'default'),
    airsApiToken: entry(undefined, 'default'),
    mgmtClientId: entry(undefined, 'default'),
    mgmtClientSecret: entry(undefined, 'default'),
    mgmtTsgId: entry(undefined, 'default'),
    ...overrides,
  };
}

const registryPath = '/state/prisma-airs/tenants.json';
const namedContext = (configPath: string): ConfigContext => ({
  path: configPath,
  selection: 'tenant',
  tenant: { name: 'dev', configPath, tsgId: '100' },
  registryPath,
});
const explicitContext = (path: string): ConfigContext => ({
  path,
  selection: 'explicit',
  registryPath,
});
const noneContext = (registered: string[] = []): ConfigContext => ({
  selection: 'none',
  registryPath,
  registered,
});
const validTenantFile = JSON.stringify({
  mgmtClientId: 'c',
  mgmtClientSecret: 's',
  mgmtTsgId: '100',
});

const never = () => new Promise<never>(() => {});

describe('doctor command', () => {
  let tempDir: string;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'doctor-test-'));
  });

  afterEach(async () => {
    vi.unstubAllEnvs();
    await rm(tempDir, { recursive: true, force: true });
  });

  describe('checkNodeVersion', () => {
    it('uses the same engine requirement as the published package', async () => {
      const pkg = JSON.parse(
        await readFile(new URL('../../../package.json', import.meta.url), 'utf8'),
      );
      expect(pkg.engines.node).toBe(SUPPORTED_NODE_VERSIONS);
    });

    it('passes on node 20 and 22', () => {
      const check = checkNodeVersion('v20.17.0');
      expect(check.status).toBe('pass');
      expect(check.name).toBe('Node.js version');
      expect(checkNodeVersion('v22.13.0').status).toBe('pass');
    });

    it('fails on node 18 with an upgrade hint', () => {
      const check = checkNodeVersion('v18.19.0');
      expect(check.status).toBe('fail');
      expect(check.hint).toMatch(/20/);
    });

    it.each([
      'v20.16.99',
      'v21.7.0',
      'v22.12.99',
      'v23.4.99',
      'v24.0.0-rc.1',
      '20junk',
      'v24',
    ])('rejects unsupported or malformed runtime %s', (version) => {
      expect(checkNodeVersion(version).status).toBe('fail');
    });

    it.each(['v20.19.0', 'v23.5.0', 'v24.0.0', 'v26.0.0'])('accepts runtime %s', (version) => {
      expect(checkNodeVersion(version).status).toBe('pass');
    });
  });

  describe('checkTenant', () => {
    it('names the selected tenant, its TSG and the registry', () => {
      const check = checkTenant(namedContext('/cfg/dev.json'));
      expect(check.status).toBe('pass');
      expect(check.detail).toContain('dev (TSG 100)');
      expect(check.detail).toContain(registryPath);
    });

    it('passes for an explicit library path', () => {
      expect(checkTenant(explicitContext('/tmp/other.json')).status).toBe('pass');
    });

    it('fails when tenants exist but none is selected', () => {
      const check = checkTenant(noneContext(['a', 'b']));
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('registered: a, b');
      expect(check.hint).toContain('airs tenant switch <name>');
    });

    it('fails and explains creation when nothing is registered', () => {
      const check = checkTenant(noneContext());
      expect(check.status).toBe('fail');
      expect(check.detail).toContain(registryPath);
      expect(check.hint).toContain('airs tenant create <name>');
    });
  });

  describe('checkConfigFile', () => {
    it('fails when the selected file is absent', async () => {
      const check = await checkConfigFile(namedContext(join(tempDir, 'nope.json')));
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('not found');
      expect(check.hint).toContain('airs tenant create');
    });

    it('passes on a valid JSON object at an explicit path', async () => {
      const p = join(tempDir, 'config.json');
      await writeFile(p, '{"scanConcurrency": 5}\n');
      expect((await checkConfigFile(explicitContext(p))).status).toBe('pass');
    });

    it('fails on malformed JSON and non-objects', async () => {
      const p = join(tempDir, 'config.json');
      await writeFile(p, '{oops');
      const malformed = await checkConfigFile(explicitContext(p));
      expect(malformed.status).toBe('fail');
      expect(malformed.detail).toContain('not valid JSON');
      await writeFile(p, '[1,2]');
      expect((await checkConfigFile(explicitContext(p))).status).toBe('fail');
    });

    it('fails schema violations naming the key but never the value', async () => {
      const p = join(tempDir, 'config.json');
      await writeFile(p, '{"scanConcurrency": 99, "mgmtClientSecret": "SUPER-SECRET"}');
      const check = await checkConfigFile(explicitContext(p));
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('scanConcurrency');
      expect(check.hint).toContain('airs tenant set');
      expect(`${check.detail}${check.hint}`).not.toContain('99');
      expect(`${check.detail}${check.hint}`).not.toContain('SUPER-SECRET');
    });

    it('validates a tenant file against its pinned TSG', async () => {
      const p = join(tempDir, 'dev.json');
      await writeFile(p, validTenantFile);
      const ok = await checkConfigFile(namedContext(p));
      expect(ok.status).toBe('pass');
      expect(ok.detail).toContain('TSG 100 matches');

      await writeFile(
        p,
        JSON.stringify({ mgmtClientId: 'c', mgmtClientSecret: 's', mgmtTsgId: '200' }),
      );
      const drifted = await checkConfigFile(namedContext(p));
      expect(drifted.status).toBe('fail');
      expect(drifted.detail).toContain('different mgmtTsgId');
      expect(drifted.detail).not.toContain('200');
    });

    it('warns about retired per-product keys still present in the file', async () => {
      const p = join(tempDir, 'dev.json');
      await writeFile(
        p,
        JSON.stringify({
          mgmtClientId: 'c',
          mgmtClientSecret: 's',
          mgmtTsgId: '100',
          redTeamTokenEndpoint: 'https://old.example',
          custom: 1,
        }),
      );
      const check = await checkConfigFile(namedContext(p));
      expect(check.status).toBe('warn');
      expect(check.detail).toContain('ignored 2 keys: redTeamTokenEndpoint, custom');
      expect(check.hint).toContain('airs tenant unset <name> <key>');
      expect(check.hint).toContain('redTeamTokenEndpoint');
    });

    it('skips when no tenant is selected', async () => {
      expect((await checkConfigFile(noneContext())).status).toBe('skip');
    });
  });

  describe('checkEnvironment', () => {
    it('warns about every configuration-looking variable, names only', () => {
      const check = checkEnvironment({
        PANW_MGMT_CLIENT_SECRET: 'SUPER-SECRET',
        PRISMA_AIRS_CONFIG_PATH: '/x',
        SCAN_CONCURRENCY: '3',
      });
      expect(check.status).toBe('warn');
      expect(check.detail).toContain(
        'ignored 3 variables: PANW_MGMT_CLIENT_SECRET, PRISMA_AIRS_CONFIG_PATH, SCAN_CONCURRENCY',
      );
      expect(check.detail).not.toContain('SUPER-SECRET');
      expect(check.hint).toContain('airs tenant set <name> <key>');
    });

    it('passes a clean environment and mentions active SDK diagnostics', () => {
      expect(checkEnvironment({ HOME: '/h' })).toMatchObject({
        status: 'pass',
        detail: 'no configuration variables set',
      });
      const diagnostics = checkEnvironment({ PANW_AI_SEC_DEBUG: '1' });
      expect(diagnostics.status).toBe('pass');
      expect(diagnostics.detail).toContain('SDK diagnostics on: PANW_AI_SEC_DEBUG');
    });
  });

  describe('checkScannerCredentials', () => {
    it('passes and reports the source for a key or a token', () => {
      const key = checkScannerCredentials(inspected({ airsApiKey: entry('sk-123') }));
      expect(key.status).toBe('pass');
      expect(key.detail).toBe('airsApiKey (file)');
      expect(key.detail).not.toContain('sk-123');
      expect(checkScannerCredentials(inspected({ airsApiToken: entry('tok') })).detail).toBe(
        'airsApiToken (file)',
      );
    });

    it('skips (not fails) when unset, with the tenant remedy', () => {
      const check = checkScannerCredentials(inspected(), namedContext('/cfg/dev.json'));
      expect(check.status).toBe('skip');
      expect(check.hint).toContain('airs tenant set dev <key>');
      expect(check.hint).not.toContain('PANW_');
    });

    it('skips when the config could not be inspected', () => {
      expect(checkScannerCredentials(undefined).status).toBe('skip');
    });
  });

  describe('checkManagementCredentials', () => {
    it('passes when all three are set', () => {
      const check = checkManagementCredentials(
        inspected({
          mgmtClientId: entry('id'),
          mgmtClientSecret: entry('secret'),
          mgmtTsgId: entry('123'),
        }),
      );
      expect(check.status).toBe('pass');
      expect(check.detail).toContain('mgmtClientSecret (file)');
      expect(check.detail).not.toContain('secret');
    });

    it('fails naming only the missing keys, with the tenant remedy', () => {
      const check = checkManagementCredentials(
        inspected({ mgmtClientId: entry('id') }),
        namedContext('/cfg/dev.json'),
      );
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('missing: mgmtClientSecret, mgmtTsgId');
      expect(check.hint).toContain('airs tenant set dev <key>');
    });

    it('only warns for a scanner-only setup', () => {
      expect(checkManagementCredentials(inspected(), undefined, true).status).toBe('warn');
    });
  });

  describe('checkScannerApi', () => {
    it('skips when no API key is configured', async () => {
      const check = await checkScannerApi(never, false, 50);
      expect(check.status).toBe('skip');
      expect(check.detail).toContain('skipped');
    });

    it('passes when the probe resolves', async () => {
      expect((await checkScannerApi(() => Promise.resolve([]), true, 50)).status).toBe('pass');
    });

    it('fails on 401/403 (bad key)', async () => {
      const err = Object.assign(new Error('Unauthorized'), { status: 401 });
      const check = await checkScannerApi(() => Promise.reject(err), true, 50);
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('401');
    });

    it('fails on SDK auth errors that carry no HTTP status (real SDK shape)', async () => {
      const err = new Error('AISEC_CLIENT_SIDE_ERROR:Invalid API Key or OAuth Token');
      const check = await checkScannerApi(() => Promise.reject(err), true, 50);
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('rejected');
    });

    it('passes on non-auth HTTP errors (endpoint reachable, key accepted)', async () => {
      const err = Object.assign(new Error('Not found'), { status: 404 });
      expect((await checkScannerApi(() => Promise.reject(err), true, 50)).status).toBe('pass');
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
    it('skips when management creds are missing', async () => {
      expect((await checkManagementAuth(never, false, 50)).status).toBe('skip');
    });

    it('passes when the probe resolves', async () => {
      const check = await checkManagementAuth(() => Promise.resolve(1), true, 50);
      expect(check.status).toBe('pass');
      expect(check.detail).toContain('1 custom topic)');
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
    });

    it('fails when the probe hangs past the timeout', async () => {
      const check = await checkManagementAuth(never, true, 20);
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('timed out');
    });
  });

  describe('checkAiGatewayApi', () => {
    it('skips when management creds are missing', async () => {
      expect((await checkAiGatewayApi(never, false, 50)).status).toBe('skip');
    });

    it('passes when the probe resolves with a workspace count', async () => {
      const check = await checkAiGatewayApi(() => Promise.resolve(3), true, 50);
      expect(check.status).toBe('pass');
      expect(check.name).toBe('AI Gateway API');
      expect(check.detail).toContain('3 workspaces');
    });

    it('warns (not fails) on a 403 — permission boundary, endpoint reachable', async () => {
      const err = Object.assign(new Error('Forbidden: errorCode AB03'), { statusCode: 403 });
      const check = await checkAiGatewayApi(() => Promise.reject(err), true, 50);
      expect(check.status).toBe('warn');
      expect(check.detail).toContain('reachable');
      expect(check.hint).toContain('workspace-scope');
    });

    it('still fails on non-403 HTTP errors', async () => {
      const err = Object.assign(new Error('boom'), { statusCode: 500 });
      expect((await checkAiGatewayApi(() => Promise.reject(err), true, 50)).status).toBe('fail');
    });

    it('fails when the probe hangs past the timeout', async () => {
      const check = await checkAiGatewayApi(never, true, 20);
      expect(check.status).toBe('fail');
      expect(check.detail).toContain('timed out');
    });
  });

  describe('runDoctor', () => {
    it('runs all nine checks in order and never throws, even when probes reject', async () => {
      const p = join(tempDir, 'dev.json');
      await writeFile(p, validTenantFile);
      const checks = await runDoctor({
        nodeVersion: 'v22.0.0',
        context: namedContext(p),
        env: {},
        inspect: async () =>
          inspected({
            airsApiKey: entry('sk-123'),
            mgmtClientId: entry('id'),
            mgmtClientSecret: entry('secret'),
            mgmtTsgId: entry('100'),
          }),
        scannerProbe: () => Promise.reject(new TypeError('fetch failed')),
        mgmtProbe: () => Promise.reject(Object.assign(new Error('boom'), { status: 500 })),
        aiGwProbe: () => Promise.reject(Object.assign(new Error('boom'), { status: 500 })),
        timeoutMs: 50,
      });

      expect(checks.map((c) => c.name)).toEqual([...DOCTOR_CHECK_NAMES]);
      for (const c of checks) {
        expect(['pass', 'warn', 'fail', 'skip']).toContain(c.status);
        expect(typeof c.detail).toBe('string');
      }
    });

    it('fails the tenant check and skips file and credential checks when nothing is selected', async () => {
      const inspect = vi.fn(async () => inspected());
      const probe = vi.fn(never);
      const checks = await runDoctor({
        nodeVersion: 'v20.17.0',
        context: noneContext(['dev']),
        env: { PANW_MGMT_CLIENT_ID: 'stale' },
        inspect,
        scannerProbe: probe,
        mgmtProbe: probe,
        aiGwProbe: probe,
        timeoutMs: 20,
      });

      const byName = Object.fromEntries(checks.map((c) => [c.name, c]));
      expect(byName.Tenant.status).toBe('fail');
      expect(byName['Config file'].status).toBe('skip');
      expect(byName.Environment.status).toBe('warn');
      expect(byName['Scanner credentials'].status).toBe('skip');
      expect(byName['Management credentials'].status).toBe('skip');
      expect(byName['Management OAuth'].status).toBe('skip');
      expect(inspect).not.toHaveBeenCalled();
      expect(probe).not.toHaveBeenCalled();
      expect(hasFailure(checks)).toBe(true);
    });

    it('passes a management-only tenant with skips, not failures', async () => {
      const p = join(tempDir, 'dev.json');
      await writeFile(p, validTenantFile);
      const checks = await runDoctor({
        nodeVersion: 'v22.13.0',
        context: namedContext(p),
        env: { PANW_AI_SEC_DEBUG: '' },
        inspect: async () =>
          inspected({
            mgmtClientId: entry('c'),
            mgmtClientSecret: entry('s'),
            mgmtTsgId: entry('100'),
          }),
        scannerProbe: never,
        mgmtProbe: () => Promise.resolve(2),
        aiGwProbe: () => Promise.resolve(1),
        timeoutMs: 50,
      });

      expect(hasFailure(checks)).toBe(false);
      expect(checks.map((c) => c.status)).toEqual([
        'pass',
        'pass',
        'pass',
        'pass',
        'skip',
        'pass',
        'skip',
        'pass',
        'pass',
      ]);
      expect(summarize(checks)).toEqual({
        failed: false,
        message: 'All checks passed (2 skipped)',
      });
    });

    it('does not evaluate credentials from a broken tenant file', async () => {
      const p = join(tempDir, 'dev.json');
      await writeFile(p, '{broken');
      const inspect = vi.fn(async () => inspected());
      const checks = await runDoctor({
        nodeVersion: 'v22.13.0',
        context: namedContext(p),
        env: {},
        inspect,
        scannerProbe: never,
        mgmtProbe: never,
        aiGwProbe: never,
        timeoutMs: 20,
      });
      const byName = Object.fromEntries(checks.map((c) => [c.name, c]));
      expect(byName['Config file'].status).toBe('fail');
      expect(byName['Management credentials'].status).toBe('skip');
      expect(inspect).not.toHaveBeenCalled();
    });

    it('fails the tenant check and skips the rest when the registry is unreadable', async () => {
      vi.stubEnv('PRISMA_AIRS_TENANTS_PATH', join(tempDir, 'tenants.json'));
      await writeFile(join(tempDir, 'tenants.json'), '{broken');
      const checks = await runDoctor({ nodeVersion: 'v22.13.0', env: {} });
      expect(checks[1]).toMatchObject({ name: 'Tenant', status: 'fail' });
      expect(checks[1].detail).toContain('tenant registry');
      expect(checks.slice(2).every((c) => c.status === 'skip')).toBe(true);
      expect(hasFailure(checks)).toBe(true);
    });

    it('loads config once and hands it to every default-style probe', async () => {
      const p = join(tempDir, 'dev.json');
      await writeFile(p, validTenantFile);
      const loader = vi.fn(async () => ({ mgmtTsgId: '100' }) as never);
      const seen: string[] = [];
      const checks = await runDoctor({
        nodeVersion: 'v22.13.0',
        context: namedContext(p),
        env: {},
        inspect: async () =>
          inspected({
            airsApiKey: entry('k'),
            mgmtClientId: entry('c'),
            mgmtClientSecret: entry('s'),
            mgmtTsgId: entry('100'),
          }),
        loadConfig: loader,
        scannerProbe: async (config) => seen.push(`scan:${config.mgmtTsgId}`),
        mgmtProbe: async (config) => seen.push(`mgmt:${config.mgmtTsgId}`),
        aiGwProbe: async (config) => seen.push(`gw:${config.mgmtTsgId}`),
        timeoutMs: 50,
      });
      expect(loader).toHaveBeenCalledTimes(1);
      expect(seen).toEqual(['scan:100', 'mgmt:100', 'gw:100']);
      expect(hasFailure(checks)).toBe(false);
    });

    it('json shape: each check has name/status/detail and only string hints', async () => {
      const checks = await runDoctor({
        nodeVersion: 'v18.0.0',
        context: noneContext(),
        env: {},
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

  describe('summarize and hasFailure (exit-code logic)', () => {
    const check = (status: DoctorCheck['status']): DoctorCheck => ({
      name: 'x',
      status,
      detail: 'd',
    });

    it('false when all pass', () => {
      expect(hasFailure([check('pass'), check('pass')])).toBe(false);
      expect(summarize([check('pass')]).message).toBe('All checks passed');
    });

    it('false when only warns and skips (exit 0)', () => {
      const checks = [check('pass'), check('warn'), check('skip'), check('skip')];
      expect(hasFailure(checks)).toBe(false);
      expect(summarize(checks).message).toBe('All checks passed (1 warning, 2 skipped)');
    });

    it('true when any fail (exit 1)', () => {
      const checks = [check('pass'), check('warn'), check('fail'), check('fail')];
      expect(hasFailure(checks)).toBe(true);
      expect(summarize(checks)).toEqual({ failed: true, message: '2 checks failed' });
    });
  });
});
