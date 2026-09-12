import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { buildProgram } from '../../../src/cli/program.js';
import { setQuiet } from '../../../src/cli/renderer/ui.js';

const state = vi.hoisted(() => ({
  configuredOutput: 'pretty',
  generate: vi.fn(),
}));
vi.mock('../../../src/config/loader.js', () => ({
  loadConfig: async () => ({
    defaultOutput: state.configuredOutput,
  }),
}));
vi.mock('../../../src/dlp/index.js', () => ({ generateCorpus: state.generate }));

const summary = {
  out: '/synthetic-corpus',
  manifestPath: '/synthetic-corpus/manifest.json',
  seed: 431,
  clean: 1,
  dirty: 4,
  byFormat: { png: { clean: 1, dirty: 4 } },
};

async function run(args: string[]) {
  await buildProgram().parseAsync(['node', 'airs', ...args]);
}

describe('DLP generate public command output and preflight', () => {
  beforeEach(() => {
    state.configuredOutput = 'pretty';
    state.generate.mockReset().mockResolvedValue(summary);
    vi.spyOn(console, 'log').mockImplementation(() => {});
    vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.spyOn(process, 'exit').mockImplementation((code) => {
      throw new Error(`exit:${code}`);
    });
  });
  afterEach(() => {
    setQuiet(false);
    vi.restoreAllMocks();
    vi.unstubAllEnvs();
  });

  it.each([
    ['--output', 'json', 'runtime', 'dlp', 'generate'],
    ['runtime', '--output', 'json', 'dlp', 'generate'],
    ['runtime', 'dlp', 'generate', '--output', 'json'],
    ['--quiet', 'runtime', 'dlp', 'generate', '--output', 'json'],
  ])('honors explicit JSON regardless of flag placement: %j', async (...args) => {
    await run(args);
    expect(console.log).toHaveBeenCalledExactlyOnceWith(JSON.stringify(summary, null, 2));
    expect(console.error).not.toHaveBeenCalled();
  });

  it('honors configured JSON without an explicit flag', async () => {
    state.configuredOutput = 'json';
    await run(['runtime', 'dlp', 'generate']);
    expect(console.log).toHaveBeenCalledExactlyOnceWith(JSON.stringify(summary, null, 2));
  });

  it('ignores the retired output environment override', async () => {
    vi.stubEnv('PANW_CLI_OUTPUT', 'json');
    await run(['runtime', 'dlp', 'generate']);
    expect(console.log).not.toHaveBeenCalledWith(JSON.stringify(summary, null, 2));
  });

  it('explicit pretty output wins over configured JSON', async () => {
    state.configuredOutput = 'json';
    await run(['runtime', 'dlp', 'generate', '--output', 'pretty']);
    expect(vi.mocked(console.log).mock.calls.flat().join('\n')).toContain(
      'DLP Test-File Generation',
    );
  });

  it.each([
    'yaml',
    'csv',
    'table',
    'markdown',
    'xml',
  ])('rejects output %s before generation', async (format) => {
    await expect(run(['runtime', 'dlp', 'generate', '--output', format])).rejects.toThrow('exit:2');
    expect(state.generate).not.toHaveBeenCalled();
    expect(console.log).not.toHaveBeenCalled();
  });

  it.each([
    '0',
    '-1',
    '1.5',
    '1junk',
    '1e2',
    '0x10',
    'NaN',
    '9007199254740992',
  ])('rejects count %s before generation', async (value) => {
    await expect(run(['runtime', 'dlp', 'generate', '--count', value])).rejects.toThrow('exit:2');
    expect(state.generate).not.toHaveBeenCalled();
    expect(console.log).not.toHaveBeenCalled();
  });

  it.each([
    '1.5',
    '7junk',
    '1e2',
    'NaN',
    'Infinity',
    '9007199254740992',
  ])('rejects seed %s before generation', async (value) => {
    await expect(run(['runtime', 'dlp', 'generate', '--seed', value])).rejects.toThrow('exit:2');
    expect(state.generate).not.toHaveBeenCalled();
    expect(console.log).not.toHaveBeenCalled();
  });

  it.each(['0', '-1', '431'])('preserves supported integer seed %s', async (value) => {
    await run(['runtime', 'dlp', 'generate', '--types', 'svg', '--count', '2', '--seed', value]);
    expect(state.generate).toHaveBeenCalledExactlyOnceWith({
      types: ['svg'],
      count: 2,
      out: './temp',
      techniques: 'all',
      seed: Number(value),
    });
  });

  it('preserves the operational failure exit code and stderr-only error', async () => {
    state.generate.mockRejectedValueOnce(new Error('synthetic write failure'));
    await expect(run(['runtime', 'dlp', 'generate', '--output', 'json'])).rejects.toThrow('exit:1');
    expect(console.log).not.toHaveBeenCalled();
    expect(vi.mocked(console.error).mock.calls.flat().join('\n')).toContain(
      'synthetic write failure',
    );
  });
});
