import type { Command } from 'commander';
import type { Format } from '../../../dlp/types.js';
import { renderGenerateSummary } from '../../renderer/dlp-generate.js';
import { fail, resolveOutput, usageError } from '../../renderer/index.js';

const ALL_FORMATS: Format[] = ['pdf', 'png', 'jpeg', 'svg', 'docx'];

const OPTIONAL_DEPS_HINT =
  'DLP generate requires optional dependencies. Install them with: pnpm add sharp pdf-lib docx piexifjs';

type DlpModule = typeof import('../../../dlp/index.js');
type DlpImporter = () => Promise<DlpModule>;

const defaultImporter: DlpImporter = () => import('../../../dlp/index.js');

/**
 * Lazily load the DLP corpus generator. The dlp module pulls in sharp,
 * pdf-lib, docx, and piexifjs (optionalDependencies) — importing it eagerly
 * would make every CLI invocation pay their startup cost, and would crash
 * installs that skipped optional deps.
 */
export async function loadGenerateCorpus(
  importer: DlpImporter = defaultImporter,
): Promise<DlpModule['generateCorpus']> {
  try {
    const mod = await importer();
    return mod.generateCorpus;
  } catch (err) {
    const code = (err as { code?: string }).code;
    if (code === 'ERR_MODULE_NOT_FOUND' || code === 'MODULE_NOT_FOUND') {
      throw new Error(OPTIONAL_DEPS_HINT);
    }
    throw err;
  }
}

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
  const command = parent
    .command('generate')
    .description(
      'Generate clean + dirty DLP test files (synthetic sensitive data) across PDF/PNG/JPEG/SVG/DOCX',
    )
    .option('--types <list>', 'Comma list: pdf,png,jpeg,svg,docx (or all)', 'all')
    .option('--count <n>', 'Clean files per type', '1')
    .option('--out <dir>', 'Output base directory', './temp')
    .option('--techniques <list>', 'all or comma list of technique ids', 'all')
    .option('--seed <n>', 'Safe integer seed for reproducible payloads')
    .option('--output <format>', 'Summary format: pretty or json', 'pretty')
    .action(async (opts) => {
      let types: Format[];
      try {
        types = parseTypes(opts.types);
      } catch (err) {
        usageError(err instanceof Error ? err.message : String(err));
      }
      const count = Number(opts.count);
      if (!/^\d+$/.test(opts.count) || !Number.isSafeInteger(count) || count < 1) {
        usageError('--count must be a positive safe integer');
      }
      const techniques =
        opts.techniques === 'all'
          ? 'all'
          : (opts.techniques as string).split(',').map((t) => t.trim());
      const seed = opts.seed === undefined ? undefined : Number(opts.seed);
      if (seed !== undefined && (!/^-?\d+$/.test(opts.seed) || !Number.isSafeInteger(seed))) {
        usageError('--seed must be a safe integer');
      }

      try {
        // Commander can consume --output at the root even after the subcommand.
        // Resolve the same explicit/global/config precedence as other CLI commands,
        // before loading native dependencies or creating any files.
        const output = await resolveOutput(command, opts, { allowed: ['pretty', 'json'] });
        const generateCorpus = await loadGenerateCorpus();
        const summary = await generateCorpus({ types, count, out: opts.out, techniques, seed });
        renderGenerateSummary(summary, output);
      } catch (err) {
        fail(err);
      }
    });
}
