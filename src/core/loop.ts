import { nanoid } from 'nanoid';
import type { ManagementService, PromptSetService, ScanService } from '../airs/types.js';
import type { LearningExtractor } from '../memory/extractor.js';
import { computeCategoryBreakdown, computeMetrics } from './metrics.js';
import type {
  AnalysisReport,
  CategoryBreakdown,
  CustomTopic,
  EfficacyMetrics,
  IterationResult,
  LoopEvent,
  RunState,
  TestCase,
  TestResult,
  UserInput,
} from './types.js';

/** Contract for LLM operations used by the refinement loop. */
export interface LlmService {
  /** Generate an initial topic definition from a user description. */
  generateTopic(description: string, intent: string, seeds?: string[]): Promise<CustomTopic>;
  /** Generate positive and negative test prompts for a topic. */
  generateTests(
    topic: CustomTopic,
    intent: string,
    categoryBreakdown?: CategoryBreakdown[],
  ): Promise<{ positiveTests: TestCase[]; negativeTests: TestCase[] }>;
  /** Analyze scan results to identify false positive/negative patterns. */
  analyzeResults(
    topic: CustomTopic,
    results: TestResult[],
    metrics: EfficacyMetrics,
    intent: string,
  ): Promise<AnalysisReport>;
  /** Simplify a topic that has regressed by removing exclusion clauses and shortening. */
  simplifyTopic(
    currentTopic: CustomTopic,
    bestTopic: CustomTopic,
    metrics: EfficacyMetrics,
    analysis: AnalysisReport,
    intent: string,
  ): Promise<CustomTopic>;
  /** Generate a broad allow companion topic for a block-intent profile. */
  generateCompanionTopic(blockTopicName: string, blockDescription: string): Promise<CustomTopic>;
  /** Refine a topic definition based on metrics and analysis from the previous iteration. */
  improveTopic(
    topic: CustomTopic,
    metrics: EfficacyMetrics,
    analysis: AnalysisReport,
    results: TestResult[],
    iteration: number,
    targetCoverage: number,
    intent: string,
    bestContext?: { bestCoverage: number; bestIteration: number; bestTopic?: CustomTopic },
  ): Promise<CustomTopic>;
}

/** Dependencies injected into the refinement loop. */
export interface LoopDependencies {
  /** LLM service for topic generation, testing, analysis, and improvement. */
  llm: LlmService;
  /** AIRS management service for topic CRUD and profile linking. */
  management: ManagementService;
  /** AIRS scan service for batch prompt scanning. */
  scanner: ScanService;
  /** Optional memory system for cross-run learning extraction. */
  memory?: { extractor: LearningExtractor };
  /** Optional prompt set service for creating custom prompt sets from test cases. */
  promptSets?: PromptSetService;
}

/** Check if a topic definition matches any previous iteration's topic. Returns the iteration number or null. */
function findDuplicateIteration(topic: CustomTopic, iterations: IterationResult[]): number | null {
  for (const iter of iterations) {
    if (
      iter.topic.description === topic.description &&
      iter.topic.examples.length === topic.examples.length &&
      iter.topic.examples.every((e, idx) => e === topic.examples[idx])
    ) {
      return iter.iteration;
    }
  }
  return null;
}

/**
 * Main refinement loop — generates, deploys, tests, and iteratively improves a topic.
 * Yields typed {@link LoopEvent} discriminated unions at each stage.
 * @param input - User input seeding the generation run.
 * @param deps - Injected service dependencies.
 */
export async function* runLoop(
  input: UserInput,
  deps: LoopDependencies,
): AsyncGenerator<LoopEvent> {
  const maxIterations = input.maxIterations ?? 20;
  const targetCoverage = input.targetCoverage ?? 0.9;

  const runState: RunState = {
    id: nanoid(),
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    userInput: input,
    iterations: [],
    currentIteration: 0,
    bestIteration: 0,
    bestCoverage: 0,
    consecutiveRegressions: 0,
    hasRevertedToBest: false,
    hasTriedSimplification: false,
    status: 'running',
  };

  let currentTopic: CustomTopic | null = null;
  let topicId = '';
  let companionTopicId = '';
  let companionTopic: CustomTopic | null = null;
  let lockedName = '';
  let accumulatedTests: TestCase[] = [];

  for (let i = 1; i <= maxIterations; i++) {
    const iterationStart = Date.now();
    runState.currentIteration = i;

    yield { type: 'iteration:start', iteration: i };

    // Step 1: Generate or improve topic
    if (i === 1) {
      currentTopic = await deps.llm.generateTopic(
        input.topicDescription,
        input.intent,
        input.seedExamples,
      );
      lockedName = currentTopic.name;
    } else if (currentTopic) {
      const prevIteration = runState.iterations[runState.iterations.length - 1];
      currentTopic = await deps.llm.improveTopic(
        currentTopic,
        prevIteration.metrics,
        prevIteration.analysis,
        prevIteration.testResults,
        i,
        targetCoverage,
        input.intent,
        {
          bestCoverage: runState.bestCoverage,
          bestIteration: runState.bestIteration,
          bestTopic: runState.iterations[runState.bestIteration - 1]?.topic,
        },
      );
      // Force the name to stay consistent across iterations
      currentTopic = { ...currentTopic, name: lockedName };

      // Skip scanning if topic is identical to a previous iteration
      const dupIter = findDuplicateIteration(currentTopic, runState.iterations);
      if (dupIter !== null) {
        yield {
          type: 'topic:duplicate' as const,
          topic: currentTopic,
          duplicateOfIteration: dupIter,
        };
        runState.consecutiveRegressions++;
        runState.currentIteration = i;
        runState.updatedAt = new Date().toISOString();

        // Recovery: revert → simplify (same tiers as end-of-iteration)
        const recoveryThreshold = 2;

        // Tier 1: Revert to best
        if (
          runState.consecutiveRegressions >= recoveryThreshold &&
          !runState.hasRevertedToBest &&
          runState.bestIteration > 0
        ) {
          const bestResult = runState.iterations[runState.bestIteration - 1];
          if (bestResult) {
            currentTopic = { ...bestResult.topic };
            runState.hasRevertedToBest = true;
            runState.consecutiveRegressions = 0;
            yield {
              type: 'topic:reverted' as const,
              topic: currentTopic,
              revertedToIteration: runState.bestIteration,
            };
          }
        }
        // Tier 2: Simplify
        else if (
          runState.consecutiveRegressions >= recoveryThreshold &&
          !runState.hasTriedSimplification &&
          runState.bestIteration > 0
        ) {
          const bestResult = runState.iterations[runState.bestIteration - 1];
          if (bestResult) {
            currentTopic = await deps.llm.simplifyTopic(
              currentTopic,
              bestResult.topic,
              bestResult.metrics,
              bestResult.analysis,
              input.intent,
            );
            currentTopic = { ...currentTopic, name: lockedName };
            runState.hasTriedSimplification = true;
            runState.consecutiveRegressions = 0;
            yield { type: 'topic:simplified' as const, topic: currentTopic };

            // If simplified topic is also a duplicate, don't reset — count it
            const simpDupIter = findDuplicateIteration(currentTopic, runState.iterations);
            if (simpDupIter !== null) {
              yield {
                type: 'topic:duplicate' as const,
                topic: currentTopic,
                duplicateOfIteration: simpDupIter,
              };
              runState.consecutiveRegressions++;
            }
          }
        }

        const maxRegressions = input.maxRegressions ?? 3;
        if (maxRegressions > 0 && runState.consecutiveRegressions >= maxRegressions) {
          break;
        }
        continue;
      }
    }

    /* v8 ignore next */
    if (!currentTopic) throw new Error('Invariant: topic must exist');
    const topic = currentTopic;
    yield { type: 'generate:complete', topic };

    // Step 2: Apply topic via management API (SDK v2)
    if (i === 1) {
      // Check if a topic with this name already exists (reuse it)
      const existing = await deps.management.listTopics();
      const match = existing.find((t) => t.topic_name === topic.name);

      if (match?.topic_id) {
        topicId = match.topic_id;
        await deps.management.updateTopic(topicId, {
          topic_name: topic.name,
          description: topic.description,
          examples: topic.examples,
          active: true,
        });
      } else {
        const response = await deps.management.createTopic({
          topic_name: topic.name,
          description: topic.description,
          examples: topic.examples,
          active: true,
        });
        /* v8 ignore next */
        if (!response.topic_id) throw new Error('Invariant: topic_id missing from create response');
        topicId = response.topic_id;
      }

      // Guardrail-level action is the INVERSE of topic intent:
      // block-intent → guardrailAction='allow' (default: allow, block topics carve out violations)
      // allow-intent → guardrailAction='block' (default: block, allow topics whitelist content)
      const guardrailAction = input.intent === 'block' ? 'allow' : 'block';

      if (input.intent === 'block') {
        // Two-phase: generate domain-specific allow companion for block-intent.
        // AIRS needs both allow + block topics sharing the same vocabulary domain
        // for the matching engine to distinguish benign from malicious content.
        companionTopic = await deps.llm.generateCompanionTopic(topic.name, topic.description);
        yield { type: 'companion:generated', topic: companionTopic };

        const companionMatch = existing.find((t) => t.topic_name === companionTopic?.name);
        if (companionMatch?.topic_id) {
          companionTopicId = companionMatch.topic_id;
          await deps.management.updateTopic(companionTopicId, {
            topic_name: companionTopic.name,
            description: companionTopic.description,
            examples: companionTopic.examples,
            active: true,
          });
        } else {
          const cResponse = await deps.management.createTopic({
            topic_name: companionTopic.name,
            description: companionTopic.description,
            examples: companionTopic.examples,
            active: true,
          });
          /* v8 ignore next */
          if (!cResponse.topic_id)
            throw new Error('Invariant: topic_id missing from companion create response');
          companionTopicId = cResponse.topic_id;
        }

        yield { type: 'companion:created', topicId: companionTopicId, topic: companionTopic };
        runState.companionTopic = companionTopic;

        // Wire both topics to profile with guardrailAction='allow'
        await deps.management.assignTopicsToProfile(
          input.profileName,
          [
            { topicId: companionTopicId, topicName: companionTopic.name, action: 'allow' },
            { topicId, topicName: topic.name, action: 'block' },
          ],
          guardrailAction,
        );
      } else {
        // Allow-intent: single topic, guardrailAction='block'
        await deps.management.assignTopicsToProfile(
          input.profileName,
          [{ topicId, topicName: topic.name, action: input.intent }],
          guardrailAction,
        );
      }
    } else {
      await deps.management.updateTopic(topicId, {
        topic_name: topic.name,
        description: topic.description,
        examples: topic.examples,
        active: true,
      });

      // Re-assign topics to profile so AIRS references the new topic revision.
      // AIRS pins topic content to the revision in the profile — without this,
      // the profile would still reference the pre-update revision.
      const guardrailAction = input.intent === 'block' ? 'allow' : 'block';
      const topicEntries: Array<{
        topicId: string;
        topicName: string;
        action: 'allow' | 'block';
      }> = [{ topicId, topicName: topic.name, action: input.intent }];
      if (input.intent === 'block' && companionTopicId && companionTopic) {
        topicEntries.unshift({
          topicId: companionTopicId,
          topicName: companionTopic.name,
          action: 'allow',
        });
      }
      await deps.management.assignTopicsToProfile(input.profileName, topicEntries, guardrailAction);
    }

    yield { type: 'apply:complete', topicId };

    // Step 3: Generate test cases (with category breakdown for weighted generation)
    const prevItResults = i > 1 ? runState.iterations[runState.iterations.length - 1] : undefined;
    const categoryBreakdown = prevItResults
      ? computeCategoryBreakdown(prevItResults.testResults)
      : undefined;
    const { positiveTests, negativeTests } = await deps.llm.generateTests(
      topic,
      input.intent,
      categoryBreakdown,
    );
    const newTests: TestCase[] = [...positiveTests, ...negativeTests].map((t) => ({
      ...t,
      source: 'generated' as const,
    }));

    // Build carried failures + regression tier from previous iteration
    let carriedFailures: TestCase[] = [];
    let regressionTier: TestCase[] = [];
    if (i > 1 && prevItResults) {
      carriedFailures = prevItResults.testResults
        .filter((r) => !r.correct)
        .map((r) => {
          const source: 'carried-fn' | 'carried-fp' =
            r.testCase.expectedTriggered && !r.actualTriggered ? 'carried-fn' : 'carried-fp';
          return { ...r.testCase, source };
        });
      regressionTier = prevItResults.testResults
        .filter((r) => r.correct)
        .map((r) => ({ ...r.testCase, source: 'regression' as const }));
    }

    // Merge: carried failures > regression > generated (dedup by prompt text)
    const seen = new Set<string>();
    const merged: TestCase[] = [];
    for (const pool of [carriedFailures, regressionTier, newTests]) {
      for (const t of pool) {
        const key = t.prompt.toLowerCase().trim();
        if (!seen.has(key)) {
          seen.add(key);
          merged.push(t);
        }
      }
    }

    // Also merge old accumulated tests if accumulation enabled
    if (input.accumulateTests && i > 1) {
      for (const t of accumulatedTests) {
        const key = t.prompt.toLowerCase().trim();
        if (!seen.has(key)) {
          seen.add(key);
          merged.push(t);
        }
      }
    }

    // Apply max cap
    const maxCap = input.maxAccumulatedTests;
    const preCapCount = merged.length;
    const allTests = maxCap && merged.length > maxCap ? merged.slice(0, maxCap) : merged;

    // Emit composition event on iteration 2+
    if (i > 1) {
      yield {
        type: 'tests:composed',
        generated: newTests.length,
        carriedFailures: carriedFailures.length,
        regressionTier: regressionTier.length,
        total: allTests.length,
      };
    }

    // Emit legacy accumulated event when accumulation enabled
    if (input.accumulateTests && i > 1) {
      yield {
        type: 'tests:accumulated',
        newCount: newTests.length,
        totalCount: allTests.length,
        droppedCount: preCapCount - allTests.length,
      };
    }
    accumulatedTests = allTests;

    // Step 4: Run scans
    const sessionId = `prisma-airs-cli-${runState.id.slice(0, 7)}-iter${i}`;
    const testResults: TestResult[] = [];
    const prompts = allTests.map((t) => t.prompt);
    const scanResults = await deps.scanner.scanBatch(
      input.profileName,
      prompts,
      undefined,
      sessionId,
    );

    for (let j = 0; j < allTests.length; j++) {
      const testCase = allTests[j];
      const scanResult = scanResults[j];
      // prompt_detected.topic_violation is the sole signal for both intents.
      const actualTriggered = scanResult.triggered;
      testResults.push({
        testCase,
        actualTriggered,
        scanAction: scanResult.action,
        scanId: scanResult.scanId,
        reportId: scanResult.reportId,
        correct: testCase.expectedTriggered === actualTriggered,
      });

      yield { type: 'test:progress', completed: j + 1, total: allTests.length };
    }

    // Step 5: Evaluate
    const metrics = computeMetrics(testResults);
    yield { type: 'evaluate:complete', metrics };

    // Step 6: Analyze
    const analysis = await deps.llm.analyzeResults(topic, testResults, metrics, input.intent);
    yield { type: 'analyze:complete', analysis };

    // Record iteration
    const iterationResult: IterationResult = {
      iteration: i,
      timestamp: new Date().toISOString(),
      topic,
      testCases: allTests,
      testResults,
      metrics,
      analysis,
      durationMs: Date.now() - iterationStart,
    };

    runState.iterations.push(iterationResult);

    if (metrics.coverage > runState.bestCoverage) {
      runState.bestCoverage = metrics.coverage;
      runState.bestIteration = i;
      runState.consecutiveRegressions = 0;
    } else {
      runState.consecutiveRegressions++;
    }

    runState.updatedAt = new Date().toISOString();

    yield { type: 'iteration:complete', result: iterationResult };

    // Check stop conditions
    if (metrics.coverage >= targetCoverage) {
      break;
    }

    // Recovery strategy: revert → simplify → stop
    const recoveryThreshold = 2;
    const maxRegressions = input.maxRegressions ?? 3;

    // Tier 1: Revert to best-performing definition (no LLM call)
    if (
      runState.consecutiveRegressions >= recoveryThreshold &&
      !runState.hasRevertedToBest &&
      runState.bestIteration > 0
    ) {
      const bestResult = runState.iterations[runState.bestIteration - 1];
      if (bestResult) {
        currentTopic = { ...bestResult.topic };
        runState.hasRevertedToBest = true;
        runState.consecutiveRegressions = 0;
        yield {
          type: 'topic:reverted' as const,
          topic: currentTopic,
          revertedToIteration: runState.bestIteration,
        };
      }
    }
    // Tier 2: LLM simplification (after revert already tried)
    else if (
      runState.consecutiveRegressions >= recoveryThreshold &&
      !runState.hasTriedSimplification &&
      runState.bestIteration > 0
    ) {
      const bestResult = runState.iterations[runState.bestIteration - 1];
      if (bestResult) {
        currentTopic = await deps.llm.simplifyTopic(
          topic,
          bestResult.topic,
          bestResult.metrics,
          bestResult.analysis,
          input.intent,
        );
        currentTopic = { ...currentTopic, name: lockedName };
        runState.hasTriedSimplification = true;
        runState.consecutiveRegressions = 0;
        yield { type: 'topic:simplified' as const, topic: currentTopic };

        // Check if simplified topic is a duplicate of a previous iteration
        const simpDupIter = findDuplicateIteration(currentTopic, runState.iterations);
        if (simpDupIter !== null) {
          yield {
            type: 'topic:duplicate' as const,
            topic: currentTopic,
            duplicateOfIteration: simpDupIter,
          };
          runState.consecutiveRegressions++;
        }
      }
    }

    // Tier 3: Early stop
    if (maxRegressions > 0 && runState.consecutiveRegressions >= maxRegressions) {
      break;
    }

    // Plateau detection: coverage oscillating in narrow band without improvement (opt-in)
    const plateauWindow = input.plateauWindow ?? 0;
    const plateauBand = input.plateauBand ?? 0.05;
    if (plateauWindow > 0 && runState.iterations.length >= plateauWindow + 1) {
      const recent = runState.iterations.slice(-plateauWindow).map((r) => r.metrics.coverage);
      const min = Math.min(...recent);
      const max = Math.max(...recent);
      if (max - min <= plateauBand && max < runState.bestCoverage + plateauBand) {
        yield {
          type: 'loop:plateau' as const,
          band: [min, max],
          bestCoverage: runState.bestCoverage,
        };
        break;
      }
    }
  }

  runState.status = 'completed';

  // Extract learnings from this run if memory is enabled
  if (deps.memory?.extractor) {
    const { learnings } = await deps.memory.extractor.extractAndSave(runState);
    yield { type: 'memory:extracted', learningCount: learnings.length };
  }

  const bestResult =
    runState.iterations[runState.bestIteration - 1] ??
    runState.iterations[runState.iterations.length - 1];

  // Create custom prompt set from best iteration's test cases if requested
  if (input.createPromptSet && deps.promptSets && bestResult) {
    const setName =
      input.promptSetName ?? `prisma-airs-cli-${bestResult.topic.name}-${runState.id.slice(0, 7)}`;
    const { uuid } = await deps.promptSets.createPromptSet(
      setName,
      `Generated by Prisma AIRS CLI run ${runState.id} — ${bestResult.testCases.length} prompts from iteration ${bestResult.iteration}`,
    );

    for (const tc of bestResult.testCases) {
      const goal = tc.expectedTriggered
        ? 'Should trigger topic guardrail'
        : 'Should NOT trigger topic guardrail';
      await deps.promptSets.addPrompt(uuid, tc.prompt, goal);
    }

    yield {
      type: 'promptset:created',
      promptSetId: uuid,
      promptSetName: setName,
      promptCount: bestResult.testCases.length,
    };
  }

  yield { type: 'loop:complete', bestResult, runState };
}
