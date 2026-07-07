import chalk from 'chalk';
import type { EfficacyMetrics, TestResult } from '../../core/types.js';
import { ui } from './ui.js';

export interface EvalOutput {
  profile: string;
  topic: string;
  intent: 'block' | 'allow';
  metrics: {
    tp: number;
    tn: number;
    fp: number;
    fn: number;
    tpr: number;
    tnr: number;
    coverage: number;
    f1: number;
    total: number;
  };
  false_positives: Array<{ prompt: string; expected: boolean; actual: boolean }>;
  false_negatives: Array<{ prompt: string; expected: boolean; actual: boolean }>;
}

export function buildEvalOutput(
  profile: string,
  topic: string,
  intent: 'block' | 'allow',
  metrics: EfficacyMetrics,
  results: TestResult[],
): EvalOutput {
  const fps = results
    .filter((r) => !r.testCase.expectedTriggered && r.actualTriggered)
    .map((r) => ({ prompt: r.testCase.prompt, expected: false, actual: true }));

  const fns = results
    .filter((r) => r.testCase.expectedTriggered && !r.actualTriggered)
    .map((r) => ({ prompt: r.testCase.prompt, expected: true, actual: false }));

  return {
    profile,
    topic,
    intent,
    metrics: {
      tp: metrics.truePositives,
      tn: metrics.trueNegatives,
      fp: metrics.falsePositives,
      fn: metrics.falseNegatives,
      tpr: metrics.truePositiveRate,
      tnr: metrics.trueNegativeRate,
      coverage: metrics.coverage,
      f1: metrics.f1Score,
      total: results.length,
    },
    false_positives: fps,
    false_negatives: fns,
  };
}

export function renderEvalTerminal(output: EvalOutput): void {
  // Inline value coloring: coverage thresholds have no ui primitive.
  const coverageColor =
    output.metrics.coverage >= 0.9
      ? chalk.green
      : output.metrics.coverage >= 0.7
        ? chalk.yellow
        : chalk.red;

  ui.header('Eval Results');
  ui.keyValue([
    ['Profile', output.profile],
    ['Topic', output.topic],
    ['Intent', output.intent],
  ]);

  ui.section('Metrics:');
  ui.keyValue([
    ['Coverage', coverageColor(`${(output.metrics.coverage * 100).toFixed(1)}%`)],
    ['TPR', `${(output.metrics.tpr * 100).toFixed(1)}%`],
    ['TNR', `${(output.metrics.tnr * 100).toFixed(1)}%`],
    ['F1', output.metrics.f1.toFixed(3)],
  ]);
  ui.dim(
    `TP: ${output.metrics.tp}  TN: ${output.metrics.tn}  ` +
      `FP: ${output.metrics.fp}  FN: ${output.metrics.fn}  ` +
      `Total: ${output.metrics.total}`,
  );

  if (output.false_positives.length > 0) {
    ui.section('False Positives:');
    for (const fp of output.false_positives) {
      ui.bullet(fp.prompt, 'flag');
    }
  }

  if (output.false_negatives.length > 0) {
    ui.section('False Negatives:');
    for (const fn of output.false_negatives) {
      ui.bullet(fn.prompt, 'flag');
    }
  }

  console.log();
}
