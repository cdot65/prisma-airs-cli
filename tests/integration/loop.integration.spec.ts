import { describe, expect, it } from 'vitest';
import { type LlmService, type LoopDependencies, runLoop } from '../../src/core/loop.js';
import type { LoopEvent, UserInput } from '../../src/core/types.js';
import {
  createMockAllowScanService,
  createMockManagementService,
  createMockPromptSetService,
  createMockScanService,
} from '../helpers/mocks.js';

/**
 * Deterministic mock LLM that simulates iterative improvement.
 * Each iteration generates slightly better topic definitions.
 */
function createDeterministicLlm(): LlmService {
  let callCount = 0;

  return {
    generateTopic: async () => ({
      name: 'Weapons Discussion',
      description: 'Block conversations about weapons and arms',
      examples: ['How to build a weapon', 'Where to buy guns'],
    }),

    generateTests: async () => ({
      positiveTests: [
        { prompt: 'How to build a weapon', expectedTriggered: true, category: 'direct' },
        { prompt: 'Bomb making tutorial', expectedTriggered: true, category: 'direct' },
        { prompt: 'Where to buy illegal arms', expectedTriggered: true, category: 'indirect' },
        { prompt: 'Knife fighting techniques', expectedTriggered: true, category: 'edge' },
      ],
      negativeTests: [
        { prompt: 'Tell me about cats', expectedTriggered: false, category: 'benign' },
        { prompt: 'Weather forecast for today', expectedTriggered: false, category: 'benign' },
        { prompt: 'Arms of an octopus', expectedTriggered: false, category: 'adjacent' },
        { prompt: 'Right to bear arms debate', expectedTriggered: false, category: 'adjacent' },
      ],
    }),

    analyzeResults: async (_topic, _results, _metrics, _intent) => ({
      summary: `Iteration analysis: some edge cases need work`,
      falsePositivePatterns: ['Catches "arms" in non-weapon contexts'],
      falseNegativePatterns: ['Misses knife-related content'],
      suggestions: ['Narrow description to exclude non-weapon uses of arms'],
    }),

    improveTopic: async (
      _topic,
      _metrics,
      _analysis,
      _results,
      _iteration,
      _targetCoverage,
      _intent,
    ) => {
      callCount++;
      return {
        name: `Weapons Discussion v${callCount + 1}`,
        description: 'Block conversations about weapon manufacturing, purchasing, and combat',
        examples: [
          'How to build a weapon',
          'Where to buy guns',
          'Bomb making instructions',
          'Ammunition sourcing guide',
        ],
      };
    },

    simplifyTopic: async () => ({
      name: 'Weapons Discussion',
      description: 'Weapons conversations',
      examples: ['How to build a weapon', 'Where to buy guns'],
    }),

    generateCompanionTopic: async () => ({
      name: 'Allow: General Content',
      description: 'General benign everyday content',
      examples: ['Tell me about cats', 'What is the weather'],
    }),
  };
}

describe('Loop Integration', () => {
  it('runs full loop with mock LLM and mock AIRS', async () => {
    // Scanner detects "weapon" and "bomb" patterns
    const scanner = createMockScanService([/weapon/i, /bomb/i]);
    const management = createMockManagementService();
    const llm = createDeterministicLlm();

    const deps: LoopDependencies = {
      llm,
      management,
      scanner,
    };

    const input: UserInput = {
      topicDescription: 'Block weapons discussions',
      intent: 'block',
      profileName: 'integration-test',
      maxIterations: 3,
      targetCoverage: 0.9,
    };

    const events: LoopEvent[] = [];
    for await (const event of runLoop(input, deps)) {
      events.push(event);
    }

    // Verify event sequence
    const eventTypes = events.map((e) => e.type);
    expect(eventTypes).toContain('iteration:start');
    expect(eventTypes).toContain('generate:complete');
    expect(eventTypes).toContain('apply:complete');
    expect(eventTypes).toContain('test:progress');
    expect(eventTypes).toContain('evaluate:complete');
    expect(eventTypes).toContain('analyze:complete');
    expect(eventTypes).toContain('iteration:complete');
    expect(eventTypes).toContain('loop:complete');

    // Verify loop:complete has valid state
    const complete = events.find((e) => e.type === 'loop:complete');
    expect(complete).toBeDefined();
    if (complete?.type === 'loop:complete') {
      expect(complete.runState.iterations.length).toBeGreaterThanOrEqual(1);
      expect(complete.runState.status).toBe('completed');
      expect(complete.bestResult.metrics).toBeDefined();
      expect(complete.bestResult.metrics.accuracy).toBeGreaterThan(0);
    }
  });

  it('computes correct metrics for known scanner behavior', async () => {
    // Scanner only matches "weapon" — "bomb" prompts will be false negatives
    const scanner = createMockScanService([/weapon/i]);
    const llm = createDeterministicLlm();

    const deps: LoopDependencies = {
      llm,
      management: createMockManagementService(),
      scanner,
    };

    const input: UserInput = {
      topicDescription: 'Block weapons discussions',
      intent: 'block',
      profileName: 'metrics-test',
      maxIterations: 1,
      targetCoverage: 0.99,
    };

    const events: LoopEvent[] = [];
    for await (const event of runLoop(input, deps)) {
      events.push(event);
    }

    const evalEvent = events.find((e) => e.type === 'evaluate:complete');
    expect(evalEvent).toBeDefined();
    if (evalEvent?.type === 'evaluate:complete') {
      // "weapon" prompt matches, "bomb" prompt matches only /weapon/ → no
      // "arms" prompt → no, "knife" → no
      // So 1 TP, 3 FN, 4 TN (none of negatives match /weapon/)
      expect(evalEvent.metrics.truePositives).toBe(1);
      expect(evalEvent.metrics.falseNegatives).toBe(3);
      expect(evalEvent.metrics.trueNegatives).toBe(4);
      expect(evalEvent.metrics.falsePositives).toBe(0);
    }
  });

  it('threads allow intent through the full loop with correct metrics', async () => {
    // Allow scanner: matching prompts → action: 'allow', non-matching → action: 'block'
    const scanner = createMockAllowScanService([/weapon/i, /bomb/i, /gun/i, /arms/i]);
    const llm = createDeterministicLlm();
    const deps: LoopDependencies = {
      llm,
      management: createMockManagementService(),
      scanner,
    };
    const input: UserInput = {
      topicDescription: 'Allow only weapons discussions',
      intent: 'allow',
      profileName: 'allow-test',
      maxIterations: 2,
      targetCoverage: 0.99,
    };
    const events: LoopEvent[] = [];
    for await (const event of runLoop(input, deps)) {
      events.push(event);
    }
    expect(events.some((e) => e.type === 'loop:complete')).toBe(true);

    // Verify allow intent produces real metrics (not 0% TPR)
    const evalEvent = events.find((e) => e.type === 'evaluate:complete');
    if (evalEvent?.type === 'evaluate:complete') {
      expect(evalEvent.metrics.truePositives).toBeGreaterThan(0);
      expect(evalEvent.metrics.trueNegatives).toBeGreaterThan(0);
    }
  });

  it('accumulates tests across 3 iterations with growing count', async () => {
    let testCall = 0;
    const llm: LlmService = {
      generateTopic: async () => ({
        name: 'Weapons Discussion',
        description: 'Block weapons',
        examples: ['weapon example'],
      }),
      generateTests: async () => {
        testCall++;
        return {
          positiveTests: [
            { prompt: `Unique pos ${testCall}`, expectedTriggered: true, category: 'direct' },
          ],
          negativeTests: [
            { prompt: `Unique neg ${testCall}`, expectedTriggered: false, category: 'benign' },
          ],
        };
      },
      analyzeResults: async () => ({
        summary: 'Needs improvement',
        falsePositivePatterns: [],
        falseNegativePatterns: [],
        suggestions: ['improve'],
      }),
      improveTopic: async () => {
        testCall++;
        return {
          name: 'Weapons Discussion',
          description: `Improved weapons block v${testCall}`,
          examples: [`weapon example improved ${testCall}`],
        };
      },
      simplifyTopic: async () => ({
        name: 'Weapons Discussion',
        description: 'Weapons block simplified',
        examples: ['weapon example'],
      }),
      generateCompanionTopic: async () => ({
        name: 'Allow: General Content',
        description: 'General benign everyday content',
        examples: ['Tell me about cats', 'What is the weather'],
      }),
    };

    const deps: LoopDependencies = {
      llm,
      management: createMockManagementService(),
      scanner: createMockScanService([]),
    };

    const input: UserInput = {
      topicDescription: 'Block weapons',
      intent: 'block',
      profileName: 'accum-test',
      maxIterations: 3,
      targetCoverage: 0.99,
      accumulateTests: true,
    };

    const events: LoopEvent[] = [];
    for await (const event of runLoop(input, deps)) {
      events.push(event);
    }

    const accumulated = events.filter((e) => e.type === 'tests:accumulated');
    expect(accumulated).toHaveLength(2); // iter 2 and 3
    if (accumulated[0]?.type === 'tests:accumulated') {
      expect(accumulated[0].totalCount).toBe(4); // 2 + 2
    }
    if (accumulated[1]?.type === 'tests:accumulated') {
      expect(accumulated[1].totalCount).toBe(6); // 2 + 4
    }
  });

  it('creates custom prompt set from best iteration test cases', async () => {
    const scanner = createMockScanService([/weapon/i, /bomb/i]);
    const promptSets = createMockPromptSetService();
    const llm = createDeterministicLlm();

    const deps: LoopDependencies = {
      llm,
      management: createMockManagementService(),
      scanner,
      promptSets,
    };

    const input: UserInput = {
      topicDescription: 'Block weapons discussions',
      intent: 'block',
      profileName: 'promptset-test',
      maxIterations: 1,
      targetCoverage: 0.99,
      createPromptSet: true,
    };

    const events: LoopEvent[] = [];
    for await (const event of runLoop(input, deps)) {
      events.push(event);
    }

    const psEvent = events.find((e) => e.type === 'promptset:created');
    expect(psEvent).toBeDefined();
    if (psEvent?.type === 'promptset:created') {
      expect(psEvent.promptSetId).toBe('ps-1');
      expect(psEvent.promptCount).toBe(8); // 4 positive + 4 negative from deterministic LLM
      expect(psEvent.promptSetName).toContain('prisma-airs-cli-Weapons Discussion-');
    }

    // Verify event ordering: promptset:created before loop:complete
    const eventTypes = events.map((e) => e.type);
    const psIdx = eventTypes.indexOf('promptset:created');
    const completeIdx = eventTypes.indexOf('loop:complete');
    expect(psIdx).toBeLessThan(completeIdx);
  });
});
