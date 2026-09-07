import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { describe, expect, it } from 'vitest';

const execute = promisify(execFile);

describe.skipIf(process.env.RUN_RUNTIME_REPORT_E2E !== '1')('live built CLI SCM dashboard', () => {
  it('authenticates, enumerates, drills down and reads content without persisting customer text', async () => {
    const entry = resolve(process.env.RUNTIME_REPORT_CLI_ENTRY ?? 'dist/cli/index.js');
    const configPath = resolve(
      process.env.PRISMA_AIRS_CONFIG_PATH ?? join(homedir(), '.prisma-airs/config.json'),
    );
    const original = await readFile(configPath);
    const config = JSON.parse(original.toString('utf8')) as Record<string, string>;
    const env = {
      ...process.env,
      PRISMA_AIRS_CONFIG_PATH: configPath,
      PANW_AI_SEC_DEBUG: '0',
      PANW_AI_SEC_DEBUG_BODY: '0',
      PANW_AI_SEC_TIMEOUT_MS: '20000',
    };
    for (const [field, variable] of Object.entries({
      mgmtClientId: 'PANW_MGMT_CLIENT_ID',
      mgmtClientSecret: 'PANW_MGMT_CLIENT_SECRET',
      mgmtTsgId: 'PANW_MGMT_TSG_ID',
      mgmtEndpoint: 'PANW_MGMT_ENDPOINT',
      mgmtTokenEndpoint: 'PANW_MGMT_TOKEN_ENDPOINT',
      mgmtDashboardEndpoint: 'PANW_MGMT_DASHBOARD_ENDPOINT',
    })) {
      if (config[field]) env[variable] = config[field];
      else delete env[variable];
    }
    const directory = resolve(
      'artifacts/runtime-dashboard',
      new Date().toISOString().replaceAll(':', '-'),
    );
    await mkdir(directory, { recursive: true, mode: 0o700 });
    const checks: string[] = [];
    let sessions = 0;
    async function read(args: string[]) {
      let stdout = '';
      try {
        ({ stdout } = await execute(
          process.execPath,
          [entry, '--quiet', 'runtime', ...args, '--output', 'json'],
          { env, cwd: directory, timeout: 180000, maxBuffer: 30_000_000 },
        ));
      } catch {
        // Never attach raw stderr, arguments, output, or SDK errors to the test report.
        throw new Error(`CLI read failed: ${args[0]} ${args[1]}`);
      }
      return JSON.parse(stdout);
    }
    try {
      const apps = await read(['dashboard', 'applications']);
      expect(Array.isArray(apps.items)).toBe(true);
      expect(apps.items.length > 0).toBe(true);
      checks.push('applications overview');
      const app = apps.items[0];
      const appArgs = ['--app-id', app.id, '--app-name', app.name];
      const detail = await read(['dashboard', 'application', ...appArgs]);
      expect(detail.id === app.id && detail.name === app.name).toBe(true);
      checks.push('application detail');
      const violations = await read(['dashboard', 'application-violations', ...appArgs]);
      expect(Array.isArray(violations.detection_type_violation_breakdown)).toBe(true);
      checks.push('application violation breakdown');
      expect(Array.isArray((await read(['dashboard', 'top-applications'])).applications)).toBe(
        true,
      );
      checks.push('top applications');
      expect(Array.isArray((await read(['dashboard', 'violations-trend'])).violations)).toBe(true);
      checks.push('application violations trend');
      expect(Array.isArray((await read(['dashboard', 'apps-list'])).applications)).toBe(true);
      checks.push('application identity list');
      expect(Array.isArray((await read(['sessions', 'chart'])).buckets)).toBe(true);
      checks.push('session chart');
      const page = await read(['sessions', 'list']);
      expect(Array.isArray(page) && page.length > 0).toBe(true);
      checks.push('session inventory page');
      const all = await read(['sessions', 'list', '--all', '--max', '0']);
      expect(Array.isArray(all) && all.length >= page.length).toBe(true);
      expect(
        new Set(
          all.map((row) =>
            JSON.stringify([row.application_id, row.application_name, row.session_id]),
          ),
        ).size,
      ).toBe(all.length);
      sessions = all.length;
      checks.push('complete session inventory with stable pagination');
      const selected =
        page.find((row) => row.application_name === 'prisma-airs-terminal') ?? page[0];
      const identity = [
        '--session-id',
        selected.session_id,
        '--app-id',
        selected.application_id,
        '--app-name',
        selected.application_name,
      ];
      const session = await read(['sessions', 'get', ...identity]);
      expect(session.id === selected.session_id && session.session_actions.length > 0).toBe(true);
      checks.push('session detail');
      const action = session.session_actions[0];
      const scan = [
        '--scan-id',
        action.scan_id,
        '--scan-sub-req-id',
        String(action.scan_sub_req_id),
      ];
      const transaction = await read(['sessions', 'transaction', ...identity, ...scan]);
      expect(
        transaction.scan_id === action.scan_id &&
          transaction.scan_sub_req_id === action.scan_sub_req_id,
      ).toBe(true);
      checks.push('session transaction with exact sub-request index');
      const content = await read(['sessions', 'scan-content', ...scan, '--show-content']);
      expect(
        content.scan_id === action.scan_id && content.sub_scan_req_id === action.scan_sub_req_id,
      ).toBe(true);
      expect(Object.hasOwn(content, 'scan_contents')).toBe(true);
      checks.push('explicit stored content (not logged or saved)');
      let legacyCode = 0;
      try {
        await execute(
          process.execPath,
          [
            entry,
            '--quiet',
            'runtime',
            'scan-logs',
            'query',
            '--interval',
            '24',
            '--unit',
            'hours',
          ],
          { env, timeout: 30000 },
        );
      } catch (error) {
        legacyCode = Number((error as { code?: number }).code);
      }
      expect(legacyCode).toBe(1);
      checks.push('legacy scan-logs exits 1 instead of fabricating empty results');
    } finally {
      const unchanged =
        createHash('sha256').update(original).digest('hex') ===
        createHash('sha256')
          .update(await readFile(configPath))
          .digest('hex');
      await writeFile(
        join(directory, 'validation.json'),
        JSON.stringify(
          {
            capturedAt: new Date().toISOString(),
            checks,
            sessions,
            expectedChecks: 13,
            complete: checks.length === 13,
            configUnchanged: unchanged,
            readOnly: true,
            contentPersisted: false,
          },
          null,
          2,
        ),
        { mode: 0o600 },
      );
      expect(unchanged).toBe(true);
    }
  }, 360000);
});
