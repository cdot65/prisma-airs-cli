import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { MemoryInjector } from '../../../src/memory/injector.js';
import { MemoryStore } from '../../../src/memory/store.js';
import type { Learning, TopicMemory } from '../../../src/memory/types.js';

function makeLearning(overrides: Partial<Learning> = {}): Learning {
  return {
    id: 'learn-1',
    runId: 'run-1',
    extractedAt: '2025-01-01T00:00:00Z',
    topicCategory: 'block-weapons-discussions',
    topicDescription: 'Block weapons discussions',
    insight: 'Short descriptions outperform nuanced ones',
    strategy: 'Keep description under 200 chars',
    outcome: 'improved',
    changeType: 'description-only',
    metrics: { coverage: 0.9, tpr: 0.95, tnr: 0.85, accuracy: 0.9, f1: 0.9 },
    corroborations: 2,
    tags: ['brevity'],
    ...overrides,
  };
}

describe('MemoryInjector', () => {
  let tempDir: string;
  let store: MemoryStore;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'injector-test-'));
    store = new MemoryStore(tempDir);
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it('formats learnings into prompt section', async () => {
    const memory: TopicMemory = {
      category: 'block-weapons-discussions',
      updatedAt: '2025-01-01T00:00:00Z',
      learnings: [
        makeLearning({
          outcome: 'improved',
          insight: 'Short descriptions work best',
          corroborations: 2,
        }),
        makeLearning({
          id: 'l2',
          outcome: 'degraded',
          insight: 'Coded language degrades TNR',
          corroborations: 1,
        }),
      ],
      bestKnown: null,
      antiPatterns: ['Adding exclusion clauses near char limit causes truncation'],
    };
    await store.save(memory);

    const injector = new MemoryInjector(store, 3000);
    const section = await injector.buildMemorySection('Block weapons discussions');

    expect(section).toContain('Learnings from Previous Runs');
    expect(section).toContain('[DO]');
    expect(section).toContain('[AVOID]');
    expect(section).toContain('Short descriptions work best');
    expect(section).toContain('seen 3x');
    expect(section).toContain('seen 2x');
  });

  it('returns empty string when no relevant memories', async () => {
    const injector = new MemoryInjector(store, 3000);
    const section = await injector.buildMemorySection('Something completely unrelated');

    expect(section).toBe('');
  });

  it('returns empty string when memories exist but have no learnings or anti-patterns', async () => {
    const memory: TopicMemory = {
      category: 'block-weapons-discussions',
      updatedAt: '2025-01-01T00:00:00Z',
      learnings: [],
      bestKnown: null,
      antiPatterns: [],
    };
    await store.save(memory);

    const injector = new MemoryInjector(store, 3000);
    const section = await injector.buildMemorySection('Block weapons discussions');
    expect(section).toBe('');
  });

  it('returns empty string when store is empty', async () => {
    const injector = new MemoryInjector(store, 3000);
    const section = await injector.buildMemorySection('Block weapons');

    expect(section).toBe('');
  });

  it('keeps output within char budget', async () => {
    const learnings: Learning[] = [];
    for (let i = 0; i < 50; i++) {
      learnings.push(
        makeLearning({
          id: `l-${i}`,
          insight: `Insight number ${i} with some extra text to increase length`,
          corroborations: i,
        }),
      );
    }

    const memory: TopicMemory = {
      category: 'block-weapons-discussions',
      updatedAt: '2025-01-01T00:00:00Z',
      learnings,
      bestKnown: null,
      antiPatterns: [],
    };
    await store.save(memory);

    const budget = 1500;
    const injector = new MemoryInjector(store, budget);
    const section = await injector.buildMemorySection('Block weapons discussions');

    expect(section.length).toBeLessThanOrEqual(budget);
  });

  it('includes all learnings when under budget (no artificial count cap)', async () => {
    const learnings: Learning[] = [];
    for (let i = 0; i < 15; i++) {
      learnings.push(
        makeLearning({
          id: `l-${i}`,
          insight: `Insight ${i}`,
          corroborations: i,
        }),
      );
    }

    const memory: TopicMemory = {
      category: 'block-weapons-discussions',
      updatedAt: '2025-01-01T00:00:00Z',
      learnings,
      bestKnown: null,
      antiPatterns: [],
    };
    await store.save(memory);

    // Large budget — all 15 should appear
    const injector = new MemoryInjector(store, 5000);
    const section = await injector.buildMemorySection('Block weapons discussions');

    for (let i = 0; i < 15; i++) {
      expect(section).toContain(`Insight ${i}`);
    }
  });

  it('uses compact format for overflow learnings', async () => {
    const learnings: Learning[] = [];
    for (let i = 0; i < 20; i++) {
      learnings.push(
        makeLearning({
          id: `l-${i}`,
          insight: `Insight number ${i} with padding text`,
          corroborations: 20 - i, // highest corroboration first
        }),
      );
    }

    const memory: TopicMemory = {
      category: 'block-weapons-discussions',
      updatedAt: '2025-01-01T00:00:00Z',
      learnings,
      bestKnown: null,
      antiPatterns: [],
    };
    await store.save(memory);

    // Tight budget: first few get verbose format, rest get compact
    const injector = new MemoryInjector(store, 800);
    const section = await injector.buildMemorySection('Block weapons discussions');

    const bulletLines = section.split('\n').filter((l) => l.startsWith('- ['));
    // Some should have metadata (verbose), some should not (compact)
    const verbose = bulletLines.filter((l) => l.includes(', seen '));
    const compact = bulletLines.filter((l) => !l.includes(', seen '));

    expect(verbose.length).toBeGreaterThan(0);
    expect(compact.length).toBeGreaterThan(0);
  });

  it('appends omission notice when learnings overflow budget', async () => {
    const learnings: Learning[] = [];
    for (let i = 0; i < 30; i++) {
      learnings.push(
        makeLearning({
          id: `l-${i}`,
          insight: `Insight number ${i} with a reasonably long description to eat the budget`,
          corroborations: 30 - i,
        }),
      );
    }

    const memory: TopicMemory = {
      category: 'block-weapons-discussions',
      updatedAt: '2025-01-01T00:00:00Z',
      learnings,
      bestKnown: null,
      antiPatterns: [],
    };
    await store.save(memory);

    const injector = new MemoryInjector(store, 800);
    const section = await injector.buildMemorySection('Block weapons discussions');

    expect(section).toMatch(/\(\+\d+ more learnings omitted\)/);
  });

  it('sorts learnings by corroborations descending', async () => {
    const memory: TopicMemory = {
      category: 'block-weapons-discussions',
      updatedAt: '2025-01-01T00:00:00Z',
      learnings: [
        makeLearning({ id: 'l1', insight: 'Low corroboration', corroborations: 0 }),
        makeLearning({ id: 'l2', insight: 'High corroboration', corroborations: 5 }),
        makeLearning({ id: 'l3', insight: 'Medium corroboration', corroborations: 2 }),
      ],
      bestKnown: null,
      antiPatterns: [],
    };
    await store.save(memory);

    const injector = new MemoryInjector(store, 3000);
    const section = await injector.buildMemorySection('Block weapons discussions');

    const highIdx = section.indexOf('High corroboration');
    const medIdx = section.indexOf('Medium corroboration');
    const lowIdx = section.indexOf('Low corroboration');

    expect(highIdx).toBeLessThan(medIdx);
    expect(medIdx).toBeLessThan(lowIdx);
  });

  it('includes anti-patterns', async () => {
    const memory: TopicMemory = {
      category: 'block-weapons-discussions',
      updatedAt: '2025-01-01T00:00:00Z',
      learnings: [makeLearning()],
      bestKnown: null,
      antiPatterns: ['Never add coded language patterns'],
    };
    await store.save(memory);

    const injector = new MemoryInjector(store, 3000);
    const section = await injector.buildMemorySection('Block weapons discussions');

    expect(section).toContain('Never add coded language patterns');
  });
});
