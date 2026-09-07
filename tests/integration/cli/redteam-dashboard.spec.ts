import { mkdtemp, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { redTeamReportClient } from '../../helpers/redteam-report.js';

const factory = vi.hoisted(() => ({ RedTeamClient: vi.fn() }));
vi.mock('@cdot65/prisma-airs-sdk', async (original) => ({
  ...(await original<typeof import('@cdot65/prisma-airs-sdk')>()),
  RedTeamClient: factory.RedTeamClient,
}));

import { buildProgram } from '../../../src/cli/program.js';
import { setQuiet } from '../../../src/cli/renderer/ui.js';

describe('airs redteam dashboard command', () => {
  let directory: string;
  let client: ReturnType<typeof redTeamReportClient>;
  beforeEach(async () => {
    vi.clearAllMocks();
    process.exitCode = undefined;
    directory = await mkdtemp(join(tmpdir(), 'airs-report-command-'));
    vi.stubEnv('PRISMA_AIRS_CONFIG_PATH', join(directory, 'config.json'));
    vi.stubEnv('PANW_CLI_OUTPUT', 'json'); // Artifact default deliberately independent of terminal format.
    client = redTeamReportClient();
    factory.RedTeamClient.mockReturnValue(client);
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });
  afterEach(async () => {
    process.exitCode = undefined;
    setQuiet(false);
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
    await rm(directory, { recursive: true, force: true });
  });
  const run = (args: string[] = []) =>
    buildProgram().parseAsync(['node', 'airs', '--quiet', 'redteam', 'dashboard', ...args]);

  it('writes HTML by default despite a general JSON output preference', async () => {
    const destination = join(directory, 'daily.html');
    await run(['--output-file', destination]);
    expect(await readFile(destination, 'utf8')).toMatch(/^<!doctype html>/);
    expect(process.exitCode).toBeUndefined();
    expect(factory.RedTeamClient.mock.calls[0][0]).toMatchObject({ numRetries: 0 });
  });

  it('writes Markdown and honors explicit global and local artifact format flags', async () => {
    const first = join(directory, 'first.md');
    await buildProgram().parseAsync([
      'node',
      'airs',
      '--output',
      'markdown',
      'redteam',
      'dashboard',
      '--output-file',
      first,
    ]);
    expect(await readFile(first, 'utf8')).toMatch(/^# Red/);
    const second = join(directory, 'second.html');
    await buildProgram().parseAsync([
      'node',
      'airs',
      '--output',
      'markdown',
      'redteam',
      'dashboard',
      '--output',
      'html',
      '--output-file',
      second,
    ]);
    expect(await readFile(second, 'utf8')).toMatch(/^<!doctype/);
  });

  it('emits only the artifact on stdout and propagates stream errors', async () => {
    const write = vi.spyOn(process.stdout, 'write').mockImplementation((_chunk, callback) => {
      if (typeof callback === 'function') callback();
      return true;
    });
    await run(['--output', 'markdown', '--output-file', '-']);
    expect(write).toHaveBeenCalledOnce();
    expect(write.mock.calls[0][0]).toMatch(/^# Red/);
    write.mockImplementation((_chunk, callback) => {
      if (typeof callback === 'function') callback(new Error('broken pipe'));
      return false;
    });
    await run(['--output-file', '-']);
    expect(process.exitCode).toBe(1);
  });

  it('delivers unique default HTML and Markdown files in the current working directory', async () => {
    vi.spyOn(process, 'cwd').mockReturnValue(directory);
    await run();
    await run(['--output', 'markdown']);
    const files = (await readdir(directory)).filter((name) =>
      name.startsWith('airs-redteam-report-'),
    );
    expect(files).toHaveLength(2);
    const htmlName = files.find((name) => name.endsWith('.html'));
    const markdownName = files.find((name) => name.endsWith('.md'));
    expect(htmlName).toBeDefined();
    expect(markdownName).toBeDefined();
    expect(await readFile(join(directory, htmlName ?? ''), 'utf8')).toMatch(/^<!doctype/);
    expect(await readFile(join(directory, markdownName ?? ''), 'utf8')).toMatch(/^# Red/);
  });

  it('writes useful partial reports successfully; strict mode then exits 1', async () => {
    client.scans.list.mockResolvedValue({});
    const first = join(directory, 'partial.html');
    await run(['--output-file', first]);
    expect(process.exitCode).toBeUndefined();
    expect(await readFile(first, 'utf8')).toContain('unavailable');
    const second = join(directory, 'strict.html');
    await run(['--strict', '--output-file', second]);
    expect(process.exitCode).toBe(1);
    expect(await readFile(second, 'utf8')).toContain('unavailable');
  });

  it('exits 1 and still creates a diagnostic report when no source is available', async () => {
    for (const method of [
      client.getDashboardOverview,
      client.getScanStatistics,
      client.getQuotaSummary,
      client.targets.list,
      client.scans.list,
      client.adapters.list,
      client.networkBroker.getChannelStats,
    ])
      method.mockRejectedValue(new Error('PRIVATE-ERROR'));
    const destination = join(directory, 'unavailable.html');
    await run(['--output-file', destination]);
    const content = await readFile(destination, 'utf8');
    expect(content).toContain('Unable to assess');
    expect(content).not.toContain('PRIVATE-ERROR');
    expect(process.exitCode).toBe(1);
  });

  it('refuses existing paths before loading credentials or making network calls', async () => {
    const destination = join(directory, 'existing.html');
    await writeFile(destination, 'KEEP');
    await run(['--output-file', destination]);
    expect(process.exitCode).toBe(1);
    expect(factory.RedTeamClient).not.toHaveBeenCalled();
    expect(await readFile(destination, 'utf8')).toBe('KEEP');
  });

  it('handles invalid config without exposing file contents', async () => {
    vi.stubEnv('SCAN_CONCURRENCY', 'SECRET');
    await run(['--output-file', join(directory, 'invalid.html')]);
    expect(process.exitCode).toBe(1);
    expect(JSON.stringify(vi.mocked(console.error).mock.calls)).not.toContain('SECRET');
  });

  it.each([
    ['--output', 'json'],
    ['--max-pages', '0'],
    ['--max-pages', '1.5'],
    ['--max-pages', '101'],
    ['--title', ''],
    ['--title', 'x'.repeat(241)],
    ['--debug'],
  ])('rejects invalid options before client creation: %j', async (...args) => {
    const exit = vi.spyOn(process, 'exit').mockImplementation(() => {
      throw new Error('exit');
    });
    await expect(run(args)).rejects.toThrow('exit');
    expect(exit).toHaveBeenCalledWith(2);
    expect(factory.RedTeamClient).not.toHaveBeenCalled();
  });
});
