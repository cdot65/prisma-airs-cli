import { describe, expect, it } from 'vitest';
import { lorem } from '../../../src/dlp/lorem.js';
import { makeRng } from '../../../src/dlp/rng.js';

describe('lorem', () => {
  it('returns the requested number of paragraphs', () => {
    expect(lorem(makeRng(1), 3).split('\n\n')).toHaveLength(3);
  });

  it('is deterministic by seed', () => {
    expect(lorem(makeRng(1), 3)).toEqual(lorem(makeRng(1), 3));
  });

  it('produces non-empty capitalized sentences', () => {
    const text = lorem(makeRng(2), 1);
    expect(text.length).toBeGreaterThan(0);
    expect(text).toMatch(/^[A-Z]/);
    expect(text.trim().endsWith('.')).toBe(true);
  });
});
