export interface Raster {
  data: Buffer;
  width: number;
  height: number;
  channels: 3;
}

/** Build a deterministic RGB gradient raster for use as image cover content. */
export function raster(rng: () => number): Raster {
  const width = 256;
  const height = 192;
  const channels = 3 as const;
  const data = Buffer.alloc(width * height * channels);
  const rb = Math.floor(rng() * 256);
  const gb = Math.floor(rng() * 256);
  const bb = Math.floor(rng() * 256);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * channels;
      data[i] = (x + rb) & 255;
      data[i + 1] = (y + gb) & 255;
      data[i + 2] = (x + y + bb) & 255;
    }
  }
  return { data, width, height, channels };
}
