import { readFile } from 'node:fs/promises';

describe('documentation sidebar layout', () => {
  it('enables the native Docusaurus sidebar collapse control', async () => {
    const config = await readFile(
      new URL('../../../docs-site/docusaurus.config.ts', import.meta.url),
      'utf8',
    );

    expect(config).toMatch(/docs:\s*{\s*sidebar:\s*{\s*hideable:\s*true,?\s*},?\s*}/s);
  });

  it('places the sidebar on the right only at the desktop breakpoint', async () => {
    const css = await readFile(
      new URL('../../../docs-site/src/css/custom.css', import.meta.url),
      'utf8',
    );

    expect(css).toContain('@media (min-width: 997px)');
    expect(css).toMatch(
      /\.theme-doc-sidebar-container\s*\{[^}]*order:\s*2;[^}]*border-right:\s*0;[^}]*border-left:/s,
    );
    expect(css).toMatch(
      /\.theme-doc-sidebar-container\s+\[aria-label='Collapse sidebar'\]\s+svg\s*\{[^}]*rotate\(0deg\)/s,
    );
    expect(css).toMatch(
      /\.theme-doc-sidebar-container\s+\[aria-label='Expand sidebar'\]\s+svg\s*\{[^}]*rotate\(180deg\)/s,
    );
  });
});
