import { Document, HeadingLevel, Packer, Paragraph, ShadingType, TextRun } from 'docx';
import { lorem } from '../lorem.js';
import { makeRng } from '../rng.js';
import type { PayloadValue, Technique } from '../types.js';

const join = (p: PayloadValue[]): string => p.map((v) => `${v.category}: ${v.value}`).join(' | ');

function bodyParagraphs(): Paragraph[] {
  return lorem(makeRng(1), 2)
    .split('\n\n')
    .map((t) => new Paragraph({ children: [new TextRun(t)] }));
}

interface Core {
  title?: string;
  subject?: string;
  keywords?: string;
}

async function build(extra: Paragraph[], core: Core = {}): Promise<Buffer> {
  const doc = new Document({
    title: core.title,
    subject: core.subject,
    keywords: core.keywords,
    sections: [
      {
        children: [
          new Paragraph({ text: 'Generated Document', heading: HeadingLevel.HEADING_1 }),
          ...bodyParagraphs(),
          ...extra,
        ],
      },
    ],
  });
  return Packer.toBuffer(doc);
}

export const docxTechniques: Record<string, Technique> = {
  'core-props': {
    id: 'core-props',
    format: 'docx',
    label: 'core document properties',
    embed: (_clean, p) => build([], { title: join(p), subject: join(p), keywords: join(p) }),
  },
  'hidden-run': {
    id: 'hidden-run',
    format: 'docx',
    label: 'hidden (vanish) run',
    embed: (_clean, p) =>
      build([new Paragraph({ children: [new TextRun({ text: join(p), vanish: true })] })]),
  },
  visible: {
    id: 'visible',
    format: 'docx',
    label: 'visible body text (foreground != background)',
    embed: (_clean, p) => build([new Paragraph({ children: [new TextRun(join(p))] })]),
  },
  'visible-samecolor': {
    id: 'visible-samecolor',
    format: 'docx',
    label: 'visible run, same color as background (camouflaged)',
    embed: (_clean, p) =>
      build([
        new Paragraph({
          children: [
            new TextRun({
              text: join(p),
              color: 'D9D9D9',
              shading: { type: ShadingType.CLEAR, color: 'auto', fill: 'D9D9D9' },
            }),
          ],
        }),
      ]),
  },
};
