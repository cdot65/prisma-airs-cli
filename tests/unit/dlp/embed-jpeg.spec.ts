import sharp from 'sharp';
import { describe, expect, it } from 'vitest';
import { jpegExifExtract, jpegTechniques } from '../../../src/dlp/embed/jpeg.js';
import { jpeg } from '../../../src/dlp/generate/jpeg.js';
import { makePayload } from '../../../src/dlp/payload.js';
import { makeRng } from '../../../src/dlp/rng.js';

const payload = makePayload(makeRng(1));

describe('jpeg embedders', () => {
  it('exif: recoverable via piexif; jpeg stays valid', async () => {
    const clean = await jpeg(makeRng(2));
    const out = Buffer.from(await jpegTechniques.exif.embed(clean, payload));
    const text = jpegExifExtract(out);
    for (const v of payload) {
      expect(text).toContain(v.value);
    }
    expect((await sharp(out).metadata()).format).toBe('jpeg');
  });

  it('com: values present in segment; jpeg stays valid', async () => {
    const clean = await jpeg(makeRng(2));
    const out = Buffer.from(await jpegTechniques.com.embed(clean, payload));
    const s = out.toString('latin1');
    for (const v of payload) {
      expect(s).toContain(v.value);
    }
    expect((await sharp(out).metadata()).format).toBe('jpeg');
  });

  it('trailer: values present after EOI', async () => {
    const clean = await jpeg(makeRng(2));
    const out = Buffer.from(await jpegTechniques.trailer.embed(clean, payload));
    const tail = out.subarray(Math.max(0, out.length - 300)).toString('latin1');
    for (const v of payload) {
      expect(tail).toContain(v.value);
    }
  });

  it('visible: renders an overlay; stays a valid jpeg of the same size', async () => {
    const clean = await jpeg(makeRng(2));
    const out = Buffer.from(await jpegTechniques.visible.embed(clean, payload));
    const cm = await sharp(clean).metadata();
    const om = await sharp(out).metadata();
    expect(om.format).toBe('jpeg');
    expect(om.width).toBe(cm.width);
    expect(om.height).toBe(cm.height);
    expect(out.equals(clean)).toBe(false);
  });
});
