import type { CommandNode, ExampleMap, PageNode } from './model.js';

function optionsTable(node: CommandNode): string {
  if (node.options.length === 0) return '';
  const rows = node.options.map((o) => {
    const req = o.required ? 'Yes' : 'No';
    const def = o.defaultValue ? `\`${o.defaultValue}\`` : '—';
    return `| \`${o.flags}\` | ${req} | ${def} | ${o.description || '—'} |`;
  });
  return [
    '| Flag | Required | Default | Description |',
    '|------|:--------:|---------|-------------|',
    ...rows,
  ].join('\n');
}

function argsList(node: CommandNode): string {
  if (node.args.length === 0) return '';
  return node.args
    .map((a) => {
      const tags = [a.required ? '(required)' : '(optional)', a.variadic ? '(variadic)' : '']
        .filter(Boolean)
        .join(' ');
      return `- \`${a.name}\` ${tags} — ${a.description || ''}`.trimEnd();
    })
    .join('\n');
}

function examplesBlock(path: string, examples: ExampleMap): string {
  const list = examples[path];
  if (!list?.length) {
    return [
      ':::warning[Example needed]',
      'No curated input/output example for this command yet.',
      ':::',
    ].join('\n');
  }
  return list
    .map((e) => {
      const parts: string[] = [];
      if (e.note) parts.push(`*${e.note}*`);
      parts.push(`\`\`\`bash\n${e.input.trimEnd()}\n\`\`\``);
      if (e.output) parts.push(`\`\`\`text\n${e.output.replace(/\n+$/, '')}\n\`\`\``);
      return parts.join('\n\n');
    })
    .join('\n\n');
}

export function renderCommand(node: CommandNode, examples: ExampleMap, level = 2): string {
  const h = '#'.repeat(level);
  const sub = '#'.repeat(level + 1);
  const out: string[] = [`${h} ${node.path}`, ''];
  if (node.description) out.push(node.description, '');
  out.push('```text', node.usage, '```');
  const args = argsList(node);
  if (args) out.push('', `${sub} Arguments`, '', args);
  const opts = optionsTable(node);
  if (opts) out.push('', `${sub} Options`, '', opts);
  out.push('', `${sub} Examples`, '', examplesBlock(node.path, examples));
  return out.join('\n');
}

export function renderGroupPage(page: PageNode, examples: ExampleMap): string {
  const multi = page.commands.length > 1;
  const blocks = page.commands.map((c) => renderCommand(c, examples, multi ? 3 : 2));
  const body = multi ? blocks.join('\n\n---\n\n') : blocks.join('\n\n');
  const label = page.title.split(' ').pop() ?? page.title;
  const frontmatter = ['---', `sidebar_label: ${label}`, '---', ''].join('\n');
  return [frontmatter, `# ${page.title}`, '', body, ''].join('\n');
}

export function renderIndex(pages: PageNode[]): string {
  const lines = pages
    .slice()
    .sort((a, b) => a.slug.localeCompare(b.slug))
    .map((p) => `- [\`airs ${p.title}\`](${p.slug}.md)`);
  return [
    '---',
    'sidebar_label: Overview',
    'sidebar_position: 0',
    'slug: /cli/',
    '---',
    '',
    '# CLI Reference',
    '',
    'Auto-generated from the `airs` command tree. Every command below lists its synopsis, options, and at least one input/output example.',
    '',
    ...lines,
    '',
  ].join('\n');
}
