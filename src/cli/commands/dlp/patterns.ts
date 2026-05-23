import type { Command } from 'commander';
import { SdkDataPatternsService } from '../../../airs/dlp/data-patterns.js';
import { dlpPatterns, type OutputFormat, renderError } from '../../renderer/index.js';
import { buildMergePatch, parseBody } from './patch.js';

function listFlags<T extends Command>(cmd: T): T {
  return cmd
    .option('--page <n>', 'Zero-indexed page number', (v) => Number.parseInt(v, 10))
    .option('--size <n>', 'Page size', (v) => Number.parseInt(v, 10))
    .option('--sort <field,dir>', 'Sort criteria (repeatable)', (v, prev: string[] = []) => [
      ...prev,
      v,
    ])
    .option('--output <fmt>', 'Output format', 'pretty');
}

export function register(dlp: Command): void {
  const group = dlp.command('patterns').description('DLP data patterns (full CRUD)');

  listFlags(group.command('list').description('List data patterns')).action(async (opts) => {
    try {
      const svc = new SdkDataPatternsService();
      dlpPatterns.renderList(
        await svc.list({ page: opts.page, size: opts.size, sort: opts.sort }),
        opts.output as OutputFormat,
      );
    } catch (err) {
      renderError(err instanceof Error ? err.message : String(err));
      process.exit(1);
    }
  });

  group
    .command('create')
    .description('Create a data pattern')
    .option('--body <json|->', 'JSON body (or "-" for stdin)')
    .option('--body-file <path>', 'Path to JSON body file')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (opts) => {
      try {
        const body = await parseBody({ body: opts.body, bodyFile: opts.bodyFile });
        if (!body) throw new Error('--body or --body-file is required');
        const svc = new SdkDataPatternsService();
        dlpPatterns.renderCreated(
          // biome-ignore lint/suspicious/noExplicitAny: parseBody returns unknown, cast for create()
          await svc.create(body as any),
          opts.output as OutputFormat,
        );
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(2);
      }
    });

  group
    .command('get <id>')
    .description('Get a data pattern by id')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (id, opts) => {
      try {
        dlpPatterns.renderGet(
          await new SdkDataPatternsService().get(id),
          opts.output as OutputFormat,
        );
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  group
    .command('replace <id>')
    .description('Full-replace a data pattern (PUT)')
    .option('--body <json|->', 'JSON body (or "-" for stdin)')
    .option('--body-file <path>', 'Path to JSON body file')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (id, opts) => {
      try {
        const body = await parseBody({ body: opts.body, bodyFile: opts.bodyFile });
        if (!body) throw new Error('--body or --body-file is required');
        dlpPatterns.renderReplaced(
          // biome-ignore lint/suspicious/noExplicitAny: parseBody returns unknown, cast for replace()
          await new SdkDataPatternsService().replace(id, body as any),
          opts.output as OutputFormat,
        );
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(2);
      }
    });

  group
    .command('patch <id>')
    .description(
      'JSON Merge Patch. Use --body-file for nested fields. ' +
        '--set/--clear coerce values: numbers/booleans/JSON literals. ' +
        'To force a string, quote: --set count=\'"5"\'.',
    )
    .option('--body-file <path>', 'JSON merge-patch body file')
    .option('--set <k=v...>', 'Set scalar field (repeatable)', (v, p: string[] = []) => [...p, v])
    .option(
      '--clear <key...>',
      'Clear field via merge-patch null (repeatable)',
      (v, p: string[] = []) => [...p, v],
    )
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (id, opts) => {
      try {
        if (opts.bodyFile && (opts.set || opts.clear)) {
          throw new Error('--body-file is mutually exclusive with --set/--clear');
        }
        const body = opts.bodyFile
          ? await parseBody({ bodyFile: opts.bodyFile })
          : buildMergePatch({ set: opts.set, clear: opts.clear });
        dlpPatterns.renderPatched(
          // biome-ignore lint/suspicious/noExplicitAny: buildMergePatch returns Record<string, unknown>, cast for patch()
          await new SdkDataPatternsService().patch(id, body as any),
          opts.output as OutputFormat,
        );
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(2);
      }
    });

  group
    .command('delete <id>')
    .description('Soft-delete (archive) a data pattern')
    .action(async (id) => {
      try {
        await new SdkDataPatternsService().delete(id);
        dlpPatterns.renderArchived(id);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
