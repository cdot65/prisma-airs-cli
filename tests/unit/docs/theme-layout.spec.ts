import { readFile } from 'node:fs/promises';

describe('documentation sidebar layout', () => {
  it('keeps the left navigation in place and enables its native collapse control', async () => {
    const config = await readFile(
      new URL('../../../docs-site/docusaurus.config.ts', import.meta.url),
      'utf8',
    );

    expect(config).toMatch(/docs:\s*{\s*sidebar:\s*{\s*hideable:\s*true,?\s*},?\s*}/s);
    const css = await readFile(
      new URL('../../../docs-site/src/css/custom.css', import.meta.url),
      'utf8',
    );

    expect(css).not.toMatch(/\.theme-doc-sidebar-container\s*\{[^}]*order:/s);
  });

  it('retains document navigation and a readable full-width article layout', async () => {
    const layout = await readFile(
      new URL('../../../docs-site/src/theme/DocItem/Layout/index.tsx', import.meta.url),
      'utf8',
    );
    expect(layout).toContain('<DocBreadcrumbs />');
    expect(layout).toContain('<DocItemPaginator />');
    expect(layout).toContain('<DocItemTOCMobile />');
  });
});
