import { readFile } from 'node:fs/promises';
import { basename } from 'node:path';
import { AISecSDKException, ErrorType } from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { SdkDictionariesService } from '../../../airs/dlp/dictionaries.js';
import type { DictionaryRequest } from '../../../airs/dlp/types.js';
import { registerPageAliases, resolvePageParams } from '../../pagination.js';
import { CliUsageError, dlpDictionaries, fail, resolveOutput } from '../../renderer/index.js';
import { loadDlpClientOptions } from './config.js';
import { buildMergePatch } from './patch.js';
import { predefinedFlag, visibleRecords } from './visibility.js';

async function readMetadata(path: string): Promise<Record<string, unknown>> {
  const raw = await readFile(path, 'utf-8');
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    // JSON.parse errors can quote keyword or other sensitive file contents.
    throw new CliUsageError('Invalid JSON in dictionary metadata file');
  }
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new CliUsageError('Dictionary metadata must be a JSON object');
  }
  return value as Record<string, unknown>;
}

function dictionaryFailure(err: unknown): never {
  if (err instanceof AISecSDKException && err.errorType === ErrorType.USER_REQUEST_PAYLOAD_ERROR) {
    // SDK request validation happens before HTTP; do not print values supplied in metadata.
    fail(new CliUsageError('Invalid dictionary metadata: verify required fields and value types'));
  }
  fail(err);
}

function uploadFailure(err: unknown): never {
  const error = err as {
    status?: number;
    statusCode?: number;
    problem?: { detail?: string; errors?: unknown[] };
  } | null;
  if (
    (error?.status ?? error?.statusCode) === 400 &&
    !error?.problem?.detail &&
    !error?.problem?.errors?.length
  ) {
    console.error(
      'Dictionary upload rejected. Verify the SCM region display name (for example, --region "United States"); region codes such as GLOBAL or us-west-2 are not equivalent.',
    );
  }
  dictionaryFailure(err);
}

// biome-ignore lint/suspicious/noExplicitAny: opts object from commander
async function buildMetadata(opts: any): Promise<DictionaryRequest> {
  if (opts.metadataFile) {
    return (await readMetadata(opts.metadataFile)) as DictionaryRequest;
  }
  if (!opts.name || !opts.category || !opts.region || !opts.file) {
    throw new CliUsageError('--name, --category, --region, and --file are required');
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
  predefinedFlag(listCmd);
  registerPageAliases(listCmd, { sizeFlag: '--size', sizeKey: 'size' });
  listCmd.action(async (opts) => {
    try {
      const { page, size } = resolvePageParams(listCmd, opts);
      const includeKeywords = opts.keywords || opts.includeKeywords;
      const svc = new SdkDictionariesService(await loadDlpClientOptions());
      const params = {
        size,
        sort: opts.sort ?? ['name,asc'],
        keywords: includeKeywords ? true : undefined,
      };
      const all = opts.all
        ? await svc.listAll({ ...params, max: Number(opts.max) })
        : (await svc.list({ ...params, page })).content;
      const visible = visibleRecords(all, opts.includePredefined);
      dlpDictionaries.renderList(
        { content: visible, totalElements: visible.length },
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
    .option('--region <s>', 'SCM region display name, e.g. "United States"')
    .option('--description <s>', '')
    .option('--classification <s>', 'Legacy top-level metadata field (server support unverified)')
    .option('--file <path>', 'Keyword file: TXT (one keyword per line), or CSV (header required)')
    .option('--metadata-file <path>', 'JSON metadata file (overrides --name/--category/...)')
    .option('--include-keywords', 'Include keywords in response')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (opts, command) => {
      try {
        const format = await resolveOutput(command, opts);
        const metadata = await buildMetadata(opts);
        if (!opts.file) throw new CliUsageError('--file is required (multipart upload)');
        const file = await readFile(opts.file);
        const r = await new SdkDictionariesService(await loadDlpClientOptions()).create({
          metadata,
          file,
          includeKeywords: opts.includeKeywords,
        });
        dlpDictionaries.renderCreated(r, format);
      } catch (err) {
        uploadFailure(err);
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
          await new SdkDictionariesService(await loadDlpClientOptions()).get(id, {
            includeKeywords,
          }),
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
    .option('--region <s>', 'SCM region display name, e.g. "United States"')
    .option('--description <s>', '')
    .option('--classification <s>', 'Legacy top-level metadata field (server support unverified)')
    .option('--file <path>', 'Required: TXT (one keyword per line), or CSV (header required)')
    .option('--metadata-file <path>', 'JSON metadata file')
    .option('--include-keywords', '')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (id, opts, command) => {
      try {
        const metadata = await buildMetadata(opts);
        const format = await resolveOutput(command, opts);
        if (!opts.file) throw new CliUsageError('--file is required (multipart upload)');
        const file = await readFile(opts.file);
        const r = await new SdkDictionariesService(await loadDlpClientOptions()).replace(id, {
          metadata,
          file,
          includeKeywords: opts.includeKeywords,
        });
        if ('kind' in r && r.kind === 'fallback') {
          dlpDictionaries.renderReplaced204Fallback(id);
        } else {
          dlpDictionaries.renderReplaced(r, format);
        }
      } catch (err) {
        uploadFailure(err);
      }
    });

  group
    .command('patch <id>')
    .option('--body-file <path>', '')
    .option('--set <k=v...>', '(repeatable)', (v, p: string[] = []) => [...p, v])
    .option('--clear <key...>', '(repeatable)', (v, p: string[] = []) => [...p, v])
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (id, opts, command) => {
      try {
        const format = await resolveOutput(command, opts);
        if (opts.bodyFile && (opts.set || opts.clear)) {
          throw new CliUsageError('--body-file is mutually exclusive with --set/--clear');
        }
        let body: Record<string, unknown>;
        if (opts.bodyFile) {
          body = await readMetadata(opts.bodyFile);
        } else {
          try {
            body = buildMergePatch({ set: opts.set, clear: opts.clear });
          } catch {
            throw new CliUsageError(
              'Invalid dictionary patch flags: use --set key=value, --clear key, or --body-file for nested fields',
            );
          }
        }
        dlpDictionaries.renderPatched(
          // biome-ignore lint/suspicious/noExplicitAny: buildMergePatch returns Record<string, unknown>, cast for patch()
          await new SdkDictionariesService(await loadDlpClientOptions()).patch(id, body as any),
          format,
        );
      } catch (err) {
        dictionaryFailure(err);
      }
    });

  group
    .command('delete <id>')
    .description('Delete a dictionary')
    .action(async (id) => {
      try {
        await new SdkDictionariesService(await loadDlpClientOptions()).delete(id);
        dlpDictionaries.renderDeleted(id);
      } catch (err) {
        fail(err);
      }
    });
}
