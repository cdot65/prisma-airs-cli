import { existsSync, mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';
import { buildProgram } from '../src/cli/program.js';
import { loadExamples, missingExamples } from './cli-docs/examples.js';
import { leafCommands, walkProgram } from './cli-docs/walk.js';
import { generate } from './gen-cli-docs.js';

const COMMITTED = 'docs-site/docs/cli';
const EXAMPLES_DIR = 'docs-site/cli-examples';
const ALLOWLIST = 'docs-site/cli-examples/.missing-allowlist';

function readAllowlist(): Set<string> {
  if (!existsSync(ALLOWLIST)) return new Set();
  return new Set(
    readFileSync(ALLOWLIST, 'utf-8')
      .split('\n')
      .map((l) => l.trim())
      .filter((l) => l && !l.startsWith('#')),
  );
}

function checkDrift(): string[] {
  const tmp = mkdtempSync(join(tmpdir(), 'cli-docs-'));
  const written = generate(tmp, EXAMPLES_DIR);
  const drifted: string[] = [];
  for (const tmpFile of written) {
    const rel = relative(tmp, tmpFile);
    const committed = join(COMMITTED, rel);
    if (
      !existsSync(committed) ||
      readFileSync(committed, 'utf-8') !== readFileSync(tmpFile, 'utf-8')
    ) {
      drifted.push(rel);
    }
  }
  return drifted;
}

function main(): void {
  const drifted = checkDrift();
  const leaves = leafCommands(walkProgram(buildProgram())).map((n) => n.path);
  const allow = readAllowlist();
  const missing = missingExamples(leaves, loadExamples(EXAMPLES_DIR));
  const unexpected = missing.filter((p) => !allow.has(p));
  const staleAllow = [...allow].filter((p) => !missing.includes(p));

  let failed = false;
  if (drifted.length) {
    failed = true;
    console.error('CLI reference is out of date. Run `pnpm docs:gen` and commit:');
    for (const f of drifted) console.error(`  - ${f}`);
  }
  if (unexpected.length) {
    failed = true;
    console.error('Commands missing input/output examples (add to sidecars or allowlist):');
    for (const p of unexpected) console.error(`  - ${p}`);
  }
  if (staleAllow.length) {
    console.warn('Allowlist entries that now HAVE examples (remove them from .missing-allowlist):');
    for (const p of staleAllow) console.warn(`  - ${p}`);
  }
  if (failed) process.exit(1);
  console.log(`docs:check OK — ${leaves.length} commands, ${missing.length} on allowlist.`);
}

main();
