/**
 * Audit evaluator — per-topic metrics and cross-topic conflict detection.
 */

import { computeMetrics } from '../core/metrics.js';
import type { EfficacyMetrics, TestResult } from '../core/types.js';
import type { ConflictPair, ProfileTopic, TopicAuditResult } from './types.js';

/** Group test results by their targetTopic field. */
export function groupResultsByTopic(results: TestResult[]): Map<string, TestResult[]> {
  const groups = new Map<string, TestResult[]>();
  for (const r of results) {
    const topic = r.testCase.targetTopic ?? 'unknown';
    const group = groups.get(topic) ?? [];
    group.push(r);
    groups.set(topic, group);
  }
  return groups;
}

/** Compute per-topic audit results with metrics. */
export function computeTopicAuditResults(
  results: TestResult[],
  topics: ProfileTopic[],
): TopicAuditResult[] {
  const groups = groupResultsByTopic(results);

  return topics.map((topic) => {
    const topicResults = groups.get(topic.topicName) ?? [];
    return {
      topic,
      testResults: topicResults,
      metrics: computeMetrics(topicResults),
    };
  });
}

/** Aggregate metrics across all topics. */
export function computeCompositeMetrics(topicResults: TopicAuditResult[]): EfficacyMetrics {
  const allResults = topicResults.flatMap((tr) => tr.testResults);
  return computeMetrics(allResults);
}

/**
 * Detect cross-topic conflicts by finding prompts that appear as FN for one
 * topic and FP for another — indicating the topics interfere with each other.
 */
export function detectConflicts(topicResults: TopicAuditResult[]): ConflictPair[] {
  if (topicResults.length < 2) return [];

  // Build per-topic FN and FP prompt sets
  const topicFNs = new Map<string, Set<string>>();
  const topicFPs = new Map<string, Set<string>>();

  for (const tr of topicResults) {
    const fns = new Set<string>();
    const fps = new Set<string>();
    for (const r of tr.testResults) {
      if (r.testCase.expectedTriggered && !r.actualTriggered) fns.add(r.testCase.prompt);
      if (!r.testCase.expectedTriggered && r.actualTriggered) fps.add(r.testCase.prompt);
    }
    topicFNs.set(tr.topic.topicName, fns);
    topicFPs.set(tr.topic.topicName, fps);
  }

  const conflicts: ConflictPair[] = [];

  // For each pair, check if topic A's FNs overlap with topic B's FPs
  for (let i = 0; i < topicResults.length; i++) {
    for (let j = i + 1; j < topicResults.length; j++) {
      const nameA = topicResults[i].topic.topicName;
      const nameB = topicResults[j].topic.topicName;
      const fnsA = topicFNs.get(nameA) ?? new Set();
      const fpsB = topicFPs.get(nameB) ?? new Set();
      const fnsB = topicFNs.get(nameB) ?? new Set();
      const fpsA = topicFPs.get(nameA) ?? new Set();

      // A's FN overlaps B's FP
      const overlapAB = [...fnsA].filter((p) => fpsB.has(p));
      // B's FN overlaps A's FP
      const overlapBA = [...fnsB].filter((p) => fpsA.has(p));
      const evidence = [...new Set([...overlapAB, ...overlapBA])];

      if (evidence.length > 0) {
        conflicts.push({
          topicA: nameA,
          topicB: nameB,
          description: `${evidence.length} prompt(s) fail as FN for one topic and FP for the other, indicating cross-topic interference`,
          evidence,
        });
      }
    }
  }

  return conflicts;
}
