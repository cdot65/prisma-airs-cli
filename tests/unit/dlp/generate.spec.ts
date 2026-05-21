import { describe, expect, it } from 'vitest';
import { generators } from '../../../src/dlp/generate/index.js';
import { makeRng } from '../../../src/dlp/rng.js';

describe('clean generators', () => {
  it('pdf starts with %PDF', async () => {
    const b = await generators.pdf(makeRng(1));
    expect(b.subarray(0, 4).toString()).toBe('%PDF');
  });

  it('png has PNG magic', async () => {
    const b = await generators.png(makeRng(1));
    expect(b.subarray(1, 4).toString()).toBe('PNG');
  });

  it('jpeg has SOI marker', async () => {
    const b = await generators.jpeg(makeRng(1));
    expect(b.subarray(0, 2).toString('hex')).toBe('ffd8');
  });

  it('svg has a namespaced root', async () => {
    const b = await generators.svg(makeRng(1));
    expect(b.toString()).toContain('<svg xmlns="http://www.w3.org/2000/svg"');
  });

  it('docx is a zip (PK)', async () => {
    const b = await generators.docx(makeRng(1));
    expect(b.subarray(0, 2).toString()).toBe('PK');
  });
});
