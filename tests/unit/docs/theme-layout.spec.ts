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

  it('adds an independent desktop collapse control to the right on-page navigation', async () => {
    const [layout, styles] = await Promise.all([
      readFile(
        new URL('../../../docs-site/src/theme/DocItem/Layout/index.tsx', import.meta.url),
        'utf8',
      ),
      readFile(
        new URL('../../../docs-site/src/theme/DocItem/Layout/styles.module.css', import.meta.url),
        'utf8',
      ),
    ]);

    expect(layout).toContain("aria-label={tocCollapsed ? 'Expand on-page navigation'");
    expect(layout).toContain('aria-expanded={!tocCollapsed}');
    expect(layout).toContain('{docTOC.desktop}');
    expect(styles).toContain('.tocColumnCollapsed');
    expect(styles).toContain('@media (min-width: 997px)');
  });
});
