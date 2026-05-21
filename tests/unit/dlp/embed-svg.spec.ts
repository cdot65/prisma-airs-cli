import { describe, expect, it } from 'vitest';
import { svgTechniques } from '../../../src/dlp/embed/svg.js';
import { svg } from '../../../src/dlp/generate/svg.js';
import { makePayload } from '../../../src/dlp/payload.js';
import { makeRng } from '../../../src/dlp/rng.js';

const payload = makePayload(makeRng(1));
const clean = svg(makeRng(2));

describe('svg embedders', () => {
  for (const [id, technique] of Object.entries(svgTechniques)) {
    it(`${id}: embeds every value and stays a valid svg`, async () => {
      const out = Buffer.from(await technique.embed(clean, payload));
      const text = out.toString('utf8');
      for (const v of payload) {
        expect(text).toContain(v.value);
      }
      expect(text.startsWith('<svg')).toBe(true);
      expect(text).toContain('</svg>');
    });
  }
});
