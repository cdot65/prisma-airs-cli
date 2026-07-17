import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { acquireBulkScanLock } from '../../../src/cli/bulk-scan-lock.js';

describe('bulk-scan job lock', () => {
  let tmpDir: string;
  let statePath: string;

  beforeEach(async () => {
    tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'airs-bulk-lock-'));
    statePath = path.join(tmpDir, 'job.bulk-scan.json');
    await fs.writeFile(statePath, '{}', 'utf-8');
  });

  afterEach(async () => {
    await fs.rm(tmpDir, { recursive: true, force: true });
  });

  it('allows only one owner and releases for a later invocation', async () => {
    const releaseFirst = await acquireBulkScanLock(statePath);

    await expect(acquireBulkScanLock(statePath)).rejects.toThrow(/already active/i);

    await releaseFirst();
    const releaseNext = await acquireBulkScanLock(statePath);
    await releaseNext();
  });

  it('recovers a lock whose owner process is no longer running', async () => {
    await fs.writeFile(
      `${statePath}.lock`,
      JSON.stringify({
        version: 1,
        pid: 2_147_483_647,
        createdAt: '2026-07-17T00:00:00.000Z',
        token: 'dead-owner',
      }),
      { mode: 0o600 },
    );

    const release = await acquireBulkScanLock(statePath);
    await release();
    await expect(fs.access(`${statePath}.lock`)).rejects.toMatchObject({ code: 'ENOENT' });
  });
});
