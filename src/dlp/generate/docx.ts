import { Document, HeadingLevel, Packer, Paragraph, TextRun } from 'docx';
import { lorem } from '../lorem.js';

/** Generate a valid DOCX (zip) filled with lorem ipsum. */
export async function docx(rng: () => number): Promise<Buffer> {
  const paragraphs = lorem(rng, 3)
    .split('\n\n')
    .map((p) => new Paragraph({ children: [new TextRun(p)] }));
  const doc = new Document({
    sections: [
      {
        children: [
          new Paragraph({ text: 'Generated Document', heading: HeadingLevel.HEADING_1 }),
          ...paragraphs,
        ],
      },
    ],
  });
  return Packer.toBuffer(doc);
}
