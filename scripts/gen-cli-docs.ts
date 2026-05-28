import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { buildProgram } from '../src/cli/program.js';
import { loadExamples } from './cli-docs/examples.js';
import { renderGroupPage, renderIndex, renderSummary } from './cli-docs/render.js';
import { collectPages, walkProgram } from './cli-docs/walk.js';

const OUT_DIR = 'docs/cli';
const EXAMPLES_DIR = 'docs/cli/examples';
const GROUP_DIRS = ['runtime', 'redteam', 'model-security'];

export function generate(outDir = OUT_DIR, examplesDir = EXAMPLES_DIR): string[] {
  // Clear previously generated group subtrees (keep examples/ and index.md handling explicit).
  for (const g of GROUP_DIRS) rmSync(join(outDir, g), { recursive: true, force: true });

  const pages = collectPages(walkProgram(buildProgram()));
  const examples = loadExamples(examplesDir);

  const written: string[] = [];
  for (const page of pages) {
    const file = join(outDir, `${page.slug}.md`);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, renderGroupPage(page, examples));
    written.push(file);
  }
  const indexFile = join(outDir, 'index.md');
  writeFileSync(indexFile, renderIndex(pages));
  written.push(indexFile);

  const summaryFile = join(outDir, 'SUMMARY.md');
  writeFileSync(summaryFile, renderSummary(pages));
  written.push(summaryFile);
  return written;
}

// Run when invoked directly via tsx.
if (import.meta.url === `file://${process.argv[1]}`) {
  const files = generate();
  console.log(`Generated ${files.length} CLI reference files.`);
}
