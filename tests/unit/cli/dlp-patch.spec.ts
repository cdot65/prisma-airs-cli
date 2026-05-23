import { describe, expect, it } from 'vitest';
import { buildMergePatch } from '../../../src/cli/commands/dlp/patch.js';

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
