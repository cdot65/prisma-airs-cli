import { describe, expect, it } from 'vitest';
import {
  computeCompositeMetrics,
  computeTopicAuditResults,
  detectConflicts,
  groupResultsByTopic,
} from '../../../src/audit/evaluator.js';
import type { ProfileTopic } from '../../../src/audit/types.js';
import type { TestResult } from '../../../src/core/types.js';

function makeResult(
  targetTopic: string,
  expected: boolean,
  actual: boolean,
  prompt = 'test prompt',
): TestResult {
  return {
    testCase: {
      prompt,
      expectedTriggered: expected,
      category: 'test',
      targetTopic,
    },
    actualTriggered: actual,
    scanAction: actual ? 'block' : 'allow',
    scanId: 'scan-1',
    reportId: 'report-1',
    correct: expected === actual,
  };
}

const topicA: ProfileTopic = {
  topicId: 't1',
  topicName: 'Weapons',
  action: 'block',
  description: 'Block weapon discussions',
  examples: ['how to build a gun'],
};

const topicB: ProfileTopic = {
  topicId: 't2',
  topicName: 'Education',
  action: 'allow',
  description: 'Allow educational content',
  examples: ['teach me math'],
};

describe('groupResultsByTopic', () => {
  it('returns empty map for empty results', () => {
    const groups = groupResultsByTopic([]);
    expect(groups.size).toBe(0);
  });

  it('groups results by targetTopic', () => {
    const results = [
      makeResult('Weapons', true, true),
      makeResult('Weapons', true, false),
      makeResult('Education', false, false),
    ];
    const groups = groupResultsByTopic(results);
    expect(groups.size).toBe(2);
    expect(groups.get('Weapons')).toHaveLength(2);
    expect(groups.get('Education')).toHaveLength(1);
  });

  it('groups results without targetTopic under "unknown"', () => {
    const result: TestResult = {
      testCase: { prompt: 'test', expectedTriggered: true, category: 'test' },
      actualTriggered: true,
      scanAction: 'block',
      scanId: 's1',
      reportId: 'r1',
      correct: true,
    };
    const groups = groupResultsByTopic([result]);
    expect(groups.get('unknown')).toHaveLength(1);
  });
});

describe('computeTopicAuditResults', () => {
  it('returns empty array when no results', () => {
    const results = computeTopicAuditResults([], [topicA]);
    expect(results).toHaveLength(1);
    expect(results[0].testResults).toHaveLength(0);
    expect(results[0].metrics.accuracy).toBe(0);
  });

  it('computes per-topic metrics correctly', () => {
    const results = [
      makeResult('Weapons', true, true),
      makeResult('Weapons', true, true),
      makeResult('Weapons', false, false),
      makeResult('Education', false, false),
      makeResult('Education', false, true), // FP
    ];
    const audit = computeTopicAuditResults(results, [topicA, topicB]);

    const weapons = audit.find((r) => r.topic.topicName === 'Weapons');
    expect(weapons?.metrics.truePositives).toBe(2);
    expect(weapons?.metrics.trueNegatives).toBe(1);
    expect(weapons?.metrics.accuracy).toBe(1);

    const edu = audit.find((r) => r.topic.topicName === 'Education');
    expect(edu?.metrics.falsePositives).toBe(1);
    expect(edu?.metrics.trueNegatives).toBe(1);
  });

  it('includes topics with no test results', () => {
    const results = [makeResult('Weapons', true, true)];
    const audit = computeTopicAuditResults(results, [topicA, topicB]);
    expect(audit).toHaveLength(2);
    const edu = audit.find((r) => r.topic.topicName === 'Education');
    expect(edu?.testResults).toHaveLength(0);
  });
});

describe('computeCompositeMetrics', () => {
  it('aggregates metrics across all topics', () => {
    const results = [
      makeResult('Weapons', true, true),
      makeResult('Weapons', false, false),
      makeResult('Education', true, false), // FN
      makeResult('Education', false, false),
    ];
    const audit = computeTopicAuditResults(results, [topicA, topicB]);
    const composite = computeCompositeMetrics(audit);

    expect(composite.truePositives).toBe(1);
    expect(composite.trueNegatives).toBe(2);
    expect(composite.falseNegatives).toBe(1);
    expect(composite.accuracy).toBe(0.75);
  });

  it('returns zero metrics for empty results', () => {
    const composite = computeCompositeMetrics([]);
    expect(composite.accuracy).toBe(0);
  });
});

describe('detectConflicts', () => {
  it('returns empty array when no conflicts', () => {
    const results = [
      makeResult('Weapons', true, true, 'how to make a bomb'),
      makeResult('Weapons', false, false, 'tell me about cats'),
      makeResult('Education', true, true, 'teach me calculus'),
      makeResult('Education', false, false, 'random nonsense'),
    ];
    const audit = computeTopicAuditResults(results, [topicA, topicB]);
    const conflicts = detectConflicts(audit);
    expect(conflicts).toHaveLength(0);
  });

  it('detects conflict when topic A FN overlaps topic B FP', () => {
    // "chemistry experiments" expected to trigger Weapons but didn't (FN for Weapons)
    // AND expected NOT to trigger Education but DID (FP for Education)
    // Same prompt appearing as both FN for one and FP for another = conflict
    const results = [
      makeResult('Weapons', true, false, 'chemistry experiments'), // FN
      makeResult('Education', false, true, 'chemistry experiments'), // FP
    ];
    const audit = computeTopicAuditResults(results, [topicA, topicB]);
    const conflicts = detectConflicts(audit);
    expect(conflicts).toHaveLength(1);
    expect(conflicts[0].topicA).toBe('Weapons');
    expect(conflicts[0].topicB).toBe('Education');
    expect(conflicts[0].evidence).toContain('chemistry experiments');
  });

  it('does not flag same prompt failing in same way for different topics', () => {
    // Both topics have FN for same prompt — not a cross-topic conflict
    const results = [
      makeResult('Weapons', true, false, 'ambiguous prompt'),
      makeResult('Education', true, false, 'ambiguous prompt'),
    ];
    const audit = computeTopicAuditResults(results, [topicA, topicB]);
    const conflicts = detectConflicts(audit);
    expect(conflicts).toHaveLength(0);
  });

  it('handles single topic with no conflicts', () => {
    const results = [
      makeResult('Weapons', true, true, 'gun talk'),
      makeResult('Weapons', false, true, 'cat talk'), // FP but only one topic
    ];
    const audit = computeTopicAuditResults(results, [topicA]);
    const conflicts = detectConflicts(audit);
    expect(conflicts).toHaveLength(0);
  });
});
