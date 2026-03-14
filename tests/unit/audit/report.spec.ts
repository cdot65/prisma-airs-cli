import { describe, expect, it } from 'vitest';
import { buildAuditReportHtml, buildAuditReportJson } from '../../../src/audit/report.js';
import type { AuditResult } from '../../../src/audit/types.js';
import { mockMetrics } from '../../helpers/mocks.js';

function makeAuditResult(overrides: Partial<AuditResult> = {}): AuditResult {
  return {
    profileName: 'test-profile',
    timestamp: '2026-01-01T00:00:00Z',
    topics: [
      {
        topic: {
          topicId: 't1',
          topicName: 'Weapons',
          action: 'block',
          description: 'Block weapons',
          examples: ['gun talk'],
        },
        testResults: [],
        metrics: mockMetrics({ coverage: 0.85, accuracy: 0.9 }),
      },
    ],
    compositeMetrics: mockMetrics({ coverage: 0.85 }),
    conflicts: [],
    ...overrides,
  };
}

describe('buildAuditReportJson', () => {
  it('maps audit result to structured JSON', () => {
    const result = makeAuditResult();
    const json = buildAuditReportJson(result);

    expect(json.version).toBe(1);
    expect(json.profileName).toBe('test-profile');
    expect(json.compositeMetrics.coverage).toBe(0.85);
    expect(json.topics).toHaveLength(1);
    expect(json.topics[0].name).toBe('Weapons');
    expect(json.topics[0].action).toBe('block');
    expect(json.topics[0].metrics.coverage).toBe(0.85);
  });

  it('includes conflicts', () => {
    const result = makeAuditResult({
      conflicts: [
        {
          topicA: 'Weapons',
          topicB: 'Education',
          description: 'overlap',
          evidence: ['chem lab'],
        },
      ],
    });
    const json = buildAuditReportJson(result);
    expect(json.conflicts).toHaveLength(1);
    expect(json.conflicts[0].topicA).toBe('Weapons');
  });

  it('handles empty topics', () => {
    const result = makeAuditResult({ topics: [] });
    const json = buildAuditReportJson(result);
    expect(json.topics).toEqual([]);
  });

  it('preserves all zero metrics', () => {
    const zeroMetrics = mockMetrics({
      truePositives: 0,
      trueNegatives: 0,
      falsePositives: 0,
      falseNegatives: 0,
      truePositiveRate: 0,
      trueNegativeRate: 0,
      accuracy: 0,
      coverage: 0,
      f1Score: 0,
    });
    const result = makeAuditResult({
      compositeMetrics: zeroMetrics,
      topics: [
        {
          topic: {
            topicId: 't1',
            topicName: 'Empty',
            action: 'block',
            description: 'No data',
            examples: [],
          },
          testResults: [],
          metrics: zeroMetrics,
        },
      ],
    });
    const json = buildAuditReportJson(result);
    expect(json.compositeMetrics.coverage).toBe(0);
    expect(json.compositeMetrics.accuracy).toBe(0);
    expect(json.compositeMetrics.tpr).toBe(0);
    expect(json.compositeMetrics.tnr).toBe(0);
    expect(json.compositeMetrics.f1).toBe(0);
    expect(json.topics[0].metrics.tp).toBe(0);
    expect(json.topics[0].metrics.tn).toBe(0);
    expect(json.topics[0].metrics.fp).toBe(0);
    expect(json.topics[0].metrics.fn).toBe(0);
  });

  it('preserves conflict details including evidence', () => {
    const result = makeAuditResult({
      conflicts: [
        {
          topicA: 'Alpha',
          topicB: 'Beta',
          description: 'overlapping categories',
          evidence: ['prompt A', 'prompt B', 'prompt C'],
        },
        {
          topicA: 'Gamma',
          topicB: 'Delta',
          description: 'second conflict',
          evidence: [],
        },
      ],
    });
    const json = buildAuditReportJson(result);
    expect(json.conflicts).toHaveLength(2);
    expect(json.conflicts[0].topicA).toBe('Alpha');
    expect(json.conflicts[0].evidence).toEqual(['prompt A', 'prompt B', 'prompt C']);
    expect(json.conflicts[1].evidence).toEqual([]);
  });

  it('maps testCount from testResults length', () => {
    const result = makeAuditResult({
      topics: [
        {
          topic: {
            topicId: 't1',
            topicName: 'WithTests',
            action: 'block',
            description: 'desc',
            examples: [],
          },
          testResults: [
            {
              testCase: { prompt: 'p1', expectedTriggered: true, category: 'direct' },
              actualTriggered: true,
              scanAction: 'block',
              scanId: 's1',
              reportId: 'r1',
              correct: true,
            },
            {
              testCase: { prompt: 'p2', expectedTriggered: false, category: 'benign' },
              actualTriggered: false,
              scanAction: 'allow',
              scanId: 's2',
              reportId: 'r2',
              correct: true,
            },
          ],
          metrics: mockMetrics(),
        },
      ],
    });
    const json = buildAuditReportJson(result);
    expect(json.topics[0].testCount).toBe(2);
  });

  it('includes description from topic in JSON output', () => {
    const result = makeAuditResult();
    const json = buildAuditReportJson(result);
    expect(json.topics[0].description).toBe('Block weapons');
  });
});

describe('buildAuditReportHtml', () => {
  it('produces valid HTML document', () => {
    const html = buildAuditReportHtml(makeAuditResult());
    expect(html).toContain('<!DOCTYPE html>');
    expect(html).toContain('</html>');
    expect(html).toContain('<style>');
  });

  it('is self-contained', () => {
    const html = buildAuditReportHtml(makeAuditResult());
    expect(html).not.toMatch(/<link[^>]+href="http/);
    expect(html).not.toMatch(/<script[^>]+src="http/);
  });

  it('includes profile name and metrics', () => {
    const html = buildAuditReportHtml(makeAuditResult());
    expect(html).toContain('test-profile');
    expect(html).toContain('85.0%');
  });

  it('includes topic rows', () => {
    const html = buildAuditReportHtml(makeAuditResult());
    expect(html).toContain('Weapons');
    expect(html).toContain('block');
  });

  it('renders conflict section when present', () => {
    const result = makeAuditResult({
      conflicts: [
        {
          topicA: 'Weapons',
          topicB: 'Education',
          description: 'overlap detected',
          evidence: ['chem lab'],
        },
      ],
    });
    const html = buildAuditReportHtml(result);
    expect(html).toContain('Weapons');
    expect(html).toContain('Education');
    expect(html).toContain('overlap detected');
    expect(html).toContain('chem lab');
  });

  it('shows no conflicts message when empty', () => {
    const html = buildAuditReportHtml(makeAuditResult());
    expect(html).toContain('No cross-topic conflicts detected');
  });

  it('escapes HTML in topic names', () => {
    const result = makeAuditResult({
      topics: [
        {
          topic: {
            topicId: 't1',
            topicName: '<script>xss</script>',
            action: 'block',
            description: 'test',
            examples: [],
          },
          testResults: [],
          metrics: mockMetrics(),
        },
      ],
    });
    const html = buildAuditReportHtml(result);
    expect(html).not.toContain('<script>xss');
    expect(html).toContain('&lt;script&gt;');
  });

  it('escapes HTML entities in conflict descriptions and evidence', () => {
    const result = makeAuditResult({
      conflicts: [
        {
          topicA: '<b>Bold</b>',
          topicB: 'Normal & Safe',
          description: 'test <img onerror=alert(1)>',
          evidence: ['prompt with "quotes" & <tags>'],
        },
      ],
    });
    const html = buildAuditReportHtml(result);
    expect(html).not.toContain('<b>Bold</b>');
    expect(html).toContain('&lt;b&gt;Bold&lt;/b&gt;');
    expect(html).toContain('Normal &amp; Safe');
    expect(html).toContain('&lt;img onerror=alert(1)&gt;');
    expect(html).toContain('&amp; &lt;tags&gt;');
  });

  it('renders conflict section with multiple conflicts', () => {
    const result = makeAuditResult({
      conflicts: [
        { topicA: 'A', topicB: 'B', description: 'first', evidence: ['e1'] },
        { topicA: 'C', topicB: 'D', description: 'second', evidence: ['e2', 'e3'] },
      ],
    });
    const html = buildAuditReportHtml(result);
    expect(html).toContain('first');
    expect(html).toContain('second');
    expect(html).toContain('e2');
    expect(html).toContain('e3');
    // conflict count in summary
    expect(html).toContain('>2<');
  });

  it('renders 0.0% for zero metrics', () => {
    const result = makeAuditResult({
      compositeMetrics: mockMetrics({
        coverage: 0,
        accuracy: 0,
        truePositiveRate: 0,
        trueNegativeRate: 0,
      }),
    });
    const html = buildAuditReportHtml(result);
    expect(html).toContain('0.0%');
  });

  it('renders empty topic table body when no topics', () => {
    const result = makeAuditResult({ topics: [] });
    const html = buildAuditReportHtml(result);
    expect(html).toContain('<tbody></tbody>');
    expect(html).toContain('>0<'); // topics count = 0
  });

  it('escapes HTML in profile name in title', () => {
    const result = makeAuditResult({ profileName: 'test<script>' });
    const html = buildAuditReportHtml(result);
    expect(html).not.toContain('<script>');
    expect(html).toContain('test&lt;script&gt;');
  });
});
