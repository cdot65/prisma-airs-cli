import sharp from 'sharp';
import type { PayloadValue } from '../types.js';

const esc = (s: string): string =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/**
 * Paint the payload as visible text onto an image (foreground != background) by
 * compositing an SVG overlay, then re-encode to the requested raster format.
 */
export async function renderVisibleText(
  clean: Buffer,
  payload: PayloadValue[],
  format: 'png' | 'jpeg',
): Promise<Buffer> {
  const meta = await sharp(clean).metadata();
  const w = meta.width ?? 256;
  const h = meta.height ?? 192;
  const lines = payload
    .map(
      (v, i) =>
        `<text x="8" y="${18 + i * 16}" font-family="sans-serif" font-size="11" fill="#000000">` +
        `${esc(`${v.category}: ${v.value}`)}</text>`,
    )
    .join('');
  const bandH = Math.min(h, 18 + payload.length * 16);
  const overlay =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">` +
    `<rect x="0" y="0" width="${w}" height="${bandH}" fill="#fafafa"/>${lines}</svg>`;
  const img = sharp(clean).composite([{ input: Buffer.from(overlay), top: 0, left: 0 }]);
  return format === 'png' ? img.png().toBuffer() : img.jpeg({ quality: 90 }).toBuffer();
}
