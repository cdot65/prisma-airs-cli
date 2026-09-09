import type { Command } from 'commander';
import { SdkDataProfilesService } from '../../../airs/dlp/data-profiles.js';
import { registerPageAliases, resolvePageParams } from '../../pagination.js';
import { dlpProfiles, fail, resolveOutput, usageError } from '../../renderer/index.js';
import { buildProfileBody, repeatable } from './build-body.js';
import { loadDlpClientOptions } from './config.js';
import { buildMergePatch, parseBody } from './patch.js';

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
    .option('--profile-type <s>', 'Profile type: basic|advanced (default: advanced)')
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

async function resolveWriteBody(opts: Record<string, unknown>): Promise<unknown> {
  if (opts.body || opts.bodyFile) {
    const body = await parseBody({
      body: opts.body as string,
      bodyFile: opts.bodyFile as string,
    });
    if (!body) throw new Error('--body or --body-file was empty');
    return body;
  }
  return buildProfileBody(opts);
}

export function register(dlp: Command): void {
  const group = dlp
    .command('profiles')
    .description(
      'DLP data profiles. No supported DELETE; status-based retirement is not live-verified.',
    );

  const listCmd = listFlags(group.command('list').description('List data profiles'));
  listCmd.action(async (opts) => {
    try {
      const { page, size } = resolvePageParams(listCmd, opts);
      const svc = new SdkDataProfilesService(await loadDlpClientOptions());
      const result = opts.all
        ? await svc.listAll({ size, sort: opts.sort, max: Number(opts.max) })
        : undefined;
      dlpProfiles.renderList(
        result
          ? { content: result, totalElements: result.length }
          : await svc.list({ page, size, sort: opts.sort }),
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
        usageError(err instanceof Error ? err.message : String(err));
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
        usageError(err instanceof Error ? err.message : String(err));
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
          throw new Error('--body-file is mutually exclusive with --set/--clear');
        }
        const body = opts.bodyFile
          ? await parseBody({ bodyFile: opts.bodyFile })
          : buildMergePatch({ set: opts.set, clear: opts.clear });
        dlpProfiles.renderPatched(
          // biome-ignore lint/suspicious/noExplicitAny: buildMergePatch returns Record<string, unknown>, cast for patch()
          await new SdkDataProfilesService(await loadDlpClientOptions()).patch(id, body as any),
          format,
        );
      } catch (err) {
        usageError(err instanceof Error ? err.message : String(err));
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
