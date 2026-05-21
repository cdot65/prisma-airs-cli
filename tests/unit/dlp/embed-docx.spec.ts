import { describe, expect, it } from 'vitest';
import { docxTechniques } from '../../../src/dlp/embed/docx.js';
import { unzipText } from '../../../src/dlp/embed/zip.js';
import { makePayload } from '../../../src/dlp/payload.js';
import { makeRng } from '../../../src/dlp/rng.js';

const payload = makePayload(makeRng(1));

describe('docx embedders', () => {
  for (const [id, technique] of Object.entries(docxTechniques)) {
    it(`${id}: values recoverable; output is a zip`, async () => {
      const out = Buffer.from(await technique.embed(Buffer.alloc(0), payload));
      expect(out.subarray(0, 2).toString()).toBe('PK');
      const text = unzipText(out);
      for (const v of payload) {
        expect(text).toContain(v.value);
      }
    });
  }
});
