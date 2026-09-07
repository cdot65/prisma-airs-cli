import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const run = promisify(execFile);
describe.skipIf(process.env.RUN_REDTEAM_DASHBOARD_E2E !== '1')(
  'live built Red Team dashboard CLI',
  () => {
    const entry = resolve(process.env.REDTEAM_DASHBOARD_CLI_ENTRY ?? 'dist/cli/index.js');
    const directory = resolve(
      'artifacts/redteam-dashboard',
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
        const r = await run(process.execPath, [entry, '--quiet', 'redteam', 'dashboard', ...args], {
          env,
          cwd: directory,
          timeout: 180_000,
          maxBuffer: 5_000_000,
        });
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
    it('creates default private HTML in CWD with all seven sources complete', async () => {
      expect((await command(['--strict'])).code).toBe(0);
      const names = (await readdir(directory)).filter((n) => n.endsWith('.html'));
      expect(names).toHaveLength(1);
      const html = await readFile(join(directory, names[0]), 'utf8');
      expect(html).toMatch(/^<!doctype html>/);
      expect(html).toContain('Evidence and completeness');
      expect((html.match(/<td>complete<\/td>/g) ?? []).length).toBe(7);
      expect((await stat(join(directory, names[0]))).mode & 0o777).toBe(0o600);
      checks.push('Default CWD HTML; 7/7 complete sources; mode 0600');
    }, 180_000);
    it('creates Markdown and streams actual command output without decoration', async () => {
      const result = await command(['--strict', '--output', 'markdown', '--output-file', '-']);
      expect(result.code).toBe(0);
      expect(result.stdout).toMatch(/^# Red Team/);
      expect(result.stdout).not.toContain('\u001b');
      await writeFile(join(directory, 'actual-stdout.md'), result.stdout, { mode: 0o600 });
      const file = await command(['--strict', '--output', 'markdown']);
      expect(file.code).toBe(0);
      const names = (await readdir(directory)).filter(
        (n) => n.startsWith('airs-redteam-report-') && n.endsWith('.md'),
      );
      expect(names).toHaveLength(1);
      expect((await stat(join(directory, names[0]))).mode & 0o777).toBe(0o600);
      checks.push('Actual Markdown stdout; default CWD Markdown; mode 0600');
    }, 360_000);
    it('writes a partial artifact then exits 1 when strict page budget is exhausted', async () => {
      const r = await command(['--strict', '--max-pages', '1', '--output-file', 'partial.html']);
      expect(r.code).toBe(1);
      expect(await readFile(join(directory, 'partial.html'), 'utf8')).toContain('<td>partial</td>');
      checks.push('Strict pagination budget preserves partial deliverable');
    }, 180_000);
    it('refuses overwrite and debug logging without changing existing files', async () => {
      const original = await readFile(join(directory, 'actual-stdout.md'));
      expect((await command(['--output-file', 'actual-stdout.md'])).code).toBe(1);
      expect(await readFile(join(directory, 'actual-stdout.md'))).toEqual(original);
      expect((await command(['--debug'])).code).toBe(2);
      expect((await readdir(directory)).some((n) => n.startsWith('debug-api-'))).toBe(false);
      checks.push('No-clobber and debug privacy guards');
    });
  },
);
