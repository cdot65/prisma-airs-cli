import type { Command } from 'commander';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { resolveDeprecatedAliases } from '../../../src/cli/deprecated-flags.js';
import { resolvePageParams } from '../../../src/cli/pagination.js';
import { buildProgram } from '../../../src/cli/program.js';

const stripAnsi = (s: string) => s.replace(/\[[0-9;]*m/g, '');

/** Walk the command tree by names, e.g. find(program, 'runtime', 'topics', 'create'). */
function find(parent: Command, ...names: string[]): Command {
  let cmd = parent;
  for (const name of names) {
    const next = cmd.commands.find((c) => c.name() === name);
    if (!next) throw new Error(`command not found: ${names.join(' ')} (missing "${name}")`);
    cmd = next;
  }
  return cmd;
}

function flagsOf(cmd: Command): string[] {
  return cmd.options.map((o) => o.long ?? '');
}

function hiddenFlagsOf(cmd: Command): string[] {
  return cmd.options.filter((o) => o.hidden).map((o) => o.long ?? '');
}

function visibleFlagsOf(cmd: Command): string[] {
  return cmd.options.filter((o) => !o.hidden).map((o) => o.long ?? '');
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('flag standardization — structural: canonical flag + hidden alias registered', () => {
  const program = buildProgram();

  const renamed: Array<{ path: string[]; canonical: string[]; aliases: string[] }> = [
    // --format → --output (format semantics)
    { path: ['runtime', 'topics', 'create'], canonical: ['--output'], aliases: ['--format'] },
    { path: ['runtime', 'topics', 'apply'], canonical: ['--output'], aliases: ['--format'] },
    { path: ['runtime', 'topics', 'eval'], canonical: ['--output'], aliases: ['--format'] },
    { path: ['runtime', 'topics', 'revert'], canonical: ['--output'], aliases: ['--format'] },
    {
      path: ['redteam', 'targets', 'backup'],
      canonical: ['--file-format'],
      aliases: ['--output', '--format'],
    },
    // --output file-path overload → --output-file
    {
      path: ['runtime', 'bulk-scan'],
      canonical: ['--file', '--output-file'],
      aliases: ['--input', '--output'],
    },
    { path: ['runtime', 'resume-poll'], canonical: ['--output-file'], aliases: ['--output'] },
    { path: ['runtime', 'topics', 'sample'], canonical: ['--output-file'], aliases: ['--output'] },
    {
      path: ['redteam', 'prompt-sets', 'download'],
      canonical: ['--output-file'],
      aliases: ['--output'],
    },
    { path: ['redteam', 'targets', 'init'], canonical: ['--output-file'], aliases: ['--output'] },
    // pagination unification
    {
      path: ['runtime', 'scan-logs', 'query'],
      canonical: ['--limit', '--offset'],
      aliases: ['--page', '--page-size'],
    },
    {
      path: ['runtime', 'dlp', 'filtering-profiles', 'list'],
      canonical: ['--limit', '--offset'],
      aliases: ['--page', '--size'],
    },
    {
      path: ['runtime', 'dlp', 'patterns', 'list'],
      canonical: ['--limit', '--offset'],
      aliases: ['--page', '--size'],
    },
    {
      path: ['runtime', 'dlp', 'profiles', 'list'],
      canonical: ['--limit', '--offset'],
      aliases: ['--page', '--size'],
    },
    {
      path: ['runtime', 'dlp', 'dictionaries', 'list'],
      canonical: ['--limit', '--offset'],
      aliases: ['--page', '--size'],
    },
    // --confirm → --force
    { path: ['redteam', 'eula', 'accept'], canonical: ['--force'], aliases: ['--confirm'] },
  ];

  for (const { path, canonical, aliases } of renamed) {
    it(`${path.join(' ')}: canonical ${canonical.join(', ')} visible; ${aliases.join(', ')} hidden`, () => {
      const cmd = find(program, ...path);
      for (const flag of canonical) {
        expect(visibleFlagsOf(cmd)).toContain(flag);
      }
      for (const alias of aliases) {
        expect(hiddenFlagsOf(cmd)).toContain(alias);
      }
      // Aliases must not leak into help output.
      const help = cmd.helpInformation();
      for (const alias of aliases) {
        expect(help).not.toContain(`${alias} `);
      }
    });
  }

  it('redteam lists gained client-side --limit/--offset', () => {
    for (const path of [
      ['redteam', 'prompt-sets', 'list'],
      ['redteam', 'properties', 'list'],
      ['redteam', 'targets', 'list'],
    ]) {
      const cmd = find(program, ...path);
      expect(flagsOf(cmd)).toContain('--limit');
      expect(flagsOf(cmd)).toContain('--offset');
    }
  });

  it('redteam read commands gained --output <format>', () => {
    for (const path of [
      ['redteam', 'prompts', 'list'],
      ['redteam', 'prompts', 'get'],
      ['redteam', 'instances', 'get'],
      ['redteam', 'registry-credentials'],
    ]) {
      const cmd = find(program, ...path);
      expect(flagsOf(cmd)).toContain('--output');
    }
  });
});

describe('read-command contract', () => {
  function walk(
    command: Command,
    path: string[] = [],
  ): Array<{ command: Command; path: string[] }> {
    return command.commands.flatMap((child) => {
      const childPath = [...path, child.name()];
      return [{ command: child, path: childPath }, ...walk(child, childPath)];
    });
  }

  it('every get/list command accepts --output', () => {
    const missing = walk(buildProgram())
      .filter(({ command }) => command.name() === 'get' || command.name() === 'list')
      .filter(({ command }) => !flagsOf(command).includes('--output'))
      .map(({ path }) => path.join(' '));
    expect(missing).toEqual([]);
  });

  it('paginated list commands register --limit, --offset, and --all as one contract', () => {
    const partial = walk(buildProgram())
      .filter(({ command }) => command.name() === 'list')
      .filter(({ command }) => {
        const flags = flagsOf(command);
        if (!flags.includes('--limit') && !flags.includes('--offset')) return false;
        const count = ['--limit', '--offset', '--all'].filter((flag) =>
          flags.includes(flag),
        ).length;
        return count !== 0 && count !== 3;
      })
      .map(({ path }) => path.join(' '));
    expect(partial).toEqual([]);
  });
});

describe('flag standardization — behavioral: old flag maps to canonical key and warns', () => {
  it('runtime topics create --format json → opts.output === "json", warns on stderr', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const cmd = find(buildProgram(), 'runtime', 'topics', 'create');
    cmd.parseOptions(['--name', 'n', '--description', 'd', '--examples', 'a', '--format', 'json']);
    const opts = resolveDeprecatedAliases(cmd, cmd.opts());
    expect(opts.output).toBe('json');
    const text = errSpy.mock.calls
      .flat()
      .map((c) => stripAnsi(String(c)))
      .join('\n');
    expect(text).toContain('--format');
    expect(text).toContain('deprecated');
    expect(text).toContain('--output');
  });

  it('runtime bulk-scan --input/--output map to file/outputFile, warns', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const cmd = find(buildProgram(), 'runtime', 'bulk-scan');
    cmd.parseOptions(['--profile', 'p', '--input', 'prompts.csv', '--output', 'out.csv']);
    const opts = resolveDeprecatedAliases(cmd, cmd.opts());
    expect(opts.file).toBe('prompts.csv');
    expect(opts.outputFile).toBe('out.csv');
    expect(errSpy).toHaveBeenCalled();
  });

  it('redteam eula accept --confirm maps to force, warns', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const cmd = find(buildProgram(), 'redteam', 'eula', 'accept');
    cmd.parseOptions(['--confirm']);
    const opts = resolveDeprecatedAliases(cmd, cmd.opts());
    expect(opts.force).toBe(true);
    expect(errSpy).toHaveBeenCalled();
  });

  it('redteam targets backup --format yaml maps to fileFormat, warns', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const cmd = find(buildProgram(), 'redteam', 'targets', 'backup');
    cmd.parseOptions(['--format', 'yaml']);
    const opts = resolveDeprecatedAliases(cmd, cmd.opts());
    expect(opts.fileFormat).toBe('yaml');
    expect(errSpy).toHaveBeenCalled();
  });

  it('canonical flag wins over deprecated alias when both are given', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const cmd = find(buildProgram(), 'runtime', 'topics', 'create');
    cmd.parseOptions(['--format', 'json', '--output', 'pretty']);
    const opts = resolveDeprecatedAliases(cmd, cmd.opts());
    expect(opts.output).toBe('pretty');
  });
});

describe('flag standardization — DLP pagination conversion', () => {
  it('legacy --page/--size pass through unchanged (zero-indexed API)', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const cmd = find(buildProgram(), 'runtime', 'dlp', 'patterns', 'list');
    cmd.parseOptions(['--page', '2', '--size', '10']);
    const { page, size } = resolvePageParams(cmd, cmd.opts());
    expect(page).toBe(2);
    expect(size).toBe(10);
    expect(errSpy).toHaveBeenCalled();
  });

  it('canonical --offset/--limit convert to a page boundary: page = floor(offset/limit)', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const cmd = find(buildProgram(), 'runtime', 'dlp', 'patterns', 'list');
    cmd.parseOptions(['--offset', '25', '--limit', '10']);
    const { page, size } = resolvePageParams(cmd, cmd.opts());
    expect(page).toBe(2); // rounds down to the page-2 boundary
    expect(size).toBe(10);
    expect(errSpy).not.toHaveBeenCalled();
  });

  it('no pagination flags → no page/size sent (API defaults)', () => {
    const cmd = find(buildProgram(), 'runtime', 'dlp', 'patterns', 'list');
    cmd.parseOptions([]);
    const { page, size } = resolvePageParams(cmd, cmd.opts());
    expect(page).toBeUndefined();
    expect(size).toBeUndefined();
  });

  it('scan-logs query is 1-indexed: defaults map to page 1, size 50', () => {
    const cmd = find(buildProgram(), 'runtime', 'scan-logs', 'query');
    cmd.parseOptions(['--interval', '1', '--unit', 'hours']);
    const { page, size } = resolvePageParams(cmd, cmd.opts(), { indexBase: 1 });
    expect(page ?? 1).toBe(1);
    expect(size).toBe(50);
  });

  it('scan-logs query --offset 100 --limit 50 → page 3 (1-indexed)', () => {
    const cmd = find(buildProgram(), 'runtime', 'scan-logs', 'query');
    cmd.parseOptions(['--interval', '1', '--unit', 'hours', '--offset', '100', '--limit', '50']);
    const { page, size } = resolvePageParams(cmd, cmd.opts(), { indexBase: 1 });
    expect(page).toBe(3);
    expect(size).toBe(50);
  });

  it('scan-logs query legacy --page 4 --page-size 20 passes through, warns', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const cmd = find(buildProgram(), 'runtime', 'scan-logs', 'query');
    cmd.parseOptions(['--interval', '1', '--unit', 'hours', '--page', '4', '--page-size', '20']);
    const { page, size } = resolvePageParams(cmd, cmd.opts(), { indexBase: 1 });
    expect(page).toBe(4);
    expect(size).toBe(20);
    expect(errSpy).toHaveBeenCalled();
  });
});
