import { describe, expect, it } from 'vitest';
import { digits, makeRng, pick, subSeed } from '../../../src/dlp/rng.js';

describe('makeRng', () => {
  it('is deterministic for a given seed', () => {
    const a = makeRng(42);
    const b = makeRng(42);
    expect([a(), a(), a()]).toEqual([b(), b(), b()]);
  });

  it('differs across seeds', () => {
    expect(makeRng(1)()).not.toEqual(makeRng(2)());
  });

  it('returns floats in [0, 1)', () => {
    const r = makeRng(7);
    for (let i = 0; i < 200; i++) {
      const v = r();
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThan(1);
    }
  });
});

describe('pick', () => {
  it('selects an element from the array', () => {
    const r = makeRng(3);
    const xs = ['a', 'b', 'c'];
    for (let i = 0; i < 50; i++) {
      expect(xs).toContain(pick(r, xs));
    }
  });
});

describe('digits', () => {
  it('returns exactly n decimal digits', () => {
    const r = makeRng(9);
    const d = digits(r, 12);
    expect(d).toHaveLength(12);
    expect(d).toMatch(/^[0-9]{12}$/);
  });
});

describe('subSeed', () => {
  it('is deterministic and index-sensitive', () => {
    expect(subSeed(5, 0)).toEqual(subSeed(5, 0));
    expect(subSeed(5, 0)).not.toEqual(subSeed(5, 1));
  });
});
