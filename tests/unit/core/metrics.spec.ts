import { describe, expect, it } from 'vitest';
import { computeCategoryBreakdown, computeMetrics } from '../../../src/core/metrics.js';
import type { TestResult } from '../../../src/core/types.js';

function makeResult(
  expected: boolean,
  actual: boolean,
  opts?: { category?: string; source?: 'generated' | 'carried-fp' | 'carried-fn' | 'regression' },
): TestResult {
  return {
    testCase: {
      prompt: 'test',
      expectedTriggered: expected,
      category: opts?.category ?? 'test',
      source: opts?.source,
    },
    actualTriggered: actual,
    scanAction: actual ? 'block' : 'allow',
    scanId: 'scan-1',
    reportId: 'report-1',
    correct: expected === actual,
  };
}

describe('metrics', () => {
  describe('computeMetrics', () => {
    it('computes perfect scores for all-correct results', () => {
      const results: TestResult[] = [
        makeResult(true, true),
        makeResult(true, true),
        makeResult(false, false),
        makeResult(false, false),
      ];
      const m = computeMetrics(results);
      expect(m.truePositives).toBe(2);
      expect(m.trueNegatives).toBe(2);
      expect(m.falsePositives).toBe(0);
      expect(m.falseNegatives).toBe(0);
      expect(m.truePositiveRate).toBe(1);
      expect(m.trueNegativeRate).toBe(1);
      expect(m.accuracy).toBe(1);
      expect(m.coverage).toBe(1);
      expect(m.f1Score).toBe(1);
      expect(m.regressionCount).toBe(0);
    });

    it('computes zero scores for all-wrong results', () => {
      const results: TestResult[] = [
        makeResult(true, false),
        makeResult(true, false),
        makeResult(false, true),
        makeResult(false, true),
      ];
      const m = computeMetrics(results);
      expect(m.truePositives).toBe(0);
      expect(m.trueNegatives).toBe(0);
      expect(m.falsePositives).toBe(2);
      expect(m.falseNegatives).toBe(2);
      expect(m.truePositiveRate).toBe(0);
      expect(m.trueNegativeRate).toBe(0);
      expect(m.accuracy).toBe(0);
      expect(m.coverage).toBe(0);
      expect(m.f1Score).toBe(0);
      expect(m.regressionCount).toBe(0);
    });

    it('handles mixed results correctly', () => {
      const results: TestResult[] = [
        makeResult(true, true), // TP
        makeResult(true, false), // FN
        makeResult(false, false), // TN
        makeResult(false, true), // FP
      ];
      const m = computeMetrics(results);
      expect(m.truePositives).toBe(1);
      expect(m.trueNegatives).toBe(1);
      expect(m.falsePositives).toBe(1);
      expect(m.falseNegatives).toBe(1);
      expect(m.truePositiveRate).toBe(0.5);
      expect(m.trueNegativeRate).toBe(0.5);
      expect(m.accuracy).toBe(0.5);
      expect(m.coverage).toBe(0.5);
      expect(m.f1Score).toBe(0.5);
    });

    it('handles empty results', () => {
      const m = computeMetrics([]);
      expect(m.truePositives).toBe(0);
      expect(m.accuracy).toBe(0);
      expect(m.coverage).toBe(0);
      expect(m.f1Score).toBe(0);
      expect(m.regressionCount).toBe(0);
    });

    it('handles only positives (no negatives)', () => {
      const results: TestResult[] = [makeResult(true, true), makeResult(true, true)];
      const m = computeMetrics(results);
      expect(m.truePositiveRate).toBe(1);
      expect(m.trueNegativeRate).toBe(0);
      expect(m.coverage).toBe(0); // min(TPR, TNR)
    });

    it('handles only negatives (no positives)', () => {
      const results: TestResult[] = [makeResult(false, false), makeResult(false, false)];
      const m = computeMetrics(results);
      expect(m.truePositiveRate).toBe(0);
      expect(m.trueNegativeRate).toBe(1);
      expect(m.coverage).toBe(0); // min(TPR, TNR)
    });

    it('computes F1 correctly for imbalanced results', () => {
      // 3 TP, 0 FN, 0 FP, 7 TN
      const results: TestResult[] = [
        ...Array(3)
          .fill(null)
          .map(() => makeResult(true, true)),
        ...Array(7)
          .fill(null)
          .map(() => makeResult(false, false)),
      ];
      const m = computeMetrics(results);
      expect(m.truePositives).toBe(3);
      expect(m.trueNegatives).toBe(7);
      expect(m.falsePositives).toBe(0);
      expect(m.falseNegatives).toBe(0);
      expect(m.accuracy).toBe(1);
      expect(m.f1Score).toBe(1);
    });

    it('counts regressions from regression-sourced failures', () => {
      const results: TestResult[] = [
        makeResult(true, true, { source: 'regression' }), // regression TP — not a regression
        makeResult(true, false, { source: 'regression' }), // regression FN — IS a regression
        makeResult(false, true, { source: 'regression' }), // regression FP — IS a regression
        makeResult(false, false, { source: 'generated' }), // generated TN
      ];
      const m = computeMetrics(results);
      expect(m.regressionCount).toBe(2);
    });

    it('returns zero regressions when no regression-sourced tests', () => {
      const results: TestResult[] = [
        makeResult(true, false, { source: 'generated' }),
        makeResult(false, true, { source: 'carried-fp' }),
      ];
      const m = computeMetrics(results);
      expect(m.regressionCount).toBe(0);
    });

    it('returns zero regressions when all regression tests pass', () => {
      const results: TestResult[] = [
        makeResult(true, true, { source: 'regression' }),
        makeResult(false, false, { source: 'regression' }),
      ];
      const m = computeMetrics(results);
      expect(m.regressionCount).toBe(0);
    });
  });

  describe('computeCategoryBreakdown', () => {
    it('groups results by category', () => {
      const results: TestResult[] = [
        makeResult(true, true, { category: 'direct' }),
        makeResult(true, false, { category: 'direct' }),
        makeResult(false, false, { category: 'benign' }),
        makeResult(false, true, { category: 'benign' }),
      ];
      const breakdown = computeCategoryBreakdown(results);
      expect(breakdown).toHaveLength(2);

      const direct = breakdown.find((b) => b.category === 'direct');
      expect(direct).toEqual({ category: 'direct', total: 2, fp: 0, fn: 1, errorRate: 0.5 });

      const benign = breakdown.find((b) => b.category === 'benign');
      expect(benign).toEqual({ category: 'benign', total: 2, fp: 1, fn: 0, errorRate: 0.5 });
    });

    it('returns empty array for empty results', () => {
      expect(computeCategoryBreakdown([])).toEqual([]);
    });

    it('sorts by error rate descending', () => {
      const results: TestResult[] = [
        makeResult(true, true, { category: 'good' }), // 0% error
        makeResult(true, false, { category: 'bad' }), // 100% error
        makeResult(true, true, { category: 'mixed' }),
        makeResult(true, false, { category: 'mixed' }), // 50% error
      ];
      const breakdown = computeCategoryBreakdown(results);
      expect(breakdown[0].category).toBe('bad');
      expect(breakdown[1].category).toBe('mixed');
      expect(breakdown[2].category).toBe('good');
    });

    it('computes error rate correctly with all correct', () => {
      const results: TestResult[] = [
        makeResult(true, true, { category: 'direct' }),
        makeResult(false, false, { category: 'direct' }),
      ];
      const breakdown = computeCategoryBreakdown(results);
      expect(breakdown[0].errorRate).toBe(0);
      expect(breakdown[0].fp).toBe(0);
      expect(breakdown[0].fn).toBe(0);
    });
  });
});
