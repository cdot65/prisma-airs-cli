import { writeFile } from 'node:fs/promises';
import type { ManifestEntry } from './types.js';

export interface Manifest {
  generatedAt: string;
  count: number;
  entries: ManifestEntry[];
}

/** Assemble a manifest from dirty-file entries. */
export function buildManifest(entries: ManifestEntry[]): Manifest {
  return {
    generatedAt: new Date().toISOString(),
    count: entries.length,
    entries,
  };
}

/** Write a manifest to disk as pretty JSON. */
export async function writeManifest(path: string, manifest: Manifest): Promise<void> {
  await writeFile(path, `${JSON.stringify(manifest, null, 2)}\n`);
}
