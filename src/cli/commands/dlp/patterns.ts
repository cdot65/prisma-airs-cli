import type { Command } from 'commander';
import { SdkDataPatternsService } from '../../../airs/dlp/data-patterns.js';
import { registerPageAliases, resolvePageParams } from '../../pagination.js';
import { dlpPatterns, fail, resolveOutput, usageError } from '../../renderer/index.js';
import { buildPatternBody, repeatable } from './build-body.js';
import { loadDlpClientOptions } from './config.js';
import { buildMergePatch, parseBody } from './patch.js';
import { predefinedFlag, visibleRecords } from './visibility.js';

function listFlags<T extends Command>(cmd: T): T {
  cmd
    .option('--limit <n>', 'Max results per page (API page size)', (v) => Number.parseInt(v, 10))
    .option('--offset <n>', 'Starting offset — rounds down to a page boundary', (v) =>
      Number.parseInt(v, 10),
    )
    .option('--sort <field,dir>', 'Sort criteria (repeatable)', repeatable)
    .option('--output <fmt>', 'Output format: pretty, table, markdown, csv, json, yaml');
  registerPageAliases(cmd, { sizeFlag: '--size', sizeKey: 'size' });
  return cmd;
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

  const listCmd = predefinedFlag(
    listFlags(group.command('list').description('List data patterns (tenant-created by default)')),
  );
  listCmd.action(async (opts) => {
    try {
      const { page, size } = resolvePageParams(listCmd, opts);
      const svc = new SdkDataPatternsService(await loadDlpClientOptions());
      const result = opts.all
        ? await svc.listAll({ size, sort: opts.sort, max: Number(opts.max) })
        : (await svc.list({ page, size, sort: opts.sort })).content;
      const visible = visibleRecords(result, opts.includePredefined);
      dlpPatterns.renderList(
        { content: visible, totalElements: visible.length },
        await resolveOutput(listCmd, opts),
      );
    } catch (err) {
      fail(err);
    }
  });

  writeFlags(group.command('create').description('Create a data pattern')).action(
    async (opts, command) => {
      try {
        const format = await resolveOutput(command, opts);
        const body = await resolveWriteBody(opts);
        dlpPatterns.renderCreated(
          // biome-ignore lint/suspicious/noExplicitAny: body shape verified by SDK Zod
          await new SdkDataPatternsService(await loadDlpClientOptions()).create(body as any),
          format,
        );
      } catch (err) {
        usageError(err instanceof Error ? err.message : String(err));
      }
    },
  );

  const getCmd = group
    .command('get <id>')
    .description('Get a data pattern by id')
    .option('--output <fmt>', 'Output format: pretty, table, markdown, csv, json, yaml')
    .action(async (id, opts) => {
      try {
        dlpPatterns.renderGet(
          await new SdkDataPatternsService(await loadDlpClientOptions()).get(id),
          await resolveOutput(getCmd, opts),
        );
      } catch (err) {
        fail(err);
      }
    });

  writeFlags(group.command('replace <id>').description('Full-replace a data pattern (PUT)')).action(
    async (id, opts, command) => {
      try {
        const format = await resolveOutput(command, opts);
        const body = await resolveWriteBody(opts);
        dlpPatterns.renderReplaced(
          // biome-ignore lint/suspicious/noExplicitAny: body shape verified by SDK Zod
          await new SdkDataPatternsService(await loadDlpClientOptions()).replace(id, body as any),
          format,
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
    .action(async (id, opts, command) => {
      try {
        const format = await resolveOutput(command, opts);
        if (opts.bodyFile && (opts.set || opts.clear)) {
          throw new Error('--body-file is mutually exclusive with --set/--clear');
        }
        const body = opts.bodyFile
          ? await parseBody({ bodyFile: opts.bodyFile })
          : buildMergePatch({ set: opts.set, clear: opts.clear });
        dlpPatterns.renderPatched(
          // biome-ignore lint/suspicious/noExplicitAny: buildMergePatch returns Record<string, unknown>, cast for patch()
          await new SdkDataPatternsService(await loadDlpClientOptions()).patch(id, body as any),
          format,
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
        await new SdkDataPatternsService(await loadDlpClientOptions()).delete(id);
        dlpPatterns.renderArchived(id);
      } catch (err) {
        fail(err);
      }
    });
}
