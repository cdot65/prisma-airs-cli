import { randomUUID } from 'node:crypto';
import { link, open, unlink } from 'node:fs/promises';
import { dirname, join } from 'node:path';

/** Atomically publish a private report without overwriting a file or following a symlink. */
export async function writeReportFile(path: string, content: string): Promise<void> {
  const temporary = join(dirname(path), `.airs-report-${randomUUID()}.tmp`);
  const file = await open(temporary, 'wx', 0o600);
  try {
    await file.writeFile(content, 'utf8');
    await file.sync();
    await file.close();
    // Hard-link publication is atomic and fails if the destination already exists.
    await link(temporary, path);
  } finally {
    await file.close();
    await unlink(temporary);
  }
}
