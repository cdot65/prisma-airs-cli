import type { Command } from 'commander';
import { SdkRedTeamService } from '../../airs/redteam.js';
import type { RedTeamTargetDetail } from '../../airs/types.js';
import { resolveOutputDir, sanitizeFilename, writeBackupFile } from '../../backup/io.js';
import type { BackupEnvelope, BackupFormat, BackupResult } from '../../backup/types.js';
import { loadConfig } from '../../config/loader.js';
import { renderBackupHeader, renderBackupSummary, renderError } from '../renderer/index.js';

async function createRedTeamService(): Promise<SdkRedTeamService> {
  const config = await loadConfig();
  return new SdkRedTeamService({
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    tokenEndpoint: config.mgmtTokenEndpoint,
  });
}

/** Convert a RedTeamTargetDetail to the create-request shape (strip server fields, snake_case). */
function toBackupData(target: RedTeamTargetDetail): Record<string, unknown> {
  const data: Record<string, unknown> = {
    name: target.name,
    target_type: target.targetType,
  };
  if (target.connectionParams) data.connection_params = target.connectionParams;
  if (target.background) data.background = target.background;
  if (target.additionalContext) data.additional_context = target.additionalContext;
  if (target.metadata) data.metadata = target.metadata;
  return data;
}

/** Core backup logic — exported for testability. */
export async function backupTargets(opts: {
  outputDir: string;
  format: BackupFormat;
  name?: string;
}): Promise<BackupResult[]> {
  const service = await createRedTeamService();
  const allTargets = await service.listTargets();

  let uuids: Array<{ uuid: string; name: string }>;
  if (opts.name) {
    const match = allTargets.find((t) => t.name === opts.name);
    if (!match) throw new Error(`Target not found: ${opts.name}`);
    uuids = [{ uuid: match.uuid, name: match.name }];
  } else {
    uuids = allTargets.map((t) => ({ uuid: t.uuid, name: t.name }));
  }

  const dir = resolveOutputDir(opts.outputDir, 'targets');
  const results: BackupResult[] = [];

  for (const entry of uuids) {
    try {
      const detail = await service.getTarget(entry.uuid);
      const envelope: BackupEnvelope<Record<string, unknown>> = {
        version: '1',
        resourceType: 'redteam-target',
        exportedAt: new Date().toISOString(),
        data: toBackupData(detail),
      };
      const filename = sanitizeFilename(entry.name);
      writeBackupFile(dir, filename, envelope, opts.format);
      results.push({
        name: entry.name,
        filename: `${filename}.${opts.format === 'yaml' ? 'yaml' : 'json'}`,
        status: 'ok',
      });
    } catch (err) {
      results.push({
        name: entry.name,
        filename: '',
        status: 'failed',
        error: err instanceof Error ? err.message : String(err),
      });
    }
  }

  return results;
}

export function registerBackupCommand(program: Command): void {
  const backup = program.command('backup').description('Backup AIRS configuration to local files');

  backup
    .command('targets')
    .description('Backup red team targets to local JSON/YAML files')
    .option('--output-dir <path>', 'Output directory', undefined)
    .option('--format <format>', 'Output format: json or yaml', 'json')
    .option('--name <targetName>', 'Backup a single target by name')
    .action(async (opts) => {
      try {
        renderBackupHeader();
        const format = opts.format as BackupFormat;
        if (format !== 'json' && format !== 'yaml') {
          throw new Error(`Invalid format: ${format} (expected json or yaml)`);
        }
        const dir = resolveOutputDir(opts.outputDir, 'targets');
        const results = await backupTargets({
          outputDir: opts.outputDir,
          format,
          name: opts.name,
        });
        renderBackupSummary(results, dir);
        const failed = results.filter((r) => r.status === 'failed').length;
        if (failed > 0) process.exit(1);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
