import type { PayloadValue, Technique } from '../types.js';

const line = (vals: PayloadValue[]): string =>
  vals.map((v) => `${v.category}: ${v.value}`).join(' | ');

/** Insert a snippet immediately after the opening `<svg ...>` tag. */
function inject(clean: Buffer, snippet: string): Buffer {
  const s = clean.toString('utf8');
  const idx = s.indexOf('>') + 1;
  return Buffer.from(s.slice(0, idx) + snippet + s.slice(idx), 'utf8');
}

export const svgTechniques: Record<string, Technique> = {
  meta: {
    id: 'meta',
    format: 'svg',
    label: 'metadata + desc',
    embed: (c, p) => inject(c, `<metadata>${line(p)}</metadata><desc>${line(p)}</desc>`),
  },
  'hidden-text': {
    id: 'hidden-text',
    format: 'svg',
    label: 'off-canvas <text>',
    embed: (c, p) =>
      inject(
        c,
        p.map((v) => `<text x="-9999" y="-9999">${v.category}: ${v.value}</text>`).join(''),
      ),
  },
  comment: {
    id: 'comment',
    format: 'svg',
    label: 'XML comment',
    embed: (c, p) => inject(c, `<!-- ${line(p)} -->`),
  },
};
