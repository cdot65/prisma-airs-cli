import { inflateRawSync } from 'node:zlib';

const LOCAL_HEADER = 0x04034b50;

/** Extract concatenated text from all entries of a ZIP buffer (e.g. a DOCX). */
export function unzipText(zip: Buffer): string {
  const parts: string[] = [];
  let off = 0;
  while (off + 30 <= zip.length && zip.readUInt32LE(off) === LOCAL_HEADER) {
    const method = zip.readUInt16LE(off + 8);
    const compSize = zip.readUInt32LE(off + 18);
    const nameLen = zip.readUInt16LE(off + 26);
    const extraLen = zip.readUInt16LE(off + 28);
    const dataStart = off + 30 + nameLen + extraLen;
    const data = zip.subarray(dataStart, dataStart + compSize);
    try {
      const content = method === 8 ? inflateRawSync(data) : data;
      parts.push(content.toString('utf8'));
    } catch {
      // skip entries that cannot be inflated
    }
    off = dataStart + compSize;
  }
  return parts.join('\n');
}
