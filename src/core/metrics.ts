import type { CategoryBreakdown, EfficacyMetrics, TestResult } from './types.js';

/**
 * Classify test results into TP/TN/FP/FN and compute efficacy metrics.
 *
 * @param results - Scan results paired with their expected outcomes.
 * @returns Confusion matrix counts, rates (TPR, TNR, accuracy, coverage, F1),
 *          and regression failure count. Division-by-zero yields 0.
 *          Coverage = min(TPR, TNR) — the loop's stop condition target.
 */
export function computeMetrics(results: TestResult[]): EfficacyMetrics {
  let truePositives = 0;
  let trueNegatives = 0;
  let falsePositives = 0;
  let falseNegatives = 0;
  let regressionCount = 0;

  for (const r of results) {
    if (r.testCase.expectedTriggered && r.actualTriggered) truePositives++;
    else if (!r.testCase.expectedTriggered && !r.actualTriggered) trueNegatives++;
    else if (!r.testCase.expectedTriggered && r.actualTriggered) falsePositives++;
    else if (r.testCase.expectedTriggered && !r.actualTriggered) falseNegatives++;

    if (r.testCase.source === 'regression' && !r.correct) regressionCount++;
  }

  const totalPositives = truePositives + falseNegatives;
  const totalNegatives = trueNegatives + falsePositives;
  const total = results.length;

  const truePositiveRate = totalPositives > 0 ? truePositives / totalPositives : 0;
  const trueNegativeRate = totalNegatives > 0 ? trueNegatives / totalNegatives : 0;
  const accuracy = total > 0 ? (truePositives + trueNegatives) / total : 0;
  const coverage = Math.min(truePositiveRate, trueNegativeRate);

  const precision =
    truePositives + falsePositives > 0 ? truePositives / (truePositives + falsePositives) : 0;
  const recall = truePositiveRate;
  const f1Score = precision + recall > 0 ? (2 * precision * recall) / (precision + recall) : 0;

  return {
    truePositives,
    trueNegatives,
    falsePositives,
    falseNegatives,
    truePositiveRate,
    trueNegativeRate,
    accuracy,
    coverage,
    f1Score,
    regressionCount,
  };
}

/** Compute per-category error breakdown from test results. Sorted by error rate descending. */
export function computeCategoryBreakdown(results: TestResult[]): CategoryBreakdown[] {
  const map = new Map<string, { total: number; fp: number; fn: number }>();

  for (const r of results) {
    const cat = r.testCase.category;
    const entry = map.get(cat) ?? { total: 0, fp: 0, fn: 0 };
    entry.total++;
    if (!r.testCase.expectedTriggered && r.actualTriggered) entry.fp++;
    if (r.testCase.expectedTriggered && !r.actualTriggered) entry.fn++;
    map.set(cat, entry);
  }

  return [...map.entries()]
    .map(([category, { total, fp, fn }]) => ({
      category,
      total,
      fp,
      fn,
      errorRate: total > 0 ? (fp + fn) / total : 0,
    }))
    .sort((a, b) => b.errorRate - a.errorRate);
}
