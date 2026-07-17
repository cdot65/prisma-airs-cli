import { randomUUID } from 'node:crypto';
import * as fs from 'node:fs/promises';

interface BulkScanLockRecord {
  version: 1;
  pid: number;
  createdAt: string;
  token: string;
}

function errorCode(error: unknown): string | undefined {
  return (error as NodeJS.ErrnoException | undefined)?.code;
}

function parseLock(raw: string, lockPath: string): BulkScanLockRecord {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    throw new Error(
      `Bulk-scan lock ${lockPath} is malformed. If no bulk-scan process is running, remove it manually.`,
    );
  }
  const record = value as Partial<BulkScanLockRecord>;
  if (
    record.version !== 1 ||
    !Number.isSafeInteger(record.pid) ||
    (record.pid ?? 0) <= 0 ||
    typeof record.createdAt !== 'string' ||
    typeof record.token !== 'string' ||
    record.token.length === 0
  ) {
    throw new Error(
      `Bulk-scan lock ${lockPath} has invalid ownership data. If no bulk-scan process is running, remove it manually.`,
    );
  }
  return record as BulkScanLockRecord;
}

function processIsAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return errorCode(error) !== 'ESRCH';
  }
}

async function installLock(lockPath: string, record: BulkScanLockRecord): Promise<void> {
  const candidate = `${lockPath}.candidate-${process.pid}-${randomUUID()}`;
  try {
    await fs.writeFile(candidate, JSON.stringify(record), {
      encoding: 'utf-8',
      flag: 'wx',
      mode: 0o600,
    });
    await fs.link(candidate, lockPath);
  } finally {
    await fs.rm(candidate, { force: true });
  }
}

/**
 * Acquire exclusive ownership of one durable bulk-scan state file.
 *
 * The returned release callback removes only the lock created by this invocation.
 * Locks owned by dead local processes are recovered automatically.
 */
export async function acquireBulkScanLock(statePath: string): Promise<() => Promise<void>> {
  const lockPath = `${statePath}.lock`;
  const record: BulkScanLockRecord = {
    version: 1,
    pid: process.pid,
    createdAt: new Date().toISOString(),
    token: randomUUID(),
  };

  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      await installLock(lockPath, record);
      return async () => {
        let current: BulkScanLockRecord;
        try {
          current = parseLock(await fs.readFile(lockPath, 'utf-8'), lockPath);
        } catch (error) {
          if (errorCode(error) === 'ENOENT') return;
          throw error;
        }
        if (current.token === record.token) await fs.rm(lockPath, { force: true });
      };
    } catch (error) {
      if (errorCode(error) !== 'EEXIST') throw error;
    }

    let owner: BulkScanLockRecord;
    try {
      owner = parseLock(await fs.readFile(lockPath, 'utf-8'), lockPath);
    } catch (error) {
      if (errorCode(error) === 'ENOENT') continue;
      throw error;
    }
    if (processIsAlive(owner.pid)) {
      throw new Error(
        `Bulk-scan job is already active in process ${owner.pid}. Wait for it to finish before resuming ${statePath}.`,
      );
    }
    await fs.rm(lockPath, { force: true });
  }

  throw new Error(`Could not acquire bulk-scan lock for ${statePath}`);
}
