import { readFile } from 'node:fs/promises';

const root = new URL('../../../', import.meta.url);
const read = (path: string) => readFile(new URL(path, root), 'utf8');

describe('Prisma AIRS CLI brand assets', () => {
  it('loads the supplied PNG and uses it throughout the documentation entrypoints', async () => {
    const [logo, home, overview, config, readme] = await Promise.all([
      readFile(new URL('docs-site/static/img/brand-logo.png', root)),
      read('docs-site/src/pages/index.tsx'),
      read('docs-site/docs/index.mdx'),
      read('docs-site/docusaurus.config.ts'),
      read('README.md'),
    ]);
    expect(logo.subarray(1, 4).toString('ascii')).toBe('PNG');
    expect(logo.readUInt32BE(16)).toBe(1254);
    expect(logo.readUInt32BE(20)).toBe(1254);
    expect(home).toContain("useBaseUrl('/img/brand-logo.png')");
    expect(overview).toContain("useBaseUrl('/img/brand-logo.png')");
    expect(config).toContain("favicon: 'img/brand-logo.png'");
    expect(config).toContain("src: 'img/brand-logo.png'");
    expect(readme).toContain('docs-site/static/img/brand-logo.png');
  });
});
