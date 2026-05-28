import type { Command } from 'commander';
import { describe, expect, it } from 'vitest';
import { buildProgram } from '../../../src/cli/program.js';

function find(parent: Command, name: string): Command | undefined {
  return parent.commands.find((c) => c.name() === name);
}

describe('runtime command tree', () => {
  const program = buildProgram();
  const runtime = find(program, 'runtime');

  it('exposes a runtime group', () => {
    expect(runtime).toBeDefined();
  });

  it('no longer exposes the legacy `dlp-profiles` subcommand', () => {
    expect(find(runtime!, 'dlp-profiles')).toBeUndefined();
  });

  it('keeps the new `dlp profiles` namespace command as the canonical listing', () => {
    const dlp = find(runtime!, 'dlp');
    expect(dlp).toBeDefined();
    expect(find(dlp!, 'profiles')).toBeDefined();
  });

  it('no longer exposes the top-level `dlp-gen` command', () => {
    expect(find(runtime!, 'dlp-gen')).toBeUndefined();
  });

  it('exposes the relocated `dlp generate` command with original flags preserved', () => {
    const dlp = find(runtime!, 'dlp');
    const generate = find(dlp!, 'generate');
    expect(generate).toBeDefined();
    const flags = generate!.options.map((o) => o.long);
    for (const f of ['--types', '--count', '--out', '--techniques', '--seed', '--output']) {
      expect(flags).toContain(f);
    }
  });
});
