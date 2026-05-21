import sharp from 'sharp';
import { raster } from './raster.js';

/** Generate a valid JPEG with deterministic gradient content. */
export async function jpeg(rng: () => number): Promise<Buffer> {
  const { data, width, height, channels } = raster(rng);
  return sharp(data, { raw: { width, height, channels } }).jpeg({ quality: 90 }).toBuffer();
}
