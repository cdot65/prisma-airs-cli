import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { BaseChatModel } from '@langchain/core/language_models/chat_models';
import { RunnableLambda } from '@langchain/core/runnables';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { RunState } from '../../../src/core/types.js';
import { LearningExtractor } from '../../../src/memory/extractor.js';
import { MemoryStore } from '../../../src/memory/store.js';

function makeRunState(overrides: Partial<RunState> = {}): RunState {
  return {
    id: 'run-1',
    createdAt: '2025-01-01T00:00:00Z',
    updatedAt: '2025-01-01T01:00:00Z',
    userInput: {
      topicDescription: 'Block weapons discussions',
      intent: 'block',
      profileName: 'test-profile',
      maxIterations: 3,
      targetCoverage: 0.9,
    },
    iterations: [
      {
        iteration: 1,
        timestamp: '2025-01-01T00:00:00Z',
        topic: {
          name: 'Weapons',
          description: 'Block weapons talk',
          examples: ['How to make a gun'],
        },
        testCases: [],
        testResults: [],
        metrics: {
          truePositives: 8,
          trueNegatives: 9,
          falsePositives: 1,
          falseNegatives: 2,
          truePositiveRate: 0.8,
          trueNegativeRate: 0.9,
          accuracy: 0.85,
          coverage: 0.8,
          f1Score: 0.84,
        },
        analysis: {
          summary: 'Decent',
          falsePositivePatterns: [],
          falseNegativePatterns: [],
          suggestions: [],
        },
        durationMs: 5000,
      },
      {
        iteration: 2,
        timestamp: '2025-01-01T00:30:00Z',
        topic: {
          name: 'Weapons',
          description: 'Block weapons manufacturing',
          examples: ['How to make a gun', 'Buy ammunition'],
        },
        testCases: [],
        testResults: [],
        metrics: {
          truePositives: 9,
          trueNegatives: 10,
          falsePositives: 0,
          falseNegatives: 1,
          truePositiveRate: 0.9,
          trueNegativeRate: 1.0,
          accuracy: 0.95,
          coverage: 0.9,
          f1Score: 0.95,
        },
        analysis: {
          summary: 'Improved',
          falsePositivePatterns: [],
          falseNegativePatterns: [],
          suggestions: [],
        },
        durationMs: 5000,
      },
    ],
    currentIteration: 2,
    bestIteration: 2,
    bestCoverage: 0.9,
    consecutiveRegressions: 0,
    hasTriedSimplification: false,
    status: 'completed',
    ...overrides,
  };
}

// Mock model that returns structured extraction output
function createMockModel() {
  const mockResponse = {
    learnings: [
      {
        insight: 'Short direct descriptions outperform nuanced ones',
        strategy: 'Kept description under 200 chars with clear keywords',
        outcome: 'improved' as const,
        changeType: 'description-only' as const,
        tags: ['brevity', 'fp-reduction'],
      },
    ],
    antiPatterns: ['Adding coded language to examples broadens matching unpredictably'],
  };

  return {
    withStructuredOutput: vi
      .fn()
      .mockReturnValue(new RunnableLambda({ func: async () => mockResponse })),
  };
}

describe('LearningExtractor', () => {
  let tempDir: string;
  let store: MemoryStore;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'extractor-test-'));
    store = new MemoryStore(tempDir);
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it('extracts learnings and saves to store', async () => {
    const model = createMockModel();
    const extractor = new LearningExtractor(model as BaseChatModel, store);
    const runState = makeRunState();

    const result = await extractor.extractAndSave(runState);

    expect(result.learnings).toHaveLength(1);
    expect(result.learnings[0].insight).toBe('Short direct descriptions outperform nuanced ones');

    // Verify saved to store
    const categories = await store.listCategories();
    expect(categories.length).toBeGreaterThan(0);
  });

  it('increments corroboration for duplicate insights', async () => {
    const model = createMockModel();
    const extractor = new LearningExtractor(model as BaseChatModel, store);
    const runState = makeRunState();

    // First extraction
    await extractor.extractAndSave(runState);

    // Second extraction with same insight
    const _result = await extractor.extractAndSave(makeRunState({ id: 'run-2' }));

    const category = (await store.listCategories())[0];
    const memory = await store.load(category);
    // Should have 1 learning with corroborations=1 (not 2 learnings)
    const matching = memory?.learnings.filter(
      (l) => l.insight === 'Short direct descriptions outperform nuanced ones',
    );
    expect(matching).toHaveLength(1);
    expect(matching[0].corroborations).toBe(1);
  });

  it('tracks bestKnown from run state', async () => {
    const model = createMockModel();
    const extractor = new LearningExtractor(model as BaseChatModel, store);
    const runState = makeRunState();

    await extractor.extractAndSave(runState);

    const category = (await store.listCategories())[0];
    const memory = await store.load(category);
    expect(memory?.bestKnown).not.toBeNull();
    expect(memory?.bestKnown?.runId).toBe('run-1');
    expect(memory?.bestKnown?.iteration).toBe(2);
  });

  it('updates bestKnown only when new run is better', async () => {
    const model = createMockModel();
    const extractor = new LearningExtractor(model as BaseChatModel, store);

    // First run: coverage 0.9
    await extractor.extractAndSave(makeRunState());

    // Second run: coverage 0.7 (worse)
    const worseRun = makeRunState({
      id: 'run-2',
      bestCoverage: 0.7,
      bestIteration: 1,
      iterations: [makeRunState().iterations[0]],
    });
    await extractor.extractAndSave(worseRun);

    const category = (await store.listCategories())[0];
    const memory = await store.load(category);
    // Should still reference original best
    expect(memory?.bestKnown?.runId).toBe('run-1');
  });

  it('saves anti-patterns', async () => {
    const model = createMockModel();
    const extractor = new LearningExtractor(model as BaseChatModel, store);

    await extractor.extractAndSave(makeRunState());

    const category = (await store.listCategories())[0];
    const memory = await store.load(category);
    expect(memory?.antiPatterns).toContain(
      'Adding coded language to examples broadens matching unpredictably',
    );
  });

  it('formats iteration history with example removals and additions', async () => {
    const model = createMockModel();
    const extractor = new LearningExtractor(model as BaseChatModel, store);
    // Iteration 2 removes an example and adds a new one
    const runState = makeRunState({
      iterations: [
        {
          ...makeRunState().iterations[0],
          topic: {
            name: 'Weapons',
            description: 'Block weapons talk',
            examples: ['How to make a gun', 'Knife fighting'],
          },
        },
        {
          ...makeRunState().iterations[1],
          topic: {
            name: 'Weapons',
            description: 'Block weapons manufacturing',
            examples: ['How to make a gun', 'Buy ammunition'],
          },
        },
      ],
    });

    const result = await extractor.extractAndSave(runState);
    expect(result.learnings).toHaveLength(1);
  });

  it('formats "none" when no changes between iterations and shows negative delta', async () => {
    const model = createMockModel();
    const extractor = new LearningExtractor(model as BaseChatModel, store);
    // Two iterations with identical desc+examples but coverage drops
    const runState = makeRunState({
      iterations: [
        {
          ...makeRunState().iterations[0],
          topic: {
            name: 'Weapons',
            description: 'Block weapons talk',
            examples: ['How to make a gun'],
          },
          metrics: {
            ...makeRunState().iterations[0].metrics,
            coverage: 0.9,
          },
        },
        {
          ...makeRunState().iterations[1],
          topic: {
            name: 'Weapons',
            description: 'Block weapons talk',
            examples: ['How to make a gun'],
          },
          metrics: {
            ...makeRunState().iterations[1].metrics,
            coverage: 0.7,
          },
        },
      ],
    });

    const result = await extractor.extractAndSave(runState);
    expect(result.learnings).toHaveLength(1);
  });

  it('falls back to last iteration when bestIteration index is out of bounds', async () => {
    const model = createMockModel();
    const extractor = new LearningExtractor(model as BaseChatModel, store);
    // bestIteration=0 means index -1 which is undefined, should fall back to last
    const runState = makeRunState({ bestIteration: 0 });

    const result = await extractor.extractAndSave(runState);
    expect(result.learnings).toHaveLength(1);
  });

  it('handles learnings without changeType (neutral outcome)', async () => {
    const mockResponse = {
      learnings: [
        {
          insight: 'Broad descriptions cause FP overlap',
          strategy: 'Observed overlap without making changes',
          outcome: 'neutral' as const,
          // changeType intentionally omitted — LLM does this for neutral outcomes
          tags: ['fp-reduction'],
        },
        {
          insight: 'Short descriptions work better',
          strategy: 'Shortened description',
          outcome: 'improved' as const,
          changeType: 'description-only' as const,
          tags: ['brevity'],
        },
      ],
      antiPatterns: [],
    };

    const model = {
      withStructuredOutput: vi
        .fn()
        .mockReturnValue(new RunnableLambda({ func: async () => mockResponse })),
    };

    const extractor = new LearningExtractor(model as BaseChatModel, store);
    const result = await extractor.extractAndSave(makeRunState());

    expect(result.learnings).toHaveLength(2);
    // Missing changeType should default to 'initial'
    expect(result.learnings[0].changeType).toBe('initial');
    // Explicit changeType should be preserved
    expect(result.learnings[1].changeType).toBe('description-only');
  });

  it('skips runs with < 1 iteration', async () => {
    const model = createMockModel();
    const extractor = new LearningExtractor(model as BaseChatModel, store);

    const result = await extractor.extractAndSave(makeRunState({ iterations: [] }));
    expect(result.learnings).toHaveLength(0);
  });
});
