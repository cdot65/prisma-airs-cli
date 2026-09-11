import { AISecSDKException, ErrorType } from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { SdkDataPatternsService } from '../../../airs/dlp/data-patterns.js';
import { SdkDataProfilesService } from '../../../airs/dlp/data-profiles.js';
import { registerPageAliases, resolvePageParams } from '../../pagination.js';
import {
  CliUsageError,
  dlpProfiles,
  fail,
  resolveOutput,
  usageError,
} from '../../renderer/index.js';
import {
  buildProfileBody,
  type ResolvedProfilePattern,
  repeatable,
  validateProfileFlags,
  validateProfileName,
  validateProfileType,
} from './build-body.js';
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
    .option('--name <s>', 'Profile name (required unless --body-file)')
    .option('--profile-type <s>', 'Profile type: advanced (basic writes are unsupported)')
    .option('--description <s>', 'Description')
    .option('--granular', 'Granular data profile')
    .option(
      '--pattern-id <id>',
      'Data pattern ID to include (repeatable). Builds a simple expression_tree.',
      repeatable,
    )
    .option(
      '--combinator <op>',
      'Combinator for --pattern-id: or|and|not|and_not|or_not (default: or)',
    )
    .option('--confidence <level>', 'Confidence level for --pattern-id leaves (default: high)')
    .option('--body <json|->', 'Raw JSON body (escape hatch; or "-" for stdin)')
    .option(
      '--body-file <path>',
      'Raw JSON body file (escape hatch; required for complex rule trees)',
    )
    .option('--output <fmt>', 'Output format', 'pretty');
}

function validateBodyType(body: unknown): void {
  if (!body || typeof body !== 'object' || Array.isArray(body)) {
    throw new CliUsageError('Profile body must be a JSON object');
  }
  try {
    validateProfileName((body as Record<string, unknown>).name);
    validateProfileType((body as Record<string, unknown>).profile_type);
  } catch (error) {
    throw new CliUsageError((error as Error).message);
  }
}

function profileFailure(error: unknown): never {
  if (
    error instanceof AISecSDKException &&
    error.errorType === ErrorType.USER_REQUEST_PAYLOAD_ERROR
  ) {
    fail(new CliUsageError('Invalid profile metadata: verify required fields and value types'));
  }
  fail(error);
}

async function resolveWriteBody(opts: Record<string, unknown>): Promise<unknown> {
  try {
    validateProfileType(opts.profileType);
  } catch (error) {
    throw new CliUsageError((error as Error).message);
  }
  if (opts.body !== undefined || opts.bodyFile !== undefined) {
    if (opts.body !== undefined && opts.bodyFile !== undefined) {
      throw new CliUsageError('--body and --body-file are mutually exclusive');
    }
    let body: unknown;
    try {
      body = await parseBody({ body: opts.body as string, bodyFile: opts.bodyFile as string });
    } catch {
      throw new CliUsageError('Cannot read profile body as JSON; verify the file and JSON syntax');
    }
    validateBodyType(body);
    return body;
  }
  try {
    validateProfileFlags(opts);
  } catch (error) {
    throw new CliUsageError((error as Error).message);
  }
  const patterns: ResolvedProfilePattern[] = [];
  const ids = opts.patternId as string[] | undefined;
  if (ids?.length) {
    const svc = new SdkDataPatternsService(await loadDlpClientOptions());
    for (const id of new Set(ids)) {
      const pattern = await svc.get(id);
      if (
        pattern.id !== id ||
        !pattern.name ||
        !pattern.detection_config?.technique ||
        !Number.isInteger(pattern.version) ||
        Number(pattern.version) < 1
      ) {
        throw new Error(
          'Referenced pattern did not return a matching ID, name, technique, and version; no profile was written',
        );
      }
      if (pattern.status !== 'active') {
        throw new Error('Referenced pattern is not active; no profile was written');
      }
      patterns.push({
        id: pattern.id,
        name: pattern.name,
        version: pattern.version as number,
        technique: pattern.detection_config.technique,
        confidenceLevels: pattern.detection_config.supported_confidence_levels,
      });
    }
  }
  try {
    return buildProfileBody(opts, patterns);
  } catch (error) {
    throw new CliUsageError((error as Error).message);
  }
}

export function register(dlp: Command): void {
  const group = dlp
    .command('profiles')
    .description(
      'DLP data profiles. No supported DELETE; status-based retirement is not live-verified.',
    );

  const listCmd = predefinedFlag(
    listFlags(group.command('list').description('List data profiles (tenant-created by default)')),
  );
  listCmd.action(async (opts) => {
    try {
      const { page, size } = resolvePageParams(listCmd, opts);
      const svc = new SdkDataProfilesService(await loadDlpClientOptions());
      const result = opts.all
        ? await svc.listAll({ size, sort: opts.sort ?? ['name,asc'], max: Number(opts.max) })
        : (await svc.list({ page, size, sort: opts.sort ?? ['name,asc'] })).content;
      const visible = visibleRecords(result, opts.includePredefined);
      dlpProfiles.renderList(
        { content: visible, totalElements: visible.length },
        await resolveOutput(listCmd, opts),
      );
    } catch (err) {
      fail(err);
    }
  });

  writeFlags(group.command('create').description('Create a data profile')).action(
    async (opts, command) => {
      try {
        const format = await resolveOutput(command, opts);
        const body = await resolveWriteBody(opts);
        dlpProfiles.renderCreated(
          // biome-ignore lint/suspicious/noExplicitAny: body shape verified by SDK Zod
          await new SdkDataProfilesService(await loadDlpClientOptions()).create(body as any),
          format,
        );
      } catch (err) {
        profileFailure(err);
      }
    },
  );

  const getCmd = group
    .command('get <id>')
    .description('Get a data profile by id')
    .option('--output <fmt>', 'Output format: pretty, table, markdown, csv, json, yaml')
    .action(async (id, opts) => {
      try {
        dlpProfiles.renderGet(
          await new SdkDataProfilesService(await loadDlpClientOptions()).get(id),
          await resolveOutput(getCmd, opts),
        );
      } catch (err) {
        fail(err);
      }
    });

  writeFlags(group.command('replace <id>').description('Full-replace a data profile (PUT)')).action(
    async (id, opts, command) => {
      try {
        const format = await resolveOutput(command, opts);
        const body = await resolveWriteBody(opts);
        dlpProfiles.renderReplaced(
          // biome-ignore lint/suspicious/noExplicitAny: body shape verified by SDK Zod
          await new SdkDataProfilesService(await loadDlpClientOptions()).replace(id, body as any),
          format,
        );
      } catch (err) {
        profileFailure(err);
      }
    },
  );

  group
    .command('patch <id>')
    .description(
      'JSON Merge Patch. body must include name + profile_type. Use --body-file for nested fields. ' +
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
          throw new CliUsageError('--body-file is mutually exclusive with --set/--clear');
        }
        let body: unknown;
        try {
          body = opts.bodyFile
            ? await parseBody({ bodyFile: opts.bodyFile })
            : buildMergePatch({ set: opts.set, clear: opts.clear });
        } catch {
          throw new CliUsageError(
            'Invalid profile patch: verify JSON syntax and --set key=value/--clear key flags',
          );
        }
        validateBodyType(body);
        dlpProfiles.renderPatched(
          // biome-ignore lint/suspicious/noExplicitAny: buildMergePatch returns Record<string, unknown>, cast for patch()
          await new SdkDataProfilesService(await loadDlpClientOptions()).patch(id, body as any),
          format,
        );
      } catch (err) {
        profileFailure(err);
      }
    });

  group
    .command('delete <id>')
    .description('Not supported — explains the cleanup limitation and exits 2')
    .action((id) => {
      usageError(
        `This DLP API has no DELETE for data profiles.\n` +
          `  Status-based retirement is not live-verified: the latest owned-fixture\n` +
          `  PATCH/PUT returned HTTP 500; an advertised DELETE returned HTTP 501.\n` +
          `  No API request was sent by this command. Do not assume the profile was removed.\n` +
          `  Inspect current state with: airs runtime dlp profiles get ${id} --output json`,
      );
    });
}
