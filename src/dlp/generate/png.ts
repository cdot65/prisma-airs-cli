import sharp from 'sharp';
import { raster } from './raster.js';

/** Generate a valid PNG with deterministic gradient content. */
export async function png(rng: () => number): Promise<Buffer> {
  const { data, width, height, channels } = raster(rng);
  return sharp(data, { raw: { width, height, channels } }).png().toBuffer();
}
