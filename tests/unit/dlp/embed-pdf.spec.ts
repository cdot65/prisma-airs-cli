import { PDFDocument } from 'pdf-lib';
import { describe, expect, it } from 'vitest';
import { pdfMetaExtract, pdfStreamText, pdfTechniques } from '../../../src/dlp/embed/pdf.js';
import { pdf } from '../../../src/dlp/generate/pdf.js';
import { makePayload } from '../../../src/dlp/payload.js';
import { makeRng } from '../../../src/dlp/rng.js';

const payload = makePayload(makeRng(1));

describe('pdf embedders', () => {
  it('meta: recoverable via info dict; pdf valid', async () => {
    const clean = await pdf(makeRng(2));
    const out = Buffer.from(await pdfTechniques.meta.embed(clean, payload));
    const text = await pdfMetaExtract(out);
    for (const v of payload) {
      expect(text).toContain(v.value);
    }
    expect(out.subarray(0, 4).toString()).toBe('%PDF');
  });

  it('hidden-text: literal present in content; pdf reloads', async () => {
    const clean = await pdf(makeRng(2));
    const out = Buffer.from(await pdfTechniques['hidden-text'].embed(clean, payload));
    const text = pdfStreamText(out);
    for (const v of payload) {
      expect(text).toContain(v.value);
    }
    await expect(PDFDocument.load(out)).resolves.toBeDefined();
  });

  it('trailer: values after final %%EOF; pdf reloads', async () => {
    const clean = await pdf(makeRng(2));
    const out = Buffer.from(await pdfTechniques.trailer.embed(clean, payload));
    const parts = out.toString('latin1').split('%%EOF');
    const tail = parts[parts.length - 1];
    for (const v of payload) {
      expect(tail).toContain(v.value);
    }
    await expect(PDFDocument.load(out)).resolves.toBeDefined();
  });

  for (const id of ['visible', 'visible-samecolor']) {
    it(`${id}: text present in page content; pdf reloads`, async () => {
      const clean = await pdf(makeRng(2));
      const out = Buffer.from(await pdfTechniques[id].embed(clean, payload));
      const text = pdfStreamText(out);
      for (const v of payload) {
        expect(text).toContain(v.value);
      }
      expect(out.subarray(0, 4).toString()).toBe('%PDF');
      await expect(PDFDocument.load(out)).resolves.toBeDefined();
    });
  }
});
