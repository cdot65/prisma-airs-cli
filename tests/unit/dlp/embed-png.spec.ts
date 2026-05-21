import sharp from 'sharp';
import { describe, expect, it } from 'vitest';
import { lsbExtract, pngTechniques } from '../../../src/dlp/embed/png.js';
import { readTextChunks } from '../../../src/dlp/embed/png-chunks.js';
import { png } from '../../../src/dlp/generate/png.js';
import { makePayload } from '../../../src/dlp/payload.js';
import { makeRng } from '../../../src/dlp/rng.js';

const payload = makePayload(makeRng(1));

describe('png embedders', () => {
  it('text-chunks: values recoverable from chunks; png stays valid', async () => {
    const clean = await png(makeRng(2));
    const out = Buffer.from(await pngTechniques['text-chunks'].embed(clean, payload));
    const text = readTextChunks(out);
    for (const v of payload) {
      expect(text).toContain(v.value);
    }
    expect((await sharp(out).metadata()).format).toBe('png');
  });

  it('trailer: values present after IEND', async () => {
    const clean = await png(makeRng(2));
    const out = Buffer.from(await pngTechniques.trailer.embed(clean, payload));
    const tail = out.subarray(out.indexOf(Buffer.from('IEND'))).toString('latin1');
    for (const v of payload) {
      expect(tail).toContain(v.value);
    }
  });

  it('stego-lsb: payload decodes back; png stays valid', async () => {
    const clean = await png(makeRng(2));
    const out = Buffer.from(await pngTechniques['stego-lsb'].embed(clean, payload));
    const recovered = await lsbExtract(out);
    for (const v of payload) {
      expect(recovered).toContain(v.value);
    }
    expect((await sharp(out).metadata()).format).toBe('png');
  });
});
