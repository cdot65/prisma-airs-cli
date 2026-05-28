import { inflateSync } from 'node:zlib';
import { PDFDocument, rgb } from 'pdf-lib';
import type { PayloadValue, Technique } from '../types.js';

const join = (p: PayloadValue[]): string => p.map((v) => `${v.category}: ${v.value}`).join(' | ');

async function setMeta(clean: Buffer, p: PayloadValue[]): Promise<Buffer> {
  const doc = await PDFDocument.load(clean);
  doc.setTitle(join(p));
  doc.setSubject(join(p));
  doc.setKeywords(p.map((v) => v.value));
  doc.setAuthor(p.find((v) => v.category === 'email')?.value ?? 'synthetic');
  return Buffer.from(await doc.save());
}

// "Hidden" text layer: drawn in white so it does not show on a white page, but the
// literal string remains in the (uncompressed) page content stream. pdf-lib does not
// expose text render-mode 3, so white-on-white is the chosen, reliably-verifiable approach.
async function hiddenText(clean: Buffer, p: PayloadValue[]): Promise<Buffer> {
  const doc = await PDFDocument.load(clean);
  const page = doc.getPage(0);
  let y = 760;
  for (const v of p) {
    page.drawText(`${v.category}: ${v.value}`, { x: 40, y, size: 6, color: rgb(1, 1, 1) });
    y -= 8;
  }
  return Buffer.from(await doc.save());
}

const trailer = (clean: Buffer, p: PayloadValue[]): Buffer =>
  Buffer.concat([clean, Buffer.from(`\n% DLP ${join(p)}\n`, 'utf8')]);

// Visible text painted onto the page over a filled background band.
//   sameColor=false -> dark text on a light band (genuinely visible)
//   sameColor=true  -> text drawn in the band's own color (camouflaged, same fg/bg)
async function visibleText(clean: Buffer, p: PayloadValue[], sameColor: boolean): Promise<Buffer> {
  const doc = await PDFDocument.load(clean);
  const page = doc.getPage(0);
  const bg = rgb(0.93, 0.93, 0.85);
  page.drawRectangle({ x: 36, y: 592, width: 524, height: 136, color: bg });
  let y = 712;
  for (const v of p) {
    page.drawText(`${v.category}: ${v.value}`, {
      x: 44,
      y,
      size: 9,
      color: sameColor ? bg : rgb(0, 0, 0),
    });
    y -= 14;
  }
  return Buffer.from(await doc.save());
}

/** Recover text from all PDF streams (inflating FlateDecode streams; falling back to raw). */
export function pdfStreamText(pdf: Buffer): string {
  const parts: string[] = [];
  let idx = 0;
  for (;;) {
    const s = pdf.indexOf('stream', idx);
    if (s < 0) {
      break;
    }
    let dataStart = s + 6;
    if (pdf[dataStart] === 0x0d) {
      dataStart++;
    }
    if (pdf[dataStart] === 0x0a) {
      dataStart++;
    }
    const e = pdf.indexOf('endstream', dataStart);
    if (e < 0) {
      break;
    }
    const raw = pdf.subarray(dataStart, e);
    try {
      parts.push(inflateSync(raw).toString('latin1'));
    } catch {
      parts.push(raw.toString('latin1'));
    }
    idx = e + 9;
  }
  const combined = parts.join('\n');
  // pdf-lib renders drawn text as PDF hex strings (<...>); decode them too.
  const decoded: string[] = [combined];
  for (const m of combined.matchAll(/<([0-9A-Fa-f]+)>/g)) {
    if (m[1].length % 2 === 0) {
      decoded.push(Buffer.from(m[1], 'hex').toString('latin1'));
    }
  }
  return decoded.join('\n');
}

/** Recover document-info-dict metadata from a PDF. */
export async function pdfMetaExtract(pdf: Buffer): Promise<string> {
  const doc = await PDFDocument.load(pdf);
  return [doc.getTitle(), doc.getSubject(), doc.getAuthor(), doc.getKeywords()]
    .filter((v): v is string => typeof v === 'string')
    .join('\n');
}

export const pdfTechniques: Record<string, Technique> = {
  meta: {
    id: 'meta',
    format: 'pdf',
    label: 'document info dict',
    embed: (c, p) => setMeta(c, p),
  },
  'hidden-text': {
    id: 'hidden-text',
    format: 'pdf',
    label: 'white (hidden) text layer',
    embed: (c, p) => hiddenText(c, p),
  },
  trailer: {
    id: 'trailer',
    format: 'pdf',
    label: 'bytes after %%EOF',
    embed: (c, p) => trailer(c, p),
  },
  visible: {
    id: 'visible',
    format: 'pdf',
    label: 'visible page text (dark on light band)',
    embed: (c, p) => visibleText(c, p, false),
  },
  'visible-samecolor': {
    id: 'visible-samecolor',
    format: 'pdf',
    label: 'visible page text, same color as background (camouflaged)',
    embed: (c, p) => visibleText(c, p, true),
  },
};
