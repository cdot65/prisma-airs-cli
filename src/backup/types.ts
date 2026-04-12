/** Supported backup serialization formats. */
export type BackupFormat = 'json' | 'yaml';

/** Discriminator for multi-resource backup directories. */
export type ResourceType = 'redteam-target';

/** Envelope wrapping any backed-up resource. */
export interface BackupEnvelope<T> {
  version: string;
  resourceType: ResourceType;
  exportedAt: string;
  data: T;
}

/** Per-target result reported after a backup run. */
export interface BackupResult {
  name: string;
  filename: string;
  status: 'ok' | 'failed';
  error?: string;
}

/** Per-target result reported after a restore run. */
export interface RestoreResult {
  name: string;
  action: 'created' | 'updated' | 'skipped' | 'failed';
  error?: string;
}
