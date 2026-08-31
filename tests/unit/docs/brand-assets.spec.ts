import { readdir, readFile } from 'node:fs/promises';

const root = new URL('../../../', import.meta.url);

async function read(relativePath: string): Promise<string> {
  return readFile(new URL(relativePath, root), 'utf8');
}

describe('Prisma AIRS CLI brand assets', () => {
  it('uses the shield, terminal, white beam, and six-color spectrum in the canonical SVG', async () => {
    const logo = await read('docs-site/static/img/logo.svg');

    expect(logo).toContain('viewBox="0 0 1200 1200"');
    expect(logo).toContain('clipPath id="shield"');
    expect(logo).toContain('linearGradient id="beam"');
    expect(logo).toContain('x="430" y="385" width="340" height="310"');
    for (const color of ['#f33b2f', '#f58a2b', '#f7e34b', '#4dc56b', '#55b9ec', '#9b7ad6']) {
      expect(logo).toContain(color);
    }
    expect(logo).not.toContain('shieldStroke');
    expect(logo).not.toContain('airsMark');
  });

  it('keeps one canonical SVG and points README and Docusaurus at it', async () => {
    const [assets, readme, home, config] = await Promise.all([
      readdir(new URL('docs-site/static/img/', root)),
      read('README.md'),
      read('docs-site/docs/index.mdx'),
      read('docs-site/docusaurus.config.ts'),
    ]);

    expect(assets.filter((name) => name.endsWith('.svg')).sort()).toEqual(['logo.svg']);
    expect(readme).toContain(
      'https://raw.githubusercontent.com/cdot65/prisma-airs-cli/main/docs-site/static/img/logo.svg',
    );
    expect(readme).not.toContain('logo-wordmark');
    expect(home).toContain("useBaseUrl('/img/logo.svg')");
    expect(home).not.toContain('logo-wordmark');
    expect(config).toContain("favicon: 'img/logo.svg'");
    expect(config).toContain("src: 'img/logo.svg'");
  });

  it('keeps the generated social card at the required 1200 by 630 dimensions', async () => {
    const card = await readFile(new URL('docs-site/static/img/social-card.png', root));

    expect(card.subarray(1, 4).toString('ascii')).toBe('PNG');
    expect(card.readUInt32BE(16)).toBe(1200);
    expect(card.readUInt32BE(20)).toBe(630);
  });
});
