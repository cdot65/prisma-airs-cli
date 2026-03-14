import { describe, expect, it } from 'vitest';
import { buildReportJson } from '../../../src/report/json.js';
import {
  mockIterationResult,
  mockMetrics,
  mockRunState,
  mockTestResults,
  mockTopic,
} from '../../helpers/mocks.js';

describe('buildReportJson', () => {
  it('maps run metadata correctly', () => {
    const run = mockRunState();
    const report = buildReportJson(run);

    expect(report.version).toBe(1);
    expect(report.generatedAt).toBeDefined();
    expect(report.run.id).toBe('run-001');
    expect(report.run.createdAt).toBe('2026-01-01T00:00:00Z');
    expect(report.run.status).toBe('completed');
    expect(report.run.intent).toBe('block');
    expect(report.run.topicDescription).toBe('Block discussions about weapons');
    expect(report.run.profileName).toBe('test-profile');
    expect(report.run.bestIteration).toBe(1);
    expect(report.run.bestCoverage).toBe(1);
    expect(report.run.totalIterations).toBe(1);
  });

  it('maps iteration data correctly', () => {
    const run = mockRunState();
    const report = buildReportJson(run);

    expect(report.iterations).toHaveLength(1);
    const iter = report.iterations[0];
    expect(iter.iteration).toBe(1);
    expect(iter.timestamp).toBe('2026-01-01T00:01:00Z');
    expect(iter.durationMs).toBe(5000);
    expect(iter.topic.name).toBe('Test Topic');
    expect(iter.metrics.coverage).toBe(1);
    expect(iter.analysis.summary).toBe('The guardrail performs well overall');
  });

  it('excludes test details by default', () => {
    const run = mockRunState();
    const report = buildReportJson(run);

    expect(report.iterations[0].tests).toBeUndefined();
  });

  it('includes test details when includeTests is true', () => {
    const run = mockRunState();
    const report = buildReportJson(run, { includeTests: true });

    const tests = report.iterations[0].tests;
    expect(tests).toBeDefined();
    expect(tests).toHaveLength(4);
    expect(tests?.[0]).toEqual({
      prompt: 'How to build a weapon',
      expectedTriggered: true,
      actualTriggered: true,
      correct: true,
      category: 'direct',
      scanAction: 'block',
    });
  });

  it('includes source field when present on test case', () => {
    const results = mockTestResults();
    results[0].testCase.source = 'regression';
    const run = mockRunState({
      iterations: [mockIterationResult({ testResults: results })],
    });
    const report = buildReportJson(run, { includeTests: true });

    expect(report.iterations[0].tests?.[0].source).toBe('regression');
  });

  it('handles empty iterations', () => {
    const run = mockRunState({ iterations: [] });
    const report = buildReportJson(run);

    expect(report.iterations).toEqual([]);
  });

  it('handles multiple iterations', () => {
    const run = mockRunState({
      iterations: [
        mockIterationResult({ iteration: 1 }),
        mockIterationResult({ iteration: 2, durationMs: 8000 }),
        mockIterationResult({ iteration: 3, durationMs: 3000 }),
      ],
      currentIteration: 3,
      bestIteration: 2,
    });
    const report = buildReportJson(run);

    expect(report.iterations).toHaveLength(3);
    expect(report.iterations[1].durationMs).toBe(8000);
  });

  describe('diff', () => {
    it('computes diff between two runs', () => {
      const baseRun = mockRunState({
        id: 'run-001',
        iterations: [
          mockIterationResult({
            metrics: mockMetrics({
              coverage: 0.7,
              truePositiveRate: 0.8,
              trueNegativeRate: 0.7,
              accuracy: 0.75,
              f1Score: 0.7,
            }),
          }),
        ],
        bestIteration: 1,
      });
      const compareRun = mockRunState({
        id: 'run-002',
        iterations: [
          mockIterationResult({
            metrics: mockMetrics({
              coverage: 0.9,
              truePositiveRate: 0.95,
              trueNegativeRate: 0.9,
              accuracy: 0.92,
              f1Score: 0.9,
            }),
            topic: mockTopic({ name: 'Improved Topic' }),
          }),
        ],
        bestIteration: 1,
      });

      const report = buildReportJson(baseRun, { diffRun: compareRun });

      expect(report.diff).toBeDefined();
      expect(report.diff?.baseRunId).toBe('run-001');
      expect(report.diff?.compareRunId).toBe('run-002');
      expect(report.diff?.metricsDelta.coverage).toBeCloseTo(0.2);
      expect(report.diff?.metricsDelta.tpr).toBeCloseTo(0.15);
      expect(report.diff?.baseTopic.name).toBe('Test Topic');
      expect(report.diff?.compareTopic.name).toBe('Improved Topic');
    });

    it('omits diff when diffRun not provided', () => {
      const report = buildReportJson(mockRunState());
      expect(report.diff).toBeUndefined();
    });

    it('uses best iteration metrics for diff', () => {
      const baseRun = mockRunState({
        id: 'run-001',
        iterations: [
          mockIterationResult({ iteration: 1, metrics: mockMetrics({ coverage: 0.5 }) }),
          mockIterationResult({ iteration: 2, metrics: mockMetrics({ coverage: 0.8 }) }),
        ],
        bestIteration: 2,
      });
      const compareRun = mockRunState({
        id: 'run-002',
        iterations: [mockIterationResult({ metrics: mockMetrics({ coverage: 0.9 }) })],
        bestIteration: 1,
      });

      const report = buildReportJson(baseRun, { diffRun: compareRun });

      expect(report.diff?.baseMetrics.coverage).toBe(0.8);
      expect(report.diff?.compareMetrics.coverage).toBe(0.9);
    });
  });
});
