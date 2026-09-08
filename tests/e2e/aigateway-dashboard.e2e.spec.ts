import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const run = promisify(execFile);
describe.skipIf(process.env.RUN_AIGATEWAY_DASHBOARD_E2E !== '1')(
  'live built AI Gateway dashboard CLI',
  () => {
    const entry = resolve(process.env.AIGATEWAY_DASHBOARD_CLI_ENTRY ?? 'dist/cli/index.js');
    const directory = resolve(
      'artifacts/aigateway-dashboard',
      new Date().toISOString().replaceAll(':', '-'),
    );
    const configPath = resolve(
      process.env.PRISMA_AIRS_CONFIG_PATH ?? join(homedir(), '.prisma-airs/config.json'),
    );
    let original: Buffer;
    let env: NodeJS.ProcessEnv;
    const checks: string[] = [];
    const digest = (b: Buffer) => createHash('sha256').update(b).digest('hex');
    async function command(args: string[]) {
      try {
        const r = await run(
          process.execPath,
          [
            entry,
            '--quiet',
            'aigateway',
            'dashboard',
            '--workspace',
            'ws-develo-71f8d8',
            '--start',
            '2026-09-07T00:00:00Z',
            '--end',
            '2026-09-08T00:00:00Z',
            ...args,
          ],
          {
            env,
            cwd: directory,
            timeout: 300_000,
            maxBuffer: 5_000_000,
          },
        );
        return { code: 0, stdout: r.stdout };
      } catch (e) {
        const r = e as { code?: number; stdout?: string };
        return { code: typeof r.code === 'number' ? r.code : -1, stdout: r.stdout ?? '' };
      }
    }
    beforeAll(async () => {
      original = await readFile(configPath);
      env = {
        ...process.env,
        PRISMA_AIRS_CONFIG_PATH: configPath,
        PANW_AI_SEC_DEBUG: '0',
        PANW_AI_SEC_DEBUG_BODY: '0',
        PANW_AI_SEC_TIMEOUT_MS: '20000',
      };
      await mkdir(directory, { recursive: true, mode: 0o700 });
    });
    afterAll(async () => {
      const configUnchanged = digest(await readFile(configPath)) === digest(original);
      await writeFile(
        join(directory, 'validation.json'),
        JSON.stringify(
          {
            capturedAt: new Date().toISOString(),
            checks,
            configUnchanged,
            readOnly: true,
            directory,
          },
          null,
          2,
        ),
        { mode: 0o600 },
      );
      expect(configUnchanged).toBe(true);
      console.log(JSON.stringify({ checks: checks.length, configUnchanged, directory }));
    });
    it('creates default private HTML in CWD with all 25 sources complete', async () => {
      expect((await command(['--strict'])).code).toBe(0);
      const names = (await readdir(directory)).filter((n) => n.endsWith('.html'));
      expect(names).toHaveLength(1);
      const html = await readFile(join(directory, names[0]), 'utf8');
      expect(html).toMatch(/^<!doctype html>/);
      expect(html).toContain('Evidence and completeness');
      expect((html.match(/<td>complete<\/td>/g) ?? []).length).toBe(25);
      expect((await stat(join(directory, names[0]))).mode & 0o777).toBe(0o600);
      checks.push('Default CWD HTML; 25/25 complete sources; mode 0600');
    }, 300_000);
    it('creates Markdown and streams actual command output without decoration', async () => {
      const result = await command(['--strict', '--output', 'markdown', '--output-file', '-']);
      expect(result.code).toBe(0);
      expect(result.stdout).toMatch(/^# AI Gateway/);
      expect(result.stdout).not.toContain('\u001b');
      await writeFile(join(directory, 'actual-stdout.md'), result.stdout, { mode: 0o600 });
      const file = await command(['--strict', '--output', 'markdown']);
      expect(file.code).toBe(0);
      const names = (await readdir(directory)).filter(
        (n) => n.startsWith('airs-aigateway-report-') && n.endsWith('.md'),
      );
      expect(names).toHaveLength(1);
      expect((await stat(join(directory, names[0]))).mode & 0o777).toBe(0o600);
      checks.push('Actual Markdown stdout; default CWD Markdown; mode 0600');
    }, 600_000);
    it('writes a partial artifact then exits 1 when strict page budget is exhausted', async () => {
      const r = await command(['--strict', '--max-pages', '1', '--output-file', 'partial.html']);
      expect(r.code).toBe(1);
      expect(await readFile(join(directory, 'partial.html'), 'utf8')).toContain('<td>partial</td>');
      checks.push('Strict pagination budget preserves partial deliverable');
    }, 300_000);
    it('refuses overwrite and debug logging without changing existing files', async () => {
      const original = await readFile(join(directory, 'actual-stdout.md'));
      expect((await command(['--output-file', 'actual-stdout.md'])).code).toBe(1);
      expect(await readFile(join(directory, 'actual-stdout.md'))).toEqual(original);
      expect((await command(['--debug'])).code).toBe(2);
      expect((await readdir(directory)).some((n) => n.startsWith('debug-api-'))).toBe(false);
      checks.push('No-clobber and debug privacy guards');
    });
    it('runs all five new atomic reads and retrieves distinct transaction pages through the CLI', async () => {
      const read = async (args: string[]) => {
        const result = await run(
          process.execPath,
          [entry, '--quiet', 'aigateway', ...args, '--output', 'json'],
          { env, cwd: directory, timeout: 60000, maxBuffer: 5000000 },
        );
        return JSON.parse(result.stdout);
      };
      const window = [
        '--workspace',
        'ws-develo-71f8d8',
        '--start',
        '2026-09-07T00:00:00Z',
        '--end',
        '2026-09-08T00:00:00Z',
      ];
      const errors = await read(['telemetry', 'error-category-trends', ...window]);
      expect(errors.success).toBe(true);
      expect(Array.isArray(errors.data.trend)).toBe(true);
      const grouped = await read(['telemetry', 'grouped-errors', ...window]);
      expect(grouped.success).toBe(true);
      expect(Array.isArray(grouped.data.trend)).toBe(true);
      const filters = await read(['telemetry', 'filter-boundaries', ...window]);
      expect(filters.success).toBe(true);
      expect(filters.data).toBe('[REDACTED]');
      const config = JSON.parse(original.toString());
      const tsgId = config.mgmtTsgId ?? config.management?.tsgId;
      expect(typeof tsgId).toBe('string');
      const organisation = await read(['organisations', 'info', '--tsg-id', tsgId]);
      expect(organisation.settings).toBe('[REDACTED]');
      const catalog = await read(['guardrails', 'catalog']);
      expect(catalog.evals.length).toBeGreaterThan(0);
      const first = await read(['telemetry', 'logs', 'list', ...window, '--current-page', '0']);
      const second = await read(['telemetry', 'logs', 'list', ...window, '--current-page', '1']);
      expect(first.success && second.success).toBe(true);
      const ids = new Set(first.data.records.map((r: { id: string }) => r.id));
      expect(ids.size).toBe(50);
      expect(second.data.records.length).toBeGreaterThan(0);
      expect(second.data.records.every((r: { id: string }) => !ids.has(r.id))).toBe(true);
      checks.push(
        'Five atomic CLI reads; default sensitive-field redaction; distinct zero-based pages 0 and 1',
      );
    }, 300000);
  },
);
