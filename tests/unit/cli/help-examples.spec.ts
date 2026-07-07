import type { Command } from 'commander';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { buildProgram } from '../../../src/cli/program.js';

function find(parent: Command, ...names: string[]): Command {
  let cmd = parent;
  for (const name of names) {
    const next = cmd.commands.find((c) => c.name() === name);
    if (!next) throw new Error(`command not found: ${names.join(' ')} (missing "${name}")`);
    cmd = next;
  }
  return cmd;
}

/** Full help text including addHelpText('after') blocks. */
function helpOf(cmd: Command): string {
  let out = '';
  const spy = vi.spyOn(process.stdout, 'write').mockImplementation(((chunk: unknown) => {
    out += String(chunk);
    return true;
  }) as never);
  cmd.outputHelp();
  spy.mockRestore();
  return out;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('help examples', () => {
  const program = buildProgram();

  const withExamples: string[][] = [
    ['runtime', 'scan'],
    ['runtime', 'bulk-scan'],
    ['runtime', 'profiles', 'list'],
    ['runtime', 'topics', 'create'],
    ['runtime', 'topics', 'eval'],
    ['redteam', 'scan'],
    ['redteam', 'targets', 'list'],
    ['model-security', 'scans', 'list'],
    ['config'],
    ['doctor'],
  ];

  for (const path of withExamples) {
    it(`${path.join(' ')} --help shows an Examples section`, () => {
      const help = helpOf(find(program, ...path));
      expect(help).toContain('Examples:');
      expect(help).toContain('$ airs');
    });
  }

  it('examples use canonical flags (bulk-scan shows --file, not --input)', () => {
    const help = helpOf(find(program, 'runtime', 'bulk-scan'));
    expect(help).toContain('--file');
    const examplesBlock = help.slice(help.indexOf('Examples:'));
    expect(examplesBlock).not.toContain('--input');
  });

  it('completion command help includes install guidance', () => {
    const help = helpOf(find(program, 'completion'));
    expect(help).toContain('Examples:');
  });
});
