import { describe, expect, it } from 'vitest';
import type { IterationResult } from '../../../src/core/types.js';
import { computeIterationDiff } from '../../../src/memory/diff.js';

function makeIteration(overrides: Partial<IterationResult> = {}): IterationResult {
  return {
    iteration: 1,
    timestamp: '2025-01-01T00:00:00Z',
    topic: {
      name: 'Test Topic',
      description: 'Original description',
      examples: ['example 1', 'example 2'],
    },
    testCases: [],
    testResults: [],
    metrics: {
      truePositives: 10,
      trueNegatives: 10,
      falsePositives: 0,
      falseNegatives: 0,
      truePositiveRate: 1.0,
      trueNegativeRate: 1.0,
      accuracy: 1.0,
      coverage: 1.0,
      f1Score: 1.0,
    },
    analysis: {
      summary: 'Good',
      falsePositivePatterns: [],
      falseNegativePatterns: [],
      suggestions: [],
    },
    durationMs: 1000,
    ...overrides,
  };
}

describe('computeIterationDiff', () => {
  it('detects description-only change', () => {
    const from = makeIteration({ iteration: 1 });
    const to = makeIteration({
      iteration: 2,
      topic: {
        name: 'Test Topic',
        description: 'New description',
        examples: ['example 1', 'example 2'],
      },
      metrics: {
        ...from.metrics,
        coverage: 0.8,
        truePositiveRate: 0.9,
        trueNegativeRate: 0.7,
        accuracy: 0.8,
        f1Score: 0.85,
      },
    });

    const diff = computeIterationDiff(from, to);

    expect(diff.descriptionChanged).toBe(true);
    expect(diff.examplesChanged).toBe(false);
    expect(diff.descriptionBefore).toBe('Original description');
    expect(diff.descriptionAfter).toBe('New description');
    expect(diff.examplesAdded).toEqual([]);
    expect(diff.examplesRemoved).toEqual([]);
  });

  it('detects examples-only change', () => {
    const from = makeIteration({ iteration: 1 });
    const to = makeIteration({
      iteration: 2,
      topic: {
        name: 'Test Topic',
        description: 'Original description',
        examples: ['example 1', 'example 3'],
      },
    });

    const diff = computeIterationDiff(from, to);

    expect(diff.descriptionChanged).toBe(false);
    expect(diff.examplesChanged).toBe(true);
    expect(diff.examplesAdded).toEqual(['example 3']);
    expect(diff.examplesRemoved).toEqual(['example 2']);
  });

  it('detects both description and examples changed', () => {
    const from = makeIteration({ iteration: 1 });
    const to = makeIteration({
      iteration: 2,
      topic: { name: 'Test Topic', description: 'Changed', examples: ['new example'] },
    });

    const diff = computeIterationDiff(from, to);

    expect(diff.descriptionChanged).toBe(true);
    expect(diff.examplesChanged).toBe(true);
  });

  it('computes metric deltas correctly', () => {
    const from = makeIteration({
      iteration: 1,
      metrics: {
        truePositives: 10,
        trueNegatives: 10,
        falsePositives: 0,
        falseNegatives: 0,
        truePositiveRate: 1.0,
        trueNegativeRate: 1.0,
        accuracy: 1.0,
        coverage: 1.0,
        f1Score: 1.0,
      },
    });
    const to = makeIteration({
      iteration: 2,
      metrics: {
        truePositives: 8,
        trueNegatives: 8,
        falsePositives: 2,
        falseNegatives: 2,
        truePositiveRate: 0.8,
        trueNegativeRate: 0.8,
        accuracy: 0.8,
        coverage: 0.8,
        f1Score: 0.8,
      },
    });

    const diff = computeIterationDiff(from, to);

    expect(diff.metricDelta.coverage).toBeCloseTo(-0.2);
    expect(diff.metricDelta.tpr).toBeCloseTo(-0.2);
    expect(diff.metricDelta.tnr).toBeCloseTo(-0.2);
    expect(diff.metricDelta.accuracy).toBeCloseTo(-0.2);
    expect(diff.metricDelta.f1).toBeCloseTo(-0.2);
  });

  it('sets fromIteration and toIteration', () => {
    const from = makeIteration({ iteration: 3 });
    const to = makeIteration({ iteration: 4 });

    const diff = computeIterationDiff(from, to);

    expect(diff.fromIteration).toBe(3);
    expect(diff.toIteration).toBe(4);
  });

  it('handles identical iterations', () => {
    const iter = makeIteration();
    const diff = computeIterationDiff(iter, iter);

    expect(diff.descriptionChanged).toBe(false);
    expect(diff.examplesChanged).toBe(false);
    expect(diff.metricDelta.coverage).toBe(0);
  });
});
