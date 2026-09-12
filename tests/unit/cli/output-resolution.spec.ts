import { Command } from 'commander';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { formatOutput, resolveOutput } from '../../../src/cli/renderer/common.js';

const state = vi.hoisted(() => ({ configured: undefined as string | undefined, fail: false }));
vi.mock('../../../src/config/loader.js', () => ({
  loadConfig: async () => {
    if (state.fail) throw new Error('No tenant selected');
    return { defaultOutput: state.configured };
  },
}));

describe('resolveOutput', () => {
  beforeEach(() => {
    state.configured = undefined;
    state.fail = false;
    vi.stubEnv('PANW_CLI_OUTPUT', 'yaml');
  });
  afterEach(() => vi.unstubAllEnvs());

  it('uses command output before the global output', async () => {
    const root = new Command().option('--output <format>');
    const child = root.command('list').option('--output <format>');
    await root.parseAsync(['node', 'test', '--output', 'yaml', 'list', '--output', 'json']);
    await expect(resolveOutput(child, child.opts())).resolves.toBe('json');
  });

  it('uses the tenant default when no flag is explicit and never the environment', async () => {
    state.configured = 'markdown';
    const command = new Command('list');
    await expect(resolveOutput(command, command.opts())).resolves.toBe('markdown');
    state.configured = undefined;
    await expect(resolveOutput(new Command('list'), {})).resolves.toBe('pretty');
  });

  it('does not let a legacy command default mask the configured output', async () => {
    state.configured = 'csv';
    const root = new Command().option('--output <format>');
    const child = root.command('list').option('--output <format>', 'format', 'pretty');
    await root.parseAsync(['node', 'test', 'list']);
    await expect(resolveOutput(child, child.opts())).resolves.toBe('csv');
  });

  it('ignores the tenant config when asked, so tenant-less commands still run', async () => {
    state.fail = true;
    await expect(resolveOutput(new Command('list'), {})).rejects.toThrow('No tenant selected');
    await expect(resolveOutput(new Command('list'), {}, { ignoreConfig: true })).resolves.toBe(
      'pretty',
    );
  });

  it('rejects invalid and restricted formats as usage errors', async () => {
    state.configured = 'xml';
    await expect(resolveOutput(new Command('list'), {})).rejects.toThrow('Invalid output format');
    state.configured = 'csv';
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
