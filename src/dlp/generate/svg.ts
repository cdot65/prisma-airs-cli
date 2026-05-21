import { pick } from '../rng.js';

/** Generate a valid SVG (namespaced root) with deterministic placeholder shapes. */
export function svg(rng: () => number): Buffer {
  const width = 240;
  const height = 160;
  const fill = pick(rng, ['#673ab7', '#2196f3', '#4caf50', '#ff9800']);
  const cx = 40 + Math.floor(rng() * 160);
  const xml =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" ` +
    `viewBox="0 0 ${width} ${height}" role="img">` +
    '<title>Generated image</title>' +
    '<desc>A generated placeholder graphic.</desc>' +
    `<rect width="${width}" height="${height}" fill="#ffffff"/>` +
    `<circle cx="${cx}" cy="80" r="48" fill="${fill}"/>` +
    '</svg>';
  return Buffer.from(xml, 'utf8');
}
