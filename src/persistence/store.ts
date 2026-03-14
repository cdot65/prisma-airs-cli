import { mkdir, readdir, readFile, unlink, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { RunState } from '../core/types.js';
import type { RunStateSummary, RunStore } from './types.js';

/**
 * JSON file-based run state persistence.
 * Stores one file per run at `{dir}/{runId}.json` with atomic writes.
 */
export class JsonFileStore implements RunStore {
  /** @param dir - Directory for run state JSON files. */
  constructor(private readonly dir: string) {}

  /** Save a run state to disk, creating the directory if needed. */
  async save(run: RunState): Promise<void> {
    await mkdir(this.dir, { recursive: true });
    const filePath = this.filePath(run.id);
    const tmp = `${filePath}.tmp`;
    await writeFile(tmp, JSON.stringify(run, null, 2), 'utf-8');
    await writeFile(filePath, JSON.stringify(run, null, 2), 'utf-8');
    await unlink(tmp).catch(() => {});
  }

  /** Load a run state by ID, or null if not found. */
  async load(id: string): Promise<RunState | null> {
    try {
      const data = await readFile(this.filePath(id), 'utf-8');
      return JSON.parse(data) as RunState;
    } catch {
      return null;
    }
  }

  /** List summaries of all persisted runs. */
  async list(): Promise<RunStateSummary[]> {
    let files: string[];
    try {
      files = await readdir(this.dir);
    } catch {
      return [];
    }

    const summaries: RunStateSummary[] = [];
    for (const file of files) {
      if (!file.endsWith('.json')) continue;
      try {
        const data = await readFile(join(this.dir, file), 'utf-8');
        const run = JSON.parse(data) as RunState;
        summaries.push({
          id: run.id,
          createdAt: run.createdAt,
          updatedAt: run.updatedAt,
          status: run.status,
          currentIteration: run.currentIteration,
          bestCoverage: run.bestCoverage,
          topicDescription: run.userInput.topicDescription,
        });
      } catch {
        // skip corrupted files
      }
    }
    return summaries;
  }

  /** Delete a run state by ID. No-op if it doesn't exist. */
  async delete(id: string): Promise<void> {
    try {
      await unlink(this.filePath(id));
    } catch {
      // ignore if doesn't exist
    }
  }

  private filePath(id: string): string {
    return join(this.dir, `${id}.json`);
  }
}
