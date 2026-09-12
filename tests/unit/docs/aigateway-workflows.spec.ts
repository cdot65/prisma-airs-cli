import { readFile } from 'node:fs/promises';
import ts from 'typescript';

const docsRoot = new URL('../../../docs-site/', import.meta.url);

async function read(relativePath: string): Promise<string> {
  return readFile(new URL(relativePath, docsRoot), 'utf8');
}

describe('AI Gateway workflow documentation', () => {
  it('expands the AI Gateway category without navigating to another sidebar', async () => {
    // Inspect the config without loading the separately installed docs-site toolchain.
    const source = ts.createSourceFile(
      'sidebars.ts',
      await read('sidebars.ts'),
      ts.ScriptTarget.Latest,
    );
    const categories: ts.ObjectLiteralExpression[] = [];
    const visit = (node: ts.Node) => {
      if (ts.isObjectLiteralExpression(node)) categories.push(node);
      ts.forEachChild(node, visit);
    };
    visit(source);
    const property = (node: ts.ObjectLiteralExpression, name: string) =>
      node.properties.find(
        (p): p is ts.PropertyAssignment =>
          ts.isPropertyAssignment(p) && p.name.getText(source) === name,
      )?.initializer;
    const gateway = categories.find((node) => {
      const label = property(node, 'label');
      return label && ts.isStringLiteral(label) && label.text === 'AI Gateway';
    });
    expect(gateway).toBeDefined();
    if (!gateway) throw new Error('Missing AI Gateway category');
    expect(property(gateway, 'link')).toBeUndefined();
    const items = property(gateway, 'items');
    if (!items || !ts.isArrayLiteralExpression(items)) throw new Error('Missing Gateway children');
    expect(items.elements[0]?.getText(source)).toBe("'cli/aigateway/workflows'");
    expect(items.elements).toHaveLength(5);
  });

  it('documents the complete workspace and integration binding workflow', async () => {
    const page = await read('docs/cli/aigateway/workflows.md');

    expect(page).toContain('# AI Gateway workflow cheat sheet');
    expect(page).toContain('airs tenant create dev');
    expect(page).toContain('--scope-name');
    expect(page).toContain('scopeName');
    expect(page).toContain('slug');
    expect(page).toContain('integrations workspaces set');
    expect(page).toContain('--workspace-binding <workspace-uuid>=true');
    expect(page).toContain('--preserve-existing');
    expect(page).toContain('replaces all existing workspace bindings');
    expect(page).toContain('AISEC_OAUTH_ERROR');
    expect(page).toContain('::::tip[Configuration-file alternative]');
    expect(page).not.toContain('\n:::tip ');
  });

  it('links the cheat sheet from primary AI Gateway navigation', async () => {
    const [sidebars, config, home] = await Promise.all([
      read('sidebars.ts'),
      read('docusaurus.config.ts'),
      read('docs/index.mdx'),
    ]);

    expect(sidebars).toContain("'cli/aigateway/workflows'");
    expect(config).toContain("to: '/cli/aigateway/workflows/'");
    expect(home).toContain('(cli/aigateway/workflows.md)');
  });

  it('documents safe defaults for API-key read commands', async () => {
    const page = await read('docs/cli/aigateway/resources.md');

    expect(page).toContain('API-key list/detail reads');
    expect(page).toContain('redacted by default');
    expect(page).toContain('`--reveal-sensitive`');
  });
});
