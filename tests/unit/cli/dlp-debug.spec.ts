import { mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, expect, it, vi } from 'vitest';
import { buildProgram } from '../../../src/cli/program.js';

const originalFetch = globalThis.fetch;
let directory: string;

afterEach(() => {
  globalThis.fetch = originalFetch;
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
  if (directory) rmSync(directory, { recursive: true, force: true });
});

it.each([
  ['before', 200],
  ['after', 200],
  ['before', 400],
  ['after', 400],
] as const)('DLP program --debug %s retains HTTP metadata but never bodies for status %s', async (position, status) => {
  directory = mkdtempSync(join(tmpdir(), 'airs-dlp-debug-'));
  vi.spyOn(process, 'cwd').mockReturnValue(directory);
  vi.spyOn(console, 'error').mockImplementation(() => {});
  vi.stubEnv('PANW_AI_SEC_DEBUG_BODY', '1');
  const response = Response.json(
    {
      title: 'Bad Request',
      detail: 'PRIVATE-ERROR-PAYLOAD',
      keywords: ['PRIVATE-KEYWORD'],
    },
    { status },
  );
  const clone = vi.spyOn(response, 'clone');
  globalThis.fetch = vi.fn().mockResolvedValue(response);
  const program = buildProgram();
  const runtime = program.commands.find((command) => command.name() === 'runtime');
  const dlp = runtime?.commands.find((command) => command.name() === 'dlp');
  const dictionaries = dlp?.commands.find((command) => command.name() === 'dictionaries');
  const get = dictionaries?.commands.find((command) => command.name() === 'get');
  if (!get) throw new Error('Missing dictionary command');
  get.action(async () => {
    expect(process.env.PANW_AI_SEC_DEBUG_BODY).toBe('0');
    await fetch('https://api.dlp.paloaltonetworks.com/v2/api/dictionaries/test?keywords=true', {
      method: 'POST',
      headers: { authorization: 'Bearer PRIVATE-TOKEN' },
      body: JSON.stringify({ name: 'PRIVATE-METADATA', keywords: ['PRIVATE-KEYWORD'] }),
    });
  });
  await program.parseAsync(
    [
      ...(position === 'before' ? ['--debug'] : []),
      'runtime',
      'dlp',
      'dictionaries',
      'get',
      'test',
      ...(position === 'after' ? ['--debug'] : []),
    ],
    { from: 'user' },
  );
  expect(process.env.PANW_AI_SEC_DEBUG_BODY).toBe('1');
  const logs = readdirSync(directory).filter((name) => name.startsWith('debug-api-'));
  expect(logs).toHaveLength(1);
  const log = readFileSync(join(directory, logs[0]), 'utf8');
  expect(log).not.toContain('PRIVATE-');
  const entry = JSON.parse(log);
  expect(entry.request.method).toBe('POST');
  expect(entry.request.url).toContain('/v2/api/dictionaries/test?keywords=true');
  expect(entry.request.body).toBe('[BODY OMITTED]');
  expect(entry.response.body).toBe('[BODY OMITTED]');
  expect(entry.response.status).toBe(status);
  expect(clone).not.toHaveBeenCalled();
});

it('restores SDK body logging when an embedded DLP action throws', async () => {
  vi.stubEnv('PANW_AI_SEC_DEBUG_BODY', '1');
  const program = buildProgram();
  const runtime = program.commands.find((command) => command.name() === 'runtime');
  const dlp = runtime?.commands.find((command) => command.name() === 'dlp');
  if (!dlp) throw new Error('Missing DLP command');
  dlp.command('debug-test-failure').action(() => {
    expect(process.env.PANW_AI_SEC_DEBUG_BODY).toBe('0');
    throw new Error('controlled action failure');
  });
  await expect(
    program.parseAsync(['runtime', 'dlp', 'debug-test-failure'], { from: 'user' }),
  ).rejects.toThrow('controlled action failure');
  expect(process.env.PANW_AI_SEC_DEBUG_BODY).toBe('1');
});
