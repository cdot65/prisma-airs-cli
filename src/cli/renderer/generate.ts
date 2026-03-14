import chalk from 'chalk';
import type {
  AnalysisReport,
  CustomTopic,
  EfficacyMetrics,
  IterationResult,
  RunState,
  TestResult,
} from '../../core/types.js';
import type { RunStateSummary } from '../../persistence/types.js';

/** Render the application banner. */
export function renderHeader(): void {
  console.log(chalk.bold.cyan('\n  Prisma AIRS Guardrail Generator'));
  console.log(chalk.dim('  Iterative custom topic refinement\n'));
}

/** Render iteration number header. */
export function renderIterationStart(iteration: number): void {
  console.log(chalk.bold(`\n━━━ Iteration ${iteration} ━━━`));
}

/** Render a topic's name, description, and examples. */
export function renderTopic(topic: CustomTopic): void {
  console.log(chalk.bold('  Topic:'));
  console.log(`    Name: ${chalk.white(topic.name)}`);
  console.log(`    Desc: ${chalk.white(topic.description)}`);
  console.log('    Examples:');
  for (const ex of topic.examples) {
    console.log(`      ${chalk.dim('•')} ${ex}`);
  }
}

/** Render companion allow topic info (two-phase generation). */
export function renderCompanionTopic(topic: CustomTopic): void {
  console.log(chalk.bold('  Companion Allow Topic:'));
  console.log(`    Name: ${chalk.white(topic.name)}`);
  console.log(`    Desc: ${chalk.white(topic.description)}`);
}

/** Render a scan progress bar with percentage. */
export function renderTestProgress(completed: number, total: number): void {
  const pct = Math.round((completed / total) * 100);
  const bar = '█'.repeat(Math.round(pct / 5)) + '░'.repeat(20 - Math.round(pct / 5));
  process.stdout.write(`\r  Scanning: ${bar} ${pct}% (${completed}/${total})`);
  if (completed === total) console.log();
}

/** Render efficacy metrics with color-coded coverage. */
export function renderMetrics(metrics: EfficacyMetrics): void {
  const coverageColor =
    metrics.coverage >= 0.9 ? chalk.green : metrics.coverage >= 0.7 ? chalk.yellow : chalk.red;

  console.log(chalk.bold('\n  Metrics:'));
  console.log(`    Coverage:  ${coverageColor(`${(metrics.coverage * 100).toFixed(1)}%`)}`);
  console.log(`    Accuracy:  ${(metrics.accuracy * 100).toFixed(1)}%`);
  console.log(`    TPR:       ${(metrics.truePositiveRate * 100).toFixed(1)}%`);
  console.log(`    TNR:       ${(metrics.trueNegativeRate * 100).toFixed(1)}%`);
  console.log(`    F1 Score:  ${metrics.f1Score.toFixed(3)}`);
  let countsLine = `    TP: ${metrics.truePositives}  TN: ${metrics.trueNegatives}  FP: ${metrics.falsePositives}  FN: ${metrics.falseNegatives}`;
  if (metrics.regressionCount > 0) {
    countsLine += chalk.red(`  Regressions: ${metrics.regressionCount}`);
  }
  console.log(chalk.dim(countsLine));
}

/** Render analysis summary with FP/FN pattern details. */
export function renderAnalysis(analysis: AnalysisReport): void {
  console.log(chalk.bold('\n  Analysis:'));
  console.log(`    ${analysis.summary}`);
  if (analysis.falsePositivePatterns.length > 0) {
    console.log(chalk.yellow('    FP patterns:'));
    for (const p of analysis.falsePositivePatterns) {
      console.log(`      ${chalk.dim('•')} ${p}`);
    }
  }
  if (analysis.falseNegativePatterns.length > 0) {
    console.log(chalk.red('    FN patterns:'));
    for (const p of analysis.falseNegativePatterns) {
      console.log(`      ${chalk.dim('•')} ${p}`);
    }
  }
}

/** Render per-test-case results table. */
export function renderTestResults(results: TestResult[]): void {
  console.log(chalk.bold('\n  Test Results:'));
  for (const r of results) {
    const icon = r.correct ? chalk.green('✓') : chalk.red('✗');
    const expected = r.testCase.expectedTriggered ? 'triggered' : 'safe';
    const actual = r.actualTriggered ? 'triggered' : 'safe';
    const status = r.correct ? '' : chalk.red(` (expected ${expected}, got ${actual})`);
    console.log(`    ${icon} ${r.testCase.prompt}${status}`);
  }
}

/** Render loop completion summary with best iteration and run ID. */
export function renderLoopComplete(runState: RunState): void {
  const best = runState.iterations[runState.bestIteration - 1];
  console.log(chalk.bold.green('\n━━━ Complete ━━━'));
  console.log(
    `  Best iteration: ${runState.bestIteration} (coverage: ${(runState.bestCoverage * 100).toFixed(1)}%)`,
  );
  console.log(`  Total iterations: ${runState.iterations.length}`);
  if (best) {
    renderTopic(best.topic);
  }
  console.log(`\n  Run ID: ${chalk.dim(runState.id)}\n`);
}

/** Render a list of saved runs with status, coverage, and topic description. */
export function renderRunList(runs: RunStateSummary[]): void {
  if (runs.length === 0) {
    console.log(chalk.dim('  No saved runs found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Saved Runs:\n'));
  for (const run of runs) {
    const statusColor =
      run.status === 'completed'
        ? chalk.green
        : run.status === 'running'
          ? chalk.blue
          : run.status === 'paused'
            ? chalk.yellow
            : chalk.red;
    console.log(`  ${chalk.dim(run.id)}`);
    console.log(
      `    Status: ${statusColor(run.status)}  Coverage: ${(run.bestCoverage * 100).toFixed(1)}%  Iterations: ${run.currentIteration}`,
    );
    console.log(`    Topic: ${run.topicDescription}`);
    console.log(`    Created: ${run.createdAt}\n`);
  }
}

/** Render a one-line iteration summary with duration and coverage. */
export function renderIterationSummary(result: IterationResult): void {
  const coverageColor =
    result.metrics.coverage >= 0.9
      ? chalk.green
      : result.metrics.coverage >= 0.7
        ? chalk.yellow
        : chalk.red;
  console.log(
    `  ${chalk.dim(`[${result.durationMs}ms]`)} Coverage: ${coverageColor(`${(result.metrics.coverage * 100).toFixed(1)}%`)} | Accuracy: ${(result.metrics.accuracy * 100).toFixed(1)}%`,
  );
}

/** Render memory loading status (count of learnings loaded or "none found"). */
export function renderMemoryLoaded(learningCount: number): void {
  if (learningCount > 0) {
    console.log(chalk.cyan(`  Memory: loaded ${learningCount} learnings from previous runs`));
  } else {
    console.log(chalk.dim('  Memory: no previous learnings found'));
  }
}

/** Render count of learnings extracted from the current run. */
export function renderMemoryExtracted(learningCount: number): void {
  console.log(chalk.cyan(`  Memory: extracted ${learningCount} learnings from this run`));
}
