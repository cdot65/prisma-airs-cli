import type { Format } from '../types.js';
import { docx } from './docx.js';
import { jpeg } from './jpeg.js';
import { pdf } from './pdf.js';
import { png } from './png.js';
import { svg } from './svg.js';

/** Clean-file generators keyed by format. */
export const generators: Record<Format, (rng: () => number) => Promise<Buffer> | Buffer> = {
  pdf,
  png,
  jpeg,
  svg,
  docx,
};

export const EXT: Record<Format, string> = {
  pdf: 'pdf',
  png: 'png',
  jpeg: 'jpg',
  svg: 'svg',
  docx: 'docx',
};
