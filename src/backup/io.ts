import fs from 'node:fs';
import path from 'node:path';
import yaml from 'js-yaml';
import type { BackupEnvelope, BackupFormat, ResourceType } from './types.js';

/**
 * Sanitize a resource name into a filesystem-safe filename (no extension).
 * Lowercases, replaces non-alphanumeric with hyphens, collapses runs, trims.
 */
export function sanitizeFilename(name: string): string {
  if (!name) return 'unnamed';
  let sanitized = name
    .toLowerCase()
    .replace(/[^a-z0-9]/g, '-')
    .replace(/-{2,}/g, '-');
  // Only trim if originally started/ended with hyphens (before any replacement)
  if (name.match(/^-+/)) {
    sanitized = sanitized.replace(/^-+/, '');
  }
  if (name.match(/-+$/)) {
    sanitized = sanitized.replace(/-+$/, '');
  }
  return sanitized || 'unnamed';
}

/** Resolve output directory — user-specified or default `./airs-backup/<subdir>/`. */
export function resolveOutputDir(userDir: string | undefined, defaultSubdir: string): string {
  if (userDir) return path.resolve(userDir);
  return path.resolve('./airs-backup', defaultSubdir);
}

/** Serialize envelope and write to `dir/filename.{json|yaml}`. Creates dir if needed. */
export function writeBackupFile<T>(
  dir: string,
  filename: string,
  envelope: BackupEnvelope<T>,
  format: BackupFormat,
): void {
  fs.mkdirSync(dir, { recursive: true });
  const ext = format === 'yaml' ? 'yaml' : 'json';
  const filePath = path.join(dir, `${filename}.${ext}`);
  const content =
    format === 'yaml'
      ? yaml.dump(envelope, { lineWidth: -1, noRefs: true })
      : `${JSON.stringify(envelope, null, 2)}\n`;
  fs.writeFileSync(filePath, content, 'utf-8');
}

/** Read and parse a single backup file. Detects format from extension. */
export function readBackupFile<T>(filePath: string): BackupEnvelope<T> {
  const ext = path.extname(filePath).toLowerCase();
  if (ext !== '.json' && ext !== '.yaml' && ext !== '.yml') {
    throw new Error(`Unsupported file format: ${ext} (expected .json, .yaml, or .yml)`);
  }
  const raw = fs.readFileSync(filePath, 'utf-8');
  const parsed = ext === '.json' ? JSON.parse(raw) : yaml.load(raw);
  if (!parsed || typeof parsed !== 'object' || !('version' in parsed) || !('data' in parsed)) {
    throw new Error(`Invalid backup file: ${filePath} (missing version or data)`);
  }
  return parsed as BackupEnvelope<T>;
}

/** Read all backup files from a directory, filtering by resourceType. */
export function readBackupDir<T>(dirPath: string, resourceType: ResourceType): BackupEnvelope<T>[] {
  const files = fs
    .readdirSync(dirPath)
    .filter((f) => /\.(json|ya?ml)$/i.test(f))
    .map((f) => path.join(dirPath, f));
  const envelopes: BackupEnvelope<T>[] = [];
  for (const filePath of files) {
    const envelope = readBackupFile<T>(filePath);
    if (envelope.resourceType === resourceType) {
      envelopes.push(envelope);
    }
  }
  return envelopes;
}
