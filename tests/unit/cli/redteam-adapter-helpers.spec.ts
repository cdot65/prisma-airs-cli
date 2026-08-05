import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { parseAdapterVariables, resolveScriptB64 } from '../../../src/cli/commands/redteam.js';

describe('resolveScriptB64', () => {
  let tempDir: string;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'adapter-script-'));
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it('passes through --script-b64 unchanged', () => {
    expect(resolveScriptB64({ scriptB64: 'c2NyaXB0' })).toBe('c2NyaXB0');
  });

  it('reads and base64-encodes --script-file', async () => {
    const p = join(tempDir, 'adapter.py');
    await writeFile(p, 'print("hi")');
    expect(resolveScriptB64({ scriptFile: p })).toBe(Buffer.from('print("hi")').toString('base64'));
  });

  it('rejects both flags together', () => {
    expect(() => resolveScriptB64({ scriptFile: 'x', scriptB64: 'y' })).toThrow(
      /mutually exclusive/,
    );
  });

  it('rejects neither flag', () => {
    expect(() => resolveScriptB64({})).toThrow(/required/);
  });
});

describe('parseAdapterVariables', () => {
  it('accepts a valid array', () => {
    expect(parseAdapterVariables('[{"key":"endpoint","value":"http://x","type":"VAR"}]')).toEqual([
      { key: 'endpoint', value: 'http://x', type: 'VAR' },
    ]);
  });

  it('accepts null values (keep stored secret)', () => {
    expect(parseAdapterVariables('[{"key":"s","value":null,"type":"SECRET"}]')).toEqual([
      { key: 's', value: null, type: 'SECRET' },
    ]);
  });

  it('rejects invalid JSON', () => {
    expect(() => parseAdapterVariables('nope')).toThrow(/invalid JSON/);
  });

  it('rejects entries with a bad type', () => {
    expect(() => parseAdapterVariables('[{"key":"k","type":"OTHER"}]')).toThrow(/VAR/);
  });

  it('rejects non-array input', () => {
    expect(() => parseAdapterVariables('{"key":"k"}')).toThrow(/array/);
  });
});
