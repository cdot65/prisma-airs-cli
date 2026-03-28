/**
 * Audit runner — async generator that evaluates all topics in a profile.
 */

import type { ManagementService, ScanService } from '../airs/types.js';
import type { TestCase, TestResult } from '../core/types.js';
import type { LlmService } from '../llm/types.js';
import { computeCompositeMetrics, computeTopicAuditResults, detectConflicts } from './evaluator.js';
import type { AuditEvent, AuditResult } from './types.js';

export interface AuditInput {
  profileName: string;
  maxTestsPerTopic?: number;
  scanConcurrency?: number;
}

export interface AuditDependencies {
  llm: LlmService;
  management: ManagementService;
  scanner: ScanService;
}

export async function* runAudit(
  input: AuditInput,
  deps: AuditDependencies,
): AsyncGenerator<AuditEvent> {
  // 1. Load topics from profile
  const topics = await deps.management.getProfileTopics(input.profileName);
  if (topics.length === 0) {
    throw new Error(`No topics found in profile "${input.profileName}"`);
  }
  yield { type: 'topics:loaded', topics };

  // 2. Generate tests per topic
  const allTests: TestCase[] = [];
  for (const topic of topics) {
    const customTopic = {
      name: topic.topicName,
      description: topic.description,
      examples: topic.examples,
    };
    const { positiveTests, negativeTests } = await deps.llm.generateTests(
      customTopic,
      topic.action,
    );
    const topicTests = [...positiveTests, ...negativeTests].map((t) => ({
      ...t,
      targetTopic: topic.topicName,
    }));
    allTests.push(...topicTests);
    yield { type: 'tests:generated', topicName: topic.topicName, count: topicTests.length };
  }

  // 3. Scan all tests against the profile
  const prompts = allTests.map((t) => t.prompt);
  const scanResults = await deps.scanner.scanBatch(
    input.profileName,
    prompts,
    input.scanConcurrency ?? 5,
  );

  yield { type: 'scan:progress', completed: prompts.length, total: prompts.length };

  // 4. Build test results
  const testResults: TestResult[] = allTests.map((testCase, i) => {
    const scan = scanResults[i];
    // prompt_detected.topic_violation is the sole signal for both intents.
    const actualTriggered = scan.triggered;
    return {
      testCase,
      actualTriggered,
      scanAction: scan.action,
      scanId: scan.scanId,
      reportId: scan.reportId,
      correct: testCase.expectedTriggered === actualTriggered,
    };
  });

  // 5. Evaluate
  const topicAuditResults = computeTopicAuditResults(testResults, topics);
  const compositeMetrics = computeCompositeMetrics(topicAuditResults);
  const conflicts = detectConflicts(topicAuditResults);

  const result: AuditResult = {
    profileName: input.profileName,
    timestamp: new Date().toISOString(),
    topics: topicAuditResults,
    compositeMetrics,
    conflicts,
  };

  yield { type: 'evaluate:complete', result };
  yield { type: 'audit:complete', result };
}
