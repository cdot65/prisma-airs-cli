import { mkdtemp, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import * as debugLogger from '../../../src/cli/debug-logger.js';
import { buildProgram } from '../../../src/cli/program.js';
import { setQuiet } from '../../../src/cli/renderer/ui.js';

describe('debug deliverables in the working directory', () => {
  let directory: string;
  const originalFetch = globalThis.fetch;
  beforeEach(async () => {
    directory = await mkdtemp(join(tmpdir(), 'airs-debug-cwd-'));
    vi.spyOn(process, 'cwd').mockReturnValue(directory);
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });
  afterEach(async () => {
    globalThis.fetch = originalFetch;
    setQuiet(false);
    vi.restoreAllMocks();
    await rm(directory, { recursive: true, force: true });
  });
  function program() {
    const cli = buildProgram();
    cli.command('debug-probe').action(() => {});
    return cli;
  }

  it('creates a private log in CWD without touching config or pruning previous artifacts', async () => {
    for (let i = 0; i < 12; i++)
      await writeFile(join(directory, `debug-api-old-${i}.jsonl`), 'KEEP');
    await program().parseAsync(['node', 'airs', '--debug', 'debug-probe']);
    const files = (await readdir(directory)).filter((name) => name.endsWith('.jsonl'));
    expect(files).toHaveLength(13);
    const created = files.find((name) => !name.includes('-old-'));
    expect(created).toBeDefined();
    expect((await stat(join(directory, created ?? ''))).mode & 0o777).toBe(0o600);
    expect(await readFile(join(directory, 'debug-api-old-0.jsonl'), 'utf8')).toBe('KEEP');
  });

  it('reports an unwritable working directory without exposing an EROFS stack trace', async () => {
    const action = vi.fn();
    vi.spyOn(debugLogger, 'installDebugLogger').mockImplementation(() => {
      throw Object.assign(new Error('EROFS private/path'), { code: 'EROFS' });
    });
    const exit = vi.spyOn(process, 'exit').mockImplementation(() => {
      throw new Error('exit');
    });
    const cli = buildProgram();
    cli.command('debug-probe').action(action);
    await expect(cli.parseAsync(['node', 'airs', '--debug', 'debug-probe'])).rejects.toThrow(
      'exit',
    );
    expect(exit).toHaveBeenCalledWith(1);
    expect(action).not.toHaveBeenCalled();
    const messages = vi.mocked(console.error).mock.calls.flat().join('\n');
    expect(messages).toContain('current working directory');
    expect(messages).not.toContain('private/path');
  });
});
