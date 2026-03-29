/**
 * Core domain types — shared across all Prisma AIRS CLI modules.
 */

// ---------------------------------------------------------------------------
// Topic definition — the guardrail artifact generated and refined by the loop
// ---------------------------------------------------------------------------
export interface CustomTopic {
  name: string;
  description: string;
  examples: string[];
}

// ---------------------------------------------------------------------------
// User input — interactive prompt answers that seed a generation run
// ---------------------------------------------------------------------------
export interface UserInput {
  topicDescription: string;
  intent: 'allow' | 'block';
  seedExamples?: string[];
  profileName: string;
  maxIterations?: number;
  targetCoverage?: number;
  maxRegressions?: number;
  plateauWindow?: number;
  plateauBand?: number;
  accumulateTests?: boolean;
  maxAccumulatedTests?: number;
  createPromptSet?: boolean;
  promptSetName?: string;
  /** Max AIRS scan API calls per second. Undefined = unlimited. */
  scanRate?: number;
}

// ---------------------------------------------------------------------------
// Testing — prompt test cases and their scan results
// ---------------------------------------------------------------------------
export interface TestCase {
  prompt: string;
  expectedTriggered: boolean;
  category: string;
  /** How this test entered the suite. Default: 'generated'. */
  source?: 'generated' | 'carried-fp' | 'carried-fn' | 'regression';
  /** Which topic this test targets (used by audit). */
  targetTopic?: string;
}

/** Per-category error breakdown from a previous iteration's results. */
export interface CategoryBreakdown {
  category: string;
  total: number;
  fp: number;
  fn: number;
  errorRate: number;
}

export interface TestResult {
  testCase: TestCase;
  actualTriggered: boolean;
  scanAction: 'allow' | 'block';
  scanId: string;
  reportId: string;
  correct: boolean;
}

// ---------------------------------------------------------------------------
// Evaluation — efficacy metrics and LLM analysis of scan results
// ---------------------------------------------------------------------------
export interface EfficacyMetrics {
  truePositives: number;
  trueNegatives: number;
  falsePositives: number;
  falseNegatives: number;
  truePositiveRate: number;
  trueNegativeRate: number;
  accuracy: number;
  coverage: number;
  f1Score: number;
  /** Count of regression-tier tests that failed (previously correct, now wrong). */
  regressionCount: number;
}

export interface AnalysisReport {
  summary: string;
  falsePositivePatterns: string[];
  falseNegativePatterns: string[];
  suggestions: string[];
}

// ---------------------------------------------------------------------------
// Iteration & run state — tracks progress across the refinement loop
// ---------------------------------------------------------------------------
export interface IterationResult {
  iteration: number;
  timestamp: string;
  topic: CustomTopic;
  testCases: TestCase[];
  testResults: TestResult[];
  metrics: EfficacyMetrics;
  analysis: AnalysisReport;
  durationMs: number;
}

export interface RunState {
  id: string;
  createdAt: string;
  updatedAt: string;
  userInput: UserInput;
  iterations: IterationResult[];
  currentIteration: number;
  bestIteration: number;
  bestCoverage: number;
  consecutiveRegressions: number;
  hasRevertedToBest: boolean;
  hasTriedSimplification: boolean;
  /** Companion allow topic created for block-intent two-phase generation. */
  companionTopic?: CustomTopic;
  status: 'running' | 'paused' | 'completed' | 'failed';
}
