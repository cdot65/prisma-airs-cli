import type { BaseChatModel } from '@langchain/core/language_models/chat_models';
import {
  byteLen,
  MAX_COMBINED_LENGTH,
  MAX_DESCRIPTION_LENGTH,
  MAX_EXAMPLE_LENGTH,
  MAX_EXAMPLES,
  MAX_NAME_LENGTH,
  validateTopic,
} from '../core/constraints.js';
import type {
  AnalysisReport,
  CategoryBreakdown,
  CustomTopic,
  EfficacyMetrics,
  TestCase,
  TestResult,
} from '../core/types.js';
import type { MemoryInjector } from '../memory/injector.js';
import { analyzeResultsPrompt } from './prompts/analyze-results.js';
import { generateCompanionPrompt } from './prompts/generate-companion.js';
import { generateTestsPrompt } from './prompts/generate-tests.js';
import { buildSeedExamplesSection, generateTopicPrompt } from './prompts/generate-topic.js';
import { improveTopicPrompt } from './prompts/improve-topic.js';
import { simplifyTopicPrompt } from './prompts/simplify-topic.js';
import {
  type AnalysisReportOutput,
  AnalysisReportSchema,
  type CustomTopicOutput,
  CustomTopicSchema,
  type TestSuiteOutput,
  TestSuiteSchema,
} from './schemas.js';
import type { LlmService } from './types.js';

const MAX_RETRIES = 3;

/** Truncate a string so its UTF-8 byte length does not exceed maxBytes. */
function sliceToBytes(s: string, maxBytes: number): string {
  while (Buffer.byteLength(s, 'utf8') > maxBytes) {
    s = s.slice(0, -1);
  }
  return s;
}

/** Clamp topic fields to fit Prisma AIRS constraints (byte-aware, including combined limit) */
function clampTopic(topic: CustomTopicOutput): CustomTopicOutput {
  const name = sliceToBytes(topic.name, MAX_NAME_LENGTH);
  let description = sliceToBytes(topic.description, MAX_DESCRIPTION_LENGTH);
  const examples = topic.examples
    .slice(0, MAX_EXAMPLES)
    .map((e) => sliceToBytes(e, MAX_EXAMPLE_LENGTH));

  // Enforce combined byte-length limit by dropping examples from the end, then trimming description
  const combined = () =>
    byteLen(name) + byteLen(description) + examples.reduce((s, e) => s + byteLen(e), 0);
  while (combined() > MAX_COMBINED_LENGTH && examples.length > 1) {
    examples.pop();
  }
  /* v8 ignore next 4 -- unreachable with current MAX constants (100+250+250=600 < 1000) */
  if (combined() > MAX_COMBINED_LENGTH) {
    const overflow = combined() - MAX_COMBINED_LENGTH;
    description = sliceToBytes(description, byteLen(description) - overflow);
  }

  return { name, description, examples };
}

/**
 * LLM service implementation using LangChain structured output.
 * Handles topic generation, test creation, result analysis, and topic improvement
 * with automatic retry and AIRS constraint clamping.
 */
export class LangChainLlmService implements LlmService {
  private memorySection = '';

  constructor(
    private model: BaseChatModel,
    private memoryInjector?: MemoryInjector,
  ) {}

  async loadMemory(topicDescription: string): Promise<number> {
    if (!this.memoryInjector) return 0;
    this.memorySection = await this.memoryInjector.buildMemorySection(topicDescription);
    // Count lines starting with "- [" as learning count
    return this.memorySection.split('\n').filter((l) => l.startsWith('- [')).length;
  }

  async generateTopic(description: string, intent: string, seeds?: string[]): Promise<CustomTopic> {
    const structured = this.model.withStructuredOutput(CustomTopicSchema);
    const chain = generateTopicPrompt.pipe(structured);

    for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
      try {
        const raw = await chain.invoke({
          topicDescription: description,
          intent,
          seedExamplesSection: buildSeedExamplesSection(seeds),
          memorySection: this.memorySection,
        });
        const result = clampTopic(raw as unknown as CustomTopicOutput);

        const errors = validateTopic(result);
        if (errors.length === 0) return result;

        if (attempt === MAX_RETRIES - 1) {
          throw new Error(
            `LLM output violates constraints after ${MAX_RETRIES} attempts: ${errors.map((e) => e.message).join(', ')}`,
          );
        }
      } catch (err) {
        if (attempt === MAX_RETRIES - 1) throw err;
      }
    }

    /* v8 ignore next */
    throw new Error('Unreachable');
  }

  async generateCompanionTopic(
    blockTopicName: string,
    blockDescription: string,
  ): Promise<CustomTopic> {
    const structured = this.model.withStructuredOutput(CustomTopicSchema);
    const chain = generateCompanionPrompt.pipe(structured);

    for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
      try {
        const raw = await chain.invoke({
          blockTopicName,
          blockTopicDescription: blockDescription,
        });
        const result = clampTopic(raw as unknown as CustomTopicOutput);

        const errors = validateTopic(result);
        if (errors.length === 0) return result;

        if (attempt === MAX_RETRIES - 1) {
          throw new Error(
            `LLM output violates constraints after ${MAX_RETRIES} attempts: ${errors.map((e) => e.message).join(', ')}`,
          );
        }
      } catch (err) {
        if (attempt === MAX_RETRIES - 1) throw err;
      }
    }

    /* v8 ignore next */
    throw new Error('Unreachable');
  }

  async generateTests(
    topic: CustomTopic,
    intent: string,
    categoryBreakdown?: CategoryBreakdown[],
  ): Promise<{ positiveTests: TestCase[]; negativeTests: TestCase[] }> {
    const structured = this.model.withStructuredOutput(TestSuiteSchema);
    const chain = generateTestsPrompt.pipe(structured);

    let categoryBreakdownSection = '';
    if (categoryBreakdown && categoryBreakdown.length > 0) {
      const lines = categoryBreakdown.map(
        (c) =>
          `- ${c.category}: ${c.fp + c.fn}/${c.total} errors (${(c.errorRate * 100).toFixed(0)}%) — ${c.fp} FP, ${c.fn} FN`,
      );
      categoryBreakdownSection = `\nPrevious iteration per-category error rates (generate proportionally more tests for high-error categories):\n${lines.join('\n')}\n`;
    }

    for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
      try {
        const raw = await chain.invoke({
          topicName: topic.name,
          topicDescription: topic.description,
          topicExamples:
            topic.examples.length > 0
              ? topic.examples.map((e, i) => `${i + 1}. ${e}`).join('\n')
              : 'None (description-only mode)',
          exampleCount: topic.examples.length,
          intent,
          categoryBreakdownSection,
          memorySection: this.memorySection,
        });
        return raw as unknown as TestSuiteOutput;
      } catch (err) {
        if (attempt === MAX_RETRIES - 1) throw err;
      }
    }

    /* v8 ignore next */
    throw new Error('Unreachable');
  }

  async analyzeResults(
    topic: CustomTopic,
    results: TestResult[],
    metrics: EfficacyMetrics,
    intent: string,
  ): Promise<AnalysisReport> {
    const structured = this.model.withStructuredOutput(AnalysisReportSchema);
    const chain = analyzeResultsPrompt.pipe(structured);

    const fps = results.filter((r) => !r.testCase.expectedTriggered && r.actualTriggered);
    const fns = results.filter((r) => r.testCase.expectedTriggered && !r.actualTriggered);

    for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
      try {
        const raw = await chain.invoke({
          topicName: topic.name,
          topicDescription: topic.description,
          topicExamples:
            topic.examples.length > 0 ? topic.examples.join(', ') : 'None (description-only)',
          exampleCount: topic.examples.length,
          tpr: `${(metrics.truePositiveRate * 100).toFixed(1)}%`,
          tnr: `${(metrics.trueNegativeRate * 100).toFixed(1)}%`,
          accuracy: `${(metrics.accuracy * 100).toFixed(1)}%`,
          coverage: `${(metrics.coverage * 100).toFixed(1)}%`,
          falsePositives:
            fps.length > 0
              ? fps.map((r) => `- "${r.testCase.prompt}" (${r.testCase.category})`).join('\n')
              : 'None',
          falseNegatives:
            fns.length > 0
              ? fns.map((r) => `- "${r.testCase.prompt}" (${r.testCase.category})`).join('\n')
              : 'None',
          intent,
          memorySection: this.memorySection,
        });
        return raw as unknown as AnalysisReportOutput;
      } catch (err) {
        if (attempt === MAX_RETRIES - 1) throw err;
      }
    }

    /* v8 ignore next */
    throw new Error('Unreachable');
  }

  async improveTopic(
    topic: CustomTopic,
    metrics: EfficacyMetrics,
    analysis: AnalysisReport,
    results: TestResult[],
    iteration: number,
    targetCoverage: number,
    intent: string,
    bestContext?: { bestCoverage: number; bestIteration: number; bestTopic?: CustomTopic },
  ): Promise<CustomTopic> {
    const structured = this.model.withStructuredOutput(CustomTopicSchema);
    const chain = improveTopicPrompt.pipe(structured);

    const fps = results.filter((r) => !r.testCase.expectedTriggered && r.actualTriggered);
    const fns = results.filter((r) => r.testCase.expectedTriggered && !r.actualTriggered);

    // Build best-context template variables
    const bestCoverage = bestContext
      ? `${(bestContext.bestCoverage * 100).toFixed(1)}%`
      : `${(metrics.coverage * 100).toFixed(1)}%`;
    const bestIteration = bestContext ? bestContext.bestIteration : iteration;

    let bestTopicSection = '';
    if (bestContext?.bestTopic && metrics.coverage < bestContext.bestCoverage) {
      const bt = bestContext.bestTopic;
      bestTopicSection = `REGRESSION WARNING: Coverage has dropped from ${(bestContext.bestCoverage * 100).toFixed(1)}% (iteration ${bestContext.bestIteration}) to ${(metrics.coverage * 100).toFixed(1)}%.
The best-performing definition was:
  Description: ${bt.description}
  Examples: ${bt.examples.join(', ')}
Consider reverting toward this simpler definition rather than adding more specificity.`;
    }

    for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
      try {
        const raw = await chain.invoke({
          currentName: topic.name,
          currentDescription: topic.description,
          currentExamples:
            topic.examples.length > 0 ? topic.examples.join(', ') : 'None (description-only)',
          exampleCount: topic.examples.length,
          iteration,
          coverage: `${(metrics.coverage * 100).toFixed(1)}%`,
          targetCoverage: `${(targetCoverage * 100).toFixed(1)}%`,
          tpr: `${(metrics.truePositiveRate * 100).toFixed(1)}%`,
          tnr: `${(metrics.trueNegativeRate * 100).toFixed(1)}%`,
          accuracy: `${(metrics.accuracy * 100).toFixed(1)}%`,
          bestCoverage,
          bestIteration,
          bestTopicSection,
          analysisSummary: analysis.summary,
          fpPatterns: analysis.falsePositivePatterns.join('; ') || 'None',
          fnPatterns: analysis.falseNegativePatterns.join('; ') || 'None',
          specificFPs: fps.map((r) => `- "${r.testCase.prompt}"`).join('\n') || 'None',
          specificFNs: fns.map((r) => `- "${r.testCase.prompt}"`).join('\n') || 'None',
          suggestions: analysis.suggestions.join('; '),
          intent,
          memorySection: this.memorySection,
        });
        const result = clampTopic(raw as unknown as CustomTopicOutput);

        const errors = validateTopic(result);
        if (errors.length === 0) return result;

        if (attempt === MAX_RETRIES - 1) {
          throw new Error(
            `LLM output violates constraints after ${MAX_RETRIES} attempts: ${errors.map((e) => e.message).join(', ')}`,
          );
        }
      } catch (err) {
        if (attempt === MAX_RETRIES - 1) throw err;
      }
    }

    /* v8 ignore next */
    throw new Error('Unreachable');
  }

  async simplifyTopic(
    currentTopic: CustomTopic,
    bestTopic: CustomTopic,
    metrics: EfficacyMetrics,
    _analysis: AnalysisReport,
    intent: string,
  ): Promise<CustomTopic> {
    const structured = this.model.withStructuredOutput(CustomTopicSchema);
    const chain = simplifyTopicPrompt.pipe(structured);

    for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
      try {
        const raw = await chain.invoke({
          currentName: currentTopic.name,
          currentDescription: currentTopic.description,
          currentExamples:
            currentTopic.examples.length > 0
              ? currentTopic.examples.join(', ')
              : 'None (description-only)',
          bestCoverage: `${(metrics.coverage * 100).toFixed(1)}%`,
          bestDescription: bestTopic.description,
          bestExamples:
            bestTopic.examples.length > 0
              ? bestTopic.examples.join(', ')
              : 'None (description-only)',
          coverage: `${(metrics.coverage * 100).toFixed(1)}%`,
          tpr: `${(metrics.truePositiveRate * 100).toFixed(1)}%`,
          tnr: `${(metrics.trueNegativeRate * 100).toFixed(1)}%`,
          intent,
          memorySection: this.memorySection,
        });
        const result = clampTopic(raw as unknown as CustomTopicOutput);

        const errors = validateTopic(result);
        if (errors.length === 0) return result;

        if (attempt === MAX_RETRIES - 1) {
          throw new Error(
            `LLM output violates constraints after ${MAX_RETRIES} attempts: ${errors.map((e) => e.message).join(', ')}`,
          );
        }
      } catch (err) {
        if (attempt === MAX_RETRIES - 1) throw err;
      }
    }

    /* v8 ignore next */
    throw new Error('Unreachable');
  }
}
