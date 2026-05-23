import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Readable } from 'node:stream';
import { describe, expect, it } from 'vitest';
import { buildMergePatch, parseBody } from '../../../src/cli/commands/dlp/patch.js';

describe('buildMergePatch', () => {
  it('sets string scalars', () => {
    expect(buildMergePatch({ set: ['name=foo'] })).toEqual({ name: 'foo' });
  });
  it('coerces numbers', () => {
    expect(buildMergePatch({ set: ['count=5'] })).toEqual({ count: 5 });
  });
  it('coerces booleans', () => {
    expect(buildMergePatch({ set: ['enabled=true'] })).toEqual({ enabled: true });
  });
  it('parses JSON arrays', () => {
    expect(buildMergePatch({ set: ['tags=["a","b"]'] })).toEqual({ tags: ['a', 'b'] });
  });
  it('parses JSON objects', () => {
    expect(buildMergePatch({ set: ['config={"a":1,"b":true}'], clear: [] })).toEqual({
      config: { a: 1, b: true },
    });
  });
  it('allows literal string "null" via quoted JSON', () => {
    expect(buildMergePatch({ set: ['name="null"'] })).toEqual({ name: 'null' });
  });
  it('clears fields via --clear', () => {
    expect(buildMergePatch({ clear: ['description'] })).toEqual({ description: null });
  });
  it('combines set and clear', () => {
    expect(buildMergePatch({ set: ['a=1'], clear: ['b'] })).toEqual({ a: 1, b: null });
  });
  it('rejects --set key=null literal', () => {
    expect(() => buildMergePatch({ set: ['name=null'] })).toThrow(/to clear a field, use --clear/);
  });
  it('rejects dotted keys', () => {
    expect(() => buildMergePatch({ set: ['nested.field=x'] })).toThrow(
      'use --body-file for nested fields',
    );
  });
  it('rejects malformed --set entries', () => {
    expect(() => buildMergePatch({ set: ['malformed'] })).toThrow('expected key=value');
  });
  it('returns empty object when no inputs', () => {
    expect(buildMergePatch({})).toEqual({});
  });
});

describe('parseBody', () => {
  it('reads JSON from a file path', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'dlp-'));
    const file = join(dir, 'body.json');
    await writeFile(file, '{"name":"foo"}');
    expect(await parseBody({ bodyFile: file })).toEqual({ name: 'foo' });
    await rm(dir, { recursive: true });
  });

  it('reads JSON from stdin when body === "-"', async () => {
    const stdin = Readable.from(['{"x":1}']);
    expect(await parseBody({ body: '-', stdin })).toEqual({ x: 1 });
  });

  it('throws on malformed JSON', async () => {
    await expect(parseBody({ body: '-', stdin: Readable.from(['not json']) })).rejects.toThrow(
      /invalid JSON/i,
    );
  });

  it('returns undefined when neither flag set', async () => {
    expect(await parseBody({})).toBeUndefined();
  });
});
