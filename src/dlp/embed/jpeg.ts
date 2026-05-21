import piexif from 'piexifjs';
import type { PayloadValue, Technique } from '../types.js';

const join = (p: PayloadValue[]): string => p.map((v) => `${v.category}: ${v.value}`).join(' | ');

function u16(n: number): Buffer {
  const b = Buffer.alloc(2);
  b.writeUInt16BE(n, 0);
  return b;
}

function setExif(clean: Buffer, text: string): Buffer {
  const zeroth: Record<number, string> = {};
  zeroth[piexif.ImageIFD.ImageDescription] = text;
  zeroth[piexif.ImageIFD.Artist] = text;
  zeroth[piexif.ImageIFD.Copyright] = text;
  const exifStr = piexif.dump({
    '0th': zeroth,
    Exif: {},
    GPS: {},
    Interop: {},
    '1st': {},
    thumbnail: null,
  });
  return Buffer.from(piexif.insert(exifStr, clean.toString('binary')), 'binary');
}

/** Recover EXIF 0th-IFD string fields from a JPEG. */
export function jpegExifExtract(jpeg: Buffer): string {
  const exif = piexif.load(jpeg.toString('binary'));
  const zeroth = exif['0th'] ?? {};
  return Object.values(zeroth)
    .filter((v: unknown): v is string => typeof v === 'string')
    .join('\n');
}

export const jpegTechniques: Record<string, Technique> = {
  exif: {
    id: 'exif',
    format: 'jpeg',
    label: 'EXIF ImageDescription/Artist/Copyright',
    embed: (c, p) => setExif(c, join(p)),
  },
  com: {
    id: 'com',
    format: 'jpeg',
    label: 'COM comment segment',
    embed: (c, p) => {
      const d = Buffer.from(`DLP ${join(p)}`, 'utf8');
      return Buffer.concat([
        c.subarray(0, 2),
        Buffer.from([0xff, 0xfe]),
        u16(d.length + 2),
        d,
        c.subarray(2),
      ]);
    },
  },
  trailer: {
    id: 'trailer',
    format: 'jpeg',
    label: 'bytes after EOI',
    embed: (c, p) => Buffer.concat([c, Buffer.from(`\nDLP ${join(p)}\n`, 'utf8')]),
  },
};
