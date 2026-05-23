import type { Command } from 'commander';
import { SdkDataFilteringProfilesService } from '../../../airs/dlp/data-filtering-profiles.js';
import { dlpFilteringProfiles, type OutputFormat, renderError } from '../../renderer/index.js';
import { parseBody } from './patch.js';

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
    .command('filtering-profiles')
    .description(
      'DLP data filtering profiles. Read + full-replace only. ' +
        'Create, patch, and delete are not exposed by the DLP API.',
    );

  listFlags(group.command('list').description('List filtering profiles')).action(async (opts) => {
    try {
      const svc = new SdkDataFilteringProfilesService();
      const r = await svc.list({ page: opts.page, size: opts.size, sort: opts.sort });
      dlpFilteringProfiles.renderList(r, opts.output as OutputFormat);
    } catch (err) {
      renderError(err instanceof Error ? err.message : String(err));
      process.exit(1);
    }
  });

  group
    .command('get <id>')
    .description('Get a filtering profile by id')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (id, opts) => {
      try {
        const svc = new SdkDataFilteringProfilesService();
        dlpFilteringProfiles.renderGet(await svc.get(id), opts.output as OutputFormat);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  group
    .command('replace <id>')
    .description('Full-replace a filtering profile (PUT)')
    .option('--body <json|->', 'JSON body (or "-" for stdin)')
    .option('--body-file <path>', 'Path to JSON body file')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (id, opts) => {
      try {
        const body = await parseBody({ body: opts.body, bodyFile: opts.bodyFile });
        if (!body) throw new Error('--body or --body-file is required');
        const svc = new SdkDataFilteringProfilesService();
        dlpFilteringProfiles.renderReplaced(
          // biome-ignore lint/suspicious/noExplicitAny: parseBody returns unknown, cast for replace()
          await svc.replace(id, body as any),
          opts.output as OutputFormat,
        );
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(2);
      }
    });
}
