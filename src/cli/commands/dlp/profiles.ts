import type { Command } from 'commander';
import { SdkDataProfilesService } from '../../../airs/dlp/data-profiles.js';
import { dlpProfiles, type OutputFormat, renderError } from '../../renderer/index.js';
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
  const group = dlp
    .command('profiles')
    .description(
      'DLP data profiles. DELETE is not exposed by the DLP API. To remove a profile, patch with profile_status: "deleted".',
    );

  listFlags(group.command('list').description('List data profiles')).action(async (opts) => {
    try {
      const svc = new SdkDataProfilesService();
      dlpProfiles.renderList(
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
    .description('Create a data profile')
    .option('--body <json|->', 'JSON body (or "-" for stdin)')
    .option('--body-file <path>', 'Path to JSON body file')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (opts) => {
      try {
        const body = await parseBody({ body: opts.body, bodyFile: opts.bodyFile });
        if (!body) throw new Error('--body or --body-file is required');
        const svc = new SdkDataProfilesService();
        dlpProfiles.renderCreated(
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
    .description('Get a data profile by id')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (id, opts) => {
      try {
        dlpProfiles.renderGet(
          await new SdkDataProfilesService().get(id),
          opts.output as OutputFormat,
        );
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  group
    .command('replace <id>')
    .description('Full-replace a data profile (PUT)')
    .option('--body <json|->', 'JSON body (or "-" for stdin)')
    .option('--body-file <path>', 'Path to JSON body file')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (id, opts) => {
      try {
        const body = await parseBody({ body: opts.body, bodyFile: opts.bodyFile });
        if (!body) throw new Error('--body or --body-file is required');
        dlpProfiles.renderReplaced(
          // biome-ignore lint/suspicious/noExplicitAny: parseBody returns unknown, cast for replace()
          await new SdkDataProfilesService().replace(id, body as any),
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
      'JSON Merge Patch. body must include name + profile_type. Use --body-file for nested fields. ' +
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
        dlpProfiles.renderPatched(
          // biome-ignore lint/suspicious/noExplicitAny: buildMergePatch returns Record<string, unknown>, cast for patch()
          await new SdkDataProfilesService().patch(id, body as any),
          opts.output as OutputFormat,
        );
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(2);
      }
    });

  group
    .command('delete <id>')
    .description('Not supported — prints the patch idiom and exits 2')
    .action((id) => {
      console.error(`
This DLP API has no DELETE for data profiles.
To soft-delete, fetch the profile to get its name + profile_type, then patch:

  airs runtime dlp profiles get ${id} --output json
  airs runtime dlp profiles patch ${id} --body-file - <<EOF
  { "name": "...", "profile_type": "...", "profile_status": "deleted" }
  EOF
`);
      process.exit(2);
    });
}
