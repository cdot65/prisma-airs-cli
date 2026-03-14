import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { RunState } from '../../../src/core/types.js';
import { JsonFileStore } from '../../../src/persistence/store.js';

function makeRunState(overrides: Partial<RunState> = {}): RunState {
  return {
    id: 'run-001',
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    userInput: {
      topicDescription: 'Block discussions about weapons',
      intent: 'block',
      profileName: 'test-profile',
    },
    iterations: [],
    currentIteration: 0,
    bestIteration: 0,
    bestCoverage: 0,
    consecutiveRegressions: 0,
    hasTriedSimplification: false,
    status: 'running',
    ...overrides,
  };
}

describe('JsonFileStore', () => {
  let dir: string;
  let store: JsonFileStore;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'guardrail-test-'));
    store = new JsonFileStore(dir);
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  describe('save + load', () => {
    it('saves and loads a run state', async () => {
      const run = makeRunState();
      await store.save(run);
      const loaded = await store.load('run-001');
      expect(loaded).toEqual(run);
    });

    it('overwrites existing run on save', async () => {
      const run = makeRunState();
      await store.save(run);
      const updated = { ...run, currentIteration: 5, updatedAt: '2026-01-02T00:00:00Z' };
      await store.save(updated);
      const loaded = await store.load('run-001');
      expect(loaded?.currentIteration).toBe(5);
    });

    it('returns null for non-existent run', async () => {
      const loaded = await store.load('nonexistent');
      expect(loaded).toBeNull();
    });
  });

  describe('list', () => {
    it('returns empty array when no runs', async () => {
      const list = await store.list();
      expect(list).toEqual([]);
    });

    it('returns summaries of all runs', async () => {
      await store.save(makeRunState({ id: 'run-001' }));
      await store.save(makeRunState({ id: 'run-002', bestCoverage: 0.85 }));
      const list = await store.list();
      expect(list).toHaveLength(2);
      expect(list.map((r) => r.id).sort()).toEqual(['run-001', 'run-002']);
    });

    it('includes summary fields', async () => {
      await store.save(makeRunState({ id: 'run-001', bestCoverage: 0.75, currentIteration: 3 }));
      const [summary] = await store.list();
      expect(summary.id).toBe('run-001');
      expect(summary.bestCoverage).toBe(0.75);
      expect(summary.currentIteration).toBe(3);
      expect(summary.topicDescription).toBe('Block discussions about weapons');
    });
  });

  describe('delete', () => {
    it('deletes existing run', async () => {
      await store.save(makeRunState());
      await store.delete('run-001');
      const loaded = await store.load('run-001');
      expect(loaded).toBeNull();
    });

    it('does not throw when deleting non-existent run', async () => {
      await expect(store.delete('nonexistent')).resolves.not.toThrow();
    });
  });

  describe('list edge cases', () => {
    it('returns empty array for non-existent directory', async () => {
      const missingStore = new JsonFileStore(join(dir, 'does-not-exist'));
      const list = await missingStore.list();
      expect(list).toEqual([]);
    });

    it('skips non-json files in directory', async () => {
      await store.save(makeRunState({ id: 'valid-run' }));
      const { writeFile: wf } = await import('node:fs/promises');
      await wf(join(dir, 'README.txt'), 'not a json file');
      await wf(join(dir, '.hidden'), 'hidden file');

      const list = await store.list();
      expect(list).toHaveLength(1);
      expect(list[0].id).toBe('valid-run');
    });

    it('skips corrupted JSON files', async () => {
      await store.save(makeRunState({ id: 'good-run' }));
      // Write a corrupted JSON file directly
      const { writeFile: wf } = await import('node:fs/promises');
      await wf(join(dir, 'bad-run.json'), 'not-valid-json!!!');

      const list = await store.list();
      expect(list).toHaveLength(1);
      expect(list[0].id).toBe('good-run');
    });
  });

  describe('atomic writes', () => {
    it('creates directory if it does not exist', async () => {
      const nested = join(dir, 'nested', 'deep');
      const nestedStore = new JsonFileStore(nested);
      await nestedStore.save(makeRunState());
      const loaded = await nestedStore.load('run-001');
      expect(loaded).toEqual(makeRunState());
    });
  });
});
