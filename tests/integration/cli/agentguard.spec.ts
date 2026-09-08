import { mkdtemp, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { client, scan } from '../../helpers/agentguard.js';

const factory = vi.hoisted(() => ({ create: vi.fn() }));
vi.mock('@cdot65/prisma-airs-sdk', async (original) => ({
  ...(await original<typeof import('@cdot65/prisma-airs-sdk')>()),
  AgentGuardClient: class {
    constructor() {
      Object.assign(this, factory.create());
    }
  },
}));

import { buildProgram } from '../../../src/cli/program.js';
import { setQuiet } from '../../../src/cli/renderer/ui.js';

describe('AgentGuard CLI', () => {
  let directory: string;
  let c: ReturnType<typeof client>;
  beforeEach(async () => {
    vi.clearAllMocks();
    process.exitCode = undefined;
    directory = await mkdtemp(join(tmpdir(), 'airs-agentguard-test-'));
    vi.stubEnv('PRISMA_AIRS_CONFIG_PATH', join(directory, 'config.json'));
    vi.stubEnv('PANW_CLI_OUTPUT', 'json');
    c = client();
    factory.create.mockReturnValue(c);
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });
  afterEach(async () => {
    process.exitCode = undefined;
    setQuiet(false);
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
    await rm(directory, { recursive: true, force: true });
  });
  const run = (args: string[]) =>
    buildProgram().parseAsync(['node', 'airs', '--quiet', 'agentguard', ...args]);

  it.each([
    'json',
    'yaml',
    'pretty',
    'table',
    'markdown',
    'csv',
  ])('renders scans in %s without sensitive metadata', async (format) => {
    const log = vi.spyOn(console, 'log').mockImplementation(() => {});
    await run(['scans', 'list', '--output', format]);
    expect(JSON.stringify(log.mock.calls)).not.toContain('PRIVATE-');
    expect(JSON.stringify(log.mock.calls)).toContain('BLOCKED');
    expect(process.exitCode).toBeUndefined();
  });
  it('walks --all pages and exposes cap/truncation metadata', async () => {
    const log = vi.spyOn(console, 'log').mockImplementation(() => {});
    c.listScans
      .mockResolvedValueOnce({ scans: [scan('first')], pagination: { total_items: 2 } })
      .mockResolvedValueOnce({ scans: [scan('second')], pagination: { total_items: 2 } });
    await run(['scans', 'list', '--all', '--limit', '1']);
    expect(JSON.parse(log.mock.calls[0][0]).pagination).toMatchObject({
      returned: 2,
      truncated: false,
    });
    log.mockClear();
    c.listScans.mockResolvedValue({ scans: [scan()], pagination: { total_items: 2 } });
    await run(['scans', 'list', '--all', '--max', '1']);
    expect(JSON.parse(log.mock.calls[0][0]).pagination).toMatchObject({
      returned: 1,
      truncated: true,
      next_offset: 1,
    });
  });
  it('refuses duplicate pages instead of silently claiming --all succeeded', async () => {
    vi.spyOn(console, 'log').mockImplementation(() => {});
    c.listScans.mockResolvedValue({ scans: [scan()], pagination: { total_items: 2 } });
    await run(['scans', 'list', '--all']);
    expect(process.exitCode).toBe(1);
  });
  it('provides explicit finding content opt-in', async () => {
    const log = vi.spyOn(console, 'log').mockImplementation(() => {});
    const id = scan().uuid;
    await run(['scans', 'vulnerabilities', id]);
    expect(JSON.stringify(log.mock.calls)).not.toContain('PRIVATE-');
    log.mockClear();
    await run(['scans', 'vulnerabilities', id, '--include-content']);
    expect(JSON.stringify(log.mock.calls)).toContain('PRIVATE-CODE');
  });
  it('reads statistics and rule catalog', async () => {
    const log = vi.spyOn(console, 'log').mockImplementation(() => {});
    await run(['stats']);
    await run(['rules', 'list']);
    expect(c.getScanStats).toHaveBeenCalledWith({ time_period: '30_DAYS' });
    expect(c.listRules).toHaveBeenCalledWith({ skip: 0, limit: 10 });
    expect(log).toHaveBeenCalledTimes(2);
  });

  it('walks rules until a short page, ignoring the server page-count total', async () => {
    const log = vi.spyOn(console, 'log').mockImplementation(() => {});
    c.listRules
      .mockResolvedValueOnce({
        rules: [{ uuid: 'one' }, { uuid: 'two' }],
        pagination: { total_items: 2 },
      })
      .mockResolvedValueOnce({ rules: [{ uuid: 'three' }], pagination: { total_items: 1 } });
    await run(['rules', 'list', '--all', '--limit', '2']);
    const p = JSON.parse(log.mock.calls[0][0]);
    expect(p.rules).toHaveLength(3);
    expect(p.pagination).toMatchObject({ total_items: 3, truncated: false });
    expect(c.listRules.mock.calls[1][0]).toMatchObject({ skip: 2, limit: 2 });
  });

  it('preserves unavailable scan metrics as null', async () => {
    const log = vi.spyOn(console, 'log').mockImplementation(() => {});
    c.listScans.mockResolvedValue({
      scans: [{ ...scan(), status: 'FAILED', summary: null, eval_outcome: null }],
      pagination: { total_items: 1 },
    });
    await run(['scans', 'list']);
    expect(JSON.parse(log.mock.calls[0][0]).scans[0]).toMatchObject({
      vulnerability_count: null,
      attack_chain_count: null,
      eval_outcome: null,
    });
  });
  it('writes private default HTML and explicit Markdown artifacts in CWD', async () => {
    vi.spyOn(process, 'cwd').mockReturnValue(directory);
    await run(['report']);
    await run(['report', '--output', 'markdown']);
    const files = (await readdir(directory)).filter((n) => n.startsWith('airs-agentguard-report-'));
    expect(files).toHaveLength(2);
    for (const f of files) {
      const text = await readFile(join(directory, f), 'utf8');
      expect(text).not.toContain('PRIVATE-');
      expect((await stat(join(directory, f))).mode & 0o777).toBe(0o600);
    }
    expect(
      await readFile(join(directory, files.find((f) => f.endsWith('.html')) ?? ''), 'utf8'),
    ).toMatch(/^<!doctype/);
  });
  it('refuses overwrite before constructing a client', async () => {
    const dest = join(directory, 'keep.html');
    await writeFile(dest, 'KEEP');
    await run(['report', '--output-file', dest]);
    expect(factory.create).not.toHaveBeenCalled();
    expect(await readFile(dest, 'utf8')).toBe('KEEP');
    expect(process.exitCode).toBe(1);
  });
  it('emits only the artifact to stdout and propagates a broken pipe', async () => {
    const write = vi.spyOn(process.stdout, 'write').mockImplementation((_chunk, callback) => {
      if (typeof callback === 'function') callback();
      return true;
    });
    await run(['report', '--output', 'markdown', '--output-file', '-']);
    expect(write).toHaveBeenCalledOnce();
    expect(write.mock.calls[0][0]).toMatch(/^# AgentGuard/);
    write.mockImplementation((_chunk, callback) => {
      if (typeof callback === 'function') callback(new Error('PIPE'));
      return false;
    });
    await run(['report', '--output-file', '-']);
    expect(process.exitCode).toBe(1);
  });
  it('writes diagnostic reports but strict mode fails incomplete collection', async () => {
    c.listRules.mockRejectedValue(new Error('PRIVATE-ERROR'));
    const path = join(directory, 'partial.md');
    await run(['report', '--strict', '--output', 'markdown', '--output-file', path]);
    expect(process.exitCode).toBe(1);
    expect(await readFile(path, 'utf8')).toContain('unavailable');
    expect(await readFile(path, 'utf8')).not.toContain('PRIVATE-');
  });
  it.each([
    ['scans', 'list', '--limit', '0'],
    ['scans', 'list', '--start', 'bad'],
    ['stats', '--time-period', 'week'],
    ['scans', 'vulnerabilities', '../escape'],
    ['report', '--max-pages', '0'],
    ['report', '--output', 'json'],
    ['report', '--debug'],
  ])('rejects invalid arguments before API work: %j', async (...args) => {
    const exit = vi.spyOn(process, 'exit').mockImplementation(() => {
      throw new Error('EXIT');
    });
    await expect(run(args)).rejects.toThrow();
    expect(factory.create).not.toHaveBeenCalled();
    expect(exit).toHaveBeenCalled();
  });
});
