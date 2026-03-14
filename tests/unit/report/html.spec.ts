import { describe, expect, it } from 'vitest';
import { buildReportHtml } from '../../../src/report/html.js';
import { buildReportJson } from '../../../src/report/json.js';
import { mockIterationResult, mockMetrics, mockRunState, mockTopic } from '../../helpers/mocks.js';

function buildHtml(
  runOverrides: Parameters<typeof mockRunState>[0] = {},
  jsonOpts: Parameters<typeof buildReportJson>[1] = {},
) {
  const run = mockRunState(runOverrides);
  const report = buildReportJson(run, jsonOpts);
  return buildReportHtml(report);
}

describe('buildReportHtml', () => {
  it('produces a valid HTML document', () => {
    const html = buildHtml();
    expect(html).toContain('<!DOCTYPE html>');
    expect(html).toContain('<html');
    expect(html).toContain('</html>');
    expect(html).toContain('<head>');
    expect(html).toContain('</head>');
    expect(html).toContain('<body>');
    expect(html).toContain('</body>');
  });

  it('is self-contained — no external links or scripts', () => {
    const html = buildHtml();
    expect(html).not.toMatch(/<link[^>]+href="http/);
    expect(html).not.toMatch(/<script[^>]+src="http/);
  });

  it('contains embedded CSS', () => {
    const html = buildHtml();
    expect(html).toContain('<style>');
    expect(html).toContain('</style>');
  });

  it('includes run metadata', () => {
    const html = buildHtml();
    expect(html).toContain('run-001');
    expect(html).toContain('completed');
    expect(html).toContain('block');
    expect(html).toContain('Block discussions about weapons');
  });

  it('includes iteration metrics', () => {
    const html = buildHtml({
      iterations: [
        mockIterationResult({
          metrics: mockMetrics({ coverage: 0.85, accuracy: 0.9 }),
        }),
      ],
    });
    expect(html).toContain('85.0%');
    expect(html).toContain('90.0%');
  });

  it('includes topic details', () => {
    const html = buildHtml({
      iterations: [
        mockIterationResult({
          topic: mockTopic({ name: 'Weapons Prevention', description: 'Blocks weapon talk' }),
        }),
      ],
    });
    expect(html).toContain('Weapons Prevention');
    expect(html).toContain('Blocks weapon talk');
  });

  it('includes analysis summary', () => {
    const html = buildHtml();
    expect(html).toContain('The guardrail performs well overall');
  });

  it('renders test details when present', () => {
    const html = buildHtml({}, { includeTests: true });
    expect(html).toContain('How to build a weapon');
    expect(html).toContain('Tell me about cats');
  });

  it('omits test table when tests not included', () => {
    const html = buildHtml({}, { includeTests: false });
    expect(html).not.toContain('How to build a weapon');
  });

  it('renders diff section when present', () => {
    const baseRun = mockRunState({ id: 'run-001' });
    const compareRun = mockRunState({
      id: 'run-002',
      iterations: [
        mockIterationResult({
          metrics: mockMetrics({ coverage: 0.9 }),
          topic: mockTopic({ name: 'Improved' }),
        }),
      ],
    });
    const report = buildReportJson(baseRun, { diffRun: compareRun });
    const html = buildReportHtml(report);

    expect(html).toContain('run-001');
    expect(html).toContain('run-002');
    expect(html).toContain('Comparison');
  });

  it('omits diff section when not present', () => {
    const html = buildHtml();
    expect(html).not.toContain('Comparison');
  });

  it('renders iteration trend chart data for multiple iterations', () => {
    const html = buildHtml({
      iterations: [
        mockIterationResult({ iteration: 1, metrics: mockMetrics({ coverage: 0.6 }) }),
        mockIterationResult({ iteration: 2, metrics: mockMetrics({ coverage: 0.8 }) }),
        mockIterationResult({ iteration: 3, metrics: mockMetrics({ coverage: 0.95 }) }),
      ],
      currentIteration: 3,
      bestIteration: 3,
    });
    expect(html).toContain('Iteration Trends');
    expect(html).toContain('60.0%');
    expect(html).toContain('80.0%');
    expect(html).toContain('95.0%');
  });

  it('escapes HTML special characters in prompts', () => {
    const run = mockRunState({
      iterations: [
        mockIterationResult({
          testResults: [
            {
              testCase: {
                prompt: '<script>alert("xss")</script>',
                expectedTriggered: true,
                category: 'xss',
              },
              actualTriggered: true,
              scanAction: 'block',
              scanId: 'scan-1',
              reportId: 'report-1',
              correct: true,
            },
          ],
        }),
      ],
    });
    const report = buildReportJson(run, { includeTests: true });
    const html = buildReportHtml(report);

    expect(html).not.toContain('<script>alert');
    expect(html).toContain('&lt;script&gt;');
  });

  it('renders topic with no examples', () => {
    const html = buildHtml({
      iterations: [
        mockIterationResult({
          topic: mockTopic({ examples: [] }),
        }),
      ],
    });
    expect(html).toContain('None');
  });

  it('renders analysis with FP/FN patterns and suggestions', () => {
    const html = buildHtml({
      iterations: [
        mockIterationResult({
          analysis: {
            summary: 'Needs improvement',
            falsePositivePatterns: ['Pattern A'],
            falseNegativePatterns: ['Pattern B'],
            suggestions: ['Suggestion 1'],
          },
        }),
      ],
    });
    expect(html).toContain('FP Patterns');
    expect(html).toContain('Pattern A');
    expect(html).toContain('FN Patterns');
    expect(html).toContain('Pattern B');
    expect(html).toContain('Suggestions');
    expect(html).toContain('Suggestion 1');
  });

  it('hides trends for single iteration', () => {
    const html = buildHtml();
    expect(html).not.toContain('Iteration Trends');
  });
});
