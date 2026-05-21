import { PDFDocument, StandardFonts } from 'pdf-lib';
import { lorem } from '../lorem.js';

/** Generate a valid single-page PDF filled with lorem ipsum. */
export async function pdf(rng: () => number): Promise<Buffer> {
  const doc = await PDFDocument.create();
  const font = await doc.embedFont(StandardFonts.Helvetica);
  const page = doc.addPage([612, 792]);
  page.drawText(lorem(rng, 3), {
    x: 54,
    y: 720,
    size: 11,
    font,
    lineHeight: 15,
    maxWidth: 504,
  });
  const bytes = await doc.save();
  return Buffer.from(bytes);
}
