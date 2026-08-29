import { readFile } from 'node:fs/promises';
import { basename } from 'node:path';
import type { Command } from 'commander';
import { SdkDictionariesService } from '../../../airs/dlp/dictionaries.js';
import type { DictionaryRequest } from '../../../airs/dlp/types.js';
import { registerPageAliases, resolvePageParams } from '../../pagination.js';
import {
  dlpDictionaries,
  fail,
  type OutputFormat,
  resolveOutput,
  usageError,
} from '../../renderer/index.js';
import { buildMergePatch, parseBody } from './patch.js';

// biome-ignore lint/suspicious/noExplicitAny: opts object from commander
async function buildMetadata(opts: any): Promise<DictionaryRequest> {
  if (opts.metadataFile) {
    return JSON.parse(await readFile(opts.metadataFile, 'utf-8'));
  }
  if (!opts.name || !opts.category || !opts.region || !opts.file) {
    throw new Error('--name, --category, --region, and --file are required');
  }
  return {
    name: opts.name,
    category: opts.category,
    region_name: opts.region,
    original_file_name: basename(opts.file),
    description: opts.description,
    classification: opts.classification,
  } as DictionaryRequest;
}

export function register(dlp: Command): void {
  const group = dlp.command('dictionaries').description('DLP dictionaries (multipart upload)');

  const listCmd = group
    .command('list')
    .description('List dictionaries')
    .option('--limit <n>', 'Max results per page (API page size)', (v) => Number.parseInt(v, 10))
    .option('--offset <n>', 'Starting offset — rounds down to a page boundary', (v) =>
      Number.parseInt(v, 10),
    )
    .option('--sort <field,dir>', '(repeatable)', (v, p: string[] = []) => [...p, v])
    .option('--keywords', 'Include keyword list in response')
    .option('--include-keywords', 'Alias for --keywords')
    .option('--output <fmt>', 'Output format: pretty, table, markdown, csv, json, yaml');
  registerPageAliases(listCmd, { sizeFlag: '--size', sizeKey: 'size' });
  listCmd.action(async (opts) => {
    try {
      const { page, size } = resolvePageParams(listCmd, opts);
      const includeKeywords = opts.keywords || opts.includeKeywords;
      const svc = new SdkDictionariesService();
      const params = {
        size,
        sort: opts.sort,
        keywords: includeKeywords ? true : undefined,
      };
      const all = opts.all ? await svc.listAll({ ...params, max: Number(opts.max) }) : undefined;
      dlpDictionaries.renderList(
        all ? { content: all, totalElements: all.length } : await svc.list({ ...params, page }),
        await resolveOutput(listCmd, opts),
      );
    } catch (err) {
      fail(err);
    }
  });

  group
    .command('create')
    .description('Create dictionary via multipart upload')
    .option('--name <s>', '')
    .option('--category <s>', '')
    .option('--region <s>', '')
    .option('--description <s>', '')
    .option('--classification <s>', '')
    .option('--file <path>', 'Keyword file')
    .option('--metadata-file <path>', 'JSON metadata file (overrides --name/--category/...)')
    .option('--include-keywords', 'Include keywords in response')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (opts) => {
      try {
        const metadata = await buildMetadata(opts);
        if (!opts.file) throw new Error('--file is required (multipart upload)');
        const file = await readFile(opts.file);
        const r = await new SdkDictionariesService().create({
          metadata,
          file,
          includeKeywords: opts.includeKeywords,
        });
        dlpDictionaries.renderCreated(r, opts.output as OutputFormat);
      } catch (err) {
        usageError(err instanceof Error ? err.message : String(err));
      }
    });

  const getCmd = group
    .command('get <id>')
    .option('--keywords', '')
    .option('--include-keywords', 'Alias for --keywords')
    .option('--output <fmt>', 'Output format: pretty, table, markdown, csv, json, yaml')
    .action(async (id, opts) => {
      try {
        const includeKeywords = opts.keywords || opts.includeKeywords;
        dlpDictionaries.renderGet(
          await new SdkDictionariesService().get(id, { includeKeywords }),
          await resolveOutput(getCmd, opts),
        );
      } catch (err) {
        fail(err);
      }
    });

  group
    .command('replace <id>')
    .description(
      'Full-replace via multipart upload. --file required. May return 200 (body) ' +
        'or 204 (re-get; falls back to "(state not echoed)" on transient failure).',
    )
    .option('--name <s>', '')
    .option('--category <s>', '')
    .option('--region <s>', '')
    .option('--description <s>', '')
    .option('--classification <s>', '')
    .option('--file <path>', 'Keyword file (required)')
    .option('--metadata-file <path>', 'JSON metadata file')
    .option('--include-keywords', '')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (id, opts) => {
      try {
        const metadata = await buildMetadata(opts);
        if (!opts.file) throw new Error('--file is required (multipart upload)');
        const file = await readFile(opts.file);
        const r = await new SdkDictionariesService().replace(id, {
          metadata,
          file,
          includeKeywords: opts.includeKeywords,
        });
        if ('kind' in r && r.kind === 'fallback') {
          dlpDictionaries.renderReplaced204Fallback(id);
        } else {
          dlpDictionaries.renderReplaced(r, opts.output as OutputFormat);
        }
      } catch (err) {
        usageError(err instanceof Error ? err.message : String(err));
      }
    });

  group
    .command('patch <id>')
    .option('--body-file <path>', '')
    .option('--set <k=v...>', '(repeatable)', (v, p: string[] = []) => [...p, v])
    .option('--clear <key...>', '(repeatable)', (v, p: string[] = []) => [...p, v])
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (id, opts) => {
      try {
        if (opts.bodyFile && (opts.set || opts.clear)) {
          throw new Error('--body-file is mutually exclusive with --set/--clear');
        }
        const body = opts.bodyFile
          ? await parseBody({ bodyFile: opts.bodyFile })
          : buildMergePatch({ set: opts.set, clear: opts.clear });
        dlpDictionaries.renderPatched(
          // biome-ignore lint/suspicious/noExplicitAny: buildMergePatch returns Record<string, unknown>, cast for patch()
          await new SdkDictionariesService().patch(id, body as any),
          opts.output as OutputFormat,
        );
      } catch (err) {
        usageError(err instanceof Error ? err.message : String(err));
      }
    });

  group
    .command('delete <id>')
    .description('Delete a dictionary')
    .action(async (id) => {
      try {
        await new SdkDictionariesService().delete(id);
        dlpDictionaries.renderDeleted(id);
      } catch (err) {
        fail(err);
      }
    });
}
