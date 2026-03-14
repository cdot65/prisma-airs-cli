import { describe, expect, it, vi } from 'vitest';
import { type LoopDependencies, runLoop } from '../../../src/core/loop.js';
import type {
  AnalysisReport,
  CategoryBreakdown,
  CustomTopic,
  EfficacyMetrics,
  LoopEvent,
  TestCase,
  TestResult,
  UserInput,
} from '../../../src/core/types.js';
import type { LearningExtractor } from '../../../src/memory/extractor.js';
import {
  createMockAllowScanService,
  createMockManagementService,
  createMockPromptSetService,
  createMockScanService,
} from '../../helpers/mocks.js';

function createMockLlm() {
  return {
    generateTopic: vi
      .fn<(desc: string, intent: string, seeds?: string[]) => Promise<CustomTopic>>()
      .mockResolvedValue({
        name: 'Weapons Discussion',
        description: 'Block conversations about weapons',
        examples: ['How to make a weapon', 'Gun manufacturing'],
      }),
    generateTests: vi
      .fn<
        (
          topic: CustomTopic,
          intent: string,
          categoryBreakdown?: CategoryBreakdown[],
        ) => Promise<{ positiveTests: TestCase[]; negativeTests: TestCase[] }>
      >()
      .mockResolvedValue({
        positiveTests: [
          { prompt: 'How to build a weapon', expectedTriggered: true, category: 'direct' },
          { prompt: 'Bomb making instructions', expectedTriggered: true, category: 'direct' },
        ],
        negativeTests: [
          { prompt: 'Tell me about cats', expectedTriggered: false, category: 'benign' },
          { prompt: 'Weather forecast', expectedTriggered: false, category: 'benign' },
        ],
      }),
    analyzeResults: vi
      .fn<
        (
          topic: CustomTopic,
          results: TestResult[],
          metrics: EfficacyMetrics,
          intent: string,
        ) => Promise<AnalysisReport>
      >()
      .mockResolvedValue({
        summary: 'Good performance',
        falsePositivePatterns: [],
        falseNegativePatterns: [],
        suggestions: ['Keep current definition'],
      }),
    improveTopic: (() => {
      let improveCall = 0;
      return vi
        .fn<
          (
            topic: CustomTopic,
            metrics: EfficacyMetrics,
            analysis: AnalysisReport,
            results: TestResult[],
            iteration: number,
            targetCoverage: number,
            intent: string,
          ) => Promise<CustomTopic>
        >()
        .mockImplementation(async () => {
          improveCall++;
          return {
            name: `Weapons Discussion v${improveCall + 1}`,
            description: `Block weapons-related conversations variant ${improveCall}`,
            examples: ['How to make a weapon', 'Gun manufacturing', `Variant ${improveCall}`],
          };
        });
    })(),
    simplifyTopic: vi
      .fn<
        (
          currentTopic: CustomTopic,
          bestTopic: CustomTopic,
          metrics: EfficacyMetrics,
          analysis: AnalysisReport,
          intent: string,
        ) => Promise<CustomTopic>
      >()
      .mockResolvedValue({
        name: 'Weapons Discussion',
        description: 'Weapons conversations',
        examples: ['How to make a weapon', 'Gun manufacturing'],
      }),
    generateCompanionTopic: vi
      .fn<(blockTopicName: string, blockDescription: string) => Promise<CustomTopic>>()
      .mockResolvedValue({
        name: 'Allow: General Content',
        description: 'General benign everyday content',
        examples: ['Tell me about cats', 'What is the weather'],
      }),
  };
}

function createDeps(overrides: Partial<LoopDependencies> = {}): LoopDependencies {
  return {
    llm: createMockLlm(),
    management: createMockManagementService(),
    scanner: createMockScanService(),
    ...overrides,
  };
}

const defaultInput: UserInput = {
  topicDescription: 'Block weapons discussions',
  intent: 'block',
  profileName: 'test-profile',
  maxIterations: 2,
  targetCoverage: 0.9,
};

describe('runLoop', () => {
  it('yields iteration:start events', async () => {
    const deps = createDeps();
    const events: LoopEvent[] = [];

    for await (const event of runLoop(defaultInput, deps)) {
      events.push(event);
    }

    const starts = events.filter((e) => e.type === 'iteration:start');
    expect(starts.length).toBeGreaterThanOrEqual(1);
  });

  it('yields generate:complete with topic', async () => {
    const deps = createDeps();
    const events: LoopEvent[] = [];

    for await (const event of runLoop(defaultInput, deps)) {
      events.push(event);
    }

    const genComplete = events.find((e) => e.type === 'generate:complete');
    expect(genComplete).toBeDefined();
    if (genComplete?.type === 'generate:complete') {
      expect(genComplete.topic.name).toBeTruthy();
    }
  });

  it('yields evaluate:complete with metrics', async () => {
    const deps = createDeps();
    const events: LoopEvent[] = [];

    for await (const event of runLoop(defaultInput, deps)) {
      events.push(event);
    }

    const evalComplete = events.find((e) => e.type === 'evaluate:complete');
    expect(evalComplete).toBeDefined();
    if (evalComplete?.type === 'evaluate:complete') {
      expect(evalComplete.metrics.accuracy).toBeDefined();
    }
  });

  it('yields loop:complete at end', async () => {
    const deps = createDeps();
    const events: LoopEvent[] = [];

    for await (const event of runLoop(defaultInput, deps)) {
      events.push(event);
    }

    const complete = events.find((e) => e.type === 'loop:complete');
    expect(complete).toBeDefined();
  });

  it('stops at maxIterations', async () => {
    const deps = createDeps();
    const events: LoopEvent[] = [];

    for await (const event of runLoop({ ...defaultInput, maxIterations: 1 }, deps)) {
      events.push(event);
    }

    const starts = events.filter((e) => e.type === 'iteration:start');
    expect(starts).toHaveLength(1);
  });

  it('stops early when target coverage reached', async () => {
    // Mock scanner that always matches correctly
    const perfectScanner = createMockScanService([/weapon/i, /bomb/i, /gun/i]);
    const deps = createDeps({ scanner: perfectScanner });
    const events: LoopEvent[] = [];

    for await (const event of runLoop(
      { ...defaultInput, maxIterations: 5, targetCoverage: 0.5 },
      deps,
    )) {
      events.push(event);
    }

    const complete = events.find((e) => e.type === 'loop:complete');
    expect(complete).toBeDefined();
  });

  it('calls management client to create topic', async () => {
    const management = createMockManagementService();
    const createSpy = vi.spyOn(management, 'createTopic');
    const deps = createDeps({ management });

    for await (const _event of runLoop({ ...defaultInput, maxIterations: 1 }, deps)) {
      // consume
    }

    expect(createSpy).toHaveBeenCalled();
  });

  it('calls management client to update topic on subsequent iterations', async () => {
    const management = createMockManagementService();
    const updateSpy = vi.spyOn(management, 'updateTopic');
    // Use a scanner that never triggers → coverage stays 0, forcing multiple iterations
    const deps = createDeps({ management, scanner: createMockScanService([]) });

    for await (const _event of runLoop({ ...defaultInput, maxIterations: 2 }, deps)) {
      // consume
    }

    expect(updateSpy).toHaveBeenCalled();
  });

  it('yields test:progress events', async () => {
    const deps = createDeps();
    const events: LoopEvent[] = [];

    for await (const event of runLoop({ ...defaultInput, maxIterations: 1 }, deps)) {
      events.push(event);
    }

    const progress = events.filter((e) => e.type === 'test:progress');
    expect(progress.length).toBeGreaterThanOrEqual(1);
  });

  it('tracks best iteration in loop:complete', async () => {
    const deps = createDeps();
    const events: LoopEvent[] = [];

    for await (const event of runLoop({ ...defaultInput, maxIterations: 2 }, deps)) {
      events.push(event);
    }

    const complete = events.find((e) => e.type === 'loop:complete');
    expect(complete).toBeDefined();
    if (complete?.type === 'loop:complete') {
      expect(complete.runState.bestIteration).toBeDefined();
      expect(complete.bestResult).toBeDefined();
    }
  });

  it('uses default maxIterations and targetCoverage when not provided', async () => {
    // Scanner always triggers → coverage = 0.5, never meets 0.9 default target
    const deps = createDeps();
    const events: LoopEvent[] = [];
    const input: UserInput = {
      topicDescription: 'Block weapons discussions',
      intent: 'block',
      profileName: 'test-profile',
      // No maxIterations or targetCoverage — should use defaults (20, 0.9)
    };

    for await (const event of runLoop(input, deps)) {
      events.push(event);
      // Stop after first iteration to avoid running all 20
      if (event.type === 'iteration:complete') break;
    }

    const starts = events.filter((e) => e.type === 'iteration:start');
    expect(starts.length).toBeGreaterThanOrEqual(1);
  });

  it('reuses existing topic instead of creating', async () => {
    const management = createMockManagementService();
    const createSpy = vi.spyOn(management, 'createTopic');
    const updateSpy = vi.spyOn(management, 'updateTopic');
    // listTopics returns a topic matching the generated name
    vi.spyOn(management, 'listTopics').mockResolvedValue([
      {
        topic_id: 'existing-123',
        topic_name: 'Weapons Discussion',
        description: 'old desc',
        examples: [],
        active: true,
      },
    ]);
    const deps = createDeps({ management });

    for await (const _event of runLoop({ ...defaultInput, maxIterations: 1 }, deps)) {
      // consume
    }

    // Block topic reused via update, not create
    // createTopic IS called once — for the companion allow topic
    expect(createSpy).toHaveBeenCalledTimes(1);
    expect(createSpy).toHaveBeenCalledWith(
      expect.objectContaining({ topic_name: 'Allow: General Content' }),
    );
    expect(updateSpy).toHaveBeenCalledWith('existing-123', expect.anything());
  });

  it('yields companion:generated and companion:created for block intent', async () => {
    const deps = createDeps();
    const events: LoopEvent[] = [];

    for await (const event of runLoop({ ...defaultInput, maxIterations: 1 }, deps)) {
      events.push(event);
    }

    const companionGen = events.find((e) => e.type === 'companion:generated');
    expect(companionGen).toBeDefined();
    if (companionGen?.type === 'companion:generated') {
      expect(companionGen.topic.name).toBe('Allow: General Content');
    }

    const companionCreated = events.find((e) => e.type === 'companion:created');
    expect(companionCreated).toBeDefined();
    if (companionCreated?.type === 'companion:created') {
      expect(companionCreated.topicId).toBeTruthy();
      expect(companionCreated.topic.name).toBe('Allow: General Content');
    }
  });

  it('does not generate companion for allow-intent runs', async () => {
    const deps = createDeps({ scanner: createMockAllowScanService([/cats/i]) });
    const events: LoopEvent[] = [];

    for await (const event of runLoop(
      { ...defaultInput, intent: 'allow', maxIterations: 1 },
      deps,
    )) {
      events.push(event);
    }

    const companionEvents = events.filter(
      (e) => e.type === 'companion:generated' || e.type === 'companion:created',
    );
    expect(companionEvents).toHaveLength(0);
  });

  it('block-intent sets guardrailAction=allow and wires both topics', async () => {
    const management = createMockManagementService();
    const assignTopicsSpy = vi.spyOn(management, 'assignTopicsToProfile');
    const deps = createDeps({ management });

    for await (const _event of runLoop({ ...defaultInput, maxIterations: 1 }, deps)) {
      // consume
    }

    expect(assignTopicsSpy).toHaveBeenCalledWith(
      'test-profile',
      [
        expect.objectContaining({ action: 'allow', topicName: 'Allow: General Content' }),
        expect.objectContaining({ action: 'block', topicName: 'Weapons Discussion' }),
      ],
      'allow',
    );
  });

  it('allow-intent sets guardrailAction=block with single topic', async () => {
    const management = createMockManagementService();
    const assignTopicsSpy = vi.spyOn(management, 'assignTopicsToProfile');
    const deps = createDeps({
      management,
      scanner: createMockAllowScanService([/cats/i]),
    });

    for await (const _event of runLoop(
      { ...defaultInput, intent: 'allow', maxIterations: 1 },
      deps,
    )) {
      // consume
    }

    expect(assignTopicsSpy).toHaveBeenCalledWith(
      'test-profile',
      [expect.objectContaining({ action: 'allow' })],
      'block',
    );
  });

  it('persists companionTopic in RunState', async () => {
    const deps = createDeps();
    const events: LoopEvent[] = [];

    for await (const event of runLoop({ ...defaultInput, maxIterations: 1 }, deps)) {
      events.push(event);
    }

    const complete = events.find((e) => e.type === 'loop:complete');
    if (complete?.type === 'loop:complete') {
      expect(complete.runState.companionTopic).toBeDefined();
      expect(complete.runState.companionTopic?.name).toBe('Allow: General Content');
    }
  });

  it('passes intent to analyzeResults', async () => {
    const llm = createMockLlm();
    const deps = createDeps({ llm });

    for await (const _event of runLoop({ ...defaultInput, maxIterations: 1 }, deps)) {
      // consume
    }

    expect(llm.analyzeResults).toHaveBeenCalledWith(
      expect.anything(),
      expect.anything(),
      expect.anything(),
      'block',
    );
  });

  it('passes intent to improveTopic', async () => {
    const llm = createMockLlm();
    const deps = createDeps({ llm, scanner: createMockScanService([]) });

    for await (const _event of runLoop({ ...defaultInput, maxIterations: 2 }, deps)) {
      // consume
    }

    expect(llm.improveTopic).toHaveBeenCalledWith(
      expect.anything(),
      expect.anything(),
      expect.anything(),
      expect.anything(),
      2,
      0.9,
      'block',
      expect.anything(),
    );
  });

  it('passes allow intent through loop', async () => {
    const llm = createMockLlm();
    const deps = createDeps({ llm, scanner: createMockScanService([]) });

    for await (const _event of runLoop(
      { ...defaultInput, intent: 'allow', maxIterations: 2 },
      deps,
    )) {
      // consume
    }

    expect(llm.analyzeResults).toHaveBeenCalledWith(
      expect.anything(),
      expect.anything(),
      expect.anything(),
      'allow',
    );
    expect(llm.improveTopic).toHaveBeenCalledWith(
      expect.anything(),
      expect.anything(),
      expect.anything(),
      expect.anything(),
      2,
      0.9,
      'allow',
      expect.anything(),
    );
  });

  it('uses topic_violation (triggered) for allow-intent detection', async () => {
    // Mock scanner: matching prompts → triggered: true (topic_violation)
    const allowScanner = createMockAllowScanService([/weapon/i, /bomb/i]);
    const llm = createMockLlm();
    const deps = createDeps({ llm, scanner: allowScanner });
    const events: LoopEvent[] = [];

    for await (const event of runLoop(
      { ...defaultInput, intent: 'allow', maxIterations: 1 },
      deps,
    )) {
      events.push(event);
    }

    const evalEvent = events.find((e) => e.type === 'evaluate:complete');
    expect(evalEvent).toBeDefined();
    if (evalEvent?.type === 'evaluate:complete') {
      // "weapon" and "bomb" prompts → triggered: true → actualTriggered = true
      // "cats" and "weather" → triggered: false → actualTriggered = false
      expect(evalEvent.metrics.truePositives).toBeGreaterThan(0);
      expect(evalEvent.metrics.trueNegatives).toBeGreaterThan(0);
    }
  });

  it('allow-intent with no triggered yields false negatives', async () => {
    // Scanner with triggered always false — all positives become FN
    const noCategoryScanner: import('../../../src/airs/types.js').ScanService = {
      scan: async () => ({
        scanId: 's1',
        reportId: 'r1',
        action: 'allow' as const,
        triggered: false,
      }),
      scanBatch: async (_p, prompts) =>
        prompts.map(() => ({
          scanId: 's1',
          reportId: 'r1',
          action: 'allow' as const,
          triggered: false,
        })),
    };
    const deps = createDeps({ scanner: noCategoryScanner });
    const events: LoopEvent[] = [];

    for await (const event of runLoop(
      { ...defaultInput, intent: 'allow', maxIterations: 1 },
      deps,
    )) {
      events.push(event);
    }

    const evalEvent = events.find((e) => e.type === 'evaluate:complete');
    expect(evalEvent).toBeDefined();
    if (evalEvent?.type === 'evaluate:complete') {
      // All triggered=false → positive tests are FN
      expect(evalEvent.metrics.falseNegatives).toBeGreaterThan(0);
    }
  });

  it('block-intent still uses triggered field', async () => {
    // Standard block scanner — triggered field is used
    const scanner = createMockScanService([/weapon/i, /bomb/i]);
    const llm = createMockLlm();
    const deps = createDeps({ llm, scanner });
    const events: LoopEvent[] = [];

    for await (const event of runLoop({ ...defaultInput, maxIterations: 1 }, deps)) {
      events.push(event);
    }

    const evalEvent = events.find((e) => e.type === 'evaluate:complete');
    expect(evalEvent).toBeDefined();
    if (evalEvent?.type === 'evaluate:complete') {
      expect(evalEvent.metrics.truePositives).toBeGreaterThan(0);
    }
  });

  describe('test accumulation', () => {
    it('does not accumulate by default', async () => {
      const llm = createMockLlm();
      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop({ ...defaultInput, maxIterations: 2 }, deps)) {
        events.push(event);
      }

      const accumulated = events.filter((e) => e.type === 'tests:accumulated');
      expect(accumulated).toHaveLength(0);
    });

    it('accumulates when enabled', async () => {
      const llm = createMockLlm();
      // Return different tests per iteration
      let testCall = 0;
      llm.generateTests.mockImplementation(async () => {
        testCall++;
        return {
          positiveTests: [
            { prompt: `Prompt A${testCall}`, expectedTriggered: true, category: 'direct' },
          ],
          negativeTests: [
            { prompt: `Prompt B${testCall}`, expectedTriggered: false, category: 'benign' },
          ],
        };
      });

      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 3, accumulateTests: true },
        deps,
      )) {
        events.push(event);
      }

      const accumulated = events.filter((e) => e.type === 'tests:accumulated');
      // Only iterations 2+ emit accumulated events
      expect(accumulated).toHaveLength(2);
      if (accumulated[0]?.type === 'tests:accumulated') {
        expect(accumulated[0].totalCount).toBe(4); // 2 new + 2 from iter1
      }
      if (accumulated[1]?.type === 'tests:accumulated') {
        expect(accumulated[1].totalCount).toBe(6); // 2 new + 4 from iter1+2
      }
    });

    it('deduplicates by prompt text case-insensitively', async () => {
      const llm = createMockLlm();
      let testCall = 0;
      llm.generateTests.mockImplementation(async () => {
        testCall++;
        if (testCall === 1) {
          return {
            positiveTests: [
              { prompt: 'How to build a weapon', expectedTriggered: true, category: 'direct' },
            ],
            negativeTests: [
              { prompt: 'Tell me about cats', expectedTriggered: false, category: 'benign' },
            ],
          };
        }
        return {
          positiveTests: [
            { prompt: 'how to build a weapon', expectedTriggered: true, category: 'direct' }, // dup
          ],
          negativeTests: [
            { prompt: 'Weather forecast', expectedTriggered: false, category: 'benign' }, // new
          ],
        };
      });

      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 2, accumulateTests: true },
        deps,
      )) {
        events.push(event);
      }

      const accumulated = events.filter((e) => e.type === 'tests:accumulated');
      expect(accumulated).toHaveLength(1);
      if (accumulated[0]?.type === 'tests:accumulated') {
        // 2 new (deduped weapon + weather), 1 carried from iter1 (cats)
        expect(accumulated[0].totalCount).toBe(3);
      }
    });

    it('caps accumulated tests at maxAccumulatedTests', async () => {
      const llm = createMockLlm();
      let testCall = 0;
      llm.generateTests.mockImplementation(async () => {
        testCall++;
        return {
          positiveTests: [
            { prompt: `Pos ${testCall}`, expectedTriggered: true, category: 'direct' },
          ],
          negativeTests: [
            { prompt: `Neg ${testCall}`, expectedTriggered: false, category: 'benign' },
          ],
        };
      });

      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 3, accumulateTests: true, maxAccumulatedTests: 3 },
        deps,
      )) {
        events.push(event);
      }

      const accumulated = events.filter((e) => e.type === 'tests:accumulated');
      // Iteration 2: 2 new + 2 old = 4 merged, capped to 3, dropped 1
      if (accumulated[0]?.type === 'tests:accumulated') {
        expect(accumulated[0].totalCount).toBe(3);
        expect(accumulated[0].droppedCount).toBe(1);
      }
      // Iteration 3: 2 new + 3 old = 5 merged, capped to 3, dropped 2
      if (accumulated[1]?.type === 'tests:accumulated') {
        expect(accumulated[1].totalCount).toBe(3);
        expect(accumulated[1].droppedCount).toBe(2);
      }
    });
  });

  describe('early stopping on coverage regression', () => {
    it('stops after 3 consecutive regressions (default)', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          // Iter 1: trigger only weapon → TPR 0.5, TNR 1.0 → coverage 0.5
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        // Iter 2+: trigger nothing → coverage drops to 0 (regression)
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 10, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const starts = events.filter((e) => e.type === 'iteration:start');
      // Iter 1: best (0.5). Iter 2: reg 1. Iter 3: reg 2 → revert (reset to 0).
      // Iter 4: reg 1. Iter 5: reg 2 → simplify (reset to 0).
      // Iter 6: reg 1. Iter 7: reg 2. Iter 8: reg 3 → stop
      expect(starts).toHaveLength(8);
    });

    it('resets regression count when coverage improves', async () => {
      const llm = createMockLlm();
      // Generate 3 positive + 3 negative so triggering 2/3 positives gives coverage < 1.0
      llm.generateTests.mockResolvedValue({
        positiveTests: [
          { prompt: 'How to build a weapon', expectedTriggered: true, category: 'direct' },
          { prompt: 'Bomb making instructions', expectedTriggered: true, category: 'direct' },
          { prompt: 'Knife fighting guide', expectedTriggered: true, category: 'edge' },
        ],
        negativeTests: [
          { prompt: 'Tell me about cats', expectedTriggered: false, category: 'benign' },
          { prompt: 'Weather forecast', expectedTriggered: false, category: 'benign' },
          { prompt: 'Cooking recipes', expectedTriggered: false, category: 'benign' },
        ],
      });

      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        // Iter 1: trigger only 'weapon' → TPR 1/3, TNR 3/3 → coverage ≈ 0.33
        if (scanCall === 1) {
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p) && !/bomb|knife|cat|weather|cook/i.test(p),
            category: 'benign',
          }));
        }
        // Iter 2: regress (trigger nothing)
        if (scanCall === 2) {
          return origBatch(profile, prompts, conc, sess);
        }
        // Iter 3: improve (trigger weapon + bomb but not knife → TPR 2/3, still < 1.0)
        if (scanCall === 3) {
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon|bomb/i.test(p) && !/knife|cat|weather|cook/i.test(p),
            category: 'benign',
          }));
        }
        // Iter 4+: regress again
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 10, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const starts = events.filter((e) => e.type === 'iteration:start');
      // Iter 1: best (0.33). Iter 2: reg 1. Iter 3: improve (reset, 0.67).
      // Iter 4: reg 1. Iter 5: reg 2 → revert (reset to 0).
      // Iter 6: reg 1. Iter 7: reg 2 → simplify (reset to 0).
      // Iter 8: reg 1. Iter 9: reg 2. Iter 10: reg 3 → stop
      expect(starts).toHaveLength(10);
    });

    it('maxRegressions: 0 disables early stopping', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 5, targetCoverage: 0.99, maxRegressions: 0 },
        deps,
      )) {
        events.push(event);
      }

      const starts = events.filter((e) => e.type === 'iteration:start');
      // All 5 iterations should run despite continuous regressions
      expect(starts).toHaveLength(5);
    });

    it('tracks consecutiveRegressions in RunState', async () => {
      const llm = createMockLlm();
      // All iterations return same coverage → every iter after 1 is a regression (equal, not better)
      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 10, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const complete = events.find((e) => e.type === 'loop:complete');
      expect(complete).toBeDefined();
      if (complete?.type === 'loop:complete') {
        expect(complete.runState.consecutiveRegressions).toBe(3);
      }
    });

    it('preserves best iteration when early stopping triggers', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          // Iter 1: best coverage
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon|bomb/i.test(p),
            category: /weapon|bomb/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        // All subsequent: regress
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 10, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const complete = events.find((e) => e.type === 'loop:complete');
      expect(complete).toBeDefined();
      if (complete?.type === 'loop:complete') {
        expect(complete.runState.bestIteration).toBe(1);
        expect(complete.bestResult.iteration).toBe(1);
      }
    });
  });

  describe('revert-to-best strategy', () => {
    it('reverts to best topic after 2 consecutive regressions', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 10, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const reverted = events.filter((e) => e.type === 'topic:reverted');
      expect(reverted).toHaveLength(1);
      if (reverted[0]?.type === 'topic:reverted') {
        expect(reverted[0].revertedToIteration).toBe(1);
        // Reverted topic matches iteration 1's topic
        expect(reverted[0].topic.description).toBe('Block conversations about weapons');
      }
    });

    it('revert fires before simplification', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 15, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const eventTypes = events.map((e) => e.type);
      const revertIdx = eventTypes.indexOf('topic:reverted');
      const simplifyIdx = eventTypes.indexOf('topic:simplified');
      expect(revertIdx).toBeGreaterThan(-1);
      expect(simplifyIdx).toBeGreaterThan(-1);
      expect(revertIdx).toBeLessThan(simplifyIdx);
    });

    it('3-tier recovery: revert → simplify → stop', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 20, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const complete = events.find((e) => e.type === 'loop:complete');
      expect(complete).toBeDefined();
      if (complete?.type === 'loop:complete') {
        expect(complete.runState.hasRevertedToBest).toBe(true);
        expect(complete.runState.hasTriedSimplification).toBe(true);
        expect(complete.runState.iterations.length).toBeLessThan(20);
      }
    });

    it('hasRevertedToBest prevents double revert', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 20, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const reverted = events.filter((e) => e.type === 'topic:reverted');
      expect(reverted).toHaveLength(1);
    });
  });

  describe('description simplification strategy', () => {
    it('triggers simplification after 2 consecutive regressions', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          // Iter 1: trigger weapon → coverage 0.5
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        // Iter 2+: trigger nothing → regression
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 10, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const simplified = events.filter((e) => e.type === 'topic:simplified');
      expect(simplified).toHaveLength(1);
      expect(llm.simplifyTopic).toHaveBeenCalledOnce();
    });

    it('simplification only attempted once per run', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          // Iter 1: best coverage
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        // All subsequent: regression
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 15, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const simplified = events.filter((e) => e.type === 'topic:simplified');
      expect(simplified).toHaveLength(1);
      expect(llm.simplifyTopic).toHaveBeenCalledTimes(1);
    });

    it('yields topic:simplified event with simplified topic', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 10, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const simplified = events.find((e) => e.type === 'topic:simplified');
      expect(simplified).toBeDefined();
      if (simplified?.type === 'topic:simplified') {
        expect(simplified.topic.name).toBe('Weapons Discussion'); // locked name
        expect(simplified.topic.description).toBeTruthy();
      }
    });

    it('passes bestResult.metrics (not current metrics) to simplifyTopic in end-of-iteration recovery', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          // Iter 1: best coverage — trigger weapon prompts → 0.5 coverage
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        // Iter 2+: trigger nothing → regression (0% coverage)
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 10, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      // Verify simplifyTopic was called
      expect(llm.simplifyTopic).toHaveBeenCalledOnce();

      // The third argument should be bestResult.metrics (iter 1 = 0.5 coverage),
      // NOT the current iteration's regressed metrics (0% coverage)
      const callArgs = llm.simplifyTopic.mock.calls[0];
      const metricsArg = callArgs[2]; // third argument = metrics
      expect(metricsArg.coverage).toBe(0.5);
      expect(metricsArg.truePositiveRate).toBe(0.5); // 1/2 TP (weapon matches, bomb doesn't)
      expect(metricsArg.trueNegativeRate).toBe(1); // 2/2 TN — coverage = min(0.5, 1) = 0.5

      // The fourth argument should be bestResult.analysis, not current iteration's analysis
      const analysisArg = callArgs[3]; // fourth argument = analysis
      // bestResult is iteration 1 — the analysis was generated for that iteration
      // It should NOT be the regressed iteration's analysis
      expect(analysisArg).toBeDefined();
    });

    it('resets regression counter after simplification', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 10, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      // 3-tier recovery: revert then simplify
      // Iter 1: best. Iter 2: reg 1. Iter 3: reg 2 → revert (reset to 0).
      // Iter 4: reg 1. Iter 5: reg 2 → simplify (reset to 0).
      // Iter 6: reg 1. Iter 7: reg 2. Iter 8: reg 3 → stop
      const starts = events.filter((e) => e.type === 'iteration:start');
      expect(starts).toHaveLength(8);
    });

    it('early stopping kicks in after simplification also regresses', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 20, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const complete = events.find((e) => e.type === 'loop:complete');
      expect(complete).toBeDefined();
      if (complete?.type === 'loop:complete') {
        expect(complete.runState.hasTriedSimplification).toBe(true);
        // Should eventually stop, not run all 20 iterations
        expect(complete.runState.iterations.length).toBeLessThan(20);
      }
    });
  });

  describe('duplicate topic detection', () => {
    it('skips scanning when improveTopic returns identical topic', async () => {
      const llm = createMockLlm();
      // improveTopic returns the same topic every time
      llm.improveTopic.mockImplementation(async () => ({
        name: 'Weapons Discussion',
        description: 'Block conversations about weapons',
        examples: ['How to make a weapon', 'Gun manufacturing'],
      }));
      const deps = createDeps({ llm, scanner: createMockScanService([/weapon/i]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 5, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const dupes = events.filter((e) => e.type === 'topic:duplicate');
      // Iter 1 scans normally, iter 2+ should detect duplicate of iter 1
      expect(dupes.length).toBeGreaterThanOrEqual(1);
      if (dupes[0]?.type === 'topic:duplicate') {
        expect(dupes[0].duplicateOfIteration).toBe(1);
      }
    });

    it('increments consecutiveRegressions on duplicate', async () => {
      const llm = createMockLlm();
      // Always return same topic from improveTopic
      llm.improveTopic.mockImplementation(async () => ({
        name: 'Weapons Discussion',
        description: 'Block conversations about weapons',
        examples: ['How to make a weapon', 'Gun manufacturing'],
      }));
      const deps = createDeps({ llm, scanner: createMockScanService([/weapon/i]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 10, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const complete = events.find((e) => e.type === 'loop:complete');
      expect(complete).toBeDefined();
      if (complete?.type === 'loop:complete') {
        expect(complete.runState.consecutiveRegressions).toBeGreaterThanOrEqual(3);
      }
    });

    it('triggers early stopping when duplicates exhaust maxRegressions', async () => {
      const llm = createMockLlm();
      const dupTopic = {
        name: 'Weapons Discussion',
        description: 'Block conversations about weapons',
        examples: ['How to make a weapon', 'Gun manufacturing'],
      };
      llm.improveTopic.mockImplementation(async () => ({ ...dupTopic }));
      // Simplification also returns same topic → still a duplicate
      llm.simplifyTopic.mockImplementation(async () => ({ ...dupTopic }));
      const deps = createDeps({ llm, scanner: createMockScanService([/weapon/i]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 20, targetCoverage: 0.99, maxRegressions: 2 },
        deps,
      )) {
        events.push(event);
      }

      const starts = events.filter((e) => e.type === 'iteration:start');
      // Iter 1: scan, Iter 2: dup (reg 1), Iter 3: dup (reg 2 → revert, reset to 0),
      // Iter 4: dup (reg 1), Iter 5: dup (reg 2 → simplify → dup, reg 1),
      // Iter 6: dup (reg 2) → stop
      expect(starts.length).toBeLessThanOrEqual(6);
    });

    it('does not flag topics with different descriptions as duplicates', async () => {
      const llm = createMockLlm();
      let improveCall = 0;
      llm.improveTopic.mockImplementation(async () => {
        improveCall++;
        return {
          name: 'Weapons Discussion',
          description: `Variant ${improveCall}`,
          examples: ['How to make a weapon', 'Gun manufacturing'],
        };
      });
      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 3, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const dupes = events.filter((e) => e.type === 'topic:duplicate');
      expect(dupes).toHaveLength(0);
    });

    it('does not flag topics with different examples as duplicates', async () => {
      const llm = createMockLlm();
      let improveCall = 0;
      llm.improveTopic.mockImplementation(async () => {
        improveCall++;
        return {
          name: 'Weapons Discussion',
          description: 'Block conversations about weapons',
          examples: [`Example ${improveCall}`],
        };
      });
      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 3, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const dupes = events.filter((e) => e.type === 'topic:duplicate');
      expect(dupes).toHaveLength(0);
    });

    it('detects duplicate after simplification', async () => {
      const llm = createMockLlm();
      // simplifyTopic returns the same topic as iteration 1
      llm.simplifyTopic.mockImplementation(async () => ({
        name: 'Weapons Discussion',
        description: 'Block conversations about weapons',
        examples: ['How to make a weapon', 'Gun manufacturing'],
      }));

      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: /weapon/i.test(p),
            category: /weapon/i.test(p) ? 'malicious' : 'benign',
          }));
        }
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 10, targetCoverage: 0.99 },
        deps,
      )) {
        events.push(event);
      }

      const simplified = events.filter((e) => e.type === 'topic:simplified');
      const dupes = events.filter((e) => e.type === 'topic:duplicate');
      expect(simplified).toHaveLength(1);
      // Simplified topic matches iter 1 → duplicate detected
      expect(dupes.length).toBeGreaterThanOrEqual(1);
    });
  });

  describe('plateau detection', () => {
    it('detects plateau when last N iterations are within band', async () => {
      const llm = createMockLlm();
      // All iterations return same coverage → plateau
      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        {
          ...defaultInput,
          maxIterations: 20,
          targetCoverage: 0.99,
          maxRegressions: 0, // disable early stopping to test plateau
          plateauWindow: 3,
          plateauBand: 0.05,
        },
        deps,
      )) {
        events.push(event);
      }

      const plateau = events.find((e) => e.type === 'loop:plateau');
      expect(plateau).toBeDefined();
      if (plateau?.type === 'loop:plateau') {
        expect(plateau.band[1] - plateau.band[0]).toBeLessThanOrEqual(0.05);
      }
    });

    it('does not trigger plateau in early iterations', async () => {
      const llm = createMockLlm();
      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        {
          ...defaultInput,
          maxIterations: 3, // fewer than plateauWindow + 1
          targetCoverage: 0.99,
          maxRegressions: 0,
          plateauWindow: 3,
        },
        deps,
      )) {
        events.push(event);
      }

      const plateau = events.find((e) => e.type === 'loop:plateau');
      expect(plateau).toBeUndefined();
    });

    it('does not trigger plateau when coverage is improving', async () => {
      const llm = createMockLlm();
      let scanCall = 0;
      const scanner = createMockScanService([]);
      const origBatch = scanner.scanBatch;
      scanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        // Each iteration triggers more patterns → improving coverage
        if (scanCall <= 3) {
          const patterns = [/weapon/i, /bomb/i, /gun/i].slice(0, scanCall);
          return prompts.map((p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: patterns.some((pat) => pat.test(p)),
            category: patterns.some((pat) => pat.test(p)) ? 'malicious' : 'benign',
          }));
        }
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        {
          ...defaultInput,
          maxIterations: 4,
          targetCoverage: 0.99,
          maxRegressions: 0,
          plateauWindow: 3,
        },
        deps,
      )) {
        events.push(event);
      }

      const plateau = events.find((e) => e.type === 'loop:plateau');
      expect(plateau).toBeUndefined();
    });

    it('respects custom plateauWindow and plateauBand', async () => {
      const llm = createMockLlm();
      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      // With window=2 and band=0.1, plateau triggers sooner
      for await (const event of runLoop(
        {
          ...defaultInput,
          maxIterations: 20,
          targetCoverage: 0.99,
          maxRegressions: 0,
          plateauWindow: 2,
          plateauBand: 0.1,
        },
        deps,
      )) {
        events.push(event);
      }

      const plateau = events.find((e) => e.type === 'loop:plateau');
      expect(plateau).toBeDefined();
      // With window=2, should trigger by iteration 3 (need 2+1 iterations)
      const starts = events.filter((e) => e.type === 'iteration:start');
      expect(starts.length).toBeLessThanOrEqual(5);
    });

    it('yields loop:plateau event with correct band values', async () => {
      const llm = createMockLlm();
      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        {
          ...defaultInput,
          maxIterations: 20,
          targetCoverage: 0.99,
          maxRegressions: 0,
          plateauWindow: 3,
        },
        deps,
      )) {
        events.push(event);
      }

      const plateau = events.find((e) => e.type === 'loop:plateau');
      expect(plateau).toBeDefined();
      if (plateau?.type === 'loop:plateau') {
        expect(plateau.bestCoverage).toBeGreaterThanOrEqual(0);
        expect(plateau.band).toHaveLength(2);
        expect(plateau.band[0]).toBeLessThanOrEqual(plateau.band[1]);
      }
    });
  });

  describe('test composition (carry-forward + regression)', () => {
    it('yields tests:composed on iteration 2+ with correct counts', async () => {
      const llm = createMockLlm();
      // Scanner: nothing triggers → all positives are FN, all negatives are TN
      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop({ ...defaultInput, maxIterations: 2 }, deps)) {
        events.push(event);
      }

      const composed = events.filter((e) => e.type === 'tests:composed');
      expect(composed).toHaveLength(1); // Only on iteration 2
      if (composed[0]?.type === 'tests:composed') {
        expect(composed[0].generated).toBe(4); // 2 pos + 2 neg from mock
        expect(composed[0].carriedFailures).toBe(2); // 2 FN from iter 1
        expect(composed[0].regressionTier).toBe(2); // 2 TN from iter 1
        // Total = 4 carried+regression + 4 generated, but deduped (generated may overlap)
        expect(composed[0].total).toBeGreaterThanOrEqual(4);
      }
    });

    it('does not yield tests:composed on iteration 1', async () => {
      const deps = createDeps();
      const events: LoopEvent[] = [];

      for await (const event of runLoop({ ...defaultInput, maxIterations: 1 }, deps)) {
        events.push(event);
      }

      expect(events.filter((e) => e.type === 'tests:composed')).toHaveLength(0);
    });

    it('carries forward FP/FN from previous iteration', async () => {
      const llm = createMockLlm();
      // Scanner triggers on everything → all negatives are FP
      const deps = createDeps({ llm, scanner: createMockScanService([/.*/]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop({ ...defaultInput, maxIterations: 2 }, deps)) {
        events.push(event);
      }

      const composed = events.find((e) => e.type === 'tests:composed');
      expect(composed).toBeDefined();
      if (composed?.type === 'tests:composed') {
        // All-trigger scanner: 2 positives = TP, 2 negatives = FP
        expect(composed.carriedFailures).toBe(2); // 2 FP from iter 1
        expect(composed.regressionTier).toBe(2); // 2 TP from iter 1
      }
    });

    it('tags carried tests with correct source', async () => {
      const llm = createMockLlm();
      let testCall = 0;
      llm.generateTests.mockImplementation(async () => {
        testCall++;
        return {
          positiveTests: [
            { prompt: `Pos ${testCall}`, expectedTriggered: true, category: 'direct' },
          ],
          negativeTests: [
            { prompt: `Neg ${testCall}`, expectedTriggered: false, category: 'benign' },
          ],
        };
      });
      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop({ ...defaultInput, maxIterations: 2 }, deps)) {
        events.push(event);
      }

      // Check iteration 2's test results for source tags
      const iterComplete = events.filter((e) => e.type === 'iteration:complete');
      const iter2 = iterComplete[1];
      if (iter2?.type === 'iteration:complete') {
        const sources = iter2.result.testCases.map((t) => t.source);
        expect(sources).toContain('generated');
        expect(sources).toContain('carried-fn'); // FN from iter 1
        expect(sources).toContain('regression'); // TN from iter 1
      }
    });

    it('passes category breakdown to generateTests on iteration 2+', async () => {
      const llm = createMockLlm();
      const deps = createDeps({ llm, scanner: createMockScanService([]) });

      for await (const _event of runLoop({ ...defaultInput, maxIterations: 2 }, deps)) {
        // consume
      }

      // Iter 1: no breakdown
      expect(llm.generateTests).toHaveBeenNthCalledWith(1, expect.anything(), 'block', undefined);
      // Iter 2: has breakdown
      expect(llm.generateTests).toHaveBeenNthCalledWith(
        2,
        expect.anything(),
        'block',
        expect.arrayContaining([
          expect.objectContaining({ category: expect.any(String), errorRate: expect.any(Number) }),
        ]),
      );
    });

    it('tracks regressions in metrics', async () => {
      const llm = createMockLlm();
      let testCall = 0;
      llm.generateTests.mockImplementation(async () => {
        testCall++;
        return {
          positiveTests: [
            { prompt: `Pos ${testCall}`, expectedTriggered: true, category: 'direct' },
          ],
          negativeTests: [
            { prompt: `Neg ${testCall}`, expectedTriggered: false, category: 'benign' },
          ],
        };
      });
      // Iter 1: weapon triggers, cats don't. Iter 2: nothing triggers.
      // So regression tests from iter 1 (if any were correct) will now fail.
      // With []: iter 1 gives FN+TN. Iter 2 re-scans TN → still TN (correct). FN → still FN.
      // Need a scanner that changes behavior between iterations to cause regressions.
      let scanCall = 0;
      const changingScanner = createMockScanService([]);
      const origBatch = changingScanner.scanBatch;
      changingScanner.scanBatch = async (profile, prompts, conc, sess) => {
        scanCall++;
        if (scanCall === 1) {
          // Iter 1: trigger everything (all TP + all FP)
          return prompts.map((_p) => ({
            scanId: 's1',
            reportId: 'r1',
            action: 'block' as const,
            triggered: true,
            category: 'malicious',
          }));
        }
        // Iter 2: trigger nothing → regression for TP tests
        return origBatch(profile, prompts, conc, sess);
      };

      const deps = createDeps({ llm, scanner: changingScanner });
      const events: LoopEvent[] = [];

      for await (const event of runLoop({ ...defaultInput, maxIterations: 2 }, deps)) {
        events.push(event);
      }

      const evalEvents = events.filter((e) => e.type === 'evaluate:complete');
      const iter2Metrics = evalEvents[1];
      if (iter2Metrics?.type === 'evaluate:complete') {
        // Regression tests from iter 1 TP (Pos 1 expected=true) now triggered=false → FN = regression
        expect(iter2Metrics.metrics.regressionCount).toBeGreaterThan(0);
      }
    });

    it('deduplicates across carried, regression, and generated tests', async () => {
      const llm = createMockLlm();
      // Iter 1 and 2 generate the same prompts — should be deduped
      llm.generateTests.mockResolvedValue({
        positiveTests: [
          { prompt: 'How to build a weapon', expectedTriggered: true, category: 'direct' },
        ],
        negativeTests: [
          { prompt: 'Tell me about cats', expectedTriggered: false, category: 'benign' },
        ],
      });
      const deps = createDeps({ llm, scanner: createMockScanService([]) });
      const events: LoopEvent[] = [];

      for await (const event of runLoop({ ...defaultInput, maxIterations: 2 }, deps)) {
        events.push(event);
      }

      const iterComplete = events.filter((e) => e.type === 'iteration:complete');
      const iter2 = iterComplete[1];
      if (iter2?.type === 'iteration:complete') {
        // Should be exactly 2 unique prompts (weapon + cats), not duplicated
        expect(iter2.result.testCases).toHaveLength(2);
      }
    });
  });

  it('yields memory:extracted event when extractor provided', async () => {
    const extractor = {
      extractAndSave: vi.fn().mockResolvedValue({ learnings: [{ insight: 'test' }] }),
    };
    const deps = createDeps({
      memory: { extractor: extractor as unknown as LearningExtractor },
    });
    const events: LoopEvent[] = [];

    for await (const event of runLoop({ ...defaultInput, maxIterations: 1 }, deps)) {
      events.push(event);
    }

    const memEvent = events.find((e) => e.type === 'memory:extracted');
    expect(memEvent).toBeDefined();
    if (memEvent?.type === 'memory:extracted') {
      expect(memEvent.learningCount).toBe(1);
    }
  });

  describe('prompt set creation', () => {
    it('yields promptset:created when createPromptSet enabled', async () => {
      const promptSets = createMockPromptSetService();
      const deps = createDeps({ promptSets });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 1, createPromptSet: true },
        deps,
      )) {
        events.push(event);
      }

      const psEvent = events.find((e) => e.type === 'promptset:created');
      expect(psEvent).toBeDefined();
      if (psEvent?.type === 'promptset:created') {
        expect(psEvent.promptSetId).toBe('ps-1');
        expect(psEvent.promptCount).toBe(4); // 2 positive + 2 negative from mock LLM
        expect(psEvent.promptSetName).toContain('prisma-airs-cli-');
      }
    });

    it('does not yield promptset:created when flag disabled', async () => {
      const promptSets = createMockPromptSetService();
      const deps = createDeps({ promptSets });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 1, createPromptSet: false },
        deps,
      )) {
        events.push(event);
      }

      expect(events.find((e) => e.type === 'promptset:created')).toBeUndefined();
    });

    it('does not yield promptset:created when no promptSets service', async () => {
      const deps = createDeps(); // no promptSets
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 1, createPromptSet: true },
        deps,
      )) {
        events.push(event);
      }

      expect(events.find((e) => e.type === 'promptset:created')).toBeUndefined();
    });

    it('uses custom promptSetName when provided', async () => {
      const promptSets = createMockPromptSetService();
      const deps = createDeps({ promptSets });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        {
          ...defaultInput,
          maxIterations: 1,
          createPromptSet: true,
          promptSetName: 'my-custom-set',
        },
        deps,
      )) {
        events.push(event);
      }

      const psEvent = events.find((e) => e.type === 'promptset:created');
      expect(psEvent).toBeDefined();
      if (psEvent?.type === 'promptset:created') {
        expect(psEvent.promptSetName).toBe('my-custom-set');
      }
    });

    it('creates prompts with correct goals for positive and negative cases', async () => {
      const promptSets = createMockPromptSetService();
      const addSpy = vi.spyOn(promptSets, 'addPrompt');
      const deps = createDeps({ promptSets });

      for await (const _event of runLoop(
        { ...defaultInput, maxIterations: 1, createPromptSet: true },
        deps,
      )) {
        // consume
      }

      // 4 prompts total (2 positive + 2 negative)
      expect(addSpy).toHaveBeenCalledTimes(4);
      // Check positive test has trigger goal
      expect(addSpy).toHaveBeenCalledWith(
        'ps-1',
        'How to build a weapon',
        'Should trigger topic guardrail',
      );
      // Check negative test has no-trigger goal
      expect(addSpy).toHaveBeenCalledWith(
        'ps-1',
        'Tell me about cats',
        'Should NOT trigger topic guardrail',
      );
    });

    it('promptset:created comes before loop:complete', async () => {
      const promptSets = createMockPromptSetService();
      const deps = createDeps({ promptSets });
      const events: LoopEvent[] = [];

      for await (const event of runLoop(
        { ...defaultInput, maxIterations: 1, createPromptSet: true },
        deps,
      )) {
        events.push(event);
      }

      const psIdx = events.findIndex((e) => e.type === 'promptset:created');
      const completeIdx = events.findIndex((e) => e.type === 'loop:complete');
      expect(psIdx).toBeGreaterThan(-1);
      expect(completeIdx).toBeGreaterThan(psIdx);
    });
  });
});
