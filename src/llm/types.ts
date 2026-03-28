import type {
  AnalysisReport,
  CategoryBreakdown,
  CustomTopic,
  EfficacyMetrics,
  TestCase,
  TestResult,
} from '../core/types.js';

/** Contract for LLM operations used by the audit runner. */
export interface LlmService {
  generateTopic(description: string, intent: string, seeds?: string[]): Promise<CustomTopic>;
  generateTests(
    topic: CustomTopic,
    intent: string,
    categoryBreakdown?: CategoryBreakdown[],
  ): Promise<{ positiveTests: TestCase[]; negativeTests: TestCase[] }>;
  analyzeResults(
    topic: CustomTopic,
    results: TestResult[],
    metrics: EfficacyMetrics,
    intent: string,
  ): Promise<AnalysisReport>;
  simplifyTopic(
    currentTopic: CustomTopic,
    bestTopic: CustomTopic,
    metrics: EfficacyMetrics,
    analysis: AnalysisReport,
    intent: string,
  ): Promise<CustomTopic>;
  generateCompanionTopic(blockTopicName: string, blockDescription: string): Promise<CustomTopic>;
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
