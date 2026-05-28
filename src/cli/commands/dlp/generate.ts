import chalk from 'chalk';
import type { Command } from 'commander';
import { generateCorpus } from '../../../dlp/index.js';
import type { Format } from '../../../dlp/types.js';

const ALL_FORMATS: Format[] = ['pdf', 'png', 'jpeg', 'svg', 'docx'];

function parseTypes(value: string): Format[] {
  if (value === 'all') {
    return ALL_FORMATS;
  }
  const types = value.split(',').map((t) => t.trim().toLowerCase());
  const invalid = types.filter((t) => !ALL_FORMATS.includes(t as Format));
  if (invalid.length > 0) {
    throw new Error(`Unknown type(s): ${invalid.join(', ')}. Valid: ${ALL_FORMATS.join(', ')}`);
  }
  return types as Format[];
}

export function register(parent: Command): void {
  parent
    .command('generate')
    .description(
      'Generate clean + dirty DLP test files (synthetic sensitive data) across PDF/PNG/JPEG/SVG/DOCX',
    )
    .option('--types <list>', 'Comma list: pdf,png,jpeg,svg,docx (or all)', 'all')
    .option('--count <n>', 'Clean files per type', '1')
    .option('--out <dir>', 'Output base directory', './temp')
    .option('--techniques <list>', 'all or comma list of technique ids', 'all')
    .option('--seed <n>', 'Seed for reproducible payloads')
    .option('--output <format>', 'Summary format: pretty or json', 'pretty')
    .action(async (opts) => {
      const types = parseTypes(opts.types);
      const count = Number.parseInt(opts.count, 10);
      if (!Number.isInteger(count) || count < 1) {
        throw new Error('--count must be a positive integer');
      }
      const techniques =
        opts.techniques === 'all'
          ? 'all'
          : (opts.techniques as string).split(',').map((t) => t.trim());
      const seed = opts.seed === undefined ? undefined : Number.parseInt(opts.seed, 10);

      const summary = await generateCorpus({ types, count, out: opts.out, techniques, seed });

      if (opts.output === 'json') {
        console.log(JSON.stringify(summary, null, 2));
        return;
      }

      console.log(chalk.bold('\n  DLP Test-File Generation'));
      console.log(`  Output:   ${summary.out}`);
      console.log(`  Seed:     ${summary.seed}`);
      console.log(`  Clean:    ${summary.clean}    Dirty: ${summary.dirty}`);
      console.log(`  Manifest: ${summary.manifestPath}\n`);
      for (const [fmt, counts] of Object.entries(summary.byFormat)) {
        console.log(`    ${fmt.padEnd(5)} clean=${counts.clean} dirty=${counts.dirty}`);
      }
      console.log('');
    });
}
