import type { RedTeamTargetCreateRequest, RedTeamTargetUpdateRequest } from '../../airs/types.js';
import { readBackupDir, readBackupFile } from '../../backup/io.js';
import type { BackupEnvelope, RestoreResult } from '../../backup/types.js';
import { createRedTeamService } from './backup.js';

/** Fields that are server-derived and must be stripped before create/update. */
const SERVER_DERIVED_FIELDS = [
  'uuid',
  'tsg_id',
  'status',
  'active',
  'validated',
  'version',
  'secret_version',
  'created_at',
  'updated_at',
  'created_by_user_id',
  'updated_by_user_id',
] as const;

/** Strip server-derived fields and normalize legacy field names for the create/update API. */
export function prepareTargetPayload(data: Record<string, unknown>): Record<string, unknown> {
  const clean = { ...data };
  for (const field of SERVER_DERIVED_FIELDS) {
    delete clean[field];
  }
  // Normalize legacy backup field names → current API names
  if ('background' in clean) {
    if (!('target_background' in clean)) {
      clean.target_background = clean.background;
    }
    delete clean.background;
  }
  if ('metadata' in clean) {
    if (!('target_metadata' in clean)) {
      clean.target_metadata = clean.metadata;
    }
    delete clean.metadata;
  }
  return clean;
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
        const payload = prepareTargetPayload(env.data);
        // Ensure routing fields are present — API requires the 4-tuple
        // (target_type, connection_type, api_endpoint_type, response_mode).
        // Merge from existing target first, then fall back to defaults.
        const current = await service.getTarget(existingUuid);
        if (!payload.target_type) payload.target_type = current.targetType;
        if (!payload.connection_type) payload.connection_type = current.connectionType ?? 'CUSTOM';
        if (!payload.api_endpoint_type)
          payload.api_endpoint_type = current.apiEndpointType ?? 'PUBLIC';
        if (!payload.response_mode) payload.response_mode = current.responseMode ?? 'REST';
        await service.updateTarget(
          existingUuid,
          payload as unknown as RedTeamTargetUpdateRequest,
          validateOpts,
        );
        results.push({ name, action: 'updated' });
      } else {
        const payload = prepareTargetPayload(env.data);
        if (!payload.connection_type) payload.connection_type = 'CUSTOM';
        if (!payload.api_endpoint_type) payload.api_endpoint_type = 'PUBLIC';
        if (!payload.response_mode) payload.response_mode = 'REST';
        await service.createTarget(payload as unknown as RedTeamTargetCreateRequest, validateOpts);
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
