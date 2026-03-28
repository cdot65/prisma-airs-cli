import chalk from 'chalk';
import type { EfficacyMetrics, TestResult } from '../../core/types.js';

export interface EvalOutput {
  profile: string;
  topic: string;
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
  const coverageColor =
    output.metrics.coverage >= 0.9
      ? chalk.green
      : output.metrics.coverage >= 0.7
        ? chalk.yellow
        : chalk.red;

  console.log(chalk.bold('\n  Eval Results'));
  console.log(chalk.dim('  ─────────────────────────'));
  console.log(`  Profile: ${chalk.white(output.profile)}`);
  console.log(`  Topic:   ${chalk.white(output.topic)}`);

  console.log(chalk.bold('\n  Metrics:'));
  console.log(`    Coverage:  ${coverageColor(`${(output.metrics.coverage * 100).toFixed(1)}%`)}`);
  console.log(`    TPR:       ${(output.metrics.tpr * 100).toFixed(1)}%`);
  console.log(`    TNR:       ${(output.metrics.tnr * 100).toFixed(1)}%`);
  console.log(`    F1:        ${output.metrics.f1.toFixed(3)}`);
  console.log(
    chalk.dim(
      `    TP: ${output.metrics.tp}  TN: ${output.metrics.tn}  ` +
        `FP: ${output.metrics.fp}  FN: ${output.metrics.fn}  ` +
        `Total: ${output.metrics.total}`,
    ),
  );

  if (output.false_positives.length > 0) {
    console.log(chalk.bold.yellow('\n  False Positives:'));
    for (const fp of output.false_positives) {
      console.log(`    ${chalk.yellow('●')} ${fp.prompt}`);
    }
  }

  if (output.false_negatives.length > 0) {
    console.log(chalk.bold.red('\n  False Negatives:'));
    for (const fn of output.false_negatives) {
      console.log(`    ${chalk.red('●')} ${fn.prompt}`);
    }
  }

  console.log();
}
