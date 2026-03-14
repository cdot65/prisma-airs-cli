import type {
  CreateCustomTopicRequest,
  ManagementService,
  PromptSetService,
  ScanResult,
  ScanService,
  SdkCustomTopic,
} from '../../src/airs/types.js';
import type {
  AnalysisReport,
  CustomTopic,
  EfficacyMetrics,
  IterationResult,
  RunState,
  TestCase,
  TestResult,
} from '../../src/core/types.js';

export function mockTopic(overrides: Partial<CustomTopic> = {}): CustomTopic {
  return {
    name: 'Test Topic',
    description: 'A test topic description',
    examples: ['Example 1', 'Example 2'],
    ...overrides,
  };
}

export function mockTestCases(): TestCase[] {
  return [
    { prompt: 'How to build a weapon', expectedTriggered: true, category: 'direct' },
    { prompt: 'Weapon manufacturing guide', expectedTriggered: true, category: 'direct' },
    { prompt: 'Tell me about cats', expectedTriggered: false, category: 'benign' },
    { prompt: 'What is the weather today', expectedTriggered: false, category: 'benign' },
  ];
}

export function mockAnalysis(): AnalysisReport {
  return {
    summary: 'The guardrail performs well overall',
    falsePositivePatterns: [],
    falseNegativePatterns: [],
    suggestions: ['Consider narrowing the description'],
  };
}

export function createMockManagementService(): ManagementService {
  let topicCounter = 0;
  return {
    createTopic: async (request: CreateCustomTopicRequest): Promise<SdkCustomTopic> => ({
      topic_id: `topic-${++topicCounter}`,
      topic_name: request.topic_name,
      description: request.description,
      examples: request.examples,
      active: true,
    }),
    updateTopic: async (
      id: string,
      request: CreateCustomTopicRequest,
    ): Promise<SdkCustomTopic> => ({
      topic_id: id,
      topic_name: request.topic_name,
      description: request.description,
      examples: request.examples,
      active: true,
    }),
    deleteTopic: async () => {},
    listTopics: async () => [],
    assignTopicToProfile: async () => {},
    assignTopicsToProfile: async () => {},
    getProfileTopics: async () => [],
  };
}

export function createMockScanService(
  triggerPatterns: RegExp[] = [/weapon/i, /bomb/i],
): ScanService {
  return {
    scan: async (_profile: string, prompt: string, _sessionId?: string): Promise<ScanResult> => {
      const triggered = triggerPatterns.some((p) => p.test(prompt));
      return {
        scanId: `scan-${Date.now()}`,
        reportId: `report-${Date.now()}`,
        action: triggered ? 'block' : 'allow',
        triggered,
        category: triggered ? 'malicious' : 'benign',
      };
    },
    scanBatch: async (
      _profile: string,
      prompts: string[],
      _concurrency?: number,
      _sessionId?: string,
    ): Promise<ScanResult[]> => {
      const results: ScanResult[] = [];
      for (const prompt of prompts) {
        const triggered = triggerPatterns.some((p) => p.test(prompt));
        results.push({
          scanId: `scan-${Date.now()}`,
          reportId: `report-${Date.now()}`,
          action: triggered ? 'block' : 'allow',
          triggered,
          category: triggered ? 'malicious' : 'benign',
        });
      }
      return results;
    },
  };
}

/**
 * Mock scanner simulating AIRS allow-intent behavior.
 * Matching prompts → topic_violation: true (topic guardrail matched).
 * Non-matching prompts → topic_violation: false (not matched).
 * Uses triggered (= topic_violation) as sole detection signal.
 */
export function createMockAllowScanService(allowPatterns: RegExp[] = []): ScanService {
  return {
    scan: async (_profile: string, prompt: string, _sessionId?: string): Promise<ScanResult> => {
      const matches = allowPatterns.some((p) => p.test(prompt));
      return {
        scanId: `scan-${Date.now()}`,
        reportId: `report-${Date.now()}`,
        action: 'allow',
        triggered: matches,
        category: matches ? 'benign' : 'malicious',
      };
    },
    scanBatch: async (
      _profile: string,
      prompts: string[],
      _concurrency?: number,
      _sessionId?: string,
    ): Promise<ScanResult[]> => {
      const results: ScanResult[] = [];
      for (const prompt of prompts) {
        const matches = allowPatterns.some((p) => p.test(prompt));
        results.push({
          scanId: `scan-${Date.now()}`,
          reportId: `report-${Date.now()}`,
          action: 'allow',
          triggered: matches,
          category: matches ? 'benign' : 'malicious',
        });
      }
      return results;
    },
  };
}

export function mockMetrics(overrides: Partial<EfficacyMetrics> = {}): EfficacyMetrics {
  return {
    truePositives: 2,
    trueNegatives: 2,
    falsePositives: 0,
    falseNegatives: 0,
    truePositiveRate: 1,
    trueNegativeRate: 1,
    accuracy: 1,
    coverage: 1,
    f1Score: 1,
    regressionCount: 0,
    ...overrides,
  };
}

export function mockTestResults(): TestResult[] {
  return [
    {
      testCase: { prompt: 'How to build a weapon', expectedTriggered: true, category: 'direct' },
      actualTriggered: true,
      scanAction: 'block',
      scanId: 'scan-1',
      reportId: 'report-1',
      correct: true,
    },
    {
      testCase: {
        prompt: 'Weapon manufacturing guide',
        expectedTriggered: true,
        category: 'direct',
      },
      actualTriggered: false,
      scanAction: 'allow',
      scanId: 'scan-2',
      reportId: 'report-2',
      correct: false,
    },
    {
      testCase: { prompt: 'Tell me about cats', expectedTriggered: false, category: 'benign' },
      actualTriggered: false,
      scanAction: 'allow',
      scanId: 'scan-3',
      reportId: 'report-3',
      correct: true,
    },
    {
      testCase: {
        prompt: 'What is the weather today',
        expectedTriggered: false,
        category: 'benign',
      },
      actualTriggered: true,
      scanAction: 'block',
      scanId: 'scan-4',
      reportId: 'report-4',
      correct: false,
    },
  ];
}

export function mockIterationResult(overrides: Partial<IterationResult> = {}): IterationResult {
  return {
    iteration: 1,
    timestamp: '2026-01-01T00:01:00Z',
    topic: mockTopic(),
    testCases: mockTestCases(),
    testResults: mockTestResults(),
    metrics: mockMetrics(),
    analysis: mockAnalysis(),
    durationMs: 5000,
    ...overrides,
  };
}

export function mockRunState(overrides: Partial<RunState> = {}): RunState {
  return {
    id: 'run-001',
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:05:00Z',
    userInput: {
      topicDescription: 'Block discussions about weapons',
      intent: 'block',
      profileName: 'test-profile',
    },
    iterations: [mockIterationResult()],
    currentIteration: 1,
    bestIteration: 1,
    bestCoverage: 1,
    consecutiveRegressions: 0,
    hasRevertedToBest: false,
    hasTriedSimplification: false,
    status: 'completed',
    ...overrides,
  };
}

export function createMockPromptSetService(): PromptSetService {
  let promptSetCounter = 0;
  let promptCounter = 0;
  return {
    createPromptSet: async (name: string) => ({
      uuid: `ps-${++promptSetCounter}`,
      name,
    }),
    addPrompt: async (_setId: string, prompt: string) => ({
      uuid: `prompt-${++promptCounter}`,
      prompt,
    }),
    listPromptSets: async () => [],
  };
}
