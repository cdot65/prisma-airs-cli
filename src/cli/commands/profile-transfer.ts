import { randomUUID } from 'node:crypto';
import { readFile, stat } from 'node:fs/promises';
import { extname, resolve } from 'node:path';
import { AISecSDKException, ManagementClient } from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { dump, JSON_SCHEMA, load } from 'js-yaml';
import { ZodError } from 'zod';
import {
  backupRuntimeProfiles,
  type ProfileTransferApi,
  planRuntimeProfilesRestore,
  RuntimeProfilesBackupSchema,
  restoreRuntimeProfiles,
} from '../../backup/runtime-profiles.js';
import { managementClientOptions } from '../../config/client-options.js';
import { loadConfig } from '../../config/loader.js';
import { writeReportFile } from '../../reports/io.js';
import { confirmOrAbort } from '../confirm.js';
import { examples } from '../examples.js';
import { CliUsageError, formatOutput, resolveOutput, ui, usageError } from '../renderer/index.js';

const MAX_BACKUP_BYTES = 20 * 1024 * 1024;
const FORMATS = 'Output format: pretty, table, markdown, csv, json, yaml';

function maxPages(value: string): number {
  if (!/^\d+$/.test(value) || Number(value) < 1 || Number(value) > 1000)
    usageError('--max-pages must be an integer from 1 to 1000');
  return Number(value);
}

function dlpMap(values: string[]): Record<string, string> {
  const entries = values.map((value) => {
    const at = value.indexOf('=');
    if (at < 1 || at === value.length - 1)
      throw new CliUsageError('--dlp-map requires source-name=destination-name');
    return [value.slice(0, at), value.slice(at + 1)] as const;
  });
  if (new Set(entries.map(([key]) => key)).size !== entries.length)
    throw new CliUsageError('Duplicate source names in --dlp-map');
  return Object.fromEntries(entries);
}

async function context(): Promise<{ api: ProfileTransferApi; tsgId: string }> {
  const config = await loadConfig();
  if (!config.mgmtTsgId || !config.mgmtClientId || !config.mgmtClientSecret)
    throw new CliUsageError(
      'Profile transfer requires configured Management client ID, secret and TSG ID',
    );
  const client = new ManagementClient({ ...managementClientOptions(config), numRetries: 0 });
  return {
    api: {
      profiles: client.profiles,
      topics: client.topics,
      dataProfiles: client.dlp.dataProfiles,
    },
    tsgId: config.mgmtTsgId,
  };
}

function failure(error: unknown): void {
  if (error instanceof CliUsageError) usageError(error.message);
  if (error instanceof ZodError) {
    ui.error('Invalid profile backup/request schema; content is not logged.');
  } else if (error instanceof AISecSDKException) {
    ui.error(
      `Profile transfer API request failed${error.statusCode ? ` (HTTP ${error.statusCode})` : ''}; no automatic retry was made.`,
    );
  } else if ((error as NodeJS.ErrnoException)?.code) {
    ui.error(
      `Profile transfer file operation failed (${(error as NodeJS.ErrnoException).code}); existing files are not overwritten.`,
    );
  } else {
    ui.error(error instanceof Error ? error.message : 'Profile transfer failed');
  }
  process.exitCode = 1;
}

export function registerProfileTransferCommands(profiles: Command): void {
  const backup = profiles
    .command('backup [profile]')
    .description(
      'Back up latest Runtime profile policies and referenced topic definitions; no argument means all profiles',
    )
    .option('--all', 'Back up all latest profiles (cannot be combined with a profile selector)')
    .option('--file-format <format>', 'Backup file format: json or yaml', 'json')
    .option('--output-file <path>', 'New private backup file; defaults to the current directory')
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
        'airs runtime profiles backup --all --output-file ./profiles.json',
        'airs runtime profiles backup "Production" --file-format yaml --output-file ./production.yaml',
      ),
    )
    .action(async (profile: string | undefined, opts) => {
      try {
        if (opts.all && profile)
          throw new CliUsageError('Choose either a profile selector or --all');
        if (!['json', 'yaml'].includes(opts.fileFormat))
          throw new CliUsageError('--file-format must be json or yaml');
        const format = await resolveOutput(backup, opts);
        const extension = opts.fileFormat === 'yaml' ? '.yaml' : '.json';
        if (opts.outputFile === '-')
          throw new CliUsageError('Backups require a private file destination, not stdout');
        const path = resolve(
          opts.outputFile ??
            `airs-runtime-profiles-backup-${new Date().toISOString().replace(/[:.]/g, '-')}-${randomUUID().slice(0, 8)}${extension}`,
        );
        if (
          !(opts.fileFormat === 'json' ? ['.json'] : ['.yaml', '.yml']).includes(
            extname(path).toLowerCase(),
          )
        )
          throw new CliUsageError('Backup filename extension must match --file-format');
        const { api, tsgId } = await context();
        const data = await backupRuntimeProfiles(api, tsgId, { profile, maxPages: opts.maxPages });
        const content =
          opts.fileFormat === 'yaml'
            ? dump(data, { schema: JSON_SCHEMA, noRefs: true, lineWidth: -1 })
            : `${JSON.stringify(data, null, 2)}\n`;
        if (Buffer.byteLength(content) > MAX_BACKUP_BYTES)
          throw new Error('Backup exceeds the 20 MiB size limit; select individual profiles');
        await writeReportFile(path, content);
        console.log(
          formatOutput(
            [
              {
                file: path,
                sourceTsgId: tsgId,
                profiles: data.profiles.length,
                topics: data.topics.length,
              },
            ],
            [
              { key: 'file', label: 'Backup file' },
              { key: 'sourceTsgId', label: 'Source TSG' },
              { key: 'profiles', label: 'Profiles' },
              { key: 'topics', label: 'Topics' },
            ],
            format === 'pretty' ? 'table' : format,
          ),
        );
      } catch (error) {
        failure(error);
      }
    });

  const restore = profiles
    .command('restore <file>')
    .description(
      'Restore profile policies into the selected tenant; validate all dependencies before changes',
    )
    .option('--dry-run', 'Read and validate a restore plan without changing destination resources')
    .option('--name-prefix <prefix>', 'Prefix restored profile and topic names to avoid collisions')
    .option('--on-conflict <policy>', 'Existing profile names: error, skip or update', 'error')
    .option(
      '--dlp-map <source=destination>',
      'Explicit cross-tenant binding to an existing DLP data profile (repeatable)',
      (value: string, prior: string[]) => [...prior, value],
      [],
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
        'airs runtime profiles restore ./profiles.json --dry-run --output json',
        'airs runtime profiles restore ./profiles.json --name-prefix migrated- --expect-tsg <destination-tsg> --force',
      ),
    )
    .action(async (file: string, opts) => {
      try {
        if (!['error', 'skip', 'update'].includes(opts.onConflict))
          throw new CliUsageError('--on-conflict must be error, skip or update');
        if (opts.force && !opts.expectTsg)
          throw new CliUsageError('--force requires --expect-tsg <destination-tsg>');
        const mappings = dlpMap(opts.dlpMap);
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
        const data = RuntimeProfilesBackupSchema.parse(input);
        const format = await resolveOutput(restore, opts);
        const { api, tsgId } = await context();
        if (opts.expectTsg && opts.expectTsg !== tsgId)
          throw new CliUsageError(
            '--expect-tsg does not match the selected destination tenant; no API request was made',
          );
        const plan = await planRuntimeProfilesRestore(api, data, tsgId, {
          namePrefix: opts.namePrefix,
          onConflict: opts.onConflict,
          maxPages: opts.maxPages,
          dlpMap: mappings,
        });
        const summary = {
          sourceTsgId: plan.sourceTsgId,
          destinationTsgId: plan.destinationTsgId,
          dryRun: Boolean(opts.dryRun),
          profiles: plan.profiles.map((p) => ({ name: p.name, action: p.action })),
          topics: plan.topics.map((t) => ({
            name: t.name,
            action: t.existing ? 'reuse' : 'create',
          })),
          dlpMappings: plan.dlpMappings.map((m) => ({
            source: m.source,
            destination: m.destination,
          })),
        };
        const columns = [
          { key: 'sourceTsgId', label: 'Source TSG' },
          { key: 'destinationTsgId', label: 'Destination TSG' },
          { key: 'profiles', label: 'Profiles' },
          { key: 'topics', label: 'Topics' },
          { key: 'dlpMappings', label: 'DLP mappings' },
          { key: 'dryRun', label: 'Dry run' },
          { key: 'complete', label: 'Complete' },
        ];
        if (opts.dryRun) {
          console.log(formatOutput([summary], columns, format === 'pretty' ? 'table' : format));
          return;
        }
        const changing = plan.profiles.filter((p) => p.action !== 'skip');
        if (changing.length)
          await confirmOrAbort(
            `Restore ${changing.length} profiles from TSG ${plan.sourceTsgId} to TSG ${tsgId} (${changing.filter((p) => p.action === 'update').length} updates)?`,
            Boolean(opts.force),
            { action: `restore profiles into TSG ${tsgId}` },
          );
        const result = await restoreRuntimeProfiles(api, plan);
        console.log(formatOutput([{ ...result }], columns, format === 'pretty' ? 'table' : format));
        if (!result.complete) {
          ui.error(result.error ?? 'Restore incomplete');
          process.exitCode = 1;
        }
      } catch (error) {
        failure(error);
      }
    });
}
