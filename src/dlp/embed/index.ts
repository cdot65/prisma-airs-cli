import type { Format, Technique } from '../types.js';
import { docxTechniques } from './docx.js';
import { jpegTechniques } from './jpeg.js';
import { pdfTechniques } from './pdf.js';
import { pngTechniques } from './png.js';
import { svgTechniques } from './svg.js';

/** All embedding techniques grouped by file format. */
export const techniquesByFormat: Record<Format, Technique[]> = {
  pdf: Object.values(pdfTechniques),
  png: Object.values(pngTechniques),
  jpeg: Object.values(jpegTechniques),
  svg: Object.values(svgTechniques),
  docx: Object.values(docxTechniques),
};

/** Unique set of all technique ids across formats. */
export const ALL_TECHNIQUE_IDS: string[] = [
  ...new Set(
    Object.values(techniquesByFormat)
      .flat()
      .map((t) => t.id),
  ),
];
