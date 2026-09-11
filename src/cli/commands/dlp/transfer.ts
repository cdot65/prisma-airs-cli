import { randomUUID } from 'node:crypto';
import { readFile, stat } from 'node:fs/promises';
import { extname, resolve } from 'node:path';
import { AISecSDKException, ManagementClient } from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { dump, JSON_SCHEMA, load } from 'js-yaml';
import { ZodError } from 'zod';
import {
  backupDlpResources,
  DLP_RESOURCE_KINDS,
  type DlpProgress,
  type DlpResourceKind,
  DlpResourcesBackupSchema,
  type DlpTransferApi,
  planDlpResourcesRestore,
  restoreDlpResources,
} from '../../../backup/dlp-resources.js';
import { managementClientOptions } from '../../../config/client-options.js';
import { loadConfig } from '../../../config/loader.js';
import { writeReportFile } from '../../../reports/io.js';
import { confirmOrAbort } from '../../confirm.js';
import { examples } from '../../examples.js';
import {
  CliUsageError,
  formatOutput,
  resolveOutput,
  ui,
  usageError,
} from '../../renderer/index.js';

const MAX_BACKUP_BYTES = 20 * 1024 * 1024;
const FORMATS = 'Output format: pretty, table, markdown, csv, json, yaml';

function maxPages(value: string): number {
  if (!/^\d+$/.test(value) || Number(value) < 1 || Number(value) > 1000)
    usageError('--max-pages must be an integer from 1 to 1000');
  return Number(value);
}

function resourceKinds(value: string): DlpResourceKind[] {
  const kinds = value.split(',').map((kind) => kind.trim());
  if (
    !kinds.length ||
    kinds.some((kind) => !DLP_RESOURCE_KINDS.includes(kind as DlpResourceKind)) ||
    new Set(kinds).size !== kinds.length
  )
    throw new CliUsageError('--resources takes a unique list of dictionaries, patterns, profiles');
  return kinds as DlpResourceKind[];
}

function patternMap(values: string[]): Record<string, string> {
  const entries = values.map((value) => {
    const at = value.indexOf('=');
    if (at < 1 || at === value.length - 1)
      throw new CliUsageError('--pattern-map requires source-name=destination-name');
    return [value.slice(0, at), value.slice(at + 1)] as const;
  });
  if (new Set(entries.map(([key]) => key)).size !== entries.length)
    throw new CliUsageError('Duplicate source names in --pattern-map');
  return Object.fromEntries(entries);
}

async function context(): Promise<{ api: DlpTransferApi; tsgId: string }> {
  const config = await loadConfig();
  if (!config.mgmtTsgId || !config.mgmtClientId || !config.mgmtClientSecret)
    throw new CliUsageError(
      'DLP transfer requires configured Management client ID, secret and TSG ID',
    );
  const client = new ManagementClient({ ...managementClientOptions(config), numRetries: 0 });
  return {
    api: {
      dictionaries: client.dlp.dictionaries,
      patterns: client.dlp.dataPatterns,
      profiles: client.dlp.dataProfiles,
    },
    tsgId: config.mgmtTsgId,
  };
}

function failure(error: unknown): void {
  if (error instanceof CliUsageError) usageError(error.message);
  if (error instanceof ZodError) {
    ui.error('Invalid DLP backup/request schema; content is not logged.');
  } else if (error instanceof AISecSDKException) {
    ui.error(
      `DLP transfer API request failed${error.statusCode ? ` (HTTP ${error.statusCode})` : ''}; no automatic retry was made.`,
    );
  } else if ((error as NodeJS.ErrnoException)?.code) {
    ui.error(
      `DLP transfer file operation failed (${(error as NodeJS.ErrnoException).code}); existing files are not overwritten.`,
    );
  } else {
    ui.error(error instanceof Error ? error.message : 'DLP transfer failed');
  }
  process.exitCode = 1;
}

const STAGE_LABELS = {
  dictionaries: 'dictionaries',
  patterns: 'data patterns',
  profiles: 'data profiles',
} as const;
const ITEM_LABELS = {
  dictionaries: 'dictionary',
  patterns: 'data pattern',
  profiles: 'data profile',
} as const;

/** Pretty output narrates progress; machine formats keep stdout parseable. */
function progressRenderer(format: string): ((event: DlpProgress) => void) | undefined {
  if (format !== 'pretty') return undefined;
  return (event) => {
    if (event.phase === 'inventories')
      ui.info('Reading inventories (dictionaries, data patterns, data profiles)…');
    else if (event.phase === 'recheck') ui.info('Rechecking destination state before writing…');
    else if (event.phase === 'stage')
      ui.info(`Restoring ${STAGE_LABELS[event.stage]} (${event.total})…`);
    else
      ui.bullet(
        `${ITEM_LABELS[event.stage]} ${event.action}: ${event.name} (${event.index}/${event.total})`,
        event.action === 'skipped' ? 'skip' : 'success',
      );
  };
}

/** Human formats get one resource per row, never giant JSON arrays inside table cells. */
function renderRestore(
  value: Record<string, unknown>,
  columns: { key: string; label: string }[],
  format: Parameters<typeof formatOutput>[2],
): string {
  if (format !== 'pretty' && format !== 'table' && format !== 'markdown')
    return formatOutput([value], columns, format);
  const markdown = format === 'markdown';
  const rows = (key: string): Record<string, unknown>[] =>
    Array.isArray(value[key]) ? (value[key] as Record<string, unknown>[]) : [];
  const heading = (text: string) => (markdown ? `### ${text}\n` : text);
  const blocks = [
    heading(
      value.dryRun
        ? 'Restore preview — no changes made'
        : value.complete
          ? 'Restore complete'
          : 'Restore incomplete — completed writes remain',
    ),
    `Source TSG: ${value.sourceTsgId}\n${markdown ? '\n' : ''}Destination TSG: ${value.destinationTsgId}`,
  ];
  for (const [key, title] of [
    ['dictionaries', 'Dictionaries'],
    ['patterns', 'Data patterns'],
    ['profiles', 'Data profiles'],
  ] as const) {
    blocks.push(heading(`${title} (${rows(key).length})`));
    blocks.push(
      formatOutput(
        rows(key),
        [
          { key: 'action', label: 'Action' },
          { key: 'name', label: 'Name' },
        ],
        markdown ? 'markdown' : 'table',
      ) || '(none)',
    );
  }
  if (rows('unresolved').length) {
    blocks.push(heading('Unresolved references — dependent profiles skipped'));
    for (const entry of rows('unresolved'))
      blocks.push(`${entry.kind} ${entry.name}: ${entry.reason}`);
  }
  if (rows('serverAdded').length) {
    blocks.push(heading('Verified server-added fields (source omitted these)'));
    for (const item of rows('serverAdded'))
      blocks.push(
        `${item.resource}: ${Array.isArray(item.fields) ? item.fields.length : 0} fields; use --output json for paths`,
      );
  }
  if (value.error) blocks.push(`Error: ${value.error}`);
  return blocks.join('\n\n');
}

export function register(dlp: Command): void {
  const backup = dlp
    .command('backup')
    .description(
      'Back up custom DLP dictionaries (with keywords), data patterns and data profiles with their referenced dependencies',
    )
    .option(
      '--resources <list>',
      'Comma-separated primary resources: dictionaries, patterns, profiles (default: all)',
      resourceKinds,
    )
    .option('--file-format <format>', 'Backup file format: json or yaml', 'json')
    .option('--output-file <path>', 'New private backup file; defaults to the current directory')
    .option(
      '--skip-unsupported',
      'Exclude profiles with non-transferable rules (multi-profile, direct EDM) instead of failing',
    )
    .option(
      '--max-pages <n>',
      'Maximum pages per inventory; incomplete exports fail',
      maxPages,
      100,
    )
    .option('--output <format>', FORMATS)
    .addHelpText(
      'after',
      examples(
        'airs runtime dlp backup --output-file ./dlp-backup.json',
        'airs runtime dlp backup --resources profiles --skip-unsupported --output-file ./profiles.yaml --file-format yaml',
      ),
    )
    .action(async (opts) => {
      try {
        if (!['json', 'yaml'].includes(opts.fileFormat))
          throw new CliUsageError('--file-format must be json or yaml');
        const format = await resolveOutput(backup, opts);
        const extension = opts.fileFormat === 'yaml' ? '.yaml' : '.json';
        if (opts.outputFile === '-')
          throw new CliUsageError(
            'Backups contain dictionary keywords and require a private file destination, not stdout',
          );
        const path = resolve(
          opts.outputFile ??
            `airs-dlp-backup-${new Date().toISOString().replace(/[:.]/g, '-')}-${randomUUID().slice(0, 8)}${extension}`,
        );
        if (
          !(opts.fileFormat === 'json' ? ['.json'] : ['.yaml', '.yml']).includes(
            extname(path).toLowerCase(),
          )
        )
          throw new CliUsageError('Backup filename extension must match --file-format');
        const { api, tsgId } = await context();
        const { backup: data, skipped } = await backupDlpResources(api, tsgId, {
          resources: opts.resources,
          maxPages: opts.maxPages,
          skipUnsupported: Boolean(opts.skipUnsupported),
          onProgress: progressRenderer(format),
        });
        for (const item of skipped)
          ui.warning(`Skipped unsupported data profile: ${item.profile} (${item.reason})`);
        const content =
          opts.fileFormat === 'yaml'
            ? dump(data, { schema: JSON_SCHEMA, noRefs: true, lineWidth: -1 })
            : `${JSON.stringify(data, null, 2)}\n`;
        if (Buffer.byteLength(content) > MAX_BACKUP_BYTES)
          throw new Error('Backup exceeds the 20 MiB size limit; select fewer --resources');
        await writeReportFile(path, content);
        console.log(
          formatOutput(
            [
              {
                file: path,
                sourceTsgId: tsgId,
                dictionaries: data.dictionaries.length,
                patterns: data.patterns.length,
                profiles: data.profiles.length,
                skipped: skipped.length,
              },
            ],
            [
              { key: 'file', label: 'Backup file' },
              { key: 'sourceTsgId', label: 'Source TSG' },
              { key: 'dictionaries', label: 'Dictionaries' },
              { key: 'patterns', label: 'Patterns' },
              { key: 'profiles', label: 'Profiles' },
              { key: 'skipped', label: 'Skipped' },
            ],
            format === 'pretty' ? 'table' : format,
          ),
        );
      } catch (error) {
        failure(error);
      }
    });

  const restore = dlp
    .command('restore <file>')
    .description(
      'Restore DLP resources into the selected tenant, staged dictionaries → patterns → profiles; validate all dependencies before changes',
    )
    .option('--dry-run', 'Read and validate a restore plan without changing destination resources')
    .option('--name-prefix <prefix>', 'Prefix restored custom resource names to avoid collisions')
    .option(
      '--on-conflict <policy>',
      'Existing data profile names: error, verify (resume without writes) or skip. Updates are unsupported until the DLP profile write path is live-verified.',
      'error',
    )
    .option(
      '--pattern-map <source=destination>',
      'Bind a tenant-bound or predefined source pattern to an existing destination pattern (repeatable)',
      (value: string, prior: string[]) => [...prior, value],
      [],
    )
    .option(
      '--skip-unresolved',
      'Skip references with no destination match, and every profile depending on them; whole profiles only, each reported',
    )
    .option(
      '--max-pages <n>',
      'Maximum pages per inventory; incomplete inventories fail',
      maxPages,
      100,
    )
    .option('--expect-tsg <id>', 'Assert destination tenant ID; required with --force')
    .option(
      '--force',
      'Execute without a prompt; requires --expect-tsg and does not override conflicts',
    )
    .option('--output <format>', FORMATS)
    .addHelpText(
      'after',
      examples(
        'airs runtime dlp restore ./dlp-backup.json --dry-run --output json',
        'airs runtime dlp restore ./dlp-backup.json --pattern-map "EDM SSN=Dest SSN" --dry-run',
        'airs runtime dlp restore ./dlp-backup.json --name-prefix migrated- --expect-tsg <destination-tsg> --force',
      ),
    )
    .action(async (file: string, opts) => {
      try {
        if (!['error', 'verify', 'skip'].includes(opts.onConflict))
          throw new CliUsageError('--on-conflict must be error, verify or skip');
        if (opts.force && !opts.expectTsg)
          throw new CliUsageError('--force requires --expect-tsg <destination-tsg>');
        const mappings = patternMap(opts.patternMap);
        const path = resolve(file);
        const extension = extname(path).toLowerCase();
        if (!['.json', '.yaml', '.yml'].includes(extension))
          throw new CliUsageError('Backup file must use .json, .yaml or .yml');
        const info = await stat(path);
        if (!info.isFile() || info.size > MAX_BACKUP_BYTES)
          throw new CliUsageError('Backup must be a regular file no larger than 20 MiB');
        let input: unknown;
        try {
          const text = await readFile(path, 'utf8');
          if (Buffer.byteLength(text) > MAX_BACKUP_BYTES) throw new Error('Backup too large');
          input = extension === '.json' ? JSON.parse(text) : load(text, { schema: JSON_SCHEMA });
        } catch {
          throw new CliUsageError('Backup is not valid JSON/YAML; contents are not logged');
        }
        const data = DlpResourcesBackupSchema.parse(input);
        const format = await resolveOutput(restore, opts);
        const { api, tsgId } = await context();
        if (opts.expectTsg && opts.expectTsg !== tsgId)
          throw new CliUsageError(
            '--expect-tsg does not match the selected destination tenant; no API request was made',
          );
        const onProgress = progressRenderer(format);
        const plan = await planDlpResourcesRestore(api, data, tsgId, {
          namePrefix: opts.namePrefix,
          onConflict: opts.onConflict,
          patternMap: mappings,
          maxPages: opts.maxPages,
          skipUnresolved: Boolean(opts.skipUnresolved),
          onProgress,
        });
        for (const entry of plan.unresolved)
          ui.warning(
            `Unresolved ${entry.kind === 'dictionary' ? 'dictionary' : 'data pattern'}: ${entry.name} (${entry.reason})${
              entry.candidates?.length
                ? `; candidates: ${entry.candidates.map((name) => `"${name}"`).join(', ')}`
                : ''
            }`,
          );
        for (const profile of plan.profiles)
          if (profile.reason)
            ui.warning(`Skipping data profile: ${profile.name} (${profile.reason})`);
        const summary = {
          sourceTsgId: plan.sourceTsgId,
          destinationTsgId: plan.destinationTsgId,
          dryRun: Boolean(opts.dryRun),
          dictionaries: plan.dictionaries.map((d) => ({ name: d.name, action: d.action })),
          patterns: plan.patterns.map((p) => ({ name: p.name, action: p.action })),
          profiles: plan.profiles.map((p) => ({
            name: p.name,
            action: p.action,
            ...(p.reason ? { reason: p.reason } : {}),
          })),
          unresolved: plan.unresolved.map(({ kind, name, reason }) => ({ kind, name, reason })),
        };
        const columns = [
          { key: 'sourceTsgId', label: 'Source TSG' },
          { key: 'destinationTsgId', label: 'Destination TSG' },
          { key: 'dictionaries', label: 'Dictionaries' },
          { key: 'patterns', label: 'Data patterns' },
          { key: 'profiles', label: 'Data profiles' },
          { key: 'unresolved', label: 'Unresolved references' },
          { key: 'serverAdded', label: 'Verified server-added fields' },
          { key: 'dryRun', label: 'Dry run' },
          { key: 'complete', label: 'Complete' },
        ];
        if (opts.dryRun) {
          console.log(renderRestore(summary, columns, format));
          return;
        }
        const writes =
          plan.dictionaries.filter((d) => d.action === 'create').length +
          plan.patterns.filter((p) => p.action === 'create').length +
          plan.profiles.filter((p) => p.action === 'create').length;
        if (writes)
          await confirmOrAbort(
            `Restore ${writes} DLP resources from TSG ${plan.sourceTsgId} to TSG ${tsgId}?`,
            Boolean(opts.force),
            { action: `restore DLP resources into TSG ${tsgId}` },
          );
        const result = await restoreDlpResources(api, plan, { onProgress });
        console.log(renderRestore({ ...result }, columns, format));
        if (!result.complete) {
          ui.error(result.error ?? 'Restore incomplete');
          process.exitCode = 1;
        }
      } catch (error) {
        failure(error);
      }
    });
}
