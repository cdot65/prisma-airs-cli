import { SdkRedTeamService } from '../../airs/redteam.js';
import type { RedTeamTargetDetail } from '../../airs/types.js';
import { sanitizeFilename, writeBackupFile } from '../../backup/io.js';
import type { BackupEnvelope, BackupFormat, BackupResult } from '../../backup/types.js';
import { loadConfig } from '../../config/loader.js';

export async function createRedTeamService(): Promise<SdkRedTeamService> {
  const config = await loadConfig();
  return new SdkRedTeamService({
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    tokenEndpoint: config.mgmtTokenEndpoint,
  });
}

/** Recursively strip keys whose value is null. */
function stripNulls(obj: unknown): unknown {
  if (Array.isArray(obj)) return obj.map(stripNulls);
  if (obj !== null && typeof obj === 'object') {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
      if (v !== null) out[k] = stripNulls(v);
    }
    return out;
  }
  return obj;
}

/** Convert a RedTeamTargetDetail to the create-request shape (strip server fields, snake_case). */
function toBackupData(target: RedTeamTargetDetail): Record<string, unknown> {
  const data: Record<string, unknown> = {
    name: target.name,
    target_type: target.targetType,
  };
  if (target.connectionType) data.connection_type = target.connectionType;
  if (target.apiEndpointType) data.api_endpoint_type = target.apiEndpointType;
  if (target.responseMode) data.response_mode = target.responseMode;
  if (target.authType) data.auth_type = target.authType;
  if (target.authConfig) data.auth_config = target.authConfig;
  if (target.networkBrokerChannelUuid)
    data.network_broker_channel_uuid = target.networkBrokerChannelUuid;
  if (target.sessionSupported != null) data.session_supported = target.sessionSupported;
  if (target.extraInfo) data.extra_info = target.extraInfo;
  if (target.connectionParams) data.connection_params = target.connectionParams;
  if (target.background) data.target_background = target.background;
  if (target.additionalContext) data.additional_context = target.additionalContext;
  if (target.metadata) data.target_metadata = target.metadata;
  return stripNulls(data) as Record<string, unknown>;
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
      writeBackupFile(opts.outputDir, filename, envelope, opts.format);
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
