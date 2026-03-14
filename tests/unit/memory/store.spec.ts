import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { MemoryStore, normalizeCategory } from '../../../src/memory/store.js';
import type { Learning, TopicMemory } from '../../../src/memory/types.js';

function makeLearning(overrides: Partial<Learning> = {}): Learning {
  return {
    id: 'learn-1',
    runId: 'run-1',
    extractedAt: '2025-01-01T00:00:00Z',
    topicCategory: 'block-weapons-discussions',
    topicDescription: 'Block weapons discussions',
    insight: 'Short descriptions work better',
    strategy: 'Keep description under 200 chars',
    outcome: 'improved',
    changeType: 'description-only',
    metrics: { coverage: 0.9, tpr: 0.95, tnr: 0.85, accuracy: 0.9, f1: 0.9 },
    corroborations: 0,
    tags: ['brevity'],
    ...overrides,
  };
}

describe('normalizeCategory', () => {
  it('normalizes a description to sorted keywords', () => {
    const result = normalizeCategory(
      'Block discussions about weapons manufacturing and procurement',
    );
    expect(result).toBe('block-discussions-manufacturing-procurement-weapons');
  });

  it('removes stop words', () => {
    const result = normalizeCategory('Block the use of weapons');
    expect(result).toBe('block-use-weapons');
  });

  it('deduplicates words', () => {
    const result = normalizeCategory('block block weapons weapons');
    expect(result).toBe('block-weapons');
  });

  it('lowercases everything', () => {
    const result = normalizeCategory('BLOCK Weapons');
    expect(result).toBe('block-weapons');
  });

  it('returns "uncategorized" for stop-word-only input', () => {
    expect(normalizeCategory('the and or')).toBe('uncategorized');
  });

  it('returns "uncategorized" for empty string', () => {
    expect(normalizeCategory('')).toBe('uncategorized');
  });
});

describe('MemoryStore', () => {
  let tempDir: string;
  let store: MemoryStore;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'memory-test-'));
    store = new MemoryStore(tempDir);
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it('saves and loads topic memory', async () => {
    const learning = makeLearning();
    const memory: TopicMemory = {
      category: 'block-weapons-discussions',
      updatedAt: '2025-01-01T00:00:00Z',
      learnings: [learning],
      bestKnown: null,
      antiPatterns: [],
    };

    await store.save(memory);
    const loaded = await store.load('block-weapons-discussions');

    expect(loaded).not.toBeNull();
    expect(loaded?.learnings).toHaveLength(1);
    expect(loaded?.learnings[0].insight).toBe('Short descriptions work better');
  });

  it('returns null for nonexistent category', async () => {
    const result = await store.load('nonexistent');
    expect(result).toBeNull();
  });

  it('findRelevant returns matching memories', async () => {
    const memory: TopicMemory = {
      category: 'block-weapons-discussions',
      updatedAt: '2025-01-01T00:00:00Z',
      learnings: [makeLearning()],
      bestKnown: null,
      antiPatterns: [],
    };
    await store.save(memory);

    const results = await store.findRelevant('Block discussions about weapons');
    expect(results).toHaveLength(1);
    expect(results[0].category).toBe('block-weapons-discussions');
  });

  it('findRelevant returns empty for unrelated topics', async () => {
    const memory: TopicMemory = {
      category: 'block-weapons-discussions',
      updatedAt: '2025-01-01T00:00:00Z',
      learnings: [makeLearning()],
      bestKnown: null,
      antiPatterns: [],
    };
    await store.save(memory);

    const results = await store.findRelevant('Allow cooking recipes');
    expect(results).toHaveLength(0);
  });

  it('overwrites existing memory on save', async () => {
    const memory1: TopicMemory = {
      category: 'test-cat',
      updatedAt: '2025-01-01T00:00:00Z',
      learnings: [makeLearning({ id: 'l1' })],
      bestKnown: null,
      antiPatterns: [],
    };
    await store.save(memory1);

    const memory2: TopicMemory = {
      category: 'test-cat',
      updatedAt: '2025-01-02T00:00:00Z',
      learnings: [
        makeLearning({ id: 'l1' }),
        makeLearning({ id: 'l2', insight: 'Second insight' }),
      ],
      bestKnown: null,
      antiPatterns: ['do not add coded language'],
    };
    await store.save(memory2);

    const loaded = await store.load('test-cat');
    expect(loaded?.learnings).toHaveLength(2);
    expect(loaded?.antiPatterns).toHaveLength(1);
  });

  it('returns empty from listCategories when dir does not exist', async () => {
    const missingStore = new MemoryStore(join(tempDir, 'nonexistent'));
    const cats = await missingStore.listCategories();
    expect(cats).toEqual([]);
  });

  it('lists all categories', async () => {
    await store.save({
      category: 'cat-a',
      updatedAt: '',
      learnings: [],
      bestKnown: null,
      antiPatterns: [],
    });
    await store.save({
      category: 'cat-b',
      updatedAt: '',
      learnings: [],
      bestKnown: null,
      antiPatterns: [],
    });

    const categories = await store.listCategories();
    expect(categories.sort()).toEqual(['cat-a', 'cat-b']);
  });
});
