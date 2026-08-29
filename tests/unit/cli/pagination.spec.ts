import { Command } from 'commander';
import { describe, expect, it } from 'vitest';
import { registerListFlags, resolveListParams } from '../../../src/cli/pagination.js';

describe('canonical list pagination', () => {
  it('registers uniform flags with a dialect-specific default', () => {
    const command = registerListFlags(new Command('list'), { dialect: 'offset' });
    command.parseOptions([]);
    expect(command.opts()).toMatchObject({ limit: 100, offset: 0 });
    expect(command.options.map((option) => option.long)).toEqual(
      expect.arrayContaining(['--limit', '--offset', '--all', '--max']),
    );
  });

  it('uses 50 for skip and page dialects', () => {
    for (const dialect of ['skip', 'page'] as const) {
      const command = registerListFlags(new Command('list'), { dialect });
      command.parseOptions([]);
      expect(resolveListParams(command, command.opts(), { dialect }).limit).toBe(50);
    }
  });

  it('rejects --all with an explicit --offset', () => {
    const command = registerListFlags(new Command('list'), { dialect: 'skip' });
    command.parseOptions(['--all', '--offset', '10']);
    expect(() => resolveListParams(command, command.opts(), { dialect: 'skip' })).toThrow(
      '--all cannot be combined with --offset',
    );
  });

  it('validates positive limit and non-negative offset/max', () => {
    const command = registerListFlags(new Command('list'), { dialect: 'offset' });
    command.parseOptions(['--limit', '0']);
    expect(() => resolveListParams(command, command.opts(), { dialect: 'offset' })).toThrow(
      '--limit must be a positive integer',
    );
  });
});
