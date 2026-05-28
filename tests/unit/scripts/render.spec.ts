import { describe, expect, it } from 'vitest';
import type { CommandNode, ExampleMap, PageNode } from '../../../scripts/cli-docs/model.js';
import { renderGroupPage, renderIndex, renderSummary } from '../../../scripts/cli-docs/render.js';

const scan: CommandNode = {
  path: 'runtime scan',
  name: 'scan',
  description: 'Scan a prompt',
  usage: 'airs runtime scan [options] <prompt>',
  args: [{ name: 'prompt', required: true, variadic: false, description: 'the prompt' }],
  options: [
    { flags: '--profile <name>', description: 'profile name', required: true },
    {
      flags: '--response <text>',
      description: 'response text',
      required: false,
      defaultValue: undefined,
    },
  ],
  subcommands: [],
  isLeaf: true,
};

const examples: ExampleMap = {
  'runtime scan': [
    { input: 'airs runtime scan --profile p "hi"', output: 'Action: ALLOW', note: 'Allowed' },
  ],
};

describe('renderGroupPage', () => {
  const page: PageNode = {
    slug: 'runtime/scan',
    title: 'runtime scan',
    groupPath: 'runtime scan',
    commands: [scan],
  };
  const md = renderGroupPage(page, examples);

  it('renders title, synopsis, options table and the example', () => {
    expect(md).toContain('# runtime scan');
    expect(md).toContain('airs runtime scan [options] <prompt>');
    expect(md).toContain('| `--profile <name>` | Yes |');
    expect(md).toContain('| `--response <text>` | No |');
    expect(md).toContain('airs runtime scan --profile p "hi"');
    expect(md).toContain('Action: ALLOW');
  });

  it('emits a warning admonition when no example exists', () => {
    const md2 = renderGroupPage(page, {});
    expect(md2).toContain('!!! warning "Example needed"');
  });
});

describe('renderIndex', () => {
  it('links each page by slug', () => {
    const idx = renderIndex([
      { slug: 'runtime/scan', title: 'runtime scan', groupPath: 'runtime scan', commands: [scan] },
    ]);
    expect(idx).toContain('# CLI Reference');
    expect(idx).toContain('runtime/scan.md');
  });
});

describe('renderSummary', () => {
  const pages: PageNode[] = [
    {
      slug: 'runtime/api-keys',
      title: 'runtime api-keys',
      groupPath: 'runtime api-keys',
      commands: [scan],
    },
    {
      slug: 'runtime/dlp/dictionaries',
      title: 'runtime dlp dictionaries',
      groupPath: 'runtime dlp dictionaries',
      commands: [scan],
    },
    {
      slug: 'runtime/dlp/filtering-profiles',
      title: 'runtime dlp filtering-profiles',
      groupPath: 'runtime dlp filtering-profiles',
      commands: [scan],
    },
    {
      slug: 'runtime/dlp/patterns',
      title: 'runtime dlp patterns',
      groupPath: 'runtime dlp patterns',
      commands: [scan],
    },
    {
      slug: 'runtime/dlp/profiles',
      title: 'runtime dlp profiles',
      groupPath: 'runtime dlp profiles',
      commands: [scan],
    },
    {
      slug: 'runtime/dlp-gen',
      title: 'runtime dlp-gen',
      groupPath: 'runtime dlp-gen',
      commands: [scan],
    },
    { slug: 'runtime/scan', title: 'runtime scan', groupPath: 'runtime scan', commands: [scan] },
    { slug: 'redteam/scan', title: 'redteam scan', groupPath: 'redteam scan', commands: [scan] },
    {
      slug: 'model-security/groups',
      title: 'model-security groups',
      groupPath: 'model-security groups',
      commands: [scan],
    },
  ];

  it('groups pages by top-level command with DLP entries flat under Runtime', () => {
    const out = renderSummary(pages);
    const lines = out.split('\n');
    const runtimeIdx = lines.findIndex((l) => l === '* Runtime');
    const redteamIdx = lines.findIndex((l) => l === '* Redteam');
    expect(runtimeIdx).toBeGreaterThanOrEqual(0);
    expect(redteamIdx).toBeGreaterThan(runtimeIdx);
    const runtimeBlock = lines
      .slice(runtimeIdx + 1, redteamIdx)
      .filter((l) => l.startsWith('    *'));
    // DLP entries listed as flat siblings — single-level indent, not nested under another group
    expect(runtimeBlock).toContain(
      '    * [airs runtime dlp dictionaries](runtime/dlp/dictionaries.md)',
    );
    expect(runtimeBlock).toContain(
      '    * [airs runtime dlp filtering-profiles](runtime/dlp/filtering-profiles.md)',
    );
    expect(runtimeBlock).toContain('    * [airs runtime dlp patterns](runtime/dlp/patterns.md)');
    expect(runtimeBlock).toContain('    * [airs runtime dlp profiles](runtime/dlp/profiles.md)');
    // No nested "Dlp" group label
    expect(out).not.toMatch(/^\s*\*\s+Dlp\s*$/m);
  });

  it('preserves existing flat pages (dlp-gen) without duplication', () => {
    const out = renderSummary(pages);
    expect(out.match(/runtime\/dlp-gen\.md/g)?.length).toBe(1);
  });

  it('no longer emits a runtime/dlp-profiles entry', () => {
    const out = renderSummary(pages);
    expect(out).not.toMatch(/runtime\/dlp-profiles\.md/);
  });

  it('sorts entries within a group by title alphabetically', () => {
    const out = renderSummary(pages);
    const lines = out.split('\n');
    const runtimeLinks = lines
      .filter((l) => l.startsWith('    * [airs runtime'))
      .map((l) => l.match(/\[(airs runtime[^\]]+)\]/)?.[1] ?? '');
    const sorted = [...runtimeLinks].sort();
    expect(runtimeLinks).toEqual(sorted);
  });

  it('emits index.md as the section landing', () => {
    expect(renderSummary(pages)).toContain('* [CLI Reference](index.md)');
  });

  it('handles all three top-level groups', () => {
    const out = renderSummary(pages);
    expect(out).toContain('* Runtime');
    expect(out).toContain('* Redteam');
    expect(out).toContain('* Model-security');
  });
});
