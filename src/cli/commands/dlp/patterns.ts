import type { Command } from 'commander';
import { SdkDataPatternsService } from '../../../airs/dlp/data-patterns.js';
import { dlpPatterns, fail, type OutputFormat, usageError } from '../../renderer/index.js';
import { buildPatternBody, repeatable } from './build-body.js';
import { buildMergePatch, parseBody } from './patch.js';

function listFlags<T extends Command>(cmd: T): T {
  return cmd
    .option('--page <n>', 'Zero-indexed page number', (v) => Number.parseInt(v, 10))
    .option('--size <n>', 'Page size', (v) => Number.parseInt(v, 10))
    .option('--sort <field,dir>', 'Sort criteria (repeatable)', repeatable)
    .option('--output <fmt>', 'Output format', 'pretty');
}

function writeFlags<T extends Command>(cmd: T): T {
  return cmd
    .option('--name <s>', 'Pattern name (required unless --body-file)')
    .option('--type <s>', 'Pattern type: predefined|custom|file_property (default: custom)')
    .option('--description <s>', 'Pattern description')
    .option('--technique <s>', 'Detection technique (default: regex)')
    .option('--confidence-levels <csv>', 'Confidence levels CSV: e.g. high,low')
    .option('--regex <pattern>', 'Regex with weight=1 (repeatable)', repeatable)
    .option('--weighted-regex <PATTERN|N>', 'Regex with explicit weight (repeatable)', repeatable)
    .option('--delimiter <s>', 'Delimiter for proximity matching')
    .option('--proximity-distance <n>', 'Proximity window (2..1000)')
    .option('--proximity-keyword <s>', 'Proximity keyword (repeatable)', repeatable)
    .option('--tag <k=v>', 'Tag (repeatable, value can be CSV)', repeatable)
    .option('--body <json|->', 'Raw JSON body (escape hatch; or "-" for stdin)')
    .option('--body-file <path>', 'Raw JSON body file (escape hatch)')
    .option('--output <fmt>', 'Output format', 'pretty');
}

async function resolveWriteBody(opts: Record<string, unknown>): Promise<unknown> {
  if (opts.body || opts.bodyFile) {
    const body = await parseBody({ body: opts.body as string, bodyFile: opts.bodyFile as string });
    if (!body) throw new Error('--body or --body-file was empty');
    return body;
  }
  return buildPatternBody(opts);
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
      fail(err);
    }
  });

  writeFlags(group.command('create').description('Create a data pattern')).action(async (opts) => {
    try {
      const body = await resolveWriteBody(opts);
      dlpPatterns.renderCreated(
        // biome-ignore lint/suspicious/noExplicitAny: body shape verified by SDK Zod
        await new SdkDataPatternsService().create(body as any),
        opts.output as OutputFormat,
      );
    } catch (err) {
      usageError(err instanceof Error ? err.message : String(err));
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
        fail(err);
      }
    });

  writeFlags(group.command('replace <id>').description('Full-replace a data pattern (PUT)')).action(
    async (id, opts) => {
      try {
        const body = await resolveWriteBody(opts);
        dlpPatterns.renderReplaced(
          // biome-ignore lint/suspicious/noExplicitAny: body shape verified by SDK Zod
          await new SdkDataPatternsService().replace(id, body as any),
          opts.output as OutputFormat,
        );
      } catch (err) {
        usageError(err instanceof Error ? err.message : String(err));
      }
    },
  );

  group
    .command('patch <id>')
    .description(
      'JSON Merge Patch. Use --body-file for nested fields. ' +
        '--set/--clear coerce values: numbers/booleans/JSON literals. ' +
        'To force a string, quote: --set count=\'"5"\'.',
    )
    .option('--body-file <path>', 'JSON merge-patch body file')
    .option('--set <k=v...>', 'Set scalar field (repeatable)', repeatable)
    .option('--clear <key...>', 'Clear field via merge-patch null (repeatable)', repeatable)
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
        usageError(err instanceof Error ? err.message : String(err));
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
        fail(err);
      }
    });
}
