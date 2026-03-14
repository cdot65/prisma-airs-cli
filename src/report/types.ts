/**
 * Structured evaluation report types — JSON/HTML export for run results.
 */

import type { AnalysisReport, CustomTopic, EfficacyMetrics } from '../core/types.js';

/** Top-level report output structure. */
export interface ReportOutput {
  version: 1;
  generatedAt: string;
  run: RunSummary;
  iterations: IterationSummary[];
  diff?: RunDiff;
}

export interface RunSummary {
  id: string;
  createdAt: string;
  status: string;
  intent: 'allow' | 'block';
  topicDescription: string;
  profileName: string;
  bestIteration: number;
  bestCoverage: number;
  totalIterations: number;
}

export interface IterationSummary {
  iteration: number;
  timestamp: string;
  durationMs: number;
  topic: CustomTopic;
  metrics: EfficacyMetrics;
  analysis: AnalysisReport;
  tests?: TestDetail[];
}

export interface TestDetail {
  prompt: string;
  expectedTriggered: boolean;
  actualTriggered: boolean;
  correct: boolean;
  category: string;
  source?: string;
  scanAction: 'allow' | 'block';
}

export interface RunDiff {
  baseRunId: string;
  compareRunId: string;
  metricsDelta: MetricsDelta;
  baseMetrics: EfficacyMetrics;
  compareMetrics: EfficacyMetrics;
  baseTopic: CustomTopic;
  compareTopic: CustomTopic;
}

export interface MetricsDelta {
  coverage: number;
  tpr: number;
  tnr: number;
  accuracy: number;
  f1: number;
}
