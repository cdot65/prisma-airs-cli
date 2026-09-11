import { Command } from 'commander';
import { predefinedFlag, visibleRecords } from '../../../src/cli/commands/dlp/visibility.js';

describe('dlp listing visibility', () => {
  it('hides predefined records by default and shows them on request', () => {
    const rows = [{ type: 'custom' }, { type: 'predefined' }, { type: null }, {}];
    expect(visibleRecords(rows, undefined)).toHaveLength(3);
    expect(visibleRecords(rows, undefined).every((row) => row.type !== 'predefined')).toBe(true);
    expect(visibleRecords(rows, true)).toHaveLength(4);
  });

  it('registers the --include-predefined flag', () => {
    const cmd = predefinedFlag(new Command('list'));
    expect(cmd.options.some((option) => option.long === '--include-predefined')).toBe(true);
  });
});
