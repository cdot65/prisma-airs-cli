import type { Command } from 'commander';
import { SdkRedTeamService } from '../../airs/redteam.js';
import type { RedTeamTargetCreateRequest, RedTeamTargetUpdateRequest } from '../../airs/types.js';
import { readBackupDir, readBackupFile } from '../../backup/io.js';
import type { BackupEnvelope, RestoreResult } from '../../backup/types.js';
import { loadConfig } from '../../config/loader.js';
import { renderBackupHeader, renderError, renderRestoreSummary } from '../renderer/index.js';

async function createRedTeamService(): Promise<SdkRedTeamService> {
  const config = await loadConfig();
  return new SdkRedTeamService({
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    tokenEndpoint: config.mgmtTokenEndpoint,
  });
}

/** Core restore logic — exported for testability. */
export async function restoreTargets(opts: {
  file?: string;
  inputDir?: string;
  overwrite?: boolean;
  validate?: boolean;
}): Promise<RestoreResult[]> {
  const service = await createRedTeamService();

  let envelopes: BackupEnvelope<Record<string, unknown>>[];
  if (opts.file) {
    const env = readBackupFile<Record<string, unknown>>(opts.file);
    if (env.version !== '1' || env.resourceType !== 'redteam-target') {
      throw new Error(`Invalid backup: version=${env.version}, resourceType=${env.resourceType}`);
    }
    envelopes = [env];
  } else if (opts.inputDir) {
    envelopes = readBackupDir<Record<string, unknown>>(opts.inputDir, 'redteam-target');
  } else {
    throw new Error('Specify --file or --input-dir');
  }

  if (envelopes.length === 0) {
    throw new Error('No valid backup files found');
  }

  const existing = await service.listTargets();
  const existingByName = new Map(existing.map((t) => [t.name, t.uuid]));
  const validateOpts = opts.validate ? { validate: true } : undefined;
  const results: RestoreResult[] = [];

  for (const env of envelopes) {
    const name = env.data.name as string;
    try {
      const existingUuid = existingByName.get(name);
      if (existingUuid && !opts.overwrite) {
        results.push({ name, action: 'skipped' });
        continue;
      }
      if (existingUuid) {
        await service.updateTarget(
          existingUuid,
          env.data as unknown as RedTeamTargetUpdateRequest,
          validateOpts,
        );
        results.push({ name, action: 'updated' });
      } else {
        await service.createTarget(env.data as unknown as RedTeamTargetCreateRequest, validateOpts);
        results.push({ name, action: 'created' });
      }
    } catch (err) {
      results.push({
        name,
        action: 'failed',
        error: err instanceof Error ? err.message : String(err),
      });
    }
  }

  return results;
}

export function registerRestoreCommand(program: Command): void {
  const restore = program
    .command('restore')
    .description('Restore AIRS configuration from local backup files');

  restore
    .command('targets')
    .description('Restore red team targets from local JSON/YAML backup files')
    .option('--input-dir <path>', 'Directory containing backup files')
    .option('--file <path>', 'Single backup file to restore')
    .option('--overwrite', 'Update existing targets with same name (default: skip)')
    .option('--validate', 'Validate target connection before saving')
    .action(async (opts) => {
      try {
        renderBackupHeader();
        if (!opts.file && !opts.inputDir) {
          throw new Error('Specify --file <path> or --input-dir <path>');
        }
        const results = await restoreTargets({
          file: opts.file,
          inputDir: opts.inputDir,
          overwrite: opts.overwrite,
          validate: opts.validate,
        });
        renderRestoreSummary(results);
        const failed = results.filter((r) => r.action === 'failed').length;
        if (failed > 0) process.exit(1);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
