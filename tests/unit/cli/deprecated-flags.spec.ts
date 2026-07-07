import { Command } from 'commander';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  registerDeprecatedAlias,
  resolveDeprecatedAliases,
} from '../../../src/cli/deprecated-flags.js';

const stripAnsi = (s: string) => s.replace(/\[[0-9;]*m/g, '');

afterEach(() => {
  vi.restoreAllMocks();
});

function makeCmd() {
  const cmd = new Command('demo').option('--output <format>', 'Output format', 'pretty');
  registerDeprecatedAlias(cmd, {
    oldFlag: '--format <format>',
    oldKey: 'format',
    canonicalFlag: '--output',
    canonicalKey: 'output',
  });
  return cmd;
}

describe('deprecated flag aliases', () => {
  it('hides the deprecated flag from help', () => {
    const cmd = makeCmd();
    expect(cmd.helpInformation()).not.toContain('--format');
    expect(cmd.helpInformation()).toContain('--output');
  });

  it('maps the old flag onto the canonical key and warns on stderr', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const cmd = makeCmd();
    cmd.parse(['--format', 'json'], { from: 'user' });
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

  it('canonical flag wins when both are supplied, still warns', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const cmd = makeCmd();
    cmd.parse(['--format', 'yaml', '--output', 'json'], { from: 'user' });
    const opts = resolveDeprecatedAliases(cmd, cmd.opts());
    expect(opts.output).toBe('json');
    expect(errSpy).toHaveBeenCalled();
  });

  it('no warning when the deprecated flag is unused', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const cmd = makeCmd();
    cmd.parse(['--output', 'json'], { from: 'user' });
    const opts = resolveDeprecatedAliases(cmd, cmd.opts());
    expect(opts.output).toBe('json');
    expect(errSpy).not.toHaveBeenCalled();
  });
});
