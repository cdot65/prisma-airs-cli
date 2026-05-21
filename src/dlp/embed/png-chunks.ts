import { deflateSync, inflateSync } from 'node:zlib';

const CRC_TABLE: Uint32Array = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf: Buffer): number {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  }
  return (c ^ 0xffffffff) >>> 0;
}

export interface Chunk {
  type: string;
  data: Buffer;
}

export function parseChunks(png: Buffer): Chunk[] {
  const chunks: Chunk[] = [];
  let off = 8;
  while (off + 8 <= png.length) {
    const len = png.readUInt32BE(off);
    const type = png.toString('latin1', off + 4, off + 8);
    chunks.push({ type, data: png.subarray(off + 8, off + 8 + len) });
    off += 12 + len;
    if (type === 'IEND') {
      break;
    }
  }
  return chunks;
}

function buildChunk(type: string, data: Buffer): Buffer {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, 'latin1');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([len, typeBuf, data, crc]);
}

export function insertBeforeIEND(png: Buffer, extra: Buffer): Buffer {
  let off = 8;
  while (off + 8 <= png.length) {
    const len = png.readUInt32BE(off);
    const type = png.toString('latin1', off + 4, off + 8);
    if (type === 'IEND') {
      break;
    }
    off += 12 + len;
  }
  return Buffer.concat([png.subarray(0, off), extra, png.subarray(off)]);
}

export const tEXt = (keyword: string, text: string): Buffer =>
  buildChunk(
    'tEXt',
    Buffer.concat([Buffer.from(keyword, 'latin1'), Buffer.from([0]), Buffer.from(text, 'latin1')]),
  );

export const zTXt = (keyword: string, text: string): Buffer =>
  buildChunk(
    'zTXt',
    Buffer.concat([
      Buffer.from(keyword, 'latin1'),
      Buffer.from([0, 0]),
      deflateSync(Buffer.from(text, 'latin1')),
    ]),
  );

export const iTXt = (keyword: string, text: string): Buffer =>
  buildChunk(
    'iTXt',
    Buffer.concat([
      Buffer.from(keyword, 'latin1'),
      Buffer.from([0, 0, 0, 0, 0]),
      Buffer.from(text, 'utf8'),
    ]),
  );

/** Recover all textual-chunk content (tEXt/zTXt/iTXt) as a single string. */
export function readTextChunks(png: Buffer): string {
  const parts: string[] = [];
  for (const ch of parseChunks(png)) {
    if (ch.type === 'tEXt') {
      parts.push(ch.data.toString('latin1'));
    } else if (ch.type === 'zTXt') {
      const nul = ch.data.indexOf(0);
      parts.push(inflateSync(ch.data.subarray(nul + 2)).toString('latin1'));
    } else if (ch.type === 'iTXt') {
      let i = ch.data.indexOf(0) + 3; // after keyword\0 + compFlag + compMethod
      i = ch.data.indexOf(0, i) + 1; // after language tag
      i = ch.data.indexOf(0, i) + 1; // after translated keyword
      parts.push(ch.data.subarray(i).toString('utf8'));
    }
  }
  return parts.join('\n');
}
