import type { Command } from 'commander';
import { SdkDataFilteringProfilesService } from '../../../airs/dlp/data-filtering-profiles.js';
import { dlpFilteringProfiles, fail, type OutputFormat, usageError } from '../../renderer/index.js';
import { buildFilteringProfileBody, repeatable } from './build-body.js';
import { parseBody } from './patch.js';

function listFlags<T extends Command>(cmd: T): T {
  return cmd
    .option('--page <n>', 'Zero-indexed page number', (v) => Number.parseInt(v, 10))
    .option('--size <n>', 'Page size', (v) => Number.parseInt(v, 10))
    .option('--sort <field,dir>', 'Sort criteria (repeatable)', repeatable)
    .option('--output <fmt>', 'Output format', 'pretty');
}

async function resolveReplaceBody(opts: Record<string, unknown>): Promise<unknown> {
  if (opts.body || opts.bodyFile) {
    const body = await parseBody({ body: opts.body as string, bodyFile: opts.bodyFile as string });
    if (!body) throw new Error('--body or --body-file was empty');
    return body;
  }
  return buildFilteringProfileBody(opts);
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
      fail(err);
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
        fail(err);
      }
    });

  group
    .command('replace <id>')
    .description('Full-replace a filtering profile (PUT)')
    .option('--file-based', 'Apply to file-based scans (boolean)')
    .option('--non-file-based', 'Apply to non-file-based scans (boolean)')
    .option('--description <s>', 'Description')
    .option('--direction <s>', 'Direction: BOTH|UPLOAD|DOWNLOAD')
    .option('--log-severity <s>', 'Severity: CRITICAL|HIGH|MEDIUM|LOW|INFORMATIONAL')
    .option('--scan-type <s>', 'Scan type: include|exclude')
    .option('--data-profile-id <n>', 'Data profile ID', (v) => Number(v))
    .option('--euc-template-id <s>', 'EUC template ID')
    .option('--end-user-coaching', 'Enable end-user coaching')
    .option('--granular', 'Granular profile')
    .option('--file-type <s>', 'File type (repeatable)', repeatable)
    .option('--body <json|->', 'Raw JSON body (escape hatch; or "-" for stdin)')
    .option('--body-file <path>', 'Raw JSON body file (escape hatch)')
    .option('--output <fmt>', 'Output format', 'pretty')
    .action(async (id, opts) => {
      try {
        const body = await resolveReplaceBody(opts);
        const svc = new SdkDataFilteringProfilesService();
        dlpFilteringProfiles.renderReplaced(
          // biome-ignore lint/suspicious/noExplicitAny: body shape verified by SDK Zod
          await svc.replace(id, body as any),
          opts.output as OutputFormat,
        );
      } catch (err) {
        usageError(err instanceof Error ? err.message : String(err));
      }
    });
}
