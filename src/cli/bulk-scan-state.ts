import * as fs from 'node:fs/promises';
import * as path from 'node:path';

export interface BulkScanState {
  scanIds: string[];
  profile: string;
  promptCount: number;
  sessionId?: string;
  timestamp?: string;
}

/**
 * Persist bulk scan IDs to a JSON file so polling can be resumed
 * if the process crashes or hits a rate limit.
 */
export async function saveBulkScanState(state: BulkScanState, dir: string): Promise<string> {
  await fs.mkdir(dir, { recursive: true });
  const ts = new Date().toISOString().replace(/[:.]/g, '-');
  const filePath = path.join(dir, `${ts}.bulk-scan.json`);
  const payload = { ...state, timestamp: new Date().toISOString() };
  await fs.writeFile(filePath, JSON.stringify(payload, null, 2), 'utf-8');
  return filePath;
}

/** Load a previously saved bulk scan state file. */
export async function loadBulkScanState(filePath: string): Promise<BulkScanState> {
  const raw = await fs.readFile(filePath, 'utf-8');
  return JSON.parse(raw) as BulkScanState;
}
