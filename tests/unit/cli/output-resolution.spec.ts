import { Command } from 'commander';
import { afterEach, describe, expect, it } from 'vitest';
import { formatOutput, resolveOutput } from '../../../src/cli/renderer/common.js';

describe('resolveOutput', () => {
  const original = process.env.PANW_CLI_OUTPUT;
  afterEach(() => {
    if (original === undefined) delete process.env.PANW_CLI_OUTPUT;
    else process.env.PANW_CLI_OUTPUT = original;
  });

  it('uses command output before the global output', async () => {
    const root = new Command().option('--output <format>');
    const child = root.command('list').option('--output <format>');
    await root.parseAsync(['node', 'test', '--output', 'yaml', 'list', '--output', 'json']);
    await expect(resolveOutput(child, child.opts())).resolves.toBe('json');
  });

  it('uses PANW_CLI_OUTPUT when no flag is explicit', async () => {
    process.env.PANW_CLI_OUTPUT = 'markdown';
    const command = new Command('list');
    await expect(resolveOutput(command, command.opts())).resolves.toBe('markdown');
  });

  it('does not let a legacy command default mask the configured output', async () => {
    process.env.PANW_CLI_OUTPUT = 'yaml';
    const root = new Command().option('--output <format>');
    const child = root.command('list').option('--output <format>', 'format', 'pretty');
    await root.parseAsync(['node', 'test', 'list']);
    await expect(resolveOutput(child, child.opts())).resolves.toBe('yaml');
  });

  it('rejects invalid and restricted formats as usage errors', async () => {
    process.env.PANW_CLI_OUTPUT = 'xml';
    await expect(resolveOutput(new Command('list'), {})).rejects.toThrow('Invalid output format');
    process.env.PANW_CLI_OUTPUT = 'csv';
    await expect(
      resolveOutput(new Command('list'), {}, { allowed: ['pretty', 'json'] }),
    ).rejects.toThrow('not supported');
  });
});

describe('empty structured output', () => {
  it('emits a bare empty array for JSON and YAML', () => {
    expect(formatOutput([], [], 'json')).toBe('[]');
    expect(formatOutput([], [], 'yaml')).toBe('[]\n');
  });
});
