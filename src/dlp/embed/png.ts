import sharp from 'sharp';
import type { PayloadValue, Technique } from '../types.js';
import { renderVisibleText } from './overlay.js';
import { insertBeforeIEND, iTXt, tEXt, zTXt } from './png-chunks.js';

const join = (p: PayloadValue[]): string => p.map((v) => `${v.category}: ${v.value}`).join(' | ');

async function lsbEmbed(png: Buffer, text: string): Promise<Buffer> {
  const { data, info } = await sharp(png).raw().toBuffer({ resolveWithObject: true });
  const payload = Buffer.from(text, 'utf8');
  const header = Buffer.alloc(4);
  header.writeUInt32BE(payload.length, 0);
  const blob = Buffer.concat([header, payload]);
  const bits = blob.length * 8;
  if (bits > data.length) {
    throw new Error('cover image too small for stego payload');
  }
  for (let i = 0; i < bits; i++) {
    const bit = (blob[i >> 3] >> (7 - (i & 7))) & 1;
    data[i] = (data[i] & 0xfe) | bit;
  }
  return sharp(data, {
    raw: { width: info.width, height: info.height, channels: info.channels },
  })
    .png()
    .toBuffer();
}

/** Recover an LSB-embedded UTF-8 payload from a PNG. */
export async function lsbExtract(png: Buffer): Promise<string> {
  const { data } = await sharp(png).raw().toBuffer({ resolveWithObject: true });
  const readByte = (start: number): number => {
    let b = 0;
    for (let k = 0; k < 8; k++) {
      b = (b << 1) | (data[start + k] & 1);
    }
    return b;
  };
  let len = 0;
  for (let i = 0; i < 4; i++) {
    len = (len << 8) | readByte(i * 8);
  }
  const out = Buffer.alloc(len);
  for (let i = 0; i < len; i++) {
    out[i] = readByte((4 + i) * 8);
  }
  return out.toString('utf8');
}

export const pngTechniques: Record<string, Technique> = {
  'text-chunks': {
    id: 'text-chunks',
    format: 'png',
    label: 'tEXt/zTXt/iTXt chunks',
    embed: (c, p) =>
      insertBeforeIEND(
        c,
        Buffer.concat([
          tEXt('Comment', join(p)),
          zTXt('Description', join(p)),
          iTXt('Keywords', join(p)),
        ]),
      ),
  },
  trailer: {
    id: 'trailer',
    format: 'png',
    label: 'bytes after IEND',
    embed: (c, p) => Buffer.concat([c, Buffer.from(`\nDLP ${join(p)}\n`, 'utf8')]),
  },
  'stego-lsb': {
    id: 'stego-lsb',
    format: 'png',
    label: 'LSB steganography',
    embed: (c, p) => lsbEmbed(c, join(p)),
  },
  visible: {
    id: 'visible',
    format: 'png',
    label: 'visible rendered text (dark on light)',
    embed: (c, p) => renderVisibleText(c, p, 'png'),
  },
};
