import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { ManagementClient } from '@cdot65/prisma-airs-sdk';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { collectRuntimeDailyReport } from '../../src/reports/runtime.js';
import type { RuntimeDailyReport } from '../../src/reports/types.js';

const runFile = promisify(execFile);
const enabled = process.env.RUN_RUNTIME_REPORT_E2E === '1';

describe.skipIf(!enabled)('live read-only runtime report', () => {
  const cliPath = resolve(process.env.RUNTIME_REPORT_CLI_ENTRY ?? 'dist/cli/index.js');
  const configPath = resolve(
    process.env.PRISMA_AIRS_CONFIG_PATH ?? join(homedir(), '.prisma-airs/config.json'),
  );
  const outputDirectory = resolve(
    'artifacts/runtime-report',
    new Date().toISOString().replaceAll(':', '-'),
  );
  let originalConfig: Buffer;
  let config: Record<string, string>;
  let env: NodeJS.ProcessEnv;
  let report: RuntimeDailyReport;
  const checks: string[] = [];
  let sessionDebugHttpStatus: number | undefined;
  const digest = (data: Buffer) => createHash('sha256').update(data).digest('hex');

  async function command(args: string[], cwd?: string) {
    try {
      const result = await runFile(
        process.execPath,
        [cliPath, '--quiet', 'runtime', 'report', ...args],
        { env, cwd, timeout: 180_000, maxBuffer: 20_000_000 },
      );
      return { code: 0, stdout: result.stdout };
    } catch (error) {
      const failed = error as { code?: number; stdout?: string };
      // No stderr/raw error output: it can contain credentials or customer metadata.
      return {
        code: typeof failed.code === 'number' ? failed.code : -1,
        stdout: failed.stdout ?? '',
      };
    }
  }

  beforeAll(async () => {
    originalConfig = await readFile(configPath);
    config = JSON.parse(originalConfig.toString('utf8'));
    env = {
      ...process.env,
      PRISMA_AIRS_CONFIG_PATH: configPath,
      PANW_AI_SEC_DEBUG: '0',
      PANW_AI_SEC_DEBUG_BODY: '0',
      PANW_AI_SEC_TIMEOUT_MS: '20000',
    };
    for (const [field, name] of Object.entries({
      mgmtClientId: 'PANW_MGMT_CLIENT_ID',
      mgmtClientSecret: 'PANW_MGMT_CLIENT_SECRET',
      mgmtTsgId: 'PANW_MGMT_TSG_ID',
      mgmtEndpoint: 'PANW_MGMT_ENDPOINT',
      mgmtDashboardEndpoint: 'PANW_MGMT_DASHBOARD_ENDPOINT',
      mgmtTokenEndpoint: 'PANW_MGMT_TOKEN_ENDPOINT',
    })) {
      if (config[field]) env[name] = config[field];
      else delete env[name];
    }
    await mkdir(outputDirectory, { recursive: true, mode: 0o700 });
  });

  afterAll(async () => {
    const unchanged = digest(await readFile(configPath)) === digest(originalConfig);
    await writeFile(
      join(outputDirectory, 'validation.json'),
      JSON.stringify(
        {
          capturedAt: new Date().toISOString(),
          product: 'AI Runtime Security',
          readOnly: true,
          configUnchanged: unchanged,
          sessionDebugHttpStatus,
          checks,
          sources: report?.sources.map(({ name, status, records, pages }) => ({
            name,
            status,
            records,
            pages,
          })),
          activity: report
            ? {
                sessions: report.activity.sessions,
                violatingSessions: report.activity.violatingSessions,
              }
            : undefined,
          findings: report?.findings.length,
        },
        null,
        2,
      ),
      { mode: 0o600 },
    );
    expect(unchanged).toBe(true);
  });

  it('collects real daily dashboard and current inventory with explicit source statuses', async () => {
    const client = new ManagementClient({
      clientId: config.mgmtClientId,
      clientSecret: config.mgmtClientSecret,
      tsgId: config.mgmtTsgId,
      apiEndpoint: config.mgmtEndpoint,
      dashboardEndpoint:
        config.mgmtDashboardEndpoint ?? 'https://api.apps.paloaltonetworks.com/aisec',
      tokenEndpoint: config.mgmtTokenEndpoint,
      numRetries: 0,
    });
    report = await collectRuntimeDailyReport(client);
    expect(report.sources).toHaveLength(7);
    expect(report.sources.every((source) => source.status === 'complete')).toBe(true);
    expect(report.sessions.entries).not.toBeNull();
    expect(report.dailyTelemetry.chart.buckets.length).toBeGreaterThan(0);
    expect(report.health).not.toBe('unknown');
    checks.push('SDK daily activity and inventories');
  }, 180_000);

  it('generates default HTML and explicit Markdown through the built public CLI', async () => {
    for (const [format, extension] of [
      ['html', 'html'],
      ['markdown', 'md'],
    ]) {
      const path = join(outputDirectory, `daily.${extension}`);
      const result = await command([
        ...(format === 'markdown' ? ['--output', format] : []),
        '--output-file',
        path,
      ]);
      expect(result.code).toBe(0);
      expect(result.stdout).toBe('');
      const content = await readFile(path, 'utf8');
      expect(content.startsWith(format === 'html' ? '<!doctype html>' : '# Daily')).toBe(true);
      expect(content.includes('Evidence and collection coverage')).toBe(true);
    }
    checks.push('Built CLI HTML and Markdown');
  }, 360_000);

  it('emits a diagnostic deliverable before returning strict completeness status', async () => {
    const path = join(outputDirectory, 'strict.html');
    const result = await command(['--strict', '--output-file', path]);
    const html = await readFile(path, 'utf8');
    const incomplete = /<td>(partial|unavailable)<\/td>/.test(html);
    expect(result.code).toBe(incomplete ? 1 : 0);
    checks.push('Strict-mode artifact and completeness exit status');
  }, 180_000);

  it('refuses to overwrite an existing report without changing its bytes', async () => {
    const path = join(outputDirectory, 'daily.html');
    const original = await readFile(path);
    expect((await command(['--output-file', path])).code).toBe(1);
    expect(digest(await readFile(path)) === digest(original)).toBe(true);
    checks.push('No-clobber file publication');
  });

  it('delivers default HTML and Markdown in the caller working directory', async () => {
    const cwd = join(outputDirectory, 'working-directory');
    await mkdir(cwd, { mode: 0o700 });
    expect((await command([], cwd)).code).toBe(0);
    expect((await command(['--output', 'markdown'], cwd)).code).toBe(0);
    const files = await readdir(cwd);
    expect(files.filter((name) => /^airs-runtime-report-.*\.html$/.test(name))).toHaveLength(1);
    expect(files.filter((name) => /^airs-runtime-report-.*\.md$/.test(name))).toHaveLength(1);
    for (const file of files) expect((await stat(join(cwd, file))).mode & 0o777).toBe(0o600);
    checks.push('Default HTML and Markdown delivered in caller CWD');
  }, 360_000);

  it('runs the replacement session debug command with body-free logs in CWD', async () => {
    const cwd = join(outputDirectory, 'working-directory');
    const result = await runFile(
      process.execPath,
      [
        cliPath,
        '--quiet',
        'runtime',
        'sessions',
        'list',
        '--interval',
        '1',
        '--unit',
        'day',
        '--debug',
        '--output',
        'json',
      ],
      { env, cwd, timeout: 180_000 },
    ).then(
      (result) => ({ code: 0, stdout: result.stdout, stderr: result.stderr }),
      (error: { code?: number; stdout?: string; stderr?: string }) => ({
        code: error.code,
        stdout: error.stdout ?? '',
        stderr: error.stderr ?? '',
      }),
    );
    expect(result.stderr.includes('EROFS')).toBe(false);
    const files = (await readdir(cwd)).filter((name) => /^debug-api-.*\.jsonl$/.test(name));
    expect(files).toHaveLength(1);
    const content = await readFile(join(cwd, files[0]), 'utf8');
    expect(content.length > 0).toBe(true);
    expect(content.includes(config.mgmtClientSecret)).toBe(false);
    expect(content.includes(encodeURIComponent(config.mgmtClientSecret))).toBe(false);
    expect((await stat(join(cwd, files[0]))).mode & 0o777).toBe(0o600);
    const entries = content
      .trim()
      .split('\n')
      .map((line) => JSON.parse(line));
    const queryEntry = entries.find((entry) => entry.request.url.includes('/sessionsoverview'));
    expect(queryEntry).toBeDefined();
    sessionDebugHttpStatus = queryEntry.response.status;
    expect(sessionDebugHttpStatus).toBe(200);
    expect(queryEntry.response.body).toBe('[BODY OMITTED]');
    expect(result.code).toBe(0);
    expect(Array.isArray(JSON.parse(result.stdout))).toBe(true);
    checks.push('Verified session debug workflow and private, body-free CWD log');
  }, 180_000);

  it('omits credential values and raw sensitive field names from deliverables', async () => {
    const secrets = Object.entries(config)
      .filter(
        ([key, value]) =>
          typeof value === 'string' && value.length >= 8 && /secret|api.?key|token/i.test(key),
      )
      .map(([, value]) => value);
    expect(secrets.length > 0).toBe(true);
    for (const filename of ['daily.html', 'daily.md', 'strict.html']) {
      const content = await readFile(join(outputDirectory, filename), 'utf8');
      for (const secret of secrets) {
        expect(
          [secret, encodeURIComponent(secret), Buffer.from(secret).toString('base64')].some(
            (value) => content.includes(value),
          ),
        ).toBe(false);
      }
      expect(/auth_code|api_keys_dp_info|user_ip|scan_result_entries/.test(content)).toBe(false);
    }
    checks.push('Credential and raw-field leak checks');
  });
});
