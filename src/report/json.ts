/**
 * JSON report builder — pure function mapping RunState to structured ReportOutput.
 */

import type { RunState } from '../core/types.js';
import type { IterationSummary, ReportOutput, RunDiff, TestDetail } from './types.js';

export interface BuildReportOptions {
  includeTests?: boolean;
  diffRun?: RunState | null;
}

export function buildReportJson(run: RunState, options: BuildReportOptions = {}): ReportOutput {
  const iterations: IterationSummary[] = run.iterations.map((iter) => {
    const summary: IterationSummary = {
      iteration: iter.iteration,
      timestamp: iter.timestamp,
      durationMs: iter.durationMs,
      topic: iter.topic,
      metrics: iter.metrics,
      analysis: iter.analysis,
    };

    if (options.includeTests) {
      summary.tests = iter.testResults.map((r): TestDetail => {
        const detail: TestDetail = {
          prompt: r.testCase.prompt,
          expectedTriggered: r.testCase.expectedTriggered,
          actualTriggered: r.actualTriggered,
          correct: r.correct,
          category: r.testCase.category,
          scanAction: r.scanAction,
        };
        if (r.testCase.source) {
          detail.source = r.testCase.source;
        }
        return detail;
      });
    }

    return summary;
  });

  const report: ReportOutput = {
    version: 1,
    generatedAt: new Date().toISOString(),
    run: {
      id: run.id,
      createdAt: run.createdAt,
      status: run.status,
      intent: run.userInput.intent,
      topicDescription: run.userInput.topicDescription,
      profileName: run.userInput.profileName,
      bestIteration: run.bestIteration,
      bestCoverage: run.bestCoverage,
      totalIterations: run.iterations.length,
    },
    iterations,
  };

  if (options.diffRun) {
    report.diff = buildDiff(run, options.diffRun);
  }

  return report;
}

function buildDiff(baseRun: RunState, compareRun: RunState): RunDiff {
  const baseIter = baseRun.iterations[baseRun.bestIteration - 1];
  const compareIter = compareRun.iterations[compareRun.bestIteration - 1];

  const baseMetrics = baseIter.metrics;
  const compareMetrics = compareIter.metrics;

  return {
    baseRunId: baseRun.id,
    compareRunId: compareRun.id,
    metricsDelta: {
      coverage: compareMetrics.coverage - baseMetrics.coverage,
      tpr: compareMetrics.truePositiveRate - baseMetrics.truePositiveRate,
      tnr: compareMetrics.trueNegativeRate - baseMetrics.trueNegativeRate,
      accuracy: compareMetrics.accuracy - baseMetrics.accuracy,
      f1: compareMetrics.f1Score - baseMetrics.f1Score,
    },
    baseMetrics,
    compareMetrics,
    baseTopic: baseIter.topic,
    compareTopic: compareIter.topic,
  };
}
