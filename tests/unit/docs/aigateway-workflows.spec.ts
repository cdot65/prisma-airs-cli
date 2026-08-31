import { readFile } from 'node:fs/promises';

const docsRoot = new URL('../../../docs-site/', import.meta.url);

async function read(relativePath: string): Promise<string> {
  return readFile(new URL(relativePath, docsRoot), 'utf8');
}

describe('AI Gateway workflow documentation', () => {
  it('documents the complete workspace and integration binding workflow', async () => {
    const page = await read('docs/cli/aigateway/workflows.md');

    expect(page).toContain('# AI Gateway workflow cheat sheet');
    expect(page).toContain('PANW_MGMT_CLIENT_ID');
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
});
